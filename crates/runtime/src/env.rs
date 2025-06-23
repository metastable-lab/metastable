use std::env;

use voda_common::EnvVars;

pub struct RuntimeEnv {
    pub openai_api_key: String,
    pub openai_base_url: String,
}

impl EnvVars for RuntimeEnv {
    fn load() -> Self {
        Self {
            openai_api_key: env::var("OPENAI_API_KEY").unwrap(),
            openai_base_url: env::var("OPENAI_BASE_URL").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "OPENAI_API_KEY" => self.openai_api_key.clone(),
            "OPENAI_BASE_URL" => self.openai_base_url.clone(),
            _ => panic!("{} is not set", key),
        }
    }
}
