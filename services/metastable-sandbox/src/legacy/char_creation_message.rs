use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use metastable_database::SqlxObject;
use metastable_runtime::{Character, MessageRole, MessageType, SystemConfig, User};
use super::RoleplaySession;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "character_creation_messages"]
pub struct CharacterCreationMessage {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_sessions", related_rust_type = "RoleplaySession")]
    pub roleplay_session_id: Uuid,

    #[foreign_key(referenced_table = "system_configs", related_rust_type = "SystemConfig")]
    pub character_creation_system_config: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    pub role: MessageRole,
    pub content_type: MessageType,

    pub character_creation_call: Json<Vec<FunctionCall>>,
    
    pub character_creation_maybe_character_str: Option<String>,
    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character_creation_maybe_character_id: Option<Uuid>,

    pub content: String,

    pub is_migrated: bool,

    pub created_at: i64,
    pub updated_at: i64,
}

impl CharacterCreationMessage {

    pub fn update_character(&self, character: &Character) -> Result<Character> {

        let maybe_character_id = self.character_creation_maybe_character_id;
        let character_id = character.id;

        if maybe_character_id.is_some() && maybe_character_id.unwrap() != character_id {
            anyhow::bail!("Character ID mismatch: {:?} != {:?}", maybe_character_id, character_id);
        }

        let mut char = character.clone();
        char.creation_message = None;
        char.creation_session = Some(self.roleplay_session_id);

        Ok(char)
    }
}