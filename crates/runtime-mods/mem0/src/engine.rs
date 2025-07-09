use std::sync::Arc;
use anyhow::{anyhow, Result};

use async_openai::{config::OpenAIConfig, Client};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, 
    ChatCompletionToolArgs, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs
};
use neo4rs::{ConfigBuilder, Graph};
use sqlx::{PgPool, postgres::PgPoolOptions};

use voda_runtime::{define_function_types, ExecutableFunctionCall, LLMRunResponse};

use crate::llm::LlmConfig;
use crate::Embedding;

define_function_types!(
    DeleteGraphMemoryToolcall(crate::llm::DeleteGraphMemoryToolcall, "delete_graph_memory"),
    EntitiesToolcall(crate::llm::EntitiesToolcall, "extract_entities"),
    FactsToolcall(crate::llm::FactsToolcall, "extract_facts"),
    MemoryUpdateToolcall(crate::llm::MemoryUpdateToolcall, "update_memory"),
    RelationshipsToolcall(crate::llm::RelationshipsToolcall, "establish_relationships"),
);

#[derive(Clone)]
pub struct Mem0Engine {
    vector_db: Arc<PgPool>,
    graph_db: Arc<Graph>,

    embeder: Client<OpenAIConfig>,
    llm: Client<OpenAIConfig>,
}

impl Mem0Engine {
    pub async fn new() -> Result<Self> {
        let env = crate::env::Mem0Env::load();
        let vector_db = PgPoolOptions::new()
            .connect(&env.get_env_var("PGVECTOR_URI"))
            .await
            .expect("[Mem0Engine::new] Failed to connect to vector db");

        let graph_config = ConfigBuilder::default()
            .uri(env.get_env_var("GRAPH_URI"))
            .user(env.get_env_var("GRAPH_USER"))
            .password(env.get_env_var("GRAPH_PASSWORD"))
            .db("neo4j")
            .build()
            .expect("[Mem0Engine::new] Failed to build graph config");

        let graph_db = Graph::connect(graph_config).await
            .expect("[Mem0Engine::new] Failed to connect to graph");

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

        Ok(Self { 
            vector_db: Arc::new(vector_db), 
            graph_db: Arc::new(graph_db), 
            embeder, llm 
        })
    }

    pub async fn init(&self) -> Result<()> {
        self.vector_db_initialize().await?;
        self.graph_db_initialize().await?;
        Ok(())
    }

    pub async fn embed(&self, text: Vec<String>) -> Result<Vec<Embedding>> {
        let request = CreateEmbeddingRequestArgs::default()
        .model(crate::EMBEDDING_MODEL)
        .input(text)
        .build()?;

        let response = self.embeder.embeddings().create(request).await?;
        let embeddings = response.data
            .into_iter()
            .map(|item| item.embedding)
            .collect();

        Ok(embeddings)
    }

    pub async fn llm(&self, config: &LlmConfig, user_message: String) -> Result<LLMRunResponse> {
        let messages = [
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(config.system_prompt.clone())
                    .build()
                    .expect("[Mem0Engine::llm] System message should build")
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_message)
                    .build()?
            ),
        ];

        let tools = config.tools.iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("[Mem0Engine::llm] Tool should build")
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
        let usage = response.usage.ok_or(|| {
            tracing::warn!("Model {} returned no usage", config.model);
        }).map_err(|_| anyhow!("Model {} returned no usage", config.model))?;

        let response = response.choices.first()
            .ok_or(anyhow!("[Mem0Engine::llm] No response from AI inference server"))?
            .message.clone();
        let content = response.content.unwrap_or_default();
        let maybe_function_call = response.tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| tool_call.function)
            .collect::<Vec<_>>();

        let mut maybe_results = Vec::new();
        for tool_call in maybe_function_call.clone() {
            println!("tool_call: {:?}", tool_call);
            let tc = RuntimeFunctionType::from_function_call(tool_call.clone())?;
        
            let result = tc.execute().await?;
            maybe_results.push(result);
        }

        Ok(LLMRunResponse {
            content,
            usage,
            maybe_function_call,
            maybe_results,
        })
    }

    pub fn get_vector_db(&self) -> &Arc<PgPool> {
        &self.vector_db
    }

    pub fn get_graph_db(&self) -> &Arc<Graph> {
        &self.graph_db
    }
}
