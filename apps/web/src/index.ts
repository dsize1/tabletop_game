// web：Express 独立站点（BFF）。
//
// 路由：
// - GET  /dev_task/:sessionId?token=xxx         计划文档页（校验 token → 拉 markdown → 渲染 HTML）
// - GET  /tabletop_game/:gameId/player/:openid  游戏资源页（按引擎渲染；三国杀见 view/sgs）
// - GET  /tabletop_game/:gameId/state           状态 JSON（轮询）
// - POST /tabletop_game/:gameId/action          **动作提交**（JSON=fetch；form=无 JS 表单 → 303）
// - GET  /health                                健康检查（cloudflared 探活用）
//
// 服务端调 server（内部 127.0.0.1）鉴权 + 拉取；浏览器不直连 server / SQLite。

import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import express from "express";
import {
  renderDocsTree,
  renderErrorPage,
  renderMarkdown,
  renderPage,
  type DocsRootTree,
  type GameView,
} from "./render.js";
import { renderAppPage } from "./view/page.js";
import { renderSgsPage } from "./view/sgs/page.js";
import { renderLoginPage } from "./view/login.js";
import type { SgsAssetMap } from "./view/sgs/types.js";

const SERVER_URL = process.env.SERVER_URL ?? "http://127.0.0.1:3000";
const PORT = Number(process.env.WEB_PORT ?? process.env.PORT ?? 18080);
/** web → server 的中台内部凭证（与 server `PROPS_ADMIN_TOKEN` 一致；未配置 = 本机放行）。 */
const PROPS_ADMIN_TOKEN = (process.env.PROPS_ADMIN_TOKEN ?? "").trim();
/** 中台构建产物目录（同源托管；ADR-0031）。 */
const PROPS_ADMIN_DIR = fileURLToPath(new URL("../../props-admin/dist/", import.meta.url));

// cloudflared 反代后源站只见回环，真实 IP 取 CF-Connecting-IP / X-Forwarded-For
const app = express();
app.set("trust proxy", true);
// 动作提交：fetch 用 JSON、无 JS 表单用 urlencoded，两者都要解析。
// 工作台模板导入可能带完整 GameState（约 30–100KB），故放宽到 1MB（仅白名单/本机可达）。
app.use(express.json({ limit: "1mb" }));
app.use(express.urlencoded({ extended: false, limit: "16kb" }));

// 静态资源（同域，走 Cloudflare 边缘缓存）
// - /board-assets：版图素材，文件名带内容哈希 → immutable 长缓存
// - /assets：页面 runtime bundle，未做哈希 → 短缓存
const PUBLIC_DIR = fileURLToPath(new URL("../public/", import.meta.url));
/** 中台素材目录（server 写入，web 同源静态服务；ADR-0036）。 */
const GAMEPROPS_ASSETS_DIR = process.env.GAMEPROPS_ASSETS_DIR
  ? path.resolve(process.env.GAMEPROPS_ASSETS_DIR)
  : fileURLToPath(new URL("../../../data/gameprops/assets/", import.meta.url));
app.use(
  "/board-assets",
  express.static(path.join(PUBLIC_DIR, "board"), { immutable: true, maxAge: "365d", fallthrough: true }),
);
app.use(
  "/assets",
  express.static(path.join(PUBLIC_DIR, "assets"), {
    etag: true,
    maxAge: 0,
    setHeaders: (res) => res.setHeader("Cache-Control", "no-cache"),
  }),
);
// 中台素材（内容寻址文件名 → immutable；公开可取，无法枚举）
app.use(
  "/game-assets",
  express.static(GAMEPROPS_ASSETS_DIR, { immutable: true, maxAge: "365d", fallthrough: true }),
);

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

// 开发文档预览：单 token 覆盖全部根（与 docs-mcp / server 约定一致）
const DOCS_RESOURCE = "docs:all";

interface VerifyResult {
  valid: boolean;
  openid?: string;
  resource?: string;
}

/** 服务端校验 token；401 → invalid，其余错误抛出。 */
async function verifyToken(token: string): Promise<VerifyResult> {
  const r = await fetch(`${SERVER_URL}/auth/verify`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token }),
  });
  if (r.status === 401) return { valid: false };
  if (!r.ok) throw new Error(`server /auth/verify ${r.status}`);
  const body = (await r.json()) as { openid?: string; resource?: string };
  return { valid: true, openid: body.openid, resource: body.resource };
}

