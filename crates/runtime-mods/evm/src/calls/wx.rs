use alloy_core::primitives::{Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use alloy_rpc_types::TransactionRequest;
use anyhow::Result;
use voda_common::CryptoHash;

use crate::client::send_transaction_with_retry;
use crate::wallet::LocalWallet;

sol! {
    #[derive(Debug)]
    function deposit() public payable;

    #[derive(Debug)]
    function withdraw(uint wad) public;
}

pub async fn deposit(
    pk: &[u8; 32], address: Address, amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(address)
        .value(amount)
        .input(depositCall {}.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}

pub async fn withdraw(
    pk: &[u8; 32], address: Address, amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(address)
        .value(U256::from(0))
        .input(withdrawCall {
            wad: amount,
        }.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}
