# feat(shared): add deterministic sorting for mixed-status records and events (#284)

## Overview
Implements deterministic total ordering for mixed-status records and lifecycle events with status precedence, ledger sequence, tie-breaker IDs, and stable cursor pagination.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `sort_mixed_status_records`; only `Admin` and `Maintainer` may trigger `reset_sort_cursor_checkpoint`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `reset_sort_cursor_checkpoint` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::deterministic_sort` stamped with `DETERMINISTIC_MIXED_SORT_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
