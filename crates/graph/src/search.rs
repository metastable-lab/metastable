use anyhow::Result;
use neo4rs::query;
use serde::Serialize;

use crate::{
    Embedding,
    GraphDatabase,
};

pub const VECTOR_SIMILARITY_THRESHOLD: f32 = 0.9;
pub const TEXT_SIMILARITY_THRESHOLD: f32 = 0.7;
pub const SEARCH_LIMIT: i32 = 100;

#[derive(Debug, Serialize)]
struct RelationInfo {
    source: String,
    source_id: i64,
    relationship: String,
    relation_id: i64,
    destination: String,
    destination_id: i64,
    similarity: f64,
}

impl GraphDatabase {
    pub async fn search_entity_with_similarity(&self,
        embedding: &Embedding,
        user_id: &str, agent_id: Option<String>
    ) -> Result<Option<String>> {
        let agent_id_filter = if let Some(agent_id) = agent_id {
            format!("AND n.agent_id = '{}'", agent_id)
        } else {
            "".to_string()
        };

        let q = format!(r#"
            CALL vector_search.search("memzero", 1, $source_embedding)
            YIELD distance, node, similarity
            WITH node AS candidate, similarity
            WHERE candidate.user_id = $user_id
            {agent_id_filter}
            AND similarity >= {VECTOR_SIMILARITY_THRESHOLD}
            RETURN id(candidate);
        "#);

        let q = query(&q)
            .param("source_embedding", embedding.clone())
            .param("user_id", user_id);

        let mut result = self.db.execute(q).await?;
        let mut results = Vec::new();
        while let Some(row) = result.next().await? {
            results.push(row);
        }

        let maybe_id = results
            .first()
            .and_then(|row| row.get("id(candidate)").unwrap_or(None));

        Ok(maybe_id)
    }

    pub async fn search(&self,
        nodes: Vec<String>,
        user_id: String,
        agent_id: Option<String>
    ) -> Result<Vec<String>> {
        let agent_id_filter = if let Some(agent_id) = agent_id {
            format!("AND found_node.agent_id = '{}'", agent_id)
        } else {
            "".to_string()
        };

        let mut all_relations = Vec::new();

        let embeddings = self.embed(nodes.clone()).await?;

        for embedding in embeddings {
            let query_str = format!(r#"
                CALL vector_search.search("memzero", 1, $embedding)
                YIELD node AS found_node, similarity
                WITH found_node, similarity
                WHERE found_node.user_id = $user_id {agent_id_filter} AND similarity >= {SIMILARITY_THRESHOLD}
                MATCH (found_node)-[r]-(b:Entity)
                WITH r, similarity, startNode(r) AS start, endNode(r) AS end
                RETURN
                    start.name AS source,
                    id(start) AS source_id,
                    type(r) AS relationship,
                    id(r) AS relation_id,
                    end.name AS destination,
                    id(end) AS destination_id,
                    similarity
                ORDER BY similarity DESC
                LIMIT {SEARCH_LIMIT}
            "#,
            agent_id_filter = agent_id_filter,
            SIMILARITY_THRESHOLD = TEXT_SIMILARITY_THRESHOLD,
            SEARCH_LIMIT = SEARCH_LIMIT
            );

            let q = query(&query_str)
                .param("embedding", embedding)
                .param("user_id", user_id.clone());

            let mut result = self.db.execute(q).await?;
            while let Some(row) = result.next().await? {
                let relation_info = RelationInfo {
                    source: row.get("source").unwrap_or_default(),
                    source_id: row.get("source_id").unwrap_or_default(),
                    relationship: row.get("relationship").unwrap_or_default(),
                    relation_id: row.get("relation_id").unwrap_or_default(),
                    destination: row.get("destination").unwrap_or_default(),
                    destination_id: row.get("destination_id").unwrap_or_default(),
                    similarity: row.get("similarity").unwrap_or_default(),
                };
                all_relations.push(serde_json::to_string(&relation_info)?);
            }
        }

        Ok(all_relations)
    }
}