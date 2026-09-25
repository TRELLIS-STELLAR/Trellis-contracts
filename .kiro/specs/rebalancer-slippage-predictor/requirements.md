# Requirements Document: Rebalancer Slippage Predictor

## Introduction

The Slippage Predictor module is responsible for estimating the price slippage that will occur when executing trades on a constant-product automated market maker (AMM). Currently, it is a placeholder that returns a constant value regardless of asset pair or trade size, making rebalancer decisions meaningless. This requirements document defines the expected behavior of a working slippage predictor that accounts for both trade size and liquidity depth to provide accurate slippage estimates for the rebalancer's decision-making process.

The slippage predictor must integrate with the oracle contract to obtain liquidity depth information for asset pairs, calculate slippage using a constant-product AMM model, and safely handle edge cases including zero amounts, unknown pairs, and insufficient liquidity.

## Glossary

- **Slippage**: The difference between the expected execution price and the actual execution price when a trade is executed on an AMM, expressed as a percentage or basis points
- **Asset_Pair**: An ordered pair of symbols representing two tradeable assets (e.g., (USDC, XLM))
- **Trade_Amount**: The quantity of the base asset (first symbol) being traded on the AMM, represented as a u128
- **Liquidity_Depth**: The amount of reserve assets in the AMM's liquidity pools for a given asset pair
- **Constant_Product_AMM**: An AMM model where the product of reserves remains constant: reserve_a × reserve_b = k (e.g., Uniswap v2)
- **Execution_Price**: The actual price received per unit of asset when executing a trade
- **Expected_Price**: The reference price obtained before executing a trade (typically from oracle or spot price)
- **Fixed_Point_Arithmetic**: Arithmetic using scaled integers to represent fractional values (e.g., i128 scaled by 10,000 for basis points)
- **Overflow**: An arithmetic operation where the result exceeds the maximum representable value (i128::MAX or U256 limits)
- **Division_by_Zero**: An arithmetic operation attempting to divide by zero, which is undefined
- **Slippage_Predictor**: The module that computes slippage estimates given an asset pair and trade amount
- **Oracle_Contract**: An external contract that provides liquidity depth and other reference data for asset pairs
- **Rebalancer**: The contract that uses slippage predictions to decide whether to execute trades and accumulates total slippage across multiple trades

## Requirements

### Requirement 1: Slippage Predictor Basic Interface

**User Story:** As a rebalancer contract, I want to call a slippage predictor function, so that I can estimate the cost of trading a specific asset pair at a specific amount.

#### Acceptance Criteria

1. THE Slippage_Predictor SHALL accept an asset_pair (tuple of two Symbols) and a trade_amount (u128) as inputs
2. THE Slippage_Predictor SHALL return a U256 representing the estimated slippage
3. THE Slippage_Predictor SHALL be callable from the rebalancer contract without requiring authentication or special permissions
4. THE Slippage_Predictor function signature SHALL match the existing predict_slippage function in slippage_predictor.rs
5. WHERE the function is called with valid inputs, THE Slippage_Predictor SHALL return immediately without blocking or requiring external contract calls to fail

### Requirement 2: Slippage Varies with Trade Size

**User Story:** As a rebalancer, I want slippage predictions to increase with larger trade amounts, so that I accurately represent the cost of large trades against limited liquidity.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor is called with the same asset pair but different trade amounts, THE predicted slippage SHALL increase monotonically as trade_amount increases
2. WHEN the Slippage_Predictor is called with a small trade (e.g., 1% of available liquidity), THE predicted slippage SHALL be materially lower than a large trade (e.g., 50% of available liquidity) for the same pair
3. WHEN the Slippage_Predictor is called with a very small trade amount relative to liquidity depth, THE predicted slippage SHALL approach zero

### Requirement 3: Slippage Varies with Asset Pair Liquidity Depth

**User Story:** As a rebalancer, I want slippage predictions to reflect the actual liquidity available for each asset pair, so that thin markets show higher slippage than deep markets.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor is called with two asset pairs where pair_A has deeper liquidity than pair_B, and both are traded with the same amount, THE predicted slippage for pair_B SHALL be materially higher than pair_A
2. WHEN the Slippage_Predictor retrieves liquidity depth for an asset pair, THE liquidity depth data SHALL come from the oracle contract or another authoritative source
3. WHERE the Slippage_Predictor cannot retrieve liquidity depth for an asset pair, THE function SHALL return a high slippage value or an error rather than a misleading low value

