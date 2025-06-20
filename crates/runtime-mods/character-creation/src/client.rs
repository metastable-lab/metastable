use std::sync::Arc;

use anyhow::Result;
use async_openai::types::FunctionCall;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use voda_common::EnvVars;
use voda_runtime::{LLMRunResponse, Memory, Message, RuntimeClient, RuntimeEnv, UserUsage};
use voda_database::SqlxCrud;

use crate::memory::CharacterCreationMemory;
use crate::CharacterCreationMessage;

#[derive(Clone)]
pub struct CharacterCreationRuntimeClient {
    db: Arc<PgPool>,
    character_creation_memory: Arc<CharacterCreationMemory>,
    client: Client<OpenAIConfig>,
    executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>,
}

impl CharacterCreationRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>,
        character_creation_memory: Arc<CharacterCreationMemory>,
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

        Self { client, db, character_creation_memory, executor }
    }
}

#[async_trait::async_trait]
impl RuntimeClient for CharacterCreationRuntimeClient {
    const NAME: &'static str = "character-creation";
    type MemoryType = CharacterCreationMemory;

    fn get_price(&self) -> u64 { 1 }
    fn get_db(&self) -> &Arc<PgPool> { &self.db }
    fn get_client(&self) -> &Client<OpenAIConfig> { &self.client }
    fn get_memory(&self) -> &Arc<CharacterCreationMemory> { &self.character_creation_memory }

    async fn on_init(&self) -> Result<()> { Ok(()) }
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &CharacterCreationMessage) -> Result<LLMRunResponse> {
        let (messages, system_config) = self
            .character_creation_memory
            .search(&message, 100, 0).await?;

        let response = self.send_llm_request(&system_config, &messages).await?;
        let mut assistant_message = CharacterCreationMessage::from_llm_response(
            response.clone(), 
            &message.roleplay_session_id, 
            &message.owner
        );
        assistant_message.character_creation_system_config = system_config.id;

        self.character_creation_memory.add_messages(&[
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

    async fn on_rollback(&self, _message: &CharacterCreationMessage) -> Result<LLMRunResponse> {
        unimplemented!()
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
