use anyhow::Result;
use serde::{Deserialize, Serialize};

use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, SqlxObject)]
#[table_name = "user_profiles"]
pub struct UserProfile {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub username: String,
    pub first_name: String, 
    pub last_name: Option<String>,

    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
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