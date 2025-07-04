use anyhow::Result;
use async_openai::types::{FunctionCall, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

use voda_runtime::ExecutableFunctionCall;

use crate::llm::LlmConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleRelationshipToolcall {
    source_entity: String,
    relatationship: String,
    destination_entity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipsToolcall {
    relationships: Vec<SingleRelationshipToolcall>,
}

pub fn get_extract_relationship_config(user_id: String, entity_type_map: std::collections::HashMap<String, String>, data: String) -> (LlmConfig, String) {
    let system_prompt = format!(
        r#"You are an advanced algorithm designed to extract structured information from text to construct knowledge graphs. Your goal is to capture comprehensive and accurate information. Follow these key principles:

1. Extract only explicitly stated information from the text.
2. Establish relationships among the entities provided.
3. Use "{}" as the source entity for any self-references (e.g., "I," "me," "my," etc.) in user messages.

Relationships:
    - Use consistent, general, and timeless relationship types.
    - Example: Prefer "professor" over "became_professor."
    - Relationships should only be established among the entities explicitly mentioned in the user message.

Entity Consistency:
    - Ensure that relationships are coherent and logically align with the context of the message.
    - Maintain consistent naming for entities across the extracted data.

Strive to construct a coherent and easily understandable knowledge graph by eshtablishing all the relationships among the entities and adherence to the user’s context.

Adhere strictly to these guidelines to ensure high-quality knowledge graph extraction."#,
        user_id
    );

    let user_prompt = format!("List of entities: {:?}. \n\nText: {}", entity_type_map.keys().collect::<Vec<_>>(), data);

    let establish_relations_tool = FunctionObject {
        name: "establish_relations".to_string(),
        description: Some("Establish relationships among the entities based on the provided text.".to_string()),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "relationships": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "source_entity": {"type": "string", "description": "The source entity of the relationship."},
                            "relatationship": {"type": "string", "description": "The relationship between the source and destination entities."},
                            "destination_entity": {"type": "string", "description": "The destination entity of the relationship."},
                        },
                        "required": ["source_entity", "relatationship", "destination_entity"],
                        "additionalProperties": false,
                    },
                    "description": "An array of relationships.",
                }
            },
            "required": ["relationships"],
            "additionalProperties": false,
        })),
        strict: Some(true),
    };

    let tools = vec![establish_relations_tool];

    let config = LlmConfig {
        model: "mistralai/ministral-8b".to_string(),
        temperature: 0.7,
        max_tokens: 5000,
        system_prompt, tools,
    };

    (config, user_prompt)
}

impl ExecutableFunctionCall for RelationshipsToolcall {
    fn name() -> &'static str {
        "establish_relations"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        let relationships: Vec<SingleRelationshipToolcall> = serde_json::from_str(&function_call.arguments)?;
        Ok(Self { relationships })
    }

    async fn execute(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.relationships)?)
    }
}