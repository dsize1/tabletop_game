#!/usr/bin/env node
// 门禁：三国杀 AI 自对弈回归（P6 harness）。
//
// 目的：任何 AI / 引擎改动后，确认「机器人能打完整 5 人局」——硬指标：
//   1. 终局率 100%（无卡死、无步数超限）；
//   2. 非法动作 0（AI 选出的动作必须都在引擎合法集内且被接受）；
//   3. 状态回环 0（破环器不应被触发）；
//   4. pending 覆盖 ≥ 8 种（出牌/响应/濒死/弃牌/选牌/无懈/观武/改判/选目标…）。
//
// 口径说明：
//   - **默认 `--generals full`**（全部已实现武将去重 148 位 + 全卡牌 160 张；
//     149 = 名称勘误前旧值，见 2026-11-05 张昭张纮 同名去重），
//     且默认 strict（终局率 100% / 0 非法 / 0 回环 / 0 卡死 / pending 覆盖 ≥8）。
//     历史：2026-11-01 P6.1 Batch-1.5 起全将才达到该口径（E1–E13、F1–F3 修复见
//     `docs/games/sanguosha/ai.md` §7）。
//   - `--generals standard` 是更快的自检口径（25 将，约 1/3 时间）。
//   - 用 release 构建跑（debug 下 10 局 ≈ 90s；release ≈ 6s；全将 100 局 ≈ 65s）。
//
// 用法：
//   node apps/scripts/check-sgs-ai.mjs                     # 10 局全将（CI）
//   node apps/scripts/check-sgs-ai.mjs --games 100         # 100 局全将（验收）
//   node apps/scripts/check-sgs-ai.mjs --generals standard # 标准 25 将（快速自检）
//   node apps/scripts/check-sgs-ai.mjs --generals full --no-strict  # 仅盘点，不判失败

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
const generalArg = String(arg("generals", "full"));
const generals =
  generalArg === "full"
    ? "standard+wind_fire_forest_mountain+yijiang+jiexian"
    : generalArg;
const strict = !args.includes("--no-strict");

const MIN_KINDS = 8;
const MIN_FINISH_RATE = 1.0;

const cmd = "cargo";
const argv = [
  "run",
  "-q",
  "--release",
  "--manifest-path",
  "crates/Cargo.toml",
  "-p",
  "sanguosha",
  "--example",
  "selfplay",
  "--",
  "--games",
  String(games),
  "--players",
  String(players),
  "--seed",
  String(seed),
  "--max-steps",
  String(maxSteps),
  "--generals",
  generals,
  "--json",
];

console.log(
  `[sgs-ai] 自对弈 ${games} 局 × ${players} 人（seed=${seed}, generals=${generals}）…`,
);
const started = Date.now();
const run = spawnSync(cmd, argv, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
if (run.error) {
  console.error(`[sgs-ai] 启动失败：${run.error.message}`);
  process.exit(1);
}
if (run.status !== 0) {
  // 自对弈程序在有非法动作/卡死时以非 0 退出；继续解析输出以便给出明细。
  console.error(run.stderr?.trim() || "");
}

const line = (run.stdout ?? "")
  .split(/\r?\n/)
  .map((s) => s.trim())
  .filter((s) => s.startsWith("{"))
  .pop();
if (!line) {
  console.error("[sgs-ai] 未取到 JSON 结果（stdout 为空或格式异常）");
  console.error((run.stdout ?? "").slice(0, 2000));
  process.exit(1);
}

let d;
try {
  d = JSON.parse(line);
} catch (err) {
  console.error(`[sgs-ai] 结果解析失败：${err.message}`);
  process.exit(1);
}

const problems = [];
if (strict) {
  if (d.finished !== d.games) {
    problems.push(`终局率 ${d.finished}/${d.games}（要求 ${MIN_FINISH_RATE * 100}%）`);
  }
  if (d.illegal > 0) problems.push(`非法动作 ${d.illegal} 次`);
  if (d.loops > 0) problems.push(`状态回环 ${d.loops} 次（破环器被触发）`);
  if (d.stalled > 0) problems.push(`卡死 ${d.stalled} 次`);
  if ((d.pending_kinds ?? []).length < MIN_KINDS) {
    problems.push(
      `pending 覆盖 ${(d.pending_kinds ?? []).length} 种（要求 ≥ ${MIN_KINDS}）：${(d.pending_kinds ?? []).join(",")}`,
    );
  }
}

const elapsed = ((Date.now() - started) / 1000).toFixed(1);
console.log(
  `[sgs-ai] ${d.games} 局 / 终局 ${d.finished} / illegal ${d.illegal} / loops ${d.loops} / stalled ${d.stalled} / ` +
    `步数 ${d.steps_total}（max ${d.max_game_steps}）/ 胜率 ${JSON.stringify(d.wins)} / ` +
    `kinds ${(d.pending_kinds ?? []).length} / ${elapsed}s`,
);
if (!strict) {
  console.log("[sgs-ai] --no-strict：记录性运行（不判失败）");
}
if ((d.errors ?? []).length) {
  console.log(`[sgs-ai] 失败明细（${d.errors.length}）：`);
  for (const e of d.errors.slice(0, 20)) console.log(`  - ${e}`);
}
if ((d.recovered ?? []).length) {
  console.log(`[sgs-ai] 引擎拒绝并回落（${d.recovered.length}）：`);
  for (const e of d.recovered.slice(0, 20)) console.log(`  - ${e}`);
}

if (problems.length) {
  console.error("[sgs-ai] 未通过：");
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[sgs-ai] 通过");
