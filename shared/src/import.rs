//! Import Pipeline with Dry-Run Validation and Rollback Guidance (Issue #37).
//!
//! Provides a configurable, safe bulk import pipeline for Trellis smart contracts.
//! Designed to handle bulk donor allocations, aid beneficiary lists, or registry
//! entries with validation, duplicate detection, dry-run simulation, idempotency
//! guarantees, and operator rollback guidance.
//!
//! ## Execution Modes
//!
//! | Mode | Behaviour |
//! |------|----------|
//! | `AllOrNothing` | Atomic execution. All rows must validate and succeed. If any row fails, the entire batch is rejected. |
//! | `BestEffort` | Partial execution. Valid rows are committed; invalid rows are skipped and recorded with row-level error details. |
//!
//! ## Duplicate Handling Policies
//!
//! | Policy | Behaviour |
//! |------|----------|
//! | `SkipExisting` | Existing records with matching external IDs are skipped (`skip_count` increments). |
//! | `UpdateExisting` | Existing records are updated with new payload data (`update_count` increments). |
//! | `RejectDuplicate` | Duplicate external IDs (within batch or in storage) are treated as errors (`error_count` increments). |
//!
//! ## Dry-Run Guarantees
//!
//! When `dry_run` is `true`:
//! - **Zero persistent writes**: The pipeline executes validation, duplicate detection, and impact calculation purely against in-memory state and read-only storage lookups.
//! - **Full preview**: Computes accurate `create_count`, `update_count`, `skip_count`, and `error_count`.
//! - **Rollback guidance**: Analyzes validation results and recommends remediation runbooks before any ledger state is altered.
//! - Emits an `("import", "simulated")` ledger event for off-chain indexer visibility.
//!
//! ## Idempotency Guarantees
//!
//! When `external_id` is supplied on rows:
//! - Repeated executions of the same batch with `SkipExisting` result in `create_count = 0`, `update_count = 0`, and `skip_count = total_rows`.
//! - No duplicate persistent records are created on re-runs.

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Map, Symbol, Vec,
};

use crate::errors::Error;
pub use crate::migration::RollbackStrategy;
use crate::storage::{
    instance_get, instance_remove, instance_set, persistent_get, persistent_has, persistent_read,
    persistent_remove, persistent_set, temporary_has, temporary_remove, temporary_set,
};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Default maximum number of rows in a single import batch.
pub const DEFAULT_MAX_IMPORT_SIZE: u32 = 50;

/// Absolute maximum allowed rows in one import transaction (gas ceiling).
pub const ABSOLUTE_MAX_IMPORT_SIZE: u32 = 100;

/// Maximum allowable length in bytes for an external ID.
pub const MAX_EXTERNAL_ID_LEN: u32 = 64;

/// Maximum number of quarantined rows retained per batch for operator review.
pub const MAX_QUARANTINE_ROWS: u32 = 100;

pub const MAX_CANCELLATION_CLEANUP: u32 = 50;

/// Temporary storage reentrancy lock key.
const REENTRANCY_KEY: Symbol = symbol_short!("imp_lck");

/// Instance storage counter key.
const IMPORT_COUNTER_KEY: Symbol = symbol_short!("imp_cnt");

/// Instance storage key for the last committed batch fingerprint (rollback anchor).
const LAST_BATCH_KEY: Symbol = symbol_short!("imp_last");

/// Instance storage key for the last committed batch record count (rollback anchor).
const LAST_BATCH_COUNT_KEY: Symbol = symbol_short!("imp_lcnt");

/// Instance cooperative-cancellation marker for an import fingerprint.
const IMPORT_CANCEL_KEY: Symbol = symbol_short!("imp_cncl");

const IMPORT_CANCEL_RECORD_KEY: Symbol = symbol_short!("imp_cnr");

// ----------------------------------------------------------------------------
// Error Codes (Range 940–949)
// ----------------------------------------------------------------------------

/// Import pipeline error codes.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ImportError {
    /// The import batch exceeds the maximum allowed operations.
    BatchTooLarge = 940,
    /// The import batch contains no records.
    EmptyBatch = 941,
    /// The import configuration is invalid (e.g., zero or excessive max_rows).
    InvalidConfig = 942,
    /// Duplicate external identifier detected under RejectDuplicate policy.
    DuplicateExternalId = 943,
    /// One or more rows failed validation in AllOrNothing mode.
    InvalidRow = 944,
    /// Reentrancy detected during import execution.
    ReentrancyDetected = 945,
    /// Row amount is zero or negative.
    InvalidAmount = 946,
    /// Row expiration ledger/timestamp is already expired.
    ExpiredRecord = 947,
    /// External identifier exceeds the maximum permitted length.
    ExternalIdTooLong = 948,
    /// Recipient address is invalid.
    InvalidRecipient = 949,
    /// The batch was cancelled before its commit phase.
    ImportCancelled = 950,
}

/// A quarantined row captured for operator review and remediation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantinedRow {
    /// Row sequence number in input batch.
    pub row_id: u32,
    /// External ID if present on the row.
    pub external_id: Option<Bytes>,
    /// Stable numeric contract error code.
    pub error_code: u32,
    /// Short symbol reason (e.g. `zero_amt`, `expired`, `dup_key`).
    pub reason: Symbol,
}

