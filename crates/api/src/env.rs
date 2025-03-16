use voda_common::EnvVars;

pub struct ApiServerEnv {
    pub secret_salt: String,
}

impl EnvVars for ApiServerEnv {
    fn load() -> Self {
        Self {
            secret_salt: std::env::var("SECRET_SALT").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "SECRET_SALT" => self.secret_salt.clone(),
            _ => panic!("{} is not set", key),
        }
    }
}
