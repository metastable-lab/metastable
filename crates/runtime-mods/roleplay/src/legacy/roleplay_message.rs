use metastable_database::SqlxObject;
use metastable_runtime::{MessageRole, MessageType, User};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{RoleplayMessageType, RoleplaySession};

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

    pub created_at: i64,
    pub updated_at: i64,
}

// impl RoleplayMessage {
//     pub fn to_message(system_config: &SystemConfig, user_message: &Self, assistant_message: &Self) -> Message {
//         Message {
//             id: user_message.id,
//             owner: user_message.owner,
//             system_config: system_config.id,
            
//             user_message_content: user_message.content.clone(),
//             user_message_content_type: user_message.content_type,
//             input_toolcall: user_message.content_v1.clone(),

//             assistant_message_content: assistant_message.content.clone(),
//             assistant_message_content_type: assistant_message.content_type,
//             assistant_message_tool_call: assistant_message.content_v1.clone().into(),

//             model_name: system_config.openai_model.clone(),

//             usage: user_message.usage,
//             finish_reason: user_message.finish_reason,
//             refusal: user_message.refusal,
//             points_consumed_claimed: user_message.points_consumed_claimed,
//             points_consumed_purchased: user_message.points_consumed_purchased,
//             points_consumed_misc: user_message.points_consumed_misc,
            
//             is_stale: user_message.is_removed,
//             is_memorizeable: user_message.is_memorizeable,
//             is_in_memory: user_message.is_saved_in_memory,
//             created_at: user_message.created_at,
//             updated_at: user_message.updated_at,
//         }
//     }
// }