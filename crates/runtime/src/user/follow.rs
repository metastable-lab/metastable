use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_database::SqlxObject;

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_follows"]
pub struct UserFollow {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub follower_id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub following_id: Uuid,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserFollow {
    pub fn new(follower_id: Uuid, following_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            follower_id,
            following_id,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}