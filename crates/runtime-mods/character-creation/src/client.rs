use std::sync::Arc;

use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::types::{Json, Uuid};
use sqlx::PgPool;
use voda_common::{get_current_timestamp, EnvVars};
use voda_runtime::{toolcalls, ExecutableFunctionCall, LLMRunResponse, Memory, MessageRole, MessageType, RuntimeClient, RuntimeEnv, SystemConfig, UserUsage};
use voda_database::{SqlxCrud, QueryCriteria, SqlxFilterQuery};
use voda_runtime_roleplay::Character;

use crate::memory::CharacterCreationMemory;
use crate::{CharacterCreationMessage, preload, SummarizeCharacterToolCall};

toolcalls!(
    ctx: (),
    tools: [
        (SummarizeCharacterToolCall, "summarize_character", Character)
    ]
);

#[derive(Clone)]
pub struct CharacterCreationRuntimeClient {
    db: Arc<PgPool>,
    memory: Arc<CharacterCreationMemory>,
    client: Client<OpenAIConfig>,
}

impl CharacterCreationRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>,
        system_config_name: String,
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

        Ok(Self { client, db, memory: Arc::new(character_creation_memory) })
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
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &CharacterCreationMessage) -> Result<LLMRunResponse> {
        let (messages, system_config) = self
            .memory
            .search(&message, 100).await?;

        let mut response = self.send_llm_request(&system_config, &messages).await?;
        let function_call = response.maybe_function_call.first()
            .ok_or(anyhow::anyhow!("[CharacterCreationRuntimeClient::on_new_message] No function call found"))?;

        let tc = RuntimeToolcall::from_function_call(function_call.clone())
            .map_err(|e| anyhow::anyhow!("[CharacterCreationRuntimeClient::on_new_message] Failed to parse function call: {}", e))?;
        let result = tc.execute(&response, &()).await?;
        let RuntimeToolcallReturn::SummarizeCharacterToolCall(character) = result;
        let mut tx = self.db.begin().await?;
        let character = character.create(&mut *tx).await?;
        let character_creation_message = CharacterCreationMessage {
            id: Uuid::new_v4(),
            roleplay_session_id: message.roleplay_session_id.clone(),
            character_creation_system_config: system_config.id.clone(),
            owner: message.owner.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            character_creation_call: Json(vec![function_call.clone()]),
            character_creation_maybe_character_str: Some(serde_json::to_string(&character)?),
            character_creation_maybe_character_id: Some(character.id),
            content: response.content.clone(),
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };
        character_creation_message.create(&mut *tx).await?;
        let user_usage = UserUsage::from_llm_response(&response);
        user_usage.create(&mut *tx).await?;
        tx.commit().await?;

        response.misc_value = Some(serde_json::json!({ "character_id": character.id }));

        Ok(response)
    }

    async fn on_rollback(&self, _message: &CharacterCreationMessage) -> Result<LLMRunResponse> {
        unimplemented!()
    }   
}
