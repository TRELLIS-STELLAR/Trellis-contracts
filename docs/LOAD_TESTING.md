# feat(shared): create load test for worst-case cardinality and burst traffic (#273)

## Overview
Simulates worst-case key cardinality and burst traffic spikes to verify bounded cache admission, rate-limit token buckets, priority-lane reservation, and post-burst recovery.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `evaluate_burst_cardinality_step`; only `Admin` and `Maintainer` may trigger `drain_and_recover_burst_state`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `drain_and_recover_burst_state` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::burst_cardinality_load` stamped with `BURST_CARDINALITY_LOAD_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
