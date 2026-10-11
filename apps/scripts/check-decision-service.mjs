#!/usr/bin/env node
// 决策服务健康探针：检查两层服务（llama-server 权重 + /v1/systemone 决策）。
// 用法：node apps/scripts/check-decision-service.mjs [--json]
// 环境：
//   LLAMA_PORT        llama-server 端口（默认 8081）
//   DECISION_PORT     决策服务端口（默认 8090）
//   SYSTEMONE_URL     决策服务根地址（优先于 DECISION_PORT）
//   PROBE_SAMPLE=0    跳过样例推理（只做 HTTP 探活）
// 退出码：0=全部健康；1=存在异常。
const asJson = process.argv.includes("--json");
const llamaPort = Number(process.env.LLAMA_PORT ?? 8081);
const decisionUrl = (
  process.env.SYSTEMONE_URL ?? `http://127.0.0.1:${process.env.DECISION_PORT ?? 8090}`
).replace(/\/+$/, "");

const results = [];
const record = (name, ok, detail) => results.push({ name, ok, detail });

async function fetchJson(url, init = {}, timeoutMs = 5000) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(url, { ...init, signal: ctl.signal });
    const text = await r.text();
    let json = null;
    try {
      json = JSON.parse(text);
    } catch {
      /* 非 JSON 响应 */
    }
    return { ok: r.ok, status: r.status, json };
  } finally {
    clearTimeout(timer);
  }
}

// 1) llama-server 权重层
try {
  const r = await fetchJson(`http://127.0.0.1:${llamaPort}/health`, {}, 3000);
  record("llama-server /health", r.ok && r.json?.status === "ok", r.json?.status ?? `HTTP ${r.status}`);
} catch (e) {
  record("llama-server /health", false, e.message);
}

// 2) 决策服务 HTTP
try {
  const r = await fetchJson(`${decisionUrl}/`, {}, 3000);
  record("decision-server GET /", r.ok && r.json?.status === "ok", r.json?.model ?? `HTTP ${r.status}`);
} catch (e) {
  record("decision-server GET /", false, e.message);
}

// 3) 样例推理（端到端）
if (process.env.PROBE_SAMPLE !== "0") {
  try {
    const r = await fetchJson(
      `${decisionUrl}/v1/systemone`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          state: { message: "我们来玩一局猜拳吧" },
          questions: {
            intent: {
              type: "choice",
              instructions: "玩家的意图是什么？",
              criteria: {
                chat: "闲聊",
                start_game: "开始新游戏",
                game_action: "游戏动作",
                dev: "开发任务",
              },
            },
          },
        }),
      },
      15000,
    );
    const a = r.json?.answers?.intent;
    record(
      "POST /v1/systemone 样例",
      Boolean(a?.choice),
      a ? `${a.choice} ${Number(a.confidence ?? 0).toFixed(3)}` : `HTTP ${r.status}`,
    );
  } catch (e) {
    record("POST /v1/systemone 样例", false, e.message);
  }
}

const allOk = results.every((r) => r.ok);
if (asJson) {
  console.log(JSON.stringify({ ok: allOk, checks: results }, null, 2));
} else {
  for (const r of results) console.log(`${r.ok ? "✓" : "✗"} ${r.name}: ${r.detail}`);
  console.log(
    allOk
      ? "✅ 决策服务健康"
      : "❌ 决策服务异常（先运行：node apps/scripts/start-decision-service.mjs）",
  );
}
process.exit(allOk ? 0 : 1);
