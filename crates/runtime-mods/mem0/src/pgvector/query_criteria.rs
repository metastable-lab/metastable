use anyhow::Result;
use pgvector::Vector;
use sqlx::{postgres::PgArguments};

use crate::{DEFAULT_VECTOR_DB_SEARCH_LIMIT, Mem0Filter};

pub struct VectorQueryCriteria<'a> {
    embedding_ref: &'a Vector,
    filter: Mem0Filter,
    limit: usize,
    similarity_threshold: Option<f32>,
}

impl<'a> VectorQueryCriteria<'a> {
    pub fn new(query_embedding: &'a Vector, filter: Mem0Filter) -> Self {
        Self {
            embedding_ref: query_embedding,
            filter,
            limit: DEFAULT_VECTOR_DB_SEARCH_LIMIT,
            similarity_threshold: None,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
    
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = Some(threshold);
        self
    }

    pub fn build_query(self) -> Result<(String, PgArguments)> {
        use sqlx::Arguments;
        let mut arguments = PgArguments::default();

        arguments.add(self.embedding_ref)
            .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add embedding to arguments: {}", e))?;

        let mut conditions = Vec::new();

        let placeholder = format!("${}", arguments.len() + 1);
        conditions.push(format!("user_id = {}", placeholder));
        arguments.add(self.filter.user_id)
            .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add user_id to arguments: {}", e))?;

        if let Some(character_id) = self.filter.character_id {
            let placeholder = format!("${}", arguments.len() + 1);
            conditions.push(format!("character_id = {}", placeholder));
            arguments.add(character_id)
                .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add character_id to arguments: {}", e))?;
        }

        if let Some(session_id) = self.filter.session_id {
            let placeholder = format!("${}", arguments.len() + 1);
            conditions.push(format!("session_id = {}", placeholder));
            arguments.add(session_id)
                .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add session_id to arguments: {}", e))?;
        }

        if let Some(threshold) = self.similarity_threshold {
            let placeholder = format!("${}", arguments.len() + 1);
            conditions.push(format!("1 - (embedding <=> $1) >= {}", placeholder));
            arguments.add(threshold)
                .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add similarity_threshold to arguments: {}", e))?;
        }

        let mut query = String::from("SELECT id, user_id, character_id, session_id, embedding, content, created_at, updated_at, 1 - (embedding <=> $1) as similarity FROM embeddings");

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY embedding <=> $1");

        let placeholder = format!("${}", arguments.len() + 1);
        query.push_str(&format!(" LIMIT {}", placeholder));
        arguments.add(self.limit as i64)
            .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add limit to arguments: {}", e))?;

        Ok((query, arguments))
    }
}
