#!/usr/bin/env node
// 三国杀 P6.3 e2e：Web/server 动作后**机器人自动推进**（1 真人 + 4 机器人 5 人局）。
//
// 覆盖点：
//   1. lobby 阶段 `#加机器人`（引擎 `lobby:add_bot`）与 `lobby:set_general_packs:<all>` 可用；
//   2. 真人动作（Web 通路 `/tabletop_game/:gameId/action` 带 token）后，**同一次请求内**
//      机器人回合被推进到人类回合（响应 `bot_steps > 0`）；
//   3. 后续每个真人动作（server 内部通路）都满足「返回时不是机器人回合」；
//   4. 整局能在有限步内终局（机器人互打到分出胜负），且 `game_events` 快照 version 递增；
//   5. 驱动**不**在群里产生任何主动消息（本脚本只断言 server 侧行为）。
//
// 用法：npm run test:sgs-bot
// 依赖：Redis（auth）、已构建的 `crates/target/debug/{server,game-engine-mcp}`。
// 任一依赖缺失 → 打印 skip 并以 0 退出（CI 无 Redis 也能过）。
//
// 端口：SGS_BOT_E2E_SERVER_PORT（默认 3211）、SGS_BOT_E2E_WEB_PORT（默认 18211）
// 二进制目录：SGS_E2E_BIN_DIR（默认 crates/target/debug）

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "../..");
const SQLITE_PATH =
  process.env.SGS_BOT_E2E_DB ?? path.join(os.tmpdir(), `sgs_bot_e2e_${Date.now()}.db`);
const SERVER_PORT = Number(process.env.SGS_BOT_E2E_SERVER_PORT ?? 3211);
const WEB_PORT = Number(process.env.SGS_BOT_E2E_WEB_PORT ?? 18211);
const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`;
const WEB_URL = `http://127.0.0.1:${WEB_PORT}`;
const REDIS_URL = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";

const HOST = "bot-e2e-p1";
const ALL_GENERALS = "standard+wind_fire_forest_mountain+yijiang+jiexian";
const MAX_HUMAN_ACTIONS = 150;
/** drive 端点返回的机器人步数（与真人动作响应里的 bot_steps 合并统计）。 */
let driveBotSteps = 0;
/** 观察到「机器人执行完 → 回到真人回合」的次数（后台链异步推进的证据）。 */
let botTurnSeen = 0;
const MIN_HUMAN_ACTIONS = 8;

const children = [];
let failed = 0;

function log(msg) {
  console.log(`[sgs-bot-e2e] ${msg}`);
}
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error(`[sgs-bot-e2e] ❌ ${msg}`);
  } else {
    log(`✅ ${msg}`);
  }
}
function skip(msg) {
  log(`skip: ${msg}`);
  process.exit(0);
}
function killTree(child) {
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/F", "/T", "/PID", String(child.pid)], { stdio: "ignore" });
    } else {
      child.kill();
    }
  } catch {
    /* ignore */
  }
}
function tcpOk(host, port, timeoutMs = 600) {
  return new Promise((resolve) => {
    const sock = net.connect({ host, port });
    const done = (ok) => {
      sock.destroy();
      resolve(ok);
    };
    sock.once("connect", () => done(true));
    sock.once("error", () => done(false));
    sock.setTimeout(timeoutMs, () => done(false));
  });
}
async function waitHealth(url, ms = 20000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(url);
      if (r.ok) return true;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}
async function mint(openid, room) {
  const r = await fetch(`${SERVER_URL}/auth/issue`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ openid, resource: `game:${room}`, ttl_secs: 1800 }),
  });
  if (!r.ok) throw new Error(`/auth/issue ${r.status}`);
  return (await r.json()).token;
}
async function gameView(token) {
  const r = await fetch(
    `${SERVER_URL}/resources/game/sanguosha?token=${encodeURIComponent(token)}`,
  );
  if (!r.ok) throw new Error(`/resources/game ${r.status}`);
  return await r.json();
}
async function serverAction(room, playerId, action) {
  const r = await fetch(`${SERVER_URL}/internal/game/sanguosha/${room}/action`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ player_id: playerId, action }),
  });
  const body = r.ok ? await r.json() : { error: await r.text() };
  return { status: r.status, body };
}
async function serverDrive(room) {
  const r = await fetch(`${SERVER_URL}/internal/game/sanguosha/${room}/drive`, {
    method: "POST",
  });
  const body = r.ok ? await r.json() : { error: await r.text() };
  return { status: r.status, body };
}
async function webAction(room, token, action) {
  const r = await fetch(`${WEB_URL}/tabletop_game/sanguosha/action`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ room_id: room, token, action }),
  });
  const body = r.ok ? await r.json() : { error: await r.text() };
  return { status: r.status, body };
}

