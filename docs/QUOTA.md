# Quota Management (Issue #65)

Quotas prevent abuse, runaway costs, and accidental resource exhaustion on
expensive operations (storage writes, payouts, reward distribution).

## Where enforcement lives

| Entry point | Resource key | Enforced in |
|---|---|---|
| `AidContract::create_aid` | `aid_crt` | `shared::quota::check_and_consume` (fail-open when unconfigured) |
| `TreasuryContract::withdraw` | `wdraw` | `shared::quota::check_and_consume` |
| Custom integrations | any `Symbol` | call `check_and_consume` cheap-first |

Unset config means **fail-open** (allowed) so existing deployments keep
working until maintainers set limits.

## Default limits (`QuotaConfig::default`)

| Field | Default | Meaning |
|---|---|---|
| `max_ops_per_window` | 100 | ops per actor per window |
| `window_ledgers` | 17280 (~1 day) | auto-reset interval |
| `max_storage_entries` | 1000 | advisory per-actor cap |
| `max_amount_per_op` | 0 (none) | per-op amount cap when > 0 |
| `allow_override` | true | maintainer reset supported |

## Maintainer runbook

```rust
// Set limits (admin only):
treasury.set_quota_config(&admin, &symbol_short!("wdraw"), &QuotaConfig {
    max_ops_per_window: 50, window_ledgers: 17_280,
    max_storage_entries: 500, max_amount_per_op: 10_000_000, allow_override: true,
});
// Inspect usage by actor/resource (user-safe, no internals leaked):
let status = treasury.quota_status(&actor, &symbol_short!("wdraw"));
// Override / reset after review:
treasury.reset_quota(&admin, &actor, &symbol_short!("wdraw"));
```

Over-limit calls return user-safe `Error::QuotaExceeded` with no usage
internals. Windows roll over automatically on ledger advance; `reset_quota`
is idempotent.

## Validation

```bash
cargo test -p shared quota
```
