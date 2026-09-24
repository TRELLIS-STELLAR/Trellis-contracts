# Code Coverage Reporting - Design (#14)

**Status**: Design Phase  
**Last Updated**: September 24, 2026  

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  GitHub Actions CI (on push/PR)                              │
├─────────────────────────────────────────────────────────────┤
│  1. Build Job (existing)                                     │
│  2. Test Job (existing)                                      │
│  3. ► Coverage Job (NEW) ◄────────────────── Depends on Test │
│  4. Clippy Job (existing)                                    │
│  5. Security Audit Job (existing)                            │
└─────────────────────────────────────────────────────────────┘
                         │
                         ▼
            ┌────────────────────────────┐
            │ Coverage Job               │
            ├────────────────────────────┤
            │ 1. Install llvm-cov        │
            │ 2. Run cargo llvm-cov      │
            │ 3. Parse results           │
            │ 4. Compare vs floor        │
            │ 5. Upload report           │
            │ 6. Report result           │
            └────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
    ┌────────┐       ┌────────┐      ┌────────┐
    │ JSON   │       │ Fail?  │      │ Badge  │
    │ Report │       │ CI if  │      │ Update │
    │Artifact│       │<floor  │      │(Future)│
    └────────┘       └────────┘      └────────┘
```

---

## CI Job Design

### Job: "coverage"

**Placement**: After "test" job (depends on test passing)

**Steps**:

1. **Checkout** (reuse existing step template)
   - Uses: `actions/checkout@v4`

2. **Install Rust stable** (reuse existing template)
   - Uses: `dtolnay/rust-toolchain@stable`
   - Targets: `wasm32v1-none`

3. **Install cargo-llvm-cov** (NEW)
   - Uses: `taiki-e/install-action@v2`
   - Tool: `cargo-llvm-cov`
   - Rationale: `cargo-llvm-cov` is maintained and has better no_std support than tarpaulin

4. **Cache cargo** (reuse existing template)
   - Paths: `~/.cargo/registry`, `~/.cargo/git`, `target`
   - Key: Same as other jobs for consistency

5. **Run cargo llvm-cov** (NEW)
   - Command: `cargo llvm-cov --all-crates --output-format json --output json-coverage.json`
   - Workspace: Covers all contracts, shared, testing
   - Format: JSON for machine parsing + comparison

6. **Parse and check floor** (NEW)
   - Script: Extract JSON coverage percentage
   - Compare: `coverage >= COVERAGE_FLOOR` (env var, e.g., "70")
   - Output: Human-readable result

7. **Upload artifact** (NEW)
   - Uses: `actions/upload-artifact@v4`
   - Path: `json-coverage.json`
   - Name: `coverage-report`
   - Condition: `always()` (upload even if step 6 fails)

8. **Report result** (NEW)
   - Set step result: Pass if `coverage >= floor`, fail otherwise
   - Log summary: "Coverage: X% vs floor: Y%"

---

## Implementation Details

### Environment Variables

```yaml
env:
  COVERAGE_FLOOR: 70  # 70% minimum coverage (to be agreed)
```

Add to job level or workflow level (recommend job level for clarity).

### YAML Structure

```yaml
coverage:
  name: Code Coverage
  runs-on: ubuntu-latest
  needs: test  # Depends on test job passing
  
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        targets: wasm32v1-none
    - uses: taiki-e/install-action@v2
      with:
        tool: cargo-llvm-cov
    - uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
        restore-keys: ${{ runner.os }}-cargo-
    
    - name: Run coverage
      run: cargo llvm-cov --all-crates --output-format json --output json-coverage.json
    
    - name: Check coverage floor
      run: |
        COVERAGE=$(jq '.coverage | .[0].percent_covered' json-coverage.json)
        FLOOR=${{ env.COVERAGE_FLOOR }}
        echo "Coverage: ${COVERAGE}% | Floor: ${FLOOR}%"
        if (( $(echo "$COVERAGE >= $FLOOR" | bc -l) )); then
          echo "✓ Coverage meets floor"
          exit 0
        else
          echo "✗ Coverage below floor"
          exit 1
        fi
    
    - name: Upload coverage report
      if: always()
      uses: actions/upload-artifact@v4
      with:
        name: coverage-report
        path: json-coverage.json
        if-no-files-found: ignore
