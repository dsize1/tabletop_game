// 读取某局三国杀的**完整出牌历史**（直接读 SQLite `game_state` 快照，不需要起服务）。
//
// 用法：
//   npm run sgs:history -- --room <room_id>
//   npm run sgs:history -- --room <room_id> --tail 40      # 只看最近 40 条
//   npm run sgs:history -- --room <room_id> --grep 麒麟弓    # 过滤关键字
//   npm run sgs:history -- --room <room_id> --json         # 输出 JSON（含公共信息）
//   npm run sgs:history -- --list                          # 列出最近的对局
//
// 默认库：`SQLITE_PATH` 或 `data/tabletop.db`（与 server/MCP 一致）。

import { DatabaseSync } from "node:sqlite";

function arg(name, fallback = null) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

const dbPath = arg("db", process.env.SQLITE_PATH ?? "data/tabletop.db");
const room = arg("room");
const tail = Number(arg("tail", 0)) || 0;
const grep = arg("grep");
const asJson = Boolean(arg("json", false));
const listOnly = Boolean(arg("list", false));

const db = new DatabaseSync(String(dbPath));

if (listOnly || !room) {
  const rows = db
    .prepare("select room_id, game_id, version, updated_at from game_state order by updated_at desc limit 20")
    .all();
  console.log(`库：${dbPath}（最近 20 局）`);
  for (const r of rows) {
    console.log(`${r.room_id}  ${r.game_id}  v${r.version}  ${new Date(Number(r.updated_at) * 1000).toISOString()}`);
  }
  if (!room) process.exit(0);
}

const row = db
  .prepare("select state from game_state where room_id = ?")
  .get(String(room));
if (!row) {
  console.error(`未找到对局 ${room}（用 --list 查看最近对局）`);
  process.exit(1);
}
const outer = JSON.parse(row.state);
const st = typeof outer.data === "string" ? JSON.parse(outer.data) : outer.data;
const history = Array.isArray(st.history) ? st.history : [];

if (asJson) {
  console.log(
    JSON.stringify(
      {
        room_id: room,
        phase: st.phase,
        winner: st.winner ?? null,
        round: st.round,
        turn_index: st.turn_index,
        history_seq: st.history_seq,
        seats: (st.seats ?? []).map((s) => ({
          player_id: s.player_id,
          general: s.general_name,
          identity: s.identity,
          hp: s.hp,
          alive: s.alive,
          is_bot: s.is_bot,
        })),
        history,
      },
      null,
      2,
    ),
  );
  process.exit(0);
}

const seatLine = (st.seats ?? [])
  .map((s) => `${s.player_id}(${s.general_name}/${s.identity}/hp${s.hp}${s.alive ? "" : "☠"})`)
  .join("  ");
console.log(`room=${room}  phase=${st.phase}  round=${st.round}  turn_index=${st.turn_index}`);
console.log(`winner=${st.winner ? `${st.winner.team}:${(st.winner.players ?? []).join(",")}` : "-"}`);
console.log(seatLine);
console.log(`history: 保留 ${history.length} 条 / 总计 ${st.history_seq} 条（上限 800，早期被截断的不可恢复）`);
console.log("".padEnd(60, "-"));

let rows = history;
if (grep) rows = rows.filter((h) => String(h.text).includes(String(grep)));
if (tail > 0) rows = rows.slice(-tail);
for (const h of rows) {
  console.log(`#${String(h.seq).padStart(4, " ")}  ${h.text}`);
}
