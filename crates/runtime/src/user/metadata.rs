use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::{Json, Uuid};

use voda_database::SqlxObject;

use crate::user::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_metadata"]
pub struct UserMetadata {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub referred_by: Option<Uuid>,
    pub referred_code: Option<String>,

    pub notes: Json<Value>,

    pub created_at: i64,
    pub updated_at: i64,
}