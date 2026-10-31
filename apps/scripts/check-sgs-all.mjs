#!/usr/bin/env node
// 三国杀批次聚合门禁（P4.4.2 harness）：一条命令跑完「每批流程」要求的检查。
//
// 用法：
//   node apps/scripts/check-sgs-all.mjs          # 默认：Rust 单测 + 文档/数据/用例/批次一致性 + Web 单测 + sgs-web e2e
//   node apps/scripts/check-sgs-all.mjs --fast   # 跳过 Web/e2e（仅 Rust + 文档/数据门禁，改代码时快速自检）
//   node apps/scripts/check-sgs-all.mjs --full   # 追加 MCP 冒烟（verify:sanguosha，需要先编译 game-engine-mcp）
//
// 说明：任一步失败仍会继续跑完（便于一次看到全部问题），最后汇总并以 exit 1 结束。

import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const fast = args.has("--fast");
const full = args.has("--full");

const node = process.execPath;
const cargo = "cargo";
const cargoTest = (filter) => ({
  cmd: [cargo, "test", "--manifest-path", "crates/Cargo.toml", "-p", "sanguosha", filter ?? ""]
    .join(" ")
    .trim(),
});

const steps = [
  { name: "cargo test -p sanguosha", ...cargoTest() },
  { name: "skill-cases（每将 ≥3 用例）", cmd: `${node} apps/scripts/check-sgs-skill-cases.mjs` },
  { name: "skill-docs（skills.md ↔ 注册表）", cmd: `${node} apps/scripts/check-sgs-skill-docs.mjs` },
  { name: "engine-data（data.rs 最新）", cmd: `${node} apps/scripts/gen-sanguosha-engine-data.mjs --check` },
  { name: "batch-status（批次/p67 一致性）", cmd: `${node} apps/scripts/sgs-batch-status.mjs --check` },
  { name: "pack-pool（选将池包过滤）", ...cargoTest("pack_pool") },
  { name: "view-contract（视图契约）", ...cargoTest("view_contract") },
  { name: "interaction（交互原语契约）", ...cargoTest("interaction_") },
];

if (!fast) {
  steps.push(
    { name: "test:web（Web 视图单测）", cmd: "npm --prefix apps run test:web" },
    { name: "test:sgs-web（端到端）", cmd: `${node} apps/scripts/test-sgs-web.mjs` },
  );
}
if (full) {
  steps.push({ name: "verify:sanguosha（MCP 冒烟）", cmd: `${node} apps/verify-sanguosha.mjs` });
}

const results = [];
let tail = [];
for (const [i, step] of steps.entries()) {
  console.log(`\n━━━ [${i + 1}/${steps.length}] ${step.name} ━━━`);
  const started = Date.now();
  const r = spawnSync(step.cmd, { stdio: "inherit", shell: true, env: process.env });
  const ok = r.status === 0;
  const secs = ((Date.now() - started) / 1000).toFixed(1);
  results.push({ ...step, ok, secs });
  tail.push(`${ok ? "✅" : "❌"} ${step.name}（${secs}s）`);
}

console.log("\n━━━ 汇总 ━━━");
for (const line of tail) console.log(`  ${line}`);
const failed = results.filter((r) => !r.ok);
if (failed.length) {
  console.error(`\n❌ ${failed.length}/${results.length} 步失败：`);
  for (const f of failed) console.error(`  - ${f.name}\n    ${f.cmd}`);
  process.exitCode = 1;
} else {
  console.log(`\n✅ 全部 ${results.length} 步通过${fast ? "（--fast：未跑 Web/e2e/MCP）" : ""}`);
}
