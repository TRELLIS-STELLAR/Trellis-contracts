# Import cancellation and cleanup

Import batches expose a deterministic fingerprint before commit. Operators can
call `shared::cancel_import` with that fingerprint to place a durable,
authenticated cancellation marker. `execute_import` checks the marker before
writing any records and returns `ImportCancelled` when cancellation was
requested.

Cancellation is cooperative: a request racing with a commit that has already
started cannot roll back writes. For a safe retry, recompute the fingerprint
and run a dry-run first; the same cancelled fingerprint remains blocked until
an administrator removes the marker with `cleanup_import_cancellations`.

## Cancellation APIs

| API | Authorization | Effect |
|---|---|---|
| `cancel_import(caller, fingerprint)` | Any authenticated caller | Compatibility path. Writes the marker and a `legacy`-reason audit record. |
| `cancel_import_with_reason(caller, fingerprint, reason)` | Shared admin | Preferred path. Writes the marker and a queryable `ImportCancellationRecord`. |
| `is_import_cancelled(fingerprint)` | Read-only | Reports whether the marker is present. |
| `get_import_cancellation(fingerprint)` | Read-only | Returns the audit record, if one exists. |
| `cleanup_import_cancellations(caller, fingerprints, older_than_ledger, confirm)` | Shared admin | Removes expired markers and records, or previews the removals. |

Every cancellation emits `("import", "cancelled")` with
`(cancelled_by, batch_fingerprint, reason, cancelled_at_ledger)`. Every cleanup
pass emits `("import", "cleaned")` with the evaluated ledger, age cutoff, and
eligible/removed/retained/not-found counts.

Repeating a cancellation for the same fingerprint is idempotent: the original
record and ledger are retained, and the marker is restored if a previous
cleanup removed it. The legacy entry point keeps its original signature and
authenticated-caller behavior; it does not require the admin role.

## Cleanup behavior

A fingerprint is eligible when its audit record satisfies
`evaluated_ledger >= cancelled_at_ledger + older_than_ledger`. The boundary is
inclusive, and the addition saturates. Marker-only entries created before this
feature have no age information, so they are eligible only when
`older_than_ledger` is zero; calling `cancel_import_with_reason` again adds an
audit record to such an entry without changing its original marker.

Each call evaluates at most `MAX_CANCELLATION_CLEANUP` (50) fingerprints and
returns `InvalidArgument` when that bound is exceeded. No storage is changed
when the call fails authorization or validation.

With `confirm == false`, the call is a dry run: `eligible` lists what would be
removed, `removed` is empty, and `destructive` is false. With `confirm == true`,
both the marker and audit record are removed for each eligible fingerprint,
`removed` lists them, and `destructive` is true if anything was removed.
Fingerprints that are too new appear in `retained`; fingerprints with neither a
marker nor a record appear in `not_found`. Duplicate input fingerprints are
evaluated independently, so a duplicate becomes `not_found` after the first
confirmed removal.

Cleanup only touches cancellation state. Committed import records, the import
counter, rollback anchors, and quarantined-row reports are never modified.

## Failure, retry, and recovery paths

- Normal: cancel, observe `ImportCancelled` from `execute_import`, wait out the
  retention window, preview cleanup, confirm cleanup, then retry the same
  fingerprint or submit a corrected batch.
- Failure: cancellation is checked after batch-shape validation and before the
  atomic preview or any persistent write. Existing invalid-row, reentrancy,
  and batch-size failures retain their original error codes and leave no
  partial import state. A failed cleanup call leaves the marker and record
  intact.
- Retry: cancellation requests and dry-run cleanup are idempotent. A confirmed
  cleanup is a one-way removal of cancellation state; repeat the call and treat
  `not_found` as success when verifying recovery.
- Authorization: non-admins cannot use the reason-aware cancellation or
  cleanup APIs. The compatibility `cancel_import` path remains available to any
  authenticated caller. `require_admin` requires the shared admin to be
  initialized.
- Race: if a commit has already started, cancellation cannot roll it back.
  Recover with a forward fix and a new fingerprint instead of attempting to
  remove committed records.

## Compatibility and migration

No existing function signature, error code, event topic, payload, or stored
record shape changed. The new audit record uses the additive storage key
`imp_cnr`; existing `imp_cncl` markers remain valid and are readable by the old
`is_import_cancelled` helper. No state migration is required. Indexers that
want to audit cancellations should begin consuming the two new event topics;
existing import event consumers are unaffected.
