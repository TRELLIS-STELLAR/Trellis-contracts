# feat(shared): implement stale session and token revocation after security events (#283)

## Overview
Tracks per-actor security epoch watermarks and idle timeouts to immediately revoke stale sessions and tokens after role changes, credential rotation, or security incidents.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `validate_or_revoke_session`; only `Admin` and `Maintainer` may trigger `revoke_all_actor_sessions`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `revoke_all_actor_sessions` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::session_revocation` stamped with `SESSION_TOKEN_REVOCATION_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
