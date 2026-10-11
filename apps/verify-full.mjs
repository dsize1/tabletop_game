// 综合验证：注入聊天 / 猜拳 / dev（非白名单）三条消息，观察 qq.push。
import { Redis } from "ioredis";

const redis = new Redis("redis://127.0.0.1:6379");

const mk = (id, content) => ({
  eventType: "GROUP_AT_MESSAGE_CREATE",
  raw: {
    id,
    author: { member_openid: "mock-openid-1", username: "测试玩家" },
    group_openid: "mock-group-1",
    content,
    message_type: 0,
    timestamp: "2026-10-11T13:00:00+08:00",
  },
  receivedAt: Date.now(),
});

await redis.xadd("qq.raw", "MAXLEN", "~", "1000", "*", "event", JSON.stringify(mk("m1", "#聊 你好")));
await redis.xadd("qq.raw", "MAXLEN", "~", "1000", "*", "event", JSON.stringify(mk("m2", "来玩猜拳 布")));
await redis.xadd("qq.raw", "MAXLEN", "~", "1000", "*", "event", JSON.stringify(mk("m3", "#开发 帮我写个功能")));

console.log("[verify] 已注入 3 条消息");
await new Promise((r) => setTimeout(r, 5000));

const push = await redis.xrange("qq.push", "-", "+");
console.log(`[verify] qq.push 条数: ${push.length}`);
for (const [id, fields] of push) console.log(`  → ${fields[1]}`);

await redis.quit();
