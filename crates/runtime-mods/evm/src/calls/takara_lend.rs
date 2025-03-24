use alloy_core::primitives::{address, Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use anyhow::Result;

use super::RawTransaction;

const TAKARA_LEND_DELEGATOR: Address = address!("A26b9BFe606d29F16B5Aecf30F9233934452c4E2");

sol! {
    #[derive(Debug)]
    function mint(uint256 mintAmount) external returns (uint256);
}

pub fn lend_to_takara_lend(
    amount: U256
) -> Result<RawTransaction> {
    Ok(RawTransaction {
        to: TAKARA_LEND_DELEGATOR,
        value: U256::from(0),
        data: mintCall {
            mintAmount: amount,
        }.abi_encode().into(),
    })
}
