mod query_criteria;
mod batch;

use anyhow::Result;
use sqlx::types::Uuid;
use sqlx::Row;

use crate::engine::Mem0Engine;
use crate::raw_message::EmbeddingMessage;
pub use query_criteria::VectorQueryCriteria;
pub use batch::{MemoryUpdateEntry, MemoryEvent};

impl Mem0Engine {
    pub async fn vector_db_initialize(&self) -> Result<()> {
        let mut tx = self.get_vector_db().begin().await?;
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS embeddings (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id UUID,
            agent_id UUID,

            content TEXT NOT NULL,
            embedding vector(1024),

            created_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now())),
            updated_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now())
        );"#)
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

    pub async fn vector_db_add_embeddings(&self, embeddings: Vec<EmbeddingMessage>) -> Result<()> {
        if embeddings.is_empty() {
            return Ok(());
        }

        let mut tx = self.get_vector_db().begin().await?;
        for embedding in embeddings {            
            sqlx::query(r#"
                INSERT INTO embeddings (id, user_id, agent_id, embedding, content, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#)
            .bind(embedding.id)
            .bind(embedding.user_id)
            .bind(embedding.agent_id)
            .bind(embedding.embedding)
            .bind(embedding.content)
            .bind(embedding.created_at)
            .bind(embedding.updated_at)
            .execute(&mut *tx)
            .await?;
        }

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
                    user_id: row.get("user_id"),
                    agent_id: row.get("agent_id"),
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

    pub async fn vector_db_delete_embeddings(&self, ids: Vec<Uuid>) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.get_vector_db().begin().await?;
        let result = sqlx::query(r#"
            DELETE FROM embeddings WHERE id = ANY($1)
        "#)
        .bind(&ids)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("No embeddings deleted"));
        }

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn vector_db_update_embeddings(&self, updates: Vec<EmbeddingMessage>) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.get_vector_db().begin().await?;
        let mut updated_count = 0;

        for update in updates {
            let result = sqlx::query(r#"
                UPDATE embeddings SET embedding = $2, content = $3, updated_at = $4 WHERE id = $1
            "#)
            .bind(update.id)
            .bind(&update.embedding)
            .bind(&update.content)
            .bind(update.created_at)
            .execute(&mut *tx)
            .await?;

            updated_count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(updated_count)
    }   

    pub async fn vector_db_get_embedding(&self, id: Uuid) -> Result<Option<EmbeddingMessage>> {
        let mut tx = self.get_vector_db().begin().await?;
        let row = sqlx::query(r#"
            SELECT id, user_id, agent_id, embedding, content, created_at, updated_at
            FROM embeddings WHERE id = $1
        "#)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(row.map(|row| EmbeddingMessage {
            id: row.get("id"),
            user_id: row.get("user_id"),
            agent_id: row.get("agent_id"),
            embedding: row.get("embedding"),
            content: row.get("content"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn vector_db_get_all_embeddings(&self, limit: i64, offset: i64) -> Result<Vec<EmbeddingMessage>> {
        let mut tx = self.get_vector_db().begin().await?;
        let rows = sqlx::query(r#"
            SELECT id, user_id, agent_id, embedding, content, created_at, updated_at
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
            user_id: row.get("user_id"),
            agent_id: row.get("agent_id"),
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

    pub async fn vector_db_delete_embeddings_by_content(&self, user_id: Uuid, content: &str) -> Result<u64> {
        let mut tx = self.get_vector_db().begin().await?;
        let result = sqlx::query(r#"
            DELETE FROM embeddings WHERE user_id = $1 AND content ILIKE $2
        "#)
        .bind(user_id)
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