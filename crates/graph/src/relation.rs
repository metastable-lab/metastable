use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String, // TODO: 
    
    pub embedding: Vec<f32>,
    pub mentions: i64,
    pub name: String,

    pub user_id: String,
    
    pub created: i64,
    // pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,

    pub name: String,
    pub mentions: i64,   
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship { // TOOLCALL Return
    pub source: String,
    pub relationship: String,
    pub destination: String,

    pub user_id: String, // uuid
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityTag { // TOOLCALL Return
    pub entity_name: String,
    pub entity_tag: String,
}

impl EntityTag {
    pub fn batch_into_hashmap(entity_tags: &[Self]) -> HashMap<String, String> {
        entity_tags.iter().map(|tag| (tag.entity_name.clone(), tag.entity_tag.clone())).collect()
    }
}