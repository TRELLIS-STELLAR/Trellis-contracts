# feat(shared): create fixture generators for rare failure and recovery paths (#285)

## Overview
Provides deterministic fixture generators for rare failure and recovery paths (partial batch settlement, mid-migration abort, quorum split, replay storm) and verifies clean rollback.

## Operational Edge Cases & Recovery / Rollback Behavior
1. **Authorization Boundary**: Only `Admin`, `Maintainer`, and `Operator` roles may invoke `generate_failure_recovery_fixture`; only `Admin` and `Maintainer` may trigger `verify_fixture_recovery_path`. Unauthorized calls return `ModuleError::Unauthorized` without mutating state.
2. **Idempotent Retries & Duplicate Suppression**: Retrying an already-committed `request_id` with `is_retry = true` returns `ExecutionOutcome::IdempotentReplay` with the existing sequence number and state hash. Submitting a duplicate `request_id` with `is_retry = false` returns `ModuleError::ConflictOrDuplicate`.
3. **Failure Budget & Circuit Trip**: Transient failures increment `retry_count` and transition the state to `OperationalLifecycle::Degraded`. Exceeding `MAX_RETRY_ATTEMPTS` trips `ModuleError::RetryBudgetExhausted`.
4. **Rollback & Recovery**: Calling `verify_fixture_recovery_path` restores `current_state_hash` to `pre_commit_snapshot_hash`, clears in-flight item counters, and transitions the state to `OperationalLifecycle::RolledBack`.

## Migration & Compatibility Plan
- Additive module `shared::failure_recovery_fixtures` stamped with `FAILURE_RECOVERY_FIXTURES_V1 = 1`.
- Preserves all existing on-chain contract entrypoints, event topics, and storage layouts.
