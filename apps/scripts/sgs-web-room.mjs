// 三国杀 Web 本地验收脚手架（P3）。
//
// 作用：用 MCP（stdio）建一局 5–8 人三国杀 → 选将 → 停在出牌阶段（不结束回合），
//      再向 server `/auth/issue` 为每位玩家签 `game:{room_id}` 短 token，
//      打印每个人可直接打开的 Web 页面链接。
//
// 前提：
//   - server 与 web 已启动，且 **SQLITE_PATH 与本脚本一致**（快照是真相源）；
//   - 动作提交由 server 执行（P3 单写者），因此页面里的出牌会写回同一个 SQLite。
//
// 用法：
//   SQLITE_PATH=data/tabletop.db node apps/scripts/sgs-web-room.mjs
//   可选：
//     --players 5..8        人数（默认 5）
//     --server URL          server 内部地址（默认 http://127.0.0.1:3000）
//     --web URL             web 站点地址（默认 http://127.0.0.1:18080）
//     --ttl 秒              token 时效（默认 3600；auth 默认 8h）
//     --dump-state FILE     导出 `{ public, players }` 视图 fixture（前端纯函数测试用）
//     --keep                建局后不退出（保持 MCP 进程；一般不需要）
//
// 说明：本脚本只做「建局/选将」，之后的对局由 Web 推进；退出后 MCP 进程结束，
//      server 会在第一次请求时按需从 `game_state` 恢复对局（resources_routes::ensure_room）。

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "../..");

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  if (i === -1) return fallback;
  const v = process.argv[i + 1];
  return v && !v.startsWith("--") ? v : true;
}

const PLAYERS = Number(arg("players", 5));
const SERVER_URL = String(arg("server", process.env.SERVER_URL ?? "http://127.0.0.1:3000"));
const WEB_BASE = String(arg("web", process.env.WEB_BASE ?? "http://127.0.0.1:18080"));
const TOKEN_TTL = Number(arg("ttl", 3600));
const DUMP_STATE = arg("dump-state", null);
const KEEP = Boolean(arg("keep", false));
const SQLITE_PATH = process.env.SQLITE_PATH ?? "data/tabletop.db";

if (!Number.isFinite(PLAYERS) || PLAYERS < 5 || PLAYERS > 8) {
  console.error("[sgs-web] --players 需在 5–8 之间");
  process.exit(1);
}

function defaultExe() {
  const exe = process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp";
  return path.resolve(projectRoot, "crates/target/debug", exe);
}

const exe = process.argv[2] && !process.argv[2].startsWith("--")
  ? process.argv[2]
  : (process.env.GAME_ENGINE_MCP ?? defaultExe());

function assert(cond, msg) {
  if (!cond) throw new Error(`断言失败：${msg}`);
}

const client = new Client({ name: "sgs-web-room", version: "0.1.0" });
const transport = new StdioClientTransport({
  command: exe,
  args: [],
  cwd: projectRoot,
  env: { ...process.env, SQLITE_PATH },
});

async function call(name, args) {
  const res = await client.callTool({ name, arguments: args });
  const block = (res.content ?? []).find((c) => c.type === "text");
  return block?.text ?? "";
}

async function callJson(name, args) {
  const raw = await call(name, args);
  assert(!raw.startsWith("错误"), `${name} 失败: ${raw}`);
  return JSON.parse(raw);
}

async function mintToken(openid, roomId) {
  const r = await fetch(`${SERVER_URL}/auth/issue`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ openid, resource: `game:${roomId}`, ttl_secs: TOKEN_TTL }),
  });
  if (!r.ok) throw new Error(`/auth/issue ${r.status}（server 是否已启动且用同一 SQLITE_PATH？）`);
  const body = await r.json();
  return body.token;
}

