# Code Coverage Reporting - Requirements (#14)

**Issue**: #14 - Add code coverage reporting to CITRELLIS-STELLAR/Trellis-contracts  
**Status**: New Spec  
**Last Updated**: September 24, 2026  

---

## Problem Statement

There is currently no code coverage measurement in the repository. Test counts per crate vary from 0 to 41, but counts are a poor proxy for actual code path coverage. Nobody knows which code paths in `aid-contract` or `treasury-contract` are actually exercised.

Coverage is how you find the untested branch in the refund path before an auditor does. It also gives the programme a visible, objective signal of improvement over time.

---

## Objectives

1. **Measure**: Compute code coverage automatically on every PR
2. **Report**: Upload and display coverage metrics
3. **Establish Baseline**: Document current coverage per crate
4. **Set Floor**: Establish minimum coverage threshold that CI enforces
5. **Visualize**: Add coverage badge to README for visibility

---

## Requirements

### R1: Coverage Computation
- Coverage must be computed using `cargo llvm-cov` (better `no_std` support for Soroban crates than tarpaulin)
- Must run on all contracts in the workspace: `aid-contract`, `treasury-contract`, `referral-contract`, `governance-contract`, `oracle-contract`, `registry-contract`, `rebalancer-contract`
- Must also cover `shared` library (common utilities)
- Must also cover `testing` crate (test utilities)
- Must exclude `brand/` directory (non-code assets)
- JSON output format for machine parsing (`--output-format json`)
- HTML report for human inspection (optional but recommended)

### R2: CI Integration
- Coverage job must run on every PR and push to `main`/`develop`
- Must run after `cargo test` completes successfully (depends on test suite passing)
- Must cache cargo metadata to avoid rebuilding
- Must fail fast if no coverage executable is found

### R3: Upload & Reporting
- JSON coverage report must be uploaded as CI artifact for PR inspection
- Optional: Upload to Codecov.io for historical tracking (requires token, can be deferred)
- Optional: Generate HTML report and upload as artifact (for human review)

### R4: Baseline Documentation
- Document current baseline coverage per crate in issue thread
- Example format:
  ```
  Baseline Coverage (as of Sept 24, 2026):
  - aid-contract: 65%
  - treasury-contract: 72%
  - shared: 88%
  - etc.
  ```

### R5: CI Floor Enforcement
- Set minimum coverage threshold (to be agreed after baseline is known)
- CI job must fail if coverage drops below floor
- Example: "fail if coverage < 70%"
- Floor can vary per crate (e.g., libraries higher than contracts)

### R6: Badge & Visibility
- Add coverage badge to README in badge section (line ~15)
- Badge should display current coverage percentage
- Link badge to coverage reports when available

---

## Acceptance Criteria

### AC1: Coverage Computed & Reported
- ✅ CI workflow includes a "coverage" job
- ✅ Job runs `cargo llvm-cov --output-format json`
- ✅ JSON report uploaded as artifact named `coverage-report.json`
- ✅ Coverage runs after test job (dependency: test job must pass first)

### AC2: Baseline Established
- ✅ Run coverage locally and document baseline per crate in issue #14
- ✅ Baseline format: `crate_name: X%` for each contract and shared lib
- ✅ Timestamp baseline (date when measured)
- ✅ Store baseline as reference for future comparisons

### AC3: Floor Set & Enforced
- ✅ Agree on minimum coverage threshold (consensus in issue discussion)
- ✅ Modify CI workflow to fail if coverage < floor
- ✅ Job output clearly shows "PASS" or "FAIL" against floor

### AC4: Badge Added
- ✅ Add Codecov badge to README badges section (or shields.io static)
- ✅ Badge displays coverage percentage or link to Codecov dashboard
- ✅ Badge is visible when opening repo

### AC5: Documentation Updated
- ✅ README.md mentions coverage and links to badge
- ✅ GitHub Actions workflow documented in comments
- ✅ Baseline documented in issue #14 thread

---

## Design Constraints

- **No_std Support**: Must work with Soroban contracts (which target `wasm32v1-none`)
- **Cache Efficiency**: Leverage GitHub Actions cache for fast reruns
- **Workspace**: Must handle workspace structure with multiple crates
- **Minimal Noise**: Avoid cluttering CI with excessive logging
- **Backwards Compatible**: Don't break existing build/test flow
- **Optional Codecov**: Can defer external upload until token is available

---

## Dependencies

- **Depends on**: Existing `cargo test` CI job (coverage requires tests to run)
- **Does NOT depend on**: Codecov account or external service (can use artifacts only initially)

---

## Skills Required

- GitHub Actions YAML configuration
- Rust tooling (`cargo llvm-cov` installation and flags)
- Markdown (README badge formatting)
- Basic bash (report parsing if needed)

---

## Effort Estimate

**1 day** (per issue estimate)
- 2 hours: Set up `cargo llvm-cov` in CI, test locally
- 1 hour: Run baseline and document in issue
- 2 hours: Add floor enforcement and badge
- 1 hour: Documentation and testing

---

## Files to Modify

- `.github/workflows/ci.yml` - Add coverage job
- `README.md` - Add coverage badge
- `#14 issue thread` - Document baseline and floor agreement

---

## Success Metrics

1. Coverage job runs successfully on every PR
2. Baseline established and documented
3. Floor enforced in CI (CI fails when coverage drops)
4. Badge visible in README
5. All acceptance criteria met

---

## Follow-up Tasks (Future)

- Integrate with Codecov.io for historical trend tracking
- Per-crate coverage reports (breakdown by contract)
- HTML coverage report generation and archiving
- Coverage targets per module (e.g., "errors.rs must be 95%+")
- Trend reporting over time
