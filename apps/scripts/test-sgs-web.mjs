// 三国杀 Web 端 e2e（harness）：起临时 server+web（独立端口/SQLite）→ MCP 建局 →
// 系统 Edge（playwright-core，无需下载浏览器）跑布局/隐藏信息/动作提交断言。
//
// 用法：npm run test:sgs-web
// 依赖：Redis（auth 服务）、已构建的 server / game-engine-mcp、系统 Edge/Chrome。
// 任一依赖缺失 → 打印 skip 并以 0 退出（便于 CI 无浏览器/无 Redis 环境）。
//
// 断言范围（P3）：
// - 四档视口无横向溢出、无座位裁切；公共区无手牌正面；文本态默认隐藏
// - /resources/game 对 sanguosha 只返回投影（public.player_area=null、seat 无 hand）
// - 越权：token 作用域不符 → POST action 403
// - 无 JS 表单：urlencoded POST → 303
// - 交互：点击「结束出牌」（必要时逐张弃牌）→ 回合推进 + 快照 version 递增

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "../..");
const SQLITE_PATH = process.env.SGS_WEB_E2E_DB ?? path.join(os.tmpdir(), `sgs_web_e2e_${Date.now()}.db`);
const SERVER_PORT = Number(process.env.SGS_WEB_E2E_SERVER_PORT ?? 3199);
const WEB_PORT = Number(process.env.SGS_WEB_E2E_WEB_PORT ?? 18199);
const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`;
const WEB_URL = `http://127.0.0.1:${WEB_PORT}`;
const REDIS_URL = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";

const players = ["e2e-p1", "e2e-p2", "e2e-p3", "e2e-p4", "e2e-p5"];
const children = [];
let failed = 0;

function log(msg) {
  console.log(`[sgs-web-e2e] ${msg}`);
}
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error(`[sgs-web-e2e] ❌ ${msg}`);
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

