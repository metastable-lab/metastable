use anyhow::Result;
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use voda_common::get_current_timestamp;
use voda_database::SqlxObject;

use crate::user::User;

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "user_usages"]
pub struct UserUsage {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub model_name: String,
    pub usage: Json<CompletionUsage>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserUsage {
    pub fn new(user_id: Uuid, model_name: String, usage: CompletionUsage) -> Self {
        Self {
            id: Uuid::default(),

            user_id,
            model_name,
            usage: Json(usage),

            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}
