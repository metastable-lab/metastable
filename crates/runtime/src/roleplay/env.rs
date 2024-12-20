use std::env;

pub struct RoleplayEnv {
    pub openai_api_key: String,
    pub openai_base_url: String,
}

impl RoleplayEnv {
    pub fn load() -> Self {
        Self {
            openai_api_key: env::var("OPENAI_API_KEY").unwrap(),
            openai_base_url: env::var("OPENAI_BASE_URL").unwrap(),
        }
    }
}
