use metastable_database::{SqlxObject, TextEnum};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{Character, Message, User};

#[derive(Debug, Clone, Default, TextEnum, PartialEq, Eq)]
pub enum MultimodelMessageType {
    #[default]
    Voice,
    Image,
}

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "multimodel_messages"]
pub struct MultimodelMessage {
    pub id: Uuid,

    #[indexed]
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,
    
    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character_id: Uuid,

    #[foreign_key(referenced_table = "messages", related_rust_type = "Message")]
    pub message_id: Uuid,

    pub message_type: MultimodelMessageType,
    pub r2_url: String,

    pub created_at: i64,
    pub updated_at: i64,
}
