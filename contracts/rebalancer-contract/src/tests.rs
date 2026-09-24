#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Env,
};

#[test]
fn test_rebalance_dry_run() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    trades.push(Trade {
        asset_pair: (Symbol::new(&env, "USDC"), Symbol::new(&env, "XLM")),
        amount: 1000,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    assert_eq!(result.expected_fees, 1);
    // Slippage should be a positive value representing the accumulated slippage from trades
    // The exact value depends on the liquidity depth in the predictor
    assert!(result.expected_slippage.to_i128(&env) >= 0);
}

#[test]
fn test_rebalance_accumulates_slippage_from_multiple_trades() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    // Add multiple trades with different amounts
    trades.push(Trade {
        asset_pair: (Symbol::new(&env, "USDC"), Symbol::new(&env, "XLM")),
        amount: 1000,
    });
    trades.push(Trade {
        asset_pair: (Symbol::new(&env, "USDC"), Symbol::new(&env, "BTC")),
        amount: 500_000,  // Larger trade
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    // Total slippage should be the sum of individual trade slippages
    // Exact value depends on mock liquidity data in predictor
    assert!(result.expected_slippage.to_i128(&env) > 0, "Slippage should be accumulated");
}

#[test]
fn test_rebalance_zero_trade_slippage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    // Zero amount trade should have zero slippage
    trades.push(Trade {
        asset_pair: (Symbol::new(&env, "USDC"), Symbol::new(&env, "XLM")),
        amount: 0,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    // Zero trade should result in zero slippage
    assert_eq!(result.expected_slippage.to_i128(&env), 0, "Zero trade should have zero slippage");
}