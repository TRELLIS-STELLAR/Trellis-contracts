# Code Coverage Reporting - Tasks (#14)

**Status**: Ready to Start  
**Total Tasks**: 5  
**Effort**: ~1 day  

---

## Task 1: Set up cargo-llvm-cov locally and establish baseline

- [ ] 1.1 Install cargo-llvm-cov
  ```bash
  cargo install cargo-llvm-cov
  ```

- [ ] 1.2 Run coverage locally
  ```bash
  cargo llvm-cov --all-crates --output-format json --output baseline.json
  ```

- [ ] 1.3 Parse and document baseline
  ```bash
  jq '.coverage' baseline.json
  ```

- [ ] 1.4 Record baseline results
  - Document coverage percentage per crate
  - Format: `crate_name: X%`
  - Include timestamp (Sept 24, 2026)
  - Save results for Task 2 (issue discussion)

- [ ] 1.5 Identify any crates with 0% coverage
  - Flag for inclusion in Task 3 (floor discussion)
  - Note which contracts/libs need more tests

**Acceptance**: Baseline numbers documented and saved

**Effort**: ~30 minutes

---

## Task 2: Add coverage job to CI workflow

- [ ] 2.1 Modify `.github/workflows/ci.yml`
  - Add `needs: test` dependency to coverage job
  - Coverage runs only after tests pass

- [ ] 2.2 Add coverage job steps
  - Checkout repo
  - Install Rust stable (with wasm32v1-none target)
  - Install cargo-llvm-cov via `taiki-e/install-action`
  - Cache cargo (reuse existing template)

- [ ] 2.3 Add coverage command
  - `cargo llvm-cov --all-crates --output-format json --output json-coverage.json`

- [ ] 2.4 Add floor checking script
  - Extract coverage percentage from JSON using `jq`
  - Compare against `COVERAGE_FLOOR` environment variable
  - Output human-readable result

- [ ] 2.5 Add artifact upload
  - Upload `json-coverage.json` as artifact named `coverage-report`
  - Condition: `if: always()` (upload even if floor check fails)

- [ ] 2.6 Test locally
  - Run workflow manually (GitHub Actions)
  - Verify job runs successfully
  - Verify artifact uploads
  - Verify pass/fail logic works

**Acceptance**: Coverage job in CI, runs successfully, artifacts upload

**Effort**: ~2-3 hours

---

## Task 3: Agree on coverage floor and set in CI

- [ ] 3.1 Post baseline results in issue #14
  - Comment with coverage percentages per crate
  - Include total workspace coverage

- [ ] 3.2 Discuss floor with team
  - Should floor be uniform or per-crate?
  - How much regression tolerance?
  - Different floors for different crate types?

- [ ] 3.3 Document agreed floor
  - Record decision in issue #14
  - Example: "Floor agreed: 70% workspace-wide"

- [ ] 3.4 Set floor in CI
  - Update `COVERAGE_FLOOR` env var in workflow
  - Example: `COVERAGE_FLOOR: 70`

- [ ] 3.5 Test floor enforcement
  - Verify CI job passes when coverage >= floor
  - Verify CI job fails when coverage < floor
  - Confirm failure is clear and actionable

**Acceptance**: Floor set, agreed, and enforced in CI

**Effort**: ~1 hour (discussion) + ~30 min (implementation)

---

## Task 4: Add coverage badge to README

- [ ] 4.1 Calculate badge URL
  - Use shields.io format: `https://img.shields.io/badge/coverage-PERCENT%25-COLOR?style=flat-square`
  - Color: `1C6B55` (matches Trellis brand)
  - Percentage: Use baseline from Task 1

- [ ] 4.2 Add badge to README
  - Insert in badges section (line ~15, after existing badges)
  - HTML img tag format to match existing badges:
    ```html
    <img src="https://img.shields.io/badge/coverage-75%25-1C6B55?style=flat-square" alt="Coverage">
    ```

- [ ] 4.3 Verify badge displays
  - Check README on GitHub
  - Confirm badge appears and links correctly

- [ ] 4.4 Document coverage in README
  - Consider adding "Testing" section if not present
  - Link to coverage reports
  - Mention: "Coverage is computed on every PR using cargo llvm-cov"

**Acceptance**: Badge visible in README, documents current coverage

**Effort**: ~30 minutes

---

## Task 5: Documentation and final testing

- [ ] 5.1 Document baseline in issue #14
  - Add comment with: "Baseline Coverage (Sept 24, 2026)"
  - List per-crate percentages
  - Note any low-coverage areas

- [ ] 5.2 Document floor decision
  - Link to agreed floor
  - Explain rationale if different per crate

- [ ] 5.3 Update CI workflow comments
  - Add inline comments explaining each coverage step
  - Document `COVERAGE_FLOOR` purpose

- [ ] 5.4 Create any follow-up issues
  - If crates have 0% coverage: File issue to add tests
  - If coverage low in critical paths: Note for future improvement

- [ ] 5.5 Final verification
  - Run CI on a PR branch
  - Confirm workflow completes
  - Verify artifact uploads
  - Verify badge displays correctly
  - Test both pass (coverage >= floor) and fail (coverage < floor) scenarios

- [ ] 5.6 Close out documentation
  - Comment in #14: "Coverage reporting implemented"
  - Link to baseline and floor documentation
  - Mark ready for team review

**Acceptance**: All documentation complete, final tests pass, issue documented

**Effort**: ~1 hour

---

## Verification Checklist

After all tasks complete:

✅ Task 1: Baseline established and documented  
✅ Task 2: Coverage job in CI, running successfully  
✅ Task 3: Floor set and enforced, CI fails/passes correctly  
✅ Task 4: Badge added to README and displays  
✅ Task 5: Documentation complete, issue #14 updated  

**Additional Checks**:
- [ ] Artifact uploads on every PR
- [ ] Coverage computation includes all crates
- [ ] Floor check provides clear pass/fail message
- [ ] Badge links correctly (or at least displays)
- [ ] No regression in build time
- [ ] All existing CI jobs still pass

---

## Acceptance Criteria from Requirements

**AC1: Coverage Computed & Reported**
- ✅ CI workflow includes coverage job
- ✅ Runs `cargo llvm-cov --output-format json`
- ✅ JSON report uploaded as artifact
- ✅ Coverage runs after test job

**AC2: Baseline Established**
- ✅ Coverage run locally and documented
- ✅ Baseline per-crate in issue
- ✅ Timestamped
- ✅ Stored as reference

**AC3: Floor Set & Enforced**
- ✅ Minimum threshold agreed
- ✅ CI fails if coverage < floor
- ✅ Clear pass/fail messages

**AC4: Badge Added**
- ✅ Badge in README badges section
- ✅ Displays coverage percentage
- ✅ Visible when opening repo

**AC5: Documentation Updated**
- ✅ README mentions coverage
- ✅ CI workflow documented
- ✅ Baseline in issue #14

---

## Notes

- **Local testing critical**: Run `cargo llvm-cov` locally first before CI (Task 1)
- **Floor discussion important**: Get team consensus to avoid rework (Task 3)
- **Badge update**: May need to update manually if using shields.io (not auto-updating)
- **Future enhancement**: Codecov integration for auto-updating badge (out of scope for MVP)

