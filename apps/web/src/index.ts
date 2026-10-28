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

const SERVER_URL = process.env.SERVER_URL ?? "http://127.0.0.1:3000";
const PORT = Number(process.env.WEB_PORT ?? process.env.PORT ?? 18080);

// cloudflared 反代后源站只见回环，真实 IP 取 CF-Connecting-IP / X-Forwarded-For
const app = express();
app.set("trust proxy", true);
// 动作提交：fetch 用 JSON、无 JS 表单用 urlencoded，两者都要解析
app.use(express.json({ limit: "16kb" }));
app.use(express.urlencoded({ extended: false, limit: "16kb" }));

// 静态资源（同域，走 Cloudflare 边缘缓存）
// - /board-assets：版图素材，文件名带内容哈希 → immutable 长缓存
// - /assets：页面 runtime bundle，未做哈希 → 短缓存
const PUBLIC_DIR = fileURLToPath(new URL("../public/", import.meta.url));
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
    const view = await fetchGameView(req.params.gameId, token);
    if (view === null) {
      res.status(404).send(renderErrorPage(404, "对局不存在或已结束"));
      return;
    }
    // 双模式页（版图默认 / 文本可切；?view=text 仅用于首屏直出旧行为）
    const mode = req.query.view === "text" ? "text" : "board";
    res.setHeader("Cache-Control", "no-store");
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
  const token = typeof req.query.token === "string" ? req.query.token : "";
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
  const token =
    (typeof req.query.token === "string" ? req.query.token : "") ||
    (typeof body.token === "string" ? body.token : "");
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
    const back =
      `/tabletop_game/${encodeURIComponent(req.params.gameId)}` +
      `/player/${encodeURIComponent(v.openid)}` +
      `?token=${encodeURIComponent(token)}&msg=${encodeURIComponent(out.message)}`;
    res.redirect(303, back);
  } catch (err) {
    console.error("[web] 提交动作失败:", err);
    fail(503, "服务暂时不可用，请稍后重试");
  }
});

app.listen(PORT, () => {
  console.log(`[web] 监听 http://127.0.0.1:${PORT}（server=${SERVER_URL}）`);
});
