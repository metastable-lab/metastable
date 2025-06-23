use std::env;

use voda_common::EnvVars;


#[cfg(feature = "mongodb")]
pub struct MongoDbEnv {
    pub mongodb_uri: String,
}

#[cfg(feature = "mongodb")]
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

#[cfg(feature = "postgres")]
pub struct PostgresDbEnv {
    pub postgres_uri: String,
}

#[cfg(feature = "postgres")]
impl EnvVars for PostgresDbEnv {
    fn load() -> Self {
        Self {
            postgres_uri: env::var("DATABASE_URL").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "POSTGRES_URI" => self.postgres_uri.clone(),
            _ => panic!("Invalid environment variable: {}", key),
        }
    }
}