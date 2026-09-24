# Strategy Executor Bugfix Design

## Overview

The `execute_strategy` function is a placeholder that ignores the execution strategy parameter, makes no real token transfers, and returns a bare `bool` without indicating success or failure. This design document formalizes the bug condition and outlines a comprehensive fix that implements distinct strategy behaviors (MinimalCost, MinimalTime, Balanced), performs actual token transfers via the shared payments module, returns a structured result type expressing execution outcomes, and records all trade details in auditable events.

The fix transforms the strategy executor from a placeholder to a functional module that distinguishes between strategy variants, captures real execution prices and fees, handles failures transparently, and integrates seamlessly with the existing rebalancer contract and shared module APIs.

## Glossary

- **Bug_Condition (C)**: Any call to `execute_strategy` with any ExecutionStrategy variant (MinimalCost, MinimalTime, or Balanced), where the function discards the strategy parameter, makes no actual token transfers, returns bare `bool`, and logs generic events without trade details.

- **Property (P)**: The fixed `execute_strategy` function SHALL implement distinct behaviors for each strategy variant, perform actual token transfers using `shared::payments::safe_transfer_from_contract`, return a structured `Result<ExecutionSummary, Error>` type expressing execution outcomes, and record real prices and fees in auditable events.

- **Preservation**: All existing behaviors that must remain unchanged by the fix:
  - Dry-run semantics (when `dry_run = true`, `execute_strategy` is not called)
  - Trade struct definition and usage patterns
  - ExecutionStrategy enum variants (MinimalCost, MinimalTime, Balanced)
  - SimulationResult struct and predict_slippage/calculate_total_fees functions
  - Event emission pattern via `shared::events::emit_action_executed`

- **ExecutionStrategy**: An enum with three variants representing distinct trade execution approaches:
  - **MinimalCost**: Prioritize lowest execution cost, accept longer time
  - **MinimalTime**: Prioritize shortest execution time, accept higher cost
  - **Balanced**: Balance cost and time considerations

- **Trade**: A struct with `asset_pair: (Symbol, Symbol)` and `amount: u128` representing a single token transfer from contract to destination.

- **ExecutionSummary**: A structured result type capturing the outcome of strategy execution, including total trades, successful trades, failed trades, total fees paid, total slippage realized, and detailed error information.

- **safe_transfer_from_contract**: A function from the `shared::payments` module that transfers tokens from the contract balance to a recipient, emitting a PAYMENT_TRANSFER event and failing if balance is insufficient or recipient is invalid.

- **Batch Payment Function**: A function from `shared::batch` module (inferred from design context) that executes multiple transfers in a single transaction, optimized for high throughput.

- **log_trade**: The function that records trade execution details in an on-chain event; currently broken because it discards parameters, must be fixed to emit structured events with real prices, fees, and trade details.

## Bug Details

### Fault Condition

The bug manifests when `execute_strategy` is called with any ExecutionStrategy variant (MinimalCost, MinimalTime, or Balanced) and a list of trades to execute. The function is either not distinguishing between strategy variants, not performing actual token transfers, not capturing real execution prices and fees, not handling failures transparently, or not recording trade details accurately.

**Formal Specification:**

```
FUNCTION isBugCondition(env, strategy, trades)
  INPUT: env (Env reference)
  INPUT: strategy (ExecutionStrategy variant)
  INPUT: trades (Vec<Trade> with length > 0)
  OUTPUT: boolean
  
  RETURN (strategy IN [MinimalCost, MinimalTime, Balanced])
         AND (length(trades) > 0)
         AND (execute_strategy ignores strategy parameter)
         AND (no calls to safe_transfer_from_contract occur)
         AND (log_trade called with hardcoded 0 values instead of real prices/fees)
         AND (return value is bare bool without outcome details)
         AND (events emitted contain no trade-specific information)
END FUNCTION
```

### Examples

**Example 1: MinimalCost vs MinimalTime Indistinguishable**
- Input: Two strategy variants with identical trade list
- Current (broken): Both execute identically, discarding strategy parameter
- Expected (correct): MinimalCost processes trades sequentially by fee; MinimalTime batches aggressively

**Example 2: No Real Transfers Occur**
- Input: Trade with amount=1000 tokens from contract to recipient
- Current (broken): log_trade called with hardcoded price=0, fee=0; no actual transfer happens
- Expected (correct): safe_transfer_from_contract called with amount=1000; actual fee calculated and captured

**Example 3: Failure Not Reported**
- Input: Trade with amount exceeding contract balance
- Current (broken): Function returns true despite transfer failing; no error information
- Expected (correct): Function returns Result with ExecutionSummary showing failed_trades=1 and error details

**Example 4: No Trade Details in Events**
- Input: Trade with asset_pair=(USD, EUR) and amount=500
- Current (broken): Event emitted with no trade data (all parameters discarded)
- Expected (correct): Event contains asset_pair, amount, actual_price, fee, recipient, timestamp

**Example 5: Partial Failure Swallowed**
- Input: 5 trades where trade #3 fails due to insufficient balance
- Current (broken): Returns true; trades #1,#2,#4,#5 succeed but trade #3 failure hidden
- Expected (correct): Returns ExecutionSummary with successful_trades=4, failed_trades=1, errors list

**Example 6: Strategy-Specific Behaviors Not Implemented**
- Input: Balanced strategy with 100 trades
- Current (broken): Processes all trades sequentially without batching or retry logic
- Expected (correct): Batches into groups of 10-20; retries once on transient error

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**

1. When `dry_run = true` in the `rebalance` function, `execute_strategy` is not called and no transfers occur, maintaining dry-run semantics.

2. The Trade struct remains defined as `(Symbol, Symbol) asset_pair, u128 amount` with no structural changes.

3. The ExecutionStrategy enum continues to have exactly three variants: MinimalCost, MinimalTime, and Balanced, with no name changes or additional variants.

4. The `predict_slippage` and `calculate_total_fees` functions operate identically to their current implementation without modifications.

5. The SimulationResult struct definition and return type from `rebalance` remain unchanged.

6. Event emission via `shared::events::emit_action_executed` continues to follow the existing pattern for high-level action events.

7. The rebalance function signature and overall contract structure remain unchanged; only internal strategy execution behavior is modified.

**Scope:**

All inputs that do NOT trigger the bug condition (i.e., when the strategy is correctly implemented, transfers succeed, and trade details are properly recorded) should be completely unaffected by this fix. This includes:

