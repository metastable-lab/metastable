use std::sync::Arc;

use anyhow::Result;
use sqlx::{PgPool, types::Uuid};

use voda_database::{SqlxCrud, QueryCriteria, SqlxFilterQuery};
use voda_runtime::{Memory, SystemConfig};
use voda_runtime_roleplay::{RoleplayMessage, RoleplaySession};

use crate::CharacterCreationMessage;

#[derive(Clone)]
pub struct CharacterCreationMemory {
    db: Arc<PgPool>,
    system_config_name: String,
    system_config: SystemConfig,
}

impl CharacterCreationMemory {
    pub fn new(db: Arc<PgPool>, system_config_name: String) -> Self {
        Self { db, system_config_name, system_config: SystemConfig::default() }
    }
}

#[async_trait::async_trait]
impl Memory for CharacterCreationMemory {
    type MessageType = CharacterCreationMessage;

    async fn initialize(&mut self) -> Result<()> {
        let system_config = SystemConfig::find_one_by_criteria(
            QueryCriteria::new().add_filter("name", "=", Some(self.system_config_name.clone()))?,
            &*self.db
        ).await?;
        self.system_config = system_config
            .ok_or(anyhow::anyhow!("[CharacterCreationMemory::initialize] System config not found"))?;
        Ok(())
    }

    async fn add_messages(&self, messages: &[CharacterCreationMessage]) -> Result<()> {
        if messages.len() == 0 {
            return Ok(());
        }

        let mut tx = self.db.begin().await?;
        for message in messages {
            let m = message.clone();
            m.create(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn search(&self, message: &CharacterCreationMessage, _limit: u64) -> Result<
        (Vec<CharacterCreationMessage>, SystemConfig)
    > {
        let mut tx = self.db.begin().await?;

        let criteria = QueryCriteria::new()
            .add_valued_filter("id", "=", message.roleplay_session_id.clone())?;
        let session = RoleplaySession::find_one_by_criteria(criteria, &mut *tx).await?
            .ok_or(anyhow::anyhow!("[CharacterCreationMemory::search] Session not found"))?;

        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[CharacterCreationMemory::search] Character not found"))?;
        let user = session.fetch_owner(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[CharacterCreationMemory::search] User not found"))?;
        let system_config = session.fetch_system_config(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[CharacterCreationMemory::search] System config not found"))?;
        let roleplay_messages_history = session.fetch_history(&mut *tx).await?;
        let system_message = RoleplayMessage::system(&session, &system_config, &character, &user);
        let first_message = RoleplayMessage::first_message(&session, &character, &user);

        let mut character_creation_message = CharacterCreationMessage::from_roleplay_messages(
            &system_message,
            &first_message,
            &roleplay_messages_history
        )?;

        let character_creation_system_message = CharacterCreationMessage::system_message(
            &message.roleplay_session_id,
            &message.owner,
            &self.system_config
        );
        character_creation_message.character_creation_system_config = character_creation_system_message.character_creation_system_config;

        tx.commit().await?;
        Ok((vec![
            character_creation_system_message,
            character_creation_message
        ], self.system_config.clone()))
    }

    async fn update(&self, messages: &[Self::MessageType]) -> Result<()> {
        let mut tx = self.db.begin().await?;

        for message in messages {
            let m = message.clone();
            m.update(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete(&self, message_ids: &[Uuid]) -> Result<()> {
        let criteria = QueryCriteria::new()
            .add_filter("id", " = ANY($1)", Some(message_ids.to_vec().clone()))?;

        CharacterCreationMessage::delete_by_criteria(criteria, &*self.db).await?;
        Ok(())
    }

    async fn reset(&self, user_id: &Uuid) -> Result<()> {
        CharacterCreationMessage::delete_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone())?,
            &*self.db
        ).await?;

        Ok(())
    }
}
