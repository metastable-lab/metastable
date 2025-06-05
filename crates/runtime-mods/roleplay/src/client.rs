use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_openai::types::FunctionCall;
use async_openai::{config::OpenAIConfig, Client};

use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use voda_common::EnvVars;
use voda_database::{QueryCriteria, SqlxFilterQuery};
use voda_runtime::{
    LLMRunResponse, RuntimeClient, RuntimeEnv, SystemConfig, User
};

use crate::{Character, RoleplayMessage, RoleplayRawMemory, RoleplaySession};

#[derive(Clone)]
pub struct RoleplayRuntimeClient {
    db: Arc<PgPool>,

    client: Client<OpenAIConfig>,

    executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>,
    system_config: SystemConfig,
}

impl RoleplayRuntimeClient {
    pub async fn new(
        db: Arc<PgPool>,
        system_config: SystemConfig,
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

        Self { client, db, executor, system_config }
    }
}

#[async_trait::async_trait]
impl RuntimeClient for RoleplayRuntimeClient {
    const NAME: &'static str = "rolplay";
    type MemoryType = RoleplayRawMemory;

    fn system_config(&self) -> &SystemConfig { &self.system_config }
    fn get_price(&self) -> u64 { 1 }
    fn get_client(&self) -> &Client<OpenAIConfig> { &self.client }

    async fn on_init(&self) -> Result<()> { Ok(()) }
    async fn on_shutdown(&self) -> Result<()> { Ok(()) }

    async fn on_new_message(&self, message: &RoleplayMessage) -> Result<LLMRunResponse> {
        let session_id = message.session_id.clone();
        let session = RoleplaySession::find_one_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("id", "=", session_id.to_hex_string())?,
            &*self.db
        ).await?
            .ok_or(anyhow!("[RoleplayRuntimeClient::on_new_message] Session not found"))?;

        let character_id = session.character_id.clone();
        let character = Character::find_one_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("id", "=", character_id.to_hex_string())?,
            &*self.db
        ).await?
            .ok_or(anyhow!("[RoleplayRuntimeClient::on_new_message] Character not found"))?;

        let user_id = session.owner.clone();
        let user = User::find_one_by_criteria(
            QueryCriteria::new()
                .add_valued_filter("id", "=", user_id.to_hex_string())?,
            &*self.db
        ).await?
            .ok_or(anyhow!("[RoleplayRuntimeClient::on_new_message] User not found"))?;

        let system_message = RoleplayMessage::system(&session, &self.system_config, &character, &user);
        let first_message = RoleplayMessage::first_message(&session, &character, &user);

        let mut messages = vec![system_message, first_message];

        let history = session.fetch_history(&*self.db).await?;
        messages.extend(history);
        messages.push(message.clone());

        let response = self.send_llm_request(&messages).await?;
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

#[cfg(test)]
mod tests {

    use crate::character::{CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus};

    use super::*;

    use sqlx::types::Json;
    use voda_common::{get_current_timestamp, CryptoHash};
    use voda_database::{init_db_pool, SqlxCrud, SqlxPopulateId};
    use voda_runtime::UserRole;

    init_db_pool!(User, Character, RoleplaySession, RoleplayMessage);

    #[tokio::test]
    async fn test_on_new_message() {

        let (tx, _rx) = mpsc::channel(100);
        let system_config = SystemConfig {
            id: CryptoHash::default(),

            name: "test".to_string(),
            system_prompt: "You are a helpful assistant".to_string(),
            system_prompt_version: 1,

            openai_base_url: "https://openrouter.ai/api/v1".to_string(),
            openai_model: "nousresearch/hermes-3-llama-3.1-70b".to_string(),
            openai_temperature: 0.7,
            openai_max_tokens: 1000,

            functions: Json::from(vec![]),
            updated_at: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        let client = RoleplayRuntimeClient::new(
            Arc::new(connect().await.clone()), system_config, tx
        ).await;

        let mut user = User {
            id: CryptoHash::default(),
            user_id: format!("test_user_{}", CryptoHash::random().to_hex_string()),
            user_aka: "test".to_string(),
            role: UserRole::User,
            provider: "test".to_string(),
            last_active: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        user.sql_populate_id().unwrap();
        let user = user.create(&*client.db).await.unwrap();

        let mut character = Character {
            id: CryptoHash::default(),
            name: "test".to_string(),
            description: "test".to_string(),
            creator: user.id.clone(),
            reviewed_by: None,
            version: 1,
            status: CharacterStatus::Published,
            gender: CharacterGender::Male,
            language: CharacterLanguage::English,
            features: vec![CharacterFeature::Roleplay],
            prompts_scenario: "test".to_string(),
            prompts_personality: "test".to_string(),
            prompts_example_dialogue: "test".to_string(),
            prompts_first_message: "test".to_string(),
            tags: vec!["test".to_string()],
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
            published_at: get_current_timestamp(),
        };
        character.sql_populate_id().unwrap();
        let character = character.create(&*client.db).await.unwrap();

        let mut session = RoleplaySession { 
            id: CryptoHash::default(),
            public: true,
            owner: user.id.clone(),
            character_id: character.id.clone(),
            history: vec![],
            updated_at: get_current_timestamp(),
            created_at: get_current_timestamp(),
        };
        session.sql_populate_id().unwrap();
        let session = session.create(&*client.db).await.unwrap();

        let message = RoleplayMessage {
            id: CryptoHash::default(),
            session_id: session.id.clone(),
            owner: user.id.clone(),
            character_id: character.id.clone(),
            role: voda_runtime::MessageRole::User,
            content_type: voda_runtime::MessageType::Text,
            content: "Hello!".to_string(),
            created_at: get_current_timestamp(),
        };

        let result = client.on_new_message(&message).await;
        assert!(result.is_ok());
    }
}