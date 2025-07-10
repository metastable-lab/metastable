use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_openai::config::OpenAIConfig;
use async_openai::Client;
use async_openai::types::{
    ChatCompletionToolArgs, ChatCompletionToolChoiceOption, CompletionUsage, CreateChatCompletionRequestArgs, FinishReason, FunctionCall
};

use sqlx::PgPool;
use sqlx::types::Uuid;
use serde::{Deserialize, Serialize};

use crate::{Memory, Message, SystemConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRunResponse {
    pub caller: Uuid,
    pub content: String,
    pub usage: CompletionUsage,
    pub maybe_function_call: Vec<FunctionCall>,
    pub finish_reason: Option<FinishReason>,
    pub system_config: SystemConfig,
    pub misc_value: Option<serde_json::Value>,
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

    async fn on_shutdown(&self) -> Result<()>;
    async fn on_new_message(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<LLMRunResponse>;
    async fn on_rollback(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<LLMRunResponse>;

    async fn send_llm_request(&self, 
        system_config: &SystemConfig,
        messages: &[<Self::MemoryType as Memory>::MessageType]
    ) -> Result<LLMRunResponse> {
        if messages.len() == 0 {
            return Err(anyhow!("[RuntimeClient::send_llm_request] No messages to send"));
        }

        let caller = messages[0].owner().clone();
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
        let response = self.get_client().chat().create(request).await?;
        let choice = response.choices.first()
            .ok_or(anyhow!("[RuntimeClient::send_llm_request] No response from AI inference server"))?;

        let message = choice.message.clone();
        let finish_reason = choice.finish_reason.clone();

        let usage = response.usage
            .ok_or(anyhow!("[RuntimeClient::send_llm_request] Model {} returned no usage", system_config.openai_model))?
            .clone();

        let content = message.content.clone().unwrap_or_default();

        let maybe_function_call = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();

        Ok(LLMRunResponse {
            caller,
            content,
            usage,
            maybe_function_call,
            finish_reason,
            system_config: system_config.clone(),
            misc_value: None,
        })
    }
}
