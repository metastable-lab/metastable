use anyhow::Result;
use async_openai::types::{FunctionCall, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

use voda_runtime::ExecutableFunctionCall;

use crate::{graph::RelationInfo, llm::LlmConfig, raw_message::Relationship};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteGraphMemoryToolcall {
    pub relationships: Vec<Relationship>,
}

const DELETE_RELATIONS_SYSTEM_PROMPT: &str = r#"You are a graph memory manager specializing in identifying, managing, and optimizing relationships within graph-based memories. Your primary task is to analyze a list of existing relationships and determine which ones should be deleted based on the new information provided.
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

Provide a list of deletion instructions, each specifying the relationship to be deleted."#;


pub fn get_delete_graph_memory_config(user_id: String, existing_memories: Vec<RelationInfo>, new_text: String) -> (LlmConfig, String) {
    let system_prompt = DELETE_RELATIONS_SYSTEM_PROMPT.replace("{user_id}", &user_id);

    let existing_memories_text = existing_memories.iter()
        .map(|r| format!("{} -- {} -- {}", r.source, r.relationship, r.destination))
        .collect::<Vec<_>>()
        .join("\n");
    let user_prompt = format!("Here are the existing memories: {} \n\n New Information: {}", existing_memories_text, new_text);

    let delete_graph_memory_tool = FunctionObject {
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
    };

    let tools = vec![delete_graph_memory_tool];

    let config = LlmConfig {
        model: "inception/mercury".to_string(),
        temperature: 0.7,
        max_tokens: 10000,
        system_prompt, tools,
    };

    (config, user_prompt)
}

impl ExecutableFunctionCall for DeleteGraphMemoryToolcall {
    fn name() -> &'static str {
        "delete_graph_memory"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        Ok(serde_json::to_string(&self)?)
    }
}