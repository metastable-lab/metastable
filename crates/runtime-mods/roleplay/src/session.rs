use serde::{Deserialize, Serialize};
use anyhow::Result;
use sqlx::{Postgres, types::Uuid};

use metastable_database::SqlxObject;
use metastable_runtime::{User, SystemConfig, Message};

use crate::Character;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_sessions"]
pub struct RoleplaySession {
    #[serde(rename = "_id")]
    pub id: Uuid,

    pub public: bool,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "system_configs", related_rust_type = "SystemConfig")]
    pub system_config: Uuid,

    #[foreign_key_many(referenced_table = "messages", related_rust_type = "Message")]
    pub history: Vec<Uuid>,

    pub use_character_memory: bool,
    pub hidden: bool,

    pub updated_at: i64,
    pub created_at: i64,
}

impl RoleplaySession {
    /// Atomically appends a message ID to the session's history in the database.
    pub async fn append_message_to_history<'e, Exe>(
        &mut self,
        message_id_to_add: &Uuid,
        executor: Exe,
    ) -> Result<(), sqlx::Error>
    where
        Exe: sqlx::Executor<'e, Database = Postgres> + Send,
    {
        sqlx::query(
            r#"
            UPDATE "roleplay_sessions"
            SET 
                history = array_append(history, $1)
            WHERE id = $2
            "#,
        )
        .bind(message_id_to_add)
        .bind(self.id)
        .execute(executor)
        .await?;

        self.history.push(message_id_to_add.clone());

        Ok(())
    }
}
