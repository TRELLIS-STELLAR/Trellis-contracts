# feat(shared): implement strict allow-list validation for URLs, file types, and destinations (#277)

## Overview
Implements strict allow-list validation for external URLs, MIME/file extensions, and settlement destinations with control-byte, traversal, and spoofing rejection.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `validate_against_allowlist`; only `Admin` and `Maintainer` may trigger `quarantine_rejected_destination`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `quarantine_rejected_destination` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::allowlist_validator` stamped with `STRICT_ALLOWLIST_VALIDATOR_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
