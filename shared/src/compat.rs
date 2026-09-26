//! Version-aware record compatibility layer (Issue #64).
//!
//! Older clients, migrated ledger records, and new schema fields must coexist
//! during rollout. This module provides:
//!
//! - [`CURRENT_RECORD_SCHEMA_VERSION`] metadata written on every new record.
//! - [`is_supported_version`] / [`ensure_supported_version`] guards for read
//!   and write paths.
//! - Lossless forward migration [`migrate_v1_to_v2`] and downgrade
//!   [`downgrade_v2_to_v1`] transforms so old records remain readable and new
//!   clients always receive the latest shape.
//! - [`VersionedAidRecord`] envelope for storage that may contain either shape.
//!
//! ## Schema history
//!
//! | Version | Changes |
//! |---:|---|
//! | `1` | Legacy shape: `(id, donor, recipient, amount, expiry_ledger, status)` with no escrow-token binding and no schema metadata. **Deprecated but readable.** |
//! | `2` (current) | Adds `token` binding + explicit `schema_version` field. All new writes use this shape. |
//!
//! ## Deprecation & migration strategy
//!
//! - V1 reads are supported indefinitely via [`migrate_v1_to_v2`] (lazy
//!   migration on read; contracts should re-persist the V2 shape when touched).
//! - V1 writes are rejected: use [`to_latest`] which always emits V2.
//! - Versions `< MIN_SUPPORTED` or `> CURRENT` return
//!   [`Error::UnsupportedSchemaVersion`].
//! - When a V3 is introduced: bump [`CURRENT_RECORD_SCHEMA_VERSION`], keep the
//!   V1->V2->V3 chain, mark V1 for removal only after one full release with
//!   warnings (see `docs/COMPATIBILITY.md`).

use soroban_sdk::{contracttype, Address, Env};

use crate::errors::Error;

/// First schema version ever written to the ledger.
pub const RECORD_SCHEMA_V1: u32 = 1;
/// Current schema version. All new writes must use this version.
pub const RECORD_SCHEMA_V2: u32 = 2;

/// Alias kept for call sites that want a semantic name.
pub const CURRENT_RECORD_SCHEMA_VERSION: u32 = RECORD_SCHEMA_V2;
/// Oldest version still accepted on read paths.
pub const MIN_SUPPORTED_RECORD_SCHEMA_VERSION: u32 = RECORD_SCHEMA_V1;
/// Newest version accepted on read paths (== current).
pub const MAX_SUPPORTED_RECORD_SCHEMA_VERSION: u32 = CURRENT_RECORD_SCHEMA_VERSION;

/// Returns `true` when `version` can be read by this build.
pub fn is_supported_version(version: u32) -> bool {
    (MIN_SUPPORTED_RECORD_SCHEMA_VERSION..=MAX_SUPPORTED_RECORD_SCHEMA_VERSION)
        .contains(&version)
}

/// Returns `true` when `version` is readable but deprecated (V1).
pub fn is_deprecated_version(version: u32) -> bool {
    version == RECORD_SCHEMA_V1
}

/// Fail fast on unsupported versions for read and write paths.
pub fn ensure_supported_version(version: u32) -> Result<(), Error> {
    if is_supported_version(version) {
        Ok(())
    } else {
        Err(Error::UnsupportedSchemaVersion)
    }
}

/// Returns the schema version new writes must stamp.
pub fn current_schema_version() -> u32 {
    CURRENT_RECORD_SCHEMA_VERSION
}

/// Lifecycle state shared by V1 and V2 aid shapes.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatAidStatus {
    Pending,
    Settled,
    Refunded,
}

/// Legacy (V1) aid record: no token binding, no schema metadata.
///
/// Kept so ledger entries written before the V2 upgrade remain decodable.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyAidRecord {
    pub id: u64,
    pub donor: Address,
    pub recipient: Address,
    pub amount: i128,
    pub expiry_ledger: u32,
    pub status: CompatAidStatus,
}