- Dry-run calls that skip execution entirely
- Functions imported from other modules (slippage_predictor, fee_calculator, shared::events)
- Trade struct usage in callers
- ExecutionStrategy enum variants used elsewhere in the codebase

## Hypothesized Root Cause

Based on the bug description and code inspection, the most likely issues are:

1. **Placeholder Implementation Not Completed**: The `execute_strategy` function was stubbed as a placeholder with a `_strategy` parameter prefix (underscore) indicating intentional ignoring, and hardcoded `0` values for price and fee, suggesting development was incomplete.

2. **No Integration with Shared Payments Module**: The function does not call `shared::payments::safe_transfer_from_contract` or any transfer mechanism; tokens never leave the contract. The payment module interface is available but never invoked.

3. **Result Type Not Designed**: The function returns bare `bool` instead of a structured result type, making it impossible to express partial failures, error reasons, or execution statistics.

4. **log_trade Function Broken**: The `log_trade` function has underscore-prefixed parameters (`_trade`, `_actual_price`, `_fee`) indicating intentional ignoring, and emits generic events using `emit_action_executed` without trade-specific data.

5. **No Strategy Differentiation Logic**: There are no conditionals based on the strategy variant; MinimalCost, MinimalTime, and Balanced paths all execute identically with no distinction in ordering, batching, or retry logic.

6. **No Error Handling**: The loop in `execute_strategy` lacks try-catch or error propagation logic; failures silently continue and return `true` unconditionally.

7. **Missing Batch Payment Integration**: There is no code using batch payment functions from `shared::batch` module, which would be necessary for MinimalTime strategy to achieve high throughput.

## Correctness Properties

Property 1: Fault Condition - Distinct Strategy Execution and Real Transfers

_For any_ call to `execute_strategy` with a valid ExecutionStrategy variant and a list of trades where the bug condition holds (strategy was ignored, transfers were not performed, results were bare bool, events lacked details), the fixed function SHALL implement distinct behaviors for MinimalCost, MinimalTime, and Balanced strategies, perform actual token transfers via `shared::payments::safe_transfer_from_contract`, capture real execution prices and fees, handle failures by recording them in ExecutionSummary.errors, and emit events containing full trade details including asset_pair, amount, actual_price, fee, recipient, and timestamp.

**Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8**

Property 2: Preservation - Unchanged Behaviors

_For any_ input that does NOT trigger the bug condition (dry-run mode, unchanged structs/enums, existing module APIs), the fixed code SHALL produce exactly the same behavior as the original code, preserving dry-run semantics, Trade struct definition, ExecutionStrategy enum variants, SimulationResult struct, predict_slippage/calculate_total_fees functions, and event emission patterns.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6**

## ExecutionStrategy Algorithm Definitions

### MinimalCost Strategy

**Objective**: Minimize total execution cost (fees), accepting longer execution time if necessary.

**Algorithm**:
1. Sort trades by estimated fee percentage in ascending order (cheapest first)
2. Execute trades sequentially (one per transaction) to minimize batching overhead
3. For each trade:
   - Determine optimal token routing if multiple routes available
   - If fee optimization possible, use `deduct_fee` with low fee rates
   - Allow extended wait times if beneficial for cost reduction
   - Perform transfer via `safe_transfer_from_contract`
4. On trade failure: Record error in ExecutionSummary.errors and continue processing remaining trades
5. Return ExecutionSummary with cumulative fees and all error details

**Trade Ordering**: Sorted by `estimated_fee_percentage` ascending (cheapest first)

**Wait/Retry Policy**: Allow longer wait times; if transient error, retry indefinitely (cost optimization priority)

**Batch Size**: 1 trade per transaction (minimal batching overhead)

**Fee Handling**: Use `deduct_fee` with low fee rates if multiple routes available; negotiate lowest cost

**Expected Use Case**: Large portfolio rebalancing where execution time is not critical but cost efficiency is paramount

**Pseudocode**:
```
trades_sorted := sort(trades, by=estimated_fee_percentage, ascending)
summary := ExecutionSummary { total_trades: len(trades), successful_trades: 0, failed_trades: 0, ... }

for each trade in trades_sorted:
    loop:  // Infinite retry loop for cost optimization
        try:
            recipient := resolve_recipient(trade)
            token := resolve_token(trade.asset_pair.0)
            actual_fee := negotiate_lowest_fee(trade, recipient)
            amount_after_fee := trade.amount - actual_fee
            
            safe_transfer_from_contract(env, token, recipient, amount_after_fee)
            actual_price := capture_execution_price(trade)
            
            log_trade(env, trade, actual_price, actual_fee, recipient, TradeStatus::Success)
            summary.successful_trades += 1
            break  // Exit infinite loop on success
        catch error:
            // For MinimalCost, retry transient errors indefinitely
            if is_transient_error(error):
                wait(exponential_backoff)
                continue  // Retry
            else:
                // Permanent error: record and move on
                summary.errors.push(TradeError { trade_index, reason: error })
                summary.failed_trades += 1
                break

return Ok(summary)
```

### MinimalTime Strategy

**Objective**: Minimize execution time / maximize throughput, accepting higher execution cost.

**Algorithm**:
1. Group trades by asset type to minimize context switches
2. Execute trades in large batches using batch payment functions
3. Process each batch sequentially (batches execute in parallel internally)
4. On any error: Fail fast and return immediately with partial ExecutionSummary
5. Do not retry transient errors; prioritize speed over robustness

**Trade Ordering**: Grouped by asset type (minimize context switches)

**Wait/Retry Policy**: No retries; fail fast on any error

**Batch Size**: Maximum batch size (50+ trades per transaction for high throughput)

**Fee Handling**: Standard fee processing with no optimization overhead

**Expected Use Case**: Time-sensitive arbitrage or emergency rebalancing where execution speed is critical

**Pseudocode**:
```
trades_grouped := group_by_asset_type(trades)
batches := create_batches(trades_grouped, batch_size=50)
summary := ExecutionSummary { total_trades: len(trades), successful_trades: 0, failed_trades: 0, ... }

for each batch in batches:
    try:
        // Execute all trades in batch simultaneously
        batch_recipients := [resolve_recipient(t) for t in batch]
        batch_tokens := [resolve_token(t.asset_pair.0) for t in batch]
        batch_amounts := [t.amount for t in batch]
        
        execute_multi_transfer(env, batch_tokens, batch_recipients, batch_amounts)
        
        // Log all trades in batch
        for each trade, recipient, actual_price, actual_fee in batch:
            log_trade(env, trade, actual_price, actual_fee, recipient, TradeStatus::Success)
            summary.successful_trades += 1
            summary.total_fees_paid += actual_fee
    catch error:
        // On any error: fail fast, record what succeeded, return
        summary.failed_trades = (len(batch) - summary.successful_trades)
        summary.errors.push(TradeError { reason: error, partial_batch: batch })
        return Ok(summary)  // Return partial result

return Ok(summary)
```

