use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use serde_json::Value;

use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_profiles"]
pub struct UserProfile {
    #[serde(rename = "_id")]
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub id: CryptoHash,

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

impl SqlxPopulateId for UserProfile {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            anyhow::bail!("[UserProfile] id is not populated");
        } else {
            Ok(())
        }
    }
}