/** 读取 Cookie（不引入 cookie-parser，只取首个同名值）。 */
function cookieValue(header: string | undefined, name: string): string | undefined {
  if (!header) return undefined;
  for (const part of header.split(";")) {
    const i = part.indexOf("=");
    if (i < 0) continue;
    if (part.slice(0, i).trim() !== name) continue;
    const raw = part.slice(i + 1).trim();
    try {
      return decodeURIComponent(raw);
    } catch {
      return raw;
    }
  }
  return undefined;
}

/** 写 HttpOnly 会话 Cookie（正式 token 只经此下发，ADR-0031）。 */
function setAuthCookie(
  res: import("express").Response,
  name: string,
  token: string,
  maxAgeSecs: number,
  secure: boolean,
): void {
  const parts = [
    `${name}=${encodeURIComponent(token)}`,
    "Path=/",
    "HttpOnly",
    "SameSite=Lax",
    `Max-Age=${Math.max(1, Math.floor(maxAgeSecs))}`,
  ];
  if (secure) parts.push("Secure");
  res.append("Set-Cookie", parts.join("; "));
}

/**
 * 解析游戏 token：显式 `?token=`（内部脚本/兼容）→ Cookie（玩家登录态）。
 * 新流程下玩家链接只带短效 `t`，游戏页靠 Cookie。
 */
function resolveGameToken(req: import("express").Request, bodyToken?: unknown): string {
  const q = typeof req.query.token === "string" ? req.query.token : "";
  if (q) return q;
  if (typeof bodyToken === "string" && bodyToken.trim()) return bodyToken.trim();
  return cookieValue(req.headers.cookie, "tg_game") ?? "";
}

interface Whoami {
  openid: string;
  resource: string;
  props_whitelisted: boolean;
  /** `admin`（全部菜单） / `workbench`（仅工作台，ADR-0035）。 */
  props_scope?: "admin" | "workbench" | null;
  props_menus?: string[];
}

/** 会话自检（server `/auth/web/whoami`）；失效 → undefined。 */
async function whoami(token: string): Promise<Whoami | undefined> {
  const r = await fetch(
    `${SERVER_URL}/auth/web/whoami?token=${encodeURIComponent(token)}`,
  );
  if (!r.ok) return undefined;
  return (await r.json()) as Whoami;
}

/** 请求自带的 Cookie 是否仍是该 openid 的有效会话（`used` 链接判断能否直接进入）。 */
async function sessionMatches(
  req: import("express").Request,
  openid: string,
): Promise<boolean> {
  for (const name of ["tg_game", "tg_props"]) {
    const token = cookieValue(req.headers.cookie, name);
    if (!token) continue;
    const v = await verifyToken(token);
    if (v.valid && v.openid === openid) return true;
  }
  return false;
}

/** 拉取计划文档 markdown；404 → null。 */
async function fetchPlan(sessionId: string, token: string): Promise<string | null> {
  const r = await fetch(
    `${SERVER_URL}/resources/dev_task/${encodeURIComponent(sessionId)}/plan?token=${encodeURIComponent(token)}`,
  );
  if (r.status === 404) return null;
  if (!r.ok) throw new Error(`server plan ${r.status}`);
  return await r.text();
}

/** 拉取游戏资源视图（server `/resources/game/{gameId}`）；404 → null。 */
async function fetchGameView(gameId: string, token: string): Promise<GameView | null> {
  const r = await fetch(
    `${SERVER_URL}/resources/game/${encodeURIComponent(gameId)}?token=${encodeURIComponent(token)}`,
  );
  if (r.status === 404) return null;
  if (!r.ok) throw new Error(`server game ${r.status}`);
  return (await r.json()) as GameView;
}

/**
 * 全 AI 局面兜底续推（P6.3 真机）：玩家阵亡后若场上只剩机器人，server 的单次推进
 * 可能停在上限（200 步）而没有任何真人动作再触发它。页面轮询 `/state` 时顺带
 * fire-and-forget 调一次内部 `/drive`，让闲置房间继续跑完（server 侧有房间锁）。
 */
