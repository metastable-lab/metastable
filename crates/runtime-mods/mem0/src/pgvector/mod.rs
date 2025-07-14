mod batch;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use voda_common::get_current_timestamp;
use voda_database::{OrderDirection, QueryCriteria, SqlxFilterQuery, SqlxObject, Vector};

pub use batch::{BatchUpdateSummary, MemoryUpdateEntry, MemoryEvent};

use crate::{Mem0Engine, Mem0Filter, DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD};

#[derive(Debug, Clone, Serialize, Deserialize, SqlxObject)]
#[table_name = "embeddings"]
pub struct EmbeddingMessage {
    pub id: Uuid,

    #[indexed]
    pub user_id: Uuid,
    pub character_id: Option<Uuid>,
    pub session_id: Option<Uuid>,

    #[vector_dimension(1024)]
    pub embedding: Vector,
    pub content: String,

    pub created_at: i64,
    pub updated_at: i64,
}

impl EmbeddingMessage {
    pub async fn batch_create(mem0_engine: &Mem0Engine, raw_messages: &[String], filter: &Mem0Filter) -> Result<Vec<Self>> {
        if raw_messages.is_empty() {
            return Ok(vec![]);
        }

        let embeddings = mem0_engine.embed(raw_messages.to_vec()).await?;
        let embedding_messages = embeddings
            .iter()
            .zip(raw_messages)
            .map(|(embedding, messages)| Self {
                id: Uuid::new_v4(),
                user_id: filter.user_id,
                character_id: filter.character_id,
                session_id: filter.session_id,

                embedding: embedding.clone().into(),
                content: messages.clone(),
                created_at: get_current_timestamp(),
                updated_at: get_current_timestamp(),
            }).collect::<Vec<_>>();

        Ok(embedding_messages)
    }

    pub async fn batch_search(mem0_engine: &Mem0Engine, filter: &Mem0Filter, embeddings: &[Self], limit: i64) -> Result<Vec<Vec<Self>>> {
        let mut tx = mem0_engine.get_vector_db().begin().await?;
    
        let mut all_results = Vec::new();
        for embedding in embeddings {
            let criteria = QueryCriteria::new()
                .find_similarity(embedding.embedding.clone(), "similarity")
                .with_similarity_threshold(DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD)
                .add_filter("user_id", "=", Some(filter.user_id))
                .add_filter("character_id", "=", filter.character_id)
                .add_filter("session_id", "=", filter.session_id)
                .order_by("similarity", OrderDirection::Desc)
                .limit(limit);
            all_results.push(EmbeddingMessage::find_by_criteria(criteria, &mut *tx).await?);
        }
        tx.commit().await?;
        Ok(all_results)
    }
}