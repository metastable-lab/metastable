use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mem0Filter {
    pub user_id: Uuid,
    pub character_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct GraphEntities {
    pub relationships: Vec<Relationship>,
    pub entity_tags: HashMap<String, String>,
    pub filter: Mem0Filter,
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
