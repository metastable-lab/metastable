use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionToolArgs, 
    CompletionUsage, CreateChatCompletionRequestArgs, FunctionCall, FinishReason, FunctionObject
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::{Json, Uuid};

use metastable_clients::LlmClient;
use metastable_common::ModuleClient;

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
pub trait Agent: Clone + Send + Sync + Sized {
    const SYSTEM_CONFIG_NAME: &'static str;
    type Tool: ToolCall;
    type Input: std::fmt::Debug + Send + Sync + Clone + Default;

    fn system_prompt() -> &'static str { "" }
    fn model() -> &'static str { "google/gemini-2.5-flash" }
    fn base_url() -> &'static str { "https://openrouter.ai/api/v1" }
    fn temperature() -> f32 { 0.7 }
    fn max_tokens() -> i32 { 20000 }

    fn llm_client(&self) -> &LlmClient;
    async fn build_input(&self, input: &Self::Input) -> Result<Vec<ChatCompletionRequestMessage>>;
    async fn handle_output(&self, input: &Self::Input, output: &LLMRunResponse, tool: &Self::Tool) -> Result<Option<Value>>;

    fn to_system_config(&self) -> SystemConfig {
        SystemConfig {
            id: Uuid::new_v4(),
            name: Self::SYSTEM_CONFIG_NAME.to_string(),
            system_prompt_version: 0,
            system_prompt: Self::system_prompt().to_string(),
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
        &self, caller: &Uuid, input: &Self::Input
    ) -> Result<(LLMRunResponse, Self::Tool)> {
        tracing::debug!("[Agent::call] Calling Agent: {}", Self::SYSTEM_CONFIG_NAME);
        let messages = self.build_input(input).await?;

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

        let response = self.llm_client().get_client().chat().create(request).await?;
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

        let mut response = LLMRunResponse {
            caller: caller.clone(),
            system_config_name: Self::SYSTEM_CONFIG_NAME,
            content,
            usage,
            finish_reason,
            misc_value: None,
        };
        let tool = Self::Tool::try_from_tool_call(&maybe_function_call[0])?;
        response.misc_value = self.handle_output(input, &response, &tool).await?;

        Ok((response, tool))
    }
}