function maybeResumeBots(view: GameView): void {
  if (view.game_id !== "sanguosha") return;
  const data = view.public?.data as
    | { phase?: string; public_board?: { seats?: Array<{ alive?: boolean; is_bot?: boolean }> } }
    | undefined;
  const board = data?.public_board;
  if (!board || data?.phase === "finished") return;
  const alive = (board.seats ?? []).filter((seat) => seat.alive);
  if (alive.length === 0 || !alive.every((seat) => seat.is_bot)) return;
  void fetch(
    `${SERVER_URL}/internal/game/${encodeURIComponent(view.game_id)}/${encodeURIComponent(view.room_id)}/drive`,
    { method: "POST" },
  ).catch(() => undefined);
}

/** 武将卡面映射缓存（60s）：server `/internal/gameprops/asset-map`。 */
let assetMapCache: { at: number; map: SgsAssetMap } | null = null;
const ASSET_MAP_TTL_MS = 60_000;

async function fetchAssetMap(): Promise<SgsAssetMap> {
  const now = Date.now();
  if (assetMapCache && now - assetMapCache.at < ASSET_MAP_TTL_MS) return assetMapCache.map;
  try {
    const headers: Record<string, string> = {};
    if (PROPS_ADMIN_TOKEN) headers["x-props-admin-token"] = PROPS_ADMIN_TOKEN;
    const r = await fetch(
      `${SERVER_URL}/internal/gameprops/asset-map?game_id=sanguosha&kind=general`,
      { headers },
    );
    if (!r.ok) throw new Error(`asset-map ${r.status}`);
    const body = (await r.json()) as { assets?: SgsAssetMap };
    const map = body.assets ?? {};
    assetMapCache = { at: now, map };
    return map;
  } catch {
    // 降级：沿用上一次成功结果（无则空 map → 前端显示占位）
    return assetMapCache?.map ?? {};
  }
}

/** 提交动作到 server 内部执行接口（仅本机）。 */
async function submitGameAction(
  gameId: string,
  roomId: string,
  openid: string,
  action: string,
): Promise<{ status: number; message: string }> {
  const r = await fetch(
    `${SERVER_URL}/internal/game/${encodeURIComponent(gameId)}/${encodeURIComponent(roomId)}/action`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ player_id: openid, action }),
    },
  );
  if (r.ok) {
    const body = (await r.json()) as { message?: string };
    return { status: 200, message: body.message ?? "已提交" };
  }
  const known = [400, 403, 404].includes(r.status) ? r.status : 503;
  const messages: Record<number, string> = {
    400: "动作不合法（可能已轮到他人或选项已变化）",
    403: "当前不能由你执行该动作",
    404: "对局不存在或已结束",
  };
  return { status: known, message: messages[known] ?? "服务暂时不可用" };
}

/** 拉取开发文档树 JSON（全部根）。 */
async function fetchDocsTree(token: string): Promise<DocsRootTree[]> {
  const r = await fetch(
    `${SERVER_URL}/resources/docs/tree?token=${encodeURIComponent(token)}`,
  );
  if (!r.ok) throw new Error(`server docs tree ${r.status}`);
  return (await r.json()) as DocsRootTree[];
}

/** 拉取 .md 文件内容；404 → null。 */
async function fetchDocsFile(root: string, filePath: string, token: string): Promise<string | null> {
  const url =
    `${SERVER_URL}/resources/docs/file?token=${encodeURIComponent(token)}` +
    `&root=${encodeURIComponent(root)}&path=${encodeURIComponent(filePath)}`;
  const r = await fetch(url);
  if (r.status === 404) return null;
  if (!r.ok) throw new Error(`server docs file ${r.status}`);
  return await r.text();
}

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

app.get("/dev_task/:sessionId", async (req, res) => {
  const { sessionId } = req.params;
  const token = typeof req.query.token === "string" ? req.query.token : "";

  if (!UUID_RE.test(sessionId)) {
    res.status(400).send(renderErrorPage(400, "非法任务标识"));
    return;
  }
  if (!token) {
    res.status(401).send(renderErrorPage(401, "缺少访问 token"));
    return;
  }

  try {
    const v = await verifyToken(token);
    if (!v.valid) {
      res.status(401).send(renderErrorPage(401, "链接已失效或无效，请重新获取"));
      return;
    }
    if (v.resource !== `dev_task:${sessionId}`) {
      res.status(403).send(renderErrorPage(403, "无权访问该资源"));
      return;
    }

    const md = await fetchPlan(sessionId, token);
    if (md === null) {
      res.status(404).send(renderErrorPage(404, "计划文档不存在或尚未生成"));
      return;
    }
    res.send(renderPage(`任务 ${sessionId}`, renderMarkdown(md)));
  } catch (err) {
    console.error("[web] 拉取计划文档失败:", err);
    res.status(503).send(renderErrorPage(503, "服务暂时不可用，请稍后重试"));
  }
});

