use std::sync::Arc;

use anyhow::Result;
use async_openai::types::FunctionCall;
use async_openai::{config::OpenAIConfig, Client};

use serde_json::json;
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use voda_common::EnvVars;
use voda_runtime::{define_function_types, FunctionExecutor, LLMRunResponse, Memory, Message, RuntimeClient, RuntimeEnv, SystemConfig, UserUsage};
use voda_database::{SqlxCrud, QueryCriteria, SqlxFilterQuery};
use voda_runtime_roleplay::Character;

use crate::memory::CharacterCreationMemory;
use crate::{CharacterCreationMessage, preload, SummarizeCharacterFunctionCall};

define_function_types!(
    SummarizeCharacterFunctionCall(SummarizeCharacterFunctionCall, "summarize_character")
);

#[derive(Clone)]
pub struct CharacterCreationRuntimeClient {
    db: Arc<PgPool>,
    memory: Arc<CharacterCreationMemory>,
    client: Client<OpenAIConfig>,
    executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>,
}

impl CharacterCreationRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>,
        system_config_name: String,
        executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Result<Self> {
        let env = RuntimeEnv::load();
        let config = OpenAIConfig::new()
            .with_api_key(env.get_env_var("OPENAI_API_KEY"))
            .with_api_base(env.get_env_var("OPENAI_BASE_URL"));

        let client = Client::build(
            reqwest::Client::new(),
            config,
            Default::default()
        );

        let mut character_creation_memory = CharacterCreationMemory::new(db.clone(), system_config_name.clone());
        character_creation_memory.initialize().await?;

        Ok(Self { client, db, memory: Arc::new(character_creation_memory), executor })
    }
}

#[async_trait::async_trait]
impl RuntimeClient for CharacterCreationRuntimeClient {
    const NAME: &'static str = "character-creation";
    type MemoryType = CharacterCreationMemory;

    fn get_price(&self) -> u64 { 1 }
    fn get_db(&self) -> &Arc<PgPool> { &self.db }
    fn get_client(&self) -> &Client<OpenAIConfig> { &self.client }
    fn get_memory(&self) -> &Arc<CharacterCreationMemory> { &self.memory }

    async fn preload(db: Arc<PgPool>) -> Result<()> {
        tracing::info!("[CharacterCreationRuntimeClient::preload] Preloading character creation runtime client");
        let mut tx = db.begin().await?;

        let preload_config = preload::get_system_configs_for_char_creation();
        match SystemConfig::find_one_by_criteria(
            QueryCriteria::new().add_filter("name", "=", Some(preload_config.name.clone()))?,
            &mut *tx
        ).await? {
            Some(mut db_config) => {
                let mut updated = false;
                if db_config.system_prompt != preload_config.system_prompt {
                    db_config.system_prompt = preload_config.system_prompt;
                    updated = true;
                }
                if db_config.functions != preload_config.functions {
                    db_config.functions = preload_config.functions.clone();
                    updated = true;
                }
                if db_config.openai_model != preload_config.openai_model {
                    db_config.openai_model = preload_config.openai_model;
                    updated = true;
                }
                if db_config.openai_temperature != preload_config.openai_temperature {
                    db_config.openai_temperature = preload_config.openai_temperature;
                    updated = true;
                }
                if db_config.openai_max_tokens != preload_config.openai_max_tokens {
                    db_config.openai_max_tokens = preload_config.openai_max_tokens;
                    updated = true;
                }
                if db_config.openai_base_url != preload_config.openai_base_url {
                    db_config.openai_base_url = preload_config.openai_base_url;
                    updated = true;
                }

                if updated {
                    db_config.update(&mut *tx).await?;
                }
            }
            None => {
                preload_config.create(&mut *tx).await?;
            }
        };
        tx.commit().await?;
        tracing::info!("[CharacterCreationRuntimeClient::on_init] Character creation runtime client initialized");
        Ok(())
    }
    async fn init_function_executor(
        queue: mpsc::Receiver<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Result<()> {        
        tracing::info!("[CharacterCreationRuntimeClient::init_function_executor] Starting function executor");
        let mut function_executor = FunctionExecutor::<RuntimeFunctionType>::new(queue);
        function_executor.run().await;

        Ok(())
    }
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &CharacterCreationMessage) -> Result<LLMRunResponse> {
        let (messages, system_config) = self
            .memory
            .search(&message, 100, 0).await?;

        let mut response = self.send_llm_request(&system_config, &messages).await?;
        let mut assistant_message = CharacterCreationMessage::from_llm_response(
            response.clone(), 
            &message.roleplay_session_id, 
            &message.owner
        );

        let mut tx = self.db.begin().await?;
        assistant_message.character_creation_system_config = system_config.id;
        if let Some(character_str) = assistant_message.character_creation_maybe_character_str.clone() {
            let mut character = serde_json::from_str::<Character>(&character_str)?;
            character.creator = message.owner.clone();
            let char = character.create(&mut *tx).await?;
            assistant_message.character_creation_maybe_character_id = Some(char.id);
        }
        let assistant_message = assistant_message.create(&mut *tx).await?;

        let user_usage = UserUsage::new(
            message.owner.clone(),
            system_config.openai_model.clone(),
            response.usage.clone()
        );
        user_usage.create(&mut *tx).await?;

        tx.commit().await?;

        response.misc_value = Some(json!({
            "character_id": assistant_message.character_creation_maybe_character_id,
        }));

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
