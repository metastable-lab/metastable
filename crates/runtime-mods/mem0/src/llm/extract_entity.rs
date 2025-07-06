use anyhow::Result;
use async_openai::types::{FunctionCall, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;
use voda_runtime::ExecutableFunctionCall;

use crate::{llm::LlmConfig, EntityTag};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitiesToolcall {
    pub entities: Vec<EntityTag>,
}

pub fn get_extract_entity_config(user_id: String, content: String) -> (LlmConfig, String) {
    let system_prompt = format!(
        "You are a smart assistant who understands entities and their types in a given text. If user message contains self reference such as 'I', 'me', 'my' etc. then use {} as the source entity. Extract all the entities from the text. ***DO NOT*** answer the question itself if the given text is a question.",
        user_id
    );

    let extract_entity_tool = FunctionObject {
        name: "extract_entities".to_string(),
        description: Some("Extract entities and their types from the text.".to_string()),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entity_name": {"type": "string", "description": "The name or identifier of the entity."},
                            "entity_tag": {"type": "string", "description": "The type or category of the entity."},
                        },
                        "required": ["entity_name", "entity_tag"],
                        "additionalProperties": false,
                    },
                    "description": "An array of entities with their types.",
                }
            },
            "required": ["entities"],
            "additionalProperties": false,
        })),
        strict: Some(true),
    };

    let tools = vec![extract_entity_tool];

    let config = LlmConfig {
        model: "mistralai/ministral-8b".to_string(),
        temperature: 0.7,
        max_tokens: 5000,
        system_prompt, tools,
    };

    (config, content)
}

impl ExecutableFunctionCall for EntitiesToolcall {
    fn name() -> &'static str {
        "extract_entities"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        Ok(serde_json::to_string(&self)?)
    }
}
