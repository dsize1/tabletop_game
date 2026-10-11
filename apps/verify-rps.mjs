// 猜拳端到端验证：注入「来玩猜拳 石头」，观察 qq.push 判定结果。
import { Redis } from "ioredis";

const redis = new Redis("redis://127.0.0.1:6379");

const mockEvent = {
  eventType: "GROUP_AT_MESSAGE_CREATE",
  raw: {
    id: "mock-msg-rps-1",
    author: { member_openid: "mock-openid-1", username: "测试玩家" },
    group_openid: "mock-group-1",
    content: "来玩猜拳 石头",
    message_type: 0,
    timestamp: "2026-10-11T12:30:00+08:00",
  },
  receivedAt: Date.now(),
};
await redis.xadd("qq.raw", "MAXLEN", "~", "1000", "*", "event", JSON.stringify(mockEvent));
console.log("[verify] 已注入猜拳消息");

await new Promise((r) => setTimeout(r, 4000));

const push = await redis.xrange("qq.push", "-", "+");
console.log(`[verify] qq.push 条数: ${push.length}`);
for (const [id, fields] of push) {
  console.log(`  ${id} → ${fields[1]}`);
}

await redis.quit();
