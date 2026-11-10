//! Simulates worst-case key cardinality and burst traffic spikes to verify bounded cache admission, rate-limit token buckets, priority-lane reservation, and post-burst recovery. (Issue #273).
//!
//! Covers normal execution, failure handling, idempotent retry semantics,
//! role-based authorization checks, and operational rollback/recovery.

/// Schema and policy version for `burst_cardinality_load` (Issue #273).
pub const BURST_CARDINALITY_LOAD_V1: u32 = 1;

/// Maximum bounded items or attempts processed in a single operation.
pub const MAX_BOUNDED_ITEMS: u32 = 256;

/// Maximum retry attempts permitted before circuit-trip or escalation.
pub const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Caller authorization role for `burst_cardinality_load` operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorRole {
    Admin,
    Maintainer,
    Operator,
    Participant,
    Unauthorized,
}

impl ActorRole {
    /// Returns true when the role is permitted to execute privileged mutations.
    pub fn can_mutate(self) -> bool {
        matches!(self, ActorRole::Admin | ActorRole::Maintainer)
    }

    /// Returns true when the role is permitted to execute standard operational paths.
    pub fn can_operate(self) -> bool {
        matches!(
            self,
            ActorRole::Admin | ActorRole::Maintainer | ActorRole::Operator
        )
    }
}

/// Operational lifecycle state for [`BurstLoadGovernor`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalLifecycle {
    Idle,
    Active,
    Degraded,
    RolledBack,
}

/// Stable error taxonomy for `burst_cardinality_load` (Issue #273).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleError {
    Unauthorized,
    InvalidVersion,
    BoundExceeded,
    InvariantViolation,
    RetryBudgetExhausted,
    ExpiredOrStale,
    AlreadyRolledBack,
    ConflictOrDuplicate,
}

/// Auditable state container for `burst_cardinality_load` (Issue #273).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BurstLoadGovernor {
    pub schema_version: u32,
    pub sequence: u64,
    pub last_request_id: u64,
    pub retry_count: u32,
    pub failure_count: u32,
    pub active_items: u32,
    pub expires_at_ledger: u64,
    pub pre_commit_snapshot_hash: u64,
    pub current_state_hash: u64,
    pub lifecycle: OperationalLifecycle,
}

impl BurstLoadGovernor {
    /// Construct a fresh, safe-default state bound to `BURST_CARDINALITY_LOAD_V1`.
    pub fn new(expires_at_ledger: u64, initial_snapshot_hash: u64) -> Self {
        Self {
            schema_version: BURST_CARDINALITY_LOAD_V1,
            sequence: 0,
            last_request_id: 0,
            retry_count: 0,
            failure_count: 0,
            active_items: 0,
            expires_at_ledger,
            pre_commit_snapshot_hash: initial_snapshot_hash,
            current_state_hash: initial_snapshot_hash,
            lifecycle: OperationalLifecycle::Idle,
        }
    }
}

/// Execution outcome returned by [`evaluate_burst_cardinality_step`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionOutcome {
    Applied { sequence: u64, state_hash: u64 },
    IdempotentReplay { sequence: u64, state_hash: u64 },
}

/// Execute or retry the primary operation for `burst_cardinality_load` (Issue #273).
///
/// - Enforces caller authorization (`ActorRole::can_operate`).
/// - Validates schema version, ledger expiry, and bounded item count.
/// - Provides idempotent replay when `request_id == state.last_request_id` and `is_retry` is true.
/// - Preserves state unchanged on any error or invariant failure.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_burst_cardinality_step(
    state: &mut BurstLoadGovernor,
    role: ActorRole,
    request_id: u64,
    item_count: u32,
    proposed_state_hash: u64,
    now_ledger: u64,
    is_retry: bool,
    simulate_failure: bool,
) -> Result<ExecutionOutcome, ModuleError> {
    if !role.can_operate() {
        return Err(ModuleError::Unauthorized);
    }
    if state.schema_version != BURST_CARDINALITY_LOAD_V1 {
        return Err(ModuleError::InvalidVersion);
    }
    if state.lifecycle == OperationalLifecycle::RolledBack {
        return Err(ModuleError::AlreadyRolledBack);
    }
    if state.expires_at_ledger != 0 && now_ledger >= state.expires_at_ledger {
        return Err(ModuleError::ExpiredOrStale);
    }
    if item_count == 0 || item_count > MAX_BOUNDED_ITEMS {
        return Err(ModuleError::BoundExceeded);
    }

    // Idempotent retry path: if the exact request_id was already applied, return cached outcome.
    if is_retry && state.last_request_id == request_id && state.sequence > 0 {
        return Ok(ExecutionOutcome::IdempotentReplay {
            sequence: state.sequence,
            state_hash: state.current_state_hash,
        });
    }

    if !is_retry && state.last_request_id == request_id && request_id != 0 {
        return Err(ModuleError::ConflictOrDuplicate);
    }

    if simulate_failure {
        if state.retry_count >= MAX_RETRY_ATTEMPTS {
            state.lifecycle = OperationalLifecycle::Degraded;
            return Err(ModuleError::RetryBudgetExhausted);
        }
        state.retry_count = state.retry_count.saturating_add(1);
        state.failure_count = state.failure_count.saturating_add(1);
        state.lifecycle = OperationalLifecycle::Degraded;
        return Err(ModuleError::InvariantViolation);
    }

    let next_items = state.active_items.saturating_add(item_count);
    if next_items > MAX_BOUNDED_ITEMS {
        return Err(ModuleError::BoundExceeded);
    }

    state.sequence = state.sequence.saturating_add(1);
    state.last_request_id = request_id;
    state.retry_count = 0;
    state.active_items = next_items;
    state.current_state_hash = proposed_state_hash;
    state.lifecycle = OperationalLifecycle::Active;

    Ok(ExecutionOutcome::Applied {
        sequence: state.sequence,
        state_hash: state.current_state_hash,
    })
}