```

---

## Baseline Measurement

### Process

1. **Local Run**
   ```bash
   cargo llvm-cov --all-crates --output-format json --output baseline.json
   jq '.coverage' baseline.json  # Pretty print
   ```

2. **Parse Output**
   - Extract coverage percentage from JSON
   - Example JSON structure:
     ```json
     {
       "coverage": [
         {
           "name": "aid-contract",
           "percent_covered": 65.3
         },
         {
           "name": "treasury-contract",
           "percent_covered": 72.1
         },
         ...
       ]
     }
     ```

3. **Document**
   - Comment in issue #14 with baseline per crate
   - Timestamp: "Measured on Sept 24, 2026"
   - Format:
     ```
     ## Baseline Coverage
     - aid-contract: 65%
     - treasury-contract: 72%
     - shared: 88%
     - testing: 91%
     - Total workspace: 75%
     ```

### Baseline Floor Calculation

Two options:
1. **Conservative**: Floor = (Total baseline - 5%) to allow for regressions
2. **Aggressive**: Floor = Total baseline to prevent regressions

Recommend conservative initially, tighten over time.

---

## Badge Integration

### Option 1: Shields.io Static Badge (Recommended for MVP)
- Badge URL: https://img.shields.io/badge/coverage-PERCENT%25-color
- Example: https://img.shields.io/badge/coverage-75%25-1C6B55
- Pros: No external dependencies, static, clean
- Cons: Requires manual update on each release

### Option 2: Codecov Badge (Future)
- Badge URL: https://codecov.io/gh/CITRELLIS-STELLAR/Trellis-contracts/branch/main/graph/badge.svg
- Pros: Auto-updates, historical tracking
- Cons: Requires Codecov account and token

### Implementation (MVP)

Add to README badges section (line ~15):
```markdown
<img src="https://img.shields.io/badge/coverage-75%25-1C6B55?style=flat-square" alt="Coverage">
```

Update after baseline is agreed.

---

## Floor Agreement

### Suggested Process

1. **Run baseline** locally (Task 1)
2. **Post results** in issue #14 comment
3. **Discuss** with team:
   - Should floor be workspace-wide or per-crate?
   - How much regression tolerance? (e.g., ±5%?)
   - Different floors for contracts vs libraries? (e.g., shared at 85%, contracts at 70%)
4. **Agree** and document in issue
5. **Set in CI** via `COVERAGE_FLOOR` env var
6. **Enforce** in workflow

### Example Floor Scenarios

**Scenario A: Workspace-wide uniform**
```yaml
COVERAGE_FLOOR: 70
```

**Scenario B: Per-crate (requires parsing JSON in CI)**
```yaml
# More complex; requires script to check each crate
```

Recommend Scenario A for MVP, upgrade to Scenario B later.

---

## Error Handling

### Failure Scenarios

1. **cargo-llvm-cov not found**
   - Mitigation: Install via `taiki-e/install-action`
   - Recovery: CI fails clearly with install error

2. **Coverage < floor**
   - Expected behavior: Job fails
   - Output: Clear message showing coverage vs floor
   - Recovery: Developer must improve tests

3. **JSON parsing fails**
   - Mitigation: `jq` is preinstalled on ubuntu-latest
   - Recovery: Job fails with JSON error

4. **Test job failed**
   - Mitigation: Job depends on "test", skipped if test fails
   - Expected: Coverage job won't run

---

## Crates Included

All workspace members:
- `aid-contract`
- `treasury-contract`
- `referral-contract`
- `governance-contract`
- `oracle-contract`
- `registry-contract`
- `rebalancer-contract` (new, being developed)
- `shared` (common library)
- `testing` (test utilities)

Excluded:
- `brand/` (image assets, not code)

---

## Performance Considerations

### Build Time Impact
- `cargo llvm-cov` requires instrumentation: ~3-5 min additional
- Can be mitigated by:
  - Caching `target/` and `~/.cargo/`
  - Running coverage job only on merge (not every commit)
  - Parallelizing with other jobs (doesn't block)

### Recommendation
- Run coverage on every PR (provides feedback)
- Can extend timeout if coverage takes >5 min
- Consider making optional for forks

---

## Future Enhancements

1. **Codecov Integration**
   - Upload to Codecov.io for historical tracking
   - Requires `CODECOV_TOKEN` secret

2. **Per-Crate Reports**
   - Generate separate HTML reports per contract
   - Upload as artifacts for detailed inspection

3. **HTML Report**
   - Generate HTML coverage report
   - Upload as artifact for human review
   - Command: `cargo llvm-cov --html --output-dir coverage-html`

4. **Trend Tracking**
   - Archive coverage reports over time
   - Create dashboard showing coverage improvement
   - Set per-crate targets

5. **Branch Protection**
   - Set coverage job as required status check
   - Prevent merging if coverage drops

---

## Testing the Implementation

### Local Test
```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run coverage locally
cargo llvm-cov --all-crates --output-format json --output test-coverage.json

# Parse JSON
jq '.coverage' test-coverage.json

# Check floor
COVERAGE=$(jq '.coverage | .[0].percent_covered' test-coverage.json)
if (( $(echo "$COVERAGE >= 70" | bc -l) )); then echo "PASS"; else echo "FAIL"; fi
```

### CI Test
- Push branch with new coverage job
- Monitor workflow execution
- Verify artifact upload
- Verify pass/fail logic

---

## Success Criteria

✅ Coverage job runs and completes  
✅ Artifact uploaded successfully  
✅ Floor enforcement working (pass when >= floor, fail when < floor)  
✅ Baseline documented in issue  
✅ Badge visible in README  
✅ All tests still passing  
✅ No performance regression to build time  

---

## Timeline

**Day 1**:
- Implement coverage job in CI (2-3 hours)
- Test locally and in GitHub Actions (1 hour)

**Day 2**:
- Run baseline (30 min)
- Document in issue and discuss floor (30 min)
- Update README with badge (30 min)
- Final testing and polish (1 hour)

**Total: 1 day** ✓

