use alloy_core::primitives::{address, Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use alloy_rpc_types::TransactionRequest;
use anyhow::Result;
use voda_common::CryptoHash;

use crate::client::send_transaction_with_retry;
use crate::wallet::LocalWallet;

const TAKARA_LEND_DELEGATOR: Address = address!("A26b9BFe606d29F16B5Aecf30F9233934452c4E2");

sol! {
    #[derive(Debug)]
    function mint(uint256 mintAmount) external returns (uint256);
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
