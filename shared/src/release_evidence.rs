//! Release gate for migration and recovery evidence (Issue #586).
//!
//! [`crate::release_readiness`] accepts any [`GateCheck`] whose status is `Passed`, so the
//! migration and rollback pillars can be marked green without proof. This module records
//! that proof per release and module, and only produces passing `Migration` and
//! `RollbackRehearsal` checks when the proof exists and is consistent:
//!
//! - **Migration evidence**: a forward schema step, a non-zero dry-run journal hash, and
//!   zero unmigrated records.
//! - **Recovery evidence**: a stored, passing [`RollbackRehearsalReceipt`] for the same
//!   module, rehearsed against the version the migration moves to.
//!
//! Recording is admin-only. A rejected submission stores nothing, so operators fix the
//! input and resubmit; an accepted resubmission replaces the previous record and bumps
//! its `attempt` counter. See `docs/RELEASE_READINESS.md` ("Migration and recovery
//! evidence") for edge cases and recovery behaviour.
//!
//! [`RollbackRehearsalReceipt`]: crate::release_readiness::RollbackRehearsalReceipt

use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

use crate::auth::require_admin;
use crate::migration::RollbackStrategy;
use crate::release_readiness::{
    create_gate_check, get_rehearsal_receipt, GateCheck, GateCheckStatus, GateKind,
    ReleaseGateError, ReleaseModule,
};
use crate::storage::{instance_get, instance_set};

const KEY_MIG_EVIDENCE: Symbol = symbol_short!("rel_mig");
const KEY_RCV_EVIDENCE: Symbol = symbol_short!("rel_rcv");

/// Proof that a module's migration for a release was dry-run and fully covers its records.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationEvidence {
    pub module: ReleaseModule,
    pub from_version: u32,
    pub to_version: u32,
    /// Hash of the dry-run migration journal the evidence was taken from.
    pub dry_run_journal: BytesN<32>,
    pub recorded_by: Address,
    pub recorded_at: u64,
    /// 1 on the first accepted submission, incremented on every accepted retry.
    pub attempt: u32,
}

/// Proof that a module can be recovered if the release's migration fails.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryEvidence {
    pub module: ReleaseModule,
    pub rehearsal_id: u64,
    /// Version the rehearsal rolled back from; must equal the migration's `to_version`.
    pub rehearsed_version: u32,
    pub strategy: RollbackStrategy,
    pub recorded_by: Address,
    pub recorded_at: u64,
    /// 1 on the first accepted submission, incremented on every accepted retry.
    pub attempt: u32,
}

fn authorize(env: &Env, admin: &Address) -> Result<(), ReleaseGateError> {
    require_admin(env, admin).map_err(|_| ReleaseGateError::UnauthorizedSignoff)
}

/// Records migration evidence for `module` in `release_version`.
///
/// Rejects (and stores nothing) when the version does not move forward, the dry-run
/// journal hash is all zeros, or any record was left unmigrated.
#[allow(clippy::too_many_arguments)] // mirrors the evidence fields one-to-one
pub fn record_migration_evidence(
    env: &Env,
    admin: &Address,
    release_version: &Symbol,
    module: ReleaseModule,
    from_version: u32,
    to_version: u32,
    dry_run_journal: BytesN<32>,
    unmigrated_records: u32,
) -> Result<MigrationEvidence, ReleaseGateError> {
    authorize(env, admin)?;
    if to_version <= from_version
        || unmigrated_records > 0
        || dry_run_journal == BytesN::from_array(env, &[0u8; 32])
    {
        return Err(ReleaseGateError::MissingMigrationVerification);
    }

    let key = (KEY_MIG_EVIDENCE, release_version.clone(), module);
    let attempt = instance_get::<_, MigrationEvidence>(env, &key)
        .map_or(1, |prev| prev.attempt.saturating_add(1));
    let evidence = MigrationEvidence {
        module,
        from_version,
        to_version,
        dry_run_journal,
        recorded_by: admin.clone(),
        recorded_at: env.ledger().timestamp(),
        attempt,
    };
    instance_set(env, &key, &evidence);
    env.events().publish(
        (KEY_MIG_EVIDENCE, release_version.clone()),
        (module as u32, to_version, attempt),
    );
    Ok(evidence)
}

