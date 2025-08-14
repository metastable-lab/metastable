use std::{cmp::Ordering, sync::Arc};

use anyhow::Result;
use sqlx::{PgPool, types::Uuid};

use tokio::sync::mpsc;
use metastable_database::{
    SqlxCrud, QueryCriteria, SqlxFilterQuery
};
use metastable_runtime::{Memory, MessageRole, SystemConfig};
use metastable_runtime_mem0::{Mem0Engine, Mem0Messages};

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

    pub fn get_mem0_engine_clone(&self) -> Arc<Mem0Engine> {
        self.mem0.clone()
    }
}

pub fn sort_history(history: &mut Vec<RoleplayMessage>) {
    // sort by created_at, if two messages have the same created_at stamp, ALWAYS have the user message at first
    // BEGINING of the history should always by the OLDEST history
    history.sort_by(|a, b| {
        if a.created_at == b.created_at {
            if a.role == MessageRole::User {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        } else {
            a.created_at.cmp(&b.created_at)
        }
    });
}

#[async_trait::async_trait]
impl Memory for RoleplayRawMemory {
    type MessageType = RoleplayMessage;

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn add(&self, messages: &[RoleplayMessage]) -> Result<()> {
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
        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Character not found"))?;

        for message in messages {
            let m = message.clone();
            let created_m = m.create(&mut *tx).await?;
            session.append_message_to_history(&created_m.id, &mut *tx).await?;
        }

        let mut all_history = session.fetch_history(&mut *tx).await?;
        sort_history(&mut all_history);

        let mut all_unsaved_history = all_history
                .iter()
                .filter(|h| !h.is_saved_in_memory && !h.is_removed)
                .collect::<Vec<_>>();

        if all_unsaved_history.len() >= 20 {
            let unsaved_history_to_save = all_unsaved_history.iter_mut().take(10).collect::<Vec<_>>();
            for h in unsaved_history_to_save {
                let mut h = h.clone();
                h.is_saved_in_memory = true;
                h.update(&mut *tx).await?;
            }

            let mem0_messages = all_unsaved_history.iter().map(|m| 
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

            if mem0_messages.len() > 0 {
                tracing::info!("[RoleplayRawMemory::add_messages] saving {} mem0 messages", mem0_messages.len());
                self.mem0_messages_tx.send(mem0_messages).await
                    .expect("[RoleplayRawMemory::add_messages] Failed to send mem0 messages");
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn search(&self, message: &RoleplayMessage, limit: u64) -> Result<
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

        let mut history = session.fetch_history(&mut *tx).await?;
        sort_history(&mut history);
        let history = history.iter()
            .filter(|h| !h.is_removed)
            .map(|h| h.clone())
            .collect::<Vec<_>>();

        let system_message = RoleplayMessage::system(&session, &system_config, &character, &user);
        let first_message = RoleplayMessage::first_message(&session, &character, &user);
        let mut messages = vec![system_message, first_message];

        let maybe_filter_by_session_id = if session.use_character_memory {
            None // ignore session on filter
        } else {
            Some(message.session_id.clone()) // use session id to filter
        };

        let mem0_query = Mem0Messages {
            id: message.id,
            user_id: message.owner,
            character_id: Some(character.id.clone()),
            session_id: maybe_filter_by_session_id,
            user_aka: user.user_aka.clone(),
            content_type: message.content_type.clone(),
            role: message.role.clone(),
            content: message.content.clone(),
            created_at: message.created_at,
            updated_at: message.updated_at,
        };

        let (mem0_messages, _) = self.mem0.search(&mem0_query, limit).await?;
        // NOTE: mem0 search ALWAYS returns 2 messages
        // the first is the memory
        // the second is the relationship
        messages.push(RoleplayMessage::from_mem0_messages(
            &message.session_id, &mem0_messages[0], &mem0_messages[1]
        ));

        if history.len() <= 12 {
            messages.extend(history.iter().cloned());
        } else {
            messages.extend(history[history.len() - 12..].iter().cloned());
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
        let mut tx = self.db.begin().await?;
        for message_id in message_ids {
            let message = RoleplayMessage::find_one_by_criteria(
                QueryCriteria::new().add_valued_filter("id", "=", message_id.clone()),
                &mut *tx
            ).await?;

            if let Some(message) = message {
                let mut message = message.clone();
                message.is_removed = true;
                message.update(&mut *tx).await?;
            }
        }
        tx.commit().await?;
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
