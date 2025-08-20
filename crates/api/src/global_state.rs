use metastable_clients::PostgresClient;
use reqwest::Client;
use metastable_runtime::{define_agent_router, AgentRouter};
use metastable_runtime_roleplay::agents::{
    RoleplayV1Agent,
    RoleplayCharacterCreationV1Agent,
    CharacterCreationAgent,
};

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
}