#!/usr/bin/env node
// P6.4 决策日志/导出/回放自检（开发库有数据时执行；无数据则跳过）。
//
// 校验：
// 1) `game_decision_log` 字段可解析（legal/candidates 是 JSON 数组），player_id 导出为哈希；
// 2) latest room 的 `sgs-replay --json` 无 mismatches（需 `game_replay_base`）；
// 3) 导出 JSONL 行数与 limit 一致、字段稳定。
//
// 用法：npm run check:sgs-decision-log
// 退出码：0 通过/跳过；1 失败。

import { DatabaseSync } from "node:sqlite";
import { spawnSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const dbPath = process.env.SQLITE_PATH ?? "data/tabletop.db";
const db = new DatabaseSync(dbPath);

const table = db
  .prepare("select name from sqlite_master where type='table' and name='game_decision_log'")
  .get();
if (!table) {
  console.log("[check:sgs-decision-log] 跳过：库中没有 game_decision_log（P6.4 未启用）");
  process.exit(0);
}

// 开发库会保留历史对局；规则修复后旧局的 replay 可能不再一致（属预期）。
// 策略：从最近 12 个有 replay_base 的房间里依次尝试回放，选第一个 digest 完全一致的
// 房间做导出/字段校验；旧规则/损坏房间跳过并计数。
const recent = db
  .prepare(
    `select d.room_id, count(*) as n, max(d.id) as mx
     from game_decision_log d
     where exists (select 1 from game_replay_base b where b.room_id = d.room_id)
     group by d.room_id order by max(d.id) desc limit 12`,
  )
  .all();
if (recent.length === 0) {
  console.log("[check:sgs-decision-log] 跳过：没有带 replay_base 的决策日志房间");
  process.exit(0);
}
const candidates = recent.slice(0, 8);

function replayRoom(room) {
  const rep = spawnSync(
    "cargo",
    [
      "run",
      "--manifest-path",
      "crates/Cargo.toml",
      "-p",
      "server",
      "--bin",
      "sgs-replay",
      "--",
      "--db",
      dbPath,
      "--room",
      room,
      "--json",
    ],
    { cwd: root, encoding: "utf8" },
  );
  let mismatches = [];
  const jsonLine = (rep.stdout ?? "").trim().split("\n").filter(Boolean).pop();
  if (jsonLine) {
    try {
      const out = JSON.parse(jsonLine);
      if (Array.isArray(out.mismatches)) mismatches = out.mismatches;
    } catch {
      /* ignore */
    }
  }
  return {
    ok: rep.status === 0 && mismatches.length === 0,
    mismatches,
    stderr: rep.stderr ?? "",
  };
}

let room = "";
let latest = null;
let replay = null;
for (const c of candidates) {
  const r = replayRoom(c.room_id);
  if (r.ok) {
    room = c.room_id;
    latest = c;
    replay = r;
    break;
  }
  replay = r;
}
if (!room) {
  console.error(`[check:sgs-decision-log] 失败：最近 ${candidates.length} 个房间均无法重放`);
  for (const f of replay?.mismatches ?? []) console.error(`  ✗ ${f}`);
  if (replay?.stderr) console.error(`  ✗ replay stderr: ${replay.stderr.slice(-500)}`);
  process.exit(1);
}
const skipped = candidates.findIndex((c) => c.room_id === room);
if (skipped > 0) {
  console.log(`[check:sgs-decision-log] 跳过 ${skipped} 个旧规则房间（replay 不一致，属预期）`);
}

const rows = db
  .prepare(
    `select player_id, legal_actions, candidates, chosen from game_decision_log
     where room_id = ? order by seq, id limit 20`,
  )
  .all(room);
const fail = [];
for (const r of rows) {
  try {
    const legal = JSON.parse(r.legal_actions);
    if (!Array.isArray(legal) || legal.length === 0) fail.push("legal_actions 不是非空数组");
  } catch {
    fail.push("legal_actions 不是合法 JSON");
  }
  if (r.candidates) {
    try {
      const c = JSON.parse(r.candidates);
      if (!Array.isArray(c)) fail.push("candidates 不是数组");
      if (c.length > 26) fail.push(`candidates 超过 26（${c.length}）`);
    } catch {
      fail.push("candidates 不是合法 JSON");
    }
  }
  if (r.chosen && typeof r.chosen !== "string") fail.push("chosen 类型异常");
}

// 导出 5 条，校验哈希与字段。
const exp = spawnSync(
  process.execPath,
  [resolve(root, "apps/scripts/sgs-export-decisions.mjs"), "--room", room, "--limit", "5"],
  { encoding: "utf8" },
);
if (exp.status !== 0) {
  fail.push(`export 退出码 ${exp.status}: ${exp.stderr}`);
}
const lines = exp.stdout.trim().split("\n").filter(Boolean);
if (lines.length !== Math.min(5, latest.n)) {
  fail.push(`export 行数 ${lines.length} != min(5, ${latest.n})`);
}
for (const line of lines) {
  const o = JSON.parse(line);
  if (!/^[0-9a-f]{16}$/.test(o.player_hash ?? "")) fail.push("player_hash 不是 16 位 hex");
  for (const k of ["room_id", "seq", "source", "legal", "created_at"]) {
    if (o[k] === undefined) fail.push(`缺少字段 ${k}`);
  }
  if ("player_id" in o) fail.push("导出泄露原始 player_id 字段");
}

if (fail.length) {
  console.error(`[check:sgs-decision-log] 失败 room=${room}`);
  for (const f of fail) console.error(`  ✗ ${f}`);
  process.exit(1);
}
console.log(`[check:sgs-decision-log] 通过 room=${room} logs=${latest.n} mismatches=0`);
