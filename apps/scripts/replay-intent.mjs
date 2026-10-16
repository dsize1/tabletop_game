#!/usr/bin/env node
// 意图置信度回放标定：读 SQLite group_messages 入站 → 打决策服务 /v1/systemone → 输出置信度分布与「意图确认」触发量。
// 用途：为 DECISION_INTENT_THRESHOLD(_HIGH) 取值提供数据（线上决策日志早期未落 conf，只能靠回放恢复）。
// 用法：
//   node apps/scripts/replay-intent.mjs [--json] [--list] [--since=YYYY-MM-DD] [--limit=N]
// 环境：
//   SYSTEMONE_URL                    决策服务根地址（默认 http://127.0.0.1:8090）
//   DECISION_INTENT_THRESHOLD        低阈值（默认 0.6）
//   DECISION_INTENT_THRESHOLD_HIGH   高阈值（默认 0.8）
//   SQLITE_PATH                      SQLite 路径（默认 data/tabletop.db）
// 说明：判定真值需人工；本脚本聚焦「模型自己给出的 choice/confidence」分布，以及非 chat 预测在 [低,高) 的触发量。
import { DatabaseSync } from "node:sqlite";

const argv = process.argv.slice(2);
const asJson = argv.includes("--json");
const list = argv.includes("--list");
const since = (argv.find((a) => a.startsWith("--since=")) ?? "").split("=")[1] ?? "";
const limit = Number((argv.find((a) => a.startsWith("--limit=")) ?? "").split("=")[1] ?? 0);

const base = (process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090").replace(/\/+$/, "");
const LOW = Number(process.env.DECISION_INTENT_THRESHOLD ?? 0.6);
const HIGH = Number(process.env.DECISION_INTENT_THRESHOLD_HIGH ?? 0.8);
const dbPath = process.env.SQLITE_PATH ?? "data/tabletop.db";

const CRITERIA = {
  chat: "闲聊、打招呼、普通提问，或与游戏操作无关的内容",
  start_game: "请求开始新游戏或新的一局",
  game_action: "在当前对局中出手势或声明一个动作",
  dev: "软件开发、部署或修复 bug 等开发任务",
};

/** 与 decision.ts `maskUrls` 一致：链接文本是强 dev 信号，送模型前屏蔽。 */
const maskUrls = (s) =>
  s.replace(/https?:\/\/[^\s<>"'）)】\]，。；：！？、（）【】《》]+/gi, "[链接]");

async function ask(message, timeoutMs = 15000) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const r = await fetch(`${base}/v1/systemone`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        state: { message: maskUrls(message) },
        questions: { intent: { type: "choice", instructions: "玩家的意图是什么？", criteria: CRITERIA } },
      }),
      signal: ctl.signal,
    });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const a = (await r.json())?.answers?.intent;
    if (!a || typeof a.confidence !== "number") throw new Error("响应缺少 intent.confidence");
    return { choice: a.choice, confidence: a.confidence };
  } finally {
    clearTimeout(timer);
  }
}

const db = new DatabaseSync(dbPath);
const rows = db
  .prepare(
    "SELECT content, created_at, direction FROM group_messages WHERE direction = 'in' ORDER BY created_at",
  )
  .all();
const inbound = rows
  .map((r) => ({
    content: (r.content ?? "").trim(),
    ts: new Date((Number(r.created_at) + 8 * 3600) * 1000).toISOString().slice(0, 19).replace("T", " "),
  }))
  .filter((r) => r.content)
  .filter((r) => !since || r.ts >= since);
db.close();

const sample = limit > 0 ? inbound.slice(-limit) : inbound;
const results = [];
for (const [i, m] of sample.entries()) {
  try {
    results.push({ ...m, ...(await ask(m.content)) });
  } catch (e) {
    results.push({ ...m, error: e.message });
  }
  if (!asJson && (i + 1) % 25 === 0) process.stderr.write(`… ${i + 1}/${sample.length}\r`);
}

const ok = results.filter((r) => !r.error);
const nonChat = ok.filter((r) => r.choice !== "chat");
const challenged = nonChat.filter((r) => r.confidence >= LOW && r.confidence < HIGH);

const quantile = (arr, q) => {
  if (!arr.length) return null;
  const s = [...arr].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor(q * s.length))];
};
const summary = {
  service: base,
  thresholds: { low: LOW, high: HIGH },
  sample: { inbound: inbound.length, evaluated: sample.length, failed: results.length - ok.length },
  choice: Object.fromEntries(
    ["chat", "start_game", "game_action", "dev"].map((c) => {
      const cs = ok.filter((r) => r.choice === c).map((r) => r.confidence);
      return [c, { n: cs.length, min: cs.length ? Math.min(...cs) : null, median: quantile(cs, 0.5), max: cs.length ? Math.max(...cs) : null }];
    }),
  ),
  nonChatPredictions: {
    n: nonChat.length,
    confidenceRange: nonChat.length
      ? [Math.min(...nonChat.map((r) => r.confidence)), Math.max(...nonChat.map((r) => r.confidence))]
      : null,
    inConfirmBand: challenged.length,
  },
};

if (asJson) {
  console.log(JSON.stringify({ ...summary, challenged: list ? challenged : undefined }, null, 2));
} else {
  console.log(`决策服务：${base}（低/高阈值 ${LOW} / ${HIGH}）`);
  console.log(`样本：入站 ${summary.sample.inbound} 条，判定 ${summary.sample.evaluated} 条，失败 ${summary.sample.failed} 条`);
  for (const [c, s] of Object.entries(summary.choice)) {
    if (s.n) console.log(`  ${c.padEnd(12)} n=${String(s.n).padStart(3)}  conf ${s.min.toFixed(3)}~${s.max.toFixed(3)}（中位 ${s.median.toFixed(3)}）`);
  }
  const nc = summary.nonChatPredictions;
  console.log(
    `非 chat 预测：n=${nc.n}` +
      (nc.confidenceRange ? ` conf ${nc.confidenceRange[0].toFixed(3)}~${nc.confidenceRange[1].toFixed(3)}` : "") +
      `；落在确认带 [${LOW},${HIGH}) 的 ${nc.inConfirmBand} 条（这些会触发「意图确认」）`,
  );
  if (list) {
    console.log("\n落在确认带的非 chat 预测（人工核真值）：");
    for (const r of challenged.sort((a, b) => a.confidence - b.confidence)) {
      console.log(`  ${r.confidence.toFixed(3)} ${r.choice.padEnd(12)} ${r.ts} | ${r.content.slice(0, 60)}`);
    }
  }
}
process.exit((results.length - ok.length) > 0 ? 1 : 0);
