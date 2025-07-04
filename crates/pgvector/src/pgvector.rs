use std::{f64, sync::Arc};
use pgvector::Vector as VectorType;
use anyhow::Result;
use sqlx::{PgPool, types::Uuid, Row};
use serde_json::Value;
use async_trait::async_trait;

/// Vector trait defining the interface for vector operations
#[async_trait]
pub trait Vector {
    /// Add embeddings to the vector store
    async fn add_embeddings(&self, embeddings: Vec<EmbeddingData>) -> Result<Vec<Uuid>>;
    
    /// Search for similar embeddings
    async fn search_embeddings(&self, query_embedding: &EmbeddingData, limit: usize) -> Result<Vec<SearchResult>>;
    
    /// Delete embeddings by IDs
    async fn delete_embeddings(&self, ids: Vec<Uuid>) -> Result<u64>;
    
    /// Update embeddings by IDs
    async fn update_embeddings(&self, updates: Vec<EmbeddingUpdate>) -> Result<u64>;
    
    /// Get embedding by ID
    async fn get_embedding(&self, id: &Uuid) -> Result<Option<EmbeddingData>>;
    
    /// Initialize the vector store (create tables, indexes, etc.)
    async fn initialize(&self) -> Result<()>;
}

/// Embedding data structure
#[derive(Debug, Clone)]
pub struct EmbeddingData {
    pub id: Uuid,
    pub user_id: Uuid,
    pub model_id: Uuid,
    pub embedding: VectorType,
    pub content: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl EmbeddingData {
    pub fn new(embedding: Vec<f32>, user_id: Uuid, model_id: Uuid, content: Option<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Self {
            id: Uuid::new_v4(),
            user_id,
            model_id,
            embedding: embedding.into(),
            content,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Search result structure
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub embedding_data: EmbeddingData,
    pub similarity: f64,
}

/// Embedding update structure
#[derive(Debug, Clone)]
pub struct EmbeddingUpdate {
    pub id: Uuid,
    pub embedding: VectorType,
    pub content: Option<String>,
}

/// PGVector implementation using PgPool from sqlx_postgres.rs
#[derive(Clone)]
pub struct PgVector {
    pool: Arc<PgPool>,
}

impl PgVector {
    /// Create a new PgVector instance using existing PgPool
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    /// Create vector index
    async fn create_vector_index(&self) -> Result<()> {
        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS embeddings_hnsw_idx 
            ON embeddings USING hnsw (embedding vector_cosine_ops) 
            WITH (m = 16, ef_construction = 64)
        "#)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Vector for PgVector {
    async fn initialize(&self,) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        // test, drop old table, support vector of any dimension
        sqlx::query("DROP TABLE IF EXISTS embeddings").execute(&mut *tx).await?;
        // Create embeddings table with dynamic dimension support
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS embeddings (
                id UUID PRIMARY KEY,
                user_id UUID NOT NULL,
                model_id UUID NOT NULL,
                embedding vector,
                content TEXT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )
        "#)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        // Create vector index
        self.create_vector_index().await?;
        
        Ok(())
    }

    async fn add_embeddings(&self, embeddings: Vec<EmbeddingData>) -> Result<Vec<Uuid>> {
        if embeddings.is_empty() {
            return Ok(vec![]);
        }
        
        let mut tx = self.pool.begin().await?;
        
        let mut ids = Vec::new();
        let mut seen_contents = std::collections::HashSet::new();
        
        for embedding_data in embeddings {
            // deduplicate: if content already exists, skip
            if let Some(content) = &embedding_data.content {
                if !seen_contents.insert(content.clone()) {
                    continue;
                }
            }
            
            sqlx::query(r#"
                INSERT INTO embeddings (id, user_id, model_id, embedding, content, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#)
            .bind(embedding_data.id)
            .bind(embedding_data.user_id)
            .bind(embedding_data.model_id)
            .bind(&embedding_data.embedding)
            .bind(&embedding_data.content)
            .bind(embedding_data.created_at)
            .bind(embedding_data.updated_at)
            .execute(&mut *tx)
            .await?;
            
            ids.push(embedding_data.id);
        }
        
        tx.commit().await?;
        Ok(ids)
    }

    async fn search_embeddings(&self, query_embedding: &EmbeddingData, limit: usize) -> Result<Vec<SearchResult>> {
        let rows = sqlx::query(r#"
            SELECT id, user_id, model_id, embedding, content, created_at, updated_at,
                   1 - (embedding <=> $1) as similarity
            FROM embeddings
            WHERE user_id = $3
            ORDER BY embedding <=> $1
            LIMIT $2
        "#)
        .bind(&query_embedding.embedding)
        .bind(limit as i64)
        .bind(query_embedding.user_id)
        .fetch_all(&*self.pool)
        .await?;
        
        let mut results = Vec::new();
        let mut seen_contents = std::collections::HashSet::new();
        
        for row in rows {
            let embedding_data = EmbeddingData {
                id: row.get("id"),
                user_id: row.get("user_id"),
                model_id: row.get("model_id"),
                embedding: row.get("embedding"),
                content: row.get("content"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            let similarity: f64 = row.get("similarity");
            
            // deduplicate: if content already exists, skip
            if let Some(content) = &embedding_data.content {
                if !seen_contents.insert(content.clone()) {
                    continue;
                }
            }
            
            results.push(SearchResult { embedding_data, similarity });
        }
        
        Ok(results)
    }

    async fn delete_embeddings(&self, ids: Vec<Uuid>) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        
        let result = sqlx::query("DELETE FROM embeddings WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&*self.pool)
            .await?;
        
        Ok(result.rows_affected())
    }

    async fn update_embeddings(&self, updates: Vec<EmbeddingUpdate>) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }
        
        let mut tx = self.pool.begin().await?;
        let mut updated_count = 0;
        
        for update in updates {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            
            let result = sqlx::query(r#"
                UPDATE embeddings 
                SET embedding = $2, 
                    content = COALESCE($3, content),
                    updated_at = $4
                WHERE id = $1
            "#)
            .bind(update.id)
            .bind(&update.embedding)
            .bind(&update.content)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            
            updated_count += result.rows_affected();
        }
        
        tx.commit().await?;
        Ok(updated_count)
    }
    
    async fn get_embedding(&self, id: &Uuid) -> Result<Option<EmbeddingData>> {
        let row = sqlx::query(r#"
            SELECT id, user_id, model_id, embedding, content, created_at, updated_at
            FROM embeddings WHERE id = $1
        "#)
        .bind(id)
        .fetch_optional(&*self.pool)
        .await?;
        
        if let Some(row) = row {
            let embedding_data = EmbeddingData {
                id: row.get("id"),
                user_id: row.get("user_id"),
                model_id: row.get("model_id"),
                embedding: row.get("embedding"),
                content: row.get("content"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            Ok(Some(embedding_data))
        } else {
            Ok(None)
        }
    }
}

// Extension methods for PgVector
impl PgVector {
    /// Search embeddings with similarity threshold
    pub async fn search_embeddings_with_threshold(
        &self,
        query_embedding: EmbeddingData,
        limit: usize,
        similarity_threshold: f32,
    ) -> Result<Vec<SearchResult>> {
        let rows = sqlx::query(r#"
            SELECT id, user_id, model_id, embedding, content, created_at, updated_at,
                   1 - (embedding <=> $1) as similarity
            FROM embeddings
            WHERE 1 - (embedding <=> $1) >= $3
            ORDER BY embedding <=> $1
            LIMIT $2
        "#)
        .bind(&query_embedding.embedding)
        .bind(limit as i64)
        .bind(similarity_threshold)
        .fetch_all(&*self.pool)
        .await?;
        
        let mut results = Vec::new();
        for row in rows {
            let embedding_data = EmbeddingData {
                id: row.get("id"),
                user_id: row.get("user_id"),
                model_id: row.get("model_id"),
                embedding: row.get("embedding"),
                content: row.get("content"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            let similarity: f64 = row.get("similarity");
            results.push(SearchResult { embedding_data, similarity });
        }
        
        Ok(results)
    }
    
    /// Search embeddings with metadata filter
    pub async fn search_embeddings_with_metadata(
        &self,
        query_embedding: EmbeddingData,
        metadata_filter: Value,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let rows = sqlx::query(r#"
            SELECT id, user_id, model_id, embedding, content, created_at, updated_at,
                   1 - (embedding <=> $1) as similarity
            FROM embeddings
            WHERE metadata @> $3
            ORDER BY embedding <=> $1
            LIMIT $2
        "#)
        .bind(&query_embedding.embedding)
        .bind(limit as i64)
        .bind(&metadata_filter)
        .fetch_all(&*self.pool)
        .await?;
        
        let mut results = Vec::new();
        for row in rows {
            let embedding_data = EmbeddingData {
                id: row.get("id"),
                user_id: row.get("user_id"),
                model_id: row.get("model_id"),
                embedding: row.get("embedding"),
                content: row.get("content"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            let similarity: f64 = row.get("similarity");
            results.push(SearchResult { embedding_data, similarity });
        }
        
        Ok(results)
    }
    
    /// Get all embeddings with pagination
    pub async fn get_all_embeddings(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<EmbeddingData>> {
        let rows = sqlx::query(r#"
            SELECT id, user_id, model_id, embedding, content, created_at, updated_at
            FROM embeddings
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
        "#)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&*self.pool)
        .await?;
        
        let mut embeddings = Vec::new();
        for row in rows {
            let embedding_data = EmbeddingData {
                id: row.get("id"),
                user_id: row.get("user_id"),
                model_id: row.get("model_id"),
                embedding: row.get("embedding"),
                content: row.get("content"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            embeddings.push(embedding_data);
        }
        
        Ok(embeddings)
    }
    
    /// Count total embeddings
    pub async fn count_embeddings(&self) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM embeddings")
            .fetch_one(&*self.pool)
            .await?;
        
        let count: i64 = row.get("count");
        Ok(count as u64)
    }
    
    /// Delete embeddings by content (for a specific user)
    pub async fn delete_embeddings_by_content(&self, user_id: &str, content_keyword: &str) -> Result<u64> {
        let user_uuid = sqlx::types::Uuid::parse_str(user_id)?;
        
        let result = sqlx::query(r#"
            DELETE FROM embeddings 
            WHERE user_id = $1 AND content ILIKE $2
        "#)
        .bind(user_uuid)
        .bind(format!("%{}%", content_keyword))
        .execute(&*self.pool)
        .await?;
        
        Ok(result.rows_affected())
    }
    
    /// Reset all embeddings (for testing)
    pub async fn reset(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        sqlx::query("DELETE FROM embeddings")
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(())
    }

    pub async fn drop(&self) -> Result<()> {
      let mut tx = self.pool.begin().await?;
      
      sqlx::query("DROP TABLE embeddings")
          .execute(&mut *tx)
          .await?;
      
      tx.commit().await?;
      Ok(())
  }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn get_test_pool() -> Result<Arc<PgPool>> {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        
        let pool = PgPool::connect(&database_url).await?;
        Ok(Arc::new(pool))
    }

    #[tokio::test]
    async fn test_pgvector_integration() -> Result<()> {
        // skip test if no database connection
        if std::env::var("DATABASE_URL").is_err() {
            println!("Skipping database test - no DATABASE_URL set");
            return Ok(());
        }

        let pool = get_test_pool().await?;
        let pg_vector = PgVector::new(pool);
        
        // test initialize
        match pg_vector.initialize().await {
            Ok(_) => println!("pgvector initialized successfully"),
            Err(e) => {
                println!("Skipping database test - pgvector initialization failed: {}", e);
                println!("This usually means pgvector extension is not available in the database");
                return Ok(());
            }
        }
        
        pg_vector.reset().await?;
        
        // test different dimensions
        let embeddings = vec![
            EmbeddingData::new(
                vec![0.1, 0.2, 0.3],
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("3D technical documents".to_string())
            ),
            EmbeddingData::new(
                vec![0.2, 0.3, 0.4, 0.5, 0.6],
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("5D news document".to_string())
            ),
            EmbeddingData::new(
                vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some("10D technical documents".to_string())
            ),
        ];
        
        let ids = pg_vector.add_embeddings(embeddings).await?;
        assert_eq!(ids.len(), 3);
        
        // test search 3d
        let query_3d = EmbeddingData::new(
            vec![0.1, 0.2, 0.3],
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("3D query".to_string())
        );
        let results_3d = pg_vector.search_embeddings(&query_3d, 5).await?;
        assert!(!results_3d.is_empty());
        // test similarity most high
        let three_dim_results: Vec<_> = results_3d.iter()
            .filter(|r| r.embedding_data.embedding.as_slice().len() == 3)
            .collect();
        assert!(!three_dim_results.is_empty());
        assert!(three_dim_results[0].similarity > 0.0);
        
        // test search 5d
        let query_5d = EmbeddingData::new(
            vec![0.2, 0.3, 0.4, 0.5, 0.6],
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("5D query".to_string())
        );
        let results_5d = pg_vector.search_embeddings(&query_5d, 5).await?;
        assert!(!results_5d.is_empty());
        let five_dim_results: Vec<_> = results_5d.iter()
            .filter(|r| r.embedding_data.embedding.as_slice().len() == 5)
            .collect();
        assert!(!five_dim_results.is_empty());
        assert!(five_dim_results[0].similarity > 0.0);
        
        // test search 10d
        let query_10d = EmbeddingData::new(
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("10D query".to_string())
        );
        let results_10d = pg_vector.search_embeddings(&query_10d, 5).await?;
        assert!(!results_10d.is_empty());
        // test similarity most high
        let ten_dim_results: Vec<_> = results_10d.iter()
            .filter(|r| r.embedding_data.embedding.as_slice().len() == 10)
            .collect();
        assert!(!ten_dim_results.is_empty());
        assert!(ten_dim_results[0].similarity > 0.0);
        
        // test get embedding
        if let Some(id) = ids.first() {
            let embedding = pg_vector.get_embedding(id).await?;
            assert!(embedding.is_some());
            let embedding = embedding.unwrap();
            assert_eq!(embedding.id, *id);
            assert_eq!(embedding.content, Some("3维技术文档".to_string()));
        }
        
        // test update
        if let Some(id) = ids.get(1) {
            let update = EmbeddingUpdate {
                id: *id,
                embedding: vec![0.9, 0.8, 0.7, 0.6, 0.5].into(), // 5维向量
                content: Some("The updated 5-dimensional document".to_string()),
            };
            
            let updated_count = pg_vector.update_embeddings(vec![update]).await?;
            assert_eq!(updated_count, 1);
            
            let updated_embedding = pg_vector.get_embedding(id).await?;
            assert!(updated_embedding.is_some());
            let updated_embedding = updated_embedding.unwrap();
            assert_eq!(updated_embedding.content, Some("The updated 5-dimensional document".to_string()));
        }
        
        // test delete
        if let Some(id) = ids.last() {
            let deleted_count = pg_vector.delete_embeddings(vec![*id]).await?;
            assert_eq!(deleted_count, 1);
            
            let deleted_embedding = pg_vector.get_embedding(id).await?;
            assert!(deleted_embedding.is_none());
        }
        
        pg_vector.reset().await?;
        
        Ok(())
    }
}