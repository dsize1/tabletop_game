// chat.log 归档单测：emitChatLog 写入格式（event_id/字段）、空内容跳过、写入失败静默。
// 运行：npx tsx test-chat-log.mts
import assert from "node:assert";
import { CHAT_LOG_STREAM, emitChatLog } from "./src/chat-log.js";

interface Entry {
  key: string;
  args: (string | number)[];
}

function memStream() {
  const entries: Entry[] = [];
  return {
    entries,
    xadd: async (key: string, ...args: (string | number)[]) => {
      entries.push({ key, args });
      return "1-1";
    },
  };
}

// 正常写入：字段完整、event_id 非空、created_at 为 ISO
{
  const s = memStream();
  await emitChatLog(s, { openid: "u1", groupId: "g1", role: "user", content: "你好" });
  assert.equal(s.entries.length, 1);
  const e = s.entries[0]!;
  assert.equal(e.key, CHAT_LOG_STREAM);
  assert.deepEqual(e.args.slice(0, 4), ["MAXLEN", "~", 1000, "*"]);
  assert.equal(e.args[4], "event");
  const ev = JSON.parse(String(e.args[5])) as Record<string, string>;
  assert.equal(ev.openid, "u1");
  assert.equal(ev.group_id, "g1");
  assert.equal(ev.role, "user");
  assert.equal(ev.content, "你好");
  assert.ok(typeof ev.event_id === "string" && ev.event_id.length > 0, "event_id 非空");
  assert.ok(typeof ev.created_at === "string" && ev.created_at.includes("T"), "created_at 为 ISO");
}

// assistant 轮也写
{
  const s = memStream();
  await emitChatLog(s, { openid: "u1", groupId: "g1", role: "assistant", content: "回复" });
  const ev = JSON.parse(String(s.entries[0]!.args[5])) as Record<string, string>;
  assert.equal(ev.role, "assistant");
}

// 空内容 → 不写
{
  const s = memStream();
  await emitChatLog(s, { openid: "u1", groupId: "g1", role: "user", content: "" });
  assert.equal(s.entries.length, 0);
}

// 写入报错 → 静默（不抛）
{
  const s = {
    xadd: async (): Promise<unknown> => {
      throw new Error("boom");
    },
  };
  await emitChatLog(s, { openid: "u1", groupId: "g1", role: "user", content: "x" });
}

console.log("✅ chat-log 单测全部通过");
