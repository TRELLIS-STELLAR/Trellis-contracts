# Technical Design: Rebalancer Slippage Predictor

## Overview

The Slippage Predictor is a Rust module within the rebalancer contract that estimates the price slippage incurred when executing trades on a constant-product automated market maker (AMM). It bridges the gap between the rebalancer's high-level trade simulation and the mathematical reality of liquidity-dependent price impact.

**Core responsibility**: Given an asset pair and a trade amount, predict the percentage slippage (in basis points) that will occur, accounting for:
1. Trade size relative to available liquidity
2. Liquidity depth of the specific asset pair
3. The constant-product AMM invariant: `reserve_a × reserve_b = k`

**Output**: All predictions return a `U256` representing basis points (where 10,000 = 100% slippage), enabling the rebalancer to accumulate total slippage across multiple trades without precision loss.

---

## Architecture

### High-Level Flow

```
predict_slippage(asset_pair, amount, env)
    ↓
[Validate Inputs]
    • trade_amount ∈ [0, u128::MAX]
    • asset_pair is Some((Symbol, Symbol))
    ↓
[Retrieve Liquidity Data]
    • Query oracle or storage for reserve_a and reserve_b
    • Return MAX_SLIPPAGE if pair not found
    ↓
[Calculate Slippage]
    • Apply constant-product formula
    • Convert to basis points (i128 fixed-point)
    • Clamp result to [0, MAX_SLIPPAGE]
    ↓
[Return U256]
    • Convert i128 result to U256
    • Always returns a bounded, valid U256
```

### Module Structure

```
contracts/rebalancer-contract/src/slippage_predictor.rs

Module-level Rustdoc:
  - Constant-product AMM model explanation
  - Mathematical assumptions and derivations
  - Fixed-point arithmetic strategy (basis points)
  - Edge case handling philosophy
  - Worked example with specific numbers

Public Interface:
  fn predict_slippage(
      asset_pair: (Symbol, Symbol),
      amount: u128,
      env: &Env,
  ) -> U256

Internal Functions:
  - get_liquidity_data(pair, env) -> Result<(i128, i128), Error>
  - calculate_slippage_bps(amount, reserve_a, reserve_b) -> Result<i128, Error>
  - clamp_slippage(bps: i128) -> U256
```

---

## Components and Interfaces

### 1. Input Validation

**Inputs**:
- `asset_pair`: `(Symbol, Symbol)` — ordered pair of assets (base, quote)
- `amount`: `u128` — trade amount in base asset units
- `env`: `&Env` — Soroban environment for contract interactions

**Validation Rules**:
- `amount = 0` → immediately return `U256(0)`
- `amount > 0` → proceed to liquidity lookup
- Invalid `asset_pair` (not found in oracle) → return `MAX_SLIPPAGE`

### 2. Liquidity Data Retrieval

**Current State**: Oracle contract stores price feeds but not liquidity depth. 

**Design Choice**: We define a liquidity data interface with two possible implementations:

#### Option A: Mock Storage (Near-term)
- Store liquidity reserves in rebalancer contract storage keyed by asset pair
- Enables testing and baseline functionality
- Limitations: requires manual updating; doesn't reflect real-time liquidity

#### Option B: Oracle Extension (Future)
- Extend oracle contract to accept and store liquidity depth submissions
- Same interface as price feeds (submitter-based, with timestamp)
- Enables real-time liquidity tracking

**For this design, we implement Option A** with structure:

```rust
// Logical structure (actual storage via soroban-sdk)
struct LiquidityData {
    reserve_a: i128,      // Reserve of first asset in pair
    reserve_b: i128,      // Reserve of second asset in pair
    timestamp: u64,       // When data was last updated
    is_active: bool,      // Whether this pair is supported
}

// Key: hash(Symbol, Symbol) or namespace-based
// Value: LiquidityData
```

**Interface assumption**:
```rust
// Placeholder for external interface design
fn get_liquidity_reserves(
    pair: (Symbol, Symbol),
    env: &Env,
) -> Result<(i128, i128), Error> {
    // In implementation: query storage, return (reserve_a, reserve_b)
    // If not found or inactive: return Error::InvalidAmount
}
```

