// 画像蒸馏管道单测：候选解析 / 组装 / 分块 / 用户轮素材 / 生成+把关 / 游标增量（全注入桩，离线）。
// 运行：npx tsx test-profile-distill.mts
import assert from "node:assert";
import {
  DISTILL_CURSOR_KEY,
  assembleProfile,
  buildDistillMessages,
  chunkLines,
  distillWindow,
  parseCandidateFacts,
  runDistillJob,
  userUtterances,
  type KvStore,
} from "./src/profile/distill.js";
import type { ChatMessageRow } from "./src/profile/client.js";

// 候选解析：去项目符号/序号、去空行、去重保序
assert.deepEqual(
  parseCandidateFacts("- 喜欢石头\n1. 爱开玩笑\n\n喜欢石头\n* 常来\n2) 手速快"),
  ["喜欢石头", "爱开玩笑", "常来", "手速快"],
);
assert.deepEqual(parseCandidateFacts(""), []);

// 组装：每行 `- `，超长截断
assert.equal(assembleProfile(["a", "b"]), "- a\n- b");
assert.equal(assembleProfile(["abcdef"], 4).length, 4);

// 素材只用「用户轮」：assistant 与空白被剔除
assert.deepEqual(
  userUtterances([
    { openid: "u", group_id: "g", role: "user", content: "我喜欢竞技", created_at: "x" },
    { openid: "u", group_id: "g", role: "assistant", content: "机器人技术细节XYZ", created_at: "x" },
    { openid: "u", group_id: "g", role: "user", content: "   ", created_at: "x" },
  ]),
  ["我喜欢竞技"],
);

// 分块（整条边界；单条超长自成一块）
assert.deepEqual(chunkLines(["aa", "bb", "cc"], 6), ["aa\nbb", "cc"]);
assert.deepEqual(chunkLines([], 10), []);
assert.deepEqual(chunkLines(["x".repeat(20)], 5), ["x".repeat(20)]);

// 蒸馏提示含旧画像
const msgs = buildDistillMessages("旧画像", "你好");
assert.equal(msgs[0]!.role, "system");
assert.ok(msgs[1]!.content.includes("旧画像"));
assert.ok(msgs[0]!.content.includes("信息抽取器"), "系统提示应为抽取器指令");

// distillWindow：assistant 不进生成输入；生成归 9B、把关归 SystemOne（桩）
{
  const rows: ChatMessageRow[] = [
    { openid: "u1", group_id: "g1", role: "user", content: "我喜欢竞技", created_at: "2026-02-14T01:00:00Z" },
    { openid: "u1", group_id: "g1", role: "assistant", content: "机器人技术细节XYZ", created_at: "2026-02-14T01:00:01Z" },
    { openid: "u2", group_id: "g1", role: "user", content: "随便聊聊", created_at: "2026-02-14T01:30:00Z" },
  ];
  const upserts: Array<{ openid: string; text: string; distilledAt: string }> = [];
  const genInputs: string[] = [];
  const gateCalls: string[] = [];
  const client = {
    async fetchActiveOpenids() {
      return ["u1", "u2"];
    },
    async fetchChatWindow() {
      return rows;
    },
    async getProfile(openid: string) {
      return openid === "u1"
        ? { openid, profile_text: "旧：喜欢合作", distilled_at: null, updated_at: "x" }
        : null;
    },
    async upsertProfile(openid: string, profileText: string, distilledAt: string) {
      upserts.push({ openid, text: profileText, distilledAt });
    },
  };
  const generate = async (m: { content: string }[]) => {
    genInputs.push(m[1]!.content);
    return m[1]!.content.includes("竞技") ? "- 喜欢竞技\n- 临时：今天很累" : "- 偶尔闲聊";
  };
  const gate = async (c: string) => {
    gateCalls.push(c);
    return !c.includes("临时");
  };
  const res = await distillWindow("2026-02-14T00:00:00Z", "2026-02-15T00:00:00Z", {
    client: client as never,
    generate,
    gate,
    minUsers: 1,
    now: () => Date.parse("2026-02-14T04:00:00Z"),
  });
  assert.equal(res.activeOpenids, 2);
  assert.equal(res.updated, 2);
  assert.equal(res.skipped, 0);
  const u1 = upserts.find((u) => u.openid === "u1")!;
  assert.equal(u1.text, "- 喜欢竞技", "临时事实被把关拒绝");
  assert.equal(u1.distilledAt, "2026-02-15T00:00:00Z", "distilled_at = 窗口 right");
  assert.ok(gateCalls.includes("临时：今天很累"));
  assert.ok(
    !genInputs.some((s) => s.includes("机器人技术细节XYZ")),
    "assistant 内容不得进入生成输入",
  );
  assert.ok(genInputs.some((s) => s.includes("我喜欢竞技")));
}

// 空输出重试一次
{
  const rows: ChatMessageRow[] = [
    { openid: "u1", group_id: "g1", role: "user", content: "话", created_at: "2026-02-14T01:00:00Z" },
    { openid: "u1", group_id: "g1", role: "user", content: "更多话", created_at: "2026-02-14T01:01:00Z" },
    { openid: "u1", group_id: "g1", role: "user", content: "还是话", created_at: "2026-02-14T01:02:00Z" },
  ];
  let calls = 0;
  const client = {
    async fetchActiveOpenids() {
      return ["u1"];
    },
    async fetchChatWindow() {
      return rows;
    },
    async getProfile() {
      return null;
    },
    async upsertProfile() {},
  };
  await distillWindow("2026-02-14T00:00:00Z", "2026-02-15T00:00:00Z", {
    client: client as never,
    generate: async () => {
      calls++;
      return calls === 1 ? "" : "- 复读机";
    },
    gate: async () => true,
    minUsers: 1,
  });
  assert.equal(calls, 2, "首次空输出应重试一次");
}

// runDistillJob：成功后推进游标
{
  const kvMap = new Map<string, string>();
  const kv: KvStore = {
    async get(k) {
      return kvMap.get(k) ?? null;
    },
    async set(k, v) {
      kvMap.set(k, v);
      return "OK";
    },
  };
  const client = {
    async fetchActiveOpenids() {
      return [];
    },
    async fetchChatWindow() {
      return [];
    },
    async getProfile() {
      return null;
    },
    async upsertProfile() {},
  };
  const res = await runDistillJob({
    client: client as never,
    generate: async () => "",
    gate: async () => true,
    kv,
    now: () => Date.parse("2026-02-14T04:00:00Z"),
  });
  assert.equal(res.activeOpenids, 0);
  assert.equal(kvMap.get(DISTILL_CURSOR_KEY), "2026-02-14T04:00:00.000Z", "游标推进到本次 to");
}

console.log("✅ profile-distill 单测全部通过");
