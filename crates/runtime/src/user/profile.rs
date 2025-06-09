use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};
use serde_json::Value;

use voda_database::SqlxObject;

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_profiles"]
pub struct UserProfile {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    
    pub extra: Option<Json<Value>>,

    pub created_at: i64,
    pub updated_at: i64,
}