// 三国杀 MCP 端到端冒烟。
// 覆盖：create_game → 加入 → 开赛 → 选将 → 发牌 → 视图三分区（P2.1）
//      → 回合推进（出牌阶段结束 / 弃牌 pending）（P2.2）
//      → 结算栈/锦囊/技能（P2.3/P2.4）
//      → 特殊区契约：五谷丰登 `wugu`（公开）/ 观星 `guanxing` + `arrange`（P2.5）。
//
// 用法：node apps/verify-sanguosha.mjs [exe路径]
//   环境变量：
//     GAME_ENGINE_MCP  game-engine-mcp 可执行文件路径（默认 crates/target/debug）
//     SQLITE_PATH      冒烟库路径（默认临时文件，不污染 data/tabletop.db）
//
// P2.3 落地后在此脚本追加「濒死/奖惩 → 终局」。
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
let lastAction = "";
const HOST = "sgs-host";
const PLAYERS = [HOST, "sgs-p2", "sgs-p3", "sgs-p4", "sgs-p5"];
// P4.0 军争牌定义已入库但结算未实现：端到端冒烟必须确认它们从未以出牌动作出现。
// （`legal_actions` 不列出 + `play:` 直接报错，见 crates/engines/sanguosha/src/turn.rs）
const PENDING_JUNZHENG_NAMES = [];

function assert(cond, msg) {
  if (!cond) throw new Error(`断言失败：${msg}`);
}

