"use strict";
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const GATES = ["migrations", "security", "smoke_tests", "rollback_rehearsal"];

// Approval is an Ed25519 signature from an externally configured trusted key.
// A manifest cannot nominate its own approver or waive a failed gate.
function evaluateEvidence({ manifest, root, publicKey, expectedCommit }) {
  const issues = [];
  try {
    if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) throw new Error("Manifest must be an object");
    const { signature, ...payload } = manifest;
    if (!publicKey || !expectedCommit || !/^[a-f0-9]{40}$/.test(expectedCommit)) throw new Error("Trusted approval key and expected release commit are required");
    const key = crypto.createPublicKey(publicKey);
    if (key.asymmetricKeyType !== "ed25519" || typeof signature !== "string" || !/^[A-Za-z0-9+/]{86}==$/.test(signature)
        || !crypto.verify(null, Buffer.from(JSON.stringify(payload)), key, Buffer.from(signature, "base64"))) throw new Error("Unauthorized or altered release evidence");
    if (payload.commit_sha !== expectedCommit) throw new Error("Evidence belongs to another release commit");
    if (typeof payload.release_version !== "string" || !payload.release_version.trim()) throw new Error("Release version is required");
    if (!Array.isArray(payload.modules) || !payload.modules.length || payload.modules.some(m => typeof m !== "string" || !/^[a-z][a-z0-9_-]*$/.test(m))
        || new Set(payload.modules).size !== payload.modules.length) throw new Error("Unique target modules are required");
    if (!Array.isArray(payload.checks)) throw new Error("Gate checks are required");
    const base = fs.realpathSync(root);
    function artifact(ref) {
      if (!ref || typeof ref.path !== "string" || path.isAbsolute(ref.path) || !/^[a-f0-9]{64}$/.test(ref.sha256)) throw new Error("Artifact path and SHA-256 are required");
      const file = fs.realpathSync(path.resolve(base, ref.path));
      const relative = path.relative(base, file);
      if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) throw new Error("Artifact escapes evidence root");
      const stat = fs.statSync(file);
      if (!stat.isFile() || stat.size === 0 || stat.size > 16 * 1024 * 1024) throw new Error("Artifact must be a nonempty file no larger than 16 MiB");
      if (crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex") !== ref.sha256) throw new Error("Artifact digest mismatch");
    }
    const seen = new Set();
    for (const check of payload.checks) {
      if (!check || !payload.modules.includes(check.module) || !GATES.includes(check.kind)) throw new Error("Unknown gate or target module");
      const id = `${check.module}:${check.kind}`;
      if (seen.has(id)) throw new Error("Duplicate gate evidence");
      seen.add(id);
      if (check.status !== "Passed") throw new Error(`${id} did not pass; exceptions cannot bypass evidence`);
      artifact(check.evidence);
      if (check.kind === "migrations") {
        if (!Number.isSafeInteger(check.from_version) || check.from_version < 0 || !Number.isSafeInteger(check.to_version)
            || check.to_version <= check.from_version || check.unmigrated_records !== 0 || check.dry_run !== true) throw new Error("Migration dry-run must advance schema with zero unmigrated records");
      }
      if (check.kind === "rollback_rehearsal") {
        if (!["Revert", "ForwardFix"].includes(check.strategy) || check.rehearsal_passed !== true) throw new Error("Rehearsed Revert or ForwardFix recovery is required");
        artifact(check.snapshot);
      }
    }
    for (const module of payload.modules) {
      for (const kind of GATES) if (!seen.has(`${module}:${kind}`)) throw new Error(`Missing ${module}:${kind} evidence`);
    }
  } catch (error) {
    // Avoid leaking arbitrary filesystem errors or key material in CI logs.
    issues.push(error.code ? "Evidence file or approval key could not be read" : error.message);
  }
  return { passed: issues.length === 0, issues };
}
module.exports = { evaluateEvidence };