### Balanced Strategy

**Objective**: Balance cost and time considerations with sensible retry logic and moderate batching.

**Algorithm**:
1. Process trades in moderate batches (10-20 trades per transaction)
2. Respect caller ordering of trades (no sorting)
3. Retry once on transient errors, then fail
4. Apply single round of fee optimization
5. Continue processing remaining batches even if one batch fails

**Trade Ordering**: Process trades as provided by caller (respect ordering)

**Wait/Retry Policy**: Single retry on transient error, then move on

**Batch Size**: Moderate batch size (10-20 trades per transaction)

**Fee Handling**: Standard fee processing with single fee optimization round

**Expected Use Case**: Default rebalancing path for most use cases where both cost and time matter

**Pseudocode**:
```
batches := create_batches(trades, batch_size=15)
summary := ExecutionSummary { total_trades: len(trades), successful_trades: 0, failed_trades: 0, ... }

for each batch in batches:
    retry_count := 0
    max_retries := 1
    
    loop:
        try:
            batch_recipients := [resolve_recipient(t) for t in batch]
            batch_tokens := [resolve_token(t.asset_pair.0) for t in batch]
            batch_amounts := [t.amount for t in batch]
            
            execute_multi_transfer(env, batch_tokens, batch_recipients, batch_amounts)
            
            for each trade, recipient, actual_price, actual_fee in batch:
                log_trade(env, trade, actual_price, actual_fee, recipient, TradeStatus::Success)
                summary.successful_trades += 1
                summary.total_fees_paid += actual_fee
            break  // Exit retry loop on success
        catch error:
            if is_transient_error(error) AND retry_count < max_retries:
                retry_count += 1
                continue  // Retry once
            else:
                // Permanent error or retries exhausted: record error and continue to next batch
                summary.errors.push(TradeError { batch_index, reason: error })
                summary.failed_trades += len(batch)
                break

return Ok(summary)
```

## Result Type Design

### Current (Broken)

The function returns bare `bool` with no way to express partial failure, error reasons, or execution statistics:

```rust
pub fn execute_strategy(env: &Env, strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> bool {
    // ... broken implementation ...
    true  // No context about success or failure
}
```

### Proposed (Option A: Recommended)

