pub mod pgvector;
pub mod llm;
pub mod env;

pub use pgvector::{
    PgVector,
    Vector,
    EmbeddingData,
    EmbeddingUpdate,
    SearchResult,
};

use std::sync::Arc;
use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, 
    ChatCompletionToolArgs, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use voda_runtime::{define_function_types, ExecutableFunctionCall};

pub type Embedding = Vec<f32>;
const EMBEDDING_DIMS: i32 = 1024;

define_function_types!(
    FactsToolcall(crate::llm::FactsToolcall, "extract_facts"),
    MemoryToolcall(crate::llm::MemoryToolcall, "extract_memory")
);

pub struct PgVectorDatabase {
    db: Arc<PgPool>,
    embeder: Client<OpenAIConfig>,
    llm: Client<OpenAIConfig>,
}

impl PgVectorDatabase {
    pub async fn new() -> Result<Self> {
        let env = crate::env::PgVectorEnv::load();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&env.get_env_var("PGVECTOR_URI"))
            .await?;

        let embeder_config = OpenAIConfig::new()
            .with_api_base(env.get_env_var("EMBEDDING_BASE_URL"))
            .with_api_key(env.get_env_var("EMBEDDING_API_KEY"));

        let llm_config = OpenAIConfig::new()
            .with_api_base(env.get_env_var("OPENAI_BASE_URL"))
            .with_api_key(env.get_env_var("OPENAI_API_KEY"));

        let embeder = Client::build(
            reqwest::Client::new(),
            embeder_config,
            Default::default()
        );
        let llm = Client::build(    
            reqwest::Client::new(),
            llm_config,
            Default::default()
        );

        Ok(Self { db: Arc::new(pool), embeder, llm })
    }

    pub async fn init(&self) -> Result<()> {
        let ddl1 = "CREATE EXTENSION IF NOT EXISTS vector;";
        sqlx::query(ddl1).execute(&*self.db).await?;

        let ddl2 = "CREATE TABLE IF NOT EXISTS embeddings (
            id SERIAL PRIMARY KEY,
            content TEXT NOT NULL,
            embedding vector(1024),
            user_id TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );";
        sqlx::query(ddl2).execute(&*self.db).await?;

        let ddl3 = "CREATE INDEX IF NOT EXISTS idx_embeddings_user_id ON embeddings(user_id);";
        sqlx::query(ddl3).execute(&*self.db).await?;

        Ok(())
    }

    pub async fn embed(&self, text: Vec<String>) -> Result<Vec<Embedding>> {
        let env = crate::env::PgVectorEnv::load();
        let response = self.embeder.embeddings().create(
            CreateEmbeddingRequestArgs::default()
                .model(&env.get_env_var("EMBEDDING_EMBEDDING_MODEL"))
                .input(text)
                .build()?
        ).await?;
        let embeddings = response.data
            .into_iter()
            .map(|item| item.embedding)
            .collect();

        Ok(embeddings)
    }

    pub async fn llm(&self, config: &crate::llm::LlmConfig, user_message: &str) -> Result<String> {
        let messages = [
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(config.system_prompt.clone())
                    .build()?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_message.to_string())
                    .build()?
            ),
        ];

        let tools = config.tools.iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("Message should build")
            )
            .collect::<Vec<_>>();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&config.model)
            .messages(messages)
            .tools(tools)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens as u32)
            .tool_choice(ChatCompletionToolChoiceOption::Auto)
            .build()?;

        let response = self.llm.chat().create(request).await?;
        let content = response.choices.first().unwrap().message.content.clone().unwrap_or_default();

        if let Some(tool_calls) = response.choices.first().unwrap().message.tool_calls.clone() {
            for tool_call in tool_calls {
                let tc = RuntimeFunctionType::from_function_call(tool_call.function.clone())?;
                println!("tc: {:#?}", tc);
                let result = tc.execute().await?;
                return Ok(result);
            }
        }

        Ok(content)
    }
}