### Requirement 4: Constant-Product AMM Model Implementation

**User Story:** As a designer of the slippage predictor, I want to use a mathematically sound model for slippage, so that predictions are accurate and defensible.

#### Acceptance Criteria

1. THE Slippage_Predictor model SHALL use the constant-product formula: reserve_a × reserve_b = k
2. THE Slippage_Predictor SHALL calculate the output amount using: output = reserve_b × (1 − 1 / (1 + trade_amount / reserve_a))
3. THE Slippage_Predictor SHALL estimate slippage as: slippage = (expected_price − execution_price) / expected_price, expressed as basis points
4. THE Slippage_Predictor SHALL document the mathematical model, assumptions, and limitations in module-level rustdoc comments

### Requirement 5: Safe Arithmetic Operations

**User Story:** As a security-conscious developer, I want slippage calculations to prevent overflow and division-by-zero errors, so that no trade can crash the rebalancer.

#### Acceptance Criteria

1. THE Slippage_Predictor SHALL use checked arithmetic operations (checked_add, checked_mul, checked_div) from shared/src/math.rs for all calculations
2. WHEN an arithmetic operation would overflow, THE Slippage_Predictor SHALL return an error or a bounded slippage value (e.g., maximum safe slippage) rather than panicking or wrapping
3. WHEN the Slippage_Predictor attempts to divide by zero, THE function SHALL return an error or a bounded slippage value rather than panicking
4. WHEN reserve_a or reserve_b is zero (indicating no liquidity), THE Slippage_Predictor SHALL return an error or maximum slippage value rather than failing
5. FOR ALL valid inputs (trade_amount ≤ u128::MAX, valid asset pair), THE Slippage_Predictor SHALL always return a valid result without panicking or overflowing

### Requirement 6: Zero Trade Amount Handling

**User Story:** As a rebalancer, I want the predictor to handle zero trade amounts gracefully, so that I can safely call it without special validation.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor is called with trade_amount = 0, THE function SHALL return slippage = 0 (or equivalent)
2. WHEN trade_amount = 0, THE function SHALL not attempt to divide by zero or perform invalid operations
3. WHEN trade_amount = 0, THE function SHALL complete successfully and return a valid U256 value

### Requirement 7: Unknown or Unsupported Asset Pair Handling

**User Story:** As a rebalancer, I want clear behavior when an asset pair is not recognized or not supported, so that I can log meaningful errors and make fallback decisions.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor is called with an asset pair not available in the oracle, THE function SHALL return a maximum slippage value, an error code, or an error status
2. WHEN an unknown asset pair is encountered, THE Slippage_Predictor SHALL not crash or return an arbitrary value
3. WHEN an unknown asset pair is encountered, THE error or maximum slippage value SHALL be distinguishable from a valid small slippage prediction

### Requirement 8: Insufficient Liquidity Edge Case

**User Story:** As a rebalancer, I want to know when a trade cannot be executed due to insufficient liquidity, so that I can avoid attempting trades that would fail.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor is called with a trade_amount greater than or equal to the available reserve in the AMM, THE function SHALL return maximum slippage or an error indicating insufficient liquidity
2. WHEN insufficient liquidity exists, THE Slippage_Predictor SHALL not return a normal slippage value that might mislead the rebalancer
3. WHERE sufficient liquidity exists, THE Slippage_Predictor SHALL return a valid slippage estimate

### Requirement 9: Output Type and Precision

**User Story:** As a rebalancer that accumulates slippage predictions, I want a numeric type that can represent slippage across multiple trades without precision loss, so that I can reliably track total costs.

#### Acceptance Criteria

1. THE Slippage_Predictor SHALL return a U256 type for all slippage predictions
2. THE U256 value SHALL represent slippage in basis points (1/10,000 of a percentage) or a consistent unit across all predictions
3. THE Slippage_Predictor output unit (basis points or other) SHALL be clearly documented in the function's rustdoc
4. THE U256 output SHALL be compatible with rebalancer's accumulation logic in lib.rs

### Requirement 10: Integration with Shared Math Module

