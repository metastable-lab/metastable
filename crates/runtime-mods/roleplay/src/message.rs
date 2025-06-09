use anyhow::Result;
use serde::{Deserialize, Serialize};

use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::{SqlxObject, SqlxPopulateId};
use voda_runtime::{LLMRunResponse, Message, MessageRole, MessageType, SystemConfig, User};

use super::{Character, RoleplaySession};

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_messages"]
pub struct RoleplayMessage {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    #[foreign_key(referenced_table = "roleplay_sessions", related_rust_type = "RoleplaySession")]
    pub session_id: CryptoHash,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: CryptoHash,

    pub role: MessageRole,
    pub content_type: MessageType,

    pub content: String,
    pub created_at: i64,
}

impl SqlxPopulateId for RoleplayMessage {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}

impl Message for RoleplayMessage {
    fn id(&self) -> &CryptoHash { &self.id }

    fn role(&self) -> &MessageRole { &self.role }
    fn owner(&self) -> &CryptoHash { &self.owner }
    
    fn content_type(&self) -> &MessageType { &self.content_type }
    fn text_content(&self) -> Option<String> { Some(self.content.clone()) }
    fn binary_content(&self) -> Option<Vec<u8>> { None }
    fn url_content(&self) -> Option<String> { None }

    fn created_at(&self) -> i64 { self.created_at }

    fn from_llm_response(response: LLMRunResponse, session_id: &CryptoHash, user_id: &CryptoHash) -> Self {
        Self {
            id: CryptoHash::default(),
            owner: user_id.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: response.content,
            created_at: get_current_timestamp(),
            session_id: session_id.clone(),
        }
    }
}

impl RoleplayMessage {
    fn replace_placeholders(
        text: &str, character_name: &str, user_name: &str,
    ) -> String {
        text.replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
    }
    
    fn replace_placeholders_system_prompt(
        character_name: &str, user_name: &str,
        system_prompt: &str,
        character_personality: &str, character_example_dialogue: &str, character_scenario: &str
    ) -> String {
        let character_personality = Self::replace_placeholders(character_personality, character_name, user_name);
        let character_example_dialogue = Self::replace_placeholders(character_example_dialogue, character_name, user_name);
        let character_scenario = Self::replace_placeholders(character_scenario, character_name, user_name);
    
        let system_prompt = system_prompt
            .replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
            .replace("{{char_personality}}", &character_personality)
            .replace("{{char_example_dialogue}}", &character_example_dialogue)
            .replace("{{char_scenario}}", &character_scenario);
    
        system_prompt
    }

    pub fn system(
        session: &RoleplaySession, system_config: &SystemConfig, character: &Character, user: &User,
    ) -> Self {
        let system_prompt = Self::replace_placeholders_system_prompt(
            &character.name, 
            &user.user_aka,
            &system_config.system_prompt, 
            &character.prompts_personality,
            &character.prompts_example_dialogue,
            &character.prompts_scenario
        );

        Self {
            id: CryptoHash::default(),
            owner: user.id.clone(),
            role: MessageRole::System,
            content_type: MessageType::Text,
            content: system_prompt,
            created_at: get_current_timestamp(),
            session_id: session.id.clone(),
        }
    }

    pub fn first_message(
        session: &RoleplaySession, character: &Character, user: &User
    ) -> Self {
        let first_message = Self::replace_placeholders(
            &character.prompts_first_message, 
            &character.name, 
            &user.user_aka
        );

        Self {
            id: CryptoHash::default(),
            owner: user.id.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: first_message,
            created_at: get_current_timestamp(),
            session_id: session.id.clone(),
        }
    }
}
