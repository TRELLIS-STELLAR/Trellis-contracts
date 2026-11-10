# feat(shared): create public health endpoint with dependency-aware status (#279)

## Overview
Aggregates dependency health probes into a sanitized public health status surface with staleness detection, authorized reporter gates, and recovery hysteresis.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `compute_dependency_aware_health`; only `Admin` and `Maintainer` may trigger `recover_dependency_health_state`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `recover_dependency_health_state` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::dependency_health_endpoint` stamped with `DEPENDENCY_HEALTH_ENDPOINT_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
