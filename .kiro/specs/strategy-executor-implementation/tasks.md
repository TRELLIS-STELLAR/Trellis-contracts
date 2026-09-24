# Strategy Executor Implementation Tasks

## Implementation Plan

- [x] 1. Write bug condition exploration test
  - **Property 1: Fault Condition** - Strategy Ignored, No Transfers, Bare Bool Return
  - **Status**: ✅ COMPLETE (Phase 1)
  - Written 4 bug exploration tests that detect all three bugs
  - Tests designed to fail on unfixed code, pass after fix
  - Report: `TASK1_BUG_EXPLORATION_REPORT.md`
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [x] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - Unchanged Behaviors (Dry-run, Structs, Enums, Module APIs)
  - **Status**: ✅ COMPLETE (Phase 2)
  - Written 7 preservation tests that verify no regressions
  - Tests pass on both fixed and unfixed code
  - Report: `TASK2_PRESERVATION_TESTS_REPORT.md`
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 3. Fix strategy executor implementation

  - [x] 3.1 Implement result type and ExecutionSummary struct
    - **Status**: ✅ COMPLETE (Phase 3.1)
    - Added `ExecutionSummary` struct with all required fields
    - Added `TradeStatus` enum (Success, PartialFill, Failed)
    - Added `TradeError` struct and `TradeErrorKind` enum
    - Changed function signature from `bool` to `Result<ExecutionSummary, Error>`
    - Updated lib.rs to import and use new types
    - Report: `TASK3_1_RESULT_TYPE_COMPLETION.md`
    - _Requirements: 2.6, 2.8_

  - [ ] 3.2 Implement token resolution and payment integration
    - Implement `resolve_token_address(env: &Env, symbol: Symbol) -> Result<Address, Error>` helper function
    - For each trade in execution loop, call `resolve_token_address` to convert Symbol to Address
    - Implement `resolve_recipient_from_context(env: &Env, trade: &Trade) -> Result<Address, Error>`
    - Call `shared::payments::safe_transfer_from_contract(env, token_address, recipient, amount)` for each trade
    - Capture actual execution price and fee from transfer result
    - On transfer success: Update ExecutionSummary.successful_trades, total_fees_paid
    - On transfer failure: Record TradeError in ExecutionSummary.errors vector
    - _Bug_Condition: C(X) where no calls to safe_transfer_from_contract occur_
    - _Expected_Behavior: P(X) where safe_transfer_from_contract is called for each trade with real amounts and captures actual prices/fees_
    - _Preservation: Preserve Trade struct usage patterns (3.4)_
    - _Requirements: 2.4, 2.5, 2.6_

  - [ ] 3.3 Implement MinimalCost strategy
    - Sort trades by estimated fee percentage in ascending order (cheapest first)
    - Execute trades sequentially (batch size = 1)
    - Implement infinite retry loop for transient errors
    - On permanent errors: Record in ExecutionSummary.errors and continue to next trade
    - Add conditional logic: `if matches!(strategy, ExecutionStrategy::MinimalCost) { ... }`
    - _Bug_Condition: C(X) where strategy parameter is ignored_
    - _Expected_Behavior: P(X) where MinimalCost produces distinct behavior: sequential execution, cost optimization, retry indefinitely_
    - _Requirements: 2.1, 2.4, 2.5_

  - [ ] 3.4 Implement MinimalTime strategy
    - Group trades by asset type to minimize context switches
    - Create batches with maximum batch size (50+ trades per transaction)
    - Execute batches sequentially with fail-fast on any error
    - Add conditional logic: `if matches!(strategy, ExecutionStrategy::MinimalTime) { ... }`
    - _Bug_Condition: C(X) where strategy parameter is ignored_
    - _Expected_Behavior: P(X) where MinimalTime produces distinct behavior: batched execution, fail-fast, high throughput_
    - _Requirements: 2.2, 2.4, 2.5_

  - [ ] 3.5 Implement Balanced strategy
    - Process trades in moderate batches (10-20 trades per transaction)
    - Respect caller ordering of trades (no sorting)
    - Implement single-retry logic on transient errors
    - Continue processing remaining batches even if one batch fails
    - Add conditional logic: `if matches!(strategy, ExecutionStrategy::Balanced) { ... }`
    - _Bug_Condition: C(X) where strategy parameter is ignored_
    - _Expected_Behavior: P(X) where Balanced produces distinct behavior: moderate batching, single retry, continue on error_
    - _Requirements: 2.3, 2.4, 2.5_

  - [ ] 3.6 Fix log_trade function to emit real prices and fees
    - Remove underscore prefixes from `log_trade` parameters
    - Add TradeStatus parameter to function signature
    - Create TradeExecuted struct with fields: asset_pair, amount, actual_price, fee, recipient, timestamp, status
    - Emit structured TradeExecuted event using `env.events().publish(("trade_executed",), trade_event)`
    - Verify actual_price and fee are real values (NOT 0) passed from caller
    - Emit high-level action event via `emit_action_executed` for compatibility
    - _Bug_Condition: C(X) where log_trade discards parameters and emits generic events with no trade details_
    - _Expected_Behavior: P(X) where log_trade emits structured events with real asset_pair, amount, actual_price, fee, recipient_
    - _Requirements: 2.5, 2.7_

  - [ ] 3.7 Add comprehensive error handling
    - Wrap each trade execution in Result match block
    - Categorize errors as TradeErrorKind variants (InsufficientBalance, TransferFailed, InvalidRecipient, FeeCalculationError, TokenResolutionError, Other)
    - Accumulate all errors in ExecutionSummary.errors vector
    - Implement per-strategy retry logic (MinimalCost: infinite, Balanced: once, MinimalTime: none)
    - Return Ok(ExecutionSummary) even if failed_trades > 0 (partial success is valid)
    - Return Err(Error) only if catastrophic failure before any trades execute
    - _Bug_Condition: C(X) where trade failures are silently swallowed and return true unconditionally_
    - _Expected_Behavior: P(X) where failures are captured and returned in ExecutionSummary.errors_
    - _Requirements: 2.6, 2.8_

