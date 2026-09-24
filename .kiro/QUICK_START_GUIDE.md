# Quick Start Guide for Implementation

**Created**: September 24, 2026  
**Purpose**: Fast reference for starting work on either workstream  

---

## 🚀 Ready to Start? Choose Your Path

### Path A: Strategy Executor Implementation (Phases 3.2+)

**Time Commitment**: 10-12 hours remaining  
**Complexity**: Medium (3 strategy variants, error handling)  
**Start Here**: Phase 3.2 (Token Resolution)  

**Quick Setup**:
```bash
# Get context
cat .kiro/specs/strategy-executor-implementation/design.md
cat .kiro/specs/strategy-executor-implementation/tasks.md

# Start implementing
vim contracts/rebalancer-contract/src/strategy_executor.rs

# Run tests after each change
cargo test --package rebalancer-contract
```

**First Task** (Task 3.2):
- Implement `resolve_token_address()` helper
- Integrate `safe_transfer_from_contract` calls
- Capture real execution data
- Tests from Phase 1-2 will verify your work

**Key Files**:
- Implementation: `contracts/rebalancer-contract/src/strategy_executor.rs`
- Tests: Same file (4 bug exploration + 7 preservation)
- Design: `.kiro/specs/strategy-executor-implementation/design.md`
- Tasks: `.kiro/specs/strategy-executor-implementation/tasks.md`

---

### Path B: Code Coverage Reporting (Tasks 1-5)

**Time Commitment**: ~1 day  
**Complexity**: Low (tooling, automation, documentation)  
**Start Here**: Task 1 (Baseline Measurement)  

**Quick Setup**:
```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run baseline
cargo llvm-cov --all-crates --output-format json --output baseline.json

# View results
jq '.coverage' baseline.json
```

**First Task** (Task 1):
- Run `cargo llvm-cov` locally
- Document coverage per crate
- Save results for Task 3 (floor discussion)
- Post results in issue #14

**Key Files**:
- Requirements: `.kiro/specs/code-coverage-reporting/requirements.md`
- Design: `.kiro/specs/code-coverage-reporting/design.md`
- Tasks: `.kiro/specs/code-coverage-reporting/tasks.md`
- CI Workflow: `.github/workflows/ci.yml` (coverage job already added)
- README: `README.md` (badge already added, needs baseline)

---

## 📋 Task Checklist Quick Reference

### Strategy Executor Phase 3.2 Checklist

```
Task 3.2: Token Resolution & Payment Integration

[ ] Implement resolve_token_address() helper
    - Convert Symbol → Address
    - Handle oracle/registry lookup
    
[ ] Implement resolve_recipient_from_context() helper
    - Determine transfer destination
    - Validate recipient address
    
[ ] Integrate safe_transfer_from_contract()
    - Call for each trade
    - Capture actual price and fee
    
[ ] Update ExecutionSummary
    - Increment successful_trades
    - Accumulate total_fees_paid
    
[ ] Handle transfer failures
    - Record TradeError in errors vector
    - Continue to next trade or fail based on strategy
    
[ ] Run tests
    - Bug exploration tests should show progress
    - Preservation tests should still pass
    
[ ] Code review & merge
```

### Coverage Task 1 Checklist

```
Task 1: Establish Baseline

[ ] Install cargo-llvm-cov
    cargo install cargo-llvm-cov
    
[ ] Run coverage locally
    cargo llvm-cov --all-crates --output-format json --output baseline.json
    
[ ] Parse results
    jq '.coverage' baseline.json
    
[ ] Document findings
    - Total workspace coverage: X%
    - Per-crate: aid-contract: X%, treasury: Y%, etc.
    - Any 0% coverage areas: list
    
[ ] Post in issue #14
    - Comment with baseline numbers
    - Note any concerns
    
[ ] Ready for Task 3
```

---

## 🔍 Key Documentation

### Strategy Executor

| Document | Purpose | Read Time |
|----------|---------|-----------|
| `bugfix.md` | Detailed bug description | 15 min |
| `design.md` | Implementation approach | 20 min |
| `tasks.md` | Task breakdown | 10 min |
| `SESSION_PROGRESS_SUMMARY.md` | What's done, what's ready | 10 min |
| `TASK3_1_RESULT_TYPE_COMPLETION.md` | Type system details | 15 min |

**Start With**: `SESSION_PROGRESS_SUMMARY.md` (overview of where you are)

### Code Coverage

| Document | Purpose | Read Time |
|----------|---------|-----------|
| `requirements.md` | What needs to happen | 10 min |
| `design.md` | How to do it | 15 min |
| `tasks.md` | Step-by-step breakdown | 10 min |

**Start With**: `requirements.md` (understand the goal)

---

## ✅ Acceptance Criteria at a Glance

### Strategy Executor Phase 3.2

```
✓ resolve_token_address() implemented and tested
✓ resolve_recipient_from_context() implemented and tested
✓ safe_transfer_from_contract called for each trade
✓ Real execution price and fee captured
✓ ExecutionSummary.successful_trades populated correctly
✓ ExecutionSummary.total_fees_paid accumulated
✓ TradeError recorded on transfer failure
✓ Tests from Phase 1 show progress toward passing
✓ Tests from Phase 2 still passing (no regressions)
```

### Coverage Task 1