try {
  // server 必须在跑（否则拿不到 token）
  const health = await fetch(`${SERVER_URL}/health`).catch(() => null);
  assert(health && health.ok, `server 不可用：${SERVER_URL}（先 npm run dev:up 或启动 server）`);

  await client.connect(transport);
  const players = Array.from({ length: PLAYERS }, (_, i) => `sgs-web-p${i + 1}`);
  const host = players[0];

  const created = await callJson("create_game", {
    game_id: "sanguosha",
    group_id: `sgs-web-${Date.now()}`,
    host_id: host,
  });
  const roomId = created.room_id;
  for (const p of players.slice(1)) {
    await callJson("execute_action", { room_id: roomId, player_id: p, action: "lobby:join" });
  }
  await callJson("execute_action", { room_id: roomId, player_id: host, action: "lobby:start" });

  // 选将：主公先选，其余按顺序；扫描有候选的玩家（机器人不存在，全部真人位）。
  let picked = 0;
  for (let guard = 0; guard < 40 && picked < PLAYERS; guard += 1) {
    let acted = false;
    for (const p of players) {
      const legal = await callJson("get_legal_actions", { room_id: roomId, player_id: p });
      const choose = legal.find((a) => a.id.startsWith("setup:choose_general:"));
      if (!choose) continue;
      await callJson("execute_action", { room_id: roomId, player_id: p, action: choose.id });
      picked += 1;
      acted = true;
      break;
    }
    if (!acted) break;
  }
  assert(picked === PLAYERS, `应完成 ${PLAYERS} 人选将，实际 ${picked}`);

  // 若首个回合先触发必须交互（如观星 arrange），按默认顺序提交，停在出牌阶段。
  for (let guard = 0; guard < 8; guard += 1) {
    const state = await callJson("get_state", { room_id: roomId });
    const pending = state.data?.public_board?.pending;
    if (!pending) break;
    const legal = await callJson("get_legal_actions", {
      room_id: roomId,
      player_id: pending.actor,
    });
    const action = legal.find((a) => a.id === "pass")?.id ?? legal[0]?.id;
    assert(action, `pending ${pending.kind} 无合法动作`);
    await callJson("execute_action", { room_id: roomId, player_id: pending.actor, action });
  }

  const state = await callJson("get_state", { room_id: roomId });
  const board = state.data.public_board;
  assert(state.data.phase === "turn", `应进入 turn，实际 ${state.data.phase}`);
  assert(board.turn.phase === "play", `应停在出牌阶段，实际 ${board.turn.phase}`);

  const tokens = {};
  for (const p of players) {
    tokens[p] = await mintToken(p, roomId);
  }

  console.log(`[sgs-web] room=${roomId}`);
  console.log(`[sgs-web] SQLITE_PATH=${SQLITE_PATH}（server 必须一致）`);
  console.log(`[sgs-web] 可在浏览器打开（每人一条，勿互传）：`);
  for (const seat of board.seats) {
    const pid = seat.player_id;
    const link =
      `${WEB_BASE}/tabletop_game/sanguosha/player/${encodeURIComponent(pid)}` +
      `?token=${encodeURIComponent(tokens[pid])}`;
    const general = seat.general_name || seat.general || "-";
    const tag = board.turn.player_id === pid ? "（当前回合）" : "";
    console.log(`  - ${pid}  ${general} hp=${seat.hp}/${seat.max_hp} ${tag}`);
    console.log(`    ${link}`);
  }

  if (DUMP_STATE) {
    const playersView = {};
    for (const p of players) {
      const pv = await callJson("get_player_view", { room_id: roomId, player_id: p });
      playersView[p] = pv.data;
    }
    const bundle = {
      generated_at: new Date().toISOString(),
      room_id: roomId,
      public: state.data,
      players: playersView,
    };
    await fs.mkdir(path.dirname(path.resolve(String(DUMP_STATE))), { recursive: true });
    await fs.writeFile(String(DUMP_STATE), JSON.stringify(bundle, null, 2), "utf8");
    console.log(`[sgs-web] 视图 fixture 已写入 ${DUMP_STATE}`);
  }

  if (KEEP) {
    console.log("[sgs-web] --keep：保持 MCP 进程（Ctrl+C 退出）");
    await new Promise(() => {});
  }
} catch (err) {
  console.error("[sgs-web] 失败:", err?.message ?? err);
  process.exitCode = 1;
} finally {
  await client.close();
}
