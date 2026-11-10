# feat(shared): implement feature rollout guardrails with emergency disablement (#295)

## Overview
Adds staged rollout guardrails with error-budget monitoring, authorization checks, idempotent retry evaluation, and emergency disablement with rollback verification.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `evaluate_rollout_guardrail`; only `Admin` and `Maintainer` may trigger `emergency_disable_rollout`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `emergency_disable_rollout` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::rollout_guardrails` stamped with `ROLLOUT_GUARDRAILS_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
