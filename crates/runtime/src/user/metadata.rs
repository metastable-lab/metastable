use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;

use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};

use crate::user::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_metadata"]
pub struct UserMetadata {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub referred_by: Option<CryptoHash>,
    pub referred_code: Option<String>,

    pub notes: Json<Value>,
}

impl SqlxPopulateId for UserMetadata {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            anyhow::bail!("[UserMetadata] id is not populated");
        } else {
            Ok(())
        }
    }
}