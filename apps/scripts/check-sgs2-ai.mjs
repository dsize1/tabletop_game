#!/usr/bin/env node
// 门禁：内核 v2 AI 自对弈（M3 DoD：10 局打满、无 illegal / 无环 / 无卡死）。
//
// 口径（沿用 v1 `check:sgs-ai` 的硬指标，但不追求 pending 覆盖数）：
//   1. 终局率 100%；
//   2. 非法动作 0；
//   3. 状态回环 0；
//   4. 卡死 0；
//   5. pending 覆盖 ≥ 4 种（响应 / 弃牌 / 选牌 / 技能…），防止「只测到出牌」。
//
// 用法：
//   node apps/scripts/check-sgs2-ai.mjs                # 10 局 × 5 人
//   node apps/scripts/check-sgs2-ai.mjs --games 30 --seed 7
//   node apps/scripts/check-sgs2-ai.mjs --no-strict    # 记录性运行

import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
function arg(name, fallback = null) {
  const i = args.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = args[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

const games = Number(arg("games", 10));
const players = Number(arg("players", 5));
const seed = Number(arg("seed", 1));
const maxSteps = Number(arg("max-steps", 20000));
const strict = !args.includes("--no-strict");
const MIN_KINDS = 4;

const argv = [
  "run",
  "-q",
  "--manifest-path",
  "crates/Cargo.toml",
  "-p",
  "sanguosha2",
  "--example",
  "sgs2-selfplay",
  "--",
  "--games",
  String(games),
  "--players",
  String(players),
  "--seed",
  String(seed),
  "--max-steps",
  String(maxSteps),
  "--json",
];

console.log(`[sgs2-ai] 自对弈 ${games} 局 × ${players} 人（seed=${seed}）…`);
const started = Date.now();
const run = spawnSync("cargo", argv, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
if (run.error) {
  console.error(`[sgs2-ai] 启动失败：${run.error.message}`);
  process.exit(1);
}
if (run.status !== 0) {
  console.error(run.stderr?.trim() || "");
}

const line = (run.stdout ?? "")
  .split(/\r?\n/)
  .map((s) => s.trim())
  .filter((s) => s.startsWith("{"))
  .pop();
if (!line) {
  console.error("[sgs2-ai] 未取到 JSON 结果");
  console.error((run.stdout ?? "").slice(0, 2000));
  process.exit(1);
}

let d;
try {
  d = JSON.parse(line);
} catch (err) {
  console.error(`[sgs2-ai] 结果解析失败：${err.message}`);
  process.exit(1);
}

const problems = [];
const kinds = d.pending_kinds ?? [];
if (strict) {
  if (d.finished !== d.games) problems.push(`终局率 ${d.finished}/${d.games}`);
  if (d.illegal > 0) problems.push(`非法动作 ${d.illegal} 次`);
  if (d.loops > 0) problems.push(`状态回环 ${d.loops} 次`);
  if (d.stalled > 0) problems.push(`卡死 ${d.stalled} 次`);
  if (kinds.length < MIN_KINDS) {
    problems.push(`pending 覆盖 ${kinds.length} 种（要求 ≥ ${MIN_KINDS}）：${kinds.join(",")}`);
  }
}

const elapsed = ((Date.now() - started) / 1000).toFixed(1);
console.log(
  `[sgs2-ai] ${d.games} 局 / 终局 ${d.finished} / illegal ${d.illegal} / loops ${d.loops} / stalled ${d.stalled} / ` +
    `步数 ${d.steps} / kinds ${kinds.length} / 胜方 ${JSON.stringify(d.winners)} / ${elapsed}s`,
);
if ((d.errors ?? []).length) {
  console.log(`[sgs2-ai] 明细（${d.errors.length}）：`);
  for (const e of d.errors.slice(0, 20)) console.log(`  - ${e}`);
}
if (problems.length) {
  console.error("[sgs2-ai] 未通过：");
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[sgs2-ai] 通过");