/// Records recovery evidence for `module` in `release_version` from a stored rehearsal.
///
/// Rejects (and stores nothing) when the receipt does not exist, belongs to another
/// module, did not pass with state intact, or relied on a `Manual` strategy.
pub fn record_recovery_evidence(
    env: &Env,
    admin: &Address,
    release_version: &Symbol,
    module: ReleaseModule,
    rehearsal_id: u64,
) -> Result<RecoveryEvidence, ReleaseGateError> {
    authorize(env, admin)?;
    let receipt = get_rehearsal_receipt(env, rehearsal_id)
        .ok_or(ReleaseGateError::MissingRollbackRehearsal)?;
    if receipt.module != module {
        return Err(ReleaseGateError::InvalidModule);
    }
    if !receipt.passed || !receipt.state_intact || receipt.strategy == RollbackStrategy::Manual {
        return Err(ReleaseGateError::RehearsalSimulationFailed);
    }

    let key = (KEY_RCV_EVIDENCE, release_version.clone(), module);
    let attempt = instance_get::<_, RecoveryEvidence>(env, &key)
        .map_or(1, |prev| prev.attempt.saturating_add(1));
    let evidence = RecoveryEvidence {
        module,
        rehearsal_id,
        rehearsed_version: receipt.from_version,
        strategy: receipt.strategy,
        recorded_by: admin.clone(),
        recorded_at: env.ledger().timestamp(),
        attempt,
    };
    instance_set(env, &key, &evidence);
    env.events().publish(
        (KEY_RCV_EVIDENCE, release_version.clone()),
        (module as u32, rehearsal_id, attempt),
    );
    Ok(evidence)
}

/// Returns the migration evidence recorded for `module` in `release_version`, if any.
pub fn get_migration_evidence(
    env: &Env,
    release_version: &Symbol,
    module: ReleaseModule,
) -> Option<MigrationEvidence> {
    instance_get(env, &(KEY_MIG_EVIDENCE, release_version.clone(), module))
}

/// Returns the recovery evidence recorded for `module` in `release_version`, if any.
pub fn get_recovery_evidence(
    env: &Env,
    release_version: &Symbol,
    module: ReleaseModule,
) -> Option<RecoveryEvidence> {
    instance_get(env, &(KEY_RCV_EVIDENCE, release_version.clone(), module))
}

/// Checks one module's evidence for `release_version`.
///
/// Fails with `MissingMigrationVerification` when there is no migration evidence and
/// with `MissingRollbackRehearsal` when recovery evidence is missing or was rehearsed
/// against a different version than the migration targets (stale after a retry).
pub fn check_module_evidence(
    env: &Env,
    release_version: &Symbol,
    module: ReleaseModule,
) -> Result<(), ReleaseGateError> {
    let migration = get_migration_evidence(env, release_version, module)
        .ok_or(ReleaseGateError::MissingMigrationVerification)?;
    let recovery = get_recovery_evidence(env, release_version, module)
        .ok_or(ReleaseGateError::MissingRollbackRehearsal)?;
    if recovery.rehearsed_version != migration.to_version {
        return Err(ReleaseGateError::MissingRollbackRehearsal);
    }
    Ok(())
}

/// Blocks a release unless every module has consistent migration and recovery evidence.
pub fn require_release_evidence(
    env: &Env,
    release_version: &Symbol,
    modules: &Vec<ReleaseModule>,
) -> Result<(), ReleaseGateError> {
    if modules.is_empty() {
        return Err(ReleaseGateError::InvalidManifest);
    }
    for module in modules.iter() {
        check_module_evidence(env, release_version, module)?;
    }
    Ok(())
}

