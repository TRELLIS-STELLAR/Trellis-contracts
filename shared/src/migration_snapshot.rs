//! Deterministic, read-only upgrade snapshots for migration dry-runs.
//!
//! Callers must provide entries in the contract's canonical storage-key order.
//! The XDR encoding of that ordered vector is the checksum input, so a changed
//! key, value, order, or entry count produces a different digest.

use soroban_sdk::{contracttype, Bytes, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageSnapshotEntry {
    pub key: Bytes,
    pub value: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationStorageSnapshot {
    pub entries: Vec<StorageSnapshotEntry>,
    pub checksum: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageDiffReport {
    pub checksum_matches: bool,
    pub added: u32,
    pub removed: u32,
    pub changed: u32,
}

pub fn snapshot_checksum(env: &Env, entries: &Vec<StorageSnapshotEntry>) -> BytesN<32> {
    env.crypto().sha256(&entries.to_xdr(env)).into()
}

pub fn build_snapshot(env: &Env, entries: Vec<StorageSnapshotEntry>) -> MigrationStorageSnapshot {
    let checksum = snapshot_checksum(env, &entries);
    MigrationStorageSnapshot { entries, checksum }
}

pub fn verify_snapshot(env: &Env, snapshot: &MigrationStorageSnapshot) -> bool {
    snapshot_checksum(env, &snapshot.entries) == snapshot.checksum
}

pub fn diff_snapshots(
    env: &Env,
    before: &MigrationStorageSnapshot,
    after: &MigrationStorageSnapshot,
) -> StorageDiffReport {
    let mut added = 0;
    let mut removed = 0;
    let mut changed = 0;
    let common = if before.entries.len() < after.entries.len() {
        before.entries.len()
    } else {
        after.entries.len()
    };
    for index in 0..common {
        let old = before.entries.get(index).unwrap();
        let new = after.entries.get(index).unwrap();
        if old.key != new.key {
            removed = removed.saturating_add(1);
            added = added.saturating_add(1);
        } else if old.value != new.value {
            changed = changed.saturating_add(1);
        }
    }
    if before.entries.len() > common {
        removed = removed.saturating_add(before.entries.len() - common);
    }
    if after.entries.len() > common {
        added = added.saturating_add(after.entries.len() - common);
    }
    StorageDiffReport {
        checksum_matches: verify_snapshot(env, before) && verify_snapshot(env, after),
        added,
        removed,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(env: &Env, key: &[u8], value: &[u8]) -> StorageSnapshotEntry {
        StorageSnapshotEntry {
            key: Bytes::from_slice(env, key),
            value: Bytes::from_slice(env, value),
        }
    }

    #[test]
    fn checksum_detects_value_changes_and_valid_snapshot_retries() {
        let env = Env::default();
        let mut before_entries = Vec::new(&env);
        before_entries.push_back(entry(&env, b"a", b"1"));
        before_entries.push_back(entry(&env, b"b", b"2"));
        let before = build_snapshot(&env, before_entries);
        assert!(verify_snapshot(&env, &before));

        let mut after_entries = before.entries.clone();
        after_entries.set(1, entry(&env, b"b", b"3"));
        let after = build_snapshot(&env, after_entries);
        let diff = diff_snapshots(&env, &before, &after);
        assert!(diff.checksum_matches);
        assert_eq!(diff.changed, 1);
    }

    #[test]
    fn rejects_tampered_checksum_and_reports_added_removed_entries() {
        let env = Env::default();
        let mut old_entries = Vec::new(&env);
        old_entries.push_back(entry(&env, b"a", b"1"));
        old_entries.push_back(entry(&env, b"b", b"2"));
        let old = build_snapshot(&env, old_entries);
        let mut new_entries = Vec::new(&env);
        new_entries.push_back(entry(&env, b"a", b"1"));
        new_entries.push_back(entry(&env, b"c", b"3"));
        new_entries.push_back(entry(&env, b"d", b"4"));
        let mut new = build_snapshot(&env, new_entries);
        new.checksum = old.checksum.clone();
        let diff = diff_snapshots(&env, &old, &new);
        assert!(!diff.checksum_matches);
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 1);
    }
}