**Edge cases**:
- Pair not found → `Error::InvalidAmount` → predictor returns `MAX_SLIPPAGE`
- `reserve_a = 0` or `reserve_b = 0` → `Error::InvalidAmount` → `MAX_SLIPPAGE`

### 3. Constant-Product AMM Slippage Calculation

#### Mathematical Model

**Constant-Product Invariant**:
```
reserve_a × reserve_b = k  (constant before and after trade)
```

**Trade Mechanics**:
- User sends `trade_amount` of asset A
- Pool outputs some quantity of asset B (the execution amount)
- The product of reserves must remain constant

**Derivation**:

Let:
- `reserve_a`, `reserve_b` = initial pool reserves
- `trade_amount` = amount of asset A being traded in
- `execution_amount` = amount of asset B received (actual output)

The new reserves after trade satisfy:
```
(reserve_a + trade_amount) × (reserve_b - execution_amount) = reserve_a × reserve_b
```

Solving for `execution_amount`:
```
reserve_b - execution_amount = (reserve_a × reserve_b) / (reserve_a + trade_amount)

execution_amount = reserve_b - (reserve_a × reserve_b) / (reserve_a + trade_amount)

execution_amount = reserve_b × [1 - 1 / (1 + trade_amount / reserve_a)]

Simplified (avoid division):
execution_amount = reserve_b × trade_amount / (reserve_a + trade_amount)
```

**Expected Output** (spot price):
```
expected_output = (reserve_b / reserve_a) × trade_amount
```

**Slippage Definition**:
```
slippage = (expected_output - execution_amount) / expected_output

Substituting:
slippage = [expected - actual] / expected
         = [(rb/ra) × amt - (rb × amt) / (ra + amt)] / [(rb/ra) × amt]
         = [1 - (ra / (ra + amt))]
         = amt / (ra + amt)
```

**In Basis Points** (1 basis point = 0.01%):
```
slippage_bps = (amt / (ra + amt)) × 10_000
```

#### Fixed-Point Arithmetic Implementation

**Problem**: We work with `i128` integers (no floating-point on-chain), but need to compute:
```
slippage_bps = (trade_amount / (reserve_a + trade_amount)) × 10_000
```

**Solution**: Reorder operations to preserve precision and avoid overflow:

```rust
// Pseudocode (actual implementation uses checked_* functions)
pub fn calculate_slippage_bps(
    trade_amount: i128,
    reserve_a: i128,
    reserve_b: i128,
) -> Result<i128, Error> {
    // Step 1: Compute denominator (reserve_a + trade_amount)
    let denominator = checked_add(reserve_a, trade_amount)?;

    // Step 2: Scale numerator to preserve precision
    // Multiply trade_amount by BPS_DENOMINATOR (10_000) first
    let scaled_numerator = checked_mul(trade_amount, 10_000)?;

    // Step 3: Divide by denominator
    let slippage_bps = checked_div(scaled_numerator, denominator)?;

    // Step 4: Clamp to valid range [0, MAX_SLIPPAGE]
    Ok(clamp(slippage_bps, 0, MAX_SLIPPAGE))
}
```

**Precision Analysis**:
- Input: `trade_amount` and `reserve_a` as `i128` (up to ~9.2 × 10^18)
- Intermediate: `trade_amount × 10_000` (up to ~9.2 × 10^22, fits in `i128`)
- Result: `slippage_bps` in range [0, 10_000 + epsilon] for normal trades
- For `trade_amount ≥ reserve_a`, result approaches or exceeds 10_000 basis points

**Why this works**:
- The numerator scales up by `10_000` before division, preserving significant digits
- Example: `trade_amount = 100`, `reserve_a = 1_000_000`
  - Slippage = (100 / 1_000_100) × 10_000 ≈ 0.999 basis points (correct)
- Example: `trade_amount = 500_000`, `reserve_a = 1_000_000`
  - Slippage = (500_000 / 1_500_000) × 10_000 ≈ 3_333 basis points (high, correct)