```
✓ cargo-llvm-cov installed
✓ Baseline measured for all crates
✓ Results documented (per-crate percentages)
✓ Posted in issue #14 with timestamp
✓ Ready for Task 3 (floor agreement)
```

---

## 🐛 Test Your Work

### Strategy Executor

```bash
# After each change, run tests
cargo test --package rebalancer-contract

# Watch for these tests to pass/fail:
# - test_bug_condition_strategy_ignored_and_bare_bool_return
# - test_bug_condition_bare_bool_return_no_failure_details
# - test_bug_condition_log_trade_hardcoded_zero_values
# - preservation_* (all 7 preservation tests)

# Run full workspace test to check for regressions
cargo test --workspace
```

### Code Coverage

```bash
# After Task 1 baseline:
cargo llvm-cov --all-crates --output-format json --output baseline.json
jq '.coverage' baseline.json

# After CI coverage job is live (Task 2):
# Watch the GitHub Actions workflow
# Verify artifact uploads

# After floor is set (Task 3):
# Push a branch and verify CI passes/fails correctly
git push origin feature/test-coverage
# Check GitHub Actions workflow result
```

---

## 📞 Getting Help

### Strategy Executor Questions?
- Check: `.kiro/specs/strategy-executor-implementation/design.md` (algorithm details)
- Check: `contracts/rebalancer-contract/src/strategy_executor.rs` (test examples)
- Check: `shared/src/payments.rs` (understand safe_transfer_from_contract)

### Code Coverage Questions?
- Check: `.kiro/specs/code-coverage-reporting/design.md` (implementation details)
- Check: `.github/workflows/ci.yml` (YAML structure)
- Run: `cargo llvm-cov --help` (tool options)

### General Questions?
- See: `.kiro/specs/WORKSTREAM_SUMMARY.md` (overview of both)
- See: `.kiro/EXECUTION_SUMMARY_20260924.md` (what was done today)

---

## ⏱️ Time Estimates

### Strategy Executor
- Phase 3.2 (Token Resolution): 2-3 hours
- Phase 3.3-3.5 (Strategy Implementations): 3-4 hours
- Phase 3.6 (log_trade fix): 1-2 hours
- Phase 3.7 (Error Handling): 1 hour
- Phases 4-8 (Testing & Integration): 2-3 hours
- **Total Remaining**: ~10-12 hours

### Code Coverage
- Task 1 (Baseline): 30 min
- Task 2 (CI Verification): 2-3 hours
- Task 3 (Floor Agreement): 1-2 hours
- Task 4 (Badge Update): 30 min
- Task 5 (Documentation): 1 hour
- **Total**: ~1 day (per issue estimate)

---

## 🚦 Status Dashboard

### Strategy Executor Implementation
```
Phase 1: ✅ COMPLETE (Bug exploration tests)
Phase 2: ✅ COMPLETE (Preservation tests)
Phase 3.1: ✅ COMPLETE (Result type implementation)
Phase 3.2: ⏳ READY (Start here or continue from 3.1)
Phase 3.3: ⏳ READY
Phase 3.4: ⏳ READY
Phase 3.5: ⏳ READY
Phase 3.6: ⏳ READY
Phase 3.7: ⏳ READY
Phase 4-8: ⏳ READY (After Phase 3 complete)
```

### Code Coverage Reporting
```
Spec: ✅ COMPLETE (Requirements, Design, Tasks)
CI Job: ✅ IMPLEMENTED (Added to workflow)
Badge: ✅ ADDED (README updated)
Task 1: ⏳ READY (Start here)
Task 2: ⏳ READY
Task 3: ⏳ READY
Task 4: ⏳ READY
Task 5: ⏳ READY
```

---

## 🎯 Next Immediate Actions

**Choose One**:

### Option A: Strategy Executor
```bash
# Get started
cd contracts/rebalancer-contract/src
vim strategy_executor.rs

# Implement Task 3.2 starting at line ~72
# Look for: // TODO: Task 3.2 - Token resolution

# Run tests frequently
cargo test --package rebalancer-contract
```

### Option B: Code Coverage
```bash
# Install the tool
cargo install cargo-llvm-cov

# Run baseline measurement
cargo llvm-cov --all-crates --output-format json --output baseline.json

# View results
jq '.coverage' baseline.json

# Post results in issue #14 and discuss floor
```

---

## 💾 Reference Commands

### Cargo Useful Commands

```bash
# Run all tests
cargo test --workspace

# Run tests for one package
cargo test --package rebalancer-contract

# Build without tests
cargo build --release

# Check code
cargo check

# Format code
cargo fmt

# Lint
cargo clippy

# Measure coverage
cargo llvm-cov --all-crates --output-format json
```

### Git Useful Commands

```bash
# Create feature branch
git checkout -b feature/strategy-task-3-2

# View uncommitted changes
git diff

# Stage changes
git add .

# Commit
git commit -m "Task 3.2: Implement token resolution"

# Push to remote
git push origin feature/strategy-task-3-2

# Create PR
gh pr create --title "Task 3.2: Token Resolution" --body "See issue #ISSUE_NUM"
```

---

## 📚 Related Issues

- **#14**: Code Coverage Reporting (THIS WORKSTREAM)
- **#TBD**: Strategy Executor Bugfix (THIS WORKSTREAM)

---

**Last Updated**: September 24, 2026  
**Version**: 1.0  
**Status**: Ready for Implementation ✓  

