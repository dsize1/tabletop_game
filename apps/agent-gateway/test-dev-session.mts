// dev 子会话（pi-sdk）冒烟验证：spawn → 跑一轮 → 取输出。
// 运行：npx tsx test-dev-session.mts（需 .env 配置 LLM_API_KEY）
import "dotenv/config";
import { randomUUID } from "node:crypto";
import { DevSessionManager } from "./src/dev-session.js";

const mgr = new DevSessionManager();
const sessionId = randomUUID();
const { session } = await mgr.spawn(sessionId);
console.log("spawned sessionId =", sessionId);
try {
  await session.prompt("请只回复两个字：收到");
  const out = session.getLastAssistantText() ?? "";
  console.log("output =", out.slice(0, 200));
  if (!out.includes("收到")) {
    throw new Error("子会话输出未包含预期内容");
  }
  console.log("test-dev-session 冒烟通过 ✅");
} finally {
  session.dispose();
}
