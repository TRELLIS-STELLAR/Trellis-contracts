"use strict";
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const { evaluateEvidence } = require("./release-evidence.cjs");

function fixture(t, base = os.tmpdir()) {
  const root = fs.mkdtempSync(path.join(base, "trellis-release-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(root, "journal.txt"), "verified dry-run and recovery output\n");
  const evidence = { path: "journal.txt", sha256: crypto.createHash("sha256").update(fs.readFileSync(path.join(root, "journal.txt"))).digest("hex") };
  const keys = crypto.generateKeyPairSync("ed25519");
  const payload = { release_version: "v2", commit_sha: "a".repeat(40), modules: ["treasury"], checks: [
    { module: "treasury", kind: "migrations", status: "Passed", evidence, from_version: 1, to_version: 2, dry_run: true, unmigrated_records: 0 },
    { module: "treasury", kind: "security", status: "Passed", evidence },
    { module: "treasury", kind: "smoke_tests", status: "Passed", evidence },
    { module: "treasury", kind: "rollback_rehearsal", status: "Passed", evidence, snapshot: evidence, strategy: "ForwardFix", rehearsal_passed: true },
  ] };
  const publicKey = keys.publicKey.export({ type: "spki", format: "pem" });
  function sign(data = payload, privateKey = keys.privateKey) {
    return { ...data, signature: crypto.sign(null, Buffer.from(JSON.stringify(data)), privateKey).toString("base64") };
  }
  const evaluate = (manifest = sign(), options = {}) => evaluateEvidence({ manifest, root, publicKey, expectedCommit: payload.commit_sha, ...options });
  return { root, payload, sign, evaluate, publicKey };
}

test("complete authorized migration and recovery evidence passes", t => {
  assert.equal(fixture(t).evaluate().passed, true);
});
test("production CLI accepts a signed manifest with verified repository artifacts", t => {
  const repo = path.resolve(__dirname, "..");
  const f = fixture(t, repo);
  const payload = { ...f.payload, checks: f.payload.checks.map(check => ({ ...check,
    evidence: { ...check.evidence, path: path.relative(repo, path.join(f.root, check.evidence.path)) },
    ...(check.snapshot ? { snapshot: { ...check.snapshot, path: path.relative(repo, path.join(f.root, check.snapshot.path)) } } : {}),
  })) };
  const file = path.join(f.root, "manifest.json");
  fs.writeFileSync(file, JSON.stringify(f.sign(payload)));
  const result = spawnSync(process.execPath, [path.join(__dirname, "release-readiness-gate.cjs"), "--manifest", file, "--json"], {
    encoding: "utf8", env: { ...process.env, RELEASE_EVIDENCE_PUBLIC_KEY: f.publicKey, RELEASE_COMMIT_SHA: payload.commit_sha },
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).ready, true);
});
test("missing, duplicate, failed or bypassed gates cannot promote", t => {
  const f = fixture(t);
  for (const mutate of [p => p.checks.pop(), p => p.checks.push(p.checks[0]), p => { p.checks[0].status = "Failed"; }, p => { p.checks[0].status = "BypassedWithException"; }, p => { p.modules = []; }]) {
    const p = structuredClone(f.payload);
    mutate(p);
    p.exception_marker = "emergency";
    assert.equal(f.evaluate(f.sign(p)).passed, false);
  }
});
test("migration and recovery edge cases fail closed", t => {
  const f = fixture(t);
  for (const [index, field, value] of [[0, "to_version", 1], [0, "unmigrated_records", 1], [0, "dry_run", false], [3, "strategy", "Manual"], [3, "rehearsal_passed", false], [3, "snapshot", null]]) {
    const p = structuredClone(f.payload);
    p.checks[index][field] = value;
    assert.equal(f.evaluate(f.sign(p)).passed, false);
  }
});
test("unknown signer, tampering, stale commit and missing trust configuration fail", t => {
  const f = fixture(t);
  assert.equal(f.evaluate(f.sign(f.payload, crypto.generateKeyPairSync("ed25519").privateKey)).passed, false);
  const altered = f.sign();
  altered.release_version = "tampered";
  assert.equal(f.evaluate(altered).passed, false);
  assert.equal(f.evaluate(f.sign(), { expectedCommit: "b".repeat(40) }).passed, false);
  assert.equal(f.evaluate(f.sign(), { publicKey: undefined }).passed, false);
});
test("corrupt and missing artifacts block; restoring evidence allows retry", t => {
  const f = fixture(t);
  const file = path.join(f.root, "journal.txt");
  const original = fs.readFileSync(file);
  fs.writeFileSync(file, "corrupted");
  assert.equal(f.evaluate().passed, false);
  fs.unlinkSync(file);
  assert.equal(f.evaluate().passed, false);
  fs.writeFileSync(file, original);
  assert.equal(f.evaluate().passed, true);
});
test("path traversal and symlink escapes cannot supply evidence", t => {
  const f = fixture(t);
  const p = structuredClone(f.payload);
  p.checks[0].evidence.path = "../outside.txt";
  assert.equal(f.evaluate(f.sign(p)).passed, false);
  // Junctions are available without symlink privileges on Windows.
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), "trellis-outside-"));
  t.after(() => fs.rmSync(outside, { recursive: true, force: true }));
  fs.writeFileSync(path.join(outside, "journal.txt"), "verified dry-run and recovery output\n");
  fs.symlinkSync(outside, path.join(f.root, "escape"), process.platform === "win32" ? "junction" : "dir");
  p.checks[0].evidence.path = "escape/journal.txt";
  assert.equal(f.evaluate(f.sign(p)).passed, false);
});
test("production CLI returns failure for unsigned manifest even with emergency PR exception", t => {
  const f = fixture(t);
  const manifest = path.join(f.root, "manifest.json");
  const body = path.join(f.root, "body.md");
  fs.writeFileSync(manifest, JSON.stringify({ ...f.payload, exception_marker: "emergency" }));
  fs.writeFileSync(body, "<!-- release-readiness-exception: emergency -->");
  const result = spawnSync(process.execPath, [path.join(__dirname, "release-readiness-gate.cjs"), "--manifest", manifest, "--body-file", body, "--json"], { encoding: "utf8" });
  assert.equal(result.status, 1);
  assert.equal(JSON.parse(result.stdout).ready, false);
});
test("CLI fails on misspelled options and missing input rather than running prerequisite checks", () => {
  for (const args of [["--manfest", "anything"], ["--manifest"], ["--manifest", "--json"], ["--module", "unknown"]]) {
    const result = spawnSync(process.execPath, [path.join(__dirname, "release-readiness-gate.cjs"), ...args]);
    assert.equal(result.status, 2);
  }
});
