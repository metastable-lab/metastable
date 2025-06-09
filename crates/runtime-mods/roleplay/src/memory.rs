use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use sqlx::PgPool;

use voda_database::{
    SqlxCrud, SqlxPopulateId,
    QueryCriteria, OrderDirection, SqlxFilterQuery
};
use voda_runtime::Memory;
use voda_common::CryptoHash;

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

    async fn initialize(&self) -> Result<()> { Ok(()) }

    async fn add_messages(&self, messages: &[RoleplayMessage]) -> Result<()> {
        let mut tx = self.db.begin().await?;

        for message in messages {
            let mut m = message.clone();
            let criteria = QueryCriteria::by_id(&m.session_id)?;
        
            let mut session = RoleplaySession::find_one_by_criteria(criteria, &mut *tx)
                .await?
                .ok_or(anyhow::anyhow!("[RoleplayRawMemory::add_message] Session not found"))?;

            m.sql_populate_id()?;
            let created_m = m.create(&mut *tx).await?;

            session.append_message_to_history(&created_m.id, &mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_one(&self, message_id: &CryptoHash) -> Result<Option<RoleplayMessage>> {
        let criteria = QueryCriteria::by_id(message_id)?;
        let message = RoleplayMessage::find_one_by_criteria(criteria, &*self.db).await?;
        Ok(message)
    }

    async fn get_all(&self, user_id: &CryptoHash, limit: u64, offset: u64) -> Result<Vec<RoleplayMessage>> {
        let user_id_hex = user_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?
            .order_by("created_at", OrderDirection::Desc)?
            .limit(limit as i64)?
            .offset(offset as i64)?;

        let messages = RoleplayMessage::find_by_criteria(criteria, &*self.db).await?;
        Ok(messages)
    }

    async fn search(&self, message: &RoleplayMessage, _limit: u64, _offset: u64) -> Result<Vec<RoleplayMessage>> {
        let mut tx = self.db.begin().await?;

        let criteria = QueryCriteria::by_id(&message.session_id)?;
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
        Ok(messages)
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

    async fn delete(&self, message_ids: &[CryptoHash]) -> Result<()> {
        let sqlx_ids = message_ids.iter().map(|id| id.to_hex_string()).collect::<Vec<_>>();
        let criteria = QueryCriteria::new()
            .add_filter("id", " = ANY($1)", Some(sqlx_ids))?;

        RoleplayMessage::delete_by_criteria(criteria, &*self.db).await?;
        Ok(())
    }

    async fn reset(&self, user_id: &CryptoHash) -> Result<()> {
        let mut tx = self.db.begin().await?;
        let user_id_hex = user_id.to_hex_string();
        let criteria_session = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex.clone())?;
        RoleplaySession::delete_by_criteria(criteria_session, &mut *tx).await?;

        let criteria_message = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?;
        RoleplayMessage::delete_by_criteria(criteria_message, &mut *tx).await?;

        tx.commit().await?;
        Ok(())
    }
}

impl RoleplayRawMemory {
    pub async fn find_public_conversations_by_character(
        &self, character_id: &CryptoHash, limit: u64, offset: u64
    ) -> Result<Vec<RoleplaySession>> {
        let criteria = QueryCriteria::new()
            .add_valued_filter("character", "=", character_id.to_hex_string())?
            .add_valued_filter("public", "=", true)?
            .order_by("updated_at", OrderDirection::Desc)?
            .limit(limit as i64)?
            .offset(offset as i64)?;

        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;
        Ok(sessions)
    }

    pub async fn find_latest_conversations(
        &self, user_id: &CryptoHash, character_id: &CryptoHash, limit: u64
    ) -> Result<Vec<RoleplaySession>> {
        let user_id_hex = user_id.to_hex_string();
        let character_id_hex = character_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?
            .add_valued_filter("character", "=", character_id_hex)?
            .order_by("updated_at", OrderDirection::Desc)?
            .limit(limit as i64)?;

        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;
        Ok(sessions)
    }

    pub async fn find_character_list_of_user(
        &self, user_id: &CryptoHash
    ) -> Result<HashMap<CryptoHash, usize>> {
        let user_id_hex = user_id.to_hex_string();
        let criteria = QueryCriteria::new()
            .add_valued_filter("owner", "=", user_id_hex)?;
        let sessions = RoleplaySession::find_by_criteria(criteria, &*self.db).await?;

        let mut character_list = HashMap::new();
        for session in sessions {
            character_list.entry(session.character).and_modify(|count| *count += 1).or_insert(1);
        }

        Ok(character_list)
    }
}
