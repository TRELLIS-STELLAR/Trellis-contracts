# Contributor Diagnostics (Issue #67)

One command checks setup, dependencies, configuration, and fixtures before
you start working.

## When to run

- After a fresh clone.
- After toolchain / env changes.
- Before opening a PR (after `cargo fmt` / `clippy` / `test`).
- When `cargo test` or deploys fail for unclear reasons.

## How to run

```bash
./scripts/diagnostics.sh
./scripts/diagnostics.sh --json        # machine-readable
./scripts/diagnostics.sh --self-test  # validate script wiring (CI)
```

The command is **read-only**: it never deploys, funds accounts, or mutates
production data. It probes tools (`rustc`, `cargo`, `soroban`, `git`), the
WASM target, required env vars, `validate-config.sh`, RPC reachability
(short-timeout probe), and workspace fixtures.

## Output

- `PASS <check>` per healthy check.
- `FAIL <check> :: <remediation>` per failure with an actionable next step.
- Summary `Diagnostics: N passed, M failed`; exit `0` when green, `1` otherwise.

Example failure:

```text
FAIL env:STELLAR_RPC_URL :: Set STELLAR_RPC_URL=https://... (see docs/CONFIGURATION.md)
```

When all green, the script points at `cargo test --workspace`.
