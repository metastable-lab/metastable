use anyhow::Result;
use async_openai::types::{FunctionCall, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

use voda_runtime::ExecutableFunctionCall;

use crate::{llm::LlmConfig, raw_message::Relationship, EntityTag};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipsToolcall {
    pub relationships: Vec<Relationship>,
}

pub fn get_extract_relationship_config(user_id: String, entity_type_map: &[EntityTag], new_information: String) -> (LlmConfig, String) {
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

    let entities_names_text = entity_type_map.iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>().join(", ");
    let user_prompt = format!("List of entities: [{}]. \n\nText: {}", entities_names_text, new_information);

    let establish_relationships_tool = FunctionObject {
        name: "establish_relationships".to_string(),
        description: Some("Establish relationships among the entities based on the provided text.".to_string()),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "relationships": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "source": {"type": "string", "description": "The source entity of the relationship."},
                            "relationship": {"type": "string", "description": "The relationship between the source and destination entities."},
                            "destination": {"type": "string", "description": "The destination entity of the relationship."},
                        },
                        "required": ["source", "relationship", "destination"],
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

    let tools = vec![establish_relationships_tool];

    let config = LlmConfig {
        model: "inception/mercury".to_string(),
        temperature: 0.7,
        max_tokens: 10000,
        system_prompt, tools,
    };

    (config, user_prompt)
}

impl ExecutableFunctionCall for RelationshipsToolcall {
    fn name() -> &'static str {
        "establish_relationships"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        println!("function_call: {:?}", function_call);
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        Ok(serde_json::to_string(&self)?)
    }
}