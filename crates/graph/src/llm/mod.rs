mod extract_entity;
mod extract_relationship;
mod del_relationship;
use async_openai::types::FunctionObject;

pub struct LlmConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: i32,
    pub system_prompt: String,
    pub tools: Vec<FunctionObject>
}

pub use crate::llm::extract_entity::{EntitiesToolcall, SingleEntityToolcall, get_extract_entity_config};
pub use crate::llm::extract_relationship::{RelationshipsToolcall, SingleRelationshipToolcall, get_extract_relationship_config};
pub use crate::llm::del_relationship::{DeleteGraphMemoryToolcall, SingleDelRelationshipToolcall, get_delete_graph_memory_config};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphDatabase;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_graph_update_flow() {
        let user_id = "123".to_string();
        let existing_memories = "xiaoming -- works_at -- beijing".to_string();
        let old_text = "xiaoming works_at beijing".to_string();
        let new_text = "xiaoming works_at shanghai".to_string();

        let db = GraphDatabase::new().await;

        let (entity_config, entity_prompt) = get_extract_entity_config(user_id.clone(), old_text.clone());
        let entity_result = db.llm(&entity_config, &entity_prompt).await.unwrap();
        println!("extracted entities: {}", entity_result);

        let entity_type_map: HashMap<String, String> = HashMap::new();

        let (rel_config, rel_prompt) = get_extract_relationship_config(user_id.clone(), entity_type_map.clone(), old_text.clone());
        let rel_result = db.llm(&rel_config, &rel_prompt).await.unwrap();
        println!("extracted relationships: {}", rel_result);

        let (delete_config, delete_prompt) = get_delete_graph_memory_config(
            user_id.clone(),
            existing_memories,
            new_text.to_string()
        );

        let delete_result = db.llm(&delete_config, &delete_prompt).await.unwrap();
        println!("need to delete: {}", delete_result);

        let (new_entity_config, new_entity_prompt) = get_extract_entity_config(user_id.clone(), new_text.clone());
        let new_entity_result = db.llm(&new_entity_config, &new_entity_prompt).await.unwrap();
        println!("new extracted entities: {}", new_entity_result);

        let new_entity_type_map: HashMap<String, String> = HashMap::new();

        let (final_rel_config, final_rel_prompt) = get_extract_relationship_config(user_id.clone(), new_entity_type_map.clone(), new_text.clone());
        let final_result = db.llm(&final_rel_config, &final_rel_prompt).await.unwrap();
        println!("final result: {}", final_result);
    }
}