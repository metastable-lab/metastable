use reqwest::Client;
use metastable_runtime_character_creation::CharacterCreationRuntimeClient;
use metastable_runtime_roleplay::RoleplayRuntimeClient;

#[derive(Clone)]
pub struct GlobalState {
    pub roleplay_client: RoleplayRuntimeClient,
    pub character_creation_client: CharacterCreationRuntimeClient,
    pub http_client: Client,
}