/// Builds the `Migration` and `RollbackRehearsal` gate checks for each module from the
/// recorded evidence, ready to append to a `ReleaseManifest` for
/// [`crate::release_readiness::evaluate_release_readiness`]. Missing or stale evidence
/// yields a `Failed` check, which blocks the release.
pub fn evidence_gate_checks(
    env: &Env,
    release_version: &Symbol,
    modules: &Vec<ReleaseModule>,
) -> Vec<GateCheck> {
    let now = env.ledger().timestamp();
    let mut checks = Vec::new(env);
    for module in modules.iter() {
        let (mig_status, mig_details) = match get_migration_evidence(env, release_version, module) {
            Some(_) => (GateCheckStatus::Passed, symbol_short!("mig_evid")),
            None => (GateCheckStatus::Failed, symbol_short!("no_mig")),
        };
        checks.push_back(create_gate_check(
            symbol_short!("mig_evid"),
            GateKind::Migration,
            module,
            mig_status,
            mig_details,
            now,
        ));

        let (rcv_status, rcv_details) = match check_module_evidence(env, release_version, module) {
            Ok(()) => (GateCheckStatus::Passed, symbol_short!("rcv_evid")),
            Err(_) if get_recovery_evidence(env, release_version, module).is_none() => {
                (GateCheckStatus::Failed, symbol_short!("no_rcv"))
            }
            Err(_) => (GateCheckStatus::Failed, symbol_short!("rcv_stale")),
        };
        checks.push_back(create_gate_check(
            symbol_short!("rcv_evid"),
            GateKind::RollbackRehearsal,
            module,
            rcv_status,
            rcv_details,
            now,
        ));
    }
    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::initialize_admin;
    use crate::release_readiness::{
        evaluate_release_readiness, rehearse_rollback, ReleaseManifest, RollbackRehearsalParams,
    };
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Env};

    #[contract]
    struct EvidenceTestContract;

    #[contractimpl]
    impl EvidenceTestContract {}

    /// Test harness. Each admin-authorized call runs in its own contract frame, because
    /// Soroban rejects a second `require_auth` for the same address within one frame.
    struct Ctx {
        env: Env,
        id: Address,
        admin: Address,
    }

    impl Ctx {
        fn new() -> Self {
            let env = Env::default();
            env.mock_all_auths();
            let id = env.register_contract(None, EvidenceTestContract);
            let admin = Address::generate(&env);
            env.as_contract(&id, || initialize_admin(&env, &admin).unwrap());
            Ctx { env, id, admin }
        }

        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.id, f)
        }

        fn journal(&self, b: u8) -> BytesN<32> {
            BytesN::from_array(&self.env, &[b; 32])
        }

        #[allow(clippy::too_many_arguments)] // passes the evidence fields straight through
        fn migrate(
            &self,
            caller: &Address,
            release: &Symbol,
            module: ReleaseModule,
            from: u32,
            to: u32,
            journal: u8,
            unmigrated: u32,
        ) -> Result<MigrationEvidence, ReleaseGateError> {
            let hash = self.journal(journal);
            self.run(|| {
                record_migration_evidence(
                    &self.env, caller, release, module, from, to, hash, unmigrated,
                )
            })
        }

        fn recover(
            &self,
            caller: &Address,
            release: &Symbol,
            module: ReleaseModule,
            rehearsal_id: u64,
        ) -> Result<RecoveryEvidence, ReleaseGateError> {
            self.run(|| record_recovery_evidence(&self.env, caller, release, module, rehearsal_id))
        }

        fn rehearse(
            &self,
            module: ReleaseModule,
            from: u32,
            to: u32,
            strategy: RollbackStrategy,
            regression: bool,
        ) -> u64 {
            let params = RollbackRehearsalParams {
                module,
                current_version: from,
                target_version: to,
                previous_wasm_hash: self.journal(9),
                strategy,
                simulate_regression: regression,
            };
            self.run(|| rehearse_rollback(&self.env, &params).unwrap().rehearsal_id)
        }

        fn modules(&self, list: &[ReleaseModule]) -> Vec<ReleaseModule> {
            let mut v = Vec::new(&self.env);
            for m in list {
                v.push_back(*m);
            }
            v
        }

        fn require(
            &self,
            release: &Symbol,
            mods: &Vec<ReleaseModule>,
        ) -> Result<(), ReleaseGateError> {
            self.run(|| require_release_evidence(&self.env, release, mods))
        }

        fn checks(&self, release: &Symbol, mods: &Vec<ReleaseModule>) -> Vec<GateCheck> {
            self.run(|| evidence_gate_checks(&self.env, release, mods))
        }

        /// Evaluates readiness with passing security and smoke checks, so the result
        /// depends only on the evidence checks.
        fn is_ready(&self, release: &Symbol, mods: &Vec<ReleaseModule>) -> (bool, u32) {
            let mut checks = self.checks(release, mods);
            for module in mods.iter() {
                for kind in [GateKind::Security, GateKind::SmokeTest] {
                    checks.push_back(create_gate_check(
                        symbol_short!("other"),
                        kind,
                        module,
                        GateCheckStatus::Passed,
                        symbol_short!("ok"),
                        0,
                    ));
                }
            }
            let manifest = ReleaseManifest {
                release_version: release.clone(),
                modules: mods.clone(),
                checks,
                signoff_admin: self.admin.clone(),
                exception_marker: None,
            };
            let report = self.run(|| evaluate_release_readiness(&self.env, &manifest).unwrap());
            (report.is_ready, report.failed_checks)
        }
    }

    #[test]
    fn test_complete_evidence_makes_release_ready() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let mods = t.modules(&[ReleaseModule::Treasury, ReleaseModule::Aid]);
        for module in mods.iter() {
            t.migrate(&t.admin, &release, module, 1, 2, 1, 0).unwrap();
            let id = t.rehearse(module, 1, 2, RollbackStrategy::Revert, true);
            let rcv = t.recover(&t.admin, &release, module, id).unwrap();
            assert_eq!(rcv.rehearsed_version, 2);
            assert_eq!(rcv.attempt, 1);
        }

        assert_eq!(t.require(&release, &mods), Ok(()));
        let checks = t.checks(&release, &mods);
        assert_eq!(checks.len(), 4);
        assert!(checks.iter().all(|c| c.status == GateCheckStatus::Passed));
        assert_eq!(t.is_ready(&release, &mods), (true, 0));
    }

    #[test]
    fn test_missing_evidence_blocks_release() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let m = ReleaseModule::Oracle;
        let mods = t.modules(&[m]);
        assert_eq!(
            t.require(&release, &mods),
            Err(ReleaseGateError::MissingMigrationVerification)
        );
        assert_eq!(t.is_ready(&release, &mods), (false, 2));

        t.migrate(&t.admin, &release, m, 3, 4, 1, 0).unwrap();
        assert_eq!(
            t.require(&release, &mods),
            Err(ReleaseGateError::MissingRollbackRehearsal)
        );
        let checks = t.checks(&release, &mods);
        assert_eq!(checks.get(0).unwrap().status, GateCheckStatus::Passed);
        assert_eq!(checks.get(1).unwrap().status, GateCheckStatus::Failed);
        assert_eq!(checks.get(1).unwrap().details, symbol_short!("no_rcv"));
        assert_eq!(t.is_ready(&release, &mods), (false, 1));
    }

    #[test]
    fn test_invalid_migration_evidence_is_rejected_and_not_stored() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let m = ReleaseModule::Referral;
        let err = Err(ReleaseGateError::MissingMigrationVerification);
        // Version does not move forward.
        assert_eq!(t.migrate(&t.admin, &release, m, 2, 2, 1, 0), err);
        // Records left unmigrated.
        assert_eq!(t.migrate(&t.admin, &release, m, 1, 2, 1, 3), err);
        // No dry-run journal.
        assert_eq!(t.migrate(&t.admin, &release, m, 1, 2, 0, 0), err);
        assert_eq!(t.run(|| get_migration_evidence(&t.env, &release, m)), None);
    }

    #[test]
    fn test_invalid_recovery_evidence_is_rejected() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let m = ReleaseModule::Governance;
        // Unknown rehearsal.
        assert_eq!(
            t.recover(&t.admin, &release, m, 99),
            Err(ReleaseGateError::MissingRollbackRehearsal)
        );
        // Rehearsal for another module.
        let other = t.rehearse(ReleaseModule::Aid, 1, 2, RollbackStrategy::Revert, true);
        assert_eq!(
            t.recover(&t.admin, &release, m, other),
            Err(ReleaseGateError::InvalidModule)
        );
        // Manual recovery fails the rehearsal under regression.
        let failed = t.rehearse(m, 1, 2, RollbackStrategy::Manual, true);
        assert_eq!(
            t.recover(&t.admin, &release, m, failed),
            Err(ReleaseGateError::RehearsalSimulationFailed)
        );
        // Manual is not accepted even when the dry run itself passed.
        let manual = t.rehearse(m, 1, 2, RollbackStrategy::Manual, false);
        assert_eq!(
            t.recover(&t.admin, &release, m, manual),
            Err(ReleaseGateError::RehearsalSimulationFailed)
        );
        assert_eq!(t.run(|| get_recovery_evidence(&t.env, &release, m)), None);
    }

    #[test]
    fn test_retry_replaces_evidence_and_flags_stale_recovery() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let m = ReleaseModule::Marketplace;
        let mods = t.modules(&[m]);
        t.migrate(&t.admin, &release, m, 1, 2, 1, 0).unwrap();
        let id = t.rehearse(m, 1, 2, RollbackStrategy::ForwardFix, true);
        t.recover(&t.admin, &release, m, id).unwrap();
        assert_eq!(t.require(&release, &mods), Ok(()));

        // A rejected retry leaves the accepted record and its attempt untouched.
        assert!(t.migrate(&t.admin, &release, m, 1, 3, 2, 5).is_err());
        let kept = t
            .run(|| get_migration_evidence(&t.env, &release, m))
            .unwrap();
        assert_eq!((kept.attempt, kept.to_version), (1, 2));

        // The plan now targets version 3, so the earlier rehearsal is stale.
        let retried = t.migrate(&t.admin, &release, m, 1, 3, 2, 0).unwrap();
        assert_eq!((retried.attempt, retried.to_version), (2, 3));
        assert_eq!(
            t.require(&release, &mods),
            Err(ReleaseGateError::MissingRollbackRehearsal)
        );
        let checks = t.checks(&release, &mods);
        assert_eq!(checks.get(1).unwrap().details, symbol_short!("rcv_stale"));

        // Re-rehearse against the new target and record again.
        let id = t.rehearse(m, 1, 3, RollbackStrategy::Revert, true);
        assert_eq!(t.recover(&t.admin, &release, m, id).unwrap().attempt, 2);
        assert_eq!(t.require(&release, &mods), Ok(()));
    }

    #[test]
    fn test_non_admin_cannot_record_evidence() {
        let t = Ctx::new();
        let release = symbol_short!("v2_0_0");
        let m = ReleaseModule::Treasury;
        let intruder = Address::generate(&t.env);
        assert_eq!(
            t.migrate(&intruder, &release, m, 1, 2, 1, 0),
            Err(ReleaseGateError::UnauthorizedSignoff)
        );
        let id = t.rehearse(m, 1, 2, RollbackStrategy::Revert, true);
        assert_eq!(
            t.recover(&intruder, &release, m, id),
            Err(ReleaseGateError::UnauthorizedSignoff)
        );
        assert_eq!(t.run(|| get_migration_evidence(&t.env, &release, m)), None);
        assert_eq!(t.run(|| get_recovery_evidence(&t.env, &release, m)), None);

        let rec = t.migrate(&t.admin, &release, m, 1, 2, 1, 0).unwrap();
        assert_eq!(rec.recorded_by, t.admin);
    }

    #[test]
    fn test_evidence_is_scoped_per_release_and_requires_modules() {
        let t = Ctx::new();
        let v1 = symbol_short!("v1_0_0");
        let v2 = symbol_short!("v2_0_0");
        let m = ReleaseModule::Aid;
        t.migrate(&t.admin, &v1, m, 1, 2, 1, 0).unwrap();
        let id = t.rehearse(m, 1, 2, RollbackStrategy::Revert, true);
        t.recover(&t.admin, &v1, m, id).unwrap();

        let mods = t.modules(&[m]);
        assert_eq!(t.require(&v1, &mods), Ok(()));
        assert_eq!(
            t.require(&v2, &mods),
            Err(ReleaseGateError::MissingMigrationVerification)
        );
        assert_eq!(
            t.require(&v1, &Vec::new(&t.env)),
            Err(ReleaseGateError::InvalidManifest)
        );
    }
}
