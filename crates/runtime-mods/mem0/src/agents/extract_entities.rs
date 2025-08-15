use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_common::get_current_timestamp;
use metastable_runtime::{Agent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig};
use metastable_clients::{EntityTag, LlmClient, Mem0Filter, PostgresClient};
use serde_json::Value;

use crate::{init_mem0, Mem0Engine};

init_mem0!();

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
pub struct ExtractEntitiesOutput {
    pub entities: Vec<EntityTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractEntitiesInput {
    pub filter: Mem0Filter,
    pub user_aka: String,
    pub new_message: String,
}

#[derive(Clone)]
pub struct ExtractEntitiesAgent {
    mem0_engine: Arc<Mem0Engine>,
    system_config: SystemConfig,
}

impl ExtractEntitiesAgent {
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
impl Agent for ExtractEntitiesAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "extract_entities_v0";
    type Tool = ExtractEntitiesOutput;
    type Input = ExtractEntitiesInput;

    fn llm_client(&self) -> &LlmClient { &self.mem0_engine.llm }
    fn db_client(&self) -> &PostgresClient { &self.mem0_engine.data_db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let system_prompt = Self::system_prompt()
            .replace("{{user}}", input.user_aka.clone());

        Ok(vec![
            Prompt::new_system(system_prompt),
            Prompt {
                role: MessageRole::User,
                content_type: MessageType::Text,
                content: input.new_message.clone(),
                toolcall: None,
                created_at: get_current_timestamp(),
            }
        ])
    }

    async fn handle_output(&self, input: &Self::Input, message: &Message, tool: &Self::Tool) -> Result<Option<Value>> {
        Ok(None)
    }

    fn system_prompt() ->  &'static str {
        r#"You are a smart assistant who understands entities and their types in a given text. If user message contains self reference such as 'I', 'me', 'my' etc. then use {{user}} as the source entity. Extract all the entities from the text. ***DO NOT*** answer the question itself if the given text is a question."#
    }
}