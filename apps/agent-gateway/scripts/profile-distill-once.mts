// 手动触发一次画像蒸馏（真机验证用，不必等到凌晨 4 点）。
//
// 前置：Redis 在线 + server 在线（/internal/*）+ 本地 9B GGUF 与 llama-server 可用。
// 运行：
//   npx tsx apps/agent-gateway/scripts/profile-distill-once.mts
//
// 环境变量（同 agent-gateway）：REDIS_URL / SERVER_URL / SYSTEMONE_URL / PROFILE_LLM_* / PROFILE_GATE_THRESHOLD
// 流程：启动 9B → 蒸馏 → 关闭 → 打印进程序列释放确认；并打印活跃/更新/跳过计数。

import "dotenv/config";
import { Redis } from "ioredis";
import { ProfileClient } from "../src/profile/client.js";
import { LocalLlm, withLocalLlm } from "../src/profile/local-llm.js";
import { runDistillJob } from "../src/profile/distill.js";
import { makeSystemOneGate } from "../src/profile/gate.js";
import { SystemOneClient } from "../src/systemone.js";

const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const serverUrl = process.env.SERVER_URL ?? "http://127.0.0.1:3000";
const modelPath = process.env.PROFILE_LLM_MODEL ?? "";
if (!modelPath) {
  console.error("[profile-once] 未配置 PROFILE_LLM_MODEL（本地 9B GGUF 路径）");
  process.exit(1);
}

const redis = new Redis(redisUrl);
const client = new ProfileClient({ baseUrl: serverUrl });
const systemone = new SystemOneClient({
  url: process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090",
  timeoutMs: Number(process.env.SYSTEMONE_TIMEOUT_MS ?? 3000),
});
const gate = makeSystemOneGate(
  systemone,
  Number(process.env.PROFILE_GATE_THRESHOLD ?? 0.5),
);

const llm = new LocalLlm({
  modelPath,
  llamaBin: process.env.PROFILE_LLM_BIN ?? "llama-server",
  port: Number(process.env.PROFILE_LLM_PORT ?? 8082),
  ngl: process.env.PROFILE_LLM_NGL ?? "99",
  ctx: process.env.PROFILE_LLM_CTX ?? "8192",
});

try {
  await withLocalLlm(llm, async (l) => {
    const res = await runDistillJob({
      client,
      generate: (msgs) => l.generate(msgs),
      gate,
      kv: redis,
      onProgress: (m) => console.log(`[profile-once] ${m}`),
    });
    console.log(
      `[profile-once] 完成：活跃 ${res.activeOpenids}，更新 ${res.updated}，跳过 ${res.skipped}`,
    );
  });
} catch (err) {
  console.error("[profile-once] 失败:", err);
  process.exitCode = 1;
} finally {
  await redis.quit();
}