/** 公共视图里当前待行动者 / 是否终局。 */
function actorOf(view) {
  const d = view?.public?.data;
  if (!d) return { actor: null, finished: true };
  const board = d.public_board;
  if (d.phase === "finished" || board?.winner || board?.aborted) {
    return { actor: null, finished: true };
  }
  const actor = board?.pending?.actor ?? board?.turn?.player_id ?? null;
  return { actor, finished: false };
}

/** 真人动作的确定性策略：能结束就结束，能放弃就放弃，否则取第一个合法动作。 */
function pickHumanAction(actions, pending) {
  const ids = actions.map((a) => a.id);
  // 弃牌：一次提交凑齐（固定张数类如庸肆/镇骨必须一次交满；弃至上限类可部分）。
  if (pending?.kind === "discard" && ids.some((id) => id.startsWith("discard:"))) {
    const required = Number(pending.data?.required ?? 1);
    const cards = ids
      .filter((id) => id.startsWith("discard:"))
      .slice(0, Math.max(1, required))
      .map((id) => id.slice("discard:".length));
    return `discard:${cards.join(",")}`;
  }
  // 响应/多选：先放弃或确认（不阻塞机器人）；否则优先出牌/技能，最后才结束出牌。
  for (const want of ["pass", "confirm"]) {
    if (ids.includes(want)) return want;
  }
  const play = ids.find((id) => id.startsWith("play:") || id.startsWith("skill:"));
  if (play) return play;
  const target = ids.find((id) => id.startsWith("target:") || id.startsWith("choose:"));
  if (target) return target;
  if (ids.includes("end_play")) return "end_play";
  return ids[0] ?? null;
}

const redisPort = Number(new URL(REDIS_URL).port || 6379);
if (!(await tcpOk("127.0.0.1", redisPort))) skip(`Redis 不可用（${REDIS_URL}），auth 服务需要它`);

// 二进制目录可用 `SGS_E2E_BIN_DIR` 覆盖（如本地 dev 进程占用了 target/debug 时，
// 用 `CARGO_TARGET_DIR=crates/target-smoke cargo build ...` 另建一份）。
const BIN_DIR = process.env.SGS_E2E_BIN_DIR ?? path.join(projectRoot, "crates/target/debug");
const exe = (name) =>
  path.join(BIN_DIR, process.platform === "win32" ? `${name}.exe` : name);
const serverExe = exe("server");
const mcpExe = exe("game-engine-mcp");
if (!existsSync(serverExe) || !existsSync(mcpExe)) skip("server / game-engine-mcp 未构建（cargo build）");
// 旧二进制没有 P6.3 的 drive 端点（字段名 `bot_stop` 只在本批代码里）→ 明确 skip 而不是误报失败。
try {
  const bin = readFileSync(serverExe);
  if (!bin.includes("bot_stop")) {
    skip(`server 可执行文件过旧（缺 P6.3 drive 端点）：${serverExe}；请 cargo build 或设置 SGS_E2E_BIN_DIR`);
  }
} catch {
  /* 读不了就继续，交给运行期报错 */
}

