# Implementation Plan: Rebalancer Slippage Predictor

## Overview

This implementation plan converts the design into discrete coding tasks that build incrementally. Each task focuses on writing, modifying, or testing code for the constant-product AMM slippage predictor. The flow is: foundation (core implementation), testing (unit and property-based), robustness (edge case verification), documentation, integration, and final validation.

The slippage predictor will replace a placeholder implementation with a fully functional module that calculates slippage based on trade size and liquidity depth using the constant-product invariant `reserve_a × reserve_b = k`.

---

## Tasks

- [x] 1. Set up module structure and mock liquidity storage
  - Create module-level rustdoc template with constant-product model explanation
  - Define data structures for liquidity reserves (reserve_a, reserve_b, timestamp, is_active)
  - Implement storage key format for asset pairs (namespace + pair serialization)
  - Create helper function `get_liquidity_reserves(pair, env) -> Result<(i128, i128), Error>`
  - Create helper function to set liquidity data for test fixtures
  - Populate at least 3 test fixture pairs with known liquidity (e.g., 1M/1M, thin pair, deep pair)
  - Verify module compiles without errors
  - _Requirements: 11.1, 11.2, 11.4_

- [x] 2. Implement core slippage calculation function
  - Implement `calculate_slippage_bps(trade_amount: i128, reserve_a: i128, reserve_b: i128) -> Result<i128, Error>`
  - Use formula: `slippage_bps = (trade_amount / (reserve_a + trade_amount)) × 10_000`
  - Implement fixed-point arithmetic: multiply by 10_000 before division to preserve precision
  - Use `checked_add`, `checked_mul`, `checked_div` from `shared::math` for all operations
  - Clamp result to [0, MAX_SLIPPAGE] using `.min(MAX_SLIPPAGE).max(0)`
  - Define constants: `BPS_DENOMINATOR = 10_000`, `MAX_SLIPPAGE = 10_000_000`, `MIN_LIQUIDITY = 1`
  - Handle overflow cases: return `MAX_SLIPPAGE` on any checked operation error
  - Include rustdoc with parameter descriptions and example calculation
  - Verify function compiles and type checks correctly
  - _Requirements: 4.1, 4.2, 4.3, 5.1, 5.2_

- [x] 3. Implement main predict_slippage function with error handling
  - Replace placeholder implementation of `predict_slippage(asset_pair, amount, env) -> U256`
  - Implement validation step: if `amount = 0`, return `U256(0)` immediately
  - Convert `u128 amount` to `i128` safely: on overflow (amount > i128::MAX), return `MAX_SLIPPAGE`
  - Call `get_liquidity_reserves` to retrieve reserves; on error or invalid reserves (≤ 0), return `MAX_SLIPPAGE`
  - Call `calculate_slippage_bps` to compute slippage; on error, return `MAX_SLIPPAGE`
  - Convert final `i128` result to `U256` using `U256::from_i128(env, slippage_bps)` or fallback
  - Verify all error paths return bounded values instead of panicking
  - Add comprehensive rustdoc explaining the flow, error handling, and output unit (basis points)
  - Include worked example with concrete numbers (e.g., 1K USDC on 1M/1M reserves)
  - _Requirements: 1.1, 1.2, 1.4, 1.5, 6.1, 6.2, 6.3, 7.1, 7.2, 13.1, 13.2, 13.3, 13.4_

- [x] 4. Verify type conversions and overflow safety
  - Audit all arithmetic operations to confirm `checked_add`, `checked_mul`, `checked_div` are used
  - Verify u128 → i128 conversion handles u128::MAX gracefully (returns MAX_SLIPPAGE, no panic)
  - Verify i128 → U256 conversion is safe after clamping
  - Test zero reserves (0, 0) returns MAX_SLIPPAGE without division-by-zero panic
  - Test trade_amount = u128::MAX with small reserves returns MAX_SLIPPAGE
  - Test minimal reserves (1, 1) compute without panic
  - Confirm all error paths return bounded values in valid U256 range
  - Document overflow handling strategy in module rustdoc
  - _Requirements: 5.2, 5.3, 5.4, 5.5, 16.1_

