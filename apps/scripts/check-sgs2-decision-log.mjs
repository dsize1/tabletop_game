#!/usr/bin/env node
// M4 harness：v2 AI 决策日志自检（复用 v1 `check:sgs-decision-log` 的口径，但不依赖 server/SQLite）。
//
// 来源：`cargo run -p sanguosha2 --example sgs2-selfplay -- --log <tmp> --json`
// 校验（每条 JSONL）：
//   1. 必填字段齐全（actor/phase/legal/chosen/hp/hand_counts/deck_count…）；
//   2. `legal` 是数组且 `legal_count === legal.length`、`chosen ∈ legal`；
//   3. 候选上限 ≤ 64（v2 手牌动作会按目标展开；v1 的 26 是 5 人+全将经验值）；
//   4. `phase` 合法；`hp`/`hand_counts` 长度 = 人数；
//   5. 同一局 step 单调递增（1..n）。
//
// 用法：node apps/scripts/check-sgs2-decision-log.mjs [--games 2] [--players 5] [--seed 1]

import { spawnSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = process.argv.slice(2);
function arg(name, fallback) {
  const i = args.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = args[i + 1];
  return v && !v.startsWith("--") ? v : fallback;
}
const games = Number(arg("games", 2));
const players = Number(arg("players", 5));
const seed = Number(arg("seed", 1));
// 2026-11-11：引擎在「大弃牌阶段/多目标」场景可合法产出 >64 个候选（实测 70）。
// 上限随引擎能力调整；AI 侧仍按打分裁剪，不依赖此门禁。
const maxLegal = 96;
const logFile = path.join(os.tmpdir(), `sgs2-decision-log-${process.pid}.jsonl`);

const run = spawnSync(
  "cargo",
  [
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
    "--log",
    logFile,
    "--json",
  ],
  { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
if (run.status !== 0) {
  console.error("[sgs2-decision-log] 自对弈失败：");
  console.error(run.stderr?.slice(0, 2000) ?? "");
  rmSync(logFile, { force: true });
  process.exit(1);
}

let text;
try {
  text = readFileSync(logFile, "utf8");
} catch {
  console.error(`[sgs2-decision-log] 未生成日志：${logFile}`);
  process.exit(1);
}
rmSync(logFile, { force: true });

const problems = [];
const lines = text.split(/\r?\n/).filter((l) => l.trim());
const byGame = new Map();
const phases = new Set(["Prepare", "Judge", "Draw", "Play", "Discard", "End"]);
for (const [i, line] of lines.entries()) {
  let r;
  try {
    r = JSON.parse(line);
  } catch (e) {
    problems.push(`第 ${i + 1} 行 JSON 解析失败：${e.message}`);
    continue;
  }
  for (const key of [
    "game",
    "step",
    "turn",
    "round",
    "phase",
    "actor",
    "legal",
    "legal_count",
    "chosen",
    "hp",
    "hand_counts",
    "deck_count",
  ]) {
    if (!(key in r)) problems.push(`第 ${i + 1} 行缺字段 ${key}`);
  }
  if (!Array.isArray(r.legal)) {
    problems.push(`第 ${i + 1} 行 legal 不是数组`);
  } else {
    if (r.legal_count !== r.legal.length) {
      problems.push(`第 ${i + 1} 行 legal_count=${r.legal_count} ≠ legal.length=${r.legal.length}`);
    }
    if (r.legal.length > maxLegal) {
      problems.push(`第 ${i + 1} 行候选 ${r.legal.length} 超过上限 ${maxLegal}`);
    }
    if (!r.legal.includes(r.chosen)) {
      problems.push(`第 ${i + 1} 行 chosen=${r.chosen} 不在 legal 内`);
    }
  }
  if (!phases.has(r.phase)) problems.push(`第 ${i + 1} 行未知 phase=${r.phase}`);
  if (!Array.isArray(r.hp) || r.hp.length !== players) {
    problems.push(`第 ${i + 1} 行 hp 长度 ≠ 人数 ${players}`);
  }
  if (!Array.isArray(r.hand_counts) || r.hand_counts.length !== players) {
    problems.push(`第 ${i + 1} 行 hand_counts 长度 ≠ 人数 ${players}`);
  }
  const arr = byGame.get(r.game) ?? [];
  arr.push(r.step);
  byGame.set(r.game, arr);
}
for (const [g, steps] of byGame) {
  for (let i = 0; i < steps.length; i += 1) {
    if (steps[i] !== i + 1) {
      problems.push(`game ${g}: step 不连续/不单调（第 ${i + 1} 条 = ${steps[i]}）`);
      break;
    }
  }
}

if (byGame.size !== games) {
  problems.push(`日志覆盖 ${byGame.size}/${games} 局`);
}

console.log(
  `[sgs2-decision-log] ${byGame.size} 局 / ${lines.length} 条决策 / 候选上限 ${maxLegal}`,
);
if (problems.length) {
  console.error(`[sgs2-decision-log] 未通过（${problems.length}）：`);
  for (const p of problems.slice(0, 20)) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[sgs2-decision-log] OK");
