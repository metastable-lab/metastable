mod del_relationship;

mod extract_entity;
mod extract_facts;
mod extract_relationship;
mod update_memory;

use async_openai::types::FunctionObject;

pub struct LlmConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: i32,
    pub system_prompt: String,
    pub tools: Vec<FunctionObject>
}

pub use crate::llm::del_relationship::{DeleteGraphMemoryToolcall, get_delete_graph_memory_config};

pub use crate::llm::extract_entity::{EntitiesToolcall, get_extract_entity_config};
pub use crate::llm::extract_facts::{FactsToolcall, get_extract_facts_config};
pub use crate::llm::extract_relationship::{RelationshipsToolcall, get_extract_relationship_config};

pub use crate::llm::update_memory::{MemoryUpdateToolcall, InputMemory, get_update_memory_config};