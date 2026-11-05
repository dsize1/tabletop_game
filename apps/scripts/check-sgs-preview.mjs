#!/usr/bin/env node
// 工作台预览/沙盒 harness（P5 基座，ADR-0033/0034）。
//
// 起临时 server + MCP（独立端口/SQLite）→ 从真实对局抓模板 → 场景注入 →
// 创建/单步/AI/重置/销毁预览 → 沙盒创建对局（meta/指定武将身份/全 bot）→
// 断言真相源表行数不变、鉴权负向、模板校验；--ui 追加 Web/抽屉/iframe 冒烟。
//
// 用法：
//   npm run check:sgs-preview          # API 级：模板/场景/预览/AI/隔离（默认）
//   npm run check:sgs-preview:ui       # 追加 Web 预览页/工作台 iframe 冒烟（需系统 Edge/Chrome）
// 依赖：Redis（auth 服务）、已构建的 server / game-engine-mcp；--ui 另需 props-admin dist。
// 任一依赖缺失 → 打印 skip 并以 0 退出。

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "../..");
const SQLITE_PATH =
  process.env.SGS_PREVIEW_E2E_DB ?? path.join(os.tmpdir(), `sgs_preview_e2e_${Date.now()}.db`);
const SERVER_PORT = Number(process.env.SGS_PREVIEW_E2E_PORT ?? 3197);
const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`;
const WEB_PORT = Number(process.env.SGS_PREVIEW_E2E_WEB_PORT ?? 18197);
const WEB_URL = `http://127.0.0.1:${WEB_PORT}`;
const UI = process.argv.includes("--ui");
const UI_OPENID = "ui-e2e";
const REDIS_URL = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const ADMIN_TOKEN = "preview-e2e-token";
const ADMIN_ACTOR = "preview-admin";
const players = ["preview-p1", "preview-p2", "preview-p3", "preview-p4", "preview-p5"];
const children = [];
let failed = 0;

