// 一次性画像 bootstrap：读导出的对话 JSON（group_messages 全量），
// 用**真实管道**（distillWindow = 本地 9B 生成 + SystemOne 把关 + 用户轮素材 + 分块 map-reduce）产出画像。
//
// 与在线日批的差异：一次覆盖**全部历史**（from/to 扩大），不读写 DB / 不推进 Redis 游标。
//
// 运行（在 apps/agent-gateway/ 下）：
//   npx tsx scripts/profile-bootstrap.mts [输入JSON] [输出JSON]
// 默认：../../data/profile_input.json → ../../data/profile_bootstrap.json

import "dotenv/config";
import { readFileSync, writeFileSync } from "node:fs";
import { LocalLlm, withLocalLlm } from "../src/profile/local-llm.js";
import { distillWindow } from "../src/profile/distill.js";
import type { ChatMessageRow } from "../src/profile/client.js";
import { makeSystemOneGate } from "../src/profile/gate.js";
import { SystemOneClient } from "../src/systemone.js";

const inPath = process.argv[2] ?? "data/profile_input.json";
const outPath = process.argv[3] ?? "data/profile_bootstrap.json";

const modelPath = process.env.PROFILE_LLM_MODEL ?? "";
if (!modelPath) {
  console.error("未配置 PROFILE_LLM_MODEL");
  process.exit(1);
}

const rows = JSON.parse(readFileSync(inPath, "utf-8")) as ChatMessageRow[];
if (rows.length === 0) {
  console.error("输入为空");
  process.exit(1);
}
const ids = [...new Set(rows.map((r) => r.openid))];
const from = rows.reduce((a, r) => (r.created_at < a ? r.created_at : a), rows[0]!.created_at);
const to = new Date(Date.now() + 1000).toISOString();

const upserts: Array<{ openid: string; profile: string; distilledAt: string }> = [];
const client = {
  async fetchActiveOpenids() {
    return ids;
  },
  async fetchChatWindow() {
    return rows;
  },
  async getProfile() {
    return null;
  },
  async upsertProfile(openid: string, profileText: string, distilledAt: string) {
    upserts.push({ openid, profile: profileText, distilledAt });
  },
};

const systemone = new SystemOneClient({
  url: process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090",
  timeoutMs: Number(process.env.SYSTEMONE_TIMEOUT_MS ?? 5000),
});
const gate = makeSystemOneGate(systemone, Number(process.env.PROFILE_GATE_THRESHOLD ?? 0.5));

const llm = new LocalLlm({
  modelPath,
  llamaBin: process.env.PROFILE_LLM_BIN ?? "llama-server",
  port: Number(process.env.PROFILE_LLM_PORT ?? 8083),
  ngl: process.env.PROFILE_LLM_NGL ?? "99",
  ctx: process.env.PROFILE_LLM_CTX ?? "8192",
});

let result: unknown = null;
await withLocalLlm(llm, async (l) => {
  result = await distillWindow(from, to, {
    client: client as never,
    generate: (m) => l.generate(m),
    gate,
    onProgress: (m) => console.log(`[bootstrap] ${m}`),
  });
});

const users = ids.map((openid) => {
  const list = rows.filter((r) => r.openid === openid);
  const u = upserts.find((x) => x.openid === openid);
  return {
    openid,
    groups: [...new Set(list.map((r) => r.group_id))],
    messages: list.length,
    userMessages: list.filter((r) => r.role !== "assistant").length,
    profile: u?.profile ?? "",
  };
});

writeFileSync(outPath, JSON.stringify({ from, to, result, users }, null, 2), "utf-8");
console.log(`[bootstrap] result=${JSON.stringify(result)}`);
console.log(`[bootstrap] 已写出 ${outPath}`);
