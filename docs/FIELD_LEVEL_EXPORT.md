# feat(shared): build privacy-safe export with field-level authorization (#294)

## Overview
Implements privacy-safe data export with field-level sensitivity classification, caller role authorization, deterministic redaction, retry-safe cursors, and export revocation.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `evaluate_field_export`; only `Admin` and `Maintainer` may trigger `revoke_field_export_session`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `revoke_field_export_session` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::field_level_export` stamped with `FIELD_LEVEL_EXPORT_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
