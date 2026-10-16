// 拉斯维加斯 MCP 端到端冒烟：通过 stdio 调用 game-engine-mcp 统一原语，
// 跑通 create_game → lobby:join → lobby:start → place/reroll → … → finished。
//
// 用法：node apps/verify-las-vegas.mjs [exe路径]
//   环境变量：
//     GAME_ENGINE_MCP  game-engine-mcp 可执行文件路径（默认 crates/target/debug）
//     SQLITE_PATH      冒烟库路径（默认临时文件，不污染 data/tabletop.db）
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");

function defaultExe() {
  const exe = process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp";
  return path.resolve(projectRoot, "crates/target/debug", exe);
}

const exe = process.argv[2] ?? process.env.GAME_ENGINE_MCP ?? defaultExe();
const dbPath = process.env.SQLITE_PATH ?? path.join(os.tmpdir(), `tt_lv_smoke_${Date.now()}.db`);

function assert(cond, msg) {
  if (!cond) throw new Error(`断言失败：${msg}`);
}

const client = new Client({ name: "verify-las-vegas", version: "0.1.0" });
const transport = new StdioClientTransport({
  command: exe,
  args: [],
  cwd: projectRoot,
  env: { SQLITE_PATH: dbPath },
});

async function call(name, args) {
  const res = await client.callTool({ name, arguments: args });
  const block = (res.content ?? []).find((c) => c.type === "text");
  return block?.text ?? "";
}

try {
  await client.connect(transport);
  console.log(`[las-vegas] 已连接 MCP: ${exe}`);

  const tools = await client.listTools();
  const names = tools.tools.map((t) => t.name);
  for (const t of ["create_game", "get_player_view", "execute_action", "get_state"]) {
    assert(names.includes(t), `缺少 MCP 原语 ${t}`);
  }
  console.log(`[las-vegas] 原语 ${names.length} 个 OK`);

  const roomJson = await call("create_game", {
    game_id: "las-vegas",
    group_id: "smoke-group",
    host_id: "smoke-host",
  });
  const created = JSON.parse(roomJson);
  assert(created.room_id, `create_game 失败: ${roomJson}`);
  const roomId = created.room_id;
  console.log(`[las-vegas] create_game room=${roomId} phase=` + JSON.parse(roomJson).data.phase);

  let out = await call("execute_action", { room_id: roomId, player_id: "smoke-p2", action: "lobby:join" });
  assert(!out.startsWith("错误"), `lobby:join 失败: ${out}`);

  out = await call("execute_action", { room_id: roomId, player_id: "smoke-host", action: "lobby:start" });
  assert(!out.startsWith("错误"), `lobby:start 失败: ${out}`);
  const started = JSON.parse(out);
  assert(started.state.data.phase === "turn_roll", `开赛后阶段异常: ${started.state.data.phase}`);
  assert(started.events?.length > 0, "开赛应产出事件");
  console.log(`[las-vegas] lobby:start OK，round=1，起始玩家 ${started.state.data.start_player}`);

  // 验证公共视图脱敏
  const pub = JSON.parse(await call("get_state", { room_id: roomId }));
  for (const c of pub.data.casinos) {
    assert(c.banknotes.length === 2 && c.banknotes.every((n) => n === 0), "公共视图钞票面额应脱敏为 0");
  }
  console.log("[las-vegas] get_state 脱敏 OK");

  let state = pub;
  let guard = 0;
  let turns = 0;
  while (state.status !== "finished") {
    guard += 1;
    assert(guard < 2000, "回合数异常，疑似死循环");
    const d = state.data;
    const cur = d.seats[d.turn]?.player_id;
    assert(cur, `第 ${guard} 步找不到当前玩家`);
    const legalJson = await call("get_legal_actions", { room_id: roomId, player_id: cur });
    const legal = JSON.parse(legalJson);
    assert(Array.isArray(legal) && legal.length > 0, `玩家 ${cur} 无合法动作（phase=${d.phase}）`);
    // 优先下注；无下注则 reroll
    const place = legal.find((a) => a.id.startsWith("place:"));
    const action = place ? place.id : legal[0].id;
    const res = JSON.parse(
      await call("execute_action", { room_id: roomId, player_id: cur, action }),
    );
    state = res.state;
    turns += 1;
  }
  const d = state.data;
  console.log(
    `[las-vegas] 对局结束：turns=${turns} phase=${d.phase} scores=${JSON.stringify(d.scores)} winners=${d.winners.join(",")}`,
  );
  assert(d.phase === "finished", "终局阶段应为 finished");
  assert(Object.keys(d.scores).length === 2, "应有 2 名玩家计分");
  assert(d.winners.length >= 1, "应产生至少 1 名胜者");

  const active = await call("get_active_game", { group_id: "smoke-group" });
  assert(active === "null", "终局后群内不应有活跃对局");

  console.log("[las-vegas] ✅ MCP 端到端冒烟通过");
} catch (err) {
  console.error(`[las-vegas] ❌ 冒烟失败: ${err.message}`);
  process.exitCode = 1;
} finally {
  await client.close().catch(() => {});
  for (const suffix of ["", "-wal", "-shm"]) {
    try {
      fs.rmSync(dbPath + suffix, { force: true });
    } catch {
      /* ignore */
    }
  }
}
