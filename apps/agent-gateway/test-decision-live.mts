// 真实服务集成验证：需本地 decision-service 在线（SYSTEMONE_URL，默认 http://127.0.0.1:8090）。
// 先启动：node apps/scripts/start-decision-service.mjs
// 运行：npx tsx test-decision-live.mts
import assert from "node:assert";
import { decideIntent } from "./src/decision.js";
import { SystemOneClient } from "./src/systemone.js";

const url = process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090";
const client = new SystemOneClient({ url, timeoutMs: 10000 });

// 探活
try {
  const r = await fetch(`${url.replace(/\/+$/, "")}/`);
  assert.ok(r.ok, `服务探活失败 HTTP ${r.status}`);
  console.log(`[live] 决策服务在线: ${url}`);
} catch (err) {
  console.error(`[live] 无法连接决策服务 ${url}：${(err as Error).message}`);
  console.error("[live] 先运行 node apps/scripts/start-decision-service.mjs");
  process.exit(2);
}

const cases: Array<[string, string]> = [
  ["大家好呀，今天天气不错", "chat"],
  ["我们来玩一局猜拳吧", "start_game"],
  ["我出石头", "game_action"],
  ["帮我把登录的 bug 修一下", "dev"],
  ["重新开始一局吧", "start_game"],
];

let pass = 0;
for (const [msg, expect] of cases) {
  const d = await decideIntent(msg, client);
  const ok = d.intent === expect && d.via === "decision";
  if (ok) pass++;
  console.log(
    `${ok ? "✓" : "✗"} "${msg}" → ${d.intent} conf=${d.confidence.toFixed(3)} via=${d.via}（期望 ${expect}）`,
  );
}
assert.equal(pass, cases.length, `意图分类 ${pass}/${cases.length} 通过`);

console.log("✅ decision live 集成验证通过");