**User Story:** As a maintainer of the codebase, I want the slippage predictor to use shared arithmetic utilities, so that calculations are consistent with other contracts and error handling is unified.

#### Acceptance Criteria

1. THE Slippage_Predictor SHALL use functions from shared/src/math.rs (checked_add, checked_mul, checked_div) for all fixed-point arithmetic
2. THE Slippage_Predictor SHALL propagate or handle Error types returned by shared math functions appropriately
3. THE Slippage_Predictor SHALL not duplicate arithmetic logic already available in the shared module
4. WHERE shared math functions handle i128, THE Slippage_Predictor SHALL convert u128 inputs to i128 safely or use appropriate scaling

### Requirement 11: Liquidity Depth Oracle Integration

**User Story:** As the slippage predictor, I want to retrieve current liquidity depth data, so that my predictions reflect real market conditions.

#### Acceptance Criteria

1. WHEN the Slippage_Predictor needs liquidity depth for a trade, THE function SHALL query or access data from the oracle contract (or a designated data source)
2. THE Slippage_Predictor SHALL cache or reuse liquidity depth data within a single function call if the same pair is queried multiple times
3. WHERE the oracle contract does not provide liquidity depth data, THE slippage predictor design SHALL explicitly define an alternative data source or fallback behavior
4. THE Slippage_Predictor SHALL document the oracle contract interface or data structure it expects

### Requirement 12: No Systematic Mispricing of Trades

**User Story:** As a rebalancer operator, I want accurate slippage predictions so that I can make profitable rebalancing decisions, not systematically lose value.

#### Acceptance Criteria

1. WHEN a trade is small relative to liquidity (e.g., < 1% of reserve), THE Slippage_Predictor SHALL predict low slippage
2. WHEN a trade is large relative to liquidity (e.g., > 25% of reserve), THE Slippage_Predictor SHALL predict materially higher slippage than a small trade
3. WHEN actual trade slippage is measured post-execution, THE predicted slippage SHALL not systematically underestimate or overestimate by more than [TBD calibration tolerance] for typical trade sizes
4. THE Slippage_Predictor SHALL not return a constant value regardless of trade size or asset pair (this was the original bug)

### Requirement 13: Documentation and Rustdoc

**User Story:** As a developer using or maintaining the slippage predictor, I want clear documentation of its behavior, model, and assumptions, so that I can use it correctly and debug issues.

#### Acceptance Criteria

1. THE Slippage_Predictor module SHALL include module-level rustdoc comments explaining the constant-product AMM model used
2. THE Slippage_Predictor rustdoc SHALL document assumptions (e.g., symmetric reserves, fee handling if applicable)
3. THE Slippage_Predictor function SHALL include parameter and return value documentation with units and ranges
4. THE Slippage_Predictor rustdoc SHALL include at least one worked example or pseudocode explaining slippage calculation
5. THE Slippage_Predictor SHALL document known limitations (e.g., single-hop trades only, assumes oracle accuracy)

### Requirement 14: Testability - Small Trade Scenarios

**User Story:** As a test author, I want to verify that the slippage predictor produces reasonable predictions for typical small trades, so that I can catch bugs in slippage calculations.

#### Acceptance Criteria

1. FOR a small trade (e.g., 1 USDC on a 1M USDC / 1M XLM reserve pair), THE Slippage_Predictor SHALL return low slippage (< 10 basis points)
2. FOR a small trade against a thin pair (e.g., 1 token on a 1K / 1K reserve pair), THE Slippage_Predictor SHALL still return reasonable slippage < 100 basis points
3. FOR small trades, THE Slippage_Predictor calculation SHALL produce repeatable results

### Requirement 15: Testability - Large Trade Scenarios

**User Story:** As a test author, I want to verify that the slippage predictor produces materially higher estimates for large trades, so that I can detect regressions where small and large trades are treated identically.

#### Acceptance Criteria

1. FOR a large trade (e.g., 50% of reserve), THE Slippage_Predictor SHALL return slippage > 5000 basis points (significant slippage)
2. FOR a trade at 50% of reserve on a typical 1M / 1M pool, THE predicted slippage SHALL be materially different from a 1-unit trade on the same pool
3. FOR large trades, THE Slippage_Predictor calculation SHALL produce repeatable results and not return a constant

### Requirement 16: Testability - Pathological Edge Cases

