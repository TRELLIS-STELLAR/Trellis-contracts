# feat(shared): add recovery checkpoints around every external side effect (#275)

## Overview
Wraps external side effects (token transfers, oracle queries, cross-contract calls) in persistent pre/post recovery checkpoints with idempotency tokens and compensating rollback.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `begin_and_verify_side_effect`; only `Admin` and `Maintainer` may trigger `rollback_failed_side_effect`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `rollback_failed_side_effect` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::side_effect_checkpoints` stamped with `SIDE_EFFECT_CHECKPOINTS_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
