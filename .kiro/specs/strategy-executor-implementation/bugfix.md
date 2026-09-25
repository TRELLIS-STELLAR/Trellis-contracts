# Bugfix Requirements Document

## Introduction

The `execute_strategy` function in `contracts/rebalancer-contract/src/strategy_executor.rs` is a placeholder implementation that ignores the execution strategy parameter and returns hardcoded `true` regardless of actual execution outcomes. This creates a critical disconnect between the contract's reported state and its actual behavior: callers believe trades have been executed and recorded when nothing actually happened. The three strategy variants (MinimalCost, MinimalTime, Balanced) are indistinguishable, failed trades are silently swallowed, and on-chain events contain no trade details. This bugfix moves the implementation from placeholder to functional, implementing distinct strategy behaviors, performing actual token transfers, and reporting execution results accurately.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN `execute_strategy` is called with any ExecutionStrategy variant (MinimalCost, MinimalTime, or Balanced) THEN the function discards the strategy parameter and executes identically for all three variants

1.2 WHEN `execute_strategy` is called with a list of trades THEN it loops through trades and calls `log_trade` with hardcoded values (0 for price, 0 for fee) instead of actual execution prices and fees

1.3 WHEN `execute_strategy` is called THEN the function makes no calls to `shared::payments::safe_transfer` or `safe_transfer_from_contract`, so no actual token transfers occur

1.4 WHEN any trade fails to execute (e.g., insufficient balance, slippage exceeds limit) THEN the failure is not reported to the caller; the function continues looping and returns `true` unconditionally

1.5 WHEN `execute_strategy` returns a result THEN the caller receives bare `bool` value (`true`) with no way to distinguish between full success, partial success, or complete failure

1.6 WHEN `log_trade` is invoked with trade details, actual execution price, and fee THEN the function discards all three parameters (_trade, _actual_price, _fee) and emits a generic event with no trade-specific information, making the on-chain event record useless for auditing or debugging

1.7 WHEN the rebalancer contract calls `execute_strategy` and the result is `true` THEN external callers and on-chain observers wrongly conclude that rebalancing actually occurred, even though no transfers were made and no meaningful events were emitted

### Expected Behavior (Correct)

2.1 WHEN `execute_strategy` is called with ExecutionStrategy::MinimalCost THEN the function executes trades prioritizing lowest execution cost, potentially accepting longer execution time

2.2 WHEN `execute_strategy` is called with ExecutionStrategy::MinimalTime THEN the function executes trades prioritizing shortest execution time, potentially accepting higher execution cost

2.3 WHEN `execute_strategy` is called with ExecutionStrategy::Balanced THEN the function executes trades using a balanced approach between execution cost and time

2.4 WHEN `execute_strategy` is called with a list of trades THEN it performs actual token transfers via `shared::payments::safe_transfer_from_contract` or similar function for each trade, moving tokens from the contract to the intended destination

2.5 WHEN a trade executes successfully THEN the actual execution price and fee incurred are captured and passed to `log_trade` instead of hardcoded 0 values

2.6 WHEN a trade fails to execute (e.g., insufficient balance, recipient validation fails) THEN the error is captured and `execute_strategy` returns a result type expressing the failure (e.g., `Result<ExecutionSummary, Error>` or similar) instead of bare `bool`

2.7 WHEN `log_trade` is called with a Trade, actual execution price, and actual fee THEN the function records and emits all three values in the on-chain event, enabling auditors and callers to verify that the logged trade matches the actual execution

2.8 WHEN `execute_strategy` completes execution of all trades (successfully or with some failures) THEN the return value conveys the summary of outcomes: total trades executed, failed trades count, total fees paid, and any errors encountered

### Unchanged Behavior (Regression Prevention)

3.1 WHEN the dry_run flag is set to true in the rebalance function THEN `execute_strategy` is not called and no transfers occur, maintaining the dry-run semantics

3.2 WHEN `emit_action_executed` is called in the rebalance function THEN it continues to emit events to the shared event system following the existing event pattern

3.3 WHEN `predict_slippage` and `calculate_total_fees` are called during rebalance THEN they continue to operate identically, returning expected fees and slippage without change

3.4 WHEN the Trade struct is used to represent asset pairs and amounts THEN the struct definition and usage patterns remain unchanged

3.5 WHEN ExecutionStrategy enum values are defined as MinimalCost, MinimalTime, and Balanced THEN the enum variants remain unchanged and their names are not altered

3.6 WHEN other modules (slippage_predictor, fee_calculator) are imported and used THEN their existing APIs and behaviors are not affected by the strategy executor fix
