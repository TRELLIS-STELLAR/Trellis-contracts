# feat(shared): implement stable error codes with documented remediation hints (#289)

## Overview
Provides a deterministic catalog of stable error codes paired with remediation hints, retryability classifications, backoff schedules, and safe fallback resolution.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `lookup_error_remediation`; only `Admin` and `Maintainer` may trigger `fallback_error_remediation`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `fallback_error_remediation` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::remediation_catalog` stamped with `REMEDIATION_CATALOG_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