app.get("/dev_docs", async (req, res) => {
  const token = typeof req.query.token === "string" ? req.query.token : "";
  if (!token) {
    res.status(401).send(renderErrorPage(401, "缺少访问 token"));
    return;
  }
  try {
    const v = await verifyToken(token);
    if (!v.valid) {
      res.status(401).send(renderErrorPage(401, "链接已失效或无效，请重新获取"));
      return;
    }
    if (v.resource !== DOCS_RESOURCE) {
      res.status(403).send(renderErrorPage(403, "无权访问该资源"));
      return;
    }
    const roots = await fetchDocsTree(token);
    res.send(renderPage("开发文档", `<div class="tree">${renderDocsTree(roots, token)}</div>`));
  } catch (err) {
    console.error("[web] 拉取文档树失败:", err);
    res.status(503).send(renderErrorPage(503, "服务暂时不可用，请稍后重试"));
  }
});

app.get("/dev_docs/file", async (req, res) => {
  const token = typeof req.query.token === "string" ? req.query.token : "";
  const root = typeof req.query.root === "string" ? req.query.root : "";
  const filePath = typeof req.query.path === "string" ? req.query.path : "";
  if (!token) {
    res.status(401).send(renderErrorPage(401, "缺少访问 token"));
    return;
  }
  if (!root || !filePath) {
    res.status(400).send(renderErrorPage(400, "缺少文件参数"));
    return;
  }
  try {
    const v = await verifyToken(token);
    if (!v.valid) {
      res.status(401).send(renderErrorPage(401, "链接已失效或无效，请重新获取"));
      return;
    }
    if (v.resource !== DOCS_RESOURCE) {
      res.status(403).send(renderErrorPage(403, "无权访问该资源"));
      return;
    }
    const content = await fetchDocsFile(root, filePath, token);
    if (content === null) {
      res.status(404).send(renderErrorPage(404, "文件不存在或不可预览"));
      return;
    }
    const name = filePath.split("/").pop() ?? filePath;
    const crumb =
      `<div class="breadcrumb"><a href="/dev_docs?token=${encodeURIComponent(token)}">← 文档树</a>` +
      ` · ${root}/${filePath}</div>`;
    res.send(renderPage(name, crumb + renderMarkdown(content)));
  } catch (err) {
    console.error("[web] 拉取文档文件失败:", err);
    res.status(503).send(renderErrorPage(503, "服务暂时不可用，请稍后重试"));
  }
});

app.get("/tabletop_game/:gameId/player/:openid", async (req, res) => {
  // 新流程（ADR-0031）：玩家链接 → 登录页 → HttpOnly Cookie；此处 Cookie 优先兜底。
  const token = resolveGameToken(req);
  if (!token) {
    res
      .status(401)
      .send(
        renderErrorPage(
          401,
          "缺少访问凭证：请在群内发送「#网页」获取登录链接（旧链接可能已过期）",
        ),
      );
    return;
  }
  try {
    const v = await verifyToken(token);
    if (!v.valid) {
      res.status(401).send(renderErrorPage(401, "链接已失效或无效，请重新获取"));
      return;
    }
    const view = await fetchGameView(req.params.gameId, token);
    if (view === null) {
      res.status(404).send(renderErrorPage(404, "对局不存在或已结束"));
      return;
    }
    res.setHeader("Cache-Control", "no-store");
    // 引擎分派（P3）：三国杀走三分区交互页；其余引擎沿用双模式版图/文本。
    const data = (view.player?.data ?? view.public?.data) as { game?: string } | undefined;
    if (data?.game === "sanguosha") {
      res.send(
        renderSgsPage(view, {
          gameId: req.params.gameId,
          token,
          openid: v.openid ?? req.params.openid,
          roomId: view.room_id,
          assets: await fetchAssetMap(),
        }),
      );
      return;
    }
    const mode = req.query.view === "text" ? "text" : "board";
    res.send(renderAppPage(view, { gameId: req.params.gameId, token, mode }));
  } catch (err) {
    console.error("[web] 拉取游戏资源失败:", err);
    res.status(503).send(renderErrorPage(503, "服务暂时不可用，请稍后重试"));
  }
});

