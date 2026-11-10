# feat(shared): implement duplicate suppression for notifications and webhooks (#280)

## Overview
Provides sliding-window idempotency key tracking and payload digest verification to suppress duplicate notifications and webhook deliveries while allowing safe transient retries.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `check_and_suppress_duplicate`; only `Admin` and `Maintainer` may trigger `release_failed_delivery_slot`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `release_failed_delivery_slot` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::duplicate_suppression` stamped with `DUPLICATE_SUPPRESSION_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
