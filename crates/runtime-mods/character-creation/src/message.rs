use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use voda_database::SqlxObject;
use voda_runtime::{Message, MessageRole, MessageType, SystemConfig, User};
use voda_runtime_roleplay::{Character, RoleplayMessage, RoleplaySession};

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
    pub created_at: i64,
    pub updated_at: i64,
}

impl Message for CharacterCreationMessage {
    fn id(&self) -> &Uuid { &self.id }

    fn role(&self) -> &MessageRole { &self.role }
    fn owner(&self) -> &Uuid { &self.owner }
    
    fn content_type(&self) -> &MessageType { &self.content_type }
    fn text_content(&self) -> Option<String> { Some(self.content.clone()) }
    fn binary_content(&self) -> Option<Vec<u8>> { None }
    fn url_content(&self) -> Option<String> { None }

    fn created_at(&self) -> i64 { self.created_at }
}

impl CharacterCreationMessage {
    pub fn blank_user_message(
        roleplay_session_id: &Uuid,
        user_id: &Uuid,
    ) -> Self {
        Self {
            id: Uuid::default(),
            owner: user_id.clone(),
            role: MessageRole::User,
            content_type: MessageType::Text,
            content: "".to_string(),
            roleplay_session_id: roleplay_session_id.clone(),
            character_creation_system_config: Uuid::default(),
            character_creation_call: Json(vec![]),
            character_creation_maybe_character_str: None,
            character_creation_maybe_character_id: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn system_message(
        session_id: &Uuid,
        user_id: &Uuid,
        system_config: &SystemConfig,
    ) -> Self {
        Self {
            id: Uuid::default(),
            owner: user_id.clone(),
            role: MessageRole::System,
            content_type: MessageType::Text,
            content: system_config.system_prompt.clone(),
            roleplay_session_id: session_id.clone(),
            character_creation_system_config: system_config.id,
            character_creation_call: Json(vec![]),
            character_creation_maybe_character_str: None,
            character_creation_maybe_character_id: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_roleplay_messages(
        roleplay_system_message: &RoleplayMessage,
        roleplay_first_message: &RoleplayMessage,
        roleplay_messages: &[RoleplayMessage],
    ) -> Result<Self> { // Self, Self
        // 1. prove all messages are from the same session
        let session_id = roleplay_system_message.session_id;
        if roleplay_first_message.session_id != session_id {
            return Err(anyhow::anyhow!(
                "[CharacterCreationMessage::from_roleplay_messages] All messages must be from the same session: {} != {}",
                roleplay_first_message.session_id, session_id
            ));
        }

        for message in roleplay_messages {
            if message.session_id != session_id {
                return Err(anyhow::anyhow!(
                    "[CharacterCreationMessage::from_roleplay_messages] All messages must be from the same session: {} != {}",
                    message.session_id, session_id
                ));
            }
        }

        // 2. pack messages and annotate them with assistant: or user:
        let mut message_pieces = vec![];
        message_pieces.push(format!("system: {}", roleplay_system_message.content));
        message_pieces.push(format!("assistant: {}", roleplay_first_message.content));
        message_pieces.extend(roleplay_messages
                .iter()
                .map(|m| format!("{}: {}", 
                    m.role(), 
                    m.text_content().unwrap_or_default()
                )));

        let all_messages = message_pieces.join("\n\n");

        // 3. build 
        Ok(Self {
            id: Uuid::default(),
            owner: roleplay_system_message.owner.clone(),
            role: MessageRole::User,
            content_type: MessageType::Text,
            content: all_messages,
            roleplay_session_id: session_id,
            character_creation_system_config: Uuid::default(), // to be populated later
            character_creation_call: Json(vec![]),
            character_creation_maybe_character_str: None,
            character_creation_maybe_character_id: None,
            created_at: 0,
            updated_at: 0,
        })
    }
}
