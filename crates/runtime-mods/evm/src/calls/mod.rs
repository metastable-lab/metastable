use alloy_core::primitives::{Address, Bytes, U256};

pub mod gitcoin;
pub mod takara_lend;
pub mod wx;
pub mod erc20;
pub mod swap;
pub mod multicall;

#[derive(Debug, Clone)]
pub struct RawTransaction {
    /// The target contract address
    pub to: Address,
    /// The ETH value to send with the call
    pub value: U256,
    /// The calldata for the transaction
    pub data: Bytes,
}
