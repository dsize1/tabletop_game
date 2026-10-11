// 临时验证脚本：注入模拟 @消息到 qq.raw，观察 internal / qq.push 链路。
// 运行前提：server（bot-adapter）与 agent-gateway 已在后台运行。
import { Redis } from "ioredis";

const redis = new Redis("redis://127.0.0.1:6379");

// 1. 注入模拟 @消息到 qq.raw
const mockEvent = {
  eventType: "GROUP_AT_MESSAGE_CREATE",
  raw: {
    id: "mock-msg-1",
    author: { member_openid: "mock-openid-1", username: "测试玩家" },
    group_openid: "mock-group-1",
    content: "#聊 你好，测试链路",
    message_type: 0,
    timestamp: "2026-10-11T12:00:00+08:00",
  },
  receivedAt: Date.now(),
};
await redis.xadd("qq.raw", "MAXLEN", "~", "1000", "*", "event", JSON.stringify(mockEvent));
console.log("[verify] 已注入 qq.raw");

// 3. 等待 server → agent-gateway 处理
await new Promise((r) => setTimeout(r, 3000));

// 4. 检查 internal
const internal = await redis.xrange("internal", "-", "+");
console.log(`[verify] internal 条数: ${internal.length}`);
for (const [id, fields] of internal) {
  console.log(`  ${id} → ${fields[1]}`);
}

// 5. 检查 qq.push
const push = await redis.xrange("qq.push", "-", "+");
console.log(`[verify] qq.push 条数: ${push.length}`);
for (const [id, fields] of push) {
  console.log(`  ${id} → ${fields[1]}`);
}

await redis.quit();
