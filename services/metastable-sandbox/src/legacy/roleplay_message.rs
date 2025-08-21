use anyhow::Result;
use async_openai::types::FunctionCall;
use metastable_database::SqlxObject;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use metastable_runtime::{Message, MessageRole, MessageType, SystemConfig, ToolCall, User};
use metastable_runtime_roleplay::agents::{RoleplayMessageType, SendMessage};
use super::RoleplaySession;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_messages"]
pub struct RoleplayMessage {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_sessions", related_rust_type = "RoleplaySession")]
    pub session_id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    pub role: MessageRole,
    pub content_type: MessageType,

    pub content: String,
    pub content_v1: Vec<RoleplayMessageType>,
    pub options: Vec<String>,

    pub is_saved_in_memory: bool,
    pub is_removed: bool,

    pub is_migrated: bool,

    pub created_at: i64,
    pub updated_at: i64,
}

impl RoleplayMessage {
    fn build_toolcall(content_v1: Vec<RoleplayMessageType>, options: Vec<String>) -> Option<FunctionCall> {
        tracing::debug!("getting toolcall for {:?} {:?}", content_v1, options);
        if content_v1.is_empty() && options.is_empty() {
            return None;
        }

        let toolcall = SendMessage {
            messages: content_v1,
            options,
            summary: String::new(),
        }.into_tool_call();

        if toolcall.is_err() {
            tracing::error!("Failed to build toolcall: {:?}", toolcall.err());
            return None;
        } else {
            Some(toolcall.unwrap())
        }
    }

    pub fn to_message(system_config: &SystemConfig, user_message: &Self, assistant_message: &Self) -> Message {
        let user_message_toolcall = Self::build_toolcall(
            user_message.content_v1.clone(), user_message.options.clone()
        );

        let assistant_message_toolcall = Self::build_toolcall(
            assistant_message.content_v1.clone(), assistant_message.options.clone()
        );

        Message {
            id: user_message.id,
            owner: user_message.owner,
            system_config: system_config.id,
            session: Some(user_message.session_id),

            user_message_content: user_message.content.clone(),
            user_message_content_type: user_message.content_type.clone(),
            input_toolcall: user_message_toolcall.into(),

            assistant_message_content: assistant_message.content.clone(),
            assistant_message_content_type: assistant_message.content_type.clone(),
            assistant_message_tool_call: assistant_message_toolcall.into(),

            model_name: system_config.openai_model.clone(),

            usage: Json(None),
            finish_reason: None,
            refusal: None,
            summary: None,

            is_stale: user_message.is_removed || assistant_message.is_removed,
            is_memorizeable: true,
            is_in_memory: user_message.is_saved_in_memory || assistant_message.is_saved_in_memory,

            created_at: user_message.created_at,
            updated_at: user_message.updated_at,
        }
    }
}