### 4. Type Conversions and Boundaries

```
Input chain:
  u128 (trade_amount)
    ↓ [convert to i128 safely]
  i128
    ↓ [multiply by 10_000]
  i128 (scaled numerator, may overflow → Error::Overflow)
    ↓ [divide by denominator]
  i128 (slippage_bps, range [0, MAX_SLIPPAGE])
    ↓ [clamp if needed]
  i128 (final result)
    ↓ [convert to U256]
  U256 (output, always valid)
```

**Conversion Rules**:

1. **u128 → i128**:
   - If `trade_amount ≤ i128::MAX` (≈ 9.2 × 10^18): safe conversion
   - If `trade_amount > i128::MAX`: return `MAX_SLIPPAGE` (trade size unrealistic)
   - Implementation: `i128::try_from(trade_amount).unwrap_or(...)`

2. **i128 → U256**:
   - Always safe after clamping (result is non-negative)
   - `U256::from_i128(env, result_bps)` (if available)
   - Fallback: `U256::from_u128(env, result_bps as u128)`

3. **Overflow Handling**:
   - `checked_mul(trade_amount_i128, 10_000)` may overflow
   - If overflow: return `MAX_SLIPPAGE` (trade too large for this pair)
   - Checked operations from `shared::math` propagate `Error::Overflow`

---

## Data Models

### Constants

```rust
// Module-level constants
const BPS_DENOMINATOR: i128 = 10_000;           // Basis points scale
const MAX_SLIPPAGE: i128 = 10_000_000;          // Max return: 10,000% (protective upper bound)
const MIN_LIQUIDITY: i128 = 1;                  // Minimum reserve to avoid div by zero
```

### Liquidity Storage Schema (Conceptual)

**Storage Key Format**:
```
"liquidity" + (serialize asset_pair)
  → {
      reserve_a: i128,
      reserve_b: i128,
      timestamp: u64,
      active: bool,
    }
```

**Example Entries**:
```
("USDC", "XLM")  → { reserve_a: 1_000_000_000, reserve_b: 100_000_000, ... }
("XLM", "BTC")   → { reserve_a: 50_000_000, reserve_b: 100, ... }
```

---

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Slippage Monotonicity with Trade Size

**For any** asset pair with fixed reserves, if we execute two trades with amounts `amt_1 < amt_2` on the same pair, the predicted slippage for `amt_2` SHALL be greater than or equal to the predicted slippage for `amt_1`.

**Validates: Requirements 2.1, 2.3**

**Reasoning**: The formula `slippage = amount / (reserve_a + amount)` is strictly increasing in `amount` (holding reserves constant). This is a fundamental property of the constant-product model and directly tests the monotonicity requirement.

---

### Property 2: Slippage Varies with Liquidity Depth

**For any** trade amount and two asset pairs where pair_A has deeper liquidity (both reserves larger than pair_B), the predicted slippage for pair_B SHALL be greater than the predicted slippage for pair_A when both trade the same amount.

**Validates: Requirements 3.1**

**Reasoning**: Deeper liquidity (larger reserve denominators) reduces slippage per the formula. This property is a metamorphic property comparing two scenarios and verifying the relationship holds.

---

### Property 3: Zero Trade Amount Returns Zero Slippage

**For any** asset pair with valid liquidity data, when `trade_amount = 0`, the predicted slippage SHALL be exactly 0 (or equivalent, e.g., `U256` representing 0 basis points).

**Validates: Requirements 6.1, 6.2**

**Reasoning**: Mathematically, `slippage = 0 / (reserve_a + 0) = 0`. This is a boundary property and critical for safe execution without division issues.

---

### Property 4: Non-Panic Guarantee for All Valid Inputs

**For any** valid u128 inputs (trade_amount ∈ [0, u128::MAX]), and any asset pair, the `predict_slippage` function SHALL complete successfully (return a valid U256) without panicking, overflowing in a way that crashes the contract, or entering an infinite loop.

**Validates: Requirements 5.2, 5.5, 16.1, 16.5**

