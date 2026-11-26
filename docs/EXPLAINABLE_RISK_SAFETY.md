# Explainable high-impact risk safety (#506)

The existing opt-in shared risk engine returns a bounded 0–1000 score, tier,
verdict and factors with threshold, observed value, weight and explanation code.
Evaluation is a public advisory preview; it grants no authorization. Enforcement
now requires the actor's Soroban signature before recording velocity history,
including for exempt actors. Host contracts still supply trusted amounts,
recipient history and strikes, enforce their own roles and financial invariants,
and call `assess_and_record` in the same transaction as the protected mutation.
This change does not silently enable scoring on existing contract entrypoints.

Amount utilization observations saturate at 10,000 basis points (100% of the
reference pool). Higher amounts already receive the maximum 350-point weight.
The calculation now handles i128 maxima without overflowing or narrowing to an
incorrect score. Strike and batch weights, and history increments, saturate
rather than overflow. Exactly 50% starts the amount factor; amounts below it do
not. Negative financial inputs are rejected by enforcement. A zero reference
still disables the ratio factor, preserving the existing nonfinancial-action
contract; callers must supply a positive reference for financial actions.

Policies reject zero velocity windows/burst limits, zero dual-auth or throttle
thresholds, thresholds over 1000, block thresholds at/below dual-auth, and
dormancy windows at/below velocity windows. Admin identity and signature are
still required for policy, exemption and reset changes. Existing stored policies
are not rewritten; review them before enabling the new enforcement code.

## Failure, retry and recovery

Block, dual-auth and throttle verdicts leave actor history unchanged. A host
contract must propagate the returned error; Soroban rolls back writes and events
when its invocation fails, so failure events are not durable evidence. Preserve
the public preview in off-chain diagnostics if needed. The helper does not accept
a co-signer parameter: dual-auth verdicts continue to reject until governance
remediates the policy/context; do not treat a preview as multi-signature approval.

After throttling, retry strictly after the velocity window since the last
successful action. The current history model measures consecutive activity gaps,
not a sliding per-action window. At the boundary the burst remains active.
Recover false positives with an authorized policy adjustment, profile reset or
audited exemption, then retry; remove temporary exemptions afterward. Never
blindly replay a transaction whose settlement is unknown.

No ABI, stored key/layout, error code or event payload changed. Existing callers
must include the actor in the transaction's authorization tree; deployments that
previously recorded on behalf of unsigned actors need a client authorization
update before adoption. Rollback may restore the previous module while retaining
stored profiles/policies, but also restores the prior weaker history protection.

Run `cargo test --locked --manifest-path scripts/contributor-safety/Cargo.toml` for tests
against production sources: extreme values, explanation sums, threshold boundary,
normal recording, denied signatures/admin changes, failed-verdict isolation,
policy correction and cooldown retry. The focused CI workflow runs this suite.
