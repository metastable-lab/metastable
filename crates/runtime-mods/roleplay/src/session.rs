use serde::{Deserialize, Serialize};
use anyhow::Result;
use sqlx::Postgres;

use voda_database::{SqlxObject, SqlxPopulateId};
use voda_common::CryptoHash;
use voda_runtime::{User, SystemConfig};

use crate::Character;
use crate::message::RoleplayMessage;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_sessions"]
pub struct RoleplaySession {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub public: bool,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: CryptoHash,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: CryptoHash,

    #[foreign_key(referenced_table = "system_configs", related_rust_type = "SystemConfig")]
    pub system_config: CryptoHash,

    #[foreign_key_many(referenced_table = "roleplay_messages", related_rust_type = "RoleplayMessage")]
    pub history: Vec<CryptoHash>,

    pub updated_at: i64,
    pub created_at: i64,
}

impl SqlxPopulateId for RoleplaySession {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}

impl RoleplaySession {
    /// Atomically appends a message ID to the session's history in the database.
    pub async fn append_message_to_history<'e, Exe>(
        &mut self,
        message_id_to_add: &CryptoHash,
        executor: Exe,
    ) -> Result<(), sqlx::Error>
    where
        Exe: sqlx::Executor<'e, Database = Postgres> + Send,
    {
        let message_id_hex = message_id_to_add.to_hex_string();
        let session_id_hex = self.id.to_hex_string();

        sqlx::query(
            r#"
            UPDATE "roleplay_sessions"
            SET 
                history = array_append(history, $1)
            WHERE id = $2
            "#,
        )
        .bind(message_id_hex)
        .bind(session_id_hex)
        .execute(executor)
        .await?;

        self.history.push(message_id_to_add.clone());

        Ok(())
    }
}

