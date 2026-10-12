// web：Express 独立站点（BFF）。
//
// 路由：
// - GET /dev_task/:sessionId?token=xxx         计划文档页（校验 token → 拉 markdown → 渲染 HTML）
// - GET /tabletop_game/:gameId/player/:openid  游戏资源页（引擎未实现，占位「暂未开放」）
// - GET /health                                健康检查（cloudflared 探活用）
//
// 服务端调 server（内部 127.0.0.1）鉴权 + 拉取；浏览器不直连 server / SQLite。

import "dotenv/config";
import express from "express";
import { renderErrorPage, renderMarkdown, renderPage } from "./render.js";

const SERVER_URL = process.env.SERVER_URL ?? "http://127.0.0.1:3000";
const PORT = Number(process.env.WEB_PORT ?? process.env.PORT ?? 18080);

// cloudflared 反代后源站只见回环，真实 IP 取 CF-Connecting-IP / X-Forwarded-For
const app = express();
app.set("trust proxy", true);

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

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

app.get("/tabletop_game/:gameId/player/:openid", (_req, res) => {
  res.send(renderPage("游戏资源", "<p>暂未开放</p>"));
});

app.listen(PORT, () => {
  console.log(`[web] 监听 http://127.0.0.1:${PORT}（server=${SERVER_URL}）`);
});
