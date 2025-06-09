use anyhow::Result;
use serde::{Deserialize, Serialize};
use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};

use voda_runtime::User;

use crate::{Character, CharacterStatus};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_character_audit_logs"]
pub struct AuditLog {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: CryptoHash,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub author: CryptoHash,

    pub previous_status: CharacterStatus,
    pub new_status: CharacterStatus,

    pub created_at: i64,
}

impl SqlxPopulateId for AuditLog {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}
