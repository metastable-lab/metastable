use alloy_core::primitives::U256;
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use anyhow::Result;

use crate::addresses::MULTICALL_ADDRESS;

use super::RawTransaction;

sol! {
    #[derive(Debug)]
    struct Call3Value {
        address target;
        bool allowFailure;
        uint256 value;
        bytes callData;
    }

    #[derive(Debug)]
    struct Aggregate3ValueResult {
        bool success;
        bytes returnData;
    }

    #[derive(Debug)]
    function aggregate3Value(Call3Value[] calldata calls) public payable returns (Aggregate3ValueResult[] memory returnData);
}

/// Execute multiple calls in a single transaction using Multicall
pub fn multicall(
    transactions: Vec<RawTransaction>,
) -> Result<RawTransaction> {
    // Convert RawTransactions to Multicall Call structs
    let calls: Vec<Call3Value> = transactions
        .into_iter()
        .map(|tx| Call3Value {
            target: tx.to,
            allowFailure: false,
            value: tx.value,
            callData: tx.data,
        })
        .collect();
    
    // Calculate the total value to send with the multicall
    let total_value: U256 = calls.iter().map(|call| call.value).sum();
    

    Ok(RawTransaction {
        to: MULTICALL_ADDRESS,
        value: total_value,
        data: aggregate3ValueCall { calls }.abi_encode().into(),
    })
}
