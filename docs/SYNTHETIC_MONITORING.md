# feat(shared): create synthetic monitoring for critical end-to-end journey (#291)

## Overview
Adds synthetic monitoring probes across the critical end-to-end journey (deposit, allocation, claim, settlement) with SLO checks, retry budgets, and automatic circuit trip.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `run_synthetic_journey_probe`; only `Admin` and `Maintainer` may trigger `reset_synthetic_journey_circuit`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `reset_synthetic_journey_circuit` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::synthetic_monitoring` stamped with `SYNTHETIC_JOURNEY_MONITOR_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
