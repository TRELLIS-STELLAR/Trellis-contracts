# feat(shared): build versioned event replay tool for rebuilding projections (#276)

## Overview
Rebuilds read projections from versioned event streams with V1-to-V2 upcasting, sequence gap detection, resumable checkpoints, and pre-rebuild snapshot rollback.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `apply_versioned_event_batch`; only `Admin` and `Maintainer` may trigger `rollback_projection_to_checkpoint`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `rollback_projection_to_checkpoint` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::versioned_event_replay` stamped with `VERSIONED_EVENT_REPLAY_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
