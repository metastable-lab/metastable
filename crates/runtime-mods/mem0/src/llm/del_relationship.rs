use anyhow::Result;
use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};
use serde_json::json;

use sqlx::types::Uuid;
use voda_runtime::{ExecutableFunctionCall, LLMRunResponse};

use crate::{ 
    llm::{LlmTool, ToolInput}, 
    raw_message::Relationship, 
    EntityTag, GraphEntities, Mem0Engine
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGraphMemoryToolInput {
    pub user_id: Uuid, pub agent_id: Option<Uuid>,

    pub type_mapping: Vec<EntityTag>,
    pub existing_memories: Vec<Relationship>,
    pub new_message: String,
}

impl ToolInput for DeleteGraphMemoryToolInput {
    fn user_id(&self) -> Uuid { self.user_id.clone() }
    fn agent_id(&self) -> Option<Uuid> { self.agent_id.clone() }

    fn build(&self) -> String {
        let existing_memories_text = self.existing_memories.iter()
            .map(|r| format!("{} -- {} -- {}", r.source, r.relationship, r.destination))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Here are the existing memories: {} \n\n New Information: {}", existing_memories_text, self.new_message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGraphMemoryToolcall {
    pub relationships: Vec<Relationship>,
    pub input: Option<DeleteGraphMemoryToolInput>,
}

#[async_trait::async_trait]
impl LlmTool for DeleteGraphMemoryToolcall {
    type ToolInput = DeleteGraphMemoryToolInput;

    fn tool_input(&self) -> Option<Self::ToolInput> {
        self.input.clone()
    }

    fn set_tool_input(&mut self, tool_input: Self::ToolInput) {
        self.input = Some(tool_input);
    }

    fn system_prompt(_input: &Self::ToolInput) -> String {
        r#"You are a graph memory manager specializing in identifying, managing, and optimizing relationships within graph-based memories. Your primary task is to analyze a list of existing relationships and determine which ones should be deleted based on the new information provided.
Input:
1. Existing Graph Memories: A list of current graph memories, each containing source, relationship, and destination information.
2. New Text: The new information to be integrated into the existing graph structure.
3. Use "{user_id}" as node for any self-references (e.g., "I," "me," "my," etc.) in user messages.

Guidelines:
1. Identification: Use the new information to evaluate existing relationships in the memory graph.
2. Deletion Criteria: Delete a relationship only if it meets at least one of these conditions:
   - Outdated or Inaccurate: The new information is more recent or accurate.
   - Contradictory: The new information conflicts with or negates the existing information.
3. DO NOT DELETE if their is a possibility of same type of relationship but different destination nodes.
4. Comprehensive Analysis:
   - Thoroughly examine each existing relationship against the new information and delete as necessary.
   - Multiple deletions may be required based on the new information.
5. Semantic Integrity:
   - Ensure that deletions maintain or improve the overall semantic structure of the graph.
   - Avoid deleting relationships that are NOT contradictory/outdated to the new information.
6. Temporal Awareness: Prioritize recency when timestamps are available.
7. Necessity Principle: Only DELETE relationships that must be deleted and are contradictory/outdated to the new information to maintain an accurate and coherent memory graph.

Note: DO NOT DELETE if their is a possibility of same type of relationship but different destination nodes. 

For example: 
Existing Memory: alice -- loves_to_eat -- pizza
New Information: Alice also loves to eat burger.

Do not delete in the above example because there is a possibility that Alice loves to eat both pizza and burger.

Memory Format:
source -- relationship -- destination

Response Format:
- Your response MUST be a call to the `delete_graph_memory` tool.
- The `relationships` argument must contain a list of all relationships to be deleted.
- If no relationships should be deleted, provide an empty list for the `relationships` argument.
- Do NOT include any other text, reasoning, or explanations in your response."#.to_string()
    }

    fn tools() -> Vec<FunctionObject> {
        vec![FunctionObject {
            name: "delete_graph_memory".to_string(),
            description: Some("Delete relationships among the entities based on the provided text.".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "relationships": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "source": {"type": "string", "description": "The identifier of the source node in the relationship."},
                                "relationship": {"type": "string", "description": "The existing relationship between the source and destination nodes that needs to be deleted."},
                                "destination": {"type": "string", "description": "The identifier of the destination node in the relationship."},
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
impl ExecutableFunctionCall for DeleteGraphMemoryToolcall {
    type CTX = Mem0Engine;
    type RETURN = usize;

    fn name() -> &'static str { "delete_graph_memory" }

    async fn execute(&self, llm_response: &LLMRunResponse, execution_context: &Self::CTX) -> Result<Self::RETURN> {
        execution_context.add_usage_report(llm_response).await?;

        tracing::debug!("[DeleteGraphMemoryToolcall::execute] Executing tool call: {:?}", self);
        let input = self.tool_input()
            .ok_or(anyhow::anyhow!("[DeleteGraphMemoryToolcall::execute] No input found"))?;

        if self.relationships.is_empty() {
            return Ok(0);
        }

        let delete_entities = GraphEntities::new(
            self.relationships.clone(),
            input.type_mapping.clone(),
            input.user_id,
            input.agent_id,
        );

        tracing::debug!("[Mem0Engine::add_messages] Deleting relationships from graph DB");
        let delete_size = execution_context.graph_db_delete(&delete_entities).await?;
        tracing::info!("[Mem0Engine::add_messages] Deleted {} relationships", delete_size);
        Ok(delete_size)
    }
}