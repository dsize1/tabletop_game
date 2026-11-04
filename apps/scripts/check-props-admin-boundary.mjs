#!/usr/bin/env node
// 中台准入边界（ADR-0031 起口径变更）。
//
// 变更前（P1）：中台**不挂公网**（web 源码不得出现 /props-admin）。
// 变更后（ADR-0031）：`/login` 与 `/props-admin` **同源公网可达**，但必须满足：
//   1. web 的 `/props-admin` 静态托管**前**先过白名单会话门禁（`propsSession`）；
//   2. web 对 `/internal/gameprops/*` 走**代理**且代理前同样过门禁（浏览器不可直连 server）；
//   3. server 仍只监听 127.0.0.1；
//   4. server `/internal/gameprops/*` 的准入必须同时支持「共享 token」与「白名单 actor」；
//   5. 文档写明「白名单准入 / 不直连 / ADR-0031」。
//
// 用法：node apps/scripts/check-props-admin-boundary.mjs

import { readFileSync, existsSync } from "node:fs";

const WEB_SRC = "apps/web/src/index.ts";
const SERVER_MAIN = "crates/server/src/main.rs";
const SERVER_PROPS = "crates/server/src/gameprops_routes.rs";
const DOC = "docs/modules/props-admin.md";

function fail(msg) {
  console.error(`❌ ${msg}`);
  process.exitCode = 1;
}

function read(path) {
  if (!existsSync(path)) {
    fail(`缺少文件 ${path}`);
    return "";
  }
  return readFileSync(path, "utf8");
}

const web = read(WEB_SRC);

// 1) 中台必须由 web 同源托管，且托管前有白名单门禁
if (!web.includes('"/props-admin"')) {
  fail("web 未同源托管 /props-admin（ADR-0031 要求与登录页同源）");
}
if (!web.includes("propsSession")) {
  fail("web 缺少 propsSession 白名单门禁（中台不得无鉴权暴露）");
}
// 门禁必须在静态托管之前（顺序即安全）
const gateAt = web.indexOf("async function propsSession");
const staticAt = web.indexOf('express.static(PROPS_ADMIN_DIR');
if (gateAt < 0 || staticAt < 0 || gateAt > staticAt) {
  fail("props-admin 静态托管未在 propsSession 门禁之后");
}

// 2) 中台 API 只能经 web 代理（门禁 → 代理）
const proxyAt = web.indexOf('app.all("/internal/gameprops/*"');
if (proxyAt < 0) {
  fail("web 缺少 /internal/gameprops/* 代理（浏览器不得直连 server）");
} else if (web.slice(proxyAt, proxyAt + 600).includes("propsSession") === false) {
  fail("中台 API 代理前未调用 propsSession 门禁");
}

// 2.5) 工作台预览：代理 + 页面都必须走 propsSession；预览页不得退回 player token
const previewProxyAt = web.indexOf('app.all(["/internal/game/preview"');
if (previewProxyAt < 0) {
  fail("web 缺少 /internal/game/preview 代理（工作台预览 API 不得由浏览器直连 server）");
} else if (!web.slice(previewProxyAt, previewProxyAt + 1500).includes("propsSession")) {
  fail("预览 API 代理前未调用 propsSession 门禁");
}
const previewPageAt = web.indexOf('app.get("/tabletop_game/:gameId/preview"');
if (previewPageAt < 0) {
  fail("web 缺少 /tabletop_game/:gameId/preview 预览页");
} else {
  const block = web.slice(previewPageAt, previewPageAt + 1800);
  if (!block.includes("propsSession")) {
    fail("预览页未走 propsSession 白名单门禁");
  }
  if (block.includes("resolveGameToken")) {
    fail("预览页不得使用玩家 game token（应由中台会话准入）");
  }
}

// 2.6) server 预览端点同样要求 require_workbench（纵深）
const preview = read("crates/server/src/preview.rs");
for (const needle of ["require_workbench", "preview:", "game_decision_log"]) {
  if (!preview.includes(needle)) {
    fail(`server 预览宿主缺少「${needle}」（隔离/准入/不写决策日志约束）`);
  }
}

