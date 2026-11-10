# feat(shared): add rollback verification after failed migrations or deployments (#287)

## Overview
Adds automated rollback verification after failed migrations or deployments, checking schema version restoration, solvency parity, zero orphaned locks, and signed audit receipts.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `verify_deployment_rollback`; only `Admin` and `Maintainer` may trigger `rehearse_deployment_rollback`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `rehearse_deployment_rollback` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::deployment_rollback_verify` stamped with `DEPLOYMENT_ROLLBACK_VERIFY_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
