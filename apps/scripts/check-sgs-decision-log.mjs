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

const latest = db
  .prepare(
    `select room_id, count(*) as n from game_decision_log
     group by room_id order by max(id) desc limit 1`,
  )
  .get();
if (!latest?.room_id) {
  console.log("[check:sgs-decision-log] 跳过：还没有决策日志数据");
  process.exit(0);
}
const room = latest.room_id;

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

// 回放（有 base 才真正校验 digest；无 base 脚本自身会降级）。
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
    "--room",
    room,
    "--json",
  ],
  { cwd: root, encoding: "utf8" },
);
if (rep.status !== 0) {
  fail.push(`replay 退出码 ${rep.status}: ${rep.stderr.slice(-500)}`);
}
const jsonLine = (rep.stdout ?? "").trim().split("\n").filter(Boolean).pop();
if (jsonLine) {
  try {
    const out = JSON.parse(jsonLine);
    if (Array.isArray(out.mismatches) && out.mismatches.length > 0) {
      fail.push(`replay mismatches: ${out.mismatches.join("; ")}`);
    }
  } catch {
    /* replay 非 JSON 时忽略（旧实现） */
  }
}

if (fail.length) {
  console.error(`[check:sgs-decision-log] 失败 room=${room}`);
  for (const f of fail) console.error(`  ✗ ${f}`);
  process.exit(1);
}
console.log(`[check:sgs-decision-log] 通过 room=${room} logs=${latest.n}`);
