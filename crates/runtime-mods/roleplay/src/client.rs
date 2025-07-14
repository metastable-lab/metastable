use std::sync::Arc;

use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::PgPool;
use sqlx::types::Uuid;
use tokio::sync::mpsc;
use tokio::time::Instant;
use voda_common::{get_current_timestamp, EnvVars};
use voda_runtime::{toolcalls, ExecutableFunctionCall, LLMRunResponse, Memory, MessageRole, MessageType, RuntimeClient, RuntimeEnv, SystemConfig, User, UserRole, UserUsage};
use voda_database::{SqlxCrud, QueryCriteria, SqlxFilterQuery};
use voda_runtime_mem0::Mem0Messages;

use crate::{RoleplayMessage, RoleplayRawMemory, preload, Character};
use crate::preload::ShowStoryOptionsToolCall;

toolcalls!(
    ctx: (),
    tools: [
        (ShowStoryOptionsToolCall, "show_story_options", Vec<String>),
    ]
);


#[derive(Clone)]
pub struct RoleplayRuntimeClient {
    db: Arc<PgPool>,
    memory: Arc<RoleplayRawMemory>,
    client: Client<OpenAIConfig>,
}

impl RoleplayRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>, pgvector_db: Arc<PgPool>,
    ) -> Result<(Self, mpsc::Receiver<Vec<Mem0Messages>>)> {
        let env = RuntimeEnv::load();
        let config = OpenAIConfig::new()
            .with_api_key(env.get_env_var("OPENAI_API_KEY"))
            .with_api_base(env.get_env_var("OPENAI_BASE_URL"));

        let client = Client::build(
            reqwest::Client::new(),
            config,
            Default::default()
        );

        let (mem0_messages_tx, mem0_messages_rx) = mpsc::channel(100);
        let memory = RoleplayRawMemory::new(db.clone(), pgvector_db.clone(), mem0_messages_tx).await?;
        Ok((Self { client, db, memory: Arc::new(memory) }, mem0_messages_rx))
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

        // 1. upsert system configs
        let preload_configs = vec![
            preload::get_system_configs_for_char_creation(),
            preload::get_system_configs_for_roleplay(),
        ];
        
        for preload_config in preload_configs {
            let existing_config = SystemConfig::find_one_by_criteria(
                QueryCriteria::new().add_filter("name", "=", Some(preload_config.name.clone())),
                &mut *tx
            ).await?;

            if existing_config.is_none() {
                preload_config.create(&mut *tx).await?;
            } else {
                let mut db_config = existing_config.unwrap();
                let mut needs_update = false;
                if db_config.system_prompt != preload_config.system_prompt {
                    db_config.system_prompt = preload_config.system_prompt.clone();
                    needs_update = true;
                }

                if db_config.openai_model != preload_config.openai_model {
                    db_config.openai_model = preload_config.openai_model.clone();
                    needs_update = true;
                }

                if db_config.openai_temperature != preload_config.openai_temperature {
                    db_config.openai_temperature = preload_config.openai_temperature;
                    needs_update = true;
                }

                if db_config.openai_max_tokens != preload_config.openai_max_tokens {
                    db_config.openai_max_tokens = preload_config.openai_max_tokens;
                    needs_update = true;
                }

                if db_config.functions != preload_config.functions {
                    db_config.functions = preload_config.functions.clone();
                    needs_update = true;
                }

                if needs_update {
                    db_config.update(&mut *tx).await?;
                }
            }
        }

        // 2. find admin user
        let admin_user = User::find_one_by_criteria(
            QueryCriteria::new().add_filter("role", "=", Some(UserRole::Admin.to_string())),
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[RoleplayRuntimeClient::on_init] No admin user found"))?;

        // 3. upsert characters
        let preload_chars = preload::get_characters_for_char_creation(admin_user.id);
        for preload_char in preload_chars {
            let existing_char = Character::find_one_by_criteria(
                QueryCriteria::new().add_filter("name", "=", Some(preload_char.name.clone())),
                &mut *tx
            ).await?;

            if existing_char.is_none() {
                preload_char.create(&mut *tx).await?;
            } else {
                let mut db_char = existing_char.unwrap();
                let mut updated = false;
                if db_char.description != preload_char.description {
                    db_char.description = preload_char.description;
                    updated = true;
                }
                if db_char.features != preload_char.features {
                    db_char.features = preload_char.features.clone();
                    updated = true;
                }
                if db_char.version != preload_char.version {
                    db_char.version = preload_char.version;
                    updated = true;
                }

                if updated {
                    db_char.update(&mut *tx).await?;
                }
            }
        }

        tx.commit().await?;
        tracing::info!("[RoleplayRuntimeClient::preload] Roleplay runtime client preloaded");
        Ok(())
    }

    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &RoleplayMessage) -> Result<LLMRunResponse> {
        tracing::debug!("[RoleplayRuntimeClient::on_new_message] New message start");
        let time = Instant::now();
        let (messages, system_config) = self.memory
            .search(&message, 100).await?;
        tracing::debug!("[RoleplayRuntimeClient::on_new_message] Memory search took {:?}", time.elapsed());

        let time = Instant::now();
        let response = self.send_llm_request(&system_config, &messages).await?;

        let mut final_options = vec![];
        if let Some(function_call) = response.maybe_function_call.first() {
            let maybe_toolcall = RuntimeToolcall::from_function_call(function_call.clone());
            if let Ok(toolcall) = maybe_toolcall {
                let toolcall_result = toolcall.execute(&response, &()).await;
                if let Ok(RuntimeToolcallReturn::ShowStoryOptionsToolCall(options)) = toolcall_result {
                    final_options = options.clone();
                }
            }
        }
        tracing::debug!("[RoleplayRuntimeClient::on_new_message] Options: {:?}", final_options);
        let user_usage = UserUsage::from_llm_response(&response);
        let assistant_message = RoleplayMessage {
            id: Uuid::default(),
            owner: message.owner.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: response.content.clone(),
            session_id: message.session_id.clone(),
            options: final_options,

            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };
        tracing::debug!("[RoleplayRuntimeClient::on_new_message] Assistant message took {:?}", time.elapsed());

        self.memory.add_messages(&[
            message.clone(),
            assistant_message.clone(),
        ]).await?;
        tracing::debug!("[RoleplayRuntimeClient::on_new_message] Memory add took {:?}", time.elapsed());

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

        let user_usage = UserUsage::from_llm_response(&response);
        user_usage.create(&*self.db).await?;

        Ok(response)
    }  
}
