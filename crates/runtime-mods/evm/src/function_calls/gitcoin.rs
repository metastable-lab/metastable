use std::{env, str::FromStr};

use alloy_core::primitives::Address;
use anyhow::Result;
use async_openai::types::FunctionCall;
use voda_common::{blake3_hash, EnvVars};
use serde::{Deserialize, Serialize};
use voda_runtime::ExecutableFunctionCall;

use crate::addresses::sei::GITCOIN_ADDRESS;
use crate::{send_transaction, to_wei, LocalWallet};
use crate::calls::gitcoin::send_donation;

pub struct GitcoinEnv {
    pub private_key_salt: String,
}

impl EnvVars for GitcoinEnv {
    fn load() -> Self {
        Self { private_key_salt: env::var("GITCOIN_PRIVATE_KEY_SALT").unwrap() }
    }

    fn get_env_var(&self, key: &str) -> String {
        match key {
            "GITCOIN_PRIVATE_KEY_SALT" => self.private_key_salt.clone(),
            _ => panic!("Invalid environment variable: {}", key),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitcoinFunctionCall {
    pub name: String,
    #[serde(rename = "recepientId")]
    pub recepient_id: String,
    pub reasoning: String,
}

impl GitcoinFunctionCall {
    pub fn new(name: String, recepient_id: String, reasoning: String) -> Self {
        Self { name, recepient_id, reasoning }
    }
}

impl ExecutableFunctionCall for GitcoinFunctionCall {
    fn name() -> &'static str {
        "gitcoin_allocate_grant"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        let env = GitcoinEnv::load();
        let pk_salt = env.get_env_var("GITCOIN_PRIVATE_KEY_SALT");
        let pk = blake3_hash(pk_salt.as_bytes());
        let local_wallet = LocalWallet::from_private_key(&pk.hash());
        
        let recepient_id = Address::from_str(&self.recepient_id)?;
        let tx = send_donation(GITCOIN_ADDRESS, recepient_id, to_wei(100)).await?;
        let tx_hash = send_transaction(tx, &local_wallet).await?;
        Ok(tx_hash.to_string())
    }
}
