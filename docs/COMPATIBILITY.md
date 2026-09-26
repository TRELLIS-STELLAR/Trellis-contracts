# Record & API Compatibility (Issue #64)

Version-aware handling lets older clients, migrated records, and new schema
fields coexist during rollout.

## Current version

- `CURRENT_RECORD_SCHEMA_VERSION = 2` (`shared::compat`).
- `MIN_SUPPORTED = 1`, `MAX_SUPPORTED = 2`.
- All new writes stamp V2 via `new_current_record` / `from_latest`.

## Schema history

| Version | Shape | Status |
|---:|---|---|
| 1 | `(id, donor, recipient, amount, expiry_ledger, status)` — no token binding | Deprecated but readable indefinitely |
| 2 | V1 + `token` binding + `schema_version` | Current; all new writes |

## Read / write paths

- **Read:** `to_latest()` normalizes any supported envelope. V1 entries are
  lazily migrated with `migrate_v1_to_v2(v1, &instance_token)`; contracts
  should re-persist the V2 shape when touched (see
  `AidContract::get_aid_latest` / `migrate_legacy_aid`).
- **Write:** `from_latest()` rejects any record whose `schema_version != 2`,
  so mixed-version writes cannot occur.
- **Legacy clients:** use `downgrade_v2_to_v1()` on outbound paths only; never
  persist the downgraded shape.
- **Unsupported:** versions `< 1` or `> 2` return
  `Error::UnsupportedSchemaVersion` (fail fast, user-safe).

## Deprecation & migration strategy

1. V1 reads remain supported for at least one full release after V3 ships,
   with `is_deprecated_version()` warnings in indexers.
2. V1 writes are already rejected.
3. When adding V3: bump `CURRENT_RECORD_SCHEMA_VERSION`, add a
   `migrate_v2_to_v3` link in the chain, extend tests (legacy read, new write,
   unsupported), and document here.
4. Never reuse a version number or change the meaning of a shipped field.

## Validation

```bash
cargo test -p shared compat
```
