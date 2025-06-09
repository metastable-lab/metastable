mod metadata;
mod points;
mod profile;
mod usage;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use voda_common::{CryptoHash, blake3_hash};
use voda_database::{SqlxObject, SqlxPopulateId};

pub use metadata::UserMetadata;
pub use points::UserPoints;
pub use profile::UserProfile;
pub use usage::UserUsage;

pub const BALANCE_CAP: i64 = 500;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)] // Added strum derives
pub enum UserRole {
    Admin,
    #[default]
    User,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "users"]
pub struct User {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub user_id: String,
    pub user_aka: String,

    pub role: UserRole,
    pub provider: String,

    pub last_active: i64,

    pub created_at: i64,
    pub updated_at: i64,
}

impl SqlxPopulateId for User {
    fn sql_populate_id(&mut self) -> Result<()> {
        if *self.id.hash() == [0u8; 32] && !self.user_id.is_empty() {
            self.id = blake3_hash(self.user_id.as_bytes());
            Ok(())
        } else {
            anyhow::bail!("User id is already populated");
        }
    }
}
