use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::{ModuleClient, get_current_timestamp};
use metastable_database::{OrderDirection, SqlxObject, Vector};

use crate::{EmbederClient, PgvectorClient, DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mem0Filter {
    pub user_id: Uuid,
    pub character_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MemoryEvent {
    Add,
    Update,
    Delete,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdateEntry {
    pub id: Uuid,
    pub filter: Mem0Filter,
    pub event: MemoryEvent,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateSummary {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

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
    pub async fn batch_create(embeder: &EmbederClient, raw_messages: &[String], filter: &Mem0Filter) -> Result<Vec<Self>> {
        if raw_messages.is_empty() {
            return Ok(vec![]);
        }

        let embeddings = embeder.embed(raw_messages.to_vec()).await?;
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

    pub async fn batch_search(vector_db: &PgvectorClient, filter: &Mem0Filter, embeddings: &[Self], limit: i64) -> Result<Vec<Vec<Self>>> {
        let mut tx = vector_db.get_client().begin().await?;
    
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

    pub async fn db_batch_update(embeder: &EmbederClient, vector_db: &PgvectorClient, updates: Vec<MemoryUpdateEntry>) -> Result<BatchUpdateSummary> {
        if updates.is_empty() {
            return Ok(BatchUpdateSummary { added: 0, updated: 0, deleted: 0 });
        }

        let mut to_add = Vec::new();
        let mut to_update = Vec::new();
        let mut to_delete_ids = Vec::new();

        for update in updates {
            match update.event {
                MemoryEvent::Add => to_add.push(update),
                MemoryEvent::Update => to_update.push(update),
                MemoryEvent::Delete => to_delete_ids.push(update.id),
                MemoryEvent::None => continue,
            }
        }

        let add_contents: Vec<String> = to_add.iter().map(|u| u.content.clone()).collect();
        let update_contents: Vec<String> = to_update.iter().map(|u| u.content.clone()).collect();

        let all_contents_to_embed = [add_contents.as_slice(), update_contents.as_slice()].concat();
        let embeddings = embeder.embed(all_contents_to_embed).await?;

        let (add_embeddings, update_embeddings) = embeddings.split_at(add_contents.len());

        let now = get_current_timestamp();

        let summary = BatchUpdateSummary { 
            added: to_add.len(), 
            updated: to_update.len(), 
            deleted: to_delete_ids.len() 
        };

        let add_messages: Vec<EmbeddingMessage> = to_add
            .into_iter()
            .zip(add_embeddings)
            .filter_map(|(update, embedding)| {
                Some(EmbeddingMessage {
                    id: Uuid::new_v4(),

                    user_id: update.filter.user_id,
                    character_id: update.filter.character_id,
                    session_id: update.filter.session_id,

                    embedding: embedding.clone().into(),
                    content: update.content,
                    created_at: now,
                    updated_at: now,
                })
            })
            .collect();

        let update_messages: Vec<EmbeddingMessage> = to_update
            .into_iter()
            .zip(update_embeddings)
            .filter_map(|(update, embedding)| {
                let id = update.id;
                Some(EmbeddingMessage {
                    id,

                    user_id: update.filter.user_id,
                    character_id: update.filter.character_id,
                    session_id: update.filter.session_id,

                    embedding: embedding.clone().into(),
                    content: update.content,
                    created_at: now,
                    updated_at: now,
                })
            })
            .collect();

        let mut tx = vector_db.get_client().begin().await?;

        for embedding in add_messages {
            embedding.create(&mut *tx).await?;
        }

        for update in update_messages {
            update.update(&mut *tx).await?;
        }

        if !to_delete_ids.is_empty() {
            EmbeddingMessage::delete_by_criteria(
                QueryCriteria::new().add_filter("id", " = ANY($1)", Some(to_delete_ids)),
                &mut *tx
            ).await?;
        }
        
        tx.commit().await?;
        Ok(summary)
    }
}