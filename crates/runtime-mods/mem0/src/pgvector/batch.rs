use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_database::{QueryCriteria, SqlxCrud, SqlxFilterQuery};

use crate::pgvector::{EmbeddingMessage};
use crate::Mem0Filter;
use crate::Mem0Engine;

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

impl Mem0Engine {
    pub async fn vector_db_batch_update(&self, updates: Vec<MemoryUpdateEntry>) -> Result<BatchUpdateSummary> {
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
        let embeddings = self.embed(all_contents_to_embed).await?;

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

        let mut tx = self.get_vector_db().begin().await?;

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