mod metadata;
mod points;
mod profile;
mod usage;
mod url;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use sqlx::types::Uuid;

use voda_database::SqlxObject;

pub use metadata::UserMetadata;
pub use points::UserPoints;
pub use profile::UserProfile;
pub use usage::UserUsage;
pub use url::UserUrl;

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
    pub id: Uuid,
    pub user_id: String,
    pub user_aka: String,

    pub role: UserRole,
    pub provider: String,

    pub last_active: i64,

    pub created_at: i64,
    pub updated_at: i64,
}
