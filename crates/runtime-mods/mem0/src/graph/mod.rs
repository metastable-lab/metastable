mod search;

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use neo4rs::query;

use crate::{Mem0Engine, EMBEDDING_DIMS};
use crate::raw_message::GraphEntities;

impl Mem0Engine {
    pub async fn graph_db_initialize(&self) -> Result<()> {
        let mut tx = self.get_graph_db().start_txn().await?;

        let create_vector_index = format!(
            "CREATE VECTOR INDEX memzero IF NOT EXISTS FOR (n:Entity) ON (n.embedding) OPTIONS {{indexConfig: {{`vector.dimensions`: {}, `vector.similarity_function`: 'cosine'}}}}",
            EMBEDDING_DIMS
        );
        let _ = tx.run(query(&create_vector_index)).await;

        let create_user_id_index = "CREATE INDEX entity_user_id_index IF NOT EXISTS FOR (n:Entity) ON (n.user_id)";
        let _ = tx.run(query(create_user_id_index)).await;

        tx.commit().await?;
        Ok(())
    }

    pub async fn graph_db_add(&self, message: &GraphEntities) -> Result<usize> {
        tracing::debug!("[Mem0Engine::graph_db_add] Adding graph entities: {:?}", message);
        let user_id = message.filter.user_id;
        let character_id = message.filter.character_id;
        let session_id = message.filter.session_id;

        let mut entity_names = HashSet::new();
        for relationship in &message.relationships {
            entity_names.insert(relationship.source.clone());
            entity_names.insert(relationship.destination.clone());
        }
        let entity_names: Vec<String> = entity_names.into_iter().collect();
        if entity_names.is_empty() { return Ok(0); }

        let embeddings = self.embed(entity_names.clone()).await?;
        let name_to_embedding: HashMap<String, Vec<f32>> = entity_names.iter().cloned().zip(embeddings).collect();

        let mut name_to_id = HashMap::new();
        for name in &entity_names {
            let embedding = name_to_embedding.get(name).unwrap();
            let id = self.graph_db_search_entity_with_similarity(embedding, &message.filter).await?;
                if let Some(id_val) = id {
                name_to_id.insert(name.clone(), id_val);
            }
        }

        let mut tx = self.get_graph_db().start_txn().await?;
        let mut count = 0;
        for relationship in &message.relationships {
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
                    let dest_type = message.entity_tags
                        .get(&relationship.destination)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    
                    let mut merge_properties = vec!["name: $destination_name", "user_id: $user_id"];
                    if character_id.is_some() { merge_properties.push("character_id: $character_id"); }
                    if session_id.is_some() { merge_properties.push("session_id: $session_id"); }

                    let cypher = format!(
                        "MATCH (source:Entity {{id: $source_id}}) \
                        MERGE (destination:`{}`:Entity {{{}}}) \
                        ON CREATE SET destination.created_at = timestamp(), destination.embedding = $destination_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        dest_type, merge_properties.join(", "), relationship.relationship
                    );

                    let mut q = query(&cypher)
                        .param("source_id", source_id.clone())
                        .param("destination_name", relationship.destination.clone())
                        .param("user_id", user_id.to_string())
                        .param("destination_embedding", dest_embed.clone());
                    
                    if let Some(cid) = character_id { q = q.param("character_id", cid.to_string()); }
                    if let Some(sid) = session_id { q = q.param("session_id", sid.to_string()); }
                    q
                }
                (None, Some(dest_id)) => {
                    let source_type = message.entity_tags
                        .get(&relationship.source)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());

                    let mut merge_properties = vec!["name: $source_name", "user_id: $user_id"];
                    if character_id.is_some() { merge_properties.push("character_id: $character_id"); }
                    if session_id.is_some() { merge_properties.push("session_id: $session_id"); }

                    let cypher = format!(
                        "MATCH (destination:Entity {{id: $dest_id}}) \
                        MERGE (source:`{}`:Entity {{{}}}) \
                        ON CREATE SET source.created_at = timestamp(), source.embedding = $source_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        source_type, merge_properties.join(", "), relationship.relationship
                    );
                    
                    let mut q = query(&cypher)
                        .param("dest_id", dest_id.clone())
                        .param("source_name", relationship.source.clone())
                        .param("user_id", user_id.to_string())
                        .param("source_embedding", source_embed.clone());

                    if let Some(cid) = character_id { q = q.param("character_id", cid.to_string()); }
                    if let Some(sid) = session_id { q = q.param("session_id", sid.to_string()); }
                    q
                }
                (None, None) => {
                    let source_type = message.entity_tags
                        .get(&relationship.source)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());
                    let dest_type = message.entity_tags
                        .get(&relationship.destination)
                        .cloned()
                        .unwrap_or_else(|| "Entity".to_string());

