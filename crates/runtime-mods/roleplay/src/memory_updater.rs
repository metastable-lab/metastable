use anyhow::{anyhow, Result};

use metastable_common::ModuleClient;
use metastable_database::{OrderDirection, QueryCriteria, SqlxFilterQuery};
use metastable_runtime::{Agent, CharacterFeature, ChatSession, Message};
use metastable_clients::{Mem0Filter, PostgresClient};
use sqlx::types::Uuid;

use crate::agents::{ExtractFactsAgent, ExtractFactsInput, MemoryExtractorAgent, MemoryExtractorInput};

#[derive(Clone)]
pub struct MemoryUpdater {
    db: PostgresClient,

    extract_fact_agent: ExtractFactsAgent,
    memory_extractor_agent: MemoryExtractorAgent,
}

impl MemoryUpdater {
    pub async fn new() -> Result<Self> {
        let db = PostgresClient::setup_connection().await;
        let extract_fact_agent = ExtractFactsAgent::new().await?;
        let memory_extractor_agent = MemoryExtractorAgent::new().await?;
        Ok(Self { db, extract_fact_agent, memory_extractor_agent })
    }

    pub async fn update_memory(&self, session_id: &Uuid) -> Result<()> {
        let mut tx = self.db.get_client().begin().await?;
        let messages = Message::find_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("session", "=", session_id.clone())
                .add_valued_filter("is_memorizeable", "=", true)
                .add_valued_filter("is_in_memory", "=", true)
                .order_by("created_at", OrderDirection::Desc),
            &mut *tx
        ).await?;
        
        let messages = messages.iter().skip(6).collect::<Vec<_>>();
        if messages.len() < 6 {
            tracing::info!("[MemoryUpdater::update_memory] too little messages to do update");
            tx.rollback().await?;
            return Ok(())  // SKIP
        }

        let session = ChatSession::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()), 
            &mut *tx
        ).await?
            .ok_or(anyhow!("[MemoryUpdater::update_memory] Session not found"))?;
        let character = session.fetch_character(&mut *tx).await?
            .ok_or(anyhow!("[MemoryUpdater::update_memory] Character not found"))?;
        let user = session.fetch_owner(&mut *tx).await?
            .ok_or(anyhow!("[MemoryUpdater::update_memory] User not found"))?;

        let session_id_filter = if session.use_character_memory && !character.features.contains(&CharacterFeature::CharacterCreation) {
            None
        } else {
            Some(session.id)
        };
        let filter = Mem0Filter {
            user_id: user.id.clone(),
            character_id: Some(character.id),
            session_id: session_id_filter,
        };

        let raw_text = messages
                .iter()
                .map(|m| m.summary.clone())
                .filter(|s| s.is_some())
                .map(|s| s.unwrap())
                .collect::<Vec<_>>().join("\n");

        let (_, facts, _) = self.extract_fact_agent.call(&user.id, &ExtractFactsInput {
            filter: filter.clone(), new_message: raw_text,
        }).await?;

        let memory_extractor_input = MemoryExtractorInput { filter, facts };
        let (_, _, summary) = self.memory_extractor_agent.call(&user.id, &memory_extractor_input).await?;
        tracing::info!("[MemoryUpdater::update_memory] summary: {:?}", summary);

        tx.commit().await?;
        Ok(())
    }
}