/**
 * 状态 JSON（页面轮询用）。
 *
 * 鉴权与原页面完全一致：缺 token 401、无效 401、不存在 404、server 异常 503。
 * 不缓存（token 在 query 中）。
 */
app.get("/tabletop_game/:gameId/state", async (req, res) => {
  const token = resolveGameToken(req);
  res.setHeader("Cache-Control", "no-store");
  if (!token) {
    res.status(401).json({ error: "missing_token" });
    return;
  }
  try {
    const v = await verifyToken(token);
    if (!v.valid) {
      res.status(401).json({ error: "invalid_token" });
      return;
    }
    const view = await fetchGameView(req.params.gameId, token);
    if (view === null) {
      res.status(404).json({ error: "not_found" });
      return;
    }
    maybeResumeBots(view);
    res.json({ ...view, server_time: new Date().toISOString() });
  } catch (err) {
    console.error("[web] 拉取状态失败:", err);
    res.status(503).json({ error: "upstream_unavailable" });
  }
});

/**
 * 动作提交（P3）：浏览器 → web（短 token）→ server `/internal/game/.../action`。
 *
 * - token 资源范围必须等于 `game:{room_id}`（越权 403）；
 * - fetch 请求（`Accept: application/json`）返回 JSON；普通表单 303 回页面（PRG，防重复提交）；
 * - 错误语义：400 非法动作 / 401 token 无效 / 403 越权 / 404 对局不存在 / 503 上游不可用。
 */
app.post("/tabletop_game/:gameId/action", async (req, res) => {
  const body = (req.body ?? {}) as Record<string, unknown>;
  const token = resolveGameToken(req, body.token);
  const roomId = typeof body.room_id === "string" ? body.room_id : "";
  const action = typeof body.action === "string" ? body.action.trim() : "";
  const wantsJson = (req.get("accept") ?? "").includes("application/json");
  const fail = (status: number, message: string) => {
    if (wantsJson) res.status(status).json({ ok: false, error: message });
    else res.status(status).send(renderErrorPage(status, message));
  };
  if (!token) return fail(401, "缺少访问 token");
  if (!roomId || !action) return fail(400, "缺少 room_id 或 action");
  try {
    const v = await verifyToken(token);
    if (!v.valid || !v.openid) return fail(401, "链接已失效或无效，请重新获取");
    if (v.resource !== `game:${roomId}`) return fail(403, "无权操作该对局");
    const out = await submitGameAction(req.params.gameId, roomId, v.openid, action);
    if (out.status !== 200) return fail(out.status, out.message);
    if (wantsJson) {
      res.json({ ok: true, message: out.message });
      return;
    }
    // 无 JS 表单：POST → 303 回页面（浏览器会自动 GET 刷新）
    // Cookie 会话不再把 token 拼进 URL（ADR-0031）；显式 `?token=` 通道仍兼容内部脚本。
    const explicit = typeof req.query.token === "string" ? req.query.token : "";
    const back =
      `/tabletop_game/${encodeURIComponent(req.params.gameId)}` +
      `/player/${encodeURIComponent(v.openid)}` +
      `?${explicit ? `token=${encodeURIComponent(explicit)}&` : ""}msg=${encodeURIComponent(out.message)}`;
    res.redirect(303, back);
  } catch (err) {
    console.error("[web] 提交动作失败:", err);
    fail(503, "服务暂时不可用，请稍后重试");
  }
});


// ==================== 工作台预览（P5 基座，ADR-0032） ====================

/** 预览页/代理统一的服务端请求头（web → server，浏览器不直连）。 */
function propsHeaders(who: Whoami): Record<string, string> {
  const headers: Record<string, string> = {
    "content-type": "application/json",
    "x-props-actor": who.openid,
  };
  if (PROPS_ADMIN_TOKEN) headers["x-props-admin-token"] = PROPS_ADMIN_TOKEN;
  return headers;
}