- [x] 5. Write unit tests for boundary and normal cases
  - [x]* 5.1 Test zero trade amount
    - Input: trade_amount = 0, any valid reserves
    - Expected: return `U256` representing 0 basis points
    - Validates: Requirements 6.1, 6.2, 6.3
    - **Property 3: Zero Trade Amount Returns Zero Slippage**
  
  - [x] 5.2 Test small trade on deep liquidity
    - Input: trade_amount = 100 on pair (1M, 1M)
    - Expected: slippage < 100 basis points (low impact)
    - Calculation: 100 / (1M + 100) × 10k ≈ 1 bp (correct)
    - Validates: Requirements 2.1, 2.3, 14.1
  
  - [x] 5.3 Test medium trade on typical liquidity
    - Input: trade_amount = 10K on pair (1M, 1M)
    - Expected: slippage 100–200 basis points (noticeable)
    - Calculation: 10K / (1M + 10K) × 10k ≈ 100 bp (expected)
    - Validates: Requirements 2.1, 2.2, 14.1
  
  - [x] 5.4 Test large trade on typical liquidity
    - Input: trade_amount = 500K on pair (1M, 1M)
    - Expected: slippage > 5000 basis points (high impact)
    - Calculation: 500K / (1.5M) × 10k ≈ 3333 bp (expected)
    - Validates: Requirements 2.1, 2.2, 15.1, 15.2
  
  - [x] 5.5 Test unknown asset pair
    - Input: asset pair not in liquidity storage
    - Expected: return `MAX_SLIPPAGE` (10M basis points)
    - Validates: Requirements 7.1, 7.2, 7.3
  
  - [x] 5.6 Test zero reserves
    - Input: pair with reserve_a = 0 or reserve_b = 0
    - Expected: return `MAX_SLIPPAGE`, no panic
    - Validates: Requirements 5.4, 8.2, 16.3
  
  - [x] 5.7 Test minimal liquidity (1, 1)
    - Input: trade_amount = 1 on pair (1, 1)
    - Expected: compute without panic, return valid slippage
    - Calculation: 1 / (1 + 1) × 10k = 5000 bp (correct)
    - Validates: Requirements 5.5, 16.2

- [x] 6. Write unit tests for edge cases and overflow scenarios
  - [x] 6.1 Test u128::MAX with small reserves
    - Input: trade_amount = u128::MAX, reserves (1000, 1000)
    - Expected: return `MAX_SLIPPAGE`, no overflow panic
    - Validates: Requirements 5.2, 5.5, 16.1
    - **Property 4: Non-Panic Guarantee for All Valid Inputs**
  
  - [x] 6.2 Test insufficient liquidity (trade ≥ reserve)
    - Input: trade_amount = 1M on pair (1M, 1M)
    - Expected: slippage ≥ 5000 basis points or MAX_SLIPPAGE
    - Calculation: 1M / (1M + 1M) × 10k = 5000 bp (threshold)
    - Validates: Requirements 8.1, 16.4
    - **Property 6: Insufficient Liquidity Detection**
  
  - [x] 6.3 Test multiple pairs with relative ordering
    - Input: two pairs with different liquidity, same trade amount
    - Pair A (deep): 10M / 10M reserves, trade 1K
    - Pair B (thin): 100K / 100K reserves, trade 1K
    - Expected: slippage_B > slippage_A
    - Validates: Requirements 3.1, 3.3
    - **Property 2: Slippage Varies with Liquidity Depth**

