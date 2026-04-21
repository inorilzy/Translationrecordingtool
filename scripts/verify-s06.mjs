#!/usr/bin/env node
/**
 * verify-s06.mjs — 仓库级最终验证门禁
 *
 * 按固定顺序串行执行自动化检查，首个失败阶段立即终止。
 * 所有阶段名在 stdout 明确打印，退出码 = 首个失败的子命令退出码。
 *
 * 阶段顺序：
 *   1. Frontend tests  (npm test)
 *   2. Rust tests      (cargo test)
 *   3. Rust check      (cargo check)
 *   4. Frontend build  (npm run build)
 */

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");

/**
 * Run a single verification stage.
 * Returns { passed, exitCode, durationMs }.
 */
function runStage(name, cmd, args, options = {}) {
  console.log(`\n${"═".repeat(60)}`);
  console.log(`  ▶ Stage: ${name}`);
  console.log(`${"═".repeat(60)}`);
  console.log(`  Command: ${cmd} ${args.join(" ")}`);
  console.log(`${"─".repeat(60)}`);

  const start = Date.now();
  const result = spawnSync(cmd, args, {
    cwd: options.cwd ?? rootDir,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, ...(options.env ?? {}) },
    timeout: options.timeout ?? 600_000, // 10 min default
  });
  const durationMs = Date.now() - start;

  if (result.error) {
    console.log(`\n  ❌ ${name} FAILED — could not spawn: ${result.error.message}`);
    return { passed: false, exitCode: 1, durationMs };
  }

  if (result.status === 0) {
    console.log(`\n  ✅ ${name} passed (${(durationMs / 1000).toFixed(1)}s)`);
    return { passed: true, exitCode: 0, durationMs };
  }

  console.log(`\n  ❌ ${name} FAILED (${(durationMs / 1000).toFixed(1)}s)`);
  return { passed: false, exitCode: result.status ?? 1, durationMs };
}

// ── Stage definitions ──────────────────────────────────────────────

const stages = [
  {
    name: "Frontend Unit Tests",
    cmd: "npm",
    args: ["run", "test"],
  },
  {
    name: "Rust Unit Tests",
    cmd: "cargo",
    args: ["test", "--manifest-path", "src-tauri/Cargo.toml"],
    timeout: 900_000, // 15 min for Rust tests
  },
  {
    name: "Rust Compiler Check",
    cmd: "cargo",
    args: ["check", "--manifest-path", "src-tauri/Cargo.toml"],
    timeout: 600_000,
  },
  {
    name: "Frontend Build",
    cmd: "npm",
    args: ["run", "build"],
    timeout: 300_000,
  },
];

// ── Execution ──────────────────────────────────────────────────────

console.log(`\n${"█".repeat(60)}`);
console.log(`  S06 Final Assembly Verification Gate`);
console.log(`  Root: ${rootDir}`);
console.log(`  Date: ${new Date().toISOString()}`);
console.log(`${"█".repeat(60)}`);

const overallStart = Date.now();
const results = [];

for (const stage of stages) {
  const result = runStage(stage.name, stage.cmd, stage.args, {
    timeout: stage.timeout,
  });
  results.push({ name: stage.name, ...result });

  if (!result.passed) {
    // Fail fast: print summary and exit
    console.log(`\n${"█".repeat(60)}`);
    console.log(`  ⛔ VERIFICATION GATE FAILED at stage: ${stage.name}`);
    console.log(`  Total elapsed: ${((Date.now() - overallStart) / 1000).toFixed(1)}s`);
    console.log(`${"█".repeat(60)}`);
    printSummary(results);
    process.exit(result.exitCode);
  }
}

// All stages passed
console.log(`\n${"█".repeat(60)}`);
console.log(`  ✅ ALL STAGES PASSED — S06 Final Assembly Verified`);
console.log(`  Total elapsed: ${((Date.now() - overallStart) / 1000).toFixed(1)}s`);
console.log(`${"█".repeat(60)}`);
printSummary(results);
process.exit(0);

// ── Helpers ────────────────────────────────────────────────────────

function printSummary(results) {
  console.log("\n  Stage Summary:");
  for (const r of results) {
    const icon = r.passed ? "✅" : "❌";
    console.log(`    ${icon} ${r.name} (${(r.durationMs / 1000).toFixed(1)}s)`);
  }
  console.log();
}
