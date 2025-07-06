use std::env;

pub struct Mem0Env {
    pgvector_uri: String,
    graph_uri: String,
    graph_user: String,
    graph_password: String,

    embedding_base_url: String,
    embedding_api_key: String,
    embedding_embedding_model: String,

    openai_base_url: String,
    openai_api_key: String,
}

impl Mem0Env {
    pub fn load() -> Self {
        Self {
            pgvector_uri: env::var("PGVECTOR_URI").expect("PGVECTOR_URI is not set"),

            graph_uri: env::var("GRAPH_URI").expect("GRAPH_URI is not set"),
            graph_user: env::var("GRAPH_USER").expect("GRAPH_USER is not set"),
            graph_password: env::var("GRAPH_PASSWORD").expect("GRAPH_PASSWORD is not set"),

            embedding_base_url: env::var("EMBEDDING_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            embedding_api_key: env::var("EMBEDDING_API_KEY").unwrap_or_else(|_| "sk-test".to_string()),
            embedding_embedding_model: env::var("EMBEDDING_EMBEDDING_MODEL").unwrap_or_else(|_| "text-embedding-3-small".to_string()),

            openai_base_url: env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_else(|_| "sk-test".to_string()),
        }
    }

    pub fn get_env_var(&self, key: &str) -> String {
        match key {
            "PGVECTOR_URI" => self.pgvector_uri.clone(),
            "GRAPH_URI" => self.graph_uri.clone(),
            "GRAPH_USER" => self.graph_user.clone(),
            "GRAPH_PASSWORD" => self.graph_password.clone(),
            "EMBEDDING_BASE_URL" => self.embedding_base_url.clone(),
            "EMBEDDING_API_KEY" => self.embedding_api_key.clone(),
            "EMBEDDING_EMBEDDING_MODEL" => self.embedding_embedding_model.clone(),
            "OPENAI_BASE_URL" => self.openai_base_url.clone(),
            "OPENAI_API_KEY" => self.openai_api_key.clone(),
            _ => panic!("Unknown environment variable: {}", key),
        }
    }
}