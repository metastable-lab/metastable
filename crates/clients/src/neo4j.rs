use std::{collections::{HashMap, HashSet}, env};

use anyhow::Result;
use metastable_common::{define_module_client, ModuleClient};
use neo4rs::{query, ConfigBuilder, Graph};
use serde::{Deserialize, Serialize};

use crate::{Embedding, EmbederClient, EMBEDDING_DIMS, DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD, DEFAULT_GRAPH_DB_SEARCH_LIMIT, DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD, Mem0Filter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship { // TOOLCALL Return
    pub source: String,
    pub relationship: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntities {
    pub relationships: Vec<Relationship>,
    pub entity_tags: HashMap<String, String>,
    pub filter: Mem0Filter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityTag { // TOOLCALL Return
    pub entity_name: String,
    pub entity_tag: String,
}


impl GraphEntities {
    pub fn new(relationships: Vec<Relationship>, entity_tags: Vec<EntityTag>, filter: Mem0Filter) -> Self {
        let entity_tags = entity_tags
            .into_iter()
            .map(|tag| (tag.entity_name, tag.entity_tag))
            .collect();

        
        Self {
            relationships,
            entity_tags,
            filter,
        }
    }
}

define_module_client! {
    (struct GraphClient, "graph")
    client_type: Graph,
    env: ["GRAPH_URI", "GRAPH_USER", "GRAPH_PASSWORD"],
    setup: async {
        let graph_uri = env::var("GRAPH_URI").expect("GRAPH_URI is not set");
        let graph_user = env::var("GRAPH_USER").expect("GRAPH_USER is not set");
        let graph_password = env::var("GRAPH_PASSWORD").expect("GRAPH_PASSWORD is not set");
        let graph_config  = ConfigBuilder::default()
            .uri(graph_uri)
            .user(graph_user)
            .password(graph_password)
            .db("neo4j")
            .build()
            .expect("[GraphClient::setup] Failed to build graph config");

        let graph = Graph::connect(graph_config).await
            .expect("[GraphClient::setup] Failed to connect to graph");

        graph
    }
}

impl GraphClient {
    pub async fn initialize(&self) -> Result<()> {
        let mut tx = self.get_client().start_txn().await?;

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

    pub async fn search_entity_with_similarity(&self,
        embedding: &Embedding, filter: &Mem0Filter,
    ) -> Result<Option<String>> {
        let character_id_filter = if let Some(character_id) = filter.character_id {
            format!("AND candidate.character_id = '{}'", character_id)
        } else {
            "".to_string()
        };
        let session_id_filter = if let Some(session_id) = filter.session_id {
            format!("AND candidate.session_id = '{}'", session_id)
        } else {
            "".to_string()
        };

        let q = format!(r#"
            CALL db.index.vector.queryNodes("memzero", 1, $source_embedding)
            YIELD node AS candidate, score AS similarity
            WHERE candidate.user_id = $user_id
            {character_id_filter}
            {session_id_filter}
            AND similarity >= {DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD}
            RETURN id(candidate);
        "#);

        let q = query(&q)
            .param("source_embedding", embedding.clone())
            .param("user_id", filter.user_id.to_string());

        let mut result = self.get_client().execute(q).await?;
        let mut results = Vec::new();
        while let Some(row) = result.next().await? {
            results.push(row);
        }

        let maybe_id = results
            .first()
            .and_then(|row| row.get("id(candidate)").unwrap_or(None));

        Ok(maybe_id)
    }

    pub async fn add(&self, message: &GraphEntities, embeder: &EmbederClient) -> Result<usize> {
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

        let embeddings = embeder.embed(entity_names.clone()).await?;
        let name_to_embedding: HashMap<String, Vec<f32>> = entity_names.iter().cloned().zip(embeddings).collect();

        let mut name_to_id = HashMap::new();
        for name in &entity_names {
            let embedding = name_to_embedding.get(name).unwrap();
            let id = self.search_entity_with_similarity(embedding, &message.filter).await?;
                if let Some(id_val) = id {
                name_to_id.insert(name.clone(), id_val);
            }
        }

        let mut tx = self.get_client().start_txn().await?;
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
                        MERGE (source)-[r:`{}`]->(destination) \
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
                        MERGE (source)-[r:`{}`]->(destination) \
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
                        MERGE (source)-[r:`{}`]->(destination) \
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
                        MERGE (source)-[r:`{}`]->(destination) \
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

    pub async fn search(&self,
        nodes_embeddings: Vec<Embedding>,
        filter: &Mem0Filter,
    ) -> Result<Vec<Relationship>> {
        let (character_id_filter_n, character_id_filter_m) = if let Some(character_id) = filter.character_id {
            (
                format!("AND n.character_id = '{}'", character_id),
                format!("AND m.character_id = '{}'", character_id)
            )
        } else {
            ("".to_string(), "".to_string())
        };
        let (session_id_filter_n, session_id_filter_m) = if let Some(session_id) = filter.session_id {
            (
                format!("AND n.session_id = '{}'", session_id),
                format!("AND m.session_id = '{}'", session_id)
            )
        } else {
            ("".to_string(), "".to_string())
        };

        let mut all_relations = Vec::new();

        for embedding in nodes_embeddings {
            let query_str = format!(r#"
                MATCH (n:Entity)
                WHERE n.embedding IS NOT NULL AND n.user_id = $user_id {character_id_filter_n} {session_id_filter_n}
                WITH n, round(2 * vector.similarity.cosine(n.embedding, $embedding) - 1, 4) AS similarity
                WHERE similarity >= {DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD}
                CALL {{
                    WITH n
                    MATCH (n)-[r]->(m:Entity)
                    WHERE m.user_id = $user_id {character_id_filter_m} {session_id_filter_m}
                    RETURN n.name AS source, elementId(n) AS source_id, type(r) AS relationship, elementId(r) AS relation_id, m.name AS destination, elementId(m) AS destination_id
                    UNION
                    WITH n
                    MATCH (m:Entity)-[r]->(n)
                    WHERE m.user_id = $user_id {character_id_filter_m} {session_id_filter_m}
                    RETURN m.name AS source, elementId(m) AS source_id, type(r) AS relationship, elementId(r) AS relation_id, n.name AS destination, elementId(n) AS destination_id
                }}
                WITH distinct source, source_id, relationship, relation_id, destination, destination_id, similarity
                RETURN
                    source,
                    relationship,
                    destination,
                    similarity
                ORDER BY similarity DESC
                LIMIT {DEFAULT_GRAPH_DB_SEARCH_LIMIT}
            "#,
            character_id_filter_n = character_id_filter_n,
            character_id_filter_m = character_id_filter_m,
            session_id_filter_n = session_id_filter_n,
            session_id_filter_m = session_id_filter_m
            );

            let q = query(&query_str)
                .param("embedding", embedding)
                .param("user_id", filter.user_id.to_string());

            let mut result = self.get_client().execute(q).await?;
            while let Some(row) = result.next().await? {
                let relation_info = Relationship {
                    source: row.get("source").unwrap_or_default(),
                    relationship: row.get("relationship").unwrap_or_default(),
                    destination: row.get("destination").unwrap_or_default(),
                };
                all_relations.push(relation_info);
            }
        }

        Ok(all_relations)
    }

    pub async fn delete(&self, message: &GraphEntities) -> Result<usize> {
        let mut count = 0;
        let mut tx = self.get_client().start_txn().await?;
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
                -[r:`{}`]->
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