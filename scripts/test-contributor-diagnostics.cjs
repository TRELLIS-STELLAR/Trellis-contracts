"use strict";
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { diagnose } = require("./contributor-diagnostics.cjs");
const manifest = fs.readFileSync(path.join(__dirname, "..", "Cargo.toml"), "utf8");
function healthy(overrides = {}) {
  return diagnose({
    read: file => file.endsWith("Cargo.toml") ? manifest : '[{"impact":"patch"}]',
    run: (exe, args) => ({ status: 0, stdout: args[0] === "metadata" ? '{"packages":[{}]}' : args[0] === "target" ? "wasm32-unknown-unknown\n" : "version" }),
    nodeVersion: "22.0.0",
    ...overrides,
  });
}
test("normal checks pass and preserve the JSON report contract", () => {
  const report = healthy({ strict: true });
  assert.equal(report.isHealthy, true);
  assert.equal(report.counts.passed, 9);
  assert.equal(report.isStrict, true);
});
test("missing executable, timeout and authorization failures are actionable and redacted", () => {
  for (const code of ["ENOENT", "ETIMEDOUT", "EACCES", "EPERM"]) {
    const report = healthy({ run: () => ({ error: Object.assign(new Error("SECRET credential"), { code }), status: null }) });
    assert.equal(report.isHealthy, false);
    assert.ok(report.results.filter(r => r.status === "Failed").every(r => r.remediation));
    assert.ok(!JSON.stringify(report).includes("SECRET"));
    if (code === "EACCES") assert.equal(report.results[1].message, "Permission denied");
  }
});
test("malformed and unauthorized file reads fail safely; retry starts clean", () => {
  assert.equal(healthy({ read: () => "invalid" }).isHealthy, false);
  assert.equal(healthy({ read: () => { throw Object.assign(new Error("private"), { code: "EACCES" }); } }).isHealthy, false);
  assert.equal(healthy().isHealthy, true);
});
test("checks stay offline, bounded, and never invoke a shell or mutation", () => {
  healthy({ run: (exe, args, opts) => {
    assert.equal(opts.shell, false);
    assert.equal(opts.timeout, 5000);
    assert.ok(["--version", "target", "metadata"].includes(args[0]));
    if (args[0] === "metadata") assert.ok(args.includes("--offline"));
    return { status: 0, stdout: args[0] === "metadata" ? '{"packages":[{}]}' : "wasm32-unknown-unknown\n" };
  } });
});
test("invalid pins, old runtime and missing migration notes fail", () => {
  assert.equal(healthy({ nodeVersion: "16.0.0" }).isHealthy, false);
  assert.equal(healthy({ read: file => file.endsWith("Cargo.toml") ? manifest.replace("=2.1.1", "2.1.1") : '[{"impact":"breaking"}]' }).counts.failed, 2);
});
test("CLI rejects mutation flags and can run from another directory", () => {
  const script = path.join(__dirname, "contributor-diagnostics.cjs");
  assert.equal(spawnSync(process.execPath, [script, "--fix"]).status, 2);
  const help = spawnSync(process.execPath, [script, "--help"], { cwd: require("node:os").tmpdir(), encoding: "utf8" });
  assert.equal(help.status, 0);
  assert.match(help.stdout, /Read-only offline/);
});
