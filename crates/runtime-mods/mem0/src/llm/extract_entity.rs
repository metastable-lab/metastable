use anyhow::Result;
use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};
use serde_json::json;
use voda_runtime::{ExecutableFunctionCall, LLMRunResponse};

use crate::llm::{LlmTool, ToolInput};
use crate::{EntityTag, Mem0Engine, Mem0Filter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractEntityToolInput {
    pub filter: Mem0Filter,
    pub new_message: String,
}

impl ToolInput for ExtractEntityToolInput {
    fn filter(&self) -> &Mem0Filter { &self.filter }

    fn build(&self) -> String {
        self.new_message.clone()
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitiesToolcall {
    pub entities: Vec<EntityTag>,
    pub input: Option<ExtractEntityToolInput>,
}

#[async_trait::async_trait]
impl LlmTool for EntitiesToolcall {
    type ToolInput = ExtractEntityToolInput;

    fn tool_input(&self) -> Option<Self::ToolInput> {
        self.input.clone()
    }

    fn set_tool_input(&mut self, tool_input: Self::ToolInput) {
        self.input = Some(tool_input);
    }

    fn system_prompt(input: &Self::ToolInput) -> String {
        format!(
            "You are a smart assistant who understands entities and their types in a given text. If user message contains self reference such as 'I', 'me', 'my' etc. then use {} as the source entity. Extract all the entities from the text. ***DO NOT*** answer the question itself if the given text is a question.",
            input.filter().user_id.to_string()
        )
    }

    fn tools() -> Vec<FunctionObject> {
        vec![FunctionObject {
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
        }]
    }
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for EntitiesToolcall {
    type CTX = Mem0Engine;
    type RETURN = Vec<EntityTag>;

    fn name() -> &'static str { "extract_entities" }

    async fn execute(&self, llm_response: &LLMRunResponse, execution_context: &Self::CTX) -> Result<Self::RETURN> {
        execution_context.add_usage_report(llm_response).await?;

        Ok(self.entities.clone())
    }
}
