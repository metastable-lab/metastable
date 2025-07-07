use anyhow::Result;
use neo4rs::query;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{Embedding, Mem0Engine, 
    DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD, DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD, DEFAULT_GRAPH_DB_SEARCH_LIMIT
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationInfo {
    pub source: String,
    pub source_id: i64,
    pub relationship: String,
    pub relation_id: i64,
    pub destination: String,
    pub destination_id: i64,
    pub similarity: f64,
}

impl Mem0Engine {
    pub async fn graph_db_search_entity_with_similarity(&self,
        embedding: &Embedding, user_id: &Uuid, agent_id: Option<Uuid>
    ) -> Result<Option<String>> {
        let agent_id_filter = if let Some(agent_id) = agent_id {
            format!("AND n.agent_id = '{}'", agent_id)
        } else {
            "".to_string()
        };

        let q = format!(r#"
            CALL db.index.vector.queryNodes("memzero", 1, $source_embedding)
            YIELD node AS candidate, score AS similarity
            WHERE candidate.user_id = $user_id
            {agent_id_filter}
            AND similarity >= {DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD}
            RETURN id(candidate);
        "#);

        let q = query(&q)
            .param("source_embedding", embedding.clone())
            .param("user_id", user_id.to_string());

        let mut result = self.get_graph_db().execute(q).await?;
        let mut results = Vec::new();
        while let Some(row) = result.next().await? {
            results.push(row);
        }

        let maybe_id = results
            .first()
            .and_then(|row| row.get("id(candidate)").unwrap_or(None));

        Ok(maybe_id)
    }

    pub async fn graph_db_search(&self,
        nodes: Vec<String>,
        user_id: Uuid,
        agent_id: Option<Uuid>
    ) -> Result<Vec<RelationInfo>> {
        let (agent_id_filter_n, agent_id_filter_m) = if let Some(agent_id) = agent_id {
            (
                format!("AND n.agent_id = '{}'", agent_id),
                format!("AND m.agent_id = '{}'", agent_id)
            )
        } else {
            ("".to_string(), "".to_string())
        };

        let mut all_relations = Vec::new();

        let embeddings = self.embed(nodes.clone()).await?;

        for embedding in embeddings {
            let query_str = format!(r#"
                MATCH (n:Entity)
                WHERE n.embedding IS NOT NULL AND n.user_id = $user_id {agent_id_filter_n}
                WITH n, round(2 * vector.similarity.cosine(n.embedding, $embedding) - 1, 4) AS similarity
                WHERE similarity >= {DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD}
                CALL {{
                    WITH n
                    MATCH (n)-[r]->(m:Entity)
                    WHERE m.user_id = $user_id {agent_id_filter_m}
                    RETURN n.name AS source, elementId(n) AS source_id, type(r) AS relationship, elementId(r) AS relation_id, m.name AS destination, elementId(m) AS destination_id
                    UNION
                    WITH n
                    MATCH (m:Entity)-[r]->(n)
                    WHERE m.user_id = $user_id {agent_id_filter_m}
                    RETURN m.name AS source, elementId(m) AS source_id, type(r) AS relationship, elementId(r) AS relation_id, n.name AS destination, elementId(n) AS destination_id
                }}
                WITH distinct source, source_id, relationship, relation_id, destination, destination_id, similarity
                RETURN
                    source,
                    source_id,
                    relationship,
                    relation_id,
                    destination,
                    destination_id,
                    similarity
                ORDER BY similarity DESC
                LIMIT {DEFAULT_GRAPH_DB_SEARCH_LIMIT}
            "#,
            agent_id_filter_n = agent_id_filter_n,
            agent_id_filter_m = agent_id_filter_m
            );

            let q = query(&query_str)
                .param("embedding", embedding)
                .param("user_id", user_id.to_string());

            let mut result = self.get_graph_db().execute(q).await?;
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
                all_relations.push(relation_info);
            }
        }

        Ok(all_relations)
    }
}