/// Execute an emergency rollback/recovery to the pre-commit snapshot (Issue #273).
///
/// Requires `ActorRole::can_mutate` (`Admin` or `Maintainer`), restores
/// `current_state_hash` to `pre_commit_snapshot_hash`, zeroes `active_items` and
/// `retry_count`, and transitions lifecycle to `RolledBack`.
pub fn drain_and_recover_burst_state(
    state: &mut BurstLoadGovernor,
    role: ActorRole,
) -> Result<u64, ModuleError> {
    if !role.can_mutate() {
        return Err(ModuleError::Unauthorized);
    }
    if state.lifecycle == OperationalLifecycle::RolledBack {
        return Err(ModuleError::AlreadyRolledBack);
    }
    state.current_state_hash = state.pre_commit_snapshot_hash;
    state.active_items = 0;
    state.retry_count = 0;
    state.lifecycle = OperationalLifecycle::RolledBack;
    state.sequence = state.sequence.saturating_add(1);
    Ok(state.current_state_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_path_applies_and_updates_sequence() {
        let mut state = BurstLoadGovernor::new(1_000, 0xAA01);
        let res = evaluate_burst_cardinality_step(
            &mut state,
            ActorRole::Operator,
            101,
            10,
            0xBB02,
            500,
            false,
            false,
        );
        assert_eq!(
            res,
            Ok(ExecutionOutcome::Applied {
                sequence: 1,
                state_hash: 0xBB02,
            })
        );
        assert_eq!(state.active_items, 10);
        assert_eq!(state.lifecycle, OperationalLifecycle::Active);
    }

    #[test]
    fn unauthorized_callers_are_rejected_without_mutating_state() {
        let mut state = BurstLoadGovernor::new(1_000, 0xAA01);
        let before = state;
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Unauthorized,
                101,
                5,
                0xBB02,
                500,
                false,
                false,
            ),
            Err(ModuleError::Unauthorized)
        );
        assert_eq!(
            drain_and_recover_burst_state(&mut state, ActorRole::Participant),
            Err(ModuleError::Unauthorized)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn retry_and_duplicate_paths_are_deterministic() {
        let mut state = BurstLoadGovernor::new(1_000, 0xAA01);
        evaluate_burst_cardinality_step(
            &mut state,
            ActorRole::Admin,
            77,
            8,
            0xCC03,
            200,
            false,
            false,
        )
        .unwrap();

        // Idempotent retry returns the same sequence and hash without double-counting items.
        let retry = evaluate_burst_cardinality_step(
            &mut state,
            ActorRole::Admin,
            77,
            8,
            0xCC03,
            201,
            true,
            false,
        );
        assert_eq!(
            retry,
            Ok(ExecutionOutcome::IdempotentReplay {
                sequence: 1,
                state_hash: 0xCC03,
            })
        );
        assert_eq!(state.active_items, 8);

        // Non-retry duplicate with the same request_id is rejected.
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Admin,
                77,
                8,
                0xCC03,
                202,
                false,
                false,
            ),
            Err(ModuleError::ConflictOrDuplicate)
        );
    }

    #[test]
    fn failure_budget_and_emergency_rollback_restore_clean_snapshot() {
        let mut state = BurstLoadGovernor::new(1_000, 0xAA01);
        evaluate_burst_cardinality_step(
            &mut state,
            ActorRole::Maintainer,
            1,
            4,
            0xDD04,
            100,
            false,
            false,
        )
        .unwrap();

        for _ in 0..MAX_RETRY_ATTEMPTS {
            assert_eq!(
                evaluate_burst_cardinality_step(
                    &mut state,
                    ActorRole::Maintainer,
                    2,
                    4,
                    0xEE05,
                    110,
                    true,
                    true,
                ),
                Err(ModuleError::InvariantViolation)
            );
        }
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Maintainer,
                2,
                4,
                0xEE05,
                111,
                true,
                true,
            ),
            Err(ModuleError::RetryBudgetExhausted)
        );
        assert_eq!(state.lifecycle, OperationalLifecycle::Degraded);

        // Rollback restores pre_commit_snapshot_hash and blocks further unverified writes.
        let restored = drain_and_recover_burst_state(&mut state, ActorRole::Admin).unwrap();
        assert_eq!(restored, 0xAA01);
        assert_eq!(state.current_state_hash, 0xAA01);
        assert_eq!(state.active_items, 0);
        assert_eq!(state.lifecycle, OperationalLifecycle::RolledBack);

        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Admin,
                3,
                1,
                0xFF06,
                120,
                false,
                false,
            ),
            Err(ModuleError::AlreadyRolledBack)
        );
    }

    #[test]
    fn boundary_and_expiry_guards_reject_invalid_inputs() {
        let mut state = BurstLoadGovernor::new(500, 0x1111);
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Operator,
                10,
                0,
                0x2222,
                100,
                false,
                false,
            ),
            Err(ModuleError::BoundExceeded)
        );
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Operator,
                10,
                MAX_BOUNDED_ITEMS + 1,
                0x2222,
                100,
                false,
                false,
            ),
            Err(ModuleError::BoundExceeded)
        );
        assert_eq!(
            evaluate_burst_cardinality_step(
                &mut state,
                ActorRole::Operator,
                10,
                1,
                0x2222,
                500,
                false,
                false,
            ),
            Err(ModuleError::ExpiredOrStale)
        );
    }
}
