use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_common::get_current_timestamp;
use metastable_runtime::{Agent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig};
use metastable_clients::{LlmClient, PostgresClient};
use serde_json::Value;

use crate::{init_mem0, Mem0Engine, graph::{EntityTag, Relationship, GraphEntities}, Mem0Filter};

init_mem0!();

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
pub struct ExtractRelationshipsOutput {
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRelationshipsInput {
    pub filter: Mem0Filter,
    pub entities: Vec<EntityTag>,
    pub user_aka: String,
    pub new_message: String,
}

#[derive(Clone)]
pub struct ExtractRelationshipsAgent {
    mem0_engine: Arc<Mem0Engine>,
    system_config: SystemConfig,
}

impl ExtractRelationshipsAgent {
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
impl Agent for ExtractRelationshipsAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "extract_relationships_v0";
    type Tool = ExtractRelationshipsOutput;
    type Input = ExtractRelationshipsInput;

    fn llm_client(&self) -> &LlmClient { &self.mem0_engine.llm }
    fn db_client(&self) -> &PostgresClient { &self.mem0_engine.data_db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let system_prompt = Self::system_prompt()
            .replace("{{user}}", &input.user_aka);

        let user_message = format!(
            "List of entities: {}\nNew information: {}", 
            input.entities.iter().map(|e| e.entity_name.clone()).collect::<Vec<_>>().join(", "), 
            input.new_message.clone()
        );

        Ok(vec![
            Prompt::new_system(&system_prompt),
            Prompt {
                role: MessageRole::User,
                content_type: MessageType::Text,
                content: user_message,
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
            relationships,
            entity_tags: input.entities.iter().map(|e| (e.entity_name.clone(), e.entity_tag.clone())).collect(),
            filter: input.filter.clone(),
        };

        let add_size = self.mem0_engine.graph_db.add(&graph_entities, &self.mem0_engine.embeder).await?;
        Ok(Some(serde_json::to_value(add_size)?))
    }

    fn system_prompt() ->  &'static str {
        r#"You are an advanced algorithm designed to extract structured information from text to construct knowledge graphs. Your goal is to capture comprehensive and accurate information. Follow these key principles:

1. Extract only explicitly stated information from the text.
2. Establish relationships among the entities provided.
3. Use "{{user}}" as the source entity for any self-references (e.g., "I," "me," "my," etc.) in user messages.

Relationships:
    - Use consistent, general, and timeless relationship types.
    - Example: Prefer "professor" over "became_professor."
    - Relationships should only be established among the entities explicitly mentioned in the user message.

Entity Consistency:
    - Ensure that relationships are coherent and logically align with the context of the message.
    - Maintain consistent naming for entities across the extracted data.

Strive to construct a coherent and easily understandable knowledge graph by eshtablishing all the relationships among the entities and adherence to the user’s context.

Adhere strictly to these guidelines to ensure high-quality knowledge graph extraction."#
    }
}