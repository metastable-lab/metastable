use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionToolArgs, 
    ChatCompletionToolChoiceOption, CompletionUsage, FunctionCall
};
use async_openai::{
    config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client
};

use tokio::sync::{mpsc, oneshot};
use voda_common::{blake3_hash, EnvVars, get_current_timestamp};
use voda_database::{get_db, Database, MongoDbEnv, MongoDbObject};
use voda_runtime::{
    Character, ConversationMemory, HistoryMessage, MessageRole, MessageType, RuntimeClient, RuntimeEnv, SystemConfig, User
};

use super::message::prepare_chat_messages;

#[derive(Clone)]
pub struct RoleplayRuntimeClient {
    db: Database,
    client: Client<OpenAIConfig>,
    executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>,
    system_config: SystemConfig,
}

impl RoleplayRuntimeClient {
    pub async fn new(
        system_config: SystemConfig,
        executor: mpsc::Sender<(FunctionCall, oneshot::Sender<Result<String>>)>
    ) -> Self {
        let env = MongoDbEnv::load();
        let db = get_db(&env.get_env_var("MONGODB_URI"), "voda_is").await;

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

    fn get_price(&self) -> u64 { 1 }
    fn get_db(&self) -> &Database { &self.db }
    fn get_client(&self) -> &Client<OpenAIConfig> { &self.client }

    async fn on_init(&self) -> Result<()> {
        Ok(())
    }

    async fn on_shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn prepare_message(&self, 
        message: &HistoryMessage
    ) -> Result<Vec<ChatCompletionRequestMessage>> {
        prepare_chat_messages(
            &self.system_config,
            &self.character,
            &self.user,
            &self.memory.history,
            message,
        )
    }

    // async fn run(
    //     &self, 
    //     character: &Character, user: &mut User, system_config: &SystemConfig,
    //     memory: &mut ConversationMemory, message: &HistoryMessage
    // ) -> Result<HistoryMessage> {
    //     let is_new_conversation = memory.history.is_empty();

    //     // this will be used to send to OpenAI
    //     let chat_messages = prepare_chat_messages(
    //         system_config, 
    //         character, user, 
    //         &memory.history, message, 
    //         is_new_conversation
    //     )?;

    //     let (
    //         content, 
    //         usage, 
    //         maybe_function_call,
    //         maybe_results
    //     ) = self.send_request( system_config, chat_messages).await?;

    //     let response_message = HistoryMessage {
    //         owner: user.id.clone(),
    //         character_id: character.id.clone(),
    //         role: MessageRole::Assistant,
    //         content,
    //         content_type: MessageType::Text,
    //         function_call_request: maybe_function_call,
    //         function_call_response: maybe_results,
    //         created_at: get_current_timestamp(),
    //     };

    //     memory.history.push((message.clone(), response_message.clone()));
    //     user.add_usage(usage, system_config.openai_model.clone());

    //     Ok(response_message)
    // }

    // async fn regenerate(
    //     &self, 
    //     character: &Character, user: &mut User, system_config: &SystemConfig,
    //     memory: &mut ConversationMemory
    // ) -> Result<HistoryMessage> {
    //     let last_message = memory
    //         .history
    //         .pop()
    //         .ok_or(anyhow!("No history found"))?;
    //     let (new_message, _) = last_message;
    //     self.run(character, user, system_config, memory, &new_message).await
    // }

    // async fn find_system_config_by_character(
    //     &self, character: &Character
    // ) -> Result<SystemConfig> {
    //     let tags = character.tags.clone();
    //     let config_name = {
    //         if tags.contains(&"gitcoin".to_string()) {
    //             "gitcoin-screening".to_string()
    //         } else {
    //             match tags[0].as_str() {
    //                 "zh" => "roleplay-zh".to_string(),
    //                 "kr" => "roleplay-kr".to_string(),
    //                 _ => "roleplay".to_string(),
    //             }
    //         }
    //     };

    //     let config_hash = blake3_hash(config_name.as_bytes());

    //     let config = SystemConfig::select_one_by_index(&self.db, &config_hash).await?
    //         .ok_or(anyhow!("System config not found"))?;

    //     Ok(config)
    // }

    // async fn execute_function_call(
    //     &self, call: &FunctionCall
    // ) -> Result<String> {
    //     let (tx, rx) = oneshot::channel();
    //     self.executor.send((call.clone(), tx)).await?;
    //     let result = rx.await?;
    //     result
    // }
}