**User Story:** As a test author, I want to ensure the slippage predictor handles extreme scenarios without crashing, so that the rebalancer is robust.

#### Acceptance Criteria

1. WHEN trade_amount = u128::MAX and liquidity is small, THE Slippage_Predictor SHALL return maximum slippage or an error without overflowing
2. WHEN both reserves are 1 (minimal liquidity), THE Slippage_Predictor SHALL not divide by zero or overflow
3. WHEN reserve_a = 0 or reserve_b = 0, THE Slippage_Predictor SHALL return maximum slippage or an error
4. WHEN trade_amount > reserve_a (insufficient liquidity for full execution), THE Slippage_Predictor SHALL return maximum slippage or an error
5. FOR ALL edge cases in this requirement, THE function SHALL complete without panicking

## Acceptance Criteria Testing Prework

This section documents testability analysis for each acceptance criterion to inform property-based test design:

### Testability Analysis

#### Requirement 1: Basic Interface
- **Criterion 1-2**: Testable as example. Simple inputs and outputs; deterministic behavior.
- **Criterion 1-3 and 1-4**: Not testable by properties; require integration or interface verification.
- **Criterion 1-5**: Testable as property. Verify no external calls required.

#### Requirement 2: Trade Size Sensitivity
- **Criterion 2-1**: Testable as property. Monotonicity invariant: slippage(pair, amount_2) ≥ slippage(pair, amount_1) when amount_2 > amount_1.
- **Criterion 2-2**: Testable as example. Compare specific amounts (1%, 50%) and verify magnitude difference.
- **Criterion 2-3**: Testable as property. Limit: as amount → 0, slippage → 0.

#### Requirement 3: Liquidity Depth Sensitivity
- **Criterion 3-1**: Testable as metamorphic property. If depth_A > depth_B, then slippage(pair_B, amt) > slippage(pair_A, amt).
- **Criterion 3-2 and 3-3**: Not testable by properties; require oracle integration design.

#### Requirement 4: Constant-Product Model
- **All criteria**: Not testable as properties; require design documentation and manual review of math.

#### Requirement 5: Safe Arithmetic
- **Criterion 5-1 and 5-2**: Testable as property. No panics for any u128 input; bounded output.
- **Criterion 5-3, 5-4, 5-5**: Testable as edge case. Verify zero reserves, zero amount, max values handled safely.

#### Requirement 6: Zero Trade Amount
- **Criterion 6-1 and 6-2**: Testable as example. Input 0, expect output 0 or equivalent.
- **Criterion 6-3**: Testable as property. trade_amount = 0 always completes without panic.

#### Requirement 7: Unknown Pair Handling
- **Criterion 7-1 and 7-2**: Testable as example. Pass unknown pair, verify max slippage or error returned.
- **Criterion 7-3**: Testable as property. Unknown pair output is distinguishable from valid small slippage.

#### Requirement 8: Insufficient Liquidity
- **Criterion 8-1 to 8-3**: Testable as example and edge case. trade_amount ≥ reserve → max slippage or error.

#### Requirement 9: Output Type and Precision
- **All criteria**: Not testable by properties; require type definition and documentation review.

#### Requirement 10: Shared Math Module Integration
- **All criteria**: Not testable by properties; require code review and integration verification.

#### Requirement 11: Oracle Integration
- **All criteria**: Not testable by properties; require design documentation and integration tests.

#### Requirement 12: No Systematic Mispricing
- **Criterion 12-1 to 12-3**: Testable as examples with calibration. Compare small vs. large trades; verify predicted ≈ actual.
- **Criterion 12-4**: Testable as property. Output must NOT be constant for all inputs.

#### Requirement 13: Documentation
- **All criteria**: Not testable by properties; require documentation review.

#### Requirement 14: Small Trade Scenarios
- **Criterion 14-1 to 14-3**: Testable as examples. Fixed inputs (1 USDC on 1M/1M pool) → verify output is in expected range.

#### Requirement 15: Large Trade Scenarios
- **Criterion 15-1 to 15-3**: Testable as examples. Fixed inputs (50% of reserve) → verify high slippage; compare to small trade.

#### Requirement 16: Pathological Edge Cases
- **Criterion 16-1 to 16-5**: Testable as edge case and property. No panics for any input; bounded output.

