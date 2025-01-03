pub struct EnvVars {
    pub secret_salt: String,
}

impl EnvVars {
    pub fn load() -> Self {
        Self {
            secret_salt: std::env::var("SECRET_SALT").unwrap(),
        }
    }
}
