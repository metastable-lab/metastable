use alloy_core::primitives::{address, Address, U256};
use alloy_core::sol;
use alloy_core::sol_types::SolCall;
use anyhow::Result;

use metastable_common::get_current_timestamp;

use crate::eth_call;

use super::{erc20, RawTransaction};

const ROUTER: Address = address!("AAA45c8F5ef92a000a121d102F4e89278a711Faa");

sol! {
    #[derive(Debug)]
    struct route {
        address from;
        address to;
        bool stable;
    }

    #[derive(Debug)]
    function addLiquidityETH(
        address token,
        bool stable,
        uint256 amountTokenDesired,
        uint256 amountTokenMin,
        uint256 amountETHMin,
        address to,
        uint256 deadline
    )
        external payable returns (
            uint256 amountToken, uint256 amountETH, uint256 liquidity
        );

    #[derive(Debug)]
    function getReserves(
        address tokenA,
        address tokenB,
        bool stable
    ) public view returns (uint256 reserveA, uint256 reserveB);
    
    #[derive(Debug)]
    function removeLiquidityETH(
        address token,
        bool stable,
        uint256 liquidity,
        uint256 amountTokenMin,
        uint256 amountETHMin,
        address to,
        uint256 deadline
    ) external returns (uint256 amountToken, uint256 amountETH);

    #[derive(Debug)]
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        route[] calldata routes,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

pub async fn add_liquidity_eth(
    recepient: Address,
    amount: U256,
    token: Address,
    stable: bool,
    amount_token_desired: U256,
    amount_token_min: U256,
    amount_eth_min: U256,
) -> Result<Vec<RawTransaction>> {
    let approve_tx = erc20::approve(token, ROUTER, amount_token_desired)?;
    let add_liquidity_tx = RawTransaction {
        to: ROUTER,
        value: amount,
        data: addLiquidityETHCall {
            token, stable,
            amountTokenDesired: amount_token_desired,
            amountTokenMin: amount_token_min,
            amountETHMin: amount_eth_min,
            to: recepient,
            deadline: U256::from(get_current_timestamp() + 20 * 60),
        }.abi_encode().into(),
    };

    Ok(vec![approve_tx, add_liquidity_tx])
}

pub fn swap_exact_tokens_for_tokens(
    recepient: Address,
    token_in: Address, token_out: Address,
    amount_in: U256, amount_out_min: U256,
) -> Result<RawTransaction> {
    Ok(RawTransaction {
        to: ROUTER,
        value: U256::from(0),
        data: swapExactTokensForTokensCall {
            amountIn: amount_in,
            amountOutMin: amount_out_min,
            routes: vec![
                route {
                    from: token_in,
                    to: token_out,
                    stable: false,
                },
            ],
            to: recepient,
            deadline: U256::from(get_current_timestamp() + 20 * 60),
        }.abi_encode().into(),
    })
}

pub async fn get_reserves(
    token_a: Address,
    token_b: Address,
    stable: bool,
) -> Result<(U256, U256)> {

    let reserves = eth_call(RawTransaction {
        to: ROUTER,
        value: U256::from(0),
        data: getReservesCall {
            tokenA: token_a,
            tokenB: token_b,
            stable: stable,
        }.abi_encode().into(),
    }).await?;

    let reserve_a = U256::from_be_slice(&reserves[0..32]) / U256::from(10).pow(U256::from(9));
    let reserve_b = U256::from_be_slice(&reserves[32..64]);

    Ok((reserve_a, reserve_b))
}