function edgePath() {
  const candidates = {
    win32: [
      "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
      "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
      "C:/Program Files/Google/Chrome/Application/chrome.exe",
    ],
    darwin: [
      "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ],
    linux: ["/usr/bin/microsoft-edge", "/usr/bin/google-chrome", "/usr/bin/chromium"],
  }[process.platform] ?? [];
  return candidates.find((p) => existsSync(p)) ?? null;
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

async function view(token) {
  const r = await fetch(`${SERVER_URL}/resources/game/sanguosha?token=${encodeURIComponent(token)}`);
  if (!r.ok) throw new Error(`/resources/game ${r.status}`);
  return await r.json();
}

const redisPort = Number(new URL(REDIS_URL).port || 6379);
if (!(await tcpOk("127.0.0.1", redisPort))) skip(`Redis 不可用（${REDIS_URL}），auth 服务需要它`);

const serverExe = path.join(projectRoot, "crates/target/debug", process.platform === "win32" ? "server.exe" : "server");
const mcpExe = path.join(projectRoot, "crates/target/debug", process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp");
if (!existsSync(serverExe) || !existsSync(mcpExe)) skip("server / game-engine-mcp 未构建（cargo build）");

let playwright = null;
try {
  playwright = await import("playwright-core");
} catch {
  skip("未安装 playwright-core（npm --prefix apps i -D playwright-core）");
}
const browserPath = edgePath();
if (!browserPath) skip("未找到系统 Edge/Chrome（本脚本不下载浏览器）");

let browser = null;
try {
  // 1) 起临时 server + web
  const server = spawn(serverExe, [], {
    cwd: projectRoot,
    env: {
      ...process.env,
      SQLITE_PATH,
      SERVER_PORT: String(SERVER_PORT),
      REDIS_URL,
      RUST_LOG: "warn",
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  children.push(server);
  const web = spawn(
    process.platform === "win32" ? "npx.cmd" : "npx",
    ["tsx", "src/index.ts"],
    {
      cwd: path.join(projectRoot, "apps/web"),
      env: { ...process.env, WEB_PORT: String(WEB_PORT), SERVER_URL, SQLITE_PATH },
      stdio: ["ignore", "ignore", "pipe"],
      shell: process.platform === "win32",
    },
  );
  children.push(web);
  if (!(await waitHealth(`${SERVER_URL}/health`))) throw new Error("server 未就绪");
  if (!(await waitHealth(`${WEB_URL}/health`))) throw new Error("web 未就绪");

  // 2) MCP 建局 + 选将
  const client = new Client({ name: "sgs-web-e2e", version: "0.1.0" });
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
  const created = await mcp("create_game", { game_id: "sanguosha", group_id: `e2e-${Date.now()}`, host_id: players[0] });
  const room = created.room_id;
  for (const p of players.slice(1)) await mcp("execute_action", { room_id: room, player_id: p, action: "lobby:join" });
  await mcp("execute_action", { room_id: room, player_id: players[0], action: "lobby:start" });
  for (let g = 0; g < 40; g += 1) {
    let acted = false;
    for (const p of players) {
      const legal = await mcp("get_legal_actions", { room_id: room, player_id: p });
      const choose = legal.find((a) => a.id.startsWith("setup:choose_general:"));
      if (choose) {
        await mcp("execute_action", { room_id: room, player_id: p, action: choose.id });
        acted = true;
        break;
      }
    }
    if (!acted) break;
  }
  await client.close();

  // 3) token 与公共/个人视图（隐藏信息 + 越权）
  const tokens = {};
  for (const p of players) tokens[p] = await mint(p, room);
  const first = await view(tokens[players[0]]);
  const board = first.public.data.public_board;
  assert(board.seat_count === 5, "公共视图含 5 个座位");
  assert(first.public.data.player_area === null, "公共视图 player_area 为 null");
  assert(board.seats.every((s) => !("hand" in s)), "公共座位不含手牌明细");
  assert(Array.isArray(first.actions), "资源响应带 actions 字段");
  const me = first.player.data.player_area;
  assert(me && Array.isArray(me.hand), "个人视图含本人手牌");

  const badToken = await (async () => {
    const r = await fetch(`${SERVER_URL}/auth/issue`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ openid: players[0], resource: "game:not-this-room" }),
    });
    return (await r.json()).token;
  })();
  const cross = await fetch(`${WEB_URL}/tabletop_game/sanguosha/action`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({ token: badToken, room_id: room, action: "end_play" }),
  });
  assert(cross.status === 403, "跨房间 token 提交动作 → 403");

  // 4) 浏览器：四档布局 + 隐藏信息 + 交互
  browser = await playwright.chromium.launch({ executablePath: browserPath, headless: true });
  const page = await browser.newPage();
  const cur = board.turn.player_id;
  const pageUrl = `${WEB_URL}/tabletop_game/sanguosha/player/${encodeURIComponent(cur)}?token=${encodeURIComponent(tokens[cur])}`;

  for (const [w, h] of [[1366, 900], [1920, 1080], [390, 844], [844, 390]]) {
    await page.setViewportSize({ width: w, height: h });
    await page.goto(pageUrl, { waitUntil: "load" });
    await page.waitForTimeout(700);
    const m = await page.evaluate(() => {
      const doc = document.documentElement;
      const seats = Array.from(document.querySelectorAll(".sgs-opponent")).map((n) => n.getBoundingClientRect());
      return {
        overflowX: doc.scrollWidth > doc.clientWidth,
        clipped: seats.filter((r) => r.left < -1 || r.right > doc.clientWidth + 1).length,
        seatCount: seats.length,
        handFaces: document.querySelectorAll(".sgs-hand .sgs-card").length,
        oppFaces: document.querySelectorAll(".sgs-opponent .sgs-card-rank").length,
        textHidden: getComputedStyle(document.querySelector("#sgs-text")).display === "none",
        ops: !!document.querySelector(".sgs-ops"),
        bundle: !!document.querySelector('script[src="/assets/sgs.js"]'),
      };
    });
    assert(!m.overflowX, `${w}x${h} 无横向溢出`);
    assert(m.clipped === 0, `${w}x${h} 座位无裁切`);
    assert(m.seatCount === 4, `${w}x${h} 显示 4 名对手`);
    assert(m.oppFaces === 0, `${w}x${h} 公共区无手牌正面`);
    assert(m.textHidden, `${w}x${h} 默认显示游戏态（文本态隐藏）`);
    assert(m.ops && m.bundle, `${w}x${h} 操作条与 sgs bundle 存在`);
  }

  // 5) 无 JS 表单（urlencoded → 303）
  const beforeForm = (await view(tokens[cur])).public.data.public_board;
  const form = await fetch(`${WEB_URL}/tabletop_game/sanguosha/action`, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ token: tokens[cur], room_id: room, action: "end_play" }),
    redirect: "manual",
  });
  assert(form.status === 303, "无 JS 表单提交 → 303 PRG");
  const afterForm = (await view(tokens[cur])).public.data.public_board;
  assert(
    afterForm.turn_index > beforeForm.turn_index ||
      JSON.stringify(afterForm.pending) !== JSON.stringify(beforeForm.pending) ||
      afterForm.turn.phase !== beforeForm.turn.phase,
    "表单提交后局面推进（回合/阶段/pending 变化）",
  );

  // 6) 浏览器交互：清掉 pending（弃牌/响应按「不出」）后 end_play，直到回合推进
  await page.setViewportSize({ width: 390, height: 844 });
  const startTurn = (await view(tokens[cur])).public.data.public_board.turn_index;
  for (let step = 0; step < 8; step += 1) {
    await page.goto(pageUrl, { waitUntil: "load" });
    await page.waitForTimeout(700);
    const pending = await page.evaluate(() => document.querySelector(".sgs-pending")?.dataset.kind ?? null);
    if (pending) {
      const clicked = await page.evaluate(() => {
        const btns = Array.from(document.querySelectorAll(".sgs-pending-actions [data-action]"));
        const pass = btns.find((b) => b.dataset.action === "pass");
        const discard = btns.find((b) => (b.dataset.action ?? "").startsWith("discard:"));
        const target = (pass ?? discard ?? btns[0]);
        if (!target) return null;
        const action = target.dataset.action;
        target.click();
        return action;
      });
      if (!clicked) break;
      await page.waitForTimeout(1200);
      continue;
    }
    const hasEnd = await page.evaluate(() => {
      const btn = document.querySelector('[data-action="end_play"]');
      if (!btn || btn.disabled) return false;
      btn.click();
      return true;
    });
    if (!hasEnd) break;
    await page.waitForTimeout(1400);
    const now = (await view(tokens[cur])).public.data.public_board;
    if (now.turn_index > startTurn) break;
  }
  const final = (await view(tokens[cur])).public.data.public_board;
  assert(final.turn_index > startTurn, "浏览器点击可推进回合（end_play + 必要弃牌）");
  assert(final.deck.draw + final.deck.discard > 0, "牌堆计数正常");
} catch (err) {
  failed += 1;
  console.error("[sgs-web-e2e] ❌ 运行失败:", err?.message ?? err);
} finally {
  try {
    await browser?.close();
  } catch {
    /* ignore */
  }
  for (const c of children) killTree(c);
  await fs.rm(SQLITE_PATH, { force: true }).catch(() => {});
}

if (failed > 0) {
  console.error(`[sgs-web-e2e] 失败 ${failed} 项`);
  process.exit(1);
}
log("全部通过 ✅");
