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

    #[tokio::test]
    async fn test_graph_update_flow() {
        let user_id = "123".to_string();
        let existing_memories = "xiaoming -- works_at -- beijing".to_string();
        let old_text = "xiaoming works_at beijing".to_string();
        let new_text = "xiaoming works_at shanghai".to_string();

        let db = GraphDatabase::new().await;

        let entity_config = get_extract_entity_config(user_id.clone());
        let entity_result = db.llm(&entity_config, &old_text).await.unwrap();
        println!("extracted entities: {}", entity_result);

        let rel_config = get_extract_relationship_config(user_id.clone());
        let rel_result = db.llm(&rel_config, &old_text).await.unwrap();
        println!("extracted relationships: {}", rel_result);

        let (delete_config, delete_prompt) = get_delete_graph_memory_config(
            user_id.clone(),
            existing_memories,
            new_text.to_string()
        );

        let delete_result = db.llm(&delete_config, &delete_prompt).await.unwrap();
        println!("need to delete: {}", delete_result);

        let new_entity_result = db.llm(&entity_config, &new_text).await.unwrap();
        println!("new extracted entities: {}", new_entity_result);

        let final_result = db.llm(&rel_config, &new_text).await.unwrap();
        println!("final result: {}", final_result);

    }
}