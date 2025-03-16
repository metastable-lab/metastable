use std::env;

use voda_common::EnvVars;

pub struct EvmEnv {
    pub eth_rpc_url: String,
}

impl EnvVars for EvmEnv {
    fn load() -> Self {
        Self {
            eth_rpc_url: env::var("ETH_RPC_URL").unwrap(),
        }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "ETH_RPC_URL" => self.eth_rpc_url.clone(),
            _ => panic!("Invalid environment variable: {}", key),
        }
    }
}