// dev 计划工作流复用验证（live，需 LLM）：spawn 子会话 → 规划 → 检查计划文档落盘。
// 运行：npx tsx test-dev-plan.mts（需 .env 配置 LLM_API_KEY）
import "dotenv/config";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { DevSessionManager, devTaskPrompt } from "./src/dev-session.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, "../..");
const mgr = new DevSessionManager();
const sessionId = randomUUID();
const { session } = await mgr.spawn(sessionId);
try {
  await session.prompt(devTaskPrompt("给 storage crate 增加一个 ping() 健康检查方法"));
  const out = session.getLastAssistantText() ?? "";
  console.log("=== 子会话输出（前 500 字）===\n", out.slice(0, 500));

  const planPath = path.join(PROJECT_ROOT, "memory_palace", "plan", `${sessionId}.md`);
  const exists = fs.existsSync(planPath);
  console.log(`\n计划文档存在: ${exists} -> ${planPath}`);
  if (exists) {
    const content = fs.readFileSync(planPath, "utf-8");
    const hasSections = ["## 计划", "## 待办实现", "## 验证事项"].every((h) => content.includes(h));
    console.log("含 6 分区关键标题:", hasSections);
    if (!hasSections) throw new Error("计划文档缺少分区");
  } else {
    throw new Error("计划文档未生成");
  }
  console.log("test-dev-plan 计划工作流复用验证通过 ✅");
} finally {
  session.dispose();
}
