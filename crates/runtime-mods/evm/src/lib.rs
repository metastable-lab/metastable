mod wallet;
mod env;
mod client;

mod function_calls;
mod calls;

use alloy_core::primitives::U256;
pub use env::EvmEnv;
pub use client::{send_transaction, get_code, eth_call, get_balance, transfer, send_transaction_with_retry};
pub use wallet::LocalWallet;
pub use calls::gitcoin::send_donation;
pub use calls::takara_lend::{deposit, withdraw, approve_to_takara_lend, lend_to_takara_lend};

pub use function_calls::gitcoin::{GitcoinEnv, GitcoinFunctionCall};

pub fn to_wei(amount: u64) -> U256 {
    U256::from(amount) * U256::from(10u64.pow(18))
}

pub fn to_wei_with_gas(amount: u64) -> U256 {
    U256::from(amount) * U256::from(10u64.pow(18)) + U256::from(10u64.pow(16))
}