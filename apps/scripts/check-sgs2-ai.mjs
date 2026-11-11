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
//   node apps/scripts/check-sgs2-ai.mjs                # 默认多个 seed × 10 局（见 DEFAULT_SEEDS）
//   node apps/scripts/check-sgs2-ai.mjs --seed 7       # 单 seed
//   node apps/scripts/check-sgs2-ai.mjs --seeds 1,2,8  # 指定 seed 列表
//   node apps/scripts/check-sgs2-ai.mjs --seeds all    # 1–12 全扫（报告用）
//   node apps/scripts/check-sgs2-ai.mjs --no-strict    # 记录性运行

import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
function arg(name, fallback = null) {
  const i = args.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = args[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

// 已知全绿 seed（S2-b 12 seed × 10 局扫描：1–7、9、11、12 全绿；8/10 各有 1 局回环，
// 默认取前 3 个（30 局，约 24s）；全量用 `--seeds all`（120 局，约 90s）。
// 见 docs/games/sanguosha/ai.md「已知缺陷」）。默认用这批 seed，防「单 seed 偶然通过」。
const DEFAULT_SEEDS = [1, 2, 3];
const games = Number(arg("games", 10));
const players = Number(arg("players", 5));
const seedArg = arg("seeds", null);
const seeds = (() => {
  if (seedArg === "all") return Array.from({ length: 12 }, (_, i) => i + 1);
  if (typeof seedArg === "string" && seedArg.trim()) {
    return seedArg
      .split(",")
      .map((s) => Number(s.trim()))
      .filter((n) => Number.isFinite(n) && n > 0);
  }
  if (args.includes("--seed")) return [Number(arg("seed", 1))];
  return DEFAULT_SEEDS;
})();
if (seeds.length === 0) {
  console.error("[sgs2-ai] --seeds 为空");
  process.exit(1);
}
const maxSteps = Number(arg("max-steps", 20000));
const strict = !args.includes("--no-strict");
const MIN_KINDS = 4;

function runSeed(seed) {
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
  const run = spawnSync("cargo", argv, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
  if (run.error) {
    console.error(`[sgs2-ai] seed=${seed} 启动失败：${run.error.message}`);
    process.exit(1);
  }
  if (run.status !== 0) console.error(run.stderr?.trim() || "");
  const line = (run.stdout ?? "")
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.startsWith("{"))
    .pop();
  if (!line) {
    console.error(`[sgs2-ai] seed=${seed} 未取到 JSON 结果`);
    console.error((run.stdout ?? "").slice(0, 2000));
    process.exit(1);
  }
  try {
    return JSON.parse(line);
  } catch (err) {
    console.error(`[sgs2-ai] seed=${seed} 结果解析失败：${err.message}`);
    process.exit(1);
  }
}

const started = Date.now();
const runs = [];
for (const seed of seeds) {
  console.log(`[sgs2-ai] 自对弈 ${games} 局 × ${players} 人（seed=${seed}）…`);
  runs.push({ seed, d: runSeed(seed) });
}
const d = {
  games: runs.reduce((n, r) => n + r.d.games, 0),
  finished: runs.reduce((n, r) => n + r.d.finished, 0),
  illegal: runs.reduce((n, r) => n + r.d.illegal, 0),
  loops: runs.reduce((n, r) => n + r.d.loops, 0),
  stalled: runs.reduce((n, r) => n + r.d.stalled, 0),
  long: runs.reduce((n, r) => n + (r.d.long ?? 0), 0),
  max_round: runs.reduce((n, r) => Math.max(n, r.d.max_round ?? 0), 0),
  label_dups: runs.reduce((n, r) => n + (r.d.label_dups ?? 0), 0),
  steps: runs.reduce((n, r) => n + r.d.steps, 0),
  pending_kinds: [...new Set(runs.flatMap((r) => r.d.pending_kinds ?? []))],
  winners: runs.reduce((acc, r) => {
    for (const [k, v] of Object.entries(r.d.winners ?? {})) acc[k] = (acc[k] ?? 0) + v;
    return acc;
  }, {}),
  errors: runs.flatMap((r) => (r.d.errors ?? []).map((e) => `seed=${r.seed} ${e}`)),
};

const problems = [];
const kinds = d.pending_kinds ?? [];
if (strict) {
  if (d.finished !== d.games) problems.push(`终局率 ${d.finished}/${d.games}`);
  if (d.illegal > 0) problems.push(`非法动作 ${d.illegal} 次`);
  if (d.loops > 0) problems.push(`状态回环 ${d.loops} 次`);
  if (d.stalled > 0) problems.push(`卡死 ${d.stalled} 次`);
  // 选项文案重复（相同文案 + 不同后果）→ 端侧无法区分（2026-11-11「使用【杀】」事故的门禁化）。
  if (d.label_dups > 0) problems.push(`选项文案重复 ${d.label_dups} 次`);
  if (kinds.length < MIN_KINDS) {
    problems.push(`pending 覆盖 ${kinds.length} 种（要求 ≥ ${MIN_KINDS}）：${kinds.join(",")}`);
  }
}

const elapsed = ((Date.now() - started) / 1000).toFixed(1);
// `long`：跨过终局阈值（round > ENDGAME_ROUND）的局数 —— 信息项（ADR-0040 房规收束长局），
// 不参与 strict 判定；`max_round` 用来观察「僵局是否又被房规绑住」。
console.log(
  `[sgs2-ai] seeds=${seeds.join(",")} · ${d.games} 局 / 终局 ${d.finished} / illegal ${d.illegal} / ` +
    `loops ${d.loops} / stalled ${d.stalled} / long ${d.long} / maxRound ${d.max_round} / ` +
    `labelDups ${d.label_dups} / ` +
    `步数 ${d.steps} / kinds ${kinds.length} / ` +
    `胜方 ${JSON.stringify(d.winners)} / ${elapsed}s`,
);
if (d.long > 0) {
  console.log(
    `[sgs2-ai] 提示：${d.long} 局跨过终局阈值（长局，已由房规收束）；最大轮次 ${d.max_round}`,
  );
}
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
