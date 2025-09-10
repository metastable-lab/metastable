use anyhow::Result;
use async_openai::types::{CompletionUsage, FunctionCall};
use serde::{Deserialize, Serialize};
use metastable_database::{SqlxObject, TextEnum, TextEnumCodec};

use sqlx::types::{Json, Uuid};
use crate::{ChatSession, SystemConfig, User};

#[derive(Debug, Serialize, Deserialize, Clone, Default, TextEnum, PartialEq, Eq)]
pub enum MessageRole {
    System,
    #[default]
    User,
    Assistant,
    ToolCall,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, TextEnum, PartialEq, Eq)]
pub enum MessageType {
    #[default]
    Text,
    Image,
}

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "messages"]
pub struct Message {
    pub id: Uuid,

    #[indexed]
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,
    #[indexed]
    #[foreign_key(referenced_table = "system_configs", related_rust_type = "SystemConfig")]
    pub system_config: Uuid,
    #[indexed]
    #[foreign_key(referenced_table = "chat_sessions", related_rust_type = "ChatSession")]
    pub session: Option<Uuid>,

    pub user_message_content: String,
    pub user_message_content_type: MessageType,

    pub input_toolcall: Json<Option<FunctionCall>>,

    pub assistant_message_content: String,
    pub assistant_message_content_type: MessageType,
    pub assistant_message_tool_call: Json<Option<FunctionCall>>,
    
    pub summary: Option<String>,

    pub model_name: String,
    pub usage: Json<Option<CompletionUsage>>,
    pub finish_reason: Option<String>,
    pub refusal: Option<String>,

    pub is_stale: bool,
    pub is_memorizeable: bool,
    pub is_in_memory: bool,

    pub is_migrated: bool,

    pub created_at: i64,
    pub updated_at: i64,
}
