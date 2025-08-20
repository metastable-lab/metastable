use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionToolArgs, CreateChatCompletionRequestArgs, FunctionCall, FunctionObject
};
use metastable_database::{QueryCriteria, SqlxCrud, SqlxFilterQuery};
use serde_json::Value;
use sqlx::types::{Json, Uuid};

use metastable_clients::{LlmClient, PostgresClient};
use metastable_common::ModuleClient;

use crate::{Message, MessageType, Prompt, SystemConfig};

// implemented inside the llm-macros crate
pub trait ToolCall: std::fmt::Debug + Sized + Clone + Send + Sync + 'static {
    fn schema() -> serde_json::Value;
    fn try_from_tool_call(tool_call: &FunctionCall) -> Result<Self, serde_json::Error>;
    fn into_tool_call(&self) -> Result<FunctionCall, serde_json::Error>;
    fn to_function_object() -> FunctionObject;
}

#[async_trait::async_trait]
pub trait Agent: Clone + Send + Sync + Sized {
    const SYSTEM_CONFIG_NAME: &'static str;
    type Tool: ToolCall;
    type Input: std::fmt::Debug + Send + Sync + Clone;

    fn system_prompt() -> &'static str;
    fn base_url() -> &'static str { "https://openrouter.ai/api/v1" }
    fn model() -> &'static str { "google/gemini-2.5-flash" }
    fn temperature() -> f32 { 0.7 }
    fn max_tokens() -> i32 { 20000 }

    fn llm_client(&self) -> &LlmClient;
    fn db_client(&self) -> &PostgresClient;

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>>;
    async fn handle_output(&self, input: &Self::Input, message: &Message, tool: &Self::Tool) -> Result<Option<Value>>;

    fn to_system_config() -> SystemConfig {
        SystemConfig {
            id: Uuid::new_v4(),
            name: Self::SYSTEM_CONFIG_NAME.to_string(),
            system_prompt_version: 0,
            system_prompt: Self::system_prompt().to_string(),
            openai_model: Self::model().to_string(),
            openai_temperature: Self::temperature(),
            openai_max_tokens: Self::max_tokens(),
            openai_base_url: Self::base_url().to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }
    fn system_config(&self) -> &SystemConfig;

    async fn preload(db: &PostgresClient) -> Result<SystemConfig> {
        let mut tx = db.get_client().begin().await?;
        let system_config = SystemConfig::find_one_by_criteria(
            QueryCriteria::new()
                .add_filter("name", "=", Some(Self::SYSTEM_CONFIG_NAME.to_string())),
            &mut *tx
        ).await?;

        let default_system_config = Self::to_system_config();

        let c = if let Some(db_config) = system_config {
            let mut db_config = db_config.clone();
            let mut needs_update = false;
            if db_config.system_prompt != default_system_config.system_prompt {
                db_config.system_prompt = default_system_config.system_prompt.clone();
                needs_update = true;
            }

            if db_config.openai_model != default_system_config.openai_model {
                db_config.openai_model = default_system_config.openai_model.clone();
                needs_update = true;
            }

            if db_config.openai_temperature != default_system_config.openai_temperature {
                db_config.openai_temperature = default_system_config.openai_temperature;
                needs_update = true;
            }

            if db_config.openai_max_tokens != default_system_config.openai_max_tokens {
                db_config.openai_max_tokens = default_system_config.openai_max_tokens;
                needs_update = true;
            }

            if needs_update {
                db_config.system_prompt_version += 1;
                db_config.clone().update(&mut *tx).await?;
            }
            db_config   
        } else {
            default_system_config.clone().create(&mut *tx).await?
        };

        tx.commit().await?;
        Ok(c)
    }

    async fn call(
        &self, caller: &Uuid, input: &Self::Input
    ) -> Result<(Message, Self::Tool, Option<Value>)> {
        tracing::debug!("[Agent::call] Calling Agent: {}", Self::SYSTEM_CONFIG_NAME);
        let messages = self.build_input(input).await?;
        let messages = Prompt::validate_and_sort(messages)?;
        let user_message = messages.last().expect("already validated");

        let llm_messages = Prompt::pack(messages.clone())?;

        let tools = vec![
            ChatCompletionToolArgs::default()
                .function(Self::Tool::to_function_object())
                .build()
                .expect("[Agent::call] Tool should build")
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(Self::model())
            .messages(llm_messages)
            .tools(tools)
            .temperature(Self::temperature())
            .max_tokens(Self::max_tokens() as u32)
            .build()?;

        let response = self.llm_client().get_client().chat().create(request).await?;
        let choice = response.choices.first()
            .ok_or(anyhow!("[Agent::call] No response from AI inference server for model {}", Self::model()))?;

        let message = choice.message.clone();
        let finish_reason = choice.finish_reason.clone();
        let refusal = choice.message.refusal.clone();
        let usage = response.usage
            .ok_or(anyhow!("[Agent::call] Model {} returned no usage", Self::model()))?
            .clone();

        let content = message.content
            .ok_or(anyhow!("[Agent::call] No content in the response"))?;

        let tool_calls = message
            .tool_calls
            .unwrap_or_default();

        if tool_calls.len() == 0 {
            return Err(anyhow!("[Agent::call] No function call in the response"));
        }

        if tool_calls.len() > 1 {
            return Err(anyhow!("[Agent::call] Multiple function calls in the response"));
        }

        let resulting_message = Message {
            id: Uuid::new_v4(),
            owner: caller.clone(),
            system_config: self.system_config().id,
            session: None,
            
            user_message_content: user_message.content.clone(),
            user_message_content_type: user_message.content_type.clone(),
            input_toolcall: Json(None),
            
            assistant_message_content: content.clone(),
            assistant_message_content_type: MessageType::Text,
            assistant_message_tool_call: Json(Some(tool_calls[0].function.clone())),

            model_name: Self::model().to_string(),
            usage: Json(Some(usage)),
            finish_reason: finish_reason.map(|finish_reason| format!("{:?}", finish_reason)),
            refusal: refusal.clone(),

            is_stale: false,
            is_memorizeable: false,
            is_in_memory: false,

            created_at: 0,
            updated_at: 0,
        };

        let tool = Self::Tool::try_from_tool_call(&tool_calls[0].function)?;
        let misc_value = self.handle_output(input, &resulting_message, &tool).await?;

        Ok((resulting_message, tool, misc_value))
    }
}
