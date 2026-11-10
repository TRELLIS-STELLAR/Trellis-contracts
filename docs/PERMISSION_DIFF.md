# feat(shared): build permission-diff report before role and policy changes (#288)

## Overview
Generates a deterministic permission-diff report before role and policy changes, flagging privilege escalations, enforcing dual-approver signoff, and capturing rollback snapshots.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `compute_permission_diff`; only `Admin` and `Maintainer` may trigger `rollback_permission_change`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `rollback_permission_change` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::permission_diff` stamped with `PERMISSION_DIFF_REPORT_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