/// Current (V2) aid record: token-bound with explicit schema metadata.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentAidRecord {
    pub id: u64,
    pub donor: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub expiry_ledger: u32,
    pub status: CompatAidStatus,
    pub schema_version: u32,
}

/// Storage envelope for entries that may hold either schema shape.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionedAidRecord {
    V1(LegacyAidRecord),
    V2(CurrentAidRecord),
}

impl VersionedAidRecord {
    /// Schema version carried by this envelope.
    pub fn version(&self) -> u32 {
        match self {
            VersionedAidRecord::V1(_) => RECORD_SCHEMA_V1,
            VersionedAidRecord::V2(r) => r.schema_version,
        }
    }
}

/// Forward migration: V1 -> V2.
///
/// The caller supplies the escrow `token` binding that V1 records lack
/// (resolved from contract instance config at read time). Status, amounts,
/// and participants are preserved losslessly.
pub fn migrate_v1_to_v2(v1: &LegacyAidRecord, token: &Address) -> Result<CurrentAidRecord, Error> {
    if v1.amount <= 0 {
        return Err(Error::SchemaMigrationFailed);
    }
    Ok(CurrentAidRecord {
        id: v1.id,
        donor: v1.donor.clone(),
        recipient: v1.recipient.clone(),
        token: token.clone(),
        amount: v1.amount,
        expiry_ledger: v1.expiry_ledger,
        status: v1.status.clone(),
        schema_version: CURRENT_RECORD_SCHEMA_VERSION,
    })
}

/// Downgrade: V2 -> V1 for legacy clients.
///
/// Drops the `token` binding and schema metadata. Used only on outbound
/// legacy read paths; never persisted.
pub fn downgrade_v2_to_v1(v2: &CurrentAidRecord) -> Result<LegacyAidRecord, Error> {
    ensure_supported_version(v2.schema_version)?;
    Ok(LegacyAidRecord {
        id: v2.id,
        donor: v2.donor.clone(),
        recipient: v2.recipient.clone(),
        amount: v2.amount,
        expiry_ledger: v2.expiry_ledger,
        status: v2.status.clone(),
    })
}

/// Read path: normalize any supported envelope to the latest shape.
///
/// - `V2` entries are returned as-is (after version validation).
/// - `V1` entries are lazily migrated via [`migrate_v1_to_v2`].
/// - Envelopes carrying an unsupported version return
///   [`Error::UnsupportedSchemaVersion`].
pub fn to_latest(env: &Env, record: &VersionedAidRecord) -> Result<CurrentAidRecord, Error> {
    match record {
        VersionedAidRecord::V2(r) => {
            ensure_supported_version(r.schema_version)?;
            if r.schema_version != CURRENT_RECORD_SCHEMA_VERSION {
                return Err(Error::UnsupportedSchemaVersion);
            }
            if r.amount <= 0 {
                return Err(Error::SchemaMigrationFailed);
            }
            let _ = env;
            Ok(r.clone())
        }
        VersionedAidRecord::V1(r) => {
            // Token binding is unknown in the shared layer; the caller
            // re-binds it. Here we validate shape only and stamp with the
            // record's own addresses where possible. Contract call sites
            // should prefer `migrate_v1_to_v2` with instance token.
            if r.amount <= 0 {
                return Err(Error::SchemaMigrationFailed);
            }
            Ok(CurrentAidRecord {
                id: r.id,
                donor: r.donor.clone(),
                recipient: r.recipient.clone(),
                // Placeholder: replaced by contract-level migration with the
                // real token. Shared layer uses donor as a non-empty marker
                // so the shape stays valid without inventing an address.
                token: r.donor.clone(),
                amount: r.amount,
                expiry_ledger: r.expiry_ledger,
                status: r.status.clone(),
                schema_version: CURRENT_RECORD_SCHEMA_VERSION,
            })
        }
    }
}