- [x] 7. Implement property-based tests for correctness properties
  - [x]* 7.1 Write property test: Monotonicity with trade size (Property 1)
    - **Property 1: Slippage Monotonicity with Trade Size**
    - **Validates: Requirements 2.1, 2.3**
    - For fixed pair, if amt_1 < amt_2, then slippage(amt_1) ≤ slippage(amt_2)
    - Use proptest to generate: amt_1 ∈ [1, 100B], amt_2 ∈ [1, 100B], reserves ∈ [1, i128::MAX]
    - Run ≥ 100 iterations to verify monotonicity invariant
    - Tag with comment: `Feature: rebalancer-slippage-predictor, Property 1: Slippage Monotonicity`
  
  - [x]* 7.2 Write property test: Liquidity depth sensitivity (Property 2)
    - **Property 2: Slippage Varies with Liquidity Depth**
    - **Validates: Requirements 3.1**
    - For same trade amount, pair_A with deeper reserves has lower slippage than pair_B
    - Metamorphic: if reserve_A > reserve_B (both components), then slippage(pair_B, amt) > slippage(pair_A, amt)
    - Use proptest with constrained reserve generation
    - Run ≥ 100 iterations
    - Tag: `Property 2: Liquidity Depth Sensitivity`
  
  - [x]* 7.3 Write property test: Zero amount returns zero (Property 3)
    - **Property 3: Zero Trade Amount Returns Zero Slippage**
    - **Validates: Requirements 6.1, 6.2**
    - For any valid reserves, predict_slippage(pair, 0) = 0
    - Use proptest to generate reserves ∈ [1, i128::MAX]
    - Run ≥ 100 iterations
    - Tag: `Property 3: Zero Trade Slippage`
  
  - [x]* 7.4 Write property test: Non-panic for all valid inputs (Property 4)
    - **Property 4: Non-Panic Guarantee for All Valid Inputs**
    - **Validates: Requirements 5.2, 5.5, 16.1, 16.5**
    - For any trade_amount ∈ [0, u128::MAX] and any asset pair, predict_slippage completes without panicking
    - Adversarial inputs: u128::MAX, u128::MIN (0), i128::MAX reserves, i128::MIN reserves (if applicable)
    - Use proptest to generate unbounded u128 and i128 values
    - Run ≥ 100 iterations; verify all return valid U256 or MAX_SLIPPAGE
    - Tag: `Property 4: Non-Panic Guarantee`
  
  - [x]* 7.5 Write property test: Invalid pair distinguishability (Property 5)
    - **Property 5: Invalid Pair Distinguishability**
    - **Validates: Requirements 7.3**
    - Unknown pair returns value materially different (MAX_SLIPPAGE) from valid pair small slippage (< 100 bp)
    - Compare: unknown_pair_result vs valid_pair_small_trade_result
    - Generate valid trades that produce < 100 bp slippage; verify unknown pair ≠ that result
    - Run ≥ 50 iterations
    - Tag: `Property 5: Invalid Pair Distinguishability`
  
  - [x]* 7.6 Write property test: Insufficient liquidity detection (Property 6)
    - **Property 6: Insufficient Liquidity Detection**
    - **Validates: Requirements 8.1**
    - If trade_amount ≥ reserve_a, then predicted slippage ≥ 5000 basis points
    - Use proptest: generate reserves, then generate trade_amount ≥ reserve_a
    - Verify slippage result ≥ 5000 bp or MAX_SLIPPAGE
    - Run ≥ 100 iterations
    - Tag: `Property 6: Insufficient Liquidity`
  
  - [x]* 7.7 Write property test: Constant-product invariant (Property 7)
    - **Property 7: Constant-Product Formula Validation**
    - **Validates: Requirements 4.1, 4.2**
    - Verify invariant: (reserve_a + trade_amt) × (reserve_b - execution) ≈ reserve_a × reserve_b
    - Calculate execution_amt = reserve_b × trade_amt / (reserve_a + trade_amt)
    - Check product after trade ≈ original product (within rounding tolerance)
    - Use proptest to generate reserves and amounts
    - Run ≥ 100 iterations; allow ±1 rounding tolerance due to fixed-point arithmetic
    - Tag: `Property 7: Constant-Product Invariant`
  
  - [x]* 7.8 Write property test: Output bounded and deterministic (Property 8)
    - **Property 8: Output Bounded and Deterministic**
    - **Validates: Requirements 9.1, 9.2**
    - For same inputs, predict_slippage always returns identical U256 (determinism)
    - Output is always in range [0, MAX_SLIPPAGE] (basis points bounded)
    - Call predict_slippage(pair, amt) three times with same inputs; verify identical output
    - Use proptest to generate various inputs and verify bounds
    - Run ≥ 100 iterations
    - Tag: `Property 8: Output Bounded & Deterministic`

