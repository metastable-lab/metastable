use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_openai::config::OpenAIConfig;
use async_openai::Client;
use async_openai::types::{
    ChatCompletionToolArgs, ChatCompletionToolChoiceOption, 
    CompletionUsage, CreateChatCompletionRequestArgs, FunctionCall
};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::{Memory, Message, SystemConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRunResponse {
    pub content: String,
    pub usage: CompletionUsage,
    pub maybe_function_call: Vec<FunctionCall>,
    pub maybe_results: Vec<String>,
}

#[async_trait::async_trait]
pub trait RuntimeClient: Clone + Send + Sync + 'static {
    const NAME: &'static str;
    type MemoryType: Memory;

    fn get_db(&self) -> &Arc<PgPool>;
    fn get_memory(&self) -> &Arc<Self::MemoryType>;
    fn get_price(&self) -> u64;
    fn get_client(&self) -> &Client<OpenAIConfig>;

    async fn preload(db: Arc<PgPool>) -> Result<()>;
    async fn init_function_executor(
        queue: mpsc::Receiver<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Result<()>;

    async fn on_shutdown(&self) -> Result<()>;
    async fn on_new_message(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<LLMRunResponse>;
    async fn on_rollback(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<LLMRunResponse>;
    async fn on_tool_call(&self, call: &FunctionCall) -> Result<String>;

    // NOTE: toolcall are sent for execution here, and results are returned here
    async fn send_llm_request(&self, 
        system_config: &SystemConfig,
        messages: &[<Self::MemoryType as Memory>::MessageType]
    ) -> Result<LLMRunResponse> {
        let messages = Message::pack(messages)?;

        let tools = system_config.functions.iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("Message should build")
            )
            .collect::<Vec<_>>();

        // Create chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&system_config.openai_model)
            .messages(messages)
            .tools(tools)
            .tool_choice(ChatCompletionToolChoiceOption::Auto)
            .temperature(system_config.openai_temperature)
            .max_tokens(system_config.openai_max_tokens as u32)
            .build()?;

        // Send request to OpenAI
        let response = self.get_client()
            .chat()
            .create(request)
            .await?;

        let content = response
            .choices
            .first()
            .ok_or(anyhow!("No response from AI inference server"))?
            .message
            .content
            .clone()
            .unwrap_or_default();

        let maybe_function_call = response
            .choices
            .first()
            .ok_or(anyhow!("No response from AI inference server"))?
            .message
            .clone()
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();


        let mut maybe_results = Vec::new();
        for maybe_function in maybe_function_call.iter() {
            let result = self.on_tool_call(maybe_function).await?;
            maybe_results.push(result);
        }

        let usage = response.usage.ok_or(|| {
            tracing::warn!("Model {} returned no usage", system_config.openai_model);
        }).map_err(|_| anyhow!("Model {} returned no usage", system_config.openai_model))?;

        Ok(LLMRunResponse {
            content,
            usage,
            maybe_function_call,
            maybe_results,
        })
    }
}
