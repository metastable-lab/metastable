use alloy_core::primitives::{address, Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use alloy_rpc_types::TransactionRequest;
use anyhow::Result;
use voda_common::CryptoHash;

use crate::client::send_transaction_with_retry;
use crate::wallet::LocalWallet;

const WSEI_ADDRESS: Address = address!("E30feDd158A2e3b13e9badaeABaFc5516e95e8C7");
const TAKARA_LEND_DELEGATOR: Address = address!("A26b9BFe606d29F16B5Aecf30F9233934452c4E2");

sol! {

    #[derive(Debug)]
    function deposit() public payable;

    #[derive(Debug)]
    function withdraw(uint wad) public;

    #[derive(Debug)]
    function approve(address guy, uint wad) public returns (bool);

    #[derive(Debug)]
    function balanceOf() public returns (uint256);

    #[derive(Debug)]
    function mint(uint256 mintAmount) external returns (uint256);
}

pub async fn deposit(
    pk: &[u8; 32], amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(WSEI_ADDRESS)
        .value(amount)
        .input(depositCall {}.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}

pub async fn withdraw(
    pk: &[u8; 32], amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(WSEI_ADDRESS)
        .value(U256::from(0))
        .input(withdrawCall {
            wad: amount,
        }.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}

pub async fn approve_to_takara_lend(
    pk: &[u8; 32], amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(WSEI_ADDRESS)
        .value(U256::from(0))
        .input(approveCall {
            guy: TAKARA_LEND_DELEGATOR,
            wad: amount,
        }.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}

pub async fn lend_to_takara_lend(
    pk: &[u8; 32], amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let tx = TransactionRequest::default()
        .to(TAKARA_LEND_DELEGATOR)
        .value(U256::from(0))
        .input(mintCall {
            mintAmount: amount,
        }.abi_encode().into());

    let tx_hash = send_transaction_with_retry(tx, &local_wallet).await?;
    Ok(tx_hash)
}