- [x] 8. Checkpoint — Verify all unit and property tests pass
  - Run full test suite: `cargo test --package rebalancer-contract`
  - Ensure all tests pass (both unit and property-based)
  - Verify no test panics or timeouts
  - Confirm property tests run ≥ 100 iterations each without failure
  - Ask the user if questions arise or if adjustments to test thresholds are needed
  - _Requirements: 5.5, 16.1, 16.5_

- [x] 9. Add comprehensive rustdoc and module documentation
  - [x] 9.1 Document the constant-product AMM model in module rustdoc
    - Explain invariant: `reserve_a × reserve_b = k`
    - Derive execution amount formula and slippage formula step-by-step
    - Include mathematical notation: `slippage = amount / (reserve_a + amount)`
    - Document basis points: 1 bp = 0.01%, 10,000 bp = 100%
    - Validate: Requirements 4.1, 4.3, 13.1, 13.2, 13.4
  
  - [x] 9.2 Include worked example in module rustdoc
    - Example: Trade 1,000 USDC on (1M USDC, 100M XLM) pair
    - Show step-by-step calculation: 1,000 / (1,000,000 + 1,000) × 10,000 ≈ 10 bp
    - Explain output: trader receives ~0.1% less XLM than spot price
    - Validate: Requirements 13.4
  
  - [x] 9.3 Document edge cases and error handling strategy
    - Document all error conditions: zero amount, unknown pair, zero reserves, overflow
    - Explain: all errors return MAX_SLIPPAGE (signal to rebalancer: risky/invalid)
    - Reference error handling table from design
    - Validate: Requirements 5.1, 7.1, 8.1, 13.1
  
  - [x] 9.4 Document assumptions and limitations
    - Symmetric reserves, single-hop trades, fixed-point arithmetic precision
    - Known limitations: no multi-hop, delayed liquidity data, no fee modeling
    - Validate: Requirements 13.3, 13.5
  
  - [x] 9.5 Document function parameters and return values
    - `predict_slippage(asset_pair, amount, env) -> U256`
    - Parameter docs: asset_pair ordered tuple, amount in base asset units, env for contract context
    - Return: U256 representing basis points (range [0, MAX_SLIPPAGE])
    - Validate: Requirements 1.1, 1.2, 9.1, 9.2, 13.3

- [x] 10. Integration testing with rebalancer contract
  - [x] 10.1 Verify slippage_predictor integration in lib.rs
    - Confirm rebalancer::rebalance can call predict_slippage without errors
    - Verify existing accumulation logic in lib.rs works with non-constant slippage
    - Check dry_run logic handles variable slippage correctly
    - Compile and verify no type errors
    - Validate: Requirements 1.3, 1.4, 1.5, 9.4
  
  - [x] 10.2 Test rebalancer accumulating slippage from multiple trades
    - Execute dry_run with 3+ trades on different asset pairs
    - Verify each trade gets different slippage prediction (not constant)
    - Verify accumulated total is sum of individual slippages
    - Compare: small trade slippage < large trade slippage on same pair
    - Validate: Requirements 2.1, 2.2, 3.1, 12.1, 12.2, 12.4
  
  - [x] 10.3 Verify integration with fee_calculator and strategy_executor
    - Confirm predict_slippage output type (U256) is compatible with fee calculations
    - Test that strategy_executor receives correct slippage-aware trade decisions
    - Validate: Requirements 1.3, 1.4, 1.5