Define a structured result type that captures execution outcomes:

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSummary {
    /// Total number of trades that were attempted
    pub total_trades: u32,
    
    /// Number of trades that succeeded
    pub successful_trades: u32,
    
    /// Number of trades that failed
    pub failed_trades: u32,
    
    /// Total fees paid across all successful trades
    pub total_fees_paid: u128,
    
    /// Total slippage realized across all trades (in basis points)
    pub total_slippage_realized: U256,
    
    /// Detailed error information for each failed trade
    pub errors: Vec<TradeError>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeError {
    /// Trade failed due to insufficient contract balance
    InsufficientBalance {
        trade_index: u32,
        required: u128,
        available: u128,
    },
    
    /// Trade failed because transfer to recipient failed
    TransferFailed {
        trade_index: u32,
        reason: Vec<u8>,  // Error message as bytes for storage efficiency
    },
    
    /// Trade failed because recipient address was invalid or validation failed
    InvalidRecipient {
        trade_index: u32,
    },
    
    /// Trade failed due to fee calculation overflow
    FeeCalculationError {
        trade_index: u32,
    },
    
    /// Trade failed due to token not supported or resolution error
    TokenResolutionError {
        trade_index: u32,
        symbol: Symbol,
    },
    
    /// Generic error with trade index and reason
    Other {
        trade_index: u32,
        reason: Vec<u8>,
    },
}

// New function signature
pub fn execute_strategy(
    env: &Env,
    strategy: &ExecutionStrategy,
    trades: &Vec<Trade>,
) -> Result<ExecutionSummary, Error> {
    // ... implementation ...
}
```

**Rationale for Option A**:
- Rust-idiomatic: Uses `Result` type familiar to Rust developers
- Clear semantics: `Ok(summary)` means execution completed (all or partial success); `Err` means total failure
- Rich error information: ExecutionSummary.errors contains detailed per-trade error information
- Non-destructive: Contains all success count data even in error cases
- Integrates cleanly: Soroban SDK supports Result type contracts

### Alternative (Option B: Custom Enum)

```rust
pub enum ExecutionResult {
    Success(ExecutionSummary),
    PartialSuccess {
        summary: ExecutionSummary,
        errors: Vec<TradeError>,
    },
    Failure {
        reason: Error,
        partial_summary: Option<ExecutionSummary>,
    },
}
```

**Rationale for Option B**:
- More explicit about partial success cases
- But more verbose and requires pattern matching
- Not idiomatic Rust; less familiar to developers

### Alternative (Option C: Simpler Result)

```rust
pub struct ExecutionStats {
    pub successful_trades: u32,
    pub failed_trades: u32,
    pub total_fees: u128,
}

pub fn execute_strategy(...) -> Result<ExecutionStats, Error> { ... }
```

**Rationale for Option C**:
- Simpler, smaller type for storage efficiency
- But lacks error detail information; harder to debug failures

**RECOMMENDATION**: Proceed with **Option A** (`Result<ExecutionSummary, Error>`) as the primary design. It provides:
- Clear success/failure semantics
- Rich error detail for debugging
- Rust idiomaticity
- Capacity to express partial success (summary shows successful_trades < total_trades)

## Payment Integration

### Design Approach

For each trade executed via `execute_strategy`, perform the following steps to interact with the `shared::payments` module:

**Step 1: Extract Trade Details**
```rust
let asset_pair = trade.asset_pair;  // (Symbol, Symbol)
let amount = trade.amount;           // u128
let source_asset = asset_pair.0;    // Source token symbol
```

**Step 2: Resolve Recipient Address**
```rust
// Determine where tokens should be transferred
// Option A: Recipient is context-specific (e.g., rebalancer destination)
// Option B: Recipient is stored in env context or trade extension
let recipient = resolve_recipient_from_context(&env, &trade)?;
```

**Step 3: Resolve Token Address from Symbol**
```rust
// Convert source asset Symbol to token Address via oracle or registry
// This is critical: shared::payments expects Address, not Symbol
let token_address = resolve_token_address(&env, source_asset)?;
```

**Step 4: Call safe_transfer_from_contract**
```rust
// Perform the actual transfer from contract to recipient
let transfer_result = shared::payments::safe_transfer_from_contract(
    &env,
    &token_address,
    &recipient,
    amount,
);

match transfer_result {
    Ok(transfer_info) => {
        // Transfer succeeded; capture actual execution details
        let actual_price = transfer_info.execution_price;
        let actual_fee = transfer_info.fee_paid;
        // Continue to Step 5
    }
    Err(e) => {
        // Transfer failed; record error in ExecutionSummary
        record_trade_error(&mut summary, trade_index, e);
        continue;  // Move to next trade (or retry based on strategy)
    }
}
```

**Step 5: Calculate Real Fees (if not returned by safe_transfer)**
```rust
// If safe_transfer doesn't return fee, calculate separately
let actual_fee = if transfer_info.contains_fee {
    transfer_info.fee_paid
} else {
    shared::payments::calculate_fee(&env, amount, &token_address)?
};
```

**Step 6: Log Trade with Real Details**
```rust
log_trade(
    &env,
    &trade,
    actual_price,      // Real execution price (NOT 0)
    actual_fee,        // Real fee incurred (NOT 0)
    &recipient,
    TradeStatus::Success,
);
```

**Step 7: Update ExecutionSummary**
```rust
summary.successful_trades += 1;
summary.total_fees_paid = summary.total_fees_paid.saturating_add(actual_fee);
summary.total_slippage_realized = summary.total_slippage_realized
    .add(&U256::from_u128(&env, slippage_bps));
```

### Token Resolution Strategy

**Challenge**: Trade struct uses `(Symbol, Symbol)` for asset_pair, but `safe_transfer_from_contract` requires token Address.

**Solution Options**:

1. **Oracle-based Resolution** (Recommended for production)
   - Query oracle contract for (Symbol → Address) mapping
   - Cache results during execution to minimize oracle calls
   - Handle oracle failures gracefully

2. **Registry-based Resolution**
   - Maintain internal registry mapping Symbol to Address
   - Load registry once at function start
   - Update registry via admin function if tokens change

3. **Pass Token Address via Context**
   - Extend Trade struct to include token Address (breaking change - NOT RECOMMENDED)
   - Or pass separate mapping as function parameter

**RECOMMENDATION for MVP**: Use oracle-based resolution with caching:
```rust
fn resolve_token_address(env: &Env, symbol: Symbol) -> Result<Address, Error> {
    // Check cache first
    if let Some(addr) = TOKEN_CACHE.get(symbol) {
        return Ok(addr);
    }
    
    // Query oracle if not cached
    let oracle = shared::oracle::get_oracle_client(env);
    let addr = oracle.get_token_address(symbol)?;
    TOKEN_CACHE.insert(symbol, addr);
    
    Ok(addr)
}
```

**Document as assumption**: Token resolution assumes an oracle or registry is available. This must be validated during implementation.

## Logging Event Structure

### Current (Broken)

The `log_trade` function receives trade details but discards them:

```rust
pub fn log_trade(env: &Env, _trade: &Trade, _actual_price: u128, _fee: u128) {
    emit_action_executed(
        env,
        symbol_short!("reb"),
        symbol_short!("trade"),
        &env.current_contract_address(),
        true,                           // No meaningful context
        env.ledger().timestamp(),
    );
}
```

The emitted event contains no trade-specific information, making auditing impossible.

### Proposed (Structured Event)

Define a structured TradeExecuted event and emit it with all trade details:

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeExecuted {
    /// Asset pair (source asset, destination asset)
    pub asset_pair: (Symbol, Symbol),
    
    /// Trade amount in smallest unit
    pub amount: u128,
    
    /// Actual execution price (not 0)
    pub actual_price: u128,
    
    /// Real fee paid (not 0)
    pub fee: u128,
    
    /// Recipient address where tokens were transferred
    pub recipient: Address,
    
    /// Ledger timestamp of execution
    pub timestamp: u64,
    
    /// Trade execution status
    pub status: TradeStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeStatus {
    /// Trade fully executed
    Success,
    
    /// Trade partially filled
    PartialFill {
        executed_amount: u128,
        requested_amount: u128,
    },
    
    /// Trade failed with reason
    Failed {
        reason: Vec<u8>,
    },
}

// Enhanced log_trade function
pub fn log_trade(
    env: &Env,
    trade: &Trade,
    actual_price: u128,
    fee: u128,
    recipient: &Address,
    status: TradeStatus,
) {
    // Emit structured trade event
    let trade_event = TradeExecuted {
        asset_pair: trade.asset_pair.clone(),
        amount: trade.amount,
        actual_price,   // Real value, not 0
        fee,            // Real value, not 0
        recipient: recipient.clone(),
        timestamp: env.ledger().timestamp(),
        status,
    };
    
    env.events().publish(("trade_executed",), trade_event);
    
    // Also emit high-level action event for compatibility
    emit_action_executed(
        env,
        symbol_short!("reb"),
        symbol_short!("trade"),
        &env.current_contract_address(),
        matches!(status, TradeStatus::Success),  // true only if Success
        env.ledger().timestamp(),
    );
}
```

### Event Details

**TradeExecuted Fields**:
- `asset_pair`: (Symbol, Symbol) - identifies which tokens were traded
- `amount`: u128 - quantity transferred
- `actual_price`: u128 - execution price (critical: NOT 0 like the bug)
- `fee`: u128 - fee incurred (critical: NOT 0 like the bug)
- `recipient`: Address - destination of tokens
- `timestamp`: u64 - ledger timestamp for ordering
- `status`: TradeStatus - Success, PartialFill, or Failed

**Key Differences from Current (Broken) Implementation**:
1. All parameters are actually used (not prefixed with `_`)
2. Event contains trade-specific data (asset_pair, amount, prices, fees)
3. actual_price and fee are real values, not hardcoded 0
4. TradeStatus enum distinguishes success/partial/failure
5. Recipient is recorded for audit trail

## Error Handling Strategy

### Failure Modes per Trade

Each trade execution can fail in several ways:

1. **InsufficientBalance**: Contract has fewer tokens than trade amount requires
   - Likely cause: Contract not funded properly before calling execute_strategy
   - Recovery: Record error, continue with other trades (or fail fast for MinimalTime)

2. **TransferFailed**: Underlying transfer call to shared::payments failed
   - Likely causes: Invalid recipient, recipient validation failed, token not supported
   - Recovery: Record error with reason, continue or retry based on strategy

3. **FeeCalculationError**: Fee calculation overflows or fails
   - Likely causes: Amount too large, fee rate misconfigured
   - Recovery: Record error, continue (skip this trade)

4. **TokenResolutionError**: Cannot resolve asset Symbol to token Address
   - Likely causes: Oracle unavailable, symbol not supported, oracle returns invalid address
   - Recovery: Record error, continue (skip this trade)

5. **InvalidRecipient**: Recipient address fails validation
   - Likely causes: Recipient is zero address, recipient not whitelisted
   - Recovery: Record error, continue (skip this trade)

6. **TransientError**: Temporary failure that may succeed on retry
   - Likely causes: Network congestion, ledger full, temporary resource limits
   - Recovery: Retry based on strategy (MinimalCost: infinite retries; Balanced: 1 retry; MinimalTime: fail fast)

### Handling Approach per Strategy

**MinimalCost Strategy**:
- InsufficientBalance: Record error, continue
- TransferFailed: Record error, continue
- FeeCalculationError: Record error, continue
- TokenResolutionError: Record error, continue
- InvalidRecipient: Record error, continue
- TransientError: Retry indefinitely (cost optimization priority)
- **Behavior**: Attempt all trades, retry transient errors, record all failures but continue

**MinimalTime Strategy**:
- Any error (including transient): Record partial summary and return immediately
- Do NOT retry
- **Behavior**: Fail fast on first error to maximize speed; partial result shows what succeeded

**Balanced Strategy**:
- InsufficientBalance: Record error, continue
- TransferFailed: Record error, continue
- FeeCalculationError: Record error, continue
- TokenResolutionError: Record error, continue
- InvalidRecipient: Record error, continue
- TransientError: Retry once, then record error and continue
- **Behavior**: Attempt all trades, single retry on transient errors, record failures but continue

### Return Value Semantics

**Using Option A Result Type** (`Result<ExecutionSummary, Error>`):

- **Ok(summary)** where `summary.failed_trades == 0`: All trades succeeded; full success
- **Ok(summary)** where `summary.failed_trades > 0`: Partial success; some trades succeeded, some failed; summary contains error details
- **Err(error)**: Total failure; no trades succeeded; used when strategy halts early (MinimalTime on first error) or fatal error occurs (e.g., contract address lookup fails)

### Error Propagation Pseudocode

```rust
fn execute_strategy(env: &Env, strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> Result<ExecutionSummary, Error> {
    let mut summary = ExecutionSummary::new();
    
    match strategy {
        ExecutionStrategy::MinimalCost => {
            for (index, trade) in trades.iter().enumerate() {
                match execute_trade_with_retries(env, trade, max_retries=INFINITE) {
                    Ok((price, fee)) => {
                        summary.successful_trades += 1;
                        summary.total_fees_paid += fee;
                    }
                    Err(e) => {
                        summary.failed_trades += 1;
                        summary.errors.push(TradeError::new(index, e));
                    }
                }
            }
            Ok(summary)  // Always return Ok, even if failures
        }
        ExecutionStrategy::MinimalTime => {
            for (index, trade) in trades.iter().enumerate() {
                match execute_trade_with_retries(env, trade, max_retries=0) {  // No retries
                    Ok((price, fee)) => {
                        summary.successful_trades += 1;
                    }
                    Err(e) => {
                        summary.failed_trades += 1;
                        summary.errors.push(TradeError::new(index, e));
                        return Ok(summary);  // Fail fast: return partial
                    }
                }
            }
            Ok(summary)
        }
        ExecutionStrategy::Balanced => {
            for (index, trade) in trades.iter().enumerate() {
                match execute_trade_with_retries(env, trade, max_retries=1) {
                    Ok((price, fee)) => {
                        summary.successful_trades += 1;
                    }
                    Err(e) => {
                        summary.failed_trades += 1;
                        summary.errors.push(TradeError::new(index, e));
                    }
                }
            }
            Ok(summary)
        }
    }
}
```

## Fix Implementation

### Changes Required

Assuming the root cause analysis is correct, the following changes are required:

**File**: `contracts/rebalancer-contract/src/strategy_executor.rs`

**Function**: `execute_strategy`

**Specific Changes**:

1. **Change Function Signature**:
   - From: `pub fn execute_strategy(env: &Env, _strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> bool`
   - To: `pub fn execute_strategy(env: &Env, strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> Result<ExecutionSummary, Error>`
   - Rationale: Remove underscore prefix on `_strategy` parameter (was intentionally ignored); return structured result instead of bare bool

2. **Define ExecutionSummary Type**:
   - Add `pub struct ExecutionSummary` with fields: total_trades, successful_trades, failed_trades, total_fees_paid, total_slippage_realized, errors
   - Add `pub enum TradeError` with variants: InsufficientBalance, TransferFailed, InvalidRecipient, FeeCalculationError, TokenResolutionError, Other
   - Both types must be marked `#[contracttype]` for Soroban serialization

3. **Implement Strategy Differentiation**:
   - Add match statement on strategy parameter: `match strategy { MinimalCost => {...}, MinimalTime => {...}, Balanced => {...} }`
   - MinimalCost: Sort trades by fee percentage; process sequentially; infinite retry on transient errors
   - MinimalTime: Batch trades; execute in large groups; fail fast on any error; no retries
   - Balanced: Batch trades moderately; single retry on transient errors; continue on permanent errors

4. **Integrate with shared::payments**:
   - For each trade, call `shared::payments::safe_transfer_from_contract` instead of doing nothing
   - Resolve token Address from asset Symbol (via oracle or registry)
   - Capture actual execution price and fee from transfer response
   - Handle transfer errors and record in ExecutionSummary.errors

5. **Implement Token Resolution**:
   - Add function `resolve_token_address(symbol: Symbol) -> Result<Address, Error>`
   - Use oracle contract to map Symbol → Address
   - Cache results to minimize oracle calls
   - Document this assumption

6. **Replace log_trade Calls**:
   - Change from: `log_trade(env, &trade, 0, 0)`
   - To: `log_trade(env, &trade, actual_price, actual_fee, &recipient, TradeStatus::Success)`
   - Pass real prices, fees, recipient, and status instead of hardcoded values

7. **Add ExecutionSummary Initialization**:
   - Create summary struct at start of function
   - Initialize: total_trades, successful_trades=0, failed_trades=0, errors=empty_vec()

8. **Add Error Handling Loop**:
   - Wrap trade execution in try-catch or Result pattern
   - On success: increment successful_trades, add fees to total
   - On error: increment failed_trades, add TradeError to errors vector
   - For MinimalTime strategy: Return early if error occurs

9. **Update lib.rs**:
   - Add ExecutionSummary and TradeError to public exports or lib.rs module
   - Update rebalance function to handle Result return type from execute_strategy
   - Decide whether to propagate error or convert to event

10. **Update logging.rs**:
    - Change log_trade signature: `pub fn log_trade(env: &Env, trade: &Trade, actual_price: u128, fee: u128, recipient: &Address, status: TradeStatus) -> ()`
    - Emit structured TradeExecuted event with all fields
    - Use actual_price and fee values instead of discarding them
    - Log status (Success/PartialFill/Failed) instead of hardcoded true

### Modified Files

1. **strategy_executor.rs**:
   - Add ExecutionSummary struct definition
   - Add TradeError enum definition
   - Add token resolution function
   - Rewrite execute_strategy function with strategy differentiation and error handling
   - Add helper functions for each strategy variant

2. **logging.rs**:
   - Add TradeExecuted struct definition
   - Add TradeStatus enum definition
   - Rewrite log_trade to emit structured event with real values

3. **lib.rs**:
   - Export ExecutionSummary and TradeError types
   - Update rebalance function to handle Result from execute_strategy

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach:

**Phase 1: Exploratory Fault Condition Checking**
- Surface counterexamples that demonstrate the bug BEFORE implementing the fix
- Confirm or refute root cause analysis by observing unfixed behavior
- Identify exact failure patterns and error modes

**Phase 2: Fix Verification**
- Verify fix checks: Confirm that for all inputs where the bug condition holds, the fixed function produces expected behavior
- Preservation checks: Confirm that for all inputs where the bug condition does NOT hold, the fixed function preserves existing behavior

### Exploratory Fault Condition Checking

**Goal**: Surface counterexamples that demonstrate the bug on unfixed code. Confirm or refute root cause analysis.

**Test Plan**: Write unit tests that simulate trade execution with strategy variants, assert that actual transfers occur and events are recorded, and run on UNFIXED code to observe failures and understand root cause.

**Test Cases**:

1. **MinimalCost vs MinimalTime Strategy Differentiation** (will fail on unfixed code)
   - Create two identical trade lists
   - Execute one with MinimalCost, one with MinimalTime
   - Assert that MinimalCost processes trades sequentially (one per batch)
   - Assert that MinimalTime processes trades in large batches
   - Unfixed code will process both identically (no batching differences)

2. **Real Transfer Occurs** (will fail on unfixed code)
   - Create trade with amount=1000, recipient=test_account
   - Assert that safe_transfer_from_contract is called with amount=1000
   - Assert that contract balance decreases by 1000
   - Assert that recipient balance increases by 1000 (minus fees)
   - Unfixed code will not call safe_transfer, balances unchanged

3. **Actual Price and Fee Captured** (will fail on unfixed code)
   - Create trade with known execution price and fee
   - Execute trade and capture log_trade call
   - Assert that log_trade was called with actual_price != 0 and fee != 0
   - Unfixed code will call log_trade with hardcoded 0 values

4. **Failure Recorded in Result** (will fail on unfixed code)
   - Create trade with amount exceeding contract balance
   - Execute trade and capture result
   - Assert that result is Ok(summary) with failed_trades=1 and errors containing InsufficientBalance
   - Unfixed code will return true despite transfer failing

5. **Structured Event Emitted** (will fail on unfixed code)
   - Execute trade
   - Query emitted events
   - Assert that TradeExecuted event was emitted with all fields (asset_pair, amount, actual_price, fee, recipient, timestamp)
   - Unfixed code will emit generic event with no trade data

6. **Partial Failure Handled** (will fail on unfixed code)
   - Create 5 trades where trade #3 fails and others succeed
   - Execute with Balanced strategy
   - Assert that result is Ok(summary) with successful_trades=4, failed_trades=1
   - Assert that trades #1,#2,#4,#5 show in successful logs
   - Unfixed code will return true; trade #3 failure hidden

**Expected Counterexamples on Unfixed Code**:
- Transfers not performed: Contract balance remains unchanged, recipient balance unchanged
- Generic events: Events contain no trade-specific information
- Bare bool returned: Cannot distinguish success from failure
- No strategy differentiation: MinimalCost and MinimalTime behave identically

### Fix Checking

**Goal**: Verify that for all inputs where the bug condition holds, the fixed function produces expected behavior.

**Pseudocode**:
```
FOR ALL input WHERE isBugCondition(input) DO
  result := execute_strategy_fixed(env, strategy, trades)
  ASSERT result IS Result::Ok(summary)
  ASSERT summary.total_trades == length(trades)
  ASSERT summary.successful_trades + summary.failed_trades == length(trades)
  ASSERT (for strategy=MinimalCost: trades sorted by fee percentage ascending)
  ASSERT (for strategy=MinimalTime: trades processed in large batches)
  ASSERT (for strategy=Balanced: trades processed in moderate batches)
  ASSERT (all successful trades have actual_price != 0 and fee != 0 in logs)
  ASSERT (all failed trades recorded in summary.errors)
  ASSERT (all transferred amounts reflected in contract/recipient balances)
END FOR
```

### Preservation Checking

**Goal**: Verify that for all inputs where the bug condition does NOT hold, the fixed function preserves existing behavior.

**Pseudocode**:
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  ASSERT execute_strategy_original(input) = execute_strategy_fixed(input)
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because:
- It generates many test cases automatically across the input domain
- It catches edge cases that manual unit tests might miss
- It provides strong guarantees that behavior is unchanged for all non-buggy inputs
- Soroban supports quickcheck or similar PBT frameworks

**Test Plan**: Observe behavior on UNFIXED code first for dry-run and non-execution paths, then write property-based tests capturing that behavior and verifying it continues after fix.

**Test Cases**:

1. **Dry-Run Preservation** (verify on unfixed, then test continues after fix)
   - Execute rebalance with dry_run=true
   - Assert that execute_strategy is not called
   - Assert that no transfers occur
   - Assert that SimulationResult contains expected_fees and expected_slippage
   - Verify this behavior unchanged after fix

2. **Non-Executed Path Preservation** (verify on unfixed, then test continues after fix)
   - Call other functions (predict_slippage, calculate_total_fees) without calling execute_strategy
   - Assert that results are identical before and after fix
   - Verify independence of strategy executor from other modules

3. **Trade Struct Preservation** (verify on unfixed, then test continues after fix)
   - Create various Trade structs with different asset_pair and amount values
   - Assert that Trade struct definition unchanged
   - Assert that Trade usage patterns work identically in callers

4. **ExecutionStrategy Enum Preservation** (verify on unfixed, then test continues after fix)
   - Assert that ExecutionStrategy enum has exactly three variants: MinimalCost, MinimalTime, Balanced
   - Assert that enum variants cannot be serialized differently after fix
   - Verify compatibility with existing callers

### Unit Tests

- Test execute_strategy with MinimalCost strategy: Sort by fee, sequential processing
- Test execute_strategy with MinimalTime strategy: Batch processing, fail fast
- Test execute_strategy with Balanced strategy: Moderate batching, single retry
- Test token resolution: Symbol → Address mapping works correctly
- Test fee calculation: Actual fees captured and aggregated
- Test error cases: InsufficientBalance, TransferFailed, InvalidRecipient handled correctly
- Test ExecutionSummary: All fields updated correctly
- Test log_trade: Emits structured event with correct fields
- Test partial failure: Some trades succeed, some fail, summary reflects both

### Property-Based Tests

- **Strategy Property**: For any valid strategy and trade list, execute_strategy returns Ok(summary) where summary.total_trades == length(trades)
- **Fee Aggregation Property**: For any list of successful trades, summary.total_fees_paid == sum of individual fees
- **Error Recording Property**: For any failed trade, exactly one TradeError exists in summary.errors with correct trade_index
- **Preservation Property**: For any non-buggy input (dry_run, non-executed paths), behavior identical before/after fix
- **Result Consistency Property**: For any execution, summary.successful_trades + summary.failed_trades == summary.total_trades
- **Event Emission Property**: For each logged trade, exactly one TradeExecuted event emitted with correct fields

### Integration Tests

- Test full rebalance flow with execute_strategy: dry_run=false, transactions succeed
- Test rebalance with multiple strategies, verify distinct behaviors
- Test rebalance with partial trade failures, verify continuation to other batches
- Test rebalance with all trades failing, verify error summary returned
- Test rebalance with contract insufficient balance, verify errors recorded
- Test integration with shared::payments and shared::events modules
- Test event audit trail: Verify all logged trades appear in events, no trades missed

## Type System Changes

### New Types (in strategy_executor.rs or separate module)

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSummary {
    pub total_trades: u32,
    pub successful_trades: u32,
    pub failed_trades: u32,
    pub total_fees_paid: u128,
    pub total_slippage_realized: U256,
    pub errors: Vec<TradeError>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeError {
    InsufficientBalance {
        trade_index: u32,
        required: u128,
        available: u128,
    },
    TransferFailed {
        trade_index: u32,
        reason: Vec<u8>,
    },
    InvalidRecipient {
        trade_index: u32,
    },
    FeeCalculationError {
        trade_index: u32,
    },
    TokenResolutionError {
        trade_index: u32,
        symbol: Symbol,
    },
    Other {
        trade_index: u32,
        reason: Vec<u8>,
    },
}
```

**In logging.rs**:

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeExecuted {
    pub asset_pair: (Symbol, Symbol),
    pub amount: u128,
    pub actual_price: u128,
    pub fee: u128,
    pub recipient: Address,
    pub timestamp: u64,
    pub status: TradeStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeStatus {
    Success,
    PartialFill {
        executed_amount: u128,
        requested_amount: u128,
    },
    Failed {
        reason: Vec<u8>,
    },
}
```

### Modified Type Signatures

**execute_strategy**:
- From: `pub fn execute_strategy(env: &Env, _strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> bool`
- To: `pub fn execute_strategy(env: &Env, strategy: &ExecutionStrategy, trades: &Vec<Trade>) -> Result<ExecutionSummary, Error>`

**log_trade**:
- From: `pub fn log_trade(env: &Env, _trade: &Trade, _actual_price: u128, _fee: u128) -> ()`
- To: `pub fn log_trade(env: &Env, trade: &Trade, actual_price: u128, fee: u128, recipient: &Address, status: TradeStatus) -> ()`

### Types That Remain Unchanged

- Trade struct: `pub struct Trade { asset_pair: (Symbol, Symbol), amount: u128 }`
- ExecutionStrategy enum: `pub enum ExecutionStrategy { MinimalCost, MinimalTime, Balanced }`
- SimulationResult struct: `pub struct SimulationResult { expected_fees: u128, expected_slippage: U256 }`

## Integration Points

### In lib.rs rebalance() Function

**Current code**:
```rust
if !dry_run {
    execute_strategy(&env, &strategy, &trades);
}

let result = SimulationResult { ... };
emit_action_executed(env, ..., !dry_run, timestamp);
result
```

**After fix**:
```rust
if !dry_run {
    match execute_strategy(&env, &strategy, &trades) {
        Ok(execution_summary) => {
            // Log execution statistics
            // Optionally emit detailed execution events
        }
        Err(e) => {
            // Handle fatal error
            // Return error or convert to event
        }
    }
}

let result = SimulationResult { ... };
emit_action_executed(env, ..., !dry_run, timestamp);
result
```

**Key changes**:
- Capture Result from execute_strategy
- Handle Ok and Err cases
- Can emit additional events with execution summary
- Decide whether to propagate error or swallow it

### With predict_slippage (Unchanged)

The `predict_slippage` function is called during rebalance to calculate expected slippage:

```rust
for trade in trades.iter() {
    let slippage = predict_slippage(trade.asset_pair.clone(), trade.amount, &env);
    // ...
}
```

**After fix**: This code is unchanged. predict_slippage operates independently of execute_strategy.

### With calculate_total_fees (Unchanged)

The `calculate_total_fees` function is called to estimate expected fees:

```rust
let total_fees = calculate_total_fees(&trades);
```

**After fix**: This code is unchanged. calculate_total_fees operates independently of execute_strategy. (Note: actual fees are captured separately during execution in ExecutionSummary.total_fees_paid.)

### With SimulationResult (Unchanged)

SimulationResult struct and its return from rebalance is unchanged:

```rust
pub struct SimulationResult {
    pub expected_fees: u128,
    pub expected_slippage: U256,
}
```

**After fix**: This remains the primary return type from rebalance. ExecutionSummary is additional information, not a replacement.

### With shared::payments Module

**New Integration Point**:
- Call `safe_transfer_from_contract` for each trade execution
- Query token addresses for symbols (from oracle)
- Capture actual fees and execution prices from transfer response

### With shared::events Module

**New Event Emission**:
- Emit TradeExecuted event for each logged trade (via env.events().publish)
- Existing emit_action_executed calls continue unchanged for high-level action events

## Constants & Configuration

Define strategy-specific constants at module level:

```rust
// MinimalCost strategy constants
pub const MINIMAL_COST_BATCH_SIZE: usize = 1;          // One trade per transaction
pub const MINIMAL_COST_MAX_RETRIES: u32 = u32::MAX;    // Infinite retries for cost optimization

// MinimalTime strategy constants
pub const MINIMAL_TIME_BATCH_SIZE: usize = 50;         // Large batches for throughput
pub const MINIMAL_TIME_MAX_RETRIES: u32 = 0;           // Fail fast, no retries

// Balanced strategy constants
pub const BALANCED_BATCH_SIZE: usize = 15;             // Moderate batches
pub const BALANCED_MAX_RETRIES: u32 = 1;               // Single retry on transient errors
pub const BALANCED_FEE_OPTIMIZATION_ROUNDS: u32 = 1;   // Single fee optimization round

// Global error handling constants
pub const MAX_TRADE_FAILURES_ABORT: u32 = 1000;        // Abort if > N failures (safety valve)
pub const TOKEN_CACHE_SIZE: usize = 100;               // Cache recent token resolutions
pub const TRANSIENT_ERROR_BACKOFF_MS: u64 = 100;       // Initial backoff for retries (ms)
pub const TRANSIENT_ERROR_BACKOFF_MULTIPLIER: u64 = 2; // Exponential backoff multiplier
pub const MAX_BACKOFF_MS: u64 = 10_000;                // Maximum backoff (10 seconds)
```

**Usage in strategy execution**:

```rust
match strategy {
    ExecutionStrategy::MinimalCost => {
        for trade in trades_sorted.iter() {
            execute_trade_with_retries(trade, MINIMAL_COST_MAX_RETRIES, MINIMAL_COST_BATCH_SIZE)
        }
    }
    ExecutionStrategy::MinimalTime => {
        for batch in batches_of_size(trades, MINIMAL_TIME_BATCH_SIZE).iter() {
            execute_batch_with_retries(batch, MINIMAL_TIME_MAX_RETRIES)
        }
    }
    ExecutionStrategy::Balanced => {
        for batch in batches_of_size(trades, BALANCED_BATCH_SIZE).iter() {
            execute_batch_with_retries(batch, BALANCED_MAX_RETRIES)
        }
    }
}
```

## Documentation Requirements

### Module-Level Rustdoc

```rust
//! Strategy Executor Module
//!
//! Implements distinct execution strategies for token rebalancing trades.
//!
//! # Strategies
//!
//! - **MinimalCost**: Execute trades sequentially, prioritizing lowest execution cost.
//!   Accepts longer execution time. Use for large portfolio rebalancing where cost matters.
//!
//! - **MinimalTime**: Execute trades in large batches, prioritizing fastest execution time.
//!   Accepts higher execution cost. Use for time-sensitive arbitrage or emergency rebalancing.
//!
//! - **Balanced**: Execute trades in moderate batches with sensible retry logic.
//!   Balances cost and time. Use for default rebalancing when both matter.
//!
//! # Execution Result
//!
//! Returns `Result<ExecutionSummary, Error>` expressing:
//! - `Ok(summary)`: Execution completed (all or partial success); summary contains statistics
//! - `Err(error)`: Total failure; all trades failed or fatal error occurred
//!
//! # Example
//!
//! ```rust,ignore
//! let trades = vec![Trade { asset_pair: (from, to), amount: 1000 }];
//! let result = execute_strategy(&env, &ExecutionStrategy::Balanced, &trades)?;
//! println!("Executed {} trades, {} failed", result.successful_trades, result.failed_trades);
//! ```
```

### Function-Level Rustdoc

```rust
/// Executes a list of trades according to the specified strategy.
///
/// Implements distinct behaviors for each ExecutionStrategy variant:
/// - **MinimalCost**: Sort trades by fee ascending; sequential processing; infinite retries on transient errors
/// - **MinimalTime**: Batch trades aggressively (50+ per tx); fail fast on any error; no retries
/// - **Balanced**: Batch trades moderately (10-20 per tx); single retry on transient errors
///
/// For each trade, performs actual token transfer via `shared::payments::safe_transfer_from_contract`,
/// captures real execution prices and fees, and emits structured TradeExecuted events.
///
/// # Returns
///
/// `Ok(ExecutionSummary)` if execution completed (even partially):
/// - `successful_trades`: Number of trades that completed successfully
/// - `failed_trades`: Number of trades that failed
/// - `total_fees_paid`: Cumulative fees across all successful trades
/// - `errors`: Detailed error information for each failed trade
///
/// `Err(Error)` if total failure or fatal error occurs (e.g., strategy parsing failed)
///
/// # Errors
///
/// Returns error if strategy is invalid or fatal errors prevent execution start.
/// Per-trade errors are captured in ExecutionSummary.errors, not propagated.
///
/// # Panics
///
/// Does not panic. All errors are captured in ExecutionSummary.errors.
pub fn execute_strategy(
    env: &Env,
    strategy: &ExecutionStrategy,
    trades: &Vec<Trade>,
) -> Result<ExecutionSummary, Error>
```

```rust
/// Logs trade execution details to on-chain event.
///
/// Emits a structured `TradeExecuted` event containing:
/// - Trade details (asset_pair, amount)
/// - Execution metrics (actual_price, fee, recipient)
/// - Status (Success, PartialFill, Failed)
/// - Timestamp and recipient for audit trail
///
/// # Parameters
///
/// - `env`: Soroban environment
/// - `trade`: Trade being logged
/// - `actual_price`: Real execution price (NOT 0)
/// - `fee`: Real fee incurred (NOT 0)
/// - `recipient`: Address where tokens were transferred
/// - `status`: TradeStatus (Success/PartialFill/Failed)
pub fn log_trade(
    env: &Env,
    trade: &Trade,
    actual_price: u128,
    fee: u128,
    recipient: &Address,
    status: TradeStatus,
)
```

This design document provides a complete technical specification for fixing the strategy executor bug. It can now be used to generate implementation tasks in Phase 3.
