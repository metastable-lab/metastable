use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_openai::{config::OpenAIConfig, Client};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionToolArgs, 
    CompletionUsage, CreateChatCompletionRequestArgs, FunctionCall, FinishReason, FunctionObject
};
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use crate::SystemConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRunResponse {
    pub caller: Uuid,
    pub system_config_name: &'static str,

    pub content: String,
    pub usage: CompletionUsage,
    pub finish_reason: Option<FinishReason>,
    pub misc_value: Option<serde_json::Value>,
}

// implemented inside the llm-macros crate
pub trait ToolCall: std::fmt::Debug + Sized + Clone + Send + Sync + 'static {
    fn schema() -> serde_json::Value;
    fn try_from_tool_call(tool_call: &FunctionCall) -> Result<Self, serde_json::Error>;
    fn to_function_object() -> FunctionObject;
}

#[async_trait::async_trait]
pub trait Agent: std::fmt::Debug + Send + Sync + Sized {
    const SYSTEM_CONFIG_NAME: &'static str;
    type Tool: ToolCall;
    type Input: std::fmt::Debug + Send + Sync + Clone + Default;

    fn system_prompt(&self) -> String;
    fn model() -> &'static str { "google/gemini-2.5-flash" }
    fn base_url() -> &'static str { "https://openrouter.ai/api/v1" }
    fn temperature() -> f32 { 0.7 }
    fn max_tokens() -> i32 { 20000 }

    fn caller(&self) -> &Uuid;
    async fn input(&self, input: &Self::Input) -> Result<Vec<ChatCompletionRequestMessage>>;
    async fn output(&self, output: &LLMRunResponse) -> Result<()>;

    fn to_system_config(&self) -> SystemConfig {
        SystemConfig {
            id: Uuid::new_v4(),
            name: Self::SYSTEM_CONFIG_NAME.to_string(),
            system_prompt_version: 0,
            system_prompt: self.system_prompt(),
            openai_model: Self::model().to_string(),
            openai_temperature: Self::temperature(),
            openai_max_tokens: Self::max_tokens(),
            openai_base_url: Self::base_url().to_string(),
            functions: Json(vec![Self::Tool::to_function_object()]),
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn call(
        &self, client: &Arc<Client<OpenAIConfig>>, input: &Self::Input
    ) -> Result<(LLMRunResponse, Self::Tool)> {
        tracing::debug!("[Agent::call] Calling Agent: {}", Self::SYSTEM_CONFIG_NAME);

        let system_prompt = self.system_prompt();
        let system = ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()
                .expect("[Agent::call] System message should build")
        );

        let mut messages = vec![system];
        let input_messages = self.input(input).await?;

        if input_messages.len() == 0 {
            return Err(anyhow!("[Agent::call] No input messages"));
        }
        messages.extend(input_messages);

        let tools = vec![
            ChatCompletionToolArgs::default()
                .function(Self::Tool::to_function_object())
                .build()
                .expect("[Agent::call] Tool should build")
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(Self::model())
            .messages(messages)
            .tools(tools)
            .temperature(Self::temperature())
            .max_tokens(Self::max_tokens() as u32)
            .build()?;

        let response = client.chat().create(request).await?;
        let choice = response.choices.first()
            .ok_or(anyhow!("[Agent::call] No response from AI inference server for model {}", Self::model()))?;

        let message = choice.message.clone();
        let finish_reason = choice.finish_reason.clone();
        let usage = response.usage
            .ok_or(anyhow!("[Agent::call] Model {} returned no usage", Self::model()))?
            .clone();

        let content = message.content.clone().unwrap_or_default();
        let maybe_function_call = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();

        if maybe_function_call.len() == 0 {
            return Err(anyhow!("[Agent::call] No function call in the response"));
        }

        if maybe_function_call.len() > 1 {
            return Err(anyhow!("[Agent::call] Multiple function calls in the response"));
        }

        Ok((LLMRunResponse {
            caller: self.caller().clone(),
            system_config_name: Self::SYSTEM_CONFIG_NAME,
            content,
            usage,
            finish_reason,
            misc_value: None,
        }, Self::Tool::try_from_tool_call(&maybe_function_call[0])?))
    }
}