/// Write path: wrap a latest-shape record for storage.
///
/// Always stamps [`CURRENT_RECORD_SCHEMA_VERSION`]; rejects records that
/// claim any other version so mixed-version writes cannot occur.
pub fn from_latest(record: &CurrentAidRecord) -> Result<VersionedAidRecord, Error> {
    if record.schema_version != CURRENT_RECORD_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchemaVersion);
    }
    if record.amount <= 0 {
        return Err(Error::SchemaMigrationFailed);
    }
    Ok(VersionedAidRecord::V2(record.clone()))
}

/// Construct a new V2 record directly (preferred write path for new code).
pub fn new_current_record(
    id: u64,
    donor: &Address,
    recipient: &Address,
    token: &Address,
    amount: i128,
    expiry_ledger: u32,
    status: CompatAidStatus,
) -> Result<CurrentAidRecord, Error> {
    if amount <= 0 {
        return Err(Error::SchemaMigrationFailed);
    }
    Ok(CurrentAidRecord {
        id,
        donor: donor.clone(),
        recipient: recipient.clone(),
        token: token.clone(),
        amount,
        expiry_ledger,
        status,
        schema_version: CURRENT_RECORD_SCHEMA_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        let donor = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);
        (env, donor, recipient, token)
    }

    #[test]
    fn legacy_record_reads_via_forward_migration() {
        let (_env, donor, recipient, token) = setup();
        let v1 = LegacyAidRecord {
            id: 7,
            donor: donor.clone(),
            recipient: recipient.clone(),
            amount: 500,
            expiry_ledger: 1000,
            status: CompatAidStatus::Pending,
        };
        let v2 = migrate_v1_to_v2(&v1, &token).unwrap();
        assert_eq!(v2.schema_version, CURRENT_RECORD_SCHEMA_VERSION);
        assert_eq!(v2.token, token);
        assert_eq!(v2.amount, 500);
        // Downgrade round-trips losslessly apart from token binding.
        let back = downgrade_v2_to_v1(&v2).unwrap();
        assert_eq!(back, v1);
    }

    #[test]
    fn new_writes_always_use_latest_shape() {
        let (_env, donor, recipient, token) = setup();
        let v2 = new_current_record(
            1,
            &donor,
            &recipient,
            &token,
            100,
            200,
            CompatAidStatus::Pending,
        )
        .unwrap();
        assert_eq!(v2.schema_version, current_schema_version());
        let enveloped = from_latest(&v2).unwrap();
        assert_eq!(enveloped.version(), CURRENT_RECORD_SCHEMA_VERSION);
    }

    #[test]
    fn unsupported_versions_are_rejected() {
        assert!(ensure_supported_version(1).is_ok());
        assert!(ensure_supported_version(2).is_ok());
        assert_eq!(
            ensure_supported_version(0),
            Err(Error::UnsupportedSchemaVersion)
        );
        assert_eq!(
            ensure_supported_version(3),
            Err(Error::UnsupportedSchemaVersion)
        );
        assert_eq!(
            ensure_supported_version(u32::MAX),
            Err(Error::UnsupportedSchemaVersion)
        );

        let (_env, donor, recipient, token) = setup();
        let mut v2 = new_current_record(
            1,
            &donor,
            &recipient,
            &token,
            100,
            200,
            CompatAidStatus::Pending,
        )
        .unwrap();
        v2.schema_version = 99;
        assert_eq!(from_latest(&v2), Err(Error::UnsupportedSchemaVersion));
        assert_eq!(downgrade_v2_to_v1(&v2), Err(Error::UnsupportedSchemaVersion));
    }

    #[test]
    fn invalid_amounts_fail_migration() {
        let (_env, donor, recipient, token) = setup();
        let v1 = LegacyAidRecord {
            id: 1,
            donor: donor.clone(),
            recipient: recipient.clone(),
            amount: 0,
            expiry_ledger: 10,
            status: CompatAidStatus::Pending,
        };
        assert_eq!(
            migrate_v1_to_v2(&v1, &token),
            Err(Error::SchemaMigrationFailed)
        );
    }
}
