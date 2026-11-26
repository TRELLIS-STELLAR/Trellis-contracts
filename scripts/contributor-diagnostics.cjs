#!/usr/bin/env node
"use strict";
// Read-only, offline contributor diagnostics (#505). Never installs or signs.
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const ROOT = path.resolve(__dirname, "..");

function diagnose({ root = ROOT, run = spawnSync, read = fs.readFileSync, nodeVersion = process.versions.node, strict = false } = {}) {
  const results = [];
  const categories = { "node-version": "Environment", git: "Environment", rustc: "Toolchain", cargo: "Toolchain", rustup: "Toolchain", "wasm-target": "Toolchain", "dependency-invariants": "DependencyInvariants", "changelog-integrity": "StorageSchema", "workspace-metadata": "WorkspaceFixtures" };
  function check(component, remediation, fn) {
    try {
      const message = fn();
      results.push({ component, category: categories[component], status: "Passed", message, remediation: null });
    } catch (error) {
      // Do not echo subprocess output, paths, environment values or credentials.
      const denied = ["EACCES", "EPERM"].includes(error.code);
      results.push({ component, category: categories[component], status: "Failed", message: denied ? "Permission denied" : "Check failed", remediation });
    }
  }
  function command(executable, args) {
    const result = run(executable, args, { cwd: root, encoding: "utf8", timeout: 5000, maxBuffer: 1024 * 1024, windowsHide: true, shell: false });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error("command failed");
    return result.stdout || "";
  }
  check("node-version", "Install Node.js 18 or newer, then rerun.", () => {
    if (Number(nodeVersion.split(".")[0]) < 18) throw new Error("old node");
    return "Node.js 18+ available";
  });
  for (const executable of ["git", "rustc", "cargo", "rustup"]) {
    check(executable, `Install or repair ${executable} on PATH, then rerun. Rustup may need the pinned toolchain installed.`, () => {
      command(executable, ["--version"]);
      return `${executable} available`;
    });
  }
  check("wasm-target", "Run rustup target add wasm32-unknown-unknown for the pinned toolchain, then rerun.", () => {
    if (!command("rustup", ["target", "list", "--installed"]).split(/\r?\n/).includes("wasm32-unknown-unknown")) throw new Error("missing target");
    return "Pinned WASM target installed";
  });
  check("dependency-invariants", "Restore workspace Soroban SDK 21.x and exact Dalek dependency pins, then rerun.", () => {
    const cargo = read(path.join(root, "Cargo.toml"), "utf8");
    if (!/^soroban-sdk\s*=\s*\{\s*version\s*=\s*"21\./m.test(cargo)) throw new Error("sdk");
    for (const [name, version] of [["ed25519-dalek", "2.1.1"], ["rand_core", "0.6.4"], ["curve25519-dalek", "4.1.3"]]) {
      if (!cargo.includes(`${name} = "=${version}"`)) throw new Error("pin");
    }
    return "Workspace dependency invariants satisfied";
  });
  check("changelog-integrity", "Restore a valid changelog entries array and migration notes for breaking entries, then rerun.", () => {
    const entries = JSON.parse(read(path.join(root, "changelog", "entries.json"), "utf8"));
    if (!Array.isArray(entries) || entries.length === 0 || entries.some(e => !e || (e.impact === "breaking" && (typeof e.migration_notes !== "string" || !e.migration_notes.trim())))) throw new Error("invalid changelog");
    return "Changelog and breaking-change migration notes valid";
  });
  check("workspace-metadata", "Run cargo metadata --offline --no-deps manually; repair manifest errors or populate the local dependency cache, then rerun.", () => {
    const data = JSON.parse(command("cargo", ["metadata", "--offline", "--no-deps", "--format-version", "1"]));
    if (!Array.isArray(data.packages) || !data.packages.length) throw new Error("no packages");
    return "Offline workspace metadata resolved (not a build or test)";
  });
  const counts = { passed: results.filter(r => r.status === "Passed").length, warnings: 0, failed: results.filter(r => r.status === "Failed").length };
  return { isHealthy: counts.failed === 0, isStrict: strict, counts, results };
}

function main(args = process.argv.slice(2)) {
  if (args.some(a => !["--json", "--strict", "--self-test", "--help", "-h"].includes(a))) {
    console.error("Unknown option. Use --help. Diagnostics never supports --fix or remote authorization.");
    return 2;
  }
  if (args.includes("--help") || args.includes("-h")) {
    console.log("Usage: node scripts/contributor-diagnostics.cjs [--json] [--strict] [--self-test]\nRead-only offline checks; exit 0 healthy, 1 failed, 2 usage error. No secrets or network access required.");
    return 0;
  }
  if (args.includes("--self-test")) {
    const result = spawnSync(process.execPath, ["--test", path.join(__dirname, "test-contributor-diagnostics.cjs")], { encoding: "utf8", windowsHide: true });
    if (args.includes("--json")) console.log(JSON.stringify({ selfTest: true, isHealthy: result.status === 0 }));
    else process.stdout.write(result.stdout || "Self-test could not run\n");
    return result.status === 0 ? 0 : 1;
  }
  const report = diagnose({ strict: args.includes("--strict") });
  if (args.includes("--json")) console.log(JSON.stringify(report, null, 2));
  else for (const r of report.results) console.log(`[${r.status}] ${r.component}: ${r.message}${r.remediation ? `\n  Recovery: ${r.remediation}` : ""}`);
  return report.isHealthy ? 0 : 1;
}
module.exports = { diagnose, main };
if (require.main === module) process.exitCode = main();