**Reasoning**: This is a robustness/safety property. All arithmetic must be checked and all error paths must return bounded values or MAX_SLIPPAGE instead of panicking. This property covers adversarial inputs like `u128::MAX`, zero reserves, and edge-case pairings.

---

### Property 5: Invalid Pair Distinguishability

**For any** unknown or unsupported asset pair, the predicted slippage SHALL be equal to `MAX_SLIPPAGE` (or a distinctly high value), which is materially different from the slippage of a valid pair being traded in small amount (e.g., < 100 basis points).

**Validates: Requirements 7.3**

**Reasoning**: The rebalancer needs to distinguish between "invalid pair" (should not trade) and "valid pair with small slippage" (safe to trade). This property ensures the error signal is clear and not ambiguous with normal low slippage.

---

### Property 6: Insufficient Liquidity Detection

**For any** asset pair where `trade_amount ≥ reserve_a`, the predicted slippage SHALL be greater than or equal to `5_000` basis points (50% slippage) or equal to `MAX_SLIPPAGE`.

**Validates: Requirements 8.1**

**Reasoning**: If a user attempts to trade an amount equal to or exceeding all available reserves of one side, the slippage approaches or equals 100% (10_000 basis points) per the formula. This property catches the insufficient liquidity edge case and signals it to the rebalancer.

---

### Property 7: Constant-Product Formula Validation

**For any** valid trade amount, reserve_a, and reserve_b, the calculated execution amount using the formula `execution = reserve_b × trade_amount / (reserve_a + trade_amount)` SHALL preserve the invariant `(reserve_a + trade_amount) × (reserve_b - execution) ≈ reserve_a × reserve_b` (within rounding tolerance of fixed-point arithmetic).

**Validates: Requirements 4.1, 4.2**

**Reasoning**: This is a mathematical invariant property that directly validates the correctness of the constant-product AMM formula implementation. Testing this ensures the math is sound.

---

### Property 8: Output Bounded and Deterministic

**For any** valid inputs, the returned U256 value SHALL be deterministic (same inputs → same output) and SHALL represent a value in basis points bounded by `[0, MAX_SLIPPAGE]`.

**Validates: Requirements 9.1, 9.2**

**Reasoning**: Determinism is essential for rebalancer decision-making. The bounded output ensures no overflow surprises downstream. This is an invariant property over all execution paths.

---

## Error Handling

### Error Conditions and Resolutions

| Condition | Detection | Resolution | Return Value |
|-----------|-----------|-----------|--------------|
| `trade_amount = 0` | Early check in validation | Return 0 immediately | `U256(0)` |
| Asset pair unknown | Query returns None from storage | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| `reserve_a = 0` or `reserve_b = 0` | Check after retrieval | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| `checked_add` overflow (ra + amt) | Caught by checked_add | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| `checked_mul` overflow (amt × 10k) | Caught by checked_mul | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| `checked_div` overflow (reserved for i128::MIN / -1) | Caught by checked_div | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| u128 → i128 overflow (amt > i128::MAX) | TryFrom fails | Return MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |
| Slippage exceeds MAX_SLIPPAGE after calc | Check after computation | Clamp to MAX_SLIPPAGE | `U256::from_i128(env, MAX_SLIPPAGE)` |

**Philosophy**: Every error path returns a high/maximal slippage value, signaling to the rebalancer "this trade is risky or invalid" rather than crashing or returning misleading low values.

### Error Propagation Strategy

```rust
// Pseudocode outline
pub fn predict_slippage(...) -> U256 {
    // All intermediate errors → MAX_SLIPPAGE
    
    let trade_amt_i128 = match i128::try_from(trade_amount) {
        Ok(v) => v,
        Err(_) => return U256::from_i128(&env, MAX_SLIPPAGE), // Overflow
    };
    
    if trade_amt_i128 == 0 {
        return U256::from_u128(&env, 0);
    }
    
    let (reserve_a, reserve_b) = match get_liquidity_reserves(&asset_pair, &env) {
        Ok((ra, rb)) if ra > 0 && rb > 0 => (ra, rb),
        _ => return U256::from_i128(&env, MAX_SLIPPAGE), // Invalid/missing data
    };
    
    let slippage_bps = match calculate_slippage_bps(trade_amt_i128, reserve_a) {
        Ok(bps) => clamped(bps),
        Err(_) => MAX_SLIPPAGE, // Any arithmetic error
    };
    
    U256::from_i128(&env, slippage_bps)
}
```

