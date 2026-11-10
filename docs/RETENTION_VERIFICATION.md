# feat(shared): add retention and deletion tests for sensitive operational data (#281)

## Overview
Enforces TTL retention policies, legal-hold exemptions, and two-phase tombstone-and-purge verification for sensitive operational data.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `evaluate_sensitive_retention`; only `Admin` and `Maintainer` may trigger `restore_held_retention_record`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `restore_held_retention_record` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::sensitive_retention_verify` stamped with `SENSITIVE_RETENTION_VERIFY_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
