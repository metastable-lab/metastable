use std::sync::Arc;

use anyhow::Result;
use sqlx::{PgPool, types::Uuid};

use tokio::sync::mpsc;
use voda_database::{
    SqlxCrud, QueryCriteria, SqlxFilterQuery
};
use voda_runtime::{Memory, SystemConfig};
use voda_runtime_mem0::{Mem0Engine, Mem0Messages};

use crate::RoleplaySession;

use super::message::RoleplayMessage;

#[derive(Clone)]
pub struct RoleplayRawMemory {
    db: Arc<PgPool>,
    mem0: Arc<Mem0Engine>,

    mem0_messages_tx: mpsc::Sender<Vec<Mem0Messages>>,
}

impl RoleplayRawMemory {
    pub async fn new(
        db: Arc<PgPool>, pgvector_db: Arc<PgPool>, 
        mem0_messages_tx: mpsc::Sender<Vec<Mem0Messages>>
    ) -> Result<Self> {
        let mut mem0 = Mem0Engine::new(db.clone(), pgvector_db).await?;
        mem0.initialize().await?;
        Ok(Self { db, mem0: Arc::new(mem0), mem0_messages_tx })
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
                .add_valued_filter("id", "=", messages[0].session_id),
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Session not found"))?;

        let user = session.fetch_owner(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] User not found"))?;

        for message in messages {
            let m = message.clone();
            let created_m = m.create(&mut *tx).await?;
            session.append_message_to_history(&created_m.id, &mut *tx).await?;
        }
        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Character not found"))?;
        tx.commit().await?;

        let mem0_messages = messages.iter().map(|m| 
                Mem0Messages {
                    id: m.id,
                    user_id: m.owner,
                    character_id: Some(character.id.clone()),
                    session_id: Some(m.session_id.clone()),
                    user_aka: user.user_aka.clone(),
                    content_type: m.content_type.clone(),
                    role: m.role.clone(),
                    content: m.content.clone(),
                    created_at: m.created_at,
                    updated_at: m.updated_at,
                }
            ).collect::<Vec<_>>();

        self.mem0_messages_tx.send(mem0_messages).await
            .expect("[RoleplayRawMemory::add_messages] Failed to send mem0 messages");
        Ok(())
    }

    async fn search(&self, message: &RoleplayMessage, _limit: u64) -> Result<
        (Vec<RoleplayMessage>, SystemConfig)
    > {
        let mut tx = self.db.begin().await?;

        let criteria = QueryCriteria::new()
            .add_valued_filter("id", "=", message.session_id.clone());
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

        let mem0_query = Mem0Messages {
            id: message.id,
            user_id: message.owner,
            character_id: Some(character.id.clone()),
            session_id: Some(message.session_id.clone()),
            user_aka: user.user_aka.clone(),
            content_type: message.content_type.clone(),
            role: message.role.clone(),
            content: message.content.clone(),
            created_at: message.created_at,
            updated_at: message.updated_at,
        };

        let (mem0_messages, _) = self.mem0.search(&mem0_query, 100).await?;
        // NOTE: mem0 search ALWAYS returns 2 messages
        // the first is the memory
        // the second is the relationship
        messages.push(RoleplayMessage::from_mem0_messages(
            &message.session_id, &mem0_messages[0], &mem0_messages[1]
        ));

        if history.len() <= 10 {
            messages.extend(history.iter().cloned());
        } else {
            messages.extend(history[history.len() - 10..].iter().cloned());
        }
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
            .add_filter("id", " = ANY($1)", Some(message_ids.to_vec().clone()));

        RoleplayMessage::delete_by_criteria(criteria, &*self.db).await?;
        Ok(())
    }

    async fn reset(&self, user_id: &Uuid) -> Result<()> {
        let mut tx = self.db.begin().await?;
        RoleplaySession::delete_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone()),
            &mut *tx
        ).await?;

        RoleplayMessage::delete_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("owner", "=", user_id.clone()),
            &mut *tx
        ).await?;

        tx.commit().await?;
        Ok(())
    }
}
