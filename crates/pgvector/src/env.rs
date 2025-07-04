use std::env;

pub struct PgVectorEnv {
    pgvector_uri: String,
    pgvector_user: String,
    pgvector_password: String,
    embedding_base_url: String,
    embedding_api_key: String,
    embedding_embedding_model: String,
    openai_base_url: String,
    openai_api_key: String,
}

impl PgVectorEnv {
    pub fn load() -> Self {
        Self {
            pgvector_uri: env::var("PGVECTOR_URI").unwrap_or_else(|_| "postgresql://localhost:5432/pgvector".to_string()),
            pgvector_user: env::var("PGVECTOR_USER").unwrap_or_else(|_| "postgres".to_string()),
            pgvector_password: env::var("PGVECTOR_PASSWORD").unwrap_or_else(|_| "password".to_string()),
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
            "PGVECTOR_USER" => self.pgvector_user.clone(),
            "PGVECTOR_PASSWORD" => self.pgvector_password.clone(),
            "EMBEDDING_BASE_URL" => self.embedding_base_url.clone(),
            "EMBEDDING_API_KEY" => self.embedding_api_key.clone(),
            "EMBEDDING_EMBEDDING_MODEL" => self.embedding_embedding_model.clone(),
            "OPENAI_BASE_URL" => self.openai_base_url.clone(),
            "OPENAI_API_KEY" => self.openai_api_key.clone(),
            _ => panic!("Unknown environment variable: {}", key),
        }
    }
}