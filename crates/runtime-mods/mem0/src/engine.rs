use std::sync::Arc;
use anyhow::Result;

use async_openai::types::CreateEmbeddingRequestArgs;
use async_openai::{config::OpenAIConfig, Client};

use metastable_runtime::user::UserUsagePoints;
use neo4rs::{ConfigBuilder, Graph};
use sqlx::PgPool;

use metastable_runtime::{toolcalls, LLMRunResponse, UserUsage};
use metastable_database::SqlxCrud;

use crate::pgvector::BatchUpdateSummary;
use crate::{Embedding, EntityTag, EmbeddingMessage};
use crate::llm::{DeleteGraphMemoryToolcall, EntitiesToolcall, FactsToolcall, MemoryUpdateToolcall, RelationshipsToolcall};

toolcalls!(
    ctx: Mem0Engine,
    tools: [
        (DeleteGraphMemoryToolcall, "delete_graph_memory", usize),
        (EntitiesToolcall, "extract_entities", Vec<EntityTag>),
        (FactsToolcall, "extract_facts", Vec<EmbeddingMessage>),
        (MemoryUpdateToolcall, "update_memory", BatchUpdateSummary),
        (RelationshipsToolcall, "establish_relationships", usize),
    ]
);

#[derive(Clone)]
pub struct Mem0Engine {
    data_db: Arc<PgPool>,
    vector_db: Arc<PgPool>,
    graph_db: Arc<Graph>,

    embeder: Client<OpenAIConfig>,
    llm: Client<OpenAIConfig>,
}

impl Mem0Engine {
    pub async fn new(data_db: Arc<PgPool>, pgvector_db: Arc<PgPool>) -> Result<Self> {
        let env = crate::env::Mem0Env::load();

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
            data_db,
            vector_db: pgvector_db, 
            graph_db: Arc::new(graph_db), 
            embeder, llm 
        })
    }

    pub async fn init(&self) -> Result<()> {
        // self.vector_db_initialize().await?;
        self.graph_db_initialize().await?;
        Ok(())
    }

    pub async fn embed(&self, text: Vec<String>) -> Result<Vec<Embedding>> {
        tracing::debug!("[Mem0Engine::embed] Embedding text: {:?}", text);
        if text.is_empty() {
            return Ok(vec![]);
        }

        let request = CreateEmbeddingRequestArgs::default()
            .model(crate::EMBEDDING_MODEL)
            .input(text)
            .build()?;

        let response = self.embeder.embeddings().create(request).await?;
        let embeddings = response.data
            .into_iter()
            .map(|item| item.embedding)
            .collect::<Vec<_>>();

        tracing::debug!("[Mem0Engine::embed] Embedding response: {}", embeddings.len());

        Ok(embeddings)
    }

    pub async fn add_usage_report(&self, response: &LLMRunResponse) -> Result<()> {
        let mut tx = self.data_db.begin().await?;
        let usage = UserUsage::from_llm_response(response, UserUsagePoints::default());
        usage.create(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub fn get_data_db(&self) -> &Arc<PgPool> {
        &self.data_db
    }

    pub fn get_vector_db(&self) -> &Arc<PgPool> {
        &self.vector_db
    }

    pub fn get_graph_db(&self) -> &Arc<Graph> {
        &self.graph_db
    }

    pub fn get_llm(&self) -> &Client<OpenAIConfig> {
        &self.llm
    }
}
