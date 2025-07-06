use std::sync::Arc;

use anyhow::Result;
use async_openai::types::FunctionCall;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use voda_common::EnvVars;
use voda_runtime::{LLMRunResponse, Memory, Message, RuntimeClient, RuntimeEnv, UserUsage};
use voda_database::SqlxCrud;

use crate::{RoleplayMessage, RoleplayRawMemory};

#[derive(Clone)]
pub struct RoleplayRuntimeClient {
    db: Arc<PgPool>,
    memory: Arc<RoleplayRawMemory>,
    client: Client<OpenAIConfig>,
    executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>,
}

impl RoleplayRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>,
        memory: Arc<RoleplayRawMemory>,
        executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Self {
        let env = RuntimeEnv::load();
        let config = OpenAIConfig::new()
            .with_api_key(env.get_env_var("OPENAI_API_KEY"))
            .with_api_base(env.get_env_var("OPENAI_BASE_URL"));

        let client = Client::build(
            reqwest::Client::new(),
            config,
            Default::default()
        );

        Self { client, db, memory, executor }
    }
}

#[async_trait::async_trait]
impl RuntimeClient for RoleplayRuntimeClient {
    const NAME: &'static str = "rolplay";
    type MemoryType = RoleplayRawMemory;

    fn get_price(&self) -> u64 { 1 }
    fn get_db(&self) -> &Arc<PgPool> { &self.db }
    fn get_client(&self) -> &Client<OpenAIConfig> { &self.client }
    fn get_memory(&self) -> &Arc<RoleplayRawMemory> { &self.memory }

    async fn on_init(&self) -> Result<()> { Ok(()) }
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &RoleplayMessage) -> Result<LLMRunResponse> {
        let (messages, system_config) = self.memory
            .search(&message, 100).await?;

        let response = self.send_llm_request(&system_config, &messages).await?;
        let assistant_message = RoleplayMessage::from_llm_response(
            response.clone(), 
            &message.session_id, 
            &message.owner
        );

        self.memory.add_messages(&[
            message.clone(),
            assistant_message.clone(),
        ]).await?;

        let user_usage = UserUsage::new(
            message.owner.clone(),
            system_config.openai_model.clone(),
            response.usage.clone()
        );
        user_usage.create(&*self.db).await?;

        Ok(response)
    }

    async fn on_rollback(&self, message: &RoleplayMessage) -> Result<LLMRunResponse> {
        let (mut messages, system_config) = self.memory
            .search(&message, 100).await?;
        let mut last_assistant_message = messages.pop()
            .ok_or(anyhow::anyhow!("[RoleplayRuntimeClient::on_rollback] No last message found"))?;

        let response = self.send_llm_request(&system_config, &messages).await?;
        last_assistant_message.content = response.content.clone();
        self.memory.update(&[last_assistant_message]).await?;

        let user_usage = UserUsage::new(
            message.owner.clone(),
            system_config.openai_model.clone(),
            response.usage.clone()
        );
        user_usage.create(&*self.db).await?;

        Ok(response)
    }

    async fn on_tool_call(
        &self, call: &FunctionCall
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.executor.send((call.clone(), tx)).await?;
        let result = rx.await?;
        result
    }    
}
