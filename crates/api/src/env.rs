use voda_common::EnvVars;

pub struct ApiServerEnv {
    pub secret_salt: String,
    pub fish_audio_api_key: String,
}

impl EnvVars for ApiServerEnv {
    fn load() -> Self {
        Self {
            secret_salt: std::env::var("SECRET_SALT").unwrap(),
            fish_audio_api_key: std::env::var("FISH_AUDIO_API_KEY").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "SECRET_SALT" => self.secret_salt.clone(),
            "FISH_AUDIO_API_KEY" => self.fish_audio_api_key.clone(),
            _ => panic!("{} is not set", key),
        }
    }
}