/** 预览页 SSR：props 会话白名单准入，不要求 game token。 */
app.get("/tabletop_game/:gameId/preview", async (req, res) => {
  res.setHeader("Cache-Control", "no-store");
  const room = typeof req.query.room === "string" ? req.query.room.trim() : "";
  const viewer = typeof req.query.viewer === "string" ? req.query.viewer.trim() : "";
  if (!room) {
    res.status(400).send(renderErrorPage(400, "缺少预览会话 room"));
    return;
  }
  try {
    const who = await propsSession(req).catch(() => undefined);
    if (!who) {
      res
        .status(403)
        .send(renderErrorPage(403, "预览需要中台登录与白名单权限：请在 QQ 群发送「#中台」"));
      return;
    }
    const qs = new URLSearchParams({ viewer });
    const r = await fetch(
      `${SERVER_URL}/internal/game/preview/${encodeURIComponent(room)}?${qs.toString()}`,
      { headers: propsHeaders(who) },
    );
    if (!r.ok) {
      const text = await r.text().catch(() => "");
      const msg = r.status === 404 ? "预览会话不存在或已过期" : `预览读取失败（${r.status}）`;
      res.status(r.status).send(renderErrorPage(r.status, text ? `${msg}：${text}` : msg));
      return;
    }
    const view = (await r.json()) as GameView & {
      seats?:
        | Array<{ seat: number; player_id: string; general_name?: string }>
        | { seats?: Array<{ seat: number; player_id: string; general_name?: string }> };
    };
    type SeatInfo = { seat: number; player_id: string; general_name?: string };
    const boardSeats = (
      (view.player?.data ?? view.public?.data) as
        | { public_board?: { seats?: SeatInfo[] } }
        | undefined
    )?.public_board?.seats;
    const seats: SeatInfo[] = Array.isArray(view.seats)
      ? view.seats
      : Array.isArray(view.seats?.seats)
        ? view.seats.seats
        : (boardSeats ?? []);
    res.send(
      renderSgsPage(view, {
        gameId: req.params.gameId,
        token: "",
        openid: viewer || who.openid,
        roomId: room,
        preview: {
          returnUrl: "/props-admin/workbench",
          seats: seats.map((s) => ({
            seat: Number(s.seat ?? 0),
            player_id: String(s.player_id ?? ""),
            general_name: s.general_name,
          })),
        },
        assets: await fetchAssetMap(),
      }),
    );
  } catch (err) {
    console.error("[web] 预览页加载失败:", err);
    res.status(503).send(renderErrorPage(503, "服务暂时不可用，请稍后重试"));
  }
});

/** 预览 API 代理：state / action / drive / reset / delete。 */
for (const [suffix, method] of [
  ["state", "GET"],
  ["action", "POST"],
  ["drive", "POST"],
  ["reset", "POST"],
] as const) {
  app.all(`/tabletop_game/:gameId/preview/${suffix}`, async (req, res) => {
    res.setHeader("Cache-Control", "no-store");
    if (req.method.toUpperCase() !== method) {
      res.status(405).json({ error: "method_not_allowed" });
      return;
    }
    const who = await propsSession(req).catch(() => undefined);
    if (!who) {
      res.status(401).json({ error: "需要中台登录（群内发 #中台）" });
      return;
    }
    const room =
      (typeof req.query.room === "string" && req.query.room.trim()) ||
      (typeof req.body?.room === "string" && req.body.room.trim()) ||
      "";
    if (!room) {
      res.status(400).json({ error: "missing_room" });
      return;
    }
    try {
      const upstreamPath =
        suffix === "state"
          ? `${SERVER_URL}/internal/game/preview/${encodeURIComponent(room)}?${new URLSearchParams({
              viewer: typeof req.query.viewer === "string" ? req.query.viewer : "",
            }).toString()}`
          : `${SERVER_URL}/internal/game/preview/${encodeURIComponent(room)}/${suffix}`;
      const body =
        suffix === "state"
          ? undefined
          : JSON.stringify({
              ...(req.body ?? {}),
              room: undefined,
            });
      const r = await fetch(upstreamPath, {
        method,
        headers: propsHeaders(who),
        body,
      });
      const text = await r.text();
      res.status(r.status).type("application/json").send(text);
    } catch (err) {
      console.error(`[web] 预览 ${suffix} 代理失败:`, err);
      res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
    }
  });
}

app.delete("/tabletop_game/:gameId/preview", async (req, res) => {
  res.setHeader("Cache-Control", "no-store");
  const who = await propsSession(req).catch(() => undefined);
  if (!who) {
    res.status(401).json({ error: "需要中台登录（群内发 #中台）" });
    return;
  }
  const room = typeof req.query.room === "string" ? req.query.room.trim() : "";
  if (!room) {
    res.status(400).json({ error: "missing_room" });
    return;
  }
  try {
    const r = await fetch(`${SERVER_URL}/internal/game/preview/${encodeURIComponent(room)}`, {
      method: "DELETE",
      headers: propsHeaders(who),
    });
    const text = await r.text();
    res.status(r.status).type("application/json").send(text);
  } catch (err) {
    console.error("[web] 预览销毁失败:", err);
    res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
  }
});

