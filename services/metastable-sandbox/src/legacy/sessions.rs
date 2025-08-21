use serde::{Deserialize, Serialize};
use anyhow::Result;
use sqlx::types::Uuid;

use metastable_database::SqlxObject;

use metastable_runtime::{User, SystemConfig, Character};
use super::RoleplayMessage;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_sessions"]
pub struct RoleplaySession {
    #[serde(rename = "_id")]
    pub id: Uuid,

    pub public: bool,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "system_configs", related_rust_type = "SystemConfig")]
    pub system_config: Uuid,

    #[foreign_key_many(referenced_table = "roleplay_messages", related_rust_type = "RoleplayMessage")]
    pub history: Vec<Uuid>,

    pub use_character_memory: bool,
    pub hidden: bool,

    pub is_migrated: bool,

    pub updated_at: i64,
    pub created_at: i64,
}
