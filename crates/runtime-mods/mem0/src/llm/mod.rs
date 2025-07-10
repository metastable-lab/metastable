mod del_relationship;

mod extract_entity;
mod extract_facts;
mod extract_relationship;
mod update_memory;

use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs, FunctionObject
};
use sqlx::types::Uuid;
use voda_runtime::{ExecutableFunctionCall, LLMRunResponse, SystemConfig};
use crate::Mem0Engine;

pub use del_relationship::{DeleteGraphMemoryToolcall, DeleteGraphMemoryToolInput};
pub use extract_entity::{EntitiesToolcall, ExtractEntityToolInput};
pub use extract_facts::{FactsToolcall, ExtractFactsToolInput};
pub use extract_relationship::{RelationshipsToolcall, ExtractRelationshipToolInput};
pub use update_memory::{MemoryUpdateToolcall, MemoryUpdateToolInput};

pub trait ToolInput: std::fmt::Debug + Send + Sync {
    fn user_id(&self) -> Uuid;
    fn agent_id(&self) -> Option<Uuid>;
    fn build(&self) -> String;
}

#[async_trait::async_trait]
pub trait LlmTool: ExecutableFunctionCall {
    type ToolInput: ToolInput;

    fn tools() -> Vec<FunctionObject>;
    fn system_prompt(input: &Self::ToolInput) -> String;

    fn tool_input(&self) -> Option<Self::ToolInput>;
    fn set_tool_input(&mut self, tool_input: Self::ToolInput);
    
    fn model() -> &'static str { "x-ai/grok-3-mini" }
    fn temperature() -> f32 { 0.7 }
    fn max_tokens() -> i32 { 10000 }

    async fn call(engine: &Mem0Engine, tool_input: Self::ToolInput) -> Result<(Self, LLMRunResponse)> {
        tracing::debug!("[LlmTool::call] Calling LLM: {}", Self::name());
        let user_message = tool_input.build();
        let messages = [
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(Self::system_prompt(&tool_input))
                    .build()
                    .expect("[LlmTool::call] System message should build")
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_message)
                    .build()?
            ),
        ];

        let tools = Self::tools().iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("[LlmTool::call] Tool should build")
            )
            .collect::<Vec<_>>();


        let request = CreateChatCompletionRequestArgs::default()
            .model(Self::model())
            .messages(messages)
            .tools(tools)
            .temperature(Self::temperature())
            .max_tokens(Self::max_tokens() as u32)
            .tool_choice(ChatCompletionToolChoiceOption::Auto)
            .build()?;

        let response = engine.get_llm().chat().create(request).await?;
        let choice = response.choices.first()
            .ok_or(anyhow!("[LlmTool::call] No response from AI inference server for model {}", Self::model()))?;

        let message = choice.message.clone();
        let finish_reason = choice.finish_reason.clone();
        let usage = response.usage
            .ok_or(anyhow!("[LlmTool::call] Model {} returned no usage", Self::model()))?
            .clone();

        let content = message.content.clone().unwrap_or_default();
        let maybe_function_call = message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();

        let llm_response = LLMRunResponse {
            caller: tool_input.user_id(),
            content,
            usage,
            maybe_function_call: maybe_function_call.clone(),
            finish_reason,
            system_config: SystemConfig::default(),
            misc_value: None,
        };

        let the_tool_call = maybe_function_call.first()
            .ok_or(anyhow!("[LlmTool::call] No tool calls found for tool {}", Self::name()))?;
        let mut tool_call = Self::from_function_call(the_tool_call.clone())?;
        tool_call.set_tool_input(tool_input);
        Ok((tool_call, llm_response))
    }
}