function log(msg) {
  console.log(`[sgs-preview] ${msg}`);
}
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error(`[sgs-preview] ❌ ${msg}`);
  } else {
    log(`✅ ${msg}`);
  }
}
function skip(msg) {
  log(`skip: ${msg}`);
  cleanup();
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

function cleanup() {
  for (const child of children) killTree(child);
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

function headers(token = ADMIN_TOKEN, actor = ADMIN_ACTOR) {
  const h = { "content-type": "application/json" };
  if (token) h["x-props-admin-token"] = token;
  if (actor) h["x-props-actor"] = actor;
  return h;
}

async function api(
  pathname,
  { method = "GET", body, token = ADMIN_TOKEN, actor = ADMIN_ACTOR } = {},
) {
  const r = await fetch(`${SERVER_URL}${pathname}`, {
    method,
    headers: headers(token, actor),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await r.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text };
  }
  return { status: r.status, body: json };
}

function dbCounts() {
  const db = new DatabaseSync(SQLITE_PATH, { readOnly: true });
  try {
    const count = (table) => db.prepare(`SELECT COUNT(*) AS n FROM ${table}`).get().n;
    const previewRows = (table) =>
      db.prepare(`SELECT COUNT(*) AS n FROM ${table} WHERE room_id LIKE 'preview:%'`).get().n;
    return {
      game_state: count("game_state"),
      game_events: count("game_events"),
      game_decision_log: count("game_decision_log"),
      templates: count("game_state_templates"),
      preview_game_state: previewRows("game_state"),
      preview_game_events: previewRows("game_events"),
      preview_decision_log: previewRows("game_decision_log"),
    };
  } finally {
    db.close();
  }
}

/**
 * UI 冒烟（--ui）：起临时 web（同源托管 props-admin dist）→ 用本地登录流程拿 `tg_props`
 * → Playwright 打开预览页/工作台，断言工具栏、iframe、视角、AI/重置。
 * 缺依赖时跳过（不算失败）。
 */
async function runUi(room, templateId, previewId, players) {
  if (!UI) return;
  const dist = path.join(projectRoot, "apps/props-admin/dist/index.html");
  if (!existsSync(dist)) {
    const build = spawnSync("npm", ["--prefix", "apps", "run", "build:props"], {
      cwd: projectRoot,
      stdio: "ignore",
      shell: process.platform === "win32",
    });
    if (build.status !== 0) {
      log("skip ui: props-admin 构建失败（npm run build:props）");
      return;
    }
  }
  let playwright = null;
  try {
    playwright = await import("playwright-core");
  } catch {
    log("skip ui: 未安装 playwright-core");
    return;
  }
  const browserPath = edgePath();
  if (!browserPath) {
    log("skip ui: 未找到系统 Edge/Chrome");
    return;
  }

  // 1) 临时 web（SSR + props-admin 同源托管 + /internal 代理）
  const web = spawn(
    process.platform === "win32" ? "npx.cmd" : "npx",
    ["tsx", "src/index.ts"],
    {
      cwd: path.join(projectRoot, "apps/web"),
      env: {
        ...process.env,
        WEB_PORT: String(WEB_PORT),
        SERVER_URL,
        SQLITE_PATH,
        PROPS_ADMIN_TOKEN: ADMIN_TOKEN,
      },
      stdio: ["ignore", "ignore", "pipe"],
      shell: process.platform === "win32",
    },
  );
  children.push(web);
  if (!(await waitHealth(`${WEB_URL}/health`))) {
    assert(false, "ui: web 未就绪");
    return;
  }

  // 2) 本地跨端登录：link → register → verify → confirm，拿 tg_props 会话 token
  const link = await fetch(`${SERVER_URL}/internal/auth/web/link`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      group_id: "preview-ui",
      openid: UI_OPENID,
      display_name: "预览冒烟",
      scope: "props",
      target: "/props-admin/workbench",
    }),
  });
  const linkBody = await link.json().catch(() => ({}));
  if (!link.ok) {
    assert(false, `ui: 申请 props 登录链接失败（${link.status}）`);
    return;
  }
  const passcode = "abc123";
  const reg = await fetch(`${SERVER_URL}/auth/web/register`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token: linkBody.token, passcode, passcode_confirm: passcode }),
  });
  const regBody = await reg.json().catch(() => ({}));
  if (!reg.ok) {
    assert(false, `ui: 注册注册码失败（${reg.status} ${regBody.error ?? ""}）`);
    return;
  }
  const ver = await fetch(`${SERVER_URL}/internal/auth/web/verify`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ group_id: "preview-ui", code: regBody.code, openid: UI_OPENID }),
  });
  const verBody = await ver.json().catch(() => ({}));
  if (!ver.ok || verBody.result !== "confirmed") {
    assert(false, `ui: 群内确认失败（${ver.status} ${verBody.result ?? ""}）`);
    return;
  }
  const conf = await fetch(
    `${SERVER_URL}/auth/web/confirm?token=${encodeURIComponent(linkBody.token)}&wait=5`,
  ).then((r) => r.json());
  if (conf.status !== "confirmed" || !conf.token) {
    assert(false, `ui: 换取 tg_props 失败（${conf.status ?? "?"}）`);
    return;
  }
  const cookie = conf.cookie === "tg_props" ? "tg_props" : "tg_props";

  // 3) 未登录访问预览页 → 403
  const anon = await fetch(
    `${WEB_URL}/tabletop_game/sanguosha/preview?room=${encodeURIComponent(previewId)}&viewer=${encodeURIComponent(players[0])}`,
  );
  assert(anon.status === 403, `ui: 未登录访问预览页 → 403（实际 ${anon.status}）`);

  // 测试玩家：只有 /props-admin/workbench 可访问；其它菜单 401，越权 API 403。
  const cookieHeader = { cookie: `tg_props=${conf.token}` };
  const deniedRoot = await fetch(`${WEB_URL}/props-admin/`, { headers: cookieHeader });
  assert(deniedRoot.status === 401, `ui: 测试玩家访问中台首页 → 401（实际 ${deniedRoot.status}）`);
  const deniedPacks = await fetch(`${WEB_URL}/props-admin/packs`, { headers: cookieHeader });
  assert(deniedPacks.status === 401, `ui: 测试玩家访问道具包菜单 → 401（实际 ${deniedPacks.status}）`);
  const allowedWorkbench = await fetch(`${WEB_URL}/props-admin/workbench`, {
    headers: cookieHeader,
  });
  assert(
    allowedWorkbench.status === 200,
    `ui: 测试玩家访问工作台 → 200（实际 ${allowedWorkbench.status}）`,
  );
  const webSession = await fetch(`${WEB_URL}/internal/gameprops/session`, {
    headers: cookieHeader,
  });
  const webSessionBody = await webSession.json().catch(() => ({}));
  assert(
    webSession.status === 200 && webSessionBody.scope === "workbench",
    `ui: 测试玩家 web session scope=workbench（实际 ${webSession.status}/${webSessionBody.scope}）`,
  );
  const webSummary = await fetch(`${WEB_URL}/internal/gameprops/summary`, {
    headers: cookieHeader,
  });
  assert(webSummary.status === 403, `ui: 测试玩家越权 API → 403（实际 ${webSummary.status}）`);

  let browser = null;
  try {
    browser = await playwright.chromium.launch({ executablePath: browserPath, headless: true });
    const context = await browser.newContext({ viewport: { width: 1366, height: 900 } });
    await context.addCookies([
      { name: cookie, value: conf.token, domain: "127.0.0.1", path: "/", httpOnly: true },
    ]);
    const page = await context.newPage();

    // 4) 预览页：工具栏/三分区/视角下拉/AI 单步/重置
    await page.goto(
      `${WEB_URL}/tabletop_game/sanguosha/preview?room=${encodeURIComponent(previewId)}&viewer=${encodeURIComponent(players[0])}`,
      { waitUntil: "domcontentloaded", timeout: 20000 },
    );
    await page.waitForTimeout(1500);
    assert((await page.title()).includes("三国杀"), "ui: 预览页标题");
    assert((await page.locator(".sgs-previewbar").count()) === 1, "ui: 预览工具栏存在");
    assert((await page.locator(".sgs-opponent").count()) === 4, "ui: 预览页显示 4 名对手");
    assert((await page.locator(".sgs-hand .sgs-card").count()) >= 2, "ui: 预览页手牌可见");
    assert(
      (await page.locator("#sgs-preview-viewer").inputValue()) === players[0],
      "ui: 视角下拉默认当前玩家",
    );
    // AI 动作可能打开 pending 浮层（遮罩会拦住 Playwright 的可操作性检查），
    // 直接派发 click 给工具栏按钮。
    await page.evaluate(() => {
      document.querySelector('[data-preview-cmd="ai-step"]')?.dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    await page.waitForTimeout(1500);
    const aiMsg = await page.locator("#sgs-msg").textContent();
    assert((aiMsg ?? "").includes("AI"), `ui: AI 单步有回执（${aiMsg ?? ""}）`);
    await page.evaluate(() => {
      document.querySelector('[data-preview-cmd="reset"]')?.dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    await page.waitForTimeout(1200);
    assert(
      ((await page.locator("#sgs-msg").textContent()) ?? "").includes("基线"),
      "ui: 重置回执",
    );
    // 其他玩家武将详情：技能必须可见（公开信息）。
    // 直接派发 Pointer 事件（座位层有绝对定位/重叠，click 的可操作性检查会超时）。
    await page.evaluate(() => {
      const el = document.querySelector(".sgs-opponent");
      if (!el) return;
      const r = el.getBoundingClientRect();
      const init = {
        bubbles: true,
        pointerId: 1,
        pointerType: "mouse",
        button: 0,
        clientX: r.left + r.width / 2,
        clientY: r.top + r.height / 2,
      };
      el.dispatchEvent(new PointerEvent("pointerdown", init));
      el.dispatchEvent(new PointerEvent("pointerup", init));
    });
    await page.waitForTimeout(400);
    assert(
      (await page.locator(".sgs-detail-skills li").count()) > 0,
      "ui: 其他玩家武将详情含技能",
    );
    await page.evaluate(() => {
      document.querySelector(".sgs-overlay-close")?.dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
    });
    await page.waitForTimeout(300);

    const other = players[1];
    await page.selectOption("#sgs-preview-viewer", other);
    await page.waitForTimeout(1500);
    assert(
      (await page.locator("#sgs-preview-viewer").inputValue()) === other,
      "ui: 视角切换生效",
    );

    // 5) 工作台：四 TAB + 创建预览 + iframe 加载
    await page.goto(`${WEB_URL}/props-admin/workbench`, {
      waitUntil: "domcontentloaded",
      timeout: 20000,
    });
    await page.waitForTimeout(2000);
    assert(
      ((await page.locator("h2").first().textContent()) ?? "").includes("工作台"),
      "ui: 工作台页面加载",
    );
    await page.waitForTimeout(800);
    assert(
      (await page.getByText("概览", { exact: true }).count()) === 0,
      "ui: 测试玩家菜单不含「概览」",
    );
    assert(
      (await page.getByText("工作台", { exact: true }).count()) > 0,
      "ui: 测试玩家菜单保留「工作台」",
    );
    for (const tab of ["模板", "预览", "AI 调试", "决策日志"]) {
      assert((await page.getByText(tab, { exact: true }).count()) > 0, `ui: TAB ${tab} 存在`);
    }
    assert((await page.locator(".wb-table tbody tr").count()) >= 1, "ui: 模板列表非空");
    // 模板列表选中行对比度（防深底深字不可读）
    const contrast = await page.evaluate(() => {
      const row =
        document.querySelector(".wb-table tbody tr.selected") ??
        document.querySelector(".wb-table tbody tr");
      if (!row) return 0;
      const style = getComputedStyle(row);
      const rgb = (value) => (value.match(/[0-9.]+/g) ?? []).slice(0, 3).map(Number);
      const lum = ([r, g, b]) => {
        const f = (v) => {
          const x = v / 255;
          return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
        };
        return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
      };
      const fg = lum(rgb(style.color));
      const bg = lum(rgb(style.backgroundColor));
      return (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
    });
    assert(contrast >= 4.5, `ui: 模板列表选中行对比度 ≥4.5（实际 ${contrast.toFixed(2)}）`);
    const createBtn = page.getByRole("button", { name: "创建预览" });
    assert((await createBtn.count()) === 1, "ui: 创建预览按钮存在");
    await createBtn.click();
    await page.waitForTimeout(2500);
    const frame = page.locator("iframe.preview-frame");
    assert((await frame.count()) === 1, "ui: 工作台 iframe 存在");
    const handle = await frame.first().elementHandle();
    const inner = handle ? await handle.contentFrame() : null;
    if (inner) {
      await inner.waitForTimeout(1200);
      assert((await inner.locator(".sgs-previewbar").count()) === 1, "ui: iframe 内预览工具栏");
      assert((await inner.locator(".sgs-opponent").count()) === 4, "ui: iframe 内 4 名对手");
    } else {
      assert(false, "ui: iframe contentFrame 不可用");
    }

    // 创建对局抽屉：切回模板 TAB → 打开抽屉 → 默认 5 行 → 提交 → iframe 加载沙盒
    await page.getByText("模板", { exact: true }).first().click();
    await page.waitForTimeout(600);
    const sandboxBtn = page.getByRole("button", { name: "创建对局" });
    assert((await sandboxBtn.count()) === 1, "ui: 创建对局按钮存在");
    await sandboxBtn.click();
    await page.waitForTimeout(800);
    assert((await page.locator(".seat-row").count()) === 5, "ui: 抽屉默认 5 个座位行");
    assert(
      (await page.getByText("创建并预览", { exact: true }).count()) === 1,
      "ui: 抽屉提交按钮存在",
    );
    await page.getByText("创建并预览", { exact: true }).click();
    await page.waitForTimeout(2500);
    const sandboxFrame = page.locator("iframe.preview-frame");
    assert((await sandboxFrame.count()) === 1, "ui: 沙盒创建后 iframe 存在");
    const sandboxHandle = await sandboxFrame.first().elementHandle();
    const sandboxInner = sandboxHandle ? await sandboxHandle.contentFrame() : null;
    if (sandboxInner) {
      await sandboxInner.waitForTimeout(1200);
      assert(
        (await sandboxInner.locator("#sgs-preview-viewer").inputValue()) === "bot:1",
        "ui: 沙盒默认视角 bot:1",
      );
    } else {
      assert(false, "ui: 沙盒 iframe contentFrame 不可用");
    }
    void templateId;
    void room;
  } catch (e) {
    assert(false, `ui: ${e?.message ?? e}`);
  } finally {
    if (browser) await browser.close().catch(() => {});
  }
}

const redisPort = Number(new URL(REDIS_URL).port || 6379);
if (!(await tcpOk("127.0.0.1", redisPort))) skip(`Redis 不可用（${REDIS_URL}），auth 服务需要它`);

const serverExe = path.join(
  projectRoot,
  "crates/target/debug",
  process.platform === "win32" ? "server.exe" : "server",
);
const mcpExe = path.join(
  projectRoot,
  "crates/target/debug",
  process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp",
);
if (!existsSync(serverExe) || !existsSync(mcpExe)) skip("server / game-engine-mcp 未构建（cargo build）");

let client = null;
try {
  // 1) 临时 server（共享 token 打开，便于测鉴权）
  const server = spawn(serverExe, [], {
    cwd: projectRoot,
    env: {
      ...process.env,
      SQLITE_PATH,
      SERVER_PORT: String(SERVER_PORT),
      REDIS_URL,
      PROPS_ADMIN_TOKEN: ADMIN_TOKEN,
      DEV_WHITELIST: ADMIN_ACTOR,
      TEST_PLAYER_WHITELIST: UI_OPENID,
      RUST_LOG: "warn",
      SGS_PREVIEW_TTL_SECS: "600",
      SGS_PREVIEW_MAX_SESSIONS: "4",
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  children.push(server);
  if (!(await waitHealth(`${SERVER_URL}/health`))) throw new Error("server 未就绪");

  // 2) MCP 建局 + 选将（真实对局快照）
  client = new Client({ name: "sgs-preview-e2e", version: "0.1.0" });
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
    group_id: `preview-e2e-${Date.now()}`,
    host_id: players[0],
  });
  const room = created.room_id;
  for (const p of players.slice(1)) {
    await mcp("execute_action", { room_id: room, player_id: p, action: "lobby:join" });
  }
  await mcp("execute_action", { room_id: room, player_id: players[0], action: "lobby:start" });
  for (let g = 0; g < 60; g += 1) {
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
  client = null;
  log(`真实对局就绪：${room}`);

  const dbBefore = dbCounts();

  // 3) 鉴权负向：无 token → 401
  const noAuth = await api("/internal/gameprops/templates", { token: "", actor: null });
  assert(noAuth.status === 401, `无中台令牌/无 actor 访问模板 → 401（实际 ${noAuth.status}）`);
  const badAuth = await api("/internal/gameprops/templates", { token: "wrong", actor: null });
  assert(badAuth.status === 401, `错误中台令牌 → 401（实际 ${badAuth.status}）`);

  // 3.5) 测试玩家白名单：仅 workbench 菜单/API，越权菜单 API 403
  const adminSession = await api("/internal/gameprops/session");
  assert(adminSession.status === 200 && adminSession.body.scope === "admin", "管理员 session scope=admin");
  const testSession = await api("/internal/gameprops/session", { actor: UI_OPENID });
  assert(
    testSession.status === 200 && testSession.body.scope === "workbench",
    `测试玩家 session scope=workbench（实际 ${testSession.body.scope}）`,
  );
  assert(
    Array.isArray(testSession.body.menus) && testSession.body.menus.length === 1 &&
      testSession.body.menus[0] === "workbench",
    "测试玩家菜单仅 workbench",
  );
  const testTemplates = await api("/internal/gameprops/templates", { actor: UI_OPENID });
  assert(testTemplates.status === 200, "测试玩家可访问工作台模板 API");
  const testPreviewMeta = await api("/internal/game/preview/meta", { actor: UI_OPENID });
  assert(testPreviewMeta.status === 200, "测试玩家可访问预览/沙盒 API");
  const testSummary = await api("/internal/gameprops/summary", { actor: UI_OPENID });
  assert(testSummary.status === 403, `测试玩家访问概览 API → 403（实际 ${testSummary.status}）`);
  const testGenerals = await api("/internal/gameprops/generals", { actor: UI_OPENID });
  assert(testGenerals.status === 403, `测试玩家访问武将表 API → 403（实际 ${testGenerals.status}）`);

  // 4) 从真实对局抓模板
  const grabbed = await api("/internal/gameprops/templates/from-room", {
    method: "POST",
    body: { room_id: room, name: "e2e-真实对局" },
  });
  assert(grabbed.status === 200, `从对局抓模板成功（${grabbed.status}）`);
  const templateId = grabbed.body?.row?.id;
  assert(typeof templateId === "number", "返回模板 id");

  const list = await api("/internal/gameprops/templates?game_id=sanguosha");
  assert(list.status === 200 && list.body.rows.some((r) => r.id === templateId), "模板列表包含新模板");
  const tpl = await api(`/internal/gameprops/templates/${templateId}`);
  assert(tpl.status === 200 && tpl.body.snapshot?.data?.seats?.length === 5, "模板详情含 5 人完整快照");

  // 5) 非法场景 → 400
  const badScene = await api("/internal/game/preview/scene", {
    method: "POST",
    body: { snapshot: tpl.body.snapshot, scene: { seats: [{ index: 0, general: "nope" }] } },
  });
  assert(badScene.status === 400, `非法场景被拒（${badScene.status}）`);

  // 6) 合法场景校验（结构化 SceneSpec）
  const scene = {
    phase: "turn",
    turn: 0,
    turn_phase: "play",
    seats: [
      {
        index: 0,
        hand: ["sha", "shan"],
        chained: true,
        mark_counts: { e2e: 1 },
      },
    ],
  };
  const sceneOk = await api("/internal/game/preview/scene", {
    method: "POST",
    body: { snapshot: tpl.body.snapshot, scene },
  });
  assert(sceneOk.status === 200 && sceneOk.body.seat_count === 5, "合法场景校验通过（5 人）");

  // 7) 创建预览（模板 + 场景）
  const createdPreview = await api("/internal/game/preview", {
    method: "POST",
    body: { template_id: templateId, scene, viewer: players[0] },
  });
  assert(createdPreview.status === 200, `创建预览成功（${createdPreview.status}）`);
  const previewId = createdPreview.body?.preview_id;
  assert(typeof previewId === "string" && previewId.startsWith("preview:"), "preview_id 前缀 preview:");
  const createdSeats = Array.isArray(createdPreview.body?.seats)
    ? createdPreview.body.seats
    : (createdPreview.body?.seats?.seats ?? []);
  assert(createdSeats.length === 5, "预览返回 5 个座位");

  const rawView = await api(
    `/internal/game/preview/${encodeURIComponent(previewId)}?raw=1&viewer=${encodeURIComponent(players[0])}`,
  );
  assert(rawView.status === 200, "读取预览 raw 状态");
  const rawState = rawView.body.raw?.data;
  assert(rawState?.seats?.[0]?.chained === true, "场景注入生效：座位 0 横置");
  assert(rawState?.seats?.[0]?.mark_counts?.e2e === 1, "场景注入生效：标记 e2e=1");
  const handDefs = (rawState?.seats?.[0]?.hand ?? []).map((id) => rawState.cards?.[id]?.def);
  assert(
    handDefs.length >= 2 && handDefs[0] === "sha" && handDefs[1] === "shan",
    `场景注入生效：手牌为 sha/shan（实际 ${handDefs.slice(0, 3).join(",")}）`,
  );

  // 8) 视图 + 动作 + AI + 重置
  const board = rawView.body.public?.data?.public_board;
  const pendingActor = board?.pending?.actor;
  const actor = pendingActor ?? board?.turn?.player_id;
  assert(typeof actor === "string" && actor.length > 0, "找到当前行动者");
  const actorView = await api(
    `/internal/game/preview/${encodeURIComponent(previewId)}?viewer=${encodeURIComponent(actor)}`,
  );
  assert(actorView.status === 200, "读取行动者个人视图");
  assert(
    actorView.body.player?.data?.player_area?.hand !== undefined,
    "个人视图含本人手牌（预览页契约）",
  );
  const actions = actorView.body.actions ?? [];
  assert(actions.length > 0, `行动者有合法动作（${actions.length}）`);
  const chosen = actions.find((a) => a.id === "end_play") ?? actions[0];
  const acted = await api(`/internal/game/preview/${encodeURIComponent(previewId)}/action`, {
    method: "POST",
    body: { player_id: actor, action: chosen.id },
  });
  assert(acted.status === 200 && acted.body.ok === true, `预览动作执行成功：${chosen.id}`);
  assert(acted.body.step?.message, "动作返回结果文案");

  const ai = await api(`/internal/game/preview/${encodeURIComponent(previewId)}/drive`, {
    method: "POST",
    body: { max_steps: 10, viewer: actor },
  });
  assert(ai.status === 200 && typeof ai.body.steps === "number", "AI 推进接口可用");
  assert(["finished", "human_turn", "max_steps", "no_actor", "no_legal_actions", "loop"].includes(ai.body.stop),
    `AI 停止原因合法：${ai.body.stop}`);

  const single = await api(`/internal/game/preview/${encodeURIComponent(previewId)}/drive`, {
    method: "POST",
    body: { max_steps: 1, actor, viewer: actor },
  });
  assert(single.status === 200, "AI 单步接口可用");
  if (single.body.trace) {
    assert(typeof single.body.trace.action === "string", "AI 单步返回 chosen action");
    assert(typeof single.body.trace.score === "number", "AI 单步返回打分");
  }

  const reset = await api(`/internal/game/preview/${encodeURIComponent(previewId)}/reset`, {
    method: "POST",
    body: {},
  });
  assert(reset.status === 200 && (reset.body.view?.steps ?? []).length === 0, "重置后步骤时间线清空");

  const sessions = await api("/internal/game/preview");
  assert(
    sessions.status === 200 && sessions.body.sessions.some((s) => s.preview_id === previewId),
    "预览会话列表包含当前会话",
  );

  // 9) 模板复制/更新/删除
  const dup = await api(`/internal/gameprops/templates/${templateId}/duplicate`, {
    method: "POST",
    body: { name: "e2e-副本" },
  });
  assert(dup.status === 200 && dup.body.row?.id !== templateId, "模板复制成功");
  const dupId = dup.body.row.id;
  const updated = await api(`/internal/gameprops/templates/${dupId}`, {
    method: "PUT",
    body: { meta: { scene: { round: 3 } } },
  });
  assert(updated.status === 200 && updated.body.row?.meta?.includes?.("scene"), "模板 meta 更新成功");
  const deleted = await api(`/internal/gameprops/templates/${dupId}`, { method: "DELETE" });
  assert(deleted.status === 200 && deleted.body.existed === true, "模板删除成功");

  // 9.5) 创建对局（沙盒开局）：meta + 指定武将/身份 + 全 bot + 跳选将
  const meta = await api("/internal/game/preview/meta");
  assert(meta.status === 200, `沙盒 meta 可用（${meta.status}）`);
  const metaGenerals = meta.body?.generals ?? [];
  // 名称勘误（2026-11-05）后默认去重池由 149 降为 148（张昭张纮 与界版同名被去重）。
  assert(metaGenerals.length === 148, `沙盒武将池为 148 去重（实际 ${metaGenerals.length}）`);
  assert(meta.body?.identities?.["5"]?.lord === 1, "沙盒身份配比：5 人 1 主");
  const g = metaGenerals.slice(0, 5).map((x) => x.code);
  const ids = ["lord", "loyalist", "rebel", "rebel", "spy"];
  const seatsSpec = g.map((general, index) => ({ index, general, identity: ids[index] }));
  const sandboxCreated = await api("/internal/game/preview/sandbox", {
    method: "POST",
    body: {
      players: 5,
      seed: 20261103,
      seats: seatsSpec,
      save_template: true,
      name: "e2e-沙盒创建",
    },
  });
  assert(sandboxCreated.status === 200, `沙盒创建成功（${sandboxCreated.status}）`);
  const sandboxId = sandboxCreated.body?.preview_id;
  const sandboxTemplateId = sandboxCreated.body?.template_id;
  assert(typeof sandboxId === "string" && sandboxId.startsWith("preview:"), "沙盒返回 preview_id");
  assert(sandboxCreated.body?.seed === 20261103, "沙盒回显 seed");
  assert(typeof sandboxTemplateId === "number", "沙盒可同时保存模板");

  const sandboxView = await api(
    `/internal/game/preview/${encodeURIComponent(sandboxId)}?raw=1&viewer=${encodeURIComponent("bot:1")}`,
  );
  assert(sandboxView.status === 200, "读取沙盒 raw 状态");
  const sandboxRaw = sandboxView.body.raw?.data;
  assert(sandboxRaw?.phase === "turn", "沙盒跳过选将：phase=turn");
  assert(sandboxRaw?.turn === 0, "沙盒主公先手（seat 0）");
  assert(
    (sandboxRaw?.seats ?? []).every((x) => x.is_bot === true),
    "沙盒所有座位都是 bot",
  );
  assert(
    (sandboxRaw?.seats ?? []).every((x) => (x.hand ?? []).length >= 4),
    "沙盒每人至少 4 张初始手牌",
  );
  assert(
    (sandboxRaw?.seats ?? []).map((x) => x.identity).join(",") === "lord,loyalist,rebel,rebel,spy",
    "沙盒身份按指定配比",
  );
  assert(
    (sandboxRaw?.seats ?? []).map((x) => x.general).join(",") === g.join(","),
    "沙盒武将按指定 code",
  );
  assert(
    new Set((sandboxRaw?.seats ?? []).map((x) => x.general)).size === 5,
    "沙盒武将不重复",
  );
  assert(
    (sandboxRaw?.deck?.draw ?? []).length + (sandboxRaw?.seats ?? []).reduce((n, x) => n + (x.hand ?? []).length, 0) <=
      Object.keys(sandboxRaw?.cards ?? {}).length,
    "沙盒牌数不超实例总数（守恒由引擎保证）",
  );

  // 负向：重复武将 / 主公超配比 / 人数越界
  const badSandbox = [
    { players: 4, seats: [] },
    {
      players: 5,
      seats: [
        { index: 0, general: g[0] },
        { index: 1, general: g[0] },
      ],
    },
    {
      players: 5,
      seats: [
        { index: 0, identity: "lord" },
        { index: 1, identity: "lord" },
      ],
    },
  ];
  for (const [i, body] of badSandbox.entries()) {
    const r = await api("/internal/game/preview/sandbox", { method: "POST", body });
    assert(r.status === 400, `沙盒负向 case ${i} 被拒（${r.status}）`);
  }

  // 随机填满（只给人数 + seed）
  const randomSandbox = await api("/internal/game/preview/sandbox", {
    method: "POST",
    body: { players: 8, seed: 7, seats: [] },
  });
  assert(randomSandbox.status === 200, "沙盒随机 8 人创建成功");
  const randomId = randomSandbox.body?.preview_id;
  const randomView = await api(
    `/internal/game/preview/${encodeURIComponent(randomId)}?raw=1&viewer=${encodeURIComponent("bot:1")}`,
  );
  const randomRaw = randomView.body.raw?.data;
  assert(
    new Set((randomRaw?.seats ?? []).map((x) => x.general)).size === 8,
    "沙盒随机武将不重复",
  );
  assert(
    (randomRaw?.seats ?? []).filter((x) => x.identity === "lord").length === 1,
    "沙盒随机身份恰有 1 主公",
  );
  await api(`/internal/game/preview/${encodeURIComponent(randomId)}`, { method: "DELETE" });

  // 10) 决策日志只读查询
  const logs = await api(`/internal/gameprops/decision-logs?room_id=${encodeURIComponent(room)}&limit=10`);
  assert(logs.status === 200 && Array.isArray(logs.body.rows), "决策日志查询可用");
  if (logs.body.rows.length > 0) {
    assert(/^[0-9a-f]{16}$/.test(logs.body.rows[0].player_id), "决策日志 player_id 已哈希（16 hex）");
  }

  // 10.5) UI 冒烟（--ui）
  await runUi(room, templateId, previewId, players);

  // 10.6) 销毁沙盒预览 + 测试模板
  await api(`/internal/game/preview/${encodeURIComponent(sandboxId)}`, { method: "DELETE" });
  if (typeof sandboxTemplateId === "number") {
    await api(`/internal/gameprops/templates/${sandboxTemplateId}`, { method: "DELETE" });
  }

  // 11) 销毁 + 404
  const removed = await api(`/internal/game/preview/${encodeURIComponent(previewId)}`, {
    method: "DELETE",
  });
  assert(removed.status === 200 && removed.body.existed === true, "销毁预览成功");
  const gone = await api(`/internal/game/preview/${encodeURIComponent(previewId)}`);
  assert(gone.status === 404, `销毁后读取 → 404（实际 ${gone.status}）`);

  // 12) 隔离：真相源表行数不变（模板表只允许新增我们自己的 2 行 + 删除 1 行）
  const dbAfter = dbCounts();
  assert(dbAfter.game_state === dbBefore.game_state, "预览未写 game_state");
  assert(dbAfter.game_events === dbBefore.game_events, "预览未写 game_events");
  assert(dbAfter.game_decision_log === dbBefore.game_decision_log, "预览未写 game_decision_log");
  assert(dbAfter.templates === dbBefore.templates + 1, "模板表仅新增 1 行（副本已删）");
  assert(dbAfter.preview_game_state === 0, "真相源无 preview:* 房间（game_state）");
  assert(dbAfter.preview_game_events === 0, "真相源无 preview:* 事件（game_events）");
  assert(dbAfter.preview_decision_log === 0, "决策日志无 preview:* 行");

  // 12.5) 非法 full snapshot（未知武将）→ 400
  const broken = JSON.parse(JSON.stringify(tpl.body.snapshot));
  broken.data.seats[0].general = "nope";
  broken.data.seats[0].general_name = "不存在";
  const brokenTpl = await api("/internal/gameprops/templates", {
    method: "POST",
    body: { game_id: "sanguosha", name: "e2e-非法快照", snapshot: broken },
  });
  assert(brokenTpl.status === 400, `非法快照模板被拒（${brokenTpl.status}）`);

  // 清理测试模板
  await api(`/internal/gameprops/templates/${templateId}`, { method: "DELETE" });
} catch (e) {
  failed += 1;
  console.error(`[sgs-preview] ❌ ${e?.stack ?? e}`);
} finally {
  if (client) await client.close().catch(() => {});
  cleanup();
}

if (failed > 0) {
  console.error(`\n[sgs-preview] 失败 ${failed} 项`);
  process.exit(1);
}
console.log("\n[sgs-preview] 全部通过 ✅");
