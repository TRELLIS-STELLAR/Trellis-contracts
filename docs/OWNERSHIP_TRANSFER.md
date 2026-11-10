# feat(shared): implement safe ownership transfer with pending acceptance and expiry (#286)

## Overview
Enforces two-step ownership transfer with explicit pending acceptance, bounded expiry window, replay-safe nonces, and proposer cancellation.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `propose_guarded_ownership_transfer`; only `Admin` and `Maintainer` may trigger `cancel_or_expire_ownership_transfer`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `cancel_or_expire_ownership_transfer` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::ownership_transfer_guard` stamped with `OWNERSHIP_TRANSFER_GUARD_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