- [ ] 4. Verify bug condition exploration test now passes
  - **Property 1: Expected Behavior** - Strategy Honored, Real Transfers, Structured Result
  - **Status**: ⏳ PENDING (Phase 4)
  - Will re-run the SAME tests from task 1 after implementing fixes in tasks 3.2-3.7
  - Test that MinimalCost and MinimalTime produce DISTINCT behavior
  - Test that `execute_strategy` returns Result<ExecutionSummary, Error> (already done in task 3.1)
  - Test that `log_trade` is called with real actual_price and fee (not 0)
  - Test that on successful trade: ExecutionSummary shows successful_trades=1, failed_trades=0
  - Test that on failed trade: ExecutionSummary shows failed_trades=1 with error details
  - **EXPECTED OUTCOME**: All tests PASS (confirms bug is fixed)
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

- [ ] 5. Verify preservation tests still pass
  - **Property 2: Preservation** - Unchanged Behaviors Maintained
  - **Status**: ⏳ PENDING (Phase 5)
  - Will re-run the SAME preservation tests from task 2 after implementing fixes
  - Verify dry_run semantics unchanged
  - Verify Trade struct: Remains as `(Symbol, Symbol) asset_pair, u128 amount`
  - Verify ExecutionStrategy enum: Still has exactly three variants
  - Verify no regressions introduced
  - **EXPECTED OUTCOME**: All 7 preservation tests PASS (confirms no regressions)
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 6. Acceptance criteria validation
  - **Status**: ⏳ PENDING (Phase 6)
  - **Criterion 1**: Each strategy variant produces observably different behavior
    - Test MinimalCost: Trades sorted by fee, executed sequentially
    - Test MinimalTime: Trades batched aggressively, fail-fast on error
    - Test Balanced: Trades in moderate batches, single retry on transient error
  - **Criterion 2**: A failed trade is reported to caller rather than swallowed
    - Create scenario where one trade fails
    - Confirm ExecutionSummary.errors contains error details
  - **Criterion 3**: Emitted events carry trade, price and fee (not hardcoded 0,0)
    - Execute successful trade and capture TradeExecuted event
    - Verify event.actual_price > 0 and event.fee is correct
  - **Criterion 4**: Tests cover every variant plus the failure path
    - Confirm all three strategies tested
    - Confirm error cases covered
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

- [ ] 7. Integration testing - end-to-end workflows
  - **Status**: ⏳ PENDING (Phase 7)
  - Test full rebalance workflow with execute_strategy integration:
    - Dry-run path: execute_strategy not called, no transfers
    - Success path: All trades execute, events emitted, ExecutionSummary reflects success
    - Partial failure path: Some trades fail, ExecutionSummary shows mixed results
  - Test strategy selection end-to-end across all three strategy variants
  - Test event audit trail: Verify TradeExecuted events have real prices/fees
  - Test error recovery: Verify per-strategy retry logic (infinite, once, none)
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 8. Final checkpoint - complete validation
  - **Status**: ⏳ PENDING (Phase 8)
  - Compile code and verify no syntax errors or warnings
  - Run all unit tests: Confirm exploration, preservation, and acceptance criteria tests all pass
  - Run all integration tests: Confirm full workflow tests pass
  - Audit event emissions: Query all TradeExecuted events and verify real prices/fees
  - Audit error handling: Verify all error paths record details in ExecutionSummary.errors
  - Confirm strategy distinctiveness: Verify MinimalCost, MinimalTime, Balanced produce different patterns
  - Document final status: All requirements satisfied, all tests passing, no regressions
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