                    let mut source_merge_props = vec!["name: $source_name", "user_id: $user_id"];
                    let mut dest_merge_props = vec!["name: $dest_name", "user_id: $user_id"];

                    if character_id.is_some() {
                        source_merge_props.push("character_id: $character_id");
                        dest_merge_props.push("character_id: $character_id");
                    }
                    if session_id.is_some() {
                        source_merge_props.push("session_id: $session_id");
                        dest_merge_props.push("session_id: $session_id");
                    }

                    let cypher = format!(
                        "MERGE (source:`{}`:Entity {{{}}}) \
                        ON CREATE SET source.created_at = timestamp(), source.embedding = $source_embedding \
                        MERGE (destination:`{}`:Entity {{{}}}) \
                        ON CREATE SET destination.created_at = timestamp(), destination.embedding = $dest_embedding \
                        MERGE (source)-[r:{}]->(destination) \
                        ON CREATE SET r.created_at = timestamp(), r.updated_at = timestamp()",
                        source_type, source_merge_props.join(", "), dest_type, dest_merge_props.join(", "), relationship.relationship
                    );

                    let mut q = query(&cypher)
                        .param("source_name", relationship.source.clone())
                        .param("dest_name", relationship.destination.clone())
                        .param("source_embedding", source_embed.clone())
                        .param("dest_embedding", dest_embed.clone())
                        .param("user_id", user_id.to_string());
                    
                    if let Some(cid) = character_id { q = q.param("character_id", cid.to_string()); }
                    if let Some(sid) = session_id { q = q.param("session_id", sid.to_string()); }
                    q
                }
            };

            let mut result = tx.execute(query).await?;
            while let Ok(Some(_)) = result.next(&mut tx.handle()).await { count += 1; }
        }

        tx.commit().await?;

        Ok(count)
    }

    pub async fn graph_db_delete(&self, message: &GraphEntities) -> Result<usize> {
        let mut count = 0;
        let mut tx = self.get_graph_db().start_txn().await?;
        for relationship in &message.relationships {
            let mut source_match_props = vec!["name: $source_name", "user_id: $user_id"];
            let mut dest_match_props = vec!["name: $dest_name", "user_id: $user_id"];

            if message.filter.character_id.is_some() {
                source_match_props.push("character_id: $character_id");
                dest_match_props.push("character_id: $character_id");
            }
            if message.filter.session_id.is_some() {
                source_match_props.push("session_id: $session_id");
                dest_match_props.push("session_id: $session_id");
            }

            let cypher = format!(r#"
                MATCH (n:Entity {{{}}})
                -[r:{}]->
                (m:Entity {{{}}})
                DELETE r
            "#, source_match_props.join(", "), relationship.relationship, dest_match_props.join(", "));

            let mut final_query = query(&cypher)
                .param("source_name", relationship.source.clone())
                .param("dest_name", relationship.destination.clone())
                .param("user_id", message.filter.user_id.to_string());

            if let Some(cid) = message.filter.character_id {
                final_query = final_query.param("character_id", cid.to_string());
            }
            if let Some(sid) = message.filter.session_id {
                final_query = final_query.param("session_id", sid.to_string());
            }

            let mut result = tx.execute(final_query).await?;
            while let Ok(Some(_)) = result.next(&mut tx.handle()).await { count += 1; }
        }

        tx.commit().await?;
        Ok(count)
    }
}