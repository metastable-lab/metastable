use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_database::SqlxObject;

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_badges"]
pub struct UserBadge {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub badge_type: String,
    pub badge_id: Uuid,

    pub created_at: i64,
    pub updated_at: i64,
}
