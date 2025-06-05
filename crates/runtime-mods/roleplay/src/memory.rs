use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use sqlx::PgPool;

use voda_database::{
    SqlxCrud, SqlxPopulateId,
    QueryCriteria, OrderDirection, SqlxFilterQuery
};
use voda_common::{get_current_timestamp, CryptoHash};

use crate::RoleplaySession;

use super::message::RoleplayMessage;

use voda_runtime::Memory;

#[derive(Clone)]
pub struct RoleplayRawMemory {
    db: Arc<PgPool>,
}

impl RoleplayRawMemory {
    pub fn new(db: Arc<PgPool>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl Memory for RoleplayRawMemory {
    type MessageType = RoleplayMessage;

    async fn initialize(&self) -> Result<()> { Ok(()) }

    async fn add_messages(&self, messages: &[RoleplayMessage]) -> Result<()> {
        let mut tx = self.db.begin().await?;

        for message in messages {
            let mut m = message.clone();

            let session_id_hex = m.session_id.to_hex_string();
            let criteria = QueryCriteria::new()
                .add_valued_filter("id", "=", session_id_hex)?;
        
            let mut session = RoleplaySession::find_one_by_criteria(criteria, &mut *tx)
                .await?
                .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Session not found"))?;

            m.sql_populate_id()?;
            let created_m = m.create(&mut *tx).await?;

            let current_timestamp = get_current_timestamp();
            session.append_message_to_history(&created_m.id, current_timestamp, &mut *tx).await?;
            session.update(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_one(&self, message_id: &CryptoHash) -> Result<Option<RoleplayMessage>> {
        let message_id_hex = message_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("id", "=", message_id_hex)?;
        
        let message = RoleplayMessage::find_one_by_criteria(criteria, &*self.db).await?;
        Ok(message)
    }

    async fn get_all(&self, user_id: &CryptoHash, limit: u64, offset: u64) -> Result<Vec<RoleplayMessage>> {
        let user_id_hex = user_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?
            .order_by("created_at", OrderDirection::Desc)?
            .limit(limit as i64)?
            .offset(offset as i64)?;

        let messages = RoleplayMessage::find_by_criteria(criteria, &*self.db).await?;
        Ok(messages)
    }

    async fn search(&self, message: &RoleplayMessage, _limit: u64, _offset: u64) -> Result<Vec<RoleplayMessage>> {
        let mut tx = self.db.begin().await?;

        let criteria = QueryCriteria::new()
            .add_valued_filter("id", "=", message.session_id.to_hex_string())?;
        let session = RoleplaySession::find_one_by_criteria(criteria, &mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::search] Session not found"))?;

        let history = session.fetch_history(&mut *tx).await?;

        tx.commit().await?;
        Ok(history)
    }

    async fn update(&self, messages: &[Self::MessageType]) -> Result<()> {
        let mut tx = self.db.begin().await?;

        for message in messages {
            let m = message.clone();
            m.update(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete(&self, message_ids: &[CryptoHash]) -> Result<()> {
        let sqlx_ids = message_ids.iter().map(|id| id.to_hex_string()).collect::<Vec<_>>();
        let criteria = QueryCriteria::new()
            .add_filter("id", " = ANY($1)", Some(sqlx_ids))?;

        RoleplayMessage::delete_by_criteria(criteria, &*self.db).await?;
        Ok(())
    }

    async fn reset(&self, user_id: &CryptoHash) -> Result<()> {
        let mut tx = self.db.begin().await?;
        let user_id_hex = user_id.to_hex_string();
        let criteria_session = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex.clone())?;
        RoleplaySession::delete_by_criteria(criteria_session, &mut *tx).await?;

        let criteria_message = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?;
        RoleplayMessage::delete_by_criteria(criteria_message, &mut *tx).await?;

        tx.commit().await?;
        Ok(())
    }
}

impl RoleplayRawMemory {
    pub async fn find_public_conversations_by_character(
        &self, character_id: &CryptoHash, limit: u64, offset: u64
    ) -> Result<Vec<RoleplaySession>> {
        let criteria = QueryCriteria::new()
            .add_valued_filter("character_id", "=", character_id.to_hex_string())?
            .add_valued_filter("public", "=", true)?
            .order_by("updated_at", OrderDirection::Desc)?
            .limit(limit as i64)?
            .offset(offset as i64)?;

        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;
        Ok(sessions)
    }

    pub async fn find_latest_conversations(
        &self, user_id: &CryptoHash, character_id: &CryptoHash, limit: u64
    ) -> Result<Vec<RoleplaySession>> {
        let user_id_hex = user_id.to_hex_string();
        let character_id_hex = character_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?
            .add_valued_filter("character_id", "=", character_id_hex)?
            .order_by("updated_at", OrderDirection::Desc)?
            .limit(limit as i64)?;

        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;
        Ok(sessions)
    }

    pub async fn find_character_list_of_user(
        &self, user_id: &CryptoHash
    ) -> Result<HashMap<CryptoHash, usize>> {
        let user_id_hex = user_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?;
        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;

        let mut character_list = HashMap::new();
        for session in sessions {
            character_list.entry(session.character_id).and_modify(|count| *count += 1).or_insert(1);
        }

        Ok(character_list)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use sqlx::PgPool;

    use voda_common::{CryptoHash, get_current_timestamp};
    use voda_database::{init_db_pool, QueryCriteria, SqlxCrud, SqlxFilterQuery, SqlxPopulateId};
    use voda_runtime::{User, Memory, UserRole, MessageRole, MessageType }; 

    use crate::character::{CharacterStatus, CharacterGender, CharacterLanguage, CharacterFeature};
    use crate::{RoleplayRawMemory, RoleplaySession, RoleplayMessage, Character};

    init_db_pool!(User, Character, RoleplaySession, RoleplayMessage);

    async fn setup_test_environment() -> Result<(Arc<PgPool>, RoleplayRawMemory, User, Character)> {
        let pool = Arc::new(connect().await.clone());
        let memory = RoleplayRawMemory::new(pool.clone());

        let user_id_str = format!("test_user_{}", CryptoHash::random().to_hex_string());
        let mut test_user_builder = User {
            id: CryptoHash::default(), 
            user_id: user_id_str.clone(),
            user_aka: "Test User Aka".to_string(),
            role: UserRole::User,
            provider: "test_provider".to_string(),
            last_active: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        test_user_builder.sql_populate_id()?;
        let test_user = test_user_builder.create(&*pool).await?;

        let mut test_character_builder = Character {
            id: CryptoHash::default(), 
            name: "Test Character".to_string(),
            description: "A character for testing".to_string(),
            creator: test_user.id.clone(),
            reviewed_by: None,
            version: 1,
            status: CharacterStatus::Published,
            gender: CharacterGender::Others("TestGender".to_string()),
            language: CharacterLanguage::English,
            features: vec![CharacterFeature::Roleplay],
            prompts_scenario: "Test Scenario".to_string(),
            prompts_personality: "Test Personality".to_string(),
            prompts_example_dialogue: "Test Dialogue".to_string(),
            prompts_first_message: "Hello from Test Character!".to_string(),
            tags: vec!["test".to_string()],
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
            published_at: get_current_timestamp(),
        };
        test_character_builder.sql_populate_id()?;
        let test_character = test_character_builder.create(&*pool).await?;

        Ok((pool, memory, test_user, test_character))
    }

    async fn create_test_session(
        pool: &PgPool,
        owner_user: &User, 
        character: &Character,
        is_public: bool,
    ) -> Result<RoleplaySession> {
        let mut session_builder = RoleplaySession {
            id: CryptoHash::default(),
            public: is_public,
            owner: owner_user.id.clone(),
            character_id: character.id.clone(),
            history: vec![],
            updated_at: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        session_builder.sql_populate_id()?;
        session_builder.create(&*pool).await.map_err(anyhow::Error::from)
    }

    fn create_test_message(
        session_id: CryptoHash,
        owner_id: CryptoHash, 
        character_id: CryptoHash, 
        role: MessageRole,    
        content: &str,
    ) -> RoleplayMessage {
        RoleplayMessage {
            id: CryptoHash::default(), 
            session_id,
            owner: owner_id,
            character_id, 
            role,
            content_type: MessageType::Text, 
            content: content.to_string(),
            created_at: get_current_timestamp(),
        }
    }

    #[tokio::test]
    async fn test_initialize() -> Result<()> {
        let (_pool, memory, _user, _character) = setup_test_environment().await?;
        assert!(memory.initialize().await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_add_messages_and_get_one() -> Result<()> {
        let (pool, memory, user, character) = setup_test_environment().await?;
        let session = create_test_session(&pool, &user, &character, true).await?;

        let system_prompt = create_test_message(
            session.id.clone(),
            character.creator.clone(), 
            character.id.clone(),      
            MessageRole::System,
            "System: Welcome!",
        );
        let user_message = create_test_message(
            session.id.clone(),
            user.id.clone(),
            character.id.clone(),
            MessageRole::User,
            "User: Hello!",
        );
        let ai_response = create_test_message(
            session.id.clone(),
            character.creator.clone(), 
            character.id.clone(),
            MessageRole::Assistant, 
            "AI: Hi there!",
        );

        let messages_to_add = vec![system_prompt.clone(), user_message.clone(), ai_response.clone()];
        memory.add_messages(&messages_to_add).await?;

        eprintln!("test_add_messages_and_get_one: Added messages. Fetching session to check history...");
        let criteria_session_updated = QueryCriteria::new().add_valued_filter("id", "=", session.id.to_hex_string())?;
        let updated_session = RoleplaySession::find_one_by_criteria(criteria_session_updated, &*pool).await?.unwrap();
        eprintln!("test_add_messages_and_get_one: Updated session: {:?}", updated_session);
        
        assert_eq!(updated_session.history.len(), 3);
        assert!(updated_session.updated_at >= session.updated_at);

        let mut populated_message_ids = Vec::new();
        for msg_id_in_history in &updated_session.history {
             eprintln!("test_add_messages_and_get_one: Fetching message with ID from history: {:?}", msg_id_in_history);
             let fetched_msg = memory.get_one(msg_id_in_history).await?.unwrap();
             eprintln!("test_add_messages_and_get_one: Fetched message: {:?}", fetched_msg);
             populated_message_ids.push(fetched_msg.id.clone());
             assert_eq!(fetched_msg.session_id, session.id);
        }

        let fetched_system_prompt = memory.get_one(&populated_message_ids[0]).await?.unwrap();
        assert_eq!(fetched_system_prompt.content, system_prompt.content);
        assert_eq!(fetched_system_prompt.role, MessageRole::System);

        let fetched_user_message = memory.get_one(&populated_message_ids[1]).await?.unwrap();
        assert_eq!(fetched_user_message.content, user_message.content);
        assert_eq!(fetched_user_message.role, MessageRole::User);

        let fetched_ai_response = memory.get_one(&populated_message_ids[2]).await?.unwrap();
        assert_eq!(fetched_ai_response.content, ai_response.content);
        assert_eq!(fetched_ai_response.role, MessageRole::Assistant);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_messages() -> Result<()> {
        let (pool, memory, user1, character) = setup_test_environment().await?;
        
        let user2_id_str = format!("user2_{}", CryptoHash::random().to_hex_string());
        let mut user2_builder = User {
            id: CryptoHash::default(),
            user_id: user2_id_str.clone(),
            user_aka: "User Two Aka".to_string(),
            role: UserRole::User,
            provider: "test_provider".to_string(),
            last_active: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        user2_builder.sql_populate_id()?;
        let user2 = user2_builder.create(&*pool).await?;

        let session1_user1 = create_test_session(&pool, &user1, &character, false).await?;
        let session2_user1 = create_test_session(&pool, &user1, &character, true).await?;
        let session1_user2 = create_test_session(&pool, &user2, &character, false).await?;

        memory.add_messages(&[
            create_test_message(session1_user1.id.clone(), user1.id.clone(), character.id.clone(), MessageRole::User, "U1S1M1"),
            create_test_message(session1_user1.id.clone(), user1.id.clone(), character.id.clone(), MessageRole::User, "U1S1M2"),
        ]).await?;
        memory.add_messages(&[
            create_test_message(session2_user1.id.clone(), user1.id.clone(), character.id.clone(), MessageRole::User, "U1S2M1"),
        ]).await?;
        memory.add_messages(&[
            create_test_message(session1_user2.id.clone(), user2.id.clone(), character.id.clone(), MessageRole::User, "U2S1M1"),
        ]).await?;

        let user1_messages = memory.get_all(&user1.id, 10, 0).await?;
        assert_eq!(user1_messages.len(), 3);
        assert!(user1_messages.iter().all(|m| m.owner == user1.id));
        let mut contents: Vec<String> = user1_messages.iter().map(|m| m.content.clone()).collect();
        contents.sort(); 
        assert_eq!(contents, vec!["U1S1M1", "U1S1M2", "U1S2M1"]);

        let user1_messages_limit1 = memory.get_all(&user1.id, 1, 0).await?;
        assert_eq!(user1_messages_limit1.len(), 1);

        let user1_messages_offset1 = memory.get_all(&user1.id, 10, 1).await?;
        assert_eq!(user1_messages_offset1.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_search_messages() -> Result<()> {
        let (pool, memory, user, character) = setup_test_environment().await?;
        let session = create_test_session(&pool, &user, &character, true).await?;

        let msg1 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, "SearchMsg1");
        let msg2 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::Assistant, "SearchMsg2");
        let msg3 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, "SearchMsg3");
        
        memory.add_messages(&[msg1.clone(), msg2.clone(), msg3.clone()]).await?;
        
        let search_trigger_message = RoleplayMessage { 
            id: CryptoHash::random(), 
            session_id: session.id.clone(), 
            owner: user.id.clone(),
            character_id: character.id.clone(),
            role: MessageRole::User,
            content_type: MessageType::Text,
            content: "irrelevant".to_string(),
            created_at: 0 
        };

        let history = memory.search(&search_trigger_message, 10, 0).await?;
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].content, "SearchMsg1"); 
        assert_eq!(history[1].content, "SearchMsg2");
        assert_eq!(history[2].content, "SearchMsg3");

        Ok(())
    }
    
    #[tokio::test]
    async fn test_update_messages() -> Result<()> {
        let (pool, memory, user, character) = setup_test_environment().await?;
        let session = create_test_session(&pool, &user, &character, false).await?;

        let original_message_content = "Original Content";
        let msg1 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, original_message_content);
        
        memory.add_messages(&[msg1.clone()]).await?;
        
        let criteria = QueryCriteria::new()
            .add_valued_filter("session_id", "=", session.id.to_hex_string())?
            .add_valued_filter("content", "=", original_message_content.to_string())?;
        let mut fetched_msg = RoleplayMessage::find_one_by_criteria(criteria, &*pool).await?.unwrap();

        let updated_content = "Updated Content";
        fetched_msg.content = updated_content.to_string();

        memory.update(&[fetched_msg.clone()]).await?;

        let retrieved_after_update = memory.get_one(&fetched_msg.id).await?.unwrap();
        assert_eq!(retrieved_after_update.content, updated_content);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_messages() -> Result<()> {
        let (pool, memory, user, character) = setup_test_environment().await?;
        let session = create_test_session(&pool, &user, &character, false).await?;

        let msg1 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, "DeleteMsg1");
        let msg2 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, "DeleteMsg2");
        let msg3 = create_test_message(session.id.clone(), user.id.clone(), character.id.clone(), MessageRole::User, "DeleteMsg3");

        memory.add_messages(&[msg1.clone(), msg2.clone(), msg3.clone()]).await?;

        let criteria_msg1 = QueryCriteria::new().add_valued_filter("content", "=", "DeleteMsg1".to_string())?.limit(1)?;
        let db_msg1 = RoleplayMessage::find_one_by_criteria(criteria_msg1, &*pool).await?.unwrap();
        let criteria_msg2 = QueryCriteria::new().add_valued_filter("content", "=", "DeleteMsg2".to_string())?.limit(1)?;
        let db_msg2 = RoleplayMessage::find_one_by_criteria(criteria_msg2, &*pool).await?.unwrap();
        let criteria_msg3 = QueryCriteria::new().add_valued_filter("content", "=", "DeleteMsg3".to_string())?.limit(1)?;
        let db_msg3 = RoleplayMessage::find_one_by_criteria(criteria_msg3, &*pool).await?.unwrap();

        memory.delete(&[db_msg1.id.clone(), db_msg3.id.clone()]).await?;

        assert!(memory.get_one(&db_msg1.id).await?.is_none());
        assert!(memory.get_one(&db_msg2.id).await?.is_some());
        assert!(memory.get_one(&db_msg3.id).await?.is_none());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_reset_user_data() -> Result<()> {
        let (pool, memory, user1, character) = setup_test_environment().await?;
        let user2_id_str = format!("user2_reset_{}", CryptoHash::random().to_hex_string());
        let mut user2_builder = User {
            id: CryptoHash::default(),
            user_id: user2_id_str.clone(),
            user_aka: "User Two Reset Aka".to_string(),
            role: UserRole::User,
            provider: "test_provider".to_string(),
            last_active: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        user2_builder.sql_populate_id()?;
        let user2 = user2_builder.create(&*pool).await?;

        let session1_user1 = create_test_session(&pool, &user1, &character, false).await?;
        let session2_user1 = create_test_session(&pool, &user1, &character, true).await?;
        let session1_user2 = create_test_session(&pool, &user2, &character, false).await?;

        memory.add_messages(&[
            create_test_message(session1_user1.id.clone(), user1.id.clone(), character.id.clone(), MessageRole::User, "U1S1M_Reset"),
        ]).await?;
         memory.add_messages(&[
            create_test_message(session2_user1.id.clone(), user1.id.clone(), character.id.clone(), MessageRole::User, "U1S2M_Reset"),
        ]).await?;
        let user2_msg = create_test_message(session1_user2.id.clone(), user2.id.clone(), character.id.clone(), MessageRole::User, "U2S1M_Reset");
        memory.add_messages(&[user2_msg.clone()]).await?;

        memory.reset(&user1.id).await?;

        let user1_messages_after_reset = memory.get_all(&user1.id, 10, 0).await?;
        assert!(user1_messages_after_reset.is_empty());

        let criteria_s1u1 = QueryCriteria::new().add_valued_filter("id", "=", session1_user1.id.to_hex_string())?;
        assert!(RoleplaySession::find_one_by_criteria(criteria_s1u1, &*pool).await?.is_none());
        let criteria_s2u1 = QueryCriteria::new().add_valued_filter("id", "=", session2_user1.id.to_hex_string())?;
        assert!(RoleplaySession::find_one_by_criteria(criteria_s2u1, &*pool).await?.is_none());

        let user2_messages_after_reset = memory.get_all(&user2.id, 10, 0).await?;
        assert_eq!(user2_messages_after_reset.len(), 1);
        assert_eq!(user2_messages_after_reset[0].content, "U2S1M_Reset");

        let criteria_s1u2 = QueryCriteria::new().add_valued_filter("id", "=", session1_user2.id.to_hex_string())?;
        assert!(RoleplaySession::find_one_by_criteria(criteria_s1u2, &*pool).await?.is_some());

        Ok(())
    }

    async fn create_test_character(pool: &PgPool, creator_id: CryptoHash, name: &str) -> Result<Character> {
        let mut char_builder = Character {
            id: CryptoHash::default(),
            name: name.to_string(),
            description: format!("Description for {}", name),
            creator: creator_id,
            status: CharacterStatus::Published,
            gender: CharacterGender::default(),
            language: CharacterLanguage::default(),
            features: vec![CharacterFeature::Roleplay],
            prompts_scenario: "Default scenario".to_string(),
            prompts_personality: "Default personality".to_string(),
            prompts_example_dialogue: "Default dialogue".to_string(),
            prompts_first_message: format!("Hello from {}!", name),
            tags: vec!["test_char_tag".to_string()],
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
            published_at: get_current_timestamp(),
            version: 1,
            reviewed_by: None,
        };
        char_builder.sql_populate_id()?;
        char_builder.create(&*pool).await.map_err(anyhow::Error::from)
    }

    #[tokio::test]
    async fn test_find_public_conversations_by_character() -> Result<()> {
        let (pool, memory, user, _char_setup) = setup_test_environment().await?;
        let char1 = create_test_character(&pool, user.id.clone(), "Char1_PublicTest").await?;
        let char2 = create_test_character(&pool, user.id.clone(), "Char2_PublicTest").await?;

        let _s1_public_char1 = create_test_session(&pool, &user, &char1, true).await?;
        let _s2_private_char1 = create_test_session(&pool, &user, &char1, false).await?;
        let _s3_public_char2 = create_test_session(&pool, &user, &char2, true).await?;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await; 
        let s4_public_char1_latest = create_test_session(&pool, &user, &char1, true).await?;

        let public_convos_char1 = memory.find_public_conversations_by_character(&char1.id, 10, 0).await?;
        assert_eq!(public_convos_char1.len(), 2);
        assert!(public_convos_char1.iter().all(|s| s.public && s.character_id == char1.id));
        assert_eq!(public_convos_char1[0].id, s4_public_char1_latest.id); 

        let public_convos_char1_limit1 = memory.find_public_conversations_by_character(&char1.id, 1, 0).await?;
        assert_eq!(public_convos_char1_limit1.len(), 1);
        assert_eq!(public_convos_char1_limit1[0].id, s4_public_char1_latest.id);
        
        let public_convos_char1_offset1 = memory.find_public_conversations_by_character(&char1.id, 1, 1).await?;
        assert_eq!(public_convos_char1_offset1.len(), 1);
        assert_ne!(public_convos_char1_offset1[0].id, s4_public_char1_latest.id); 

        Ok(())
    }

    #[tokio::test]
    async fn test_find_latest_conversations() -> Result<()> {
        let (pool, memory, user, character) = setup_test_environment().await?;

        let _s1_older = create_test_session(&pool, &user, &character, true).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await; 
        let s2_middle = create_test_session(&pool, &user, &character, false).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let s3_latest = create_test_session(&pool, &user, &character, true).await?;

        let latest_convos = memory.find_latest_conversations(&user.id, &character.id, 2).await?;
        assert_eq!(latest_convos.len(), 2);
        assert_eq!(latest_convos[0].id, s3_latest.id);
        assert_eq!(latest_convos[1].id, s2_middle.id);
        
        let all_latest_convos = memory.find_latest_conversations(&user.id, &character.id, 10).await?;
        assert_eq!(all_latest_convos.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_find_character_list_of_user() -> Result<()> {
        let (pool, memory, user, _char_setup) = setup_test_environment().await?;
        let char1 = create_test_character(&pool, user.id.clone(), "Char1_ListTest").await?;
        let char2 = create_test_character(&pool, user.id.clone(), "Char2_ListTest").await?;
        let char3 = create_test_character(&pool, user.id.clone(), "Char3_ListTest").await?;

        create_test_session(&pool, &user, &char1, true).await?;
        create_test_session(&pool, &user, &char1, false).await?;
        create_test_session(&pool, &user, &char1, true).await?;
        create_test_session(&pool, &user, &char2, true).await?;

        let char_list = memory.find_character_list_of_user(&user.id).await?;
        
        assert_eq!(char_list.len(), 2); 
        assert_eq!(char_list.get(&char1.id), Some(&3));
        assert_eq!(char_list.get(&char2.id), Some(&1));
        assert!(char_list.get(&char3.id).is_none());

        Ok(())
    }
} 