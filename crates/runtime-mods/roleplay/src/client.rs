use std::sync::Arc;

use anyhow::Result;
use async_openai::types::FunctionCall;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use voda_common::EnvVars;
use voda_runtime::{LLMRunResponse, Memory, Message, RuntimeClient, RuntimeEnv, UserUsage, User, SystemConfig, UserRole};
use voda_database::{SqlxCrud, QueryCriteria, SqlxFilterQuery};

use crate::{RoleplayMessage, RoleplayRawMemory, preload, Character, CharacterFeature};

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

        let memory = RoleplayRawMemory::new(db.clone());
        Ok(Self { client, db, memory: Arc::new(memory), executor })
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

    async fn preload(db: Arc<PgPool>) -> Result<()> { 
        tracing::info!("[RoleplayRuntimeClient::preload] Preloading roleplay runtime client");
        let mut tx = db.begin().await?;

        // 1. upsert system config
        let preload_config = preload::get_system_configs_for_char_creation();
        let _system_config_id = match SystemConfig::find_one_by_criteria(
            QueryCriteria::new().add_filter("name", "=", Some(preload_config.name.clone()))?,
            &mut *tx
        ).await? {
            Some(mut db_config) => {
                if db_config.system_prompt != preload_config.system_prompt {
                    db_config.system_prompt = preload_config.system_prompt;
                    db_config = db_config.update(&mut *tx).await?;
                }
                db_config.id
            }
            None => {
                let new_config = preload_config.create(&mut *tx).await?;
                new_config.id
            }
        };

        // 2. find admin user
        let admin_user = User::find_one_by_criteria(
            QueryCriteria::new().add_filter("role", "=", Some(UserRole::Admin.to_string()))?,
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[RoleplayRuntimeClient::on_init] No admin user found"))?;

        // 3. upsert characters
        let preload_chars = preload::get_characters_for_char_creation(admin_user.id);
        for mut preload_char in preload_chars {
            preload_char.features = vec![ CharacterFeature::CharacterCreation ];

            match Character::find_one_by_criteria(
                QueryCriteria::new().add_filter("name", "=", Some(preload_char.name.clone()))?,
                &mut *tx
            ).await? {
                Some(mut db_char) => {
                    let mut updated = false;
                    if db_char.description != preload_char.description {
                        db_char.description = preload_char.description;
                        updated = true;
                    }
                    if db_char.features != preload_char.features {
                        db_char.features = preload_char.features.clone();
                        updated = true;
                    }

                    if updated {
                        db_char.update(&mut *tx).await?;
                    }
                }
                None => {
                    preload_char.create(&mut *tx).await?;
                }
            }
        }

        tx.commit().await?;
        tracing::info!("[RoleplayRuntimeClient::preload] Roleplay runtime client preloaded");
        Ok(())
    }

    async fn init_function_executor(
        _queue: mpsc::Receiver<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Result<()> {
        tracing::info!("[RoleplayRuntimeClient::init_function_executor] Starting function executor");
        Ok(())
    }
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &RoleplayMessage) -> Result<LLMRunResponse> {
        let (messages, system_config) = self.memory
            .search(&message, 100, 0).await?;

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
            .search(&message, 100, 0).await?;
        messages.pop(); // pop the placeholder message
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
