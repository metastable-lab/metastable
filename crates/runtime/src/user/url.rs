use anyhow::Result;
use serde::{Deserialize, Serialize};
use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::{SqlxObject, SqlxPopulateId};

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_urls"]
pub struct UserUrl {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub created_by: CryptoHash,
    #[foreign_key_many(referenced_table = "users", related_rust_type = "User")]
    pub used_by: Vec<CryptoHash>,

    pub url_type: String,
    pub path: String,

    pub created_at: i64,
    pub updated_at: i64,
}

impl SqlxPopulateId for UserUrl {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}

impl UserUrl {
    pub fn new(created_by: CryptoHash, path: String, url_type: String) -> Self {
        let mut url = Self::default();
        url.id = CryptoHash::random();
        url.created_at = get_current_timestamp();
        url.created_by = created_by;
        url.path = path;
        url.url_type = url_type;
        url
    }
}