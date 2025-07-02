use voda_common::EnvVars;

pub struct GraphEnv {
    pub uri: String,
    pub user: String,
    pub password: String,

    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_embedding_model: String,
}

impl EnvVars for GraphEnv {
    fn load() -> Self {
        Self {
            uri: std::env::var("GRAPH_URI").unwrap(),
            user: std::env::var("GRAPH_USER").unwrap(),
            password: std::env::var("GRAPH_PASSWORD").unwrap(),

            openai_api_key: std::env::var("EMBEDDING_API_KEY").unwrap(),
            openai_base_url: std::env::var("EMBEDDING_BASE_URL").unwrap(),
            openai_embedding_model: std::env::var("EMBEDDING_EMBEDDING_MODEL").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "GRAPH_URI" => self.uri.clone(),
            "GRAPH_USER" => self.user.clone(),
            "GRAPH_PASSWORD" => self.password.clone(),

            "OPENAI_API_KEY" => self.openai_api_key.clone(),
            "OPENAI_BASE_URL" => self.openai_base_url.clone(),
            "OPENAI_EMBEDDING_MODEL" => self.openai_embedding_model.clone(),

            _ => panic!("{} is not set", key),
        }
    }
}