use metastable_common::EnvVars;

pub struct ApiServerEnv {
    pub secret_salt: String,
    pub fish_audio_api_key: String,
    pub hasura_graphql_url: String,
    pub hasura_graphql_admin_secret: String,
    pub otp_secret_key: String,
    pub maileroo_api_key: String,
}

impl EnvVars for ApiServerEnv {
    fn load() -> Self {
        Self {
            secret_salt: std::env::var("SECRET_SALT").unwrap(),
            fish_audio_api_key: std::env::var("FISH_AUDIO_API_KEY").unwrap(),
            hasura_graphql_url: std::env::var("HASURA_GRAPHQL_URL").unwrap(),
            hasura_graphql_admin_secret: std::env::var("HASURA_GRAPHQL_ADMIN_SECRET").unwrap(),
            otp_secret_key: std::env::var("OTP_SECRET_KEY").unwrap(),
            maileroo_api_key: std::env::var("MAILEROO_API_KEY").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "SECRET_SALT" => self.secret_salt.clone(),
            "FISH_AUDIO_API_KEY" => self.fish_audio_api_key.clone(),
            "HASURA_GRAPHQL_URL" => self.hasura_graphql_url.clone(),
            "HASURA_GRAPHQL_ADMIN_SECRET" => self.hasura_graphql_admin_secret.clone(),
            "OTP_SECRET_KEY" => self.otp_secret_key.clone(),
            "MAILEROO_API_KEY" => self.maileroo_api_key.clone(),
            _ => panic!("{} is not set", key),
        }
    }
}