// 预览相关内部 API 也经 web 代理（生产同源；dev 由 Vite 直连本机 server）。
app.all(["/internal/game/preview", "/internal/game/preview/*"], async (req, res) => {
  res.setHeader("Cache-Control", "no-store");
  const who = await propsSession(req).catch(() => undefined);
  if (!who) {
    res.status(401).json({ error: "需要中台登录（群内发 #中台）" });
    return;
  }
  try {
    const method = req.method.toUpperCase();
    const r = await fetch(`${SERVER_URL}${req.originalUrl}`, {
      method,
      headers: propsHeaders(who),
      body: method === "GET" || method === "HEAD" ? undefined : JSON.stringify(req.body ?? {}),
    });
    const text = await r.text();
    res.status(r.status).type("application/json").send(text);
  } catch (err) {
    console.error("[web] 预览内部 API 代理失败:", err);
    res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
  }
});

// ==================== Web 跨端登录（ADR-0031） ====================

/** 登录页：短效 token 只在 URL，页面不落 localStorage。 */
app.get("/login", (req, res) => {
  const t = typeof req.query.t === "string" ? req.query.t.trim() : "";
  res.setHeader("Cache-Control", "no-store");
  res.setHeader("Referrer-Policy", "no-referrer");
  res.send(renderLoginPage({ token: t }));
});

/** 页面状态代理（同时判定浏览器是否已有可用的正式会话 Cookie）。 */
app.get("/login/status", async (req, res) => {
  const t = typeof req.query.t === "string" ? req.query.t : "";
  res.setHeader("Cache-Control", "no-store");
  try {
    const r = await fetch(`${SERVER_URL}/auth/web/status?token=${encodeURIComponent(t)}`);
    const body = (await r.json().catch(() => ({}))) as Record<string, unknown>;
    if (!r.ok) {
      res.status(r.status).json(body);
      return;
    }
    const openid = typeof body.openid === "string" ? body.openid : "";
    res.json({ ...body, has_session: openid ? await sessionMatches(req, openid) : false });
  } catch (err) {
    console.error("[web] 登录状态查询失败:", err);
    res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
  }
});

