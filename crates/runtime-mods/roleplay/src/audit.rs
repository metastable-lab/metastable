use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_database::SqlxObject;
use metastable_runtime::User;

use crate::{Character, CharacterStatus};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_character_audit_logs"]
pub struct AuditLog {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub author: Uuid,

    pub previous_status: CharacterStatus,
    pub new_status: CharacterStatus,

    pub notes: String,
    pub created_at: i64,
}