- [x] 11. Acceptance criteria validation and spot-checks
  - [x] 11.1 Verify Requirement 2.1: Slippage monotonicity
    - Test small (1K), medium (100K), large (500K) trades on same pair
    - Confirm: slippage_1K < slippage_100K < slippage_500K
    - Validate: Requirements 2.1
  
  - [x] 11.2 Verify Requirement 3.1: Liquidity depth sensitivity
    - Compare slippage for same trade on deep pair vs thin pair
    - Example: 1K trade on (1M, 1M) vs (1K, 1K)
    - Confirm: slippage_thin > slippage_deep
    - Validate: Requirements 3.1
  
  - [x] 11.3 Verify Requirement 12.1–12.4: No systematic mispricing
    - Spot-check: small trade (1% of reserve) produces < 100 bp slippage
    - Spot-check: large trade (50% of reserve) produces > 5000 bp slippage
    - Confirm: results are repeatable and not constant across all inputs
    - Validate: Requirements 12.1, 12.2, 12.4
  
  - [x] 11.4 Verify Requirement 5.5: All tests pass without panicking
    - Run full test suite once more
    - Confirm all tests pass, property tests complete ≥ 100 iterations each
    - Verify no timeouts or hangs
    - Validate: Requirements 5.5, 16.1, 16.5
  
  - [x] 11.5 Verify Requirement 13.1–13.5: Documentation complete
    - Check module rustdoc exists and explains constant-product model
    - Confirm function documentation includes parameters, return values, and examples
    - Verify edge cases and assumptions are documented
    - Validate: Requirements 13.1, 13.2, 13.3, 13.4, 13.5

- [x] 12. Final checkpoint — Ensure all tests pass and feature is complete
  - Run full rebalancer contract test suite: `cargo test --package rebalancer-contract`
  - Confirm all unit tests pass
  - Confirm all property-based tests pass (≥ 100 iterations each)
  - Verify compilation with no warnings (or document acceptable warnings)
  - Ask the user if questions arise or if any adjustments needed before handoff
  - _Requirements: 5.5, 16.1, 16.5, 1.5_

---

## Notes

- **Implementation Language**: Rust (Soroban contracts)
- **Testing Frameworks**: Rust built-in `#[test]` for unit tests, `proptest` for property-based tests
- **Math Library**: All arithmetic uses `shared::math::checked_add`, `checked_mul`, `checked_div` for overflow safety
- **Error Handling**: All error paths return bounded values (MAX_SLIPPAGE or Error), never panic
- **Correctness Properties**: 8 properties are validated via property-based tests (tasks 7.1–7.8)
- **Optional Tasks**: Tasks marked with `*` are optional test-related sub-tasks; core implementation tasks (1–4, 9–12) are required
- **Acceptance Criteria**: Each task references specific requirements from the requirements document
- **Checkpoints**: Tasks 8 and 12 serve as integration and validation checkpoints

---

## Traceability

| Property | Test Task | Requirements |
|----------|-----------|--------------|
| 1: Monotonicity | 7.1 | 2.1, 2.3 |
| 2: Liquidity Depth | 7.2 | 3.1 |
| 3: Zero Amount | 7.3 | 6.1, 6.2 |
| 4: Non-Panic | 7.4 | 5.2, 5.5, 16.1, 16.5 |
| 5: Invalid Pair Distinguishability | 7.5 | 7.3 |
| 6: Insufficient Liquidity | 7.6 | 8.1 |
| 7: Constant-Product Invariant | 7.7 | 4.1, 4.2 |
| 8: Output Bounded & Deterministic | 7.8 | 9.1, 9.2 |