// ----------------------------------------------------------------------------
// Pipeline Types
// ----------------------------------------------------------------------------

/// Atomicity mode for executing the import.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportMode {
    /// All rows must succeed or entire batch fails / reverts.
    AllOrNothing,
    /// Valid rows persist; invalid rows are skipped and reported.
    BestEffort,
}

/// Strategy for resolving rows whose external ID already exists.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicatePolicy {
    /// Ignore new row and leave existing record untouched.
    SkipExisting,
    /// Overwrite existing record with new values.
    UpdateExisting,
    /// Reject as an error and record in RowError.
    RejectDuplicate,
}

/// A single row in an import batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportItem {
    /// Zero-based row sequence number within the input batch.
    pub row_id: u32,
    /// Optional external client/NGO identifier for idempotency & deduplication.
    pub external_id: Option<Bytes>,
    /// Target recipient address.
    pub recipient: Address,
    /// Disbursement or allocation amount (must be > 0).
    pub amount: i128,
    /// Expiration ledger sequence or timestamp (must be > current ledger).
    pub expiry: u64,
    /// Optional metadata / payload hash (e.g. SHA-256 of off-chain metadata).
    pub metadata_hash: Option<Bytes>,
}

/// Import pipeline execution configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportConfig {
    /// When `true`, validates and calculates diffs without making any persistent writes.
    pub dry_run: bool,
    /// Atomicity mode (`AllOrNothing` vs `BestEffort`).
    pub mode: ImportMode,
    /// How to handle records with duplicate external IDs.
    pub duplicate_policy: DuplicatePolicy,
    /// Batch size ceiling for this invocation (cannot exceed `ABSOLUTE_MAX_IMPORT_SIZE`).
    pub max_rows: u32,
}

/// Detailed error information for an invalid row.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowError {
    /// Row sequence number in input batch.
    pub row_id: u32,
    /// External ID if present on the row.
    pub external_id: Option<Bytes>,
    /// Stable numeric contract error code.
    pub error_code: u32,
    /// Short symbol reason (e.g. `zero_amt`, `expired`, `dup_key`).
    pub reason: Symbol,
}

/// Prescriptive rollback and remediation guidance returned in the import report.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackGuidance {
    /// High-level rollback strategy (`Revert`, `ForwardFix`, `Manual`).
    pub strategy: RollbackStrategy,
    /// Short action code for operators (e.g. `no_action`, `fix_rows`, `reverted`, `part_fix`).
    pub action: Symbol,
    /// Number of records needing attention or remediation.
    pub affected_records: u32,
    /// Runbook reference code for the operational team.
    pub runbook_code: Symbol,
}

/// Comprehensive outcome report for both dry-run previews and committed imports.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportReport {
    /// `true` if this report was generated during a dry-run preview.
    pub is_dry_run: bool,
    /// Total number of rows evaluated.
    pub total_rows: u32,
    /// Number of records created.
    pub create_count: u32,
    /// Number of records updated.
    pub update_count: u32,
    /// Number of records skipped.
    pub skip_count: u32,
    /// Number of rows that encountered validation or duplicate errors.
    pub error_count: u32,
    /// Detailed row-level error entries.
    pub errors: Vec<RowError>,
    /// Rollback and remediation guidance.
    pub rollback_guidance: RollbackGuidance,
    /// Rows quarantined for operator review (unsafe rows rejected from commit).
    pub quarantined: Vec<QuarantinedRow>,
    /// Deterministic 32-byte cryptographic fingerprint of the import batch.
    pub batch_fingerprint: BytesN<32>,
}

/// Stored record format for persistent state tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredImportRecord {
    /// Monotonically-assigned internal record identifier.
    pub record_id: u64,
    /// External identifier if one was provided.
    pub external_id: Bytes,
    /// Target recipient address.
    pub recipient: Address,
    /// Allocation amount.
    pub amount: i128,
    /// Expiration timestamp or ledge sequence.
    pub expiry: u64,
    /// Optional metadata hash.
    pub metadata_hash: Option<Bytes>,
    /// Ledger timestamp when the record was imported.
    pub imported_at: u64,
    /// Record schema version.
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportCancellationRecord {
    pub fingerprint: BytesN<32>,
    pub cancelled_by: Address,
    pub reason: Symbol,
    pub cancelled_at_ledger: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportCancellationCleanupReport {
    pub evaluated_ledger: u64,
    pub eligible: Vec<BytesN<32>>,
    pub removed: Vec<BytesN<32>>,
    pub retained: Vec<BytesN<32>>,
    pub not_found: Vec<BytesN<32>>,
    pub destructive: bool,
}

// ----------------------------------------------------------------------------
// Storage Key Helpers
// ----------------------------------------------------------------------------

fn external_record_key(external_id: &Bytes) -> (Symbol, Bytes) {
    (symbol_short!("imp_ext"), external_id.clone())
}

/// Storage key for a batch-level traceability marker.
fn batch_marker_key(fingerprint: &BytesN<32>) -> (Symbol, BytesN<32>) {
    (symbol_short!("imp_bat"), fingerprint.clone())
}

fn cancellation_record_key(fingerprint: &BytesN<32>) -> (Symbol, BytesN<32>) {
    (IMPORT_CANCEL_RECORD_KEY, fingerprint.clone())
}

// ----------------------------------------------------------------------------
// Reentrancy Guard
// ----------------------------------------------------------------------------

fn acquire_reentrancy_guard(env: &Env) -> Result<(), ImportError> {
    if temporary_has(env, &REENTRANCY_KEY) {
        return Err(ImportError::ReentrancyDetected);
    }
    temporary_set(env, &REENTRANCY_KEY, &true);
    Ok(())
}

fn release_reentrancy_guard(env: &Env) {
    temporary_remove(env, &REENTRANCY_KEY);
}

// ----------------------------------------------------------------------------
// Fingerprint & Validation
// ----------------------------------------------------------------------------

/// Computes a deterministic SHA-256 fingerprint of the input batch.
pub fn compute_batch_fingerprint(env: &Env, items: &Vec<ImportItem>) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    let len = items.len();
    payload.append(&Bytes::from_slice(env, b"trellis_import_v1:"));

    for i in 0..len {
        let item = items.get(i).unwrap();
        // Pack row_id, amount, expiry
        let mut row_bytes = Bytes::new(env);
        row_bytes.append(&Bytes::from_slice(env, &item.row_id.to_be_bytes()));
        row_bytes.append(&Bytes::from_slice(env, &item.amount.to_be_bytes()));
        row_bytes.append(&Bytes::from_slice(env, &item.expiry.to_be_bytes()));
        if let Some(ext) = &item.external_id {
            row_bytes.append(ext);
        }
        payload.append(&row_bytes);
    }
    env.crypto().sha256(&payload).into()
}

