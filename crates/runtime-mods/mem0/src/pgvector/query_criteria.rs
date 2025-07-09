use anyhow::Result;
use pgvector::Vector;
use sqlx::{postgres::PgArguments, types::Uuid};

use crate::DEFAULT_VECTOR_DB_SEARCH_LIMIT;

pub struct VectorQueryCriteria<'a> {
    embedding_ref: &'a Vector,
    user_id: Uuid,
    agent_id: Option<Uuid>,
    limit: usize,
    similarity_threshold: Option<f32>,
}

impl<'a> VectorQueryCriteria<'a> {
    pub fn new(query_embedding: &'a Vector, user_id: Uuid) -> Self {
        Self {
            embedding_ref: query_embedding,
            user_id,
            agent_id: None,
            limit: DEFAULT_VECTOR_DB_SEARCH_LIMIT,
            similarity_threshold: None,
        }
    }

    pub fn with_agent_id(mut self, agent_id: Option<Uuid>) -> Self {
        self.agent_id = agent_id;
        self
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
        arguments.add(self.user_id)
            .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add user_id to arguments: {}", e))?;

        if let Some(agent_id) = self.agent_id {
            let placeholder = format!("${}", arguments.len() + 1);
            conditions.push(format!("agent_id = {}", placeholder));
            arguments.add(agent_id)
                .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add agent_id to arguments: {}", e))?;
        }

        if let Some(threshold) = self.similarity_threshold {
            let placeholder = format!("${}", arguments.len() + 1);
            conditions.push(format!("1 - (embedding <=> $1) >= {}", placeholder));
            arguments.add(threshold)
                .map_err(|e| anyhow::anyhow!("[VectorQueryCriteria::build_query] Failed to add similarity_threshold to arguments: {}", e))?;
        }

        let mut query = String::from("SELECT id, user_id, agent_id, embedding, content, created_at, updated_at, 1 - (embedding <=> $1) as similarity FROM embeddings");

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