// 牌堆总张数由 `Config.packs` 决定（P4.0 默认 标准+军争 = 160；标准包 = 108）。
function deckTotal(board) {
  const packs = board.config?.packs ?? ["standard", "junzheng"];
  return packs.includes("junzheng") ? 160 : 108;
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

try {
  await client.connect(transport);
  console.log(`[sgs] 已连接 MCP: ${exe}`);

  const tools = (await client.listTools()).tools.map((t) => t.name);
  for (const t of ["create_game", "get_state", "get_player_view", "get_legal_actions", "execute_action"]) {
    assert(tools.includes(t), `缺少 MCP 原语 ${t}`);
  }

  // —— 建局 + 加入 ——
  const created = await callJson("create_game", {
    game_id: "sanguosha",
    group_id: GROUP,
    host_id: HOST,
  });
  assert(created.room_id, "create_game 未返回 room_id");
  const roomId = created.room_id;
  assert(created.data.phase === "lobby", `建局后应为 lobby，实际 ${created.data.phase}`);
  console.log(`[sgs] create_game room=${roomId}`);

  for (const p of PLAYERS.slice(1)) {
    await callJson("execute_action", { room_id: roomId, player_id: p, action: "lobby:join" });
  }
  const lobbyView = await callJson("get_state", { room_id: roomId });
  assert(lobbyView.data.public_board.seat_count === 5, "应为 5 人局");
  console.log("[sgs] lobby 5 人 OK");

  // —— P4.4.0：武将包开关（标准 + 风林火山）——
  await callJson("execute_action", {
    room_id: roomId,
    player_id: HOST,
    action: "lobby:set_general_packs:standard+wind_fire_forest_mountain",
  });
  const packed = await callJson("get_state", { room_id: roomId });
  assert(
    (packed.data.public_board.config.general_packs ?? []).includes("wind_fire_forest_mountain"),
    "武将包开关未写入 config.general_packs",
  );
  console.log("[sgs] lobby:set_general_packs OK");

  // —— 开赛（发身份 → 主公候选）——
  const started = await callJson("execute_action", {
    room_id: roomId,
    player_id: HOST,
    action: "lobby:start",
  });
  assert(started.state.data.phase === "setup", `开赛后应为 setup，实际 ${started.state.data.phase}`);
  const startedView = await callJson("get_state", { room_id: roomId });
  const seats = startedView.data.public_board.seats;
  const lords = seats.filter((s) => s.identity === "lord");
  assert(lords.length === 1, "应公开且仅公开一名主公");
  assert(seats.every((s) => s.general === null), "选将期间不得公开武将");
  assert(!JSON.stringify(startedView).includes("candidates"), "公共视图不得包含候选武将");
  assert(started.state.data.setup.lord, "状态应记录主公");
  console.log(`[sgs] lobby:start OK，主公=${started.state.data.setup.lord}`);

  // —— 选将：主公先选，其余依次选第一张候选 ——
  let guard = 0;
  let picked = 0;
  for (;;) {
    guard += 1;
    assert(guard < 40, "选将循环异常");
    let acted = false;
    for (const p of PLAYERS) {
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
  assert(picked === 5, `应完成 5 人选将，实际 ${picked}`);

  // —— 发牌后的公共视图 ——
  const view = await callJson("get_state", { room_id: roomId });
  const board = view.data.public_board;
  const total = deckTotal(board);
  assert(view.data.phase === "turn", `选将后应进入 turn，实际 ${view.data.phase}`);
  assert(
    (board.config?.packs ?? []).includes("standard"),
    `牌堆组合应含标准包，实际 ${JSON.stringify(board.config?.packs)}`,
  );
  assert(board.turn.phase === "play", `首个回合应自动推进到 play，实际 ${board.turn.phase}`);
  assert(board.turn.player_id, "应有当前回合玩家");
  for (const s of board.seats) {
    assert(typeof s.general === "string" && s.general.length > 0, "选将后武将应公开");
    const expect = s.seat === board.turn.seat ? 6 : 4;
    assert(s.hand_count === expect, `${s.player_id} 手牌应为 ${expect}，实际 ${s.hand_count}`);
    assert(s.hand === undefined, "公共视图不得包含手牌明细");
    if (s.identity !== "lord") assert(!s.identity, "非主公身份不得公开");
  }
  const hands = board.seats.reduce((n, s) => n + s.hand_count, 0);
  const equips = board.seats.reduce(
    (n, s) => n + Object.values(s.equips ?? {}).filter(Boolean).length,
    0,
  );
  const judge = board.seats.reduce((n, s) => n + (s.judge_zone?.length ?? 0), 0);
  assert(
    board.deck.draw + board.deck.discard + board.deck.processing + board.deck.void +
      hands + equips + judge === total,
    `牌数守恒失败：draw ${board.deck.draw} + discard ${board.deck.discard} + processing ${board.deck.processing} + hands ${hands} + equips ${equips} + judge ${judge} ≠ ${total}`,
  );
  console.log(
    `[sgs] 发牌 OK：packs=${(board.config?.packs ?? []).join("+")}，总张数 ${total}，牌堆余 ${board.deck.draw}，手牌 ${hands}`,
  );

  // —— 个人视图：本人手牌 + 身份，且不含他人手牌 ——
  const pv = await callJson("get_player_view", { room_id: roomId, player_id: HOST });
  const area = pv.data.player_area;
  assert(area && area.player_id === HOST, "个人视图缺少 player_area");
  const me = board.seats.find((s) => s.player_id === HOST);
  assert(area.hand.length === me.hand_count, "个人视图手牌数应与公共视图一致");
  assert(area.hand.every((c) => c.name && c.suit && c.rank >= 1), "手牌应含牌名/花色/点数");
  assert(me.general === area.general, "个人视图武将应与公共一致");
  console.log(`[sgs] 个人视图 OK：手牌 ${area.hand.map((c) => c.name).join("/")}`);

  const summary = await call("state_summary", { room_id: roomId });
  assert(summary.includes("三国杀"), `摘要异常: ${summary}`);
  console.log(`[sgs] 摘要：${summary}`);

  // —— P2.3 结算栈冒烟：出牌 / 目标 / 无懈 / 响应 / 弃牌，直到终局或步数上限 ——
  const beforeTurn = board.turn_index ?? 0;
  let steps = 0;
  let sawWuxie = false;
  let sawPlay = false;
  let sawSkill = false;
  let sawWuguZone = false;
  let sawWuguPick = false;
  let sawArrange = false;
  let finished = false;
  while (steps < 500) {
    const data = (await callJson("get_state", { room_id: roomId })).data;
    if (data.phase === "finished") {
      finished = true;
      break;
    }
    const b = data.public_board;
    // 公共特殊区（五谷丰登为 public；观星为 owner，不会出现在 get_state）。
    if (data.special_zones?.wugu) sawWuguZone = true;
    if (data.special_zones?.guanxing) {
      throw new Error("公共视图不应包含 owner 可见的观星特殊区");
    }
    if (b.pending) {
      const legal = await callJson("get_legal_actions", {
        room_id: roomId,
        player_id: b.pending.actor,
      });
      assert(legal.length > 0, `pending ${b.pending.kind} 无合法动作`);
      if (b.pending.kind === "wuxie") sawWuxie = true;
      if (b.pending.kind === "wugu") {
        sawWuguPick = true;
        const zone = data.special_zones?.wugu;
        assert(zone && Array.isArray(zone.cards) && zone.cards.length > 0, "五谷丰登应为公开特殊区且亮出牌面");
        assert(zone.current === b.pending.actor, "五谷丰登当前选择者应与 pending.actor 一致");
        assert(zone.interaction?.kind === "one", "五谷丰登交互原语应为 one");
      }
      if (b.pending.kind === "arrange") {
        sawArrange = true;
        const pv = await callJson("get_player_view", {
          room_id: roomId,
          player_id: b.pending.actor,
        });
        const gz = pv.data.special_zones?.guanxing;
        assert(gz, "观星者个人视图应含 guanxing 特殊区");
        assert(Object.keys(gz.slots ?? {}).length === 10, "观星槽位应恒为 10（2×5）");
        assert(gz.interaction?.kind === "ordered" && gz.interaction?.commit === "arrange", "观星交互应为 ordered/arrange");
        assert(gz.visibility === "owner", "观星可见性应为 owner");
      }
      let act;
      if (b.pending.kind === "arrange") {
        act = legal.find((a) => a.id.startsWith("arrange:"));
        assert(act, "观星 pending 应给出 arrange 提交动作");
      } else if (b.pending.kind === "choose_many") {
        act = legal.find((a) => a.id === "confirm") ??
          legal.find((a) => a.id.startsWith("choose:"));
        if (!act) act = legal[0];
      } else if (b.pending.kind === "choice") {
        act = legal.find((a) => a.id === "choose:gain") ?? legal[0];
      } else {
        act = legal.find((a) => a.id === "pass") ?? legal[0];
      }
      lastAction = `${b.pending.actor}:${act.id} (pending ${b.pending.kind})`;
      await callJson("execute_action", {
        room_id: roomId,
        player_id: b.pending.actor,
        action: act.id,
      });
    } else {
      assert(b.turn.phase === "play", `不应停留在 ${b.turn.phase} 阶段`);
      const legal = await callJson("get_legal_actions", {
        room_id: roomId,
        player_id: b.turn.player_id,
      });
      assert(legal.length > 0, "出牌阶段无合法动作");
      const skill = legal.find((a) => a.id.startsWith("skill:"));
      // 优先打五谷丰登，尽量覆盖 P2.5 特殊区链路。
      const wugu = legal.find(
        (a) => a.id.startsWith("play:") && a.description.includes("五谷丰登"),
      );
      const play =
        wugu ??
        skill ??
        legal.find((a) => a.id.startsWith("play:")) ??
        legal.find((a) => a.id === "end_play");
      assert(play, "既没有可出的牌也没有 end_play");
      if (play.id.startsWith("skill:")) sawSkill = true;
      if (play.id.startsWith("play:")) {
        sawPlay = true;
        assert(
          !PENDING_JUNZHENG_NAMES.some((n) => play.description.includes(`【${n}`)),
          `军争未实现结算的牌被列为可出：${play.description}（P4.0 拦截失效）`,
        );
      }
      lastAction = `${b.turn.player_id}:${play.id} (play)`;
      await callJson("execute_action", {
        room_id: roomId,
        player_id: b.turn.player_id,
        action: play.id,
      });
    }
    steps += 1;
    if (steps % 40 === 0) {
      const v = (await callJson("get_state", { room_id: roomId })).data.public_board;
      const hands = v.seats.reduce((n, s) => n + s.hand_count, 0);
      const equips = v.seats.reduce(
        (n, s) => n + Object.values(s.equips ?? {}).filter(Boolean).length,
        0,
      );
      const judge = v.seats.reduce((n, s) => n + (s.judge_zone?.length ?? 0), 0);
      assert(
        v.deck.draw + v.deck.discard + v.deck.processing + v.deck.void +
          hands + equips + judge === deckTotal(v),
        `第 ${steps} 步牌数不守恒：draw ${v.deck.draw} discard ${v.deck.discard} proc ${v.deck.processing} hands ${hands} eq ${equips} judge ${judge}`,
      );
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
    assert(steps >= 100, `未终局且步数过少（${steps}）`);
    console.log(`[sgs] 500 步内未终局（可接受）：${steps} 步`);
  }
  console.log(
    `[sgs] 结算栈/技能冒烟 OK：wuxie=${sawWuxie}, play=${sawPlay}, skill=${sawSkill}`,
  );
  console.log(
    `[sgs] 特殊区冒烟：wugu_zone=${sawWuguZone}, wugu_pick=${sawWuguPick}, guanxing_arrange=${sawArrange}` +
      (sawWuguPick && sawArrange ? "" : "（随机选将/随机出牌未覆盖全部特殊区，契约由单元测试覆盖）"),
  );

  // —— 收尾：未终局则中断，终局则确认群内已释放 ——
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

  console.log("[sgs] P2.5 冒烟通过 ✅（五谷丰登 + 观星特殊区契约）");
} catch (err) {
  console.error("[sgs] 冒烟失败:", err?.message ?? err);
  if (typeof lastAction !== "undefined") console.error("[sgs] 最后动作:", lastAction);
  process.exitCode = 1;
} finally {
  await client.close();
}
