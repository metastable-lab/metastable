use std::collections::HashMap;
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Debug, Clone)]
pub struct EmbeddingMessage {
    pub id: Uuid,

    pub user_id: Uuid,
    pub agent_id: Option<Uuid>,

    pub embedding: Vector,
    pub content: String,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct GraphEntities {
    pub relationships: Vec<Relationship>,
    pub entity_tags: HashMap<String, String>,

    pub user_id: Uuid,
    pub agent_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship { // TOOLCALL Return
    pub source: String,
    pub relationship: String,
    pub destination: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityTag { // TOOLCALL Return
    pub entity_name: String,
    pub entity_tag: String,
}

impl GraphEntities {
    pub fn new(relationships: Vec<Relationship>, entity_tags: Vec<EntityTag>, user_id: Uuid, agent_id: Option<Uuid>) -> Self {
        let entity_tags = entity_tags
            .into_iter()
            .map(|tag| (tag.entity_name, tag.entity_tag))
            .collect();

        
        Self {
            relationships,
            entity_tags,
            user_id,
            agent_id,
        }
    }
}
