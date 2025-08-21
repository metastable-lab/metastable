use anyhow::Result;
use metastable_clients::PostgresClient;
use metastable_common::ModuleClient;
use reqwest::Client;
use metastable_runtime::{define_agent_router, AgentRouter};
use metastable_runtime_roleplay::agents::{
    RoleplayV1Agent,
    RoleplayCharacterCreationV1Agent,
    CharacterCreationAgent,
};
use sqlx::types::Uuid;
use tokio::sync::mpsc;

define_agent_router! {
    RoleplayV1 as roleplay_v1 (RoleplayV1Agent),
    RoleplayCharacterCreationV1 as roleplay_character_creation_v1 (RoleplayCharacterCreationV1Agent),
    CharacterCreation as character_creation (CharacterCreationAgent),
}

#[derive(Clone)]
pub struct GlobalState {
    pub db: PostgresClient,
    pub agents_router: AgentsRouter,
    pub http_client: Client,
    pub memory_update_tx: mpsc::Sender<Uuid>,
}

impl GlobalState {
    pub async fn new() -> Result<(Self, mpsc::Receiver<Uuid>)> {
        let db = PostgresClient::setup_connection().await;
        let agents_router = AgentsRouter::new().await?;
        let http_client = Client::new();
        let (memory_update_tx, memory_update_rx) = mpsc::channel(50);
        Ok((Self { db, agents_router, http_client, memory_update_tx }, memory_update_rx))
    }
}