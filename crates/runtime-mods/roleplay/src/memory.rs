use anyhow::{anyhow, Result};

use metastable_common::ModuleClient;
use metastable_database::{OrderDirection, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use metastable_runtime::{CharacterFeature, ChatSession, Message, Prompt, SystemConfig};
use serde::{Deserialize, Serialize};
use metastable_clients::{EmbeddingMessage, EmbederClient, Mem0Filter, PgvectorClient, PostgresClient};
use sqlx::types::{Json, Uuid};

use crate::{agents::SendMessage, try_prase_message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoleplayInput {
    ContinueSession(Uuid, Prompt), // session_id
    RegenerateSession(Uuid), // session_id
}

#[derive(Clone)]
pub struct RoleplayMemory {
    pgvector: PgvectorClient,
    embeder: EmbederClient,
    db: PostgresClient,
}

impl RoleplayMemory {
    pub async fn new() -> Result<Self> {
        let pgvector = PgvectorClient::setup_connection().await;
        let embeder = EmbederClient::setup_connection().await;
        let db = PostgresClient::setup_connection().await;

        Ok(Self { pgvector, embeder, db })
    }
}

impl RoleplayMemory {
    pub async fn build_inputs(&self, input: &RoleplayInput, system_config: &SystemConfig) -> Result<Vec<Prompt>> {
        let mut tx = self.db.get_client().begin().await?;
        let (session_id, user_message) = match &input {
            RoleplayInput::ContinueSession(session_id, user_message) => (session_id.clone(), user_message.clone()),
            RoleplayInput::RegenerateSession(session_id) => (session_id.clone(), Prompt::empty())
        };

        let session = ChatSession::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
            &mut *tx 
        ).await?
            .ok_or(anyhow!("[RoleplayInput::build_input] Session not found"))?;

        let user = session.fetch_owner(&mut *tx).await?
            .ok_or(anyhow!("[RoleplayInput::build_input] User not found"))?;
        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow!("[RoleplayInput::build_input] Character not found"))?;

        let history = Message::find_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("session", "=", session.id)
                .order_by("created_at", OrderDirection::Desc),
            &mut *tx
        ).await?;

        // seperate memories into pieces
        // 1. the LATEST 3 messages will be passed in as they are
        // 2. the FOLLOWING 10 messages will extract memories from them
        // 3. the rest of the memories will be disgarded and will be replaced with vector db fragments

        let latest_3_messages = history.iter().take(3).flat_map(|m| Prompt::from_message(m)).collect::<Vec<_>>();
        let follwing_unmemorized_messages = history.iter()
            .filter(|m| !m.is_in_memory)
            .map(|m| m.summary.clone())
            .filter(|s| s.is_some())
            .map(|s| s.unwrap())
            .collect::<Vec<_>>();

       let vector_db_memories = {
            if user_message.content.is_empty() {
                vec![]
            } else {
                // build historical memories
                let session_id_filter = if session.use_character_memory && !character.features.contains(&CharacterFeature::CharacterCreation) {
                    None
                } else {
                    Some(session.id)
                };
                let filter = Mem0Filter {
                    user_id: user.id,
                    character_id: Some(character.id),
                    session_id: session_id_filter,
                };
                let query = EmbeddingMessage::batch_create(&self.embeder, &[user_message.content.clone()], &filter).await?;
                EmbeddingMessage::batch_search(&self.pgvector, &filter, &query, 20).await?
                    .iter().flatten().map(|r| r.content.clone()).collect::<Vec<_>>()   
            }
       };

        let mut system_prompt = character.build_system_prompt(&system_config.system_prompt, &user.user_aka);
        system_prompt.inject_system_memory(follwing_unmemorized_messages, vector_db_memories);
        let first_message = character.build_first_message(&user.user_aka);

        let mut prompts = vec![system_prompt, first_message];

        prompts.extend(latest_3_messages);
        prompts = Prompt::sort(prompts)?;
        prompts.push(user_message.clone());

        if let RoleplayInput::RegenerateSession(_) = &input {
            if prompts.len() < 3 {
                return Err(anyhow!("[RoleplayInput::build_inputs] too little messages to do regenerate"));
            }
            prompts.pop(); // pop the empty user msg
            prompts.pop(); // pop the last message
        }

        tx.commit().await?;
        Ok(prompts)
    }

    pub async fn handle_outputs(&self, input: &RoleplayInput, message: &Message, tool: &SendMessage) -> Result<Message> {
        let mut tx = self.db.get_client().begin().await?;

        if tool.messages.is_empty() && message.assistant_message_content.is_empty() {
            return Err(anyhow!("[RoleplayInput::handle_outputs] No messages returned"));
        }

        let (mut msg, session_id) = match &input {
            RoleplayInput::ContinueSession(session_id, _) => {
                let mut message = message.clone();
                message.session = Some(session_id.clone());
                message.summary = Some(tool.summary.clone());
                (message.create(&mut *tx).await?, session_id.clone())
            },
            RoleplayInput::RegenerateSession(session_id) => {
                let latest_message = Message::find_one_by_criteria(
                    QueryCriteria::new()
                        .add_valued_filter("session", "=", session_id.clone())
                        .order_by("created_at", OrderDirection::Desc),
                    &mut *tx
                ).await?;
                let latest_message = latest_message.ok_or(anyhow!("Unexpected [RoleplayInput::handle_outputs] Latest message not found"))?;
                let mut message = message.clone();
                message.id = latest_message.id;
                message.session = Some(session_id.clone());
                message.summary = Some(tool.summary.clone());
                (message.update(&mut *tx).await?, session_id.clone())
            },
        };

        let tc = try_prase_message(&msg)?;
        msg.assistant_message_tool_call = Json(Some(tc));
        let msg = msg.update(&mut *tx).await?;

        let mut session = ChatSession::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
            &mut *tx
        ).await?
            .ok_or(anyhow!("[RoleplayInput::handle_outputs] Session not found"))?;

        session.nonce += 1;
        session.update(&mut *tx).await?;

        tx.commit().await?;
        Ok(msg)
    }
}
