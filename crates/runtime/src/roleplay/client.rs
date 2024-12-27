use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestMessage, 
    ChatCompletionTool, ChatCompletionToolChoiceOption, CompletionUsage
};
use async_openai::{
    config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client
};

use crate::character::Character;
use crate::system_config::SystemConfig;
use crate::user::User;
use crate::core::{HistoryMessage, HistoryMessagePair, MessageRole, MessageType, RuntimeClient};

use super::env::RoleplayEnv;
use super::message::prepare_chat_messages;

#[derive(Clone)]
pub struct RoleplayRuntimeClient {
    system_configs: Vec<SystemConfig>,
    tools: Vec<ChatCompletionTool>,
    client: Client<OpenAIConfig>,
}

#[derive(Clone)]

pub struct RoleplayClientInput {
    character: Character,
    user: User,

    history: Vec<HistoryMessagePair>,
    new_message: HistoryMessage,
}

#[derive(Clone)]
pub struct RoleplayClientOutput {
    message: HistoryMessage,
    usage: CompletionUsage,
    // TODO: tool call generalization
    maybe_function_call: Option<Vec<ChatCompletionMessageToolCall>>,
}

impl RoleplayRuntimeClient {
    pub fn new(system_configs: Vec<SystemConfig>, tools: Vec<ChatCompletionTool>) -> Self {
        let env = RoleplayEnv::load();
        let config = OpenAIConfig::new()
            .with_api_key(env.openai_api_key)
            .with_api_base(env.openai_base_url);

        let client = Client::build(
            reqwest::Client::new(),
            config,
            Default::default()
        );

        Self { system_configs, tools, client }
    }

    pub async fn send_request(
        &self, 
        system_config: &SystemConfig,
        messages: Vec<ChatCompletionRequestMessage>, 
    ) -> Result<(
        String, CompletionUsage, Option<Vec<ChatCompletionMessageToolCall>>
    )> {
        // Create chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&system_config.openai_model)
            .messages(messages)
            .tools(self.tools.clone())
            .tool_choice(ChatCompletionToolChoiceOption::Auto)
            .temperature(system_config.openai_temperature)
            .max_tokens(system_config.openai_max_tokens)
            .build()?;

        // Send request to OpenAI
        let response = self.client
            .chat()
            .create(request)
            .await?;

        let content = response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response from AI inference server"))?
            .message
            .content
            .clone()
            .unwrap_or_default();

        let maybe_function_call = response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response from AI inference server"))?
            .message
            .clone()
            .tool_calls;

        let usage = response.usage.ok_or(|| {
            tracing::warn!("Model {} returned no usage", system_config.openai_model);
        }).map_err(|_| anyhow!("Model {} returned no usage", system_config.openai_model))?;

        Ok((content, usage, maybe_function_call))
    }
}

impl RuntimeClient<
    RoleplayClientInput, 
    RoleplayClientOutput
> for RoleplayRuntimeClient {
    type Error = anyhow::Error;
    fn get_price(&self) -> u64 { 1 }

    async fn run(
        &self, input: &RoleplayClientInput
    ) -> Result<RoleplayClientOutput, Self::Error> {
        let is_new_conversation = input.history.is_empty();

        // this will be used to send to OpenAI
        let chat_messages = prepare_chat_messages(
            &self.system_configs[0], 
            &input.character, &input.user, 
            &input.history, &input.new_message, 
            is_new_conversation
        )?;

        let (
            content, 
            usage, 
            maybe_function_call
        ) = self.send_request(
            &self.system_configs[0], 
            chat_messages
        ).await?;

        let response_message = HistoryMessage {
            owner: input.user.id.clone(),
            character_id: input.character.id.clone(),
            role: MessageRole::Assistant,
            content,
            content_type: MessageType::Text,
        };

        Ok(RoleplayClientOutput {
            message: response_message,
            usage,
            maybe_function_call,
        })
    }

    async fn regenerate(
        &self, input: &RoleplayClientInput
    ) -> Result<RoleplayClientOutput, Self::Error> {
        let mut new_input = input.clone();
        let last_message = new_input
            .history
            .pop()
            .ok_or(anyhow!("No history found"))?;
        let (new_message, _) = last_message;
        new_input.new_message = new_message;

        self.run(&new_input).await
    }
}
