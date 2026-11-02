#!/usr/bin/env node
// 导出三国杀决策日志（P6.4）：SQLite `game_decision_log` → JSONL。
//
// 用法：
//   npm run sgs:export-decisions -- --room <room_id> [--out decisions.jsonl]
//   npm run sgs:export-decisions -- --room <room_id> --since 100 --limit 500
//   npm run sgs:export-decisions -- --game sanguosha --limit 100
//
// 口径：
// - 默认库 `SQLITE_PATH` 或 `data/tabletop.db`（与 server/MCP 一致）；
// - `player_id` 只输出 **sha256 前 16 位**，不落原始 openid；
// - 输出按 `seq,id` 升序，字段稳定，便于 diff / 训练集合并。

import { DatabaseSync } from "node:sqlite";
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";

function arg(name, fallback = null) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

const dbPath = String(arg("db", process.env.SQLITE_PATH ?? "data/tabletop.db"));
const room = arg("room");
const game = arg("game", "sanguosha");
const since = Number(arg("since", 0)) || 0;
const limit = Number(arg("limit", 100000)) || 100000;
const out = arg("out");

const db = new DatabaseSync(String(dbPath));

function tableExists(name) {
  return (
    db
      .prepare("select name from sqlite_master where type='table' and name = ?")
      .get(name) !== undefined
  );
}

if (!tableExists("game_decision_log")) {
  console.error(
    `[sgs-export-decisions] ${dbPath} 没有 game_decision_log 表（P6.4 尚未启用或库太旧）`,
  );
  process.exit(1);
}

const where = ["game_id = ?", "seq >= ?"];
const params = [String(game), since];
if (room) {
  where.push("room_id = ?");
  params.push(String(room));
}
const rows = db
  .prepare(
    `select id, game_id, room_id, seq, player_id, source, legal_actions, candidates,
            chosen, confidence, latency_ms, reason, state_digest, created_at
     from game_decision_log
     where ${where.join(" and ")}
     order by seq asc, id asc
     limit ?`,
  )
  .all(...params, limit);

const hash = (id) => createHash("sha256").update(String(id)).digest("hex").slice(0, 16);
const parseJson = (text, fallback) => {
  if (!text) return fallback;
  try {
    return JSON.parse(text);
  } catch {
    return fallback;
  }
};

const lines = rows.map((r) =>
  JSON.stringify({
    id: r.id,
    game_id: r.game_id,
    room_id: r.room_id,
    seq: r.seq,
    player_hash: hash(r.player_id),
    source: r.source,
    legal: parseJson(r.legal_actions, []),
    candidates: parseJson(r.candidates, null),
    chosen: r.chosen,
    confidence: r.confidence,
    latency_ms: r.latency_ms,
    reason: r.reason,
    state_digest: r.state_digest,
    created_at: r.created_at,
  }),
);

const text = lines.length ? `${lines.join("\n")}\n` : "";
if (out && out !== true) {
  writeFileSync(String(out), text);
  console.error(`[sgs-export-decisions] ${lines.length} 条 → ${out}`);
} else {
  process.stdout.write(text);
}
