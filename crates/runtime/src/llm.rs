use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_openai::{config::OpenAIConfig, Client};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionToolArgs, 
    CompletionUsage, CreateChatCompletionRequestArgs, FunctionCall, FinishReason, FunctionObject
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::SystemConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRunResponse {
    pub caller: Uuid,
    pub content: String,
    pub usage: CompletionUsage,
    pub finish_reason: Option<FinishReason>,
    pub system_config: SystemConfig,
    pub misc_value: Option<serde_json::Value>,
}

// implemented inside the llm-macros crate
pub trait ToolCall: std::fmt::Debug + Sized + Clone + Send + Sync + 'static {
    fn schema() -> serde_json::Value;
    fn try_from_tool_call(tool_call: &FunctionCall) -> Result<Self, serde_json::Error>;
    fn to_function_object() -> FunctionObject;
}

pub trait LlmInput: std::fmt::Debug + Send + Sync {
    fn caller(&self) -> &Uuid;
    fn build(&self) -> Result<Vec<ChatCompletionRequestMessage>>;
}

#[async_trait::async_trait]
pub trait LlmCall: std::fmt::Debug + Send + Sync + Sized + ToolCall {
    const NAME: &'static str;
    type Input: LlmInput;

    fn system_prompt(system_config: &SystemConfig, input: &Self::Input) -> String;

    async fn call(
        client: &Arc<Client<OpenAIConfig>>,
        system_config: &SystemConfig,
        input: &Self::Input
    ) -> Result<(LLMRunResponse, Self)> {
        tracing::debug!("[LlmCall::call] Calling LLM: {}", Self::NAME);

        let system_prompt = Self::system_prompt(system_config, &input);
        let system = ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()
                .expect("[LlmCall::call] System message should build")
        );

        let mut messages = vec![system];
        let input_messages = input.build()?;

        if input_messages.len() == 0 {
            return Err(anyhow!("[LlmCall::call] No input messages"));
        }
        messages.extend(input_messages);

        let tools = system_config.functions.iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("[LlmCall::call] Tool should build")
            )
            .collect::<Vec<_>>();

        let request = CreateChatCompletionRequestArgs::default()
            .model(system_config.openai_model.clone())
            .messages(messages)
            .tools(tools)
            .temperature(system_config.openai_temperature)
            .max_tokens(system_config.openai_max_tokens as u32)
            .build()?;

        let response = client.chat().create(request).await?;
        let choice = response.choices.first()
            .ok_or(anyhow!("[LlmCall::call] No response from AI inference server for model {}", system_config.openai_model))?;

        let message = choice.message.clone();
        let finish_reason = choice.finish_reason.clone();
        let usage = response.usage
            .ok_or(anyhow!("[LlmCall::call] Model {} returned no usage", system_config.openai_model))?
            .clone();

        let content = message.content.clone().unwrap_or_default();
        let maybe_function_call = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();

        if maybe_function_call.len() == 0 {
            return Err(anyhow!("[LlmCall::call] No function call in the response"));
        }

        if maybe_function_call.len() > 1 {
            return Err(anyhow!("[LlmCall::call] Multiple function calls in the response"));
        }

        let output = Self::try_from_tool_call(&maybe_function_call[0])?;

        Ok((LLMRunResponse {
            caller: input.caller().clone(),
            content,
            usage,
            finish_reason,
            system_config: system_config.clone(),
            misc_value: None,
        }, output))
    }
}