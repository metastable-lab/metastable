use voda_common::{CryptoHash, EnvVars};

use alloy_core::primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use anyhow::Result;

use crate::{env::EvmEnv, wallet::LocalWallet};

pub async fn get_code(address: Address) -> Result<Vec<u8>> {
    let env = EvmEnv::load();
    let provider = ProviderBuilder::new()
        .on_http(env.get_env_var("ETH_RPC_URL").parse()?);
    let code = provider.get_code_at(address).await?;
    Ok(code.into())
}

pub async fn send_transaction(tx: TransactionRequest, wallet: &LocalWallet) -> Result<CryptoHash> {
    let env = EvmEnv::load();
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet.into_alloy_wallet())
        .on_http(env.get_env_var("ETH_RPC_URL").parse()?);

    let tx_hash = provider.send_transaction(tx).await?.watch().await?;
    Ok(CryptoHash::new(tx_hash.into()))
}

// send transaction with retry 
pub async fn send_transaction_with_retry(tx: TransactionRequest, wallet: &LocalWallet) -> Result<CryptoHash> {
    for _ in 0..50 {
        let tx_hash = send_transaction(tx.clone(), wallet).await;
        if tx_hash.is_ok() {
            return tx_hash;
        }

        println!("Failed to send transaction, retrying...");
    }
    Err(anyhow::anyhow!("Failed to send transaction"))
}

pub async fn eth_call(tx: TransactionRequest) -> Result<Vec<u8>> {
    let env = EvmEnv::load();
    let provider = ProviderBuilder::new()
        .on_http(env.get_env_var("ETH_RPC_URL").parse()?);
    let result = provider.call(&tx).await?;
    Ok(result.into())
}

pub async fn get_balance(address: Address) -> Result<u64> {
    let env = EvmEnv::load();
    let provider = ProviderBuilder::new()
        .on_http(env.get_env_var("ETH_RPC_URL").parse()?);
    let balance = provider.get_balance(address).await?;
    let gwei_balance = balance
        .checked_div(U256::from(10u64.pow(9)))
        .ok_or_else(|| anyhow::anyhow!("Balance too large to convert to Gwei"))?;
    
    if gwei_balance > U256::from(u64::MAX) {
        return Err(anyhow::anyhow!("Balance exceeds u64::MAX in Gwei"));
    }
    
    // SAFETY: We've already checked that the balance is less than u64::MAX
    Ok(gwei_balance.try_into().unwrap())
}

pub async fn transfer(pk: &[u8; 32], to: Address, amount: U256) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(to)
        .value(amount);

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}