---

## Testing Strategy

### Dual Testing Approach

The slippage predictor requires both **unit tests** (specific examples and edge cases) and **property-based tests** (universal properties across generated inputs) for comprehensive coverage.

#### Unit Testing

**Scope**: Specific scenarios, integration points, and edge cases where deterministic inputs verify concrete behavior.

**Categories**:

1. **Boundary Examples** (3-4 tests):
   - Zero trade amount: `amount = 0` → slippage = 0
   - Very small trade (< 1% of reserve): `amount = 100` on 1M/1M pair → slippage < 100 bps
   - Large trade (50% of reserve): `amount = 500k` on 1M/1M pair → slippage > 5000 bps
   - Maximum trade attempt: `amount = u128::MAX` → slippage = MAX_SLIPPAGE (no panic)

2. **Edge Cases** (4-5 tests):
   - Unknown asset pair → MAX_SLIPPAGE
   - Zero reserves → MAX_SLIPPAGE
   - Reserves = 1 (minimal liquidity) → computes without panic
   - Trade amount >= reserve → high slippage (> 50%)

3. **Integration** (2-3 tests):
   - Multiple pairs with different liquidity → relative slippage ordering correct
   - Sequential trades on same pair → deterministic results

**Implementation**: Use Rust's built-in `#[test]` or a test framework (e.g., `proptest` for hybrid testing).

#### Property-Based Testing

**Scope**: Universal properties that must hold across many generated inputs (minimum 100 iterations per property).

