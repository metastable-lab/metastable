mod query_criteria;
mod batch;

use anyhow::Result;
use sqlx::Row;

use crate::{engine::Mem0Engine, Mem0Filter};
use crate::raw_message::EmbeddingMessage;
pub use query_criteria::VectorQueryCriteria;
pub use batch::{BatchUpdateSummary, MemoryUpdateEntry, MemoryEvent};

impl Mem0Engine {
    pub async fn vector_db_initialize(&self) -> Result<()> {
        let mut tx = self.get_vector_db().begin().await?;
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS embeddings (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                user_id UUID NOT NULL,
                character_id UUID,
                session_id UUID,
                content TEXT NOT NULL,
                embedding vector(1024),
                created_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now())),
                updated_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now()))
            );
        "#)
        .execute(&mut *tx)
        .await?;

        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_embeddings_user_id ON embeddings(user_id);
        "#)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn vector_db_search_embeddings<'a>(&self, criteria: VectorQueryCriteria<'a>) -> Result<Vec<(EmbeddingMessage, f64)>> {
        let (query_str, args) = criteria.build_query()?;

        let mut tx = self.get_vector_db().begin().await?;
        let rows = sqlx::query_with(&query_str, args)
            .fetch_all(&mut *tx)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push((
                EmbeddingMessage {
                    id: row.get("id"),
                    filter: Mem0Filter {
                        user_id: row.get("user_id"),
                        character_id: row.get("character_id"),
                        session_id: row.get("session_id"),
                    },
                    embedding: row.get("embedding"),
                    content: row.get("content"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                },
                row.get("similarity"),
            ));
        }

        tx.commit().await?;

        Ok(results)
    }

    pub async fn vector_db_get_all_embeddings(&self, limit: i64, offset: i64) -> Result<Vec<EmbeddingMessage>> {
        let mut tx = self.get_vector_db().begin().await?;
        let rows = sqlx::query(r#"
            SELECT id, user_id, character_id, session_id, embedding, content, created_at, updated_at
            FROM embeddings
            ORDER BY created_at DESC
            LIMIT $1
            OFFSET $2
        "#)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await?;

        let embeddings = rows.into_iter().map(|row| EmbeddingMessage {
            id: row.get("id"),
            filter: Mem0Filter {
                user_id: row.get("user_id"),
                character_id: row.get("character_id"),
                session_id: row.get("session_id"),
            },
            embedding: row.get("embedding"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect();

        tx.commit().await?;
        Ok(embeddings)
    }

    pub async fn vector_db_count_embeddings(&self) -> Result<i64> {
        let mut tx = self.get_vector_db().begin().await?;
        let row = sqlx::query(r#"
            SELECT COUNT(*) FROM embeddings
        "#)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(row.get("count"))
    }

    pub async fn vector_db_delete_embeddings_by_content(&self, filter: Mem0Filter, content: &str) -> Result<u64> {
        let mut tx = self.get_vector_db().begin().await?;
        let result = sqlx::query(r#"
            DELETE FROM embeddings WHERE user_id = $1 AND character_id = $2 AND session_id = $3 AND content ILIKE $4
        "#)
        .bind(filter.user_id)
        .bind(filter.character_id)
        .bind(filter.session_id)
        .bind(format!("%{}%", content))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn vector_db_reset(&self) -> Result<()> {
        let mut tx = self.get_vector_db().begin().await?;
        sqlx::query("DELETE FROM embeddings")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn vector_db_drop(&self) -> Result<()> {
        let mut tx = self.get_vector_db().begin().await?;
        sqlx::query("DROP TABLE IF EXISTS embeddings")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}