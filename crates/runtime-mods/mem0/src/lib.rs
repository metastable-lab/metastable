mod pgvector;
mod agents;
mod engine;

pub use pgvector::EmbeddingMessage;
use anyhow::Result;

use metastable_clients::{EmbederClient, LlmClient, PgvectorClient, PostgresClient};
use metastable_common::ModuleClient;

#[cfg(feature = "graph")]
use metastable_clients::GraphClient;

#[derive(Clone)]
pub struct Mem0Engine {
    pub(crate) data_db: PostgresClient,
    pub(crate) vector_db: PgvectorClient,
    pub(crate) llm: LlmClient,
    #[cfg(feature = "graph")]
    pub(crate) graph_db: GraphClient,
    pub(crate) embeder: EmbederClient,
}

impl Mem0Engine {
    pub async fn new() -> Result<Self> {
        let data_db = PostgresClient::setup_connection().await;
        let vector_db = PgvectorClient::setup_connection().await;
        #[cfg(feature = "graph")]
        let graph_db = GraphClient::setup_connection().await;
        let embeder = EmbederClient::setup_connection().await;
        let llm = LlmClient::setup_connection().await;

        Ok(Self {  data_db, vector_db,  #[cfg(feature = "graph")] graph_db, embeder, llm })
    }

    pub async fn init(&self) -> Result<()> {
        #[cfg(feature = "graph")]
        self.graph_db.initialize().await?;
        Ok(())
    }
}

#[macro_export]
macro_rules! init_mem0 {
    () => {
        static MEM0_ENGINE: tokio::sync::OnceCell<Mem0Engine> = tokio::sync::OnceCell::const_new();

        async fn get_mem0_engine() -> &'static Mem0Engine {
            MEM0_ENGINE
                .get_or_init(|| async {
                    let mem0_engine = Mem0Engine::new().await.expect("Failed to initialize Mem0Engine");
                    mem0_engine.init().await.expect("Failed to initialize Mem0Engine");
                    mem0_engine
                })
                .await
        }
    };
}