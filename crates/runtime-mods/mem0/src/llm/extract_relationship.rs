use anyhow::Result;
use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};
use serde_json::json;

use voda_runtime::{ExecutableFunctionCall, LLMRunResponse};

use crate::llm::{LlmTool, ToolInput};
use crate::{raw_message::Relationship, EntityTag, GraphEntities, Mem0Engine, Mem0Filter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRelationshipToolInput {
    pub filter: Mem0Filter,
    pub entities: Vec<EntityTag>,
    pub new_information: String,
}

impl ToolInput for ExtractRelationshipToolInput {
    fn filter(&self) -> &Mem0Filter { &self.filter }

    fn build(&self) -> String {
        let entities_names_text = self.entities.iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>().join(", ");
        format!("List of entities: {}\nNew information: {}", entities_names_text, self.new_information)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipsToolcall {
    pub relationships: Vec<Relationship>,
    pub input: Option<ExtractRelationshipToolInput>,
}

#[async_trait::async_trait]
impl LlmTool for RelationshipsToolcall {
    type ToolInput = ExtractRelationshipToolInput;

    fn tool_input(&self) -> Option<Self::ToolInput> { self.input.clone() }
    fn set_tool_input(&mut self, tool_input: Self::ToolInput) { self.input = Some(tool_input); }

    fn system_prompt(input: &Self::ToolInput) -> String {
        format!(
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
        input.filter().user_id.to_string())
    }

    fn tools() -> Vec<FunctionObject> {
        vec![FunctionObject {
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
        }]
    }
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for RelationshipsToolcall {
    type CTX = Mem0Engine;
    type RETURN = usize;

    fn name() -> &'static str { "extract_relationships" }

    async fn execute(&self, llm_response: &LLMRunResponse, execution_context: &Self::CTX) -> Result<Self::RETURN> {
        execution_context.add_usage_report(llm_response).await?;

        let input = self.tool_input()
            .ok_or(anyhow::anyhow!("[RelationshipsToolcall::execute] No input found"))?;

        if self.relationships.is_empty() { return Ok(0); }

        let add_entities = GraphEntities::new(
            self.relationships.clone(),
            input.entities.clone(),
            input.filter.clone(),
        );

        let add_size = execution_context.graph_db_add(&add_entities).await?;
        Ok(add_size)
    }
}
