use alloy_core::primitives::{bytes, Address, Bytes, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use alloy_rpc_types::TransactionRequest;
use anyhow::Result;
use voda_common::CryptoHash;

use crate::client::send_transaction;
use crate::wallet::LocalWallet;

sol! {
    #[derive(Debug)]
    function allocate(uint256[] _poolId, uint256[] _amount, bytes[] memory _data) external payable;
}

fn pad_to_32(data: &[u8]) -> Vec<u8> {
    let mut padded = vec![0; 32];
    padded[32 - data.len()..].copy_from_slice(data);
    padded
}

fn build_data(recepient_id: Address, amount: U256) -> Vec<u8> {

    let mut data = vec![];
    data.extend_from_slice(&pad_to_32(&recepient_id.into_array()));
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000000"));
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000060")); 
    data.extend_from_slice(&bytes!("000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000000"));
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000000"));
    data.extend_from_slice(&bytes!("00000000000000000000000000000000000000000000000000000000000000a0"));
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000020"));
    data.extend_from_slice(&bytes!("0000000000000000000000000000000000000000000000000000000000000000"));

    data
}

pub async fn send_donation(
    pk: &[u8; 32], gitcoin_address: Address, recepient_id: Address, amount: U256
) -> Result<CryptoHash> {
    let local_wallet = LocalWallet::from_private_key(pk);
    let data = build_data(recepient_id, amount);
    let calldata = allocateCall {
        _poolId: vec![U256::from(21)],
        _amount: vec![amount],
        _data: vec![Bytes::from(data)],
    };

    let tx = TransactionRequest::default()
        .to(gitcoin_address)
        .value(amount)
        .input(calldata.abi_encode().into());

    let tx_hash = send_transaction(tx, &local_wallet).await.unwrap();
    Ok(tx_hash)
}

