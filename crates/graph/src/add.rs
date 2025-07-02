use anyhow::Result;
use neo4rs::{query, Row};
use std::collections::{HashMap, HashSet};

use crate::relation::{EntityTag, Relationship};
use crate::GraphDatabase;

impl GraphDatabase {
    pub async fn add(
        &self,
        relationships: &[Relationship],
        entity_tags: &[EntityTag],
    ) -> Result<Vec<Row>> {
        let entity_tags = EntityTag::batch_into_hashmap(entity_tags);

        // 1. Collect unique entity names
        let mut entity_names = HashSet::new();
        for r in relationships {
            entity_names.insert(r.source.clone());
            entity_names.insert(r.destination.clone());
        }
        let entity_names: Vec<String> = entity_names.into_iter().collect();
        if entity_names.is_empty() {
            return Ok(vec![]);
        }

        // 2. Embed all entities
        let embeddings = self.embed(entity_names.clone()).await?;
        let name_to_embedding: HashMap<String, Vec<f32>> =
            entity_names.iter().cloned().zip(embeddings).collect();

        // 3. Search for existing entities
        let mut name_to_id = HashMap::new();
        let user_id = &relationships[0].user_id; // Assume all have same user_id
        for name in &entity_names {
            let embedding = name_to_embedding.get(name).unwrap();
            let id = self
                .search_entity_with_similarity(embedding, user_id, None)
                .await?;
            if let Some(id_val) = id {
                name_to_id.insert(name.clone(), id_val);
            }
        }

        // 4. Start transaction and add relationships
        let mut tx = self.db.start_txn().await?;
        let mut results = Vec::new();

        for relationship in relationships {
            let source_id = name_to_id.get(&relationship.source);
            let dest_id = name_to_id.get(&relationship.destination);

            let source_embed = name_to_embedding.get(&relationship.source).unwrap();
            let dest_embed = name_to_embedding.get(&relationship.destination).unwrap();

            let query = match (source_id, dest_id) {
                (Some(source_id), Some(dest_id)) => {
                    let cypher = format!(
                        "MATCH (source:Entity {{id: $source_id}}), (destination:Entity {{id: $dest_id}}) \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        relationship.relationship
                    );
                    query(&cypher)
                        .param("source_id", source_id.clone())
                        .param("dest_id", dest_id.clone())
                }
                (Some(source_id), None) => {
                    let dest_type = entity_tags
                        .get(&relationship.destination)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    let cypher = format!(
                        "MATCH (source:Entity {{id: $source_id}}) \
                        MERGE (destination:{}:Entity {{name: $destination_name, user_id: $user_id}}) \
                        ON CREATE SET destination.created_at = timestamp(), destination.embedding = $destination_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        dest_type, relationship.relationship
                    );
                    query(&cypher)
                        .param("source_id", source_id.clone())
                        .param("destination_name", relationship.destination.clone())
                        .param("destination_embedding", dest_embed.clone())
                        .param("user_id", relationship.user_id.clone())
                }
                (None, Some(dest_id)) => {
                    let source_type = entity_tags
                        .get(&relationship.source)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    let cypher = format!(
                        "MATCH (destination:Entity {{id: $dest_id}}) \
                        MERGE (source:{}:Entity {{name: $source_name, user_id: $user_id}}) \
                        ON CREATE SET source.created_at = timestamp(), source.embedding = $source_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        source_type, relationship.relationship
                    );
                    query(&cypher)
                        .param("dest_id", dest_id.clone())
                        .param("source_name", relationship.source.clone())
                        .param("source_embedding", source_embed.clone())
                        .param("user_id", relationship.user_id.clone())
                }
                (None, None) => {
                    let source_type = entity_tags
                        .get(&relationship.source)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    let dest_type = entity_tags
                        .get(&relationship.destination)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    let cypher = format!(
                        "MERGE (source:{}:Entity {{name: $source_name, user_id: $user_id}}) \
                        ON CREATE SET source.created_at = timestamp(), source.embedding = $source_embedding \
                        MERGE (destination:{}:Entity {{name: $dest_name, user_id: $user_id}}) \
                        ON CREATE SET destination.created_at = timestamp(), destination.embedding = $dest_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        source_type, dest_type, relationship.relationship
                    );
                    query(&cypher)
                        .param("source_name", relationship.source.clone())
                        .param("dest_name", relationship.destination.clone())
                        .param("source_embedding", source_embed.clone())
                        .param("dest_embedding", dest_embed.clone())
                        .param("user_id", relationship.user_id.clone())
                }
            };
            let mut result = tx.execute(query).await?;
            while let Ok(Some(row)) = result.next(&mut tx.handle()).await {
                results.push(row);
            }
        }

        tx.commit().await?;

        Ok(results)
    }
}