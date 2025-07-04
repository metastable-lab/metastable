use std::sync::Arc;

use anyhow::Result;
use sqlx::{PgPool, types::Uuid};

use voda_database::{
    SqlxCrud, QueryCriteria, OrderDirection, SqlxFilterQuery
};
use voda_runtime::{Memory, SystemConfig};

use crate::RoleplaySession;

use super::message::RoleplayMessage;


#[derive(Clone)]
pub struct RoleplayRawMemory {
    db: Arc<PgPool>,
}

impl RoleplayRawMemory {
    pub fn new(db: Arc<PgPool>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl Memory for RoleplayRawMemory {
    type MessageType = RoleplayMessage;

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn add_messages(&self, messages: &[RoleplayMessage]) -> Result<()> {
        if messages.len() == 0 {
            return Ok(());
        }

        let mut tx = self.db.begin().await?;

        // NOTE: ASSUME ALL MESSAGES HAVE THE SAME SESSION_ID
        let mut session = RoleplaySession::find_one_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("id", "=", messages[0].session_id)?,
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Session not found"))?;

        for message in messages {
            let m = message.clone();
            let created_m = m.create(&mut *tx).await?;
            session.append_message_to_history(&created_m.id, &mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_one(&self, message_id: &Uuid) -> Result<Option<RoleplayMessage>> {
        let message = RoleplayMessage::find_one_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("id", "=", message_id.clone())?,
            &*self.db
        ).await?;
        Ok(message)
    }

    async fn get_all(&self, user_id: &Uuid, limit: u64, offset: u64) -> Result<Vec<RoleplayMessage>> {
        let messages = RoleplayMessage::find_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone())?
                .order_by("created_at", OrderDirection::Desc)?
                .limit(limit as i64)?
                .offset(offset as i64)?,
            &*self.db
        ).await?;
        Ok(messages)
    }

    async fn search(&self, message: &RoleplayMessage, _limit: u64, _offset: u64) -> Result<
        (Vec<RoleplayMessage>, SystemConfig)
    > {
        let mut tx = self.db.begin().await?;

        let criteria = QueryCriteria::new()
            .add_valued_filter("id", "=", message.session_id.clone())?;
        let session = RoleplaySession::find_one_by_criteria(criteria, &mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::search] Session not found"))?;

        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::search] Character not found"))?;
        let user = session.fetch_owner(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::search] User not found"))?;
        let system_config = session.fetch_system_config(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::search] System config not found"))?;

        let history = session.fetch_history(&mut *tx).await?;
        let system_message = RoleplayMessage::system(&session, &system_config, &character, &user);
        let first_message = RoleplayMessage::first_message(&session, &character, &user);
        let mut messages = vec![system_message, first_message];
        messages.extend(history);
        messages.push(message.clone());

        tx.commit().await?;
        Ok((messages, system_config))
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

        RoleplayMessage::delete_by_criteria(criteria, &*self.db).await?;
        Ok(())
    }

    async fn reset(&self, user_id: &Uuid) -> Result<()> {
        let mut tx = self.db.begin().await?;
        RoleplaySession::delete_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone())?,
            &mut *tx
        ).await?;

        RoleplayMessage::delete_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone())?,
            &mut *tx
        ).await?;

        tx.commit().await?;
        Ok(())
    }
}
