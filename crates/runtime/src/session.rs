use serde::{Deserialize, Serialize};
use anyhow::Result;
use sqlx::types::Uuid;

use metastable_database::SqlxObject;
use crate::{Character, User};

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "chat_sessions"]
pub struct ChatSession {
    pub id: Uuid,
    pub public: bool,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    pub use_character_memory: bool,
    pub hidden: bool,

    pub nonce: i64, // only used for refresh the updated_at
    pub user_mask: Option<String>,

    pub updated_at: i64,
    pub created_at: i64,
}

impl ChatSession {
    pub fn new(character_id: Uuid, owner: Uuid, use_character_memory: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            public: false,
            owner,
            character: character_id,
            use_character_memory,
            hidden: false,
            nonce: 0,
            user_mask: None,
            updated_at: 0,
            created_at: 0,
        }
    }
}