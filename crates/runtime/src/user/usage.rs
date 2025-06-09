use anyhow::Result;
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};

use sqlx::types::Json;
use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::{SqlxObject, SqlxPopulateId};

use crate::user::User;

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "user_usages"]
pub struct UserUsage {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: CryptoHash,
    pub model_name: String,
    pub usage: Json<CompletionUsage>,

    pub created_at: i64,
    pub updated_at: i64,
}


impl SqlxPopulateId for UserUsage {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.user_id == CryptoHash::default() {
            anyhow::bail!("[UserUsage] user_id is not populated");
        } else {
            self.id = CryptoHash::random();
            Ok(())
        }
    }
}
impl UserUsage {
    pub fn new(user_id: CryptoHash, model_name: String, usage: CompletionUsage) -> Self {
        Self {
            id: CryptoHash::default(),

            user_id,
            model_name,
            usage: Json(usage),

            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}
