use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_common::get_current_timestamp;
use metastable_runtime::{Agent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig};
use metastable_clients::{EntityTag, GraphEntities, LlmClient, Mem0Filter, PostgresClient, Relationship};
use serde_json::Value;

use crate::{init_mem0, Mem0Engine};

init_mem0!();

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
pub struct DeleteRelationshipsOutput {
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRelationshipsInput {
    pub filter: Mem0Filter,
    pub user_aka: String,

    pub type_mapping: Vec<EntityTag>,
    pub new_message: String,
}

#[derive(Clone)]
pub struct DeleteRelationshipsAgent {
    mem0_engine: Arc<Mem0Engine>,
    system_config: SystemConfig,
}

impl DeleteRelationshipsAgent {
    pub async fn new() -> Result<Self> {
        let mem0_engine = get_mem0_engine().await;
        let system_config = Self::preload(&mem0_engine.data_db).await?;

        Ok(Self { 
            mem0_engine: Arc::new(mem0_engine.clone()), 
            system_config 
        })
    }
}

#[async_trait::async_trait]
impl Agent for DeleteRelationshipsAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "delete_relationships_v0";
    type Tool = DeleteRelationshipsOutput;
    type Input = DeleteRelationshipsInput;

    fn llm_client(&self) -> &LlmClient { &self.mem0_engine.llm }
    fn db_client(&self) -> &PostgresClient { &self.mem0_engine.data_db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let system_prompt = Self::system_prompt()
            .replace("{{user}}", input.user_aka.clone());

        let type_mapping_keys = input.type_mapping.iter().map(|e| e.entity_name.clone()).collect::<Vec<_>>();
        let existing_memories = self.mem0_engine.graph_db
            .search(&type_mapping_keys, &input.filter).await?;
        let existing_memories_text = existing_memories.iter()
            .map(|r| format!("{} -- {} -- {}", r.source, r.relationship, r.destination))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(vec![
            Prompt::new_system(&system_prompt),
            Prompt {
                role: MessageRole::User,
                content_type: MessageType::Text,
                content: format!("Existing Memories: {}\nNew Information: {}", existing_memories_text, input.new_message.clone()),
                toolcall: None,
                created_at: get_current_timestamp(),
            }
        ])
    }

    async fn handle_output(&self, input: &Self::Input, _message: &Message, tool: &Self::Tool) -> Result<Option<Value>> {
        let relationships = tool.relationships.clone();
        if relationships.is_empty() {
            return Ok(Some(serde_json::to_value(0)?));
        }

        let graph_entities = GraphEntities {
            relationships: relationships,
            entity_tags: input.type_mapping.iter().map(|e| (e.entity_name.clone(), e.entity_tag.clone())).collect(),
            filter: input.filter.clone(),
        };

        let delete_size = self.mem0_engine.graph_db.delete(&graph_entities).await?;
        Ok(Some(serde_json::to_value(delete_size)?))
    }

    fn system_prompt() ->  &'static str {
        r#"You are a graph memory manager specializing in identifying, managing, and optimizing relationships within graph-based memories. Your primary task is to analyze a list of existing relationships and determine which ones should be deleted based on the new information provided.
Input:
1. Existing Graph Memories: A list of current graph memories, each containing source, relationship, and destination information.
2. New Text: The new information to be integrated into the existing graph structure.
3. Use "{{user}}" as node for any self-references (e.g., "I," "me," "my," etc.) in user messages.

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
- Do NOT include any other text, reasoning, or explanations in your response."#
    }
}