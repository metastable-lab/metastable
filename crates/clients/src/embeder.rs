use std::env;

use anyhow::Result;
use metastable_common::{define_module_client, ModuleClient};

use async_openai::{
    config::OpenAIConfig, 
    types::CreateEmbeddingRequestArgs, 
    Client
};

use crate::{
    Embedding, EMBEDDING_MODEL
};

define_module_client! {
    (struct EmbederClient, "embeder")
    client_type: Client<OpenAIConfig>,
    env: ["EMBEDDING_BASE_URL", "EMBEDDING_API_KEY"],
    setup: async {
        let base_url = env::var("EMBEDDING_BASE_URL").expect("EMBEDDING_BASE_URL is not set");
        let api_key = env::var("EMBEDDING_API_KEY").expect("EMBEDDING_API_KEY is not set");
        let embeder_config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key);

        Client::build(
            reqwest::Client::new(),
            embeder_config,
            Default::default()
        )
    }
}

impl EmbederClient {
    pub async fn embed(&self, text: Vec<String>) -> Result<Vec<Embedding>> {
        tracing::debug!("[EmbederClient::embed] Embedding text: {:?}", text);
        if text.is_empty() {
            return Ok(vec![]);
        }

        let request = CreateEmbeddingRequestArgs::default()
            .model(EMBEDDING_MODEL)
            .input(text)
            .build()?;

        let response = self.get_client().embeddings().create(request).await?;
        let embeddings = response.data
            .into_iter()
            .map(|item| item.embedding)
            .collect::<Vec<_>>();

        tracing::debug!("[EmbedderClient::embed] Embedding response: {}", embeddings.len());

        Ok(embeddings)
    }
}   