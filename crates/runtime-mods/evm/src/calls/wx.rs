use alloy_core::primitives::{Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use anyhow::Result;

use super::RawTransaction;

sol! {
    #[derive(Debug)]
    function deposit() public payable;

    #[derive(Debug)]
    function withdraw(uint wad) public;
}

pub fn deposit(
    address: Address, amount: U256
) -> Result<RawTransaction> {
    Ok(RawTransaction {
        to: address,
        value: amount,
        data: depositCall {}.abi_encode().into(),
    })
}

pub fn withdraw(
    address: Address, amount: U256
) -> Result<RawTransaction> {
    Ok(RawTransaction {
        to: address,
        value: U256::from(0),
        data: withdrawCall {
            wad: amount,
        }.abi_encode().into(),
    })
}
