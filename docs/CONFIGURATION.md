# Configuration & Secrets (Issue #66)

Startup and deployment fail fast on missing, malformed, or unsafe config.
Secrets are never printed in full.

## Required variables

| Var | Local | Staging | Production |
|---|---|---|---|
| `TRELLIS_ENV` | `local` | `staging` | `production` |
| `STELLAR_NETWORK` | `standalone` | `testnet` | `mainnet` |
| `STELLAR_RPC_URL` | `http://localhost:8000` allowed | `https://...` | `https://...` (https enforced) |
| `TRELLIS_SECRET_KEY` | **absent** (ephemeral keys only) | `S...` 56 chars | `S...` 56 chars via secret manager |
| `TRELLIS_FEATURE_*` | optional `0/1` | optional `0/1` | optional `0/1` |

Rules (mirrored in `shared::config` and `scripts/validate-config.sh`):

- Missing -> `Error::ConfigMissing`.
- Bad URL / unknown network / bad flag -> `Error::ConfigInvalid`.
- Placeholder (`changeme`, `password`, `test`, ...) or short secrets ->
  `Error::UnsafeSecret`.
- Well-formed `S...` seed with `TRELLIS_ENV=local` -> `Error::UnsafeSecret`
  (catches accidental production-key reuse locally).
- Production RPC must be `https://`.

## Usage

```bash
# Validate before deploy / startup:
./scripts/validate-config.sh
TRELLIS_ENV=production ./scripts/validate-config.sh

# Quiet mode for scripts/CI (exit code only):
./scripts/validate-config.sh --quiet
```

Failures print an actionable `Fix:` line. Secret previews render as
`SAAA...AA` (first4...last2) via `RedactedSecret`; full values never appear
in logs. Never commit keys — load from env or a secret manager.

## Validation

```bash
cargo test -p shared config
./scripts/validate-config.sh --quiet; echo $?
```
