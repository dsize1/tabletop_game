// 三国杀 MCP 端到端冒烟（S3：v2 接管 `game_id=sanguosha` 后的最小链路）。
//
// 覆盖：
//   create_game → lobby（`#加入`）→ 武将包菜单 → `#开始` → 选将 → 发牌
//   → 三分区视图（公共/个人/守恒）→ 摘要 → 出牌循环（use/skill/end_play + pending）
//   → 终局或中断 → 群内释放；8 人局 lobby 快速校验。
//
// 说明：v2 用 `use:*`（v1 是 `play:*`），特殊交互区（五谷/观星）在 S4 之前用 pending 表达，
// 因此本脚本不再断言 `special_zones`；内核契约由 `npm run check:sgs2` 与视图契约覆盖。
//
// 用法：node apps/verify-sanguosha.mjs [exe路径]
//   环境变量：GAME_ENGINE_MCP / SQLITE_PATH

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");

function defaultExe() {
  const exe = process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp";
  return path.resolve(projectRoot, "crates/target/debug", exe);
}

const exe = process.argv[2] ?? process.env.GAME_ENGINE_MCP ?? defaultExe();
const dbPath = process.env.SQLITE_PATH ?? path.join(os.tmpdir(), `tt_sgs_smoke_${Date.now()}.db`);
const GROUP = `sgs-smoke-${Date.now()}`;
const HOST = "sgs-host";
const PLAYERS = [HOST, "sgs-p2", "sgs-p3", "sgs-p4", "sgs-p5"];
let lastAction = "";

function assert(cond, msg) {
  if (!cond) throw new Error(`断言失败：${msg}`);
}

/** 牌堆总张数：v2 config.packs 是字符串（`standard` / `standard+junzheng`）。 */
function deckTotal(board) {
  const packs = board.config?.packs ?? "standard+junzheng";
  const text = Array.isArray(packs) ? packs.join("+") : String(packs);
  return text.includes("junzheng") ? 160 : 108;
}

function seatConservation(board) {
  const hands = board.seats.reduce((n, s) => n + s.hand_count, 0);
  const equips = board.seats.reduce(
    (n, s) => n + Object.values(s.equips ?? {}).filter(Boolean).length,
    0,
  );
  const judge = board.seats.reduce((n, s) => n + (s.judge_zone?.length ?? 0), 0);
  const marks = board.seats.reduce(
    (n, s) => n + Object.values(s.marks ?? {}).reduce((m, v) => m + v.length, 0),
    0,
  );
  const deck = board.deck;
  return {
    hands,
    equips,
    judge,
    marks,
    total: deck.draw + deck.discard + deck.processing + deck.void + hands + equips + judge + marks,
  };
}

const client = new Client({ name: "verify-sanguosha", version: "0.1.0" });
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

async function callJson(name, args) {
  const raw = await call(name, args);
  assert(!raw.startsWith("错误"), `${name} 失败: ${raw}`);
  return JSON.parse(raw);
}