**Property-Test Configuration**:
- Framework: `proptest` (Rust's standard PBT library for i128/u128 generation)
- Iterations: Minimum 100 per property (higher for critical properties)
- Generators:
  - `trade_amount`: Any u128 value
  - `reserve_a`, `reserve_b`: Positive i128 values (1 to i128::MAX)
  - Asset pairs: Generated Symbols (safe within string limits)

**Property Test Tags and Mapping**:

Each test includes a comment tag: `Feature: rebalancer-slippage-predictor, Property {N}: {description}`

| Property | Test Name | Tag |
|----------|-----------|-----|
| 1 | test_prop_monotonicity_with_trade_size | Property 1: Slippage Monotonicity |
| 2 | test_prop_liquidity_depth_sensitivity | Property 2: Liquidity Depth Variation |
| 3 | test_prop_zero_amount_returns_zero | Property 3: Zero Trade Slippage |
| 4 | test_prop_no_panic_all_inputs | Property 4: Non-Panic Guarantee |
| 5 | test_prop_invalid_pair_distinguishable | Property 5: Invalid Pair Distinguishability |
| 6 | test_prop_insufficient_liquidity_detection | Property 6: Insufficient Liquidity |
| 7 | test_prop_constant_product_invariant | Property 7: Constant-Product Formula |
| 8 | test_prop_output_bounded_deterministic | Property 8: Output Bounded & Deterministic |

**Example Property Test Skeleton** (Property 1):

```rust
#[cfg(test)]
mod prop_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        // Feature: rebalancer-slippage-predictor, Property 1: Slippage Monotonicity
        fn test_prop_monotonicity_with_trade_size(
            amt1 in 1u128..1_000_000_000,
            amt2 in 1u128..1_000_000_000,
            reserve_a in 1i128..i128::MAX,
            reserve_b in 1i128..i128::MAX,
        ) {
            let (smaller_amt, larger_amt) = if amt1 < amt2 {
                (amt1, amt2)
            } else {
                (amt2, amt1)
            };

            let slippage1 = predict_slippage((small_sym, large_sym), smaller_amt, &env);
            let slippage2 = predict_slippage((small_sym, large_sym), larger_amt, &env);

            // Monotonicity: slippage2 >= slippage1
            prop_assert!(slippage2 >= slippage1);
        }
    }
}
```

### Testing Best Practices

1. **No Panic Requirement**: Every test must verify the function completes without unwinding.
2. **Bounds Checking**: Verify all outputs fit within expected ranges.
3. **Determinism**: Run same inputs multiple times, verify identical outputs.
4. **Oracle Mocking**: Mock liquidity storage in tests to avoid external dependencies.
5. **Seed Isolation**: Use fixed seeds for reproducible property test failures.

---

## Implementation Notes

### Fixed-Point Arithmetic Patterns

**Safe scaling pattern** (used throughout):
```rust
// To compute (a / b) × scale without intermediate overflow:
// 1. Multiply numerator first
// 2. Divide second
// 3. Clamp result

let scaled = checked_mul(a, scale)?;  // Scale up for precision
let result = checked_div(scaled, b)?;  // Divide
```

**Why not use floating-point?**
- Soroban/WASM don't have efficient FP support
- Fixed-point is deterministic across platforms
- On-chain requires predictable rounding

### Shared Math Module Integration

All arithmetic uses functions from `shared/src/math.rs`:
- `checked_add(a, b)` → `Result<i128, Error>`
- `checked_mul(a, b)` → `Result<i128, Error>`
- `checked_div(a, b)` → `Result<i128, Error>` (zero → `Error::InvalidAmount`)

**No direct `.checked_*()` calls** on i128 — always go through shared module for consistency and error handling.

### Liquidity Storage Assumptions

1. **Synchronization**: Liquidity data is updated out-of-band (via governance or oracle feeds).
2. **Pair Ordering**: Asset pairs have a canonical order `(base, quote)` — `(USDC, XLM)` ≠ `(XLM, USDC)`.
3. **Reserve Symmetry**: `reserve_a` and `reserve_b` are non-negative and represent units of each asset (not converted to common denomination).
4. **Timeliness**: For accuracy, liquidity should be updated at most every N blocks (not specified here; governance decision).

### Pseudocode Reference Implementation

```rust
/// Estimates price slippage when trading a specific amount on a given asset pair.
///
/// Uses the constant-product AMM formula to calculate slippage based on trade size
/// and available liquidity depth. All arithmetic is checked to prevent overflow.
///
/// # Mathematical Model
///
/// For a constant-product AMM (reserve_a × reserve_b = k):
/// - Execution amount: `output = reserve_b × trade_amount / (reserve_a + trade_amount)`
/// - Expected amount (spot price): `expected = (reserve_b / reserve_a) × trade_amount`
/// - Slippage: `(expected - output) / expected = trade_amount / (reserve_a + trade_amount)`
/// - In basis points: `slippage_bps = (trade_amount / (reserve_a + trade_amount)) × 10,000`
///
/// # Edge Cases
///
/// - `trade_amount = 0` → returns 0
/// - Unknown asset pair → returns MAX_SLIPPAGE
/// - `reserve_a = 0` or `reserve_b = 0` → returns MAX_SLIPPAGE
/// - Arithmetic overflow → returns MAX_SLIPPAGE
/// - `trade_amount > reserve_a` → returns high slippage (> 5,000 bps)
///
/// # Example
///
/// Pair (USDC, XLM) with reserves: 1,000,000 USDC, 100,000,000 XLM.
/// Trade: 1,000 USDC.
///
/// Slippage = 1,000 / (1,000,000 + 1,000) × 10,000 ≈ 10 basis points (0.1%)
pub fn predict_slippage(
    asset_pair: (Symbol, Symbol),
    amount: u128,
    env: &Env,
) -> U256 {
    // Step 1: Handle zero amount edge case
    if amount == 0 {
        return U256::from_u128(env, 0);
    }

    // Step 2: Convert u128 to i128 safely
    let amount_i128 = match i128::try_from(amount) {
        Ok(v) => v,
        Err(_) => return U256::from_i128(env, MAX_SLIPPAGE),
    };

    // Step 3: Retrieve liquidity reserves for the asset pair
    let (reserve_a, reserve_b) = match get_liquidity_reserves(&asset_pair, env) {
        Ok((ra, rb)) if ra > 0 && rb > 0 => (ra, rb),
        _ => return U256::from_i128(env, MAX_SLIPPAGE),
    };

    // Step 4: Calculate slippage in basis points
    let slippage_bps = match calculate_slippage_bps(amount_i128, reserve_a, reserve_b) {
        Ok(bps) => bps,
        Err(_) => MAX_SLIPPAGE,
    };

    // Step 5: Return as U256
    U256::from_i128(env, slippage_bps)
}

/// Internal: Calculates slippage in basis points for a given trade.
fn calculate_slippage_bps(
    trade_amount: i128,
    reserve_a: i128,
    reserve_b: i128,
) -> Result<i128, Error> {
    use shared::math::{checked_add, checked_mul, checked_div};

    // Denominator: reserve_a + trade_amount
    let denominator = checked_add(reserve_a, trade_amount)?;

    // Scale numerator: trade_amount × 10,000
    let scaled_numerator = checked_mul(trade_amount, 10_000)?;

    // Divide to get basis points
    let slippage_bps = checked_div(scaled_numerator, denominator)?;

    // Clamp to safe range
    Ok(slippage_bps.min(MAX_SLIPPAGE).max(0))
}

/// Internal: Retrieves liquidity reserves for an asset pair from storage.
fn get_liquidity_reserves(
    pair: &(Symbol, Symbol),
    env: &Env,
) -> Result<(i128, i128), Error> {
    // Placeholder: queries storage or oracle
    // Returns: Ok((reserve_a, reserve_b)) on success
    //          Err(Error::InvalidAmount) if pair not found or inactive
    todo!("Implement storage lookup")
}
```

---

## Constants and Boundaries

| Constant | Value | Purpose |
|----------|-------|---------|
| `BPS_DENOMINATOR` | 10,000 | Basis points scale (100% = 10,000 bps) |
| `MAX_SLIPPAGE` | 10,000,000 | Maximum slippage return (10,000% protective bound) |
| `MIN_LIQUIDITY` | 1 | Minimum reserve to avoid division by zero |
| `i128::MAX` | ~9.2 × 10^18 | Maximum i128 value (safe for checked ops) |

### Slippage Output Ranges

| Scenario | Slippage Range | Interpretation |
|----------|----------------|-----------------|
| Very small trade (0.01% of reserve) | 0–10 bps | Minimal price impact |
| Small trade (1% of reserve) | 50–100 bps | Low price impact |
| Medium trade (10% of reserve) | 500–1,000 bps | Noticeable price impact |
| Large trade (50% of reserve) | 3,000–5,000 bps | Significant slippage |
| Extreme trade (100%+ of reserve) | 10,000+ bps → MAX_SLIPPAGE | Insufficient liquidity |
| Invalid/unknown pair | MAX_SLIPPAGE | Error signal |

---

## Assumptions and Limitations

### Assumptions

1. **Symmetric Reserves**: The formula assumes `reserve_a` and `reserve_b` are the only reserves in the pool (single-hop trades, no intermediate contracts).
2. **No Fees**: Slippage calculation does not account for AMM trading fees (those are handled separately by the rebalancer or oracle).
3. **Liquidity Stability**: Reserves retrieved from storage are current and accurate within the time of execution (no flash loans or rapid changes mid-block).
4. **Single-Hop Trades**: The predictor estimates slippage for trading directly between two assets, not through routing/aggregation.
5. **Fixed-Point Precision**: Basis-point arithmetic has inherent rounding error; results are rounded down (conservative for the rebalancer).

### Known Limitations

1. **No Multi-Hop Support**: Cannot estimate slippage for trades routed through intermediate pairs.
2. **Delayed Liquidity Data**: Liquidity reserves are not real-time; predictions lag actual market conditions.
3. **No Fee Modeling**: Slippage excludes trading fees, which are handled separately.
4. **Assumed Independence**: Slippage is calculated assuming the trade doesn't affect reserves of other pairs (true for single-pair trades only).
5. **Precision Loss**: Fixed-point arithmetic at 1 basis point granularity; sub-basis-point slippage is rounded to nearest basis point.

### Future Enhancements

1. Extend oracle to accept liquidity depth submissions (real-time feeds).
2. Support multi-hop routing with aggregated slippage estimation.
3. Integrate fee modeling into slippage calculations.
4. Higher precision basis-point representation if needed (e.g., sub-basis-point rounding).

---

## Type Conversion Diagram

```
Input: u128 (trade_amount)
  │
  ├─→ Check if 0 → Return U256(0)
  │
  ├─→ TryFrom → i128
  │    └─ Overflow? → MAX_SLIPPAGE → U256
  │
  ├─→ Retrieve reserves (i128, i128)
  │    └─ Not found or invalid? → MAX_SLIPPAGE → U256
  │
  ├─→ Calculate slippage
  │    ├─ checked_add(reserve_a, amount)
  │    │   └─ Overflow? → MAX_SLIPPAGE
  │    │
  │    ├─ checked_mul(amount, 10_000)
  │    │   └─ Overflow? → MAX_SLIPPAGE
  │    │
  │    └─ checked_div(scaled, denom)
  │        └─ Error? → MAX_SLIPPAGE
  │
  ├─→ Clamp result to [0, MAX_SLIPPAGE]
  │    └─ Result: i128 (basis points)
  │
  └─→ Convert i128 → U256
       └─ Output: U256 (final result)
```

---

## Module-Level Rustdoc Template

```rust
//! Slippage Predictor for Constant-Product AMMs
//!
//! This module estimates price slippage when trading on a constant-product
//! automated market maker (AMM). Slippage represents the difference between
//! the expected execution price (spot price) and the actual price received
//! due to finite liquidity depth.
//!
//! # Mathematical Model
//!
//! The predictor uses the constant-product invariant: `reserve_a × reserve_b = k`.
//! Given a trade of `amount` units of asset A:
//!
//! - **Execution amount**: `output = reserve_b × amount / (reserve_a + amount)`
//! - **Spot price**: `spot = reserve_b / reserve_a`
//! - **Slippage**: `(spot_output - actual_output) / spot_output = amount / (reserve_a + amount)`
//! - **In basis points**: `slippage_bps = (amount / (reserve_a + amount)) × 10,000`
//!
//! # Worked Example
//!
//! **Scenario**: Trade 1,000 USDC on a USDC/XLM pair with reserves (1M USDC, 100M XLM).
//!
//! ```ignore
//! amount = 1,000 (USDC)
//! reserve_a = 1,000,000 (USDC)
//! reserve_b = 100,000,000 (XLM)
//!
//! slippage = 1,000 / (1,000,000 + 1,000) × 10,000
//!          = 1,000 / 1,001,000 × 10,000
//!          ≈ 9.99 basis points
//! ```
//!
//! The trader receives approximately 9.99 basis points (0.0999%) less XLM than
//! if they had traded at the spot price.
//!
//! # Edge Cases
//!
//! - **Zero amount**: Returns 0 basis points.
//! - **Unknown pair**: Returns MAX_SLIPPAGE (10,000,000 bps).
//! - **Zero reserves**: Returns MAX_SLIPPAGE.
//! - **Insufficient liquidity** (`amount ≥ reserve_a`): Returns high slippage.
//! - **Arithmetic overflow**: Returns MAX_SLIPPAGE instead of panicking.
//!
//! # Assumptions
//!
//! - Liquidity reserves are up-to-date and retrieved from oracle storage.
//! - Trades are single-hop (direct asset pair, no routing).
//! - No trading fees are included (handled separately).
//! - All arithmetic uses checked operations to prevent overflow.
//!
//! # Output Unit
//!
//! All slippage predictions return a `U256` representing **basis points**:
//! - 1 bp = 0.01% slippage
//! - 10,000 bp = 100% slippage
//! - Results are clamped to [0, MAX_SLIPPAGE].
```