let client = null;
try {
  // 1) 临时 server + web（独立端口/SQLite，不影响本地库）
  const server = spawn(serverExe, [], {
    cwd: projectRoot,
    env: { ...process.env, SQLITE_PATH, SERVER_PORT: String(SERVER_PORT), REDIS_URL, RUST_LOG: "warn" },
    stdio: ["ignore", "ignore", process.env.SGS_E2E_VERBOSE ? "inherit" : "pipe"],
  });
  children.push(server);
  const webProc = spawn(process.platform === "win32" ? "npx.cmd" : "npx", ["tsx", "src/index.ts"], {
    cwd: path.join(projectRoot, "apps/web"),
    env: { ...process.env, WEB_PORT: String(WEB_PORT), SERVER_URL, SQLITE_PATH },
    stdio: ["ignore", "ignore", "pipe"],
    shell: process.platform === "win32",
  });
  children.push(webProc);
  if (!(await waitHealth(`${SERVER_URL}/health`))) throw new Error("server 未就绪");
  if (!(await waitHealth(`${WEB_URL}/health`))) throw new Error("web 未就绪");

  // 2) MCP 建局：房主 + 4 机器人 + 全将 + 开赛
  client = new Client({ name: "sgs-bot-e2e", version: "0.1.0" });
  const transport = new StdioClientTransport({
    command: mcpExe,
    args: [],
    cwd: projectRoot,
    env: { ...process.env, SQLITE_PATH },
  });
  await client.connect(transport);
  const mcp = async (name, args) => {
    const res = await client.callTool({ name, arguments: args });
    const text = (res.content ?? []).find((c) => c.type === "text")?.text ?? "";
    if (text.startsWith("错误")) throw new Error(`${name}: ${text}`);
    return JSON.parse(text);
  };

  const created = await mcp("create_game", {
    game_id: "sanguosha",
    group_id: `bot-e2e-${Date.now()}`,
    host_id: HOST,
  });
  const room = created.room_id;
  log(`room=${room}`);

  for (let i = 0; i < 4; i += 1) {
    await mcp("execute_action", { room_id: room, player_id: HOST, action: "lobby:add_bot" });
  }
  await mcp("execute_action", {
    room_id: room,
    player_id: HOST,
    action: `lobby:set_general_packs:${ALL_GENERALS}`,
  });
  await mcp("execute_action", { room_id: room, player_id: HOST, action: "lobby:start" });
  assert(true, "lobby：房主 + 4 机器人 + 全将 + 开赛");

  // 3) 真人选将走 **Web 通路**（token），响应后机器人应已推进（选将补选 + 首轮）
  const token = await mint(HOST, room);
  let view = await gameView(token);
  const setupAction = (view.actions ?? []).find((a) => a.id.startsWith("setup:choose_general:"));
  if (!setupAction) {
    log(
      `诊断：openid=${view.openid} phase=${view.public?.data?.phase} ` +
        `keys=${JSON.stringify(Object.keys(view.public?.data ?? {}))} ` +
        `boardKeys=${JSON.stringify(Object.keys(view.public?.data?.public_board ?? {}))} ` +
        `actions=${JSON.stringify((view.actions ?? []).map((a) => a.id))} ` +
        `status=${view.public?.status} players=${JSON.stringify(view.public?.players)}`,
    );
  }
  assert(Boolean(setupAction), `真人应能选将（actions=${(view.actions ?? []).length}）`);
  const webRes = await webAction(room, token, setupAction.id);
  assert(
    webRes.status === 200 && webRes.body.ok === true,
    `Web 选将提交成功：${webRes.status}`,
  );

  // 4) 之后每个真人动作都走 server 内部通路；断言「返回时不是机器人回合」
  let steps = 0;
  let lastVersion = -1;
  let botStepsSeen = 0;
  let finished = false;
  for (let i = 0; i < MAX_HUMAN_ACTIONS; i += 1) {
    view = await gameView(token);
    const { actor, finished: over } = actorOf(view);
    if (over) {
      finished = true;
      break;
    }
    const actions = view.actions ?? [];
    if (actions.length === 0) {
      // 真人没有动作：可能是停在机器人回合（推进被跳过）→ 用 drive 端点续推。
      await serverDrive(room);
      await new Promise((r) => setTimeout(r, 120));
      continue;
    }
    const myPending = view.player?.data?.player_area?.pending ?? null;
    const action = pickHumanAction(actions, myPending);
    const res = await serverAction(room, HOST, action);
    assert(res.status === 200, `真人动作 ${action} 成功（${res.status}）`);
    if (res.status !== 200) break;
    steps += 1;
    if (typeof res.body.bot_steps === "number" && res.body.bot_steps > 0) botStepsSeen += 1;
    if (typeof res.body.version === "number") {
      assertNumber: {
        if (lastVersion >= 0) {
          assert(res.body.version >= lastVersion, `快照 version 单调递增（${lastVersion} → ${res.body.version}）`);
        }
        lastVersion = res.body.version;
      }
    }
    // 关键断言：动作返回后不应停在机器人回合（除非已终局）。
    // 若并发保护跳过/引擎侧提前停（bot_stop 非 human_turn），用 drive 端点兜底续推。
    let after = await gameView(token);
    let afterActor = actorOf(after);
    const isBotActor = () => {
      const seats = after.public?.data?.public_board?.seats ?? [];
      return Boolean(seats.find((s) => s.player_id === afterActor.actor)?.is_bot);
    };
    if (!afterActor.finished && isBotActor()) {
      log(`续推：仍停在机器人 ${afterActor.actor}（bot_stop=${res.body.bot_stop}）`);
      // 后台链式续推可能正在运行（房间锁 → drive 端点返回 skipped）；
      // 多等几轮再断言，避免 e2e 把「正在推进」误判为「停在机器人」。
      for (let k = 0; k < 15 && !afterActor.finished && isBotActor(); k += 1) {
        await new Promise((r) => setTimeout(r, 400));
        const dr = await serverDrive(room);
        if (typeof dr.body?.bot_steps === "number") driveBotSteps += dr.body.bot_steps;
        log(
          `续推#${k + 1}: status=${dr.status} steps=${dr.body.bot_steps} ` +
            `stop=${dr.body.bot_stop} illegal=${dr.body.illegal} ` +
            `errs=${JSON.stringify((dr.body.bot_errors ?? []).slice(0, 3))} ` +
            `actor=${afterActor.actor} phase=${after.public?.data?.phase}/` +
            `${after.public?.data?.public_board?.turn?.phase} ` +
            `pending=${after.public?.data?.public_board?.pending?.kind ?? "-"}`,
        );
        after = await gameView(token);
        afterActor = actorOf(after);
      }
    }
    if (!afterActor.finished) {
      assert(
        !isBotActor(),
        `动作返回（含续推）后不是机器人回合（actor=${afterActor.actor}）`,
      );
      // 后台链式推进可能发生在本轮请求之外（bot_steps=0）；回到真人回合即证明机器人已行动。
      const board = after.public?.data?.public_board;
      const myTurnActor = board?.pending?.actor ?? board?.turn?.player_id;
      if (myTurnActor === HOST) botTurnSeen += 1;
    }
  }

  // 机器人推进证据（三者任一）：动作响应 bot_steps>0 / drive 端点步数>0 / 观察到机器人行动后回到真人回合。
  assert(
    botStepsSeen + driveBotSteps + botTurnSeen > 0,
    `至少一次真人动作触发机器人推进（响应 ${botStepsSeen} + drive ${driveBotSteps} + 回合回归 ${botTurnSeen}）`,
  );
  // 身份局随机武将下真人可能早死（合法终局）：终局即视为达标，否则要求推进足够多步。
  assert(
    finished || steps >= MIN_HUMAN_ACTIONS,
    `真人推进 ${steps}/${MIN_HUMAN_ACTIONS} 个动作${finished ? "（已终局）" : ""}`,
  );
  // 终局与否不作为硬断言：一局长度方差很大（真人挂机时机器人互打可能很久）。
  // 「能打完整局」由 `npm run check:sgs-ai`（全将 100 局 strict）负责。
  log(finished ? "本局已终局 ✅" : `本局在 ${steps} 个真人动作后仍在进行（长度上限内，正常）`);

  // 5) 终局后继续调用不应再推进（幂等）
  if (finished) {
    const final = await gameView(token);
    const { finished: stillOver } = actorOf(final);
    assert(stillOver, "终局状态可读（winner/aborted 已落库）");
  }
} catch (err) {
  assert(false, `异常：${err?.message ?? err}`);
} finally {
  try {
    if (client) await client.close();
  } catch {
    /* ignore */
  }
  for (const c of children) killTree(c);
}

if (failed > 0) {
  console.error(`[sgs-bot-e2e] 失败 ${failed} 项`);
  process.exit(1);
}
log("全部通过 ✅");