// 2.7) 素材静态服务：web 同源托管（内容寻址），server 独占写盘
if (!web.includes('"/game-assets"') || !web.includes("GAMEPROPS_ASSETS_DIR")) {
  fail("web 缺少 /game-assets 静态托管 / GAMEPROPS_ASSETS_DIR（素材由 server 写入、web 同源服务）");
}
if (!read(SERVER_PROPS).includes("props_assets_dir") && !read("crates/server/src/gameprops_assets.rs").includes("props_assets_dir")) {
  fail("server 缺少素材目录（props_assets_dir）");
}

// 3) server 仍只监听回环
const server = read(SERVER_MAIN);
if (!server.includes("127.0.0.1")) {
  fail("server 未找到 127.0.0.1 监听约束");
}
if (/SocketAddr::from\(\(\[0, 0, 0, 0\]/.test(server)) {
  fail("server 监听 0.0.0.0；内部 API 可能被公网访问");
}
// 内部登录接口只有本机可达（web 不代理 /internal/auth）
if (web.includes('"/internal/auth/web/link"')) {
  fail("web 不得代理 /internal/auth/web/link（仅网关本机调用）");
}

// 4) server 侧纵深：共享 token 或白名单 actor 才放行
const props = read(SERVER_PROPS);
for (const needle of ["x-props-admin-token", "x-props-actor", "props_scope"]) {
  if (!props.includes(needle)) {
    fail(`server 中台准入缺少 ${needle}（需同时支持共享 token 与白名单 actor）`);
  }
}

// 4.5) 工作台文档/ADR 与门禁入口（静态，避免文档漂移）
const adr = read("memory_palace/decisions/ADR-0033-workbench-preview-base.md");
if (!adr.includes("预览隔离")) {
  fail("缺少 ADR-0033（工作台基座与预览隔离）或口径不一致");
}
const adrIndex = read("memory_palace/decisions/README.md");
if (!adrIndex.includes("ADR-0033")) {
  fail("decisions/README.md 未登记 ADR-0033");
}
const sandboxAdr = read("memory_palace/decisions/ADR-0034-workbench-sandbox-game.md");
if (!sandboxAdr.includes("沙盒开局") || !sandboxAdr.includes("finish_setup")) {
  fail("缺少 ADR-0034（工作台创建对局·沙盒开局）或口径不一致");
}
if (!adrIndex.includes("ADR-0034")) {
  fail("decisions/README.md 未登记 ADR-0034");
}
const scopeAdr = read("memory_palace/decisions/ADR-0035-test-player-whitelist.md");
if (!scopeAdr.includes("TEST_PLAYER_WHITELIST") || !scopeAdr.includes("workbench")) {
  fail("缺少 ADR-0035（测试玩家白名单/工作台最小权限）或口径不一致");
}
if (!adrIndex.includes("ADR-0035")) {
  fail("decisions/README.md 未登记 ADR-0035");
}
const previewRoutes = read("crates/server/src/preview.rs");
const templateRoutes = read("crates/server/src/template_routes.rs");
const serverMain = read("crates/server/src/main.rs");
if (!serverMain.includes("TEST_PLAYER_WHITELIST")) {
  fail("server 未读取 TEST_PLAYER_WHITELIST");
}
if (!previewRoutes.includes("require_workbench") || !templateRoutes.includes("require_workbench")) {
  fail("预览/模板 API 未使用 require_workbench（测试玩家需要工作台权限）");
}
if (!web.includes("isWorkbenchOnlyAllowed") || !web.includes("props_scope")) {
  fail("web 缺少工作台路径门禁/props_scope 判定");
}
const pkg = JSON.parse(read("package.json") || "{}");
if (!pkg.scripts?.["check:sgs-preview"]) {
  fail("package.json 缺少 check:sgs-preview 门禁入口");
}
if (!pkg.scripts?.["check:sgs-preview:ui"]) {
  fail("package.json 缺少 check:sgs-preview:ui（UI 冒烟）入口");
}

// 5) 文档口径
const doc = read(DOC);
for (const needle of ["白名单", "不直连", "ADR-0031", "ADR-0033"]) {
  if (!doc.includes(needle)) {
    fail(`props-admin 文档缺少「${needle}」边界说明`);
  }
}

if (process.exitCode) {
  console.error("中台准入边界检查未通过");
} else {
  console.log("✅ 中台准入边界检查通过（同源公网 + 白名单准入 + 内部 API 不直连）");
}