/** 注册码提交代理（注册 / 登录）。 */
for (const kind of ["register", "login"] as const) {
  app.post(`/login/${kind}`, async (req, res) => {
    const body = (req.body ?? {}) as Record<string, unknown>;
    res.setHeader("Cache-Control", "no-store");
    try {
      const r = await fetch(`${SERVER_URL}/auth/web/${kind}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          token: typeof body.t === "string" ? body.t : "",
          passcode: typeof body.passcode === "string" ? body.passcode : "",
          passcode_confirm:
            typeof body.passcode_confirm === "string" ? body.passcode_confirm : undefined,
        }),
      });
      const text = await r.text();
      res.status(r.status).type("application/json").send(text);
    } catch (err) {
      console.error(`[web] 登录代理失败(${kind}):`, err);
      res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
    }
  });
}

/**
 * 确认长轮询代理。
 *
 * 确认成功时由 web 写 **HttpOnly** Cookie（正式 8h token），响应体里**剔除** token，
 * 页面只拿到 `{status:"confirmed", target}` 后跳转；这样 token 不落在页面/URL/JS 可读处。
 */
app.get("/login/confirm", async (req, res) => {
  const t = typeof req.query.t === "string" ? req.query.t : "";
  res.setHeader("Cache-Control", "no-store");
  try {
    const r = await fetch(
      `${SERVER_URL}/auth/web/confirm?token=${encodeURIComponent(t)}`,
    );
    const body = (await r.json().catch(() => ({}))) as Record<string, unknown>;
    if (!r.ok) {
      res.status(r.status).json(body);
      return;
    }
    if (body.status === "confirmed") {
      const token = typeof body.token === "string" ? body.token : "";
      const cookie = body.cookie === "tg_props" ? "tg_props" : "tg_game";
      const maxAge = Number(body.expires_in_secs ?? 8 * 3600);
      if (token) setAuthCookie(res, cookie, token, maxAge, req.secure);
      delete body.token;
      delete body.cookie;
      res.json(body);
      return;
    }
    if (body.status === "used") {
      const openid = typeof body.openid === "string" ? body.openid : "";
      res.json({ ...body, has_session: openid ? await sessionMatches(req, openid) : false });
      return;
    }
    res.json(body);
  } catch (err) {
    console.error("[web] 登录确认轮询失败:", err);
    res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
  }
});

// ==================== 中台（同源托管 + 白名单准入，ADR-0031） ====================

/** 中台页面准入：Cookie 会话 + server 白名单判定（双重校验，server 侧也会再判）。 */
async function propsSession(
  req: import("express").Request,
): Promise<Whoami | undefined> {
  const token = cookieValue(req.headers.cookie, "tg_props");
  if (!token) return undefined;
  const who = await whoami(token);
  if (!who || !who.props_whitelisted) return undefined;
  // 兼容旧 server 响应（无 props_scope）：按 Admin 处理，避免升级顺序导致误 401。
  if (!who.props_scope) who.props_scope = "admin";
  return who;
}

/** 测试玩家（workbench scope）只允许的静态/SPA 路径（ADR-0035）。 */
function isWorkbenchOnlyAllowed(url: string): boolean {
  const path = (url.split("?")[0] ?? "").replace(/\/+$/, "") || "/";
  if (path === "/props-admin/workbench") return true;
  if (path.startsWith("/props-admin/assets/")) return true;
  if (path === "/props-admin/index.html" || path === "/props-admin/favicon.ico") return true;
  return false;
}

app.use(
  "/props-admin",
  async (req, res, next) => {
    res.setHeader("Cache-Control", "no-store");
    const who = await propsSession(req).catch(() => undefined);
    if (!who) {
      res
        .status(403)
        .send(
          renderErrorPage(
            403,
            "中台需要登录与白名单权限：请在 QQ 群发送「#中台」获取登录链接",
          ),
        );
      return;
    }
    if (who.props_scope !== "admin" && !isWorkbenchOnlyAllowed(req.originalUrl)) {
      res
        .status(401)
        .send(
          renderErrorPage(
            401,
            "当前账号仅可访问「工作台」菜单：请从工作台入口进入",
          ),
        );
      return;
    }
    next();
  },
  express.static(PROPS_ADMIN_DIR, {
    index: "index.html",
    setHeaders: (res) => res.setHeader("Cache-Control", "no-store"),
  }),
);

/** SPA 回退（Vue Router history 模式）。 */
app.get("/props-admin/*", async (req, res) => {
  const who = await propsSession(req).catch(() => undefined);
  if (!who) {
    res.status(403).send(renderErrorPage(403, "中台需要登录与白名单权限"));
    return;
  }
  if (who.props_scope !== "admin" && !isWorkbenchOnlyAllowed(req.originalUrl)) {
    res
      .status(401)
      .send(renderErrorPage(401, "当前账号仅可访问「工作台」菜单：请从工作台入口进入"));
    return;
  }
  res.setHeader("Cache-Control", "no-store");
  res.sendFile(path.join(PROPS_ADMIN_DIR, "index.html"));
});

/** 中台 API 代理：浏览器只与 web 同源通信；server 侧再校验白名单/凭据。 */
app.all("/internal/gameprops/*", async (req, res) => {
  res.setHeader("Cache-Control", "no-store");
  const who = await propsSession(req).catch(() => undefined);
  if (!who) {
    res.status(401).json({ error: "需要中台登录（群内发 #中台）" });
    return;
  }
  try {
    const headers: Record<string, string> = {
      "content-type": req.get("content-type") ?? "application/json",
      "x-props-actor": who.openid,
    };
    if (PROPS_ADMIN_TOKEN) headers["x-props-admin-token"] = PROPS_ADMIN_TOKEN;
    const method = req.method.toUpperCase();
    const r = await fetch(`${SERVER_URL}${req.originalUrl}`, {
      method,
      headers,
      body: method === "GET" || method === "HEAD" ? undefined : JSON.stringify(req.body ?? {}),
    });
    const text = await r.text();
    res.status(r.status).type("application/json").send(text);
  } catch (err) {
    console.error("[web] 中台 API 代理失败:", err);
    res.status(503).json({ error: "服务暂时不可用，请稍后重试" });
  }
});

app.listen(PORT, () => {
  console.log(`[web] 监听 http://127.0.0.1:${PORT}（server=${SERVER_URL}）`);
});
