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
const steps = [
  // S6：事实源已切 v2（v1 crate 仅保留给对拍/退役，不参与门禁）。
  { name: "skill-cases（每将 ≥1 条 v2 用例锚点）", cmd: `${node} apps/scripts/check-sgs-skill-cases.mjs` },
  { name: "general-names（武将名/错别字/近似名）", cmd: `${node} apps/scripts/check-sgs-general-names.mjs` },
  {
    name: "batch-consistency（验证批次 ↔ 去重池 ↔ 开发批次）",
    cmd: `${node} apps/scripts/check-sgs-batch-consistency.mjs`,
  },
  { name: "skill-docs（skills.md ↔ v2 注册表 + §4.1 授予技）", cmd: `${node} apps/scripts/check-sgs-skill-docs.mjs` },
  // 2026-11-06：计数类事实门禁（防 149/83 这类文档漂移）+ 技能对齐单 + 内核 v2 spike
  { name: "counts（232/85/147/预设人数 ↔ 文档）", cmd: `${node} apps/scripts/check-sgs-counts.mjs` },
  { name: "align（技能对齐单一致性/冲突阻断）", cmd: `${node} apps/scripts/check-sgs-align.mjs` },
  { name: "sgs2（内核 v2 用例 + 6 类缺陷门禁）", cmd: `${cargo} test --manifest-path crates/Cargo.toml -p sanguosha2` },
  { name: "sgs2-plugin（EnginePlugin 完备性/视图脱敏）", cmd: `${cargo} test --manifest-path crates/Cargo.toml -p sanguosha2 plugin_contract` },
  { name: "retire（旧 crate 退役 deadline 提醒）", cmd: `${node} apps/scripts/check-sgs-retire.mjs` },
  { name: "engine-data（v1 data.rs + v2 cards + catalog 最新）", cmd: `${node} apps/scripts/gen-sanguosha-engine-data.mjs --check` },
  { name: "sgs2-parity-view（v2 视图覆盖前端字段，strict）", cmd: `${node} apps/scripts/check-sgs2-view-parity.mjs --strict` },
  { name: "sgs2-coupling（v1 引用台账，白名单外失败）", cmd: `${node} apps/scripts/check-sgs2-coupling.mjs` },
  { name: "sgs2-registry（注册表/对齐单/用例覆盖）", cmd: `${node} apps/scripts/check-sgs2-registry.mjs` },
  { name: "sgs2-conventions（机制/迁移欠账防复发：体力/内核名/标记/计数/出口/kind）", cmd: `${node} apps/scripts/check-sgs2-conventions.mjs` },
  { name: "sgs2-decision-log（决策日志 schema）", cmd: `${node} apps/scripts/check-sgs2-decision-log.mjs` },
  { name: "skill-text（长按技能简述最新）", cmd: `${node} apps/scripts/gen-sgs-skill-text.mjs --check` },
  { name: "fx-assets（特效素材/manifest/预算）", cmd: `${node} apps/scripts/check-fx-assets.mjs` },
  // P6.4：决策日志/回放的自检（脚本内部用临时 SQLite，不依赖运行中的 server）
  { name: "decision-log（存储/回放基线）", cmd: `cargo test --manifest-path crates/Cargo.toml -p storage decision_log` },
  { name: "batch-status（批次/p67 一致性）", cmd: `${node} apps/scripts/sgs-batch-status.mjs --check` },
  {
    name: "pack-presets（武将包预设 ↔ engine-catalog.json ↔ data-model.md）",
    cmd: `${node} apps/scripts/check-sgs-pack-presets.mjs`,
  },
  { name: "pack-pool（v2 选将池/包过滤）", cmd: `${cargo} test --manifest-path crates/Cargo.toml -p sanguosha2 lobby::` },
  { name: "view-contract（v2 视图契约）", cmd: `npm run check:sgs-view-contract` },
  { name: "sgs2-ai（内核 v2 自对弈 10 局）", cmd: `${node} apps/scripts/check-sgs2-ai.mjs` },
  // v1 交互原语 / 自对弈门禁由 v2 的 pending 出口 + `check:sgs2-ai` 覆盖。
  // 全武将验证：矩阵 ↔ 名录/批次表一致性（纯静态，无需 server）
  { name: "generals-matrix（全武将验证矩阵一致性）", cmd: `${node} apps/scripts/sgs-general-audit.mjs --check-matrix` },
];

if (!fast) {
  steps.push(
    // P5 基座：模板/场景/预览/AI/隔离（起临时 server + MCP；需 Redis）
    { name: "workbench-preview（预览隔离/场景/AI）", cmd: `${node} apps/scripts/check-sgs-preview.mjs` },
    { name: "build:props（工作台类型检查/构建）", cmd: "npm --prefix apps run build:props" },
    { name: "test:web（Web 视图单测）", cmd: "npm --prefix apps run test:web" },
    { name: "test:sgs-web（端到端 + fx）", cmd: `${node} apps/scripts/test-sgs-web.mjs --fx` },
    // P6.3：1 真人 + 4 机器人 —— Web 动作后机器人自动推进（需 server/MCP 已构建 + Redis）
    { name: "test:sgs-bot（机器人推进 e2e）", cmd: `${node} apps/scripts/test-sgs-bot-web.mjs` },
    { name: "test:web-login（跨端登录 e2e）", cmd: `${node} apps/scripts/test-web-login.mjs` },
    // 回放自检放在「机器人推进 e2e」之后：该 e2e 会新建一局并打完整局，
    // 保证库里存在**用当前引擎**产生的可回放房间（规则修复后旧局回放不一致属预期）。
    { name: "decision-log（导出/回放自检）", cmd: `${node} apps/scripts/check-sgs-decision-log.mjs` },
  );
}
if (full) {
  steps.push({ name: "verify:sanguosha（MCP 冒烟）", cmd: `${node} apps/verify-sanguosha.mjs` });
  // 全量机器证据：148 将 L1 走查（约 70s；失败项非 0 退出）
  steps.push({
    name: "audit:generals-full（148 将 L1）",
    cmd: `${node} apps/scripts/sgs-general-audit.mjs --all --repeat 1 --out data/audit/all.json`,
  });
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
