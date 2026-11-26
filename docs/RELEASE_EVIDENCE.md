# Release gate for migration and recovery evidence (#507)

`--check-all` remains a repository prerequisite check, not deployment approval.
Release promotion must use the stricter `--manifest` mode. Existing unsigned
manifest producers must migrate to the signed format below; on-chain contracts,
storage, events and the existing PR checklist mode remain unchanged.

Configure `RELEASE_EVIDENCE_PUBLIC_KEY` as a trusted Ed25519 SPKI PEM public key
outside the manifest (a repository Actions variable for CI). Keep the private
approval key with the release authority. Set `RELEASE_COMMIT_SHA` to the exact
40-character lowercase commit to release. Missing/invalid trust configuration,
wrong signer, or evidence for another commit blocks promotion. Do not accept
the expected SHA or public key from an untrusted manifest. Key ownership and
rotation are operational responsibilities; a signature attests that the release
authority reviewed the evidence, not that this script independently ran it.

## Manifest and executable use

For every unique target module, declare exactly one Passed check for each of
`migrations`, `security`, `smoke_tests`, and `rollback_rehearsal`. Unknown gates,
duplicates, failed checks and bypass statuses are rejected. Each check references
a nonempty evidence file under the repository root with its lowercase SHA-256.
Files larger than 16 MiB, missing files, digest mismatches and paths/symlinks
escaping the root are rejected. Store immutable, sanitized reports; never include
signing keys or donor data in evidence committed to the repository.

The unsigned payload has this shape (repeat the checks for each target module):

```json
{
  "release_version": "v2.0.0",
  "commit_sha": "<exact target commit SHA>",
  "modules": ["treasury"],
  "checks": [
    {
      "module": "treasury", "kind": "migrations", "status": "Passed",
      "from_version": 1, "to_version": 2,
      "dry_run": true, "unmigrated_records": 0,
      "evidence": { "path": "release-evidence/migration.json", "sha256": "<64 hex>" }
    },
    {
      "module": "treasury", "kind": "security", "status": "Passed",
      "evidence": { "path": "release-evidence/security.txt", "sha256": "<64 hex>" }
    },
    {
      "module": "treasury", "kind": "smoke_tests", "status": "Passed",
      "evidence": { "path": "release-evidence/smoke.txt", "sha256": "<64 hex>" }
    },
    {
      "module": "treasury", "kind": "rollback_rehearsal", "status": "Passed",
      "strategy": "ForwardFix", "rehearsal_passed": true,
      "snapshot": { "path": "release-evidence/snapshot.json", "sha256": "<64 hex>" },
      "evidence": { "path": "release-evidence/recovery.json", "sha256": "<64 hex>" }
    }
  ]
}
```

Migration dry runs must advance nonnegative integer schema versions with no
unmigrated records. Recovery requires a snapshot and a passed rehearsal using
Revert or ForwardFix; Manual alone cannot establish recovery readiness. Use a
ForwardFix rehearsal when schema changes cannot safely be downgraded.

Sign the UTF-8 bytes of `JSON.stringify(payload)` with Ed25519, then append the
base64 result as a top-level `signature` property. Verification removes that
property and serializes the remaining object in the same property order. JSON
whitespace may change; property order may not. Do not embed the manifest into
the commit whose SHA it signs: generate it after the evidence commit is final
and supply it externally to the gate to avoid a self-referential commit hash.

```sh
node scripts/release-readiness-gate.cjs --manifest release-manifest.json --json
node --test scripts/test-release-evidence.cjs
```

The Release Readiness workflow's manual dispatch accepts signed manifest JSON,
checks artifacts from the selected ref and binds verification to that commit.
Deployment pipelines should require its successful `release-evidence` job before
promotion; this repository's workflow does not deploy automatically. PR checks
remain prerequisite checks and cannot substitute for signed release approval.

## Failures and recovery

Exit 1 blocks promotion. Correct the failed migration or recovery rehearsal,
regenerate reports and hashes, then obtain a new approval signature and rerun.
Restoring an accidentally missing artifact with the original bytes permits a
retry without changing approval. Changes to the payload or release commit always
need a new signature. Emergency PR exception markers cannot waive manifest
verification, even when both modes run together. The gate itself never migrates,
deploys, elevates privileges or modifies evidence; a failed gate needs no rollback.
After a deployed failure, follow the approved snapshot/rehearsal plan and existing
runbook. Reverting the gate would restore unsigned acceptance and must not be
used as a release bypass.
