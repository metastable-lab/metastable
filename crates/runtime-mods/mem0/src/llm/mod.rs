mod del_relationship;

mod extract_entity;
mod extract_facts;
mod extract_relationship;
mod update_memory;

use anyhow::Result;
use async_openai::types::FunctionObject;

pub struct LlmConfig {
    pub name: String,
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
use crate::Mem0Engine;

impl LlmConfig {
    pub async fn call<T>(&self, engine: &Mem0Engine, user_message: String) -> Result<T>
        where T: serde::de::DeserializeOwned + std::fmt::Debug
    {
        tracing::debug!("[LlmConfig::call] Calling LLM: {}", self.name);
        let response = engine.llm(&self, user_message).await?;
        let tool_calls = response.maybe_results.first()
            .ok_or(anyhow::anyhow!("[LlmConfig::call] No tool calls found"))?;
        tracing::debug!("[LlmConfig::call] Tool calls: {:?}", tool_calls);
        let parsed_result = serde_json::from_str::<T>(&tool_calls.to_string())
            .map_err(|e| anyhow::anyhow!("[LlmConfig::call] Failed to parse result: {}", e))?;
        tracing::debug!("[LlmConfig::call] Parsed result: {:?}", parsed_result);
        Ok(parsed_result)
    }
}