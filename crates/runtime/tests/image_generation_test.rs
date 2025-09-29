use anyhow::Result;
use metastable_clients::{LlmClient, PostgresClient, R2Client};
use metastable_common::ModuleClient;
use metastable_runtime::{
    Agent, ImageAgent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Uuid;

// Test tool for image generation
#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
#[llm_tool(
    name = "generate_image",
    description = "Generate an image based on the prompt"
)]
pub struct TestImageTool {
    pub image_urls: Vec<String>,
    pub description: String,
}

#[derive(Clone)]
struct TestImageAgent {
    llm_client: LlmClient,
    r2_client: R2Client,
    db_client: PostgresClient,
    system_config: SystemConfig,
}

impl TestImageAgent {
    async fn new() -> Result<Self> {
        let llm_client = LlmClient::setup_connection().await;
        let r2_client = R2Client::setup_connection().await;
        let db_client = PostgresClient::setup_connection().await;

        let system_config = Self::preload(&db_client).await?;
        Ok(Self { llm_client, r2_client, db_client, system_config })
    }
}

#[async_trait::async_trait]
impl Agent for TestImageAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "test_image_generation_v0";
    type Tool = TestImageTool;
    type Input = String;

    fn system_prompt() -> &'static str {
        "You are an AI image generator. Generate beautiful images based on user prompts. Always respond with image generation."
    }
    fn model() -> &'static str { "google/gemini-2.5-flash-image-preview" }
    fn llm_client(&self) -> &LlmClient { &self.llm_client }
    fn db_client(&self) -> &PostgresClient { &self.db_client }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        Ok(vec![
            // System prompt
            Prompt {
                toolcall: None,
                content: Self::system_prompt().to_string(),
                content_type: MessageType::Text,
                role: MessageRole::System,
                created_at: 0,
            },
            // User prompt
            Prompt {
                toolcall: None,
                content: input.clone(),
                content_type: MessageType::Text,
                role: MessageRole::User,
                created_at: 1,
            },
        ])
    }

    async fn handle_output(&self, _input: &Self::Input, message: &Message, _tool: &Self::Tool) -> Result<(Message, Option<Value>)> {
        // For testing, just return the message as-is
        Ok((message.clone(), None))
    }

    fn system_config(&self) -> &SystemConfig {
        &self.system_config
    }
}

#[async_trait::async_trait]
impl ImageAgent for TestImageAgent {
    fn r2_client(&self) -> &R2Client {
        &self.r2_client
    }
}

#[tokio::test]
async fn test_simple_image_generation() {
    let agent = TestImageAgent::new().await
        .expect("Failed to create test agent");

    let caller = Uuid::new_v4();
    let input = "Generate a red square".to_string();

    let result = agent.generate_image(&caller, &input).await;

    assert!(result.is_ok(), "Image generation should succeed: {:?}", result.err());

    let (message, _tool, _misc) = result.unwrap();
    assert_eq!(message.assistant_message_content_type, MessageType::Image);

    println!("✅ Simple image generation test passed");
}

