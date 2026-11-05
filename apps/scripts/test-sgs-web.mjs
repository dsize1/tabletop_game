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
const FX = process.argv.includes("--fx");
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
  // 0) 构建前端 bundle（e2e 用最新 client；失败则中止）
  const build = spawnSync("npm", ["--prefix", "apps/web", "run", "build:client"], {
    cwd: projectRoot,
    stdio: "ignore",
    shell: process.platform === "win32",
  });
  if (build.status !== 0) throw new Error("build:client 失败");

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

  // 非当前行动者（本房间合法 token）→ 引擎 PermissionDenied（P8 动作级权鉴）。
  const nonActor = players.find((p) => p !== board.turn.player_id);
  const nonActorResp = await fetch(`${WEB_URL}/tabletop_game/sanguosha/action`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify({
      token: tokens[nonActor],
      room_id: room,
      action: "end_play",
    }),
  });
  assert(
    nonActorResp.status === 403 || nonActorResp.status === 400,
    `非行动者动作应被拒绝（实际 ${nonActorResp.status}）`,
  );

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
        bundle: !!document.querySelector('script[src^="/assets/sgs.js"]'),
        stacked: (() => {
          const backs = Array.from(document.querySelectorAll(".sgs-opponent .sgs-card.back"));
          if (backs.length < 2) return null;
          return parseFloat(getComputedStyle(backs[1]).marginLeft) < 0;
        })(),
      };
    });
    assert(!m.overflowX, `${w}x${h} 无横向溢出`);
    assert(m.clipped === 0, `${w}x${h} 座位无裁切`);
    assert(m.seatCount === 4, `${w}x${h} 显示 4 名对手`);
    assert(m.oppFaces === 0, `${w}x${h} 公共区无手牌正面`);
    assert(m.textHidden, `${w}x${h} 默认显示游戏态（文本态隐藏）`);
    assert(m.ops && m.bundle, `${w}x${h} 操作条与 sgs bundle 存在`);
    if (m.stacked !== null) assert(m.stacked, `${w}x${h} 他人手牌背面堆叠（负 margin）`);
  }

  // 4.5) 侧栏（阶段/历史）必须随动作即时更新，而不是只有整页刷新后才更新
  const boardNow = async () =>
    (await view(tokens[players[0]])).public.data.public_board;
  const pageUrlFor = (pid) =>
    `${WEB_URL}/tabletop_game/sanguosha/player/${encodeURIComponent(pid)}?token=${encodeURIComponent(tokens[pid])}`;
  const currentActor = async () => {
    const b = await boardNow();
    return b.pending?.actor ?? b.turn.player_id;
  };
  const openActionPage = async () => {
    const actor = await currentActor();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(pageUrlFor(actor), { waitUntil: "load" });
    await page.waitForTimeout(900);
    return actor;
  };
  const startTurn = (await boardNow()).turn_index;
  await openActionPage();
  const sideBefore = await page.evaluate(() => ({
    items: document.querySelectorAll("#sgs-side .sgs-history li").length,
    last: document.querySelector("#sgs-side .sgs-history li:last-child")?.textContent ?? "",
  }));
  const clickedEnd = await page.evaluate(() => {
    const pendingBtn = document.querySelector(".sgs-pending-actions [data-action]");
    const end = document.querySelector('[data-action="end_play"]');
    const target = pendingBtn ?? end;
    if (!target || target.disabled) return null;
    const action = target.dataset.action;
    target.click();
    return action;
  });
  assert(Boolean(clickedEnd), "当前玩家存在可点击动作（待办或「结束出牌」）");
  if (clickedEnd) {
    await page.waitForTimeout(1500);
    const sideAfter = await page.evaluate(() => ({
      items: document.querySelectorAll("#sgs-side .sgs-history li").length,
      last: document.querySelector("#sgs-side .sgs-history li:last-child")?.textContent ?? "",
      phases: Array.from(document.querySelectorAll("#sgs-side .sgs-phase.on")).map((n) => n.textContent),
    }));
    assert(
      sideAfter.items > sideBefore.items || sideAfter.last !== sideBefore.last,
      "侧栏出牌历史随动作即时更新（无需刷新）",
    );
    assert(sideAfter.phases.length >= 1, "侧栏阶段条随动作更新");
  }

  // 5) 无 JS 表单（urlencoded → 303）：按当前局面取一个合法动作（pending 优先）
  const formActorNow = await currentActor();
  const snap0 = await view(tokens[formActorNow]);
  const b0 = snap0.public.data.public_board;
  let formActor = b0.turn.player_id;
  let formAction = null;
  if (b0.pending) {
    formActor = b0.pending.actor;
    const pv = await view(tokens[formActor]);
    const opts = pv.player.data.player_area?.pending?.options ?? [];
    formAction = (opts.find((o) => o.id === "pass") ?? opts[0])?.id ?? null;
  } else {
    formAction = (snap0.actions ?? []).find((a) => a.id === "end_play")?.id ?? (snap0.actions ?? [])[0]?.id ?? null;
  }
  assert(Boolean(formActor && formAction), "取到可提交的合法动作（无 JS 表单）");
  const beforeForm = b0;
  const beforePendingOpts = snap0.player.data.player_area?.pending?.options ?? [];
  const form = await fetch(`${WEB_URL}/tabletop_game/sanguosha/action`, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ token: tokens[formActor], room_id: room, action: formAction }),
    redirect: "manual",
  });
  assert(form.status === 303, "无 JS 表单提交 → 303 PRG");
  const afterView = await view(tokens[formActor]);
  const afterForm = afterView.public.data.public_board;
  const afterPendingOpts = afterView.player.data.player_area?.pending?.options ?? [];
  assert(
    afterForm.turn_index > beforeForm.turn_index ||
      JSON.stringify(afterForm.pending) !== JSON.stringify(beforeForm.pending) ||
      afterForm.turn.phase !== beforeForm.turn.phase ||
      JSON.stringify(afterPendingOpts) !== JSON.stringify(beforePendingOpts),
    "表单提交后局面推进（回合/阶段/pending/选项变化）",
  );

  // 6) 浏览器交互：动态定位当前行动者，清掉 pending（弃牌/响应按「不出」）后 end_play，直到回合推进
  const startBoard2 = await boardNow();
  const startTurn2 = startBoard2.turn_index;
  const startPlayer2 = startBoard2.turn.player_id;
  for (let step = 0; step < 12; step += 1) {
    await openActionPage();
    let clicked = null;
    // 弃牌 pending：多选手牌 → 一次确认（P4.4.4 真机：逐张点改为多选确认）。
    // 手牌选择走 pointer 手势（bindPress），必须用真实鼠标点击，不能用 element.click()。
    const isDiscardPending = await page.evaluate(
      () => Boolean(document.querySelector(".sgs-pending-discard")),
    );
    if (isDiscardPending) {
      const required = await page.evaluate(() =>
        Number(document.querySelector(".sgs-pending-discard")?.dataset.required ?? 1),
      );
      const cards = page.locator(".sgs-hand .sgs-card");
      const n = Math.min(required, await cards.count());
      for (let i = 0; i < n; i += 1) await cards.nth(i).click();
      const confirm = page.locator('[data-op="discard-confirm"]');
      if (!(await confirm.isDisabled())) {
        await confirm.click();
        clicked = `discard-multi(${n})`;
      }
    } else {
      clicked = await page.evaluate(() => {
        const btns = Array.from(document.querySelectorAll(".sgs-pending-actions [data-action]"));
        const pass = btns.find((b) => b.dataset.action === "pass");
        const pendingTarget = pass ?? btns[0];
        const end = document.querySelector('[data-action="end_play"]');
        const target = pendingTarget ?? end;
        if (!target || target.disabled) return null;
        const action = target.dataset.action;
        target.click();
        return action;
      });
    }
    if (!clicked) break;
    await page.waitForTimeout(1300);
    const now = await boardNow();
    if (now.turn_index > startTurn2 || now.turn.player_id !== startPlayer2) break;
  }
  const final = await boardNow();
  assert(
    final.turn_index > startTurn2 || final.turn.player_id !== startPlayer2,
    "浏览器点击可推进回合（end_play + 必要弃牌）",
  );
  assert(final.deck.draw + final.deck.discard > 0, "牌堆计数正常");

  // 7) `--fx`：固定历史注入 → 特效队列/输入锁定/暂停轮询（不依赖真实 AI 随机）
  if (FX) {
    const actor = await currentActor();
    let calls = 0;
    let injected = false;
    await page.route("**/tabletop_game/sanguosha/state*", async (route) => {
      const resp = await route.fetch();
      const json = await resp.json();
      const data = json.player?.data ?? json.public?.data;
      if (data?.public_board) {
        const b = data.public_board;
        if (data.player_area) data.player_area.pending = null;
        json.actions = [];
        calls += 1;
        if (!injected && calls >= 2 && b.history) {
          injected = true;
          const maxSeq = b.history.at(-1)?.seq ?? 0;
          const other = (b.seats ?? []).find((s) => s.player_id !== actor) ?? b.seats?.[0];
          b.history = [
            ...b.history,
            {
              seq: maxSeq + 1,
              turn: b.turn_index,
              actor: null,
              text: `${other.player_id} 受到 2 点雷电伤害（体力 1）`,
            },
          ];
        }
      }
      await route.fulfill({ response: resp, json });
    });
    await page.goto(pageUrlFor(actor), { waitUntil: "load" });
    await page.waitForTimeout(1200);
    // SSR/首屏可能带 pending 浮层；测试只看特效层，先移开浮层（不改状态）。
    await page.evaluate(() => document.querySelector(".sgs-overlay")?.classList.remove("on"));
    const layoutBefore = await page.evaluate(() => {
      const r = document.querySelector(".sgs-main")?.getBoundingClientRect();
      return r ? { w: Math.round(r.width), h: Math.round(r.height), x: Math.round(r.x), y: Math.round(r.y) } : null;
    });
    let sample = null;
    const fxStart = Date.now();
    while (Date.now() - fxStart < 15000) {
      const s = await page.evaluate(() => {
        const img = document.querySelector("#sgs-fx-layer .fx-cue img");
        return {
          locked: document.body.classList.contains("sgs-fx-locked"),
          cues: document.querySelectorAll("#sgs-fx-layer .fx-cue").length,
          kind: img?.closest(".fx-cue")?.className ?? "",
          src: img?.getAttribute("src") ?? "",
          blend: img ? getComputedStyle(img).mixBlendMode : "",
          appPe: getComputedStyle(document.getElementById("sgs-app")).pointerEvents,
          sidePe: getComputedStyle(document.getElementById("sgs-side") || document.body).pointerEvents,
          sync: document.getElementById("sgs-sync")?.textContent ?? "",
        };
      });
      if (s.locked && s.cues > 0 && s.src) {
        sample = s;
        break;
      }
      await page.waitForTimeout(100);
    }
    assert(Boolean(sample), "--fx：注入历史触发特效队列");
    if (sample) {
      assert(sample.kind.includes("fx-thunder"), "--fx：雷电 cue 映射正确");
      assert(sample.src.startsWith("/fx-assets/"), "--fx：加载动画素材");
      assert(sample.blend === "normal", "--fx：透明素材使用 normal 混合");
      assert(sample.appPe === "none", "--fx：消费期间禁止操作");
      assert(sample.sidePe === "auto", "--fx：消费期间历史仍可滚动");
      const callsAtLock = calls;
      let pausedOk = true;
      for (let i = 0; i < 5; i += 1) {
        await page.waitForTimeout(60);
        if (!(await page.evaluate(() => document.body.classList.contains("sgs-fx-locked")))) break;
        if (calls !== callsAtLock) pausedOk = false;
      }
      assert(pausedOk, "--fx：消费期间暂停轮询");
      await page.evaluate(() => document.querySelector(".sgs-hand .sgs-card")?.click());
      const selected = await page.evaluate(
        () => document.querySelectorAll(".sgs-hand .sgs-card.selected").length,
      );
      assert(selected === 0, "--fx：锁定期间点击手牌不选中");
      await page.waitForTimeout(1200);
      const unlocked = await page.evaluate(
        () => !document.body.classList.contains("sgs-fx-locked"),
      );
      assert(unlocked, "--fx：队列结束后解锁");
      const layoutAfter = await page.evaluate(() => {
        const r = document.querySelector(".sgs-main")?.getBoundingClientRect();
        return r ? { w: Math.round(r.width), h: Math.round(r.height), x: Math.round(r.x), y: Math.round(r.y) } : null;
      });
      assert(
        JSON.stringify(layoutBefore) === JSON.stringify(layoutAfter),
        "--fx：特效层不改变现有版面布局",
      );
    }
    await page.unroute("**/tabletop_game/sanguosha/state*");
  }
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
