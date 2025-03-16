use std::env;

use voda_common::EnvVars;

pub struct MongoDbEnv {
    pub mongodb_uri: String,
}

impl EnvVars for MongoDbEnv {
    fn load() -> Self {
        Self {
            mongodb_uri: env::var("MONGODB_URI").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "MONGODB_URI" => self.mongodb_uri.clone(),
            _ => panic!("Invalid environment variable: {}", key),
        }
    }
}