/// Validates configuration bounds.
pub fn validate_config(config: &ImportConfig) -> Result<(), ImportError> {
    if config.max_rows == 0 || config.max_rows > ABSOLUTE_MAX_IMPORT_SIZE {
        return Err(ImportError::InvalidConfig);
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Row Validation
// ----------------------------------------------------------------------------

/// Validates the fields of a single row against the pipeline constraints.
///
/// On failure the stable numeric error code and a short reason symbol are
/// returned so the caller can surface them in [`RowError`] and
/// [`QuarantinedRow`] entries.
fn validate_item(_env: &Env, item: &ImportItem, current_time: u64) -> Result<(), (u32, Symbol)> {
    if item.amount <= 0 {
        return Err((ImportError::InvalidAmount as u32, symbol_short!("zero_amt")));
    }
    if item.expiry <= current_time {
        return Err((ImportError::ExpiredRecord as u32, symbol_short!("expired")));
    }
    if let Some(ext_id) = &item.external_id {
        if ext_id.is_empty() || ext_id.len() > MAX_EXTERNAL_ID_LEN {
            return Err((
                ImportError::ExternalIdTooLong as u32,
                symbol_short!("bad_id"),
            ));
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Rollback Guidance
// ----------------------------------------------------------------------------

/// Computes the operator-facing rollback and remediation guidance.
///
/// | Situation | `strategy` | `action` | Runbook |
/// |-----------|------------|----------|---------|
/// | No row failed | `Manual` | `no_action` | `clean` |
/// | Dry-run preview with failures | `ForwardFix` | `fix_rows` | `dry_err` |
/// | Atomic batch aborted | `Revert` | `reverted` | `atom_rev` |
/// | Best-effort batch with failures | `ForwardFix` | `part_fix` | `part_rem` |
pub fn generate_rollback_guidance(
    is_dry_run: bool,
    mode: ImportMode,
    total_rows: u32,
    error_count: u32,
    _create_count: u32,
    _update_count: u32,
) -> RollbackGuidance {
    if error_count == 0 {
        return RollbackGuidance {
            strategy: RollbackStrategy::Manual,
            action: symbol_short!("no_action"),
            affected_records: 0,
            runbook_code: symbol_short!("clean"),
        };
    }

    if is_dry_run {
        return RollbackGuidance {
            strategy: RollbackStrategy::ForwardFix,
            action: symbol_short!("fix_rows"),
            affected_records: error_count,
            runbook_code: symbol_short!("dry_err"),
        };
    }

    match mode {
        ImportMode::AllOrNothing => RollbackGuidance {
            strategy: RollbackStrategy::Revert,
            action: symbol_short!("reverted"),
            affected_records: total_rows,
            runbook_code: symbol_short!("atom_rev"),
        },
        ImportMode::BestEffort => RollbackGuidance {
            strategy: RollbackStrategy::ForwardFix,
            action: symbol_short!("part_fix"),
            affected_records: error_count,
            runbook_code: symbol_short!("part_rem"),
        },
    }
}

// ----------------------------------------------------------------------------
// Pipeline Internals
// ----------------------------------------------------------------------------

/// Aggregated outcome of a single pass over an import batch.
struct BatchOutcome {
    /// Number of rows that create a new record.
    create_count: u32,
    /// Number of rows that update an existing record.
    update_count: u32,
    /// Number of rows skipped by the duplicate policy.
    skip_count: u32,
    /// Number of rows that failed validation or duplicate detection.
    error_count: u32,
    /// Row-level error diagnostics.
    errors: Vec<RowError>,
    /// Rows quarantined for operator review.
    quarantined: Vec<QuarantinedRow>,
}

/// Records a row failure in both the error list and the quarantine list.
///
/// The quarantine list is capped at [`MAX_QUARANTINE_ROWS`] so a large batch
/// cannot bloat the report beyond the operator review budget.
fn record_row_failure(
    _env: &Env,
    outcome: &mut BatchOutcome,
    item: &ImportItem,
    error_code: u32,
    reason: Symbol,
) {
    outcome.error_count += 1;
    outcome.errors.push_back(RowError {
        row_id: item.row_id,
        external_id: item.external_id.clone(),
        error_code,
        reason: reason.clone(),
    });
    if outcome.quarantined.len() < MAX_QUARANTINE_ROWS {
        outcome.quarantined.push_back(QuarantinedRow {
            row_id: item.row_id,
            external_id: item.external_id.clone(),
            error_code,
            reason,
        });
    }
}

/// Runs one pass over the batch, optionally persisting the accepted rows.
///
/// When `commit` is `false` the pass is a pure preview: it only reads
/// persistent storage (never writing, never bumping a TTL) and accumulates the
/// in-memory duplicate map. When `commit` is `true` the accepted rows are
/// written to persistent storage and the import counter is advanced.
fn run_pipeline(
    env: &Env,
    items: &Vec<ImportItem>,
    config: &ImportConfig,
    commit: bool,
) -> BatchOutcome {
    let current_time = env.ledger().timestamp();

    let mut outcome = BatchOutcome {
        create_count: 0,
        update_count: 0,
        skip_count: 0,
        error_count: 0,
        errors: Vec::new(env),
        quarantined: Vec::new(env),
    };

    // In-memory set for tracking duplicates within this batch.
    // Maps external_id -> occurrence count.
    let mut seen_in_batch: Map<Bytes, u32> = Map::new(env);
    let mut counter: u64 = instance_get(env, &IMPORT_COUNTER_KEY).unwrap_or(0);

    for item in items.iter() {
        // 1. Validate row fields.
        if let Err((code, reason)) = validate_item(env, &item, current_time) {
            record_row_failure(env, &mut outcome, &item, code, reason);
            continue;
        }

        // 2. Duplicate detection and persistence.
        let ext_id = match &item.external_id {
            Some(ext_id) => ext_id,
            // Rows without an external ID are anonymous creates.
            None => {
                outcome.create_count += 1;
                if commit {
                    counter += 1;
                }
                continue;
            }
        };

        let key = external_record_key(ext_id);
        let in_batch_count = seen_in_batch.get(ext_id.clone()).unwrap_or(0);
        let existing: Option<StoredImportRecord> = persistent_read(env, &key);

        if in_batch_count > 0 || existing.is_some() {
            match config.duplicate_policy {
                DuplicatePolicy::SkipExisting => {
                    outcome.skip_count += 1;
                }
                DuplicatePolicy::UpdateExisting => {
                    outcome.update_count += 1;
                    if commit {
                        let record_id = match &existing {
                            Some(record) => record.record_id,
                            None => {
                                counter += 1;
                                counter
                            }
                        };
                        let record = StoredImportRecord {
                            record_id,
                            external_id: ext_id.clone(),
                            recipient: item.recipient.clone(),
                            amount: item.amount,
                            expiry: item.expiry,
                            metadata_hash: item.metadata_hash.clone(),
                            imported_at: current_time,
                            version: 1,
                        };
                        persistent_set(env, &key, &record);
                    }
                }
                DuplicatePolicy::RejectDuplicate => {
                    let reason = if in_batch_count > 0 {
                        symbol_short!("dup_batch")
                    } else {
                        symbol_short!("dup_store")
                    };
                    record_row_failure(
                        env,
                        &mut outcome,
                        &item,
                        ImportError::DuplicateExternalId as u32,
                        reason,
                    );
                }
            }
        } else {
            outcome.create_count += 1;
            if commit {
                counter += 1;
                let record = StoredImportRecord {
                    record_id: counter,
                    external_id: ext_id.clone(),
                    recipient: item.recipient.clone(),
                    amount: item.amount,
                    expiry: item.expiry,
                    metadata_hash: item.metadata_hash.clone(),
                    imported_at: current_time,
                    version: 1,
                };
                persistent_set(env, &key, &record);
            }
        }

        seen_in_batch.set(ext_id.clone(), in_batch_count + 1);
    }

    if commit {
        instance_set(env, &IMPORT_COUNTER_KEY, &counter);
    }

    outcome
}

// ----------------------------------------------------------------------------
// Dry Run Simulation
// ----------------------------------------------------------------------------

/// Performs a dry-run validation of the import batch.
///
/// **Safety guarantee**: performs **ZERO persistent writes**. All duplicate
/// checking and preview calculations use read-only lookups and in-memory
/// tracking. The import counter and every stored record are left untouched.
///
/// Rows that fail validation — or that are rejected as duplicates under
/// [`DuplicatePolicy::RejectDuplicate`] — are reported in
/// [`ImportReport::errors`] and quarantined in
/// [`ImportReport::quarantined`] for operator review.
pub fn dry_run(
    env: &Env,
    items: &Vec<ImportItem>,
    config: &ImportConfig,
) -> Result<ImportReport, ImportError> {
    validate_config(config)?;
    let total_rows = items.len();
    if total_rows == 0 {
        return Err(ImportError::EmptyBatch);
    }
    if total_rows > config.max_rows {
        return Err(ImportError::BatchTooLarge);
    }

    let fingerprint = compute_batch_fingerprint(env, items);
    let outcome = run_pipeline(env, items, config, false);

    let rollback_guidance = generate_rollback_guidance(
        true,
        config.mode,
        total_rows,
        outcome.error_count,
        outcome.create_count,
        outcome.update_count,
    );

    crate::events::emit_import_simulated(
        env,
        total_rows,
        outcome.create_count,
        outcome.update_count,
        outcome.skip_count,
        outcome.error_count,
        &fingerprint,
    );

    Ok(ImportReport {
        is_dry_run: true,
        total_rows,
        create_count: outcome.create_count,
        update_count: outcome.update_count,
        skip_count: outcome.skip_count,
        error_count: outcome.error_count,
        errors: outcome.errors,
        rollback_guidance,
        quarantined: outcome.quarantined,
        batch_fingerprint: fingerprint,
    })
}

// ----------------------------------------------------------------------------
// Execute Import
// ----------------------------------------------------------------------------

/// Executes an import batch against persistent storage with full guardrails.
///
/// * [`ImportMode::AllOrNothing`] validates the whole batch before touching
///   persistent storage; a single invalid row aborts the batch with
///   [`ImportError::InvalidRow`] and nothing is written.
/// * [`ImportMode::BestEffort`] commits the valid rows and reports the
///   rejected ones in [`ImportReport::errors`] /
///   [`ImportReport::quarantined`].
/// * A reentrant invocation is rejected with [`ImportError::ReentrancyDetected`].
///
/// The returned [`ImportReport::batch_fingerprint`] identifies the committed
/// batch; it is also stored as the rollback anchor for the operator runbook.
pub fn execute_import(
    env: &Env,
    caller: &Address,
    items: &Vec<ImportItem>,
    config: &ImportConfig,
) -> Result<ImportReport, ImportError> {
    // A dry-run configuration never writes; route it to the preview path.
    if config.dry_run {
        return dry_run(env, items, config);
    }

    validate_config(config)?;
    let total_rows = items.len();
    if total_rows == 0 {
        return Err(ImportError::EmptyBatch);
    }
    if total_rows > config.max_rows {
        return Err(ImportError::BatchTooLarge);
    }

    acquire_reentrancy_guard(env)?;

    let fingerprint = compute_batch_fingerprint(env, items);

    if is_import_cancelled(env, &fingerprint) {
        release_reentrancy_guard(env);
        return Err(ImportError::ImportCancelled);
    }

    // Atomic mode: validate the whole batch before any persistent write so a
    // single invalid row leaves the ledger state untouched.
    if config.mode == ImportMode::AllOrNothing {
        let preview = run_pipeline(env, items, config, false);
        if preview.error_count > 0 {
            release_reentrancy_guard(env);
            crate::events::emit_import_failed(
                env,
                caller,
                total_rows,
                preview.error_count,
                ImportError::InvalidRow as u32,
            );
            return Err(ImportError::InvalidRow);
        }
    }

    let outcome = run_pipeline(env, items, config, true);

    // Rollback anchor: remember the batch that was just committed so the
    // operator runbook can locate the affected rows later.
    let committed_records = outcome.create_count + outcome.update_count;
    let marker_key = batch_marker_key(&fingerprint);
    if !persistent_has(env, &marker_key) {
        persistent_set(env, &marker_key, &committed_records);
    }
    instance_set(env, &LAST_BATCH_KEY, &fingerprint);
    instance_set(env, &LAST_BATCH_COUNT_KEY, &committed_records);

    release_reentrancy_guard(env);

    let rollback_guidance = generate_rollback_guidance(
        false,
        config.mode,
        total_rows,
        outcome.error_count,
        outcome.create_count,
        outcome.update_count,
    );

    crate::events::emit_import_committed(
        env,
        caller,
        total_rows,
        outcome.create_count,
        outcome.update_count,
        outcome.skip_count,
        outcome.error_count,
        &fingerprint,
    );

    Ok(ImportReport {
        is_dry_run: false,
        total_rows,
        create_count: outcome.create_count,
        update_count: outcome.update_count,
        skip_count: outcome.skip_count,
        error_count: outcome.error_count,
        errors: outcome.errors,
        rollback_guidance,
        quarantined: outcome.quarantined,
        batch_fingerprint: fingerprint,
    })
}

fn record_cancellation(
    env: &Env,
    caller: &Address,
    fingerprint: &BytesN<32>,
    reason: Symbol,
) -> ImportCancellationRecord {
    if let Some(existing) = persistent_get(env, &cancellation_record_key(fingerprint)) {
        if !is_import_cancelled(env, fingerprint) {
            instance_set(env, &(IMPORT_CANCEL_KEY, fingerprint.clone()), &true);
        }
        return existing;
    }

    let cancelled_at_ledger = env.ledger().sequence() as u64;
    instance_set(env, &(IMPORT_CANCEL_KEY, fingerprint.clone()), &true);

    let record = ImportCancellationRecord {
        fingerprint: fingerprint.clone(),
        cancelled_by: caller.clone(),
        reason,
        cancelled_at_ledger,
    };
    persistent_set(env, &cancellation_record_key(fingerprint), &record);

    crate::events::emit_import_cancelled(
        env,
        caller,
        fingerprint,
        &record.reason,
        cancelled_at_ledger,
    );
    record
}

/// Request cancellation of a pending import before it reaches its commit
/// phase. The marker is keyed by the deterministic batch fingerprint, so a
/// retry of the same batch observes the same decision and cannot partially
/// apply after an operator has stopped it.
pub fn cancel_import(
    env: &Env,
    caller: &Address,
    fingerprint: &BytesN<32>,
) -> Result<(), ImportError> {
    caller.require_auth();
    record_cancellation(env, caller, fingerprint, symbol_short!("legacy"));
    Ok(())
}

pub fn cancel_import_with_reason(
    env: &Env,
    caller: &Address,
    fingerprint: &BytesN<32>,
    reason: Symbol,
) -> Result<ImportCancellationRecord, Error> {
    crate::auth::require_admin(env, caller)?;
    Ok(record_cancellation(env, caller, fingerprint, reason))
}

/// Returns whether an operator has cancelled this import fingerprint.
pub fn is_import_cancelled(env: &Env, fingerprint: &BytesN<32>) -> bool {
    instance_get(env, &(IMPORT_CANCEL_KEY, fingerprint.clone())).unwrap_or(false)
}

pub fn get_import_cancellation(
    env: &Env,
    fingerprint: &BytesN<32>,
) -> Option<ImportCancellationRecord> {
    persistent_get(env, &cancellation_record_key(fingerprint))
}

pub fn cleanup_import_cancellations(
    env: &Env,
    caller: &Address,
    fingerprints: &Vec<BytesN<32>>,
    older_than_ledger: u32,
    confirm: bool,
) -> Result<ImportCancellationCleanupReport, Error> {
    crate::auth::require_admin(env, caller)?;
    if fingerprints.len() > MAX_CANCELLATION_CLEANUP {
        return Err(Error::InvalidArgument);
    }

    let evaluated_ledger = env.ledger().sequence() as u64;
    let minimum_age = older_than_ledger as u64;
    let mut eligible = Vec::new(env);
    let mut removed = Vec::new(env);
    let mut retained = Vec::new(env);
    let mut not_found = Vec::new(env);

    for index in 0..fingerprints.len() {
        let Some(fingerprint) = fingerprints.get(index) else {
            continue;
        };
        let marker_key = (IMPORT_CANCEL_KEY, fingerprint.clone());
        let marker: bool = instance_get(env, &marker_key).unwrap_or(false);
        let record: Option<ImportCancellationRecord> =
            persistent_get(env, &cancellation_record_key(&fingerprint));

        if !marker && record.is_none() {
            not_found.push_back(fingerprint);
            continue;
        }

        let is_old_enough = match &record {
            Some(record) => {
                evaluated_ledger >= record.cancelled_at_ledger.saturating_add(minimum_age)
            }
            None => minimum_age == 0,
        };
        if !is_old_enough {
            retained.push_back(fingerprint);
            continue;
        }

        eligible.push_back(fingerprint.clone());
        if confirm {
            instance_remove(env, &marker_key);
            persistent_remove(env, &cancellation_record_key(&fingerprint));
            removed.push_back(fingerprint);
        }
    }

    let destructive = confirm && !removed.is_empty();
    let report = ImportCancellationCleanupReport {
        evaluated_ledger,
        eligible,
        removed,
        retained,
        not_found,
        destructive,
    };
    crate::events::emit_import_cancellation_cleaned(
        env,
        caller,
        evaluated_ledger,
        older_than_ledger,
        (
            report.eligible.len(),
            report.removed.len(),
            report.retained.len(),
            report.not_found.len(),
        ),
        confirm,
    );
    Ok(report)
}

// ----------------------------------------------------------------------------
// Query Helpers
// ----------------------------------------------------------------------------

/// Fetches a stored import record by its external identifier.
pub fn get_imported_record(env: &Env, external_id: &Bytes) -> Option<StoredImportRecord> {
    persistent_get(env, &external_record_key(external_id))
}

/// Checks whether an external identifier has already been imported.
pub fn has_imported_record(env: &Env, external_id: &Bytes) -> bool {
    persistent_has(env, &external_record_key(external_id))
}

/// Returns the number of import records committed so far.
pub fn get_import_counter(env: &Env) -> u64 {
    instance_get(env, &IMPORT_COUNTER_KEY).unwrap_or(0)
}

#[cfg(test)]
mod cancellation_tests {
    use super::{
        cancel_import, cancel_import_with_reason, cleanup_import_cancellations,
        compute_batch_fingerprint, execute_import, get_import_cancellation, get_import_counter,
        get_imported_record, has_imported_record, is_import_cancelled, DuplicatePolicy, Error,
        ImportConfig, ImportError, ImportItem, ImportMode, MAX_CANCELLATION_CLEANUP,
    };
    use crate::auth::set_admin;
    use crate::events::{IMPORT_CANCELLED, IMPORT_CANCEL_CLEANED};
    use soroban_sdk::{
        contract, contractimpl, symbol_short,
        testutils::{Address as _, Events as _, Ledger as _},
        Address, Bytes, BytesN, Env, IntoVal, Vec,
    };

    #[contract]
    struct DummyImportContract;

    #[contractimpl]
    impl DummyImportContract {
        pub fn noop(_env: Env) {}
    }

    struct Ctx {
        env: Env,
        contract_id: Address,
        admin: Address,
        alice: Address,
    }

    impl Ctx {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract_id, f)
        }
    }

    fn setup() -> Ctx {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        env.ledger().set_timestamp(1_000);
        env.ledger().set_sequence_number(100);

        let contract_id = env.register_contract(None, DummyImportContract);
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        env.as_contract(&contract_id, || set_admin(&env, &admin));

        Ctx {
            env,
            contract_id,
            admin,
            alice,
        }
    }

    fn batch(ctx: &Ctx, external_id: &[u8]) -> Vec<ImportItem> {
        let mut items = Vec::new(&ctx.env);
        items.push_back(ImportItem {
            row_id: 0,
            external_id: Some(Bytes::from_slice(&ctx.env, external_id)),
            recipient: ctx.alice.clone(),
            amount: 100,
            expiry: 10_000,
            metadata_hash: None,
        });
        items
    }

    fn config() -> ImportConfig {
        ImportConfig {
            dry_run: false,
            mode: ImportMode::AllOrNothing,
            duplicate_policy: DuplicatePolicy::SkipExisting,
            max_rows: 10,
        }
    }

    fn fingerprints(ctx: &Ctx, values: &[BytesN<32>]) -> Vec<BytesN<32>> {
        let mut result = Vec::new(&ctx.env);
        for value in values {
            result.push_back(value.clone());
        }
        result
    }

    #[test]
    fn cancellation_blocks_retry_until_bounded_admin_cleanup() {
        let ctx = setup();
        let items = batch(&ctx, b"CANCEL-RETRY");
        let fingerprint = ctx.run(|| compute_batch_fingerprint(&ctx.env, &items));

        let record = ctx.run(|| {
            cancel_import_with_reason(
                &ctx.env,
                &ctx.admin,
                &fingerprint,
                symbol_short!("incident"),
            )
            .unwrap()
        });
        assert_eq!(record.cancelled_by, ctx.admin);
        assert_eq!(record.reason, symbol_short!("incident"));
        assert_eq!(record.cancelled_at_ledger, 100);
        assert!(ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));
        assert_eq!(
            ctx.run(|| get_import_cancellation(&ctx.env, &fingerprint)),
            Some(record)
        );

        let (_, topics, _) = ctx.env.events().all().last().unwrap();
        assert_eq!(
            topics,
            (symbol_short!("import"), IMPORT_CANCELLED).into_val(&ctx.env)
        );

        assert_eq!(
            ctx.run(|| execute_import(&ctx.env, &ctx.admin, &items, &config())),
            Err(ImportError::ImportCancelled)
        );
        assert_eq!(ctx.run(|| get_import_counter(&ctx.env)), 0);

        let candidates = fingerprints(&ctx, &[fingerprint.clone()]);

        ctx.env.ledger().set_sequence_number(105);
        let retained = ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 10, false).unwrap()
        });
        assert_eq!(retained.eligible.len(), 0);
        assert_eq!(retained.retained.len(), 1);
        assert!(!retained.destructive);
        assert!(ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));

        ctx.env.ledger().set_sequence_number(110);
        let preview = ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 10, false).unwrap()
        });
        assert_eq!(preview.eligible, candidates);
        assert_eq!(preview.removed.len(), 0);
        assert!(!preview.destructive);
        assert!(ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));

        let cleaned = ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 10, true).unwrap()
        });
        assert_eq!(cleaned.removed, candidates);
        assert!(cleaned.destructive);
        assert!(!ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));
        assert_eq!(
            ctx.run(|| get_import_cancellation(&ctx.env, &fingerprint)),
            None
        );

        let (_, topics, _) = ctx.env.events().all().last().unwrap();
        assert_eq!(
            topics,
            (symbol_short!("import"), IMPORT_CANCEL_CLEANED).into_val(&ctx.env)
        );

        let report = ctx.run(|| execute_import(&ctx.env, &ctx.admin, &items, &config()).unwrap());
        assert_eq!(report.create_count, 1);
        assert_eq!(ctx.run(|| get_import_counter(&ctx.env)), 1);
    }

    #[test]
    fn privileged_cancellation_paths_reject_non_admin() {
        let ctx = setup();
        ctx.run(|| {
            let fingerprint = BytesN::from_array(&ctx.env, &[7; 32]);
            assert_eq!(
                cancel_import_with_reason(
                    &ctx.env,
                    &ctx.alice,
                    &fingerprint,
                    symbol_short!("incident"),
                ),
                Err(Error::Unauthorized)
            );
            assert!(!is_import_cancelled(&ctx.env, &fingerprint));
            assert_eq!(get_import_cancellation(&ctx.env, &fingerprint), None);

            let candidates = fingerprints(&ctx, &[fingerprint.clone()]);
            assert_eq!(
                cleanup_import_cancellations(&ctx.env, &ctx.alice, &candidates, 0, true),
                Err(Error::Unauthorized)
            );
            assert!(!is_import_cancelled(&ctx.env, &fingerprint));
            assert_eq!(get_import_cancellation(&ctx.env, &fingerprint), None);
        });
    }

    #[test]
    fn legacy_cancellation_remains_authenticated_and_idempotent() {
        let ctx = setup();
        let items = batch(&ctx, b"LEGACY-CANCEL");
        let fingerprint = ctx.run(|| compute_batch_fingerprint(&ctx.env, &items));

        ctx.run(|| cancel_import(&ctx.env, &ctx.alice, &fingerprint).unwrap());
        let legacy = ctx.run(|| get_import_cancellation(&ctx.env, &fingerprint).unwrap());
        assert_eq!(legacy.cancelled_by, ctx.alice);
        assert_eq!(legacy.reason, symbol_short!("legacy"));

        let repeated = ctx.run(|| {
            cancel_import_with_reason(
                &ctx.env,
                &ctx.admin,
                &fingerprint,
                symbol_short!("incident"),
            )
            .unwrap()
        });
        assert_eq!(repeated, legacy);
        assert_eq!(
            ctx.run(|| execute_import(&ctx.env, &ctx.admin, &items, &config())),
            Err(ImportError::ImportCancelled)
        );

        let candidates = fingerprints(&ctx, &[fingerprint.clone()]);
        let cleaned = ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 0, true).unwrap()
        });
        assert_eq!(cleaned.removed, candidates);
        assert!(!ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));
        let first = ctx.run(|| execute_import(&ctx.env, &ctx.admin, &items, &config()).unwrap());
        let second = ctx.run(|| execute_import(&ctx.env, &ctx.admin, &items, &config()).unwrap());
        assert_eq!(first.create_count, 1);
        assert_eq!(second.skip_count, 1);
    }

    #[test]
    fn cleanup_classifies_missing_and_too_new_entries_and_is_bounded() {
        let ctx = setup();
        let items = batch(&ctx, b"RETAIN-CANCEL");
        let fingerprint = ctx.run(|| compute_batch_fingerprint(&ctx.env, &items));
        let missing = BytesN::from_array(&ctx.env, &[9; 32]);
        ctx.run(|| {
            cancel_import_with_reason(
                &ctx.env,
                &ctx.admin,
                &fingerprint,
                symbol_short!("operator"),
            )
            .unwrap()
        });

        ctx.env.ledger().set_sequence_number(105);
        let candidates = fingerprints(&ctx, &[fingerprint.clone(), missing.clone()]);
        let report = ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 10, true).unwrap()
        });
        assert_eq!(report.eligible.len(), 0);
        assert_eq!(report.removed.len(), 0);
        assert_eq!(report.retained, fingerprints(&ctx, &[fingerprint.clone()]));
        assert_eq!(report.not_found, fingerprints(&ctx, &[missing]));
        assert!(!report.destructive);
        assert!(ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));

        let mut oversized = fingerprints(&ctx, &[fingerprint.clone()]);
        for value in 0..MAX_CANCELLATION_CLEANUP {
            oversized.push_back(BytesN::from_array(&ctx.env, &[value as u8; 32]));
        }
        assert_eq!(
            ctx.run(|| cleanup_import_cancellations(&ctx.env, &ctx.admin, &oversized, 0, true)),
            Err(Error::InvalidArgument)
        );
        assert!(ctx.run(|| is_import_cancelled(&ctx.env, &fingerprint)));
    }

    #[test]
    fn cleanup_preserves_committed_import_records() {
        let ctx = setup();
        let cancelled_items = batch(&ctx, b"CANCELLED-BATCH");
        let cancelled_fingerprint =
            ctx.run(|| compute_batch_fingerprint(&ctx.env, &cancelled_items));
        ctx.run(|| {
            cancel_import_with_reason(
                &ctx.env,
                &ctx.admin,
                &cancelled_fingerprint,
                symbol_short!("incident"),
            )
            .unwrap()
        });

        let committed_items = batch(&ctx, b"COMMITTED-BATCH");
        let committed_fingerprint =
            ctx.run(|| compute_batch_fingerprint(&ctx.env, &committed_items));
        ctx.run(|| execute_import(&ctx.env, &ctx.admin, &committed_items, &config()).unwrap());

        let candidates = fingerprints(&ctx, &[cancelled_fingerprint.clone()]);
        ctx.run(|| {
            cleanup_import_cancellations(&ctx.env, &ctx.admin, &candidates, 0, true).unwrap()
        });

        assert!(!ctx.run(|| is_import_cancelled(&ctx.env, &cancelled_fingerprint)));
        assert!(ctx.run(|| has_imported_record(
            &ctx.env,
            &Bytes::from_slice(&ctx.env, b"COMMITTED-BATCH")
        )));
        assert_eq!(
            ctx.run(|| get_imported_record(
                &ctx.env,
                &Bytes::from_slice(&ctx.env, b"COMMITTED-BATCH")
            )
            .unwrap()
            .amount),
            100
        );
        assert_eq!(ctx.run(|| get_import_counter(&ctx.env)), 1);
        assert_eq!(
            committed_fingerprint,
            ctx.run(|| compute_batch_fingerprint(&ctx.env, &committed_items))
        );
    }
}
