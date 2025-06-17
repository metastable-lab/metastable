use reqwest::Client;
use voda_runtime_roleplay::RoleplayRuntimeClient;

#[derive(Clone)]
pub struct GlobalState {
    pub roleplay_client: RoleplayRuntimeClient,
    pub http_client: Client,
}