/** 选将：轮到谁就选第一个候选。 */
async function pickGenerals(roomId, players) {
  let picked = 0;
  for (let guard = 0; guard < 40; guard += 1) {
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
  return picked;
}

/** 推进 pending，直到出牌阶段 / 终局（用于准备阶段技能）。 */
async function drainPending(roomId, maxSteps = 60) {
  for (let i = 0; i < maxSteps; i += 1) {
    const view = await callJson("get_state", { room_id: roomId });
    const b = view.data?.public_board;
    if (!b?.pending) return view;
    const actor = b.pending.actor;
    const legal = await callJson("get_legal_actions", { room_id: roomId, player_id: actor });
    assert(legal.length > 0, `pending ${b.pending.kind} 无合法动作`);
    const pick =
      legal.find((a) => a.id === "confirm") ??
      legal.find((a) => a.id.startsWith("respond:")) ??
      legal.find((a) => a.id === "pass") ??
      legal[0];
    lastAction = `${actor}:${pick.id} (pending ${b.pending.kind})`;
    await callJson("execute_action", { room_id: roomId, player_id: actor, action: pick.id });
  }
  return callJson("get_state", { room_id: roomId });
}

try {
  await client.connect(transport);
  console.log(`[sgs] 已连接 MCP: ${exe}`);

  const tools = (await client.listTools()).tools.map((t) => t.name);
  for (const t of [
    "create_game",
    "get_state",
    "get_player_view",
    "get_legal_actions",
    "execute_action",
    "state_summary",
  ]) {
    assert(tools.includes(t), `缺少 MCP 原语 ${t}`);
  }

  // —— 建局 + 加入 ——
  const created = await callJson("create_game", {
    game_id: "sanguosha",
    group_id: GROUP,
    host_id: HOST,
  });
  assert(created.room_id, "create_game 未返回 room_id");
  assert(created.game_id === "sanguosha", `引擎 id 应为 sanguosha，实际 ${created.game_id}`);
  assert(created.data.lobby, "建局后应停在大厅（data.lobby 非空）");
  const roomId = created.room_id;
  console.log(`[sgs] create_game room=${roomId}`);

  for (const p of PLAYERS.slice(1)) {
    await callJson("execute_action", { room_id: roomId, player_id: p, action: "lobby:join" });
  }
  let view = await callJson("get_state", { room_id: roomId });
  assert(view.data.phase === "lobby", `大厅阶段应为 lobby，实际 ${view.data.phase}`);
  assert(view.data.public_board.seat_count === 5, "应为 5 人局");
  console.log("[sgs] lobby 5 人 OK");

  // —— 武将包（预设来自引擎 lobby 合法动作）——
  const lobbyActs = await callJson("get_legal_actions", { room_id: roomId, player_id: HOST });
  const packIds = lobbyActs
    .map((a) => a.id)
    .filter((id) => id.startsWith("lobby:set_general_packs:"));
  assert(packIds.length >= 5, `大厅应给出多个武将包预设，实际 ${packIds.length}`);
  await callJson("execute_action", {
    room_id: roomId,
    player_id: HOST,
    action: "lobby:set_general_packs:standard+wind_fire_forest_mountain",
  });
  view = await callJson("get_state", { room_id: roomId });
  assert(
    String(view.data.public_board.config.general_packs).includes("wind_fire_forest_mountain"),
    "武将包开关未写入 config.general_packs",
  );
  // 恢复默认（全部包），后续流程用更大的武将池。
  await callJson("execute_action", {
    room_id: roomId,
    player_id: HOST,
    action: "lobby:set_general_packs:standard+wind_fire_forest_mountain+yijiang+jiexian",
  });
  console.log(`[sgs] 武将包预设 ${packIds.length} 个 OK`);

  // —— 开赛（发身份 → 主公候选）——
  const started = await callJson("execute_action", {
    room_id: roomId,
    player_id: HOST,
    action: "lobby:start",
  });
  assert(started.state.data.setup, "开赛后应进入选将（data.setup 非空）");
  view = await callJson("get_state", { room_id: roomId });
  const setupSeats = view.data.public_board.seats;
  assert(
    setupSeats.filter((s) => s.identity === "lord").length === 1,
    "应公开且仅公开一名主公",
  );
  assert(
    setupSeats.filter((s) => s.general).length === 0,
    "选将期间不得公开任何武将",
  );
  assert(!JSON.stringify(view).includes("candidate_details"), "公共视图不得包含候选武将详情");
  assert(
    Number.isInteger(started.state.data.setup.lord),
    "状态应记录主公座位下标",
  );
  console.log(`[sgs] lobby:start OK，主公座位=${started.state.data.setup.lord}`);

  // —— 选将 + 推进准备阶段技能 ——
  const picked = await pickGenerals(roomId, PLAYERS);
  assert(picked === 5, `应完成 5 人选将，实际 ${picked}`);
  view = await drainPending(roomId);
  const board = view.data.public_board;
  assert(view.data.phase !== "setup", `选将后不应仍在 setup，实际 ${view.data.phase}`);
  if (view.data.phase !== "finished") {
    assert(board.turn.phase === "play", `首个回合应推进到 play，实际 ${board.turn.phase}`);
    assert(board.turn.player_id, "应有当前回合玩家");
  }
  for (const s of board.seats) {
    assert(typeof s.general === "string" && s.general.length > 0, "选将后武将应公开");
    assert(s.hand === undefined, "公共视图不得包含手牌明细");
    if (s.identity !== "lord") assert(!s.identity, "非主公身份不得公开");
  }
  const cons = seatConservation(board);
  assert(cons.total === deckTotal(board), `牌数守恒失败：${JSON.stringify(cons)}`);
  console.log(
    `[sgs] 发牌 OK：总张数 ${deckTotal(board)}，牌堆余 ${board.deck.draw}，手牌 ${cons.hands}`,
  );

  // —— 个人视图 ——
  const pv = await callJson("get_player_view", { room_id: roomId, player_id: HOST });
  const area = pv.data.player_area;
  assert(area && area.player_id === HOST, "个人视图缺少 player_area");
  const me = board.seats.find((s) => s.player_id === HOST);
  assert(area.hand.length === me.hand_count, "个人视图手牌数应与公共视图一致");
  assert(
    area.hand.every((c) => c.name && c.suit && c.rank >= 1),
    "手牌应含牌名/花色/点数",
  );
  console.log(`[sgs] 个人视图 OK：手牌 ${area.hand.map((c) => c.name).join("/")}`);

  const summary = await call("state_summary", { room_id: roomId });
  assert(summary.includes("三国杀"), `摘要异常: ${summary}`);
  console.log(`[sgs] 摘要：${summary}`);

  // —— 出牌循环（v2：use / skill / end_play；pending 收口） ——
  const beforeTurn = board.turn_index ?? 0;
  let steps = 0;
  let sawUse = false;
  let sawSkill = false;
  let sawPending = false;
  let finished = view.data.phase === "finished";
  while (steps < 400 && !finished) {
    const data = (await callJson("get_state", { room_id: roomId })).data;
    if (data.phase === "finished") {
      finished = true;
      break;
    }
    const b = data.public_board;
    if (b.pending) {
      sawPending = true;
      const legal = await callJson("get_legal_actions", {
        room_id: roomId,
        player_id: b.pending.actor,
      });
      assert(legal.length > 0, `pending ${b.pending.kind} 无合法动作`);
      const pick =
        legal.find((a) => a.id === "confirm") ??
        legal.find((a) => a.id.startsWith("respond:")) ??
        legal.find((a) => a.id === "pass") ??
        legal[0];
      lastAction = `${b.pending.actor}:${pick.id} (pending ${b.pending.kind})`;
      await callJson("execute_action", {
        room_id: roomId,
        player_id: b.pending.actor,
        action: pick.id,
      });
    } else {
      assert(b.turn.phase === "play", `不应停留在 ${b.turn.phase} 阶段`);
      const legal = await callJson("get_legal_actions", {
        room_id: roomId,
        player_id: b.turn.player_id,
      });
      assert(legal.length > 0, "出牌阶段无合法动作");
      const use = legal.find((a) => a.id.startsWith("use:"));
      const skill = legal.find((a) => a.id.startsWith("skill:"));
      const pick = use ?? skill ?? legal.find((a) => a.id === "end_play");
      assert(pick, "既没有可出的牌也没有 end_play");
      if (pick.id.startsWith("use:")) sawUse = true;
      if (pick.id.startsWith("skill:")) sawSkill = true;
      lastAction = `${b.turn.player_id}:${pick.id} (play)`;
      try {
        await callJson("execute_action", {
          room_id: roomId,
          player_id: b.turn.player_id,
          action: pick.id,
        });
      } catch (err) {
        const retry = await callJson("get_legal_actions", {
          room_id: roomId,
          player_id: b.turn.player_id,
        });
        const fallback = retry.find((a) => a.id === "end_play");
        if (!fallback) throw err;
        await callJson("execute_action", {
          room_id: roomId,
          player_id: b.turn.player_id,
          action: fallback.id,
        });
      }
    }
    steps += 1;
    if (steps % 40 === 0) {
      const v = (await callJson("get_state", { room_id: roomId })).data.public_board;
      const c = seatConservation(v);
      assert(c.total === deckTotal(v), `第 ${steps} 步牌数不守恒：${JSON.stringify(c)}`);
    }
  }

  const after = (await callJson("get_state", { room_id: roomId })).data;
  const boardAfter = after.public_board;
  assert(steps > 0, "应至少执行一步");
  assert(boardAfter.turn_index >= beforeTurn, "回合序号不应倒退");
  assert(boardAfter.history.length > 0, "应有出牌历史");
  if (finished) {
    assert(after.phase === "finished", "终局阶段应为 finished");
    assert(boardAfter.winner && boardAfter.winner.team, "终局应有胜方");
    console.log(`[sgs] 终局：winner=${boardAfter.winner.team}，共 ${steps} 步`);
  } else {
    console.log(`[sgs] 400 步内未终局（可接受）：${steps} 步`);
  }
  console.log(`[sgs] 结算冒烟 OK：use=${sawUse} skill=${sawSkill} pending=${sawPending}`);

  // —— 收尾：未终局则中断 ——
  if (!finished) {
    const aborted = await callJson("execute_action", {
      room_id: roomId,
      player_id: HOST,
      action: "lobby:abort",
    });
    assert(aborted.finished, "中断后应 finished");
  }
  const active = await callJson("get_active_game", { group_id: GROUP });
  assert(active === null, "终局/中断后群内不应有活跃对局");

  // —— 8 人局：大厅 / 双内奸开关 / 选将 / 发牌 ——
  const GROUP8 = `sgs-smoke8-${Date.now()}`;
  const PLAYERS8 = ["sgs8-host", ...Array.from({ length: 7 }, (_, i) => `sgs8-p${i + 2}`)];
  const c8 = await callJson("create_game", {
    game_id: "sanguosha",
    group_id: GROUP8,
    host_id: PLAYERS8[0],
  });
  const room8 = c8.room_id;
  for (const p of PLAYERS8.slice(1)) {
    await callJson("execute_action", { room_id: room8, player_id: p, action: "lobby:join" });
  }
  const acts8 = await callJson("get_legal_actions", {
    room_id: room8,
    player_id: PLAYERS8[0],
  });
  assert(
    acts8.some((a) => a.id === "lobby:set_double_spy:1"),
    "8 人局大厅应提供双内奸开关",
  );
  await callJson("execute_action", {
    room_id: room8,
    player_id: PLAYERS8[0],
    action: "lobby:start",
  });
  const picked8 = await pickGenerals(room8, PLAYERS8);
  assert(picked8 === 8, `8 人局应完成 8 人选将，实际 ${picked8}`);
  const view8 = await drainPending(room8);
  const b8 = view8.data.public_board;
  const c8cons = seatConservation(b8);
  assert(c8cons.total === deckTotal(b8), `8 人局牌数不守恒：${JSON.stringify(c8cons)}`);
  console.log(`[sgs] 8 人局 OK：座位 ${b8.seat_count}，手牌 ${c8cons.hands}`);

  console.log(`[sgs] 全部冒烟通过（最后动作：${lastAction}）`);
} finally {
  await client.close().catch(() => {});
}
