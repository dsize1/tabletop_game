// 主动推送验证脚本：向 qq.push 注入一条「主动消息」指令（无 reply_msg_id），观察 qq-gateway 出站消费与发送。
// 运行前提：Redis 已起（dev-environment §3）；qq-gateway 已运行。
//   真实发送需 QQBOT 凭证 + 机器人已加群 + WebSocket 在线；未提供真实群时用 mock 群，仅验证「消费/发送路径」。
// 用法：node apps/verify-push.mjs [group_openid] [content] [--no-cleanup]
//   环境变量：REDIS_URL（默认 redis://127.0.0.1:6379）、PUSH_TEST_GROUP（测试群 openid）
import { Redis } from "ioredis";

const args = process.argv.slice(2);
const positional = args.filter((a) => !a.startsWith("--"));
const groupId = positional[0] ?? process.env.PUSH_TEST_GROUP ?? "mock-group-1";
const content =
  positional[1] ?? `主动推送测试 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
const noCleanup = args.includes("--no-cleanup");

const redis = new Redis(process.env.REDIS_URL ?? "redis://127.0.0.1:6379");

// 主动推送指令：reply_msg_id 为 null → qq-gateway 走 sendText（主动消息）
const event = {
  event_id: `verify-push-${Date.now()}`,
  group_id: groupId,
  content,
  reply_msg_id: null,
  at_member_id: null,
};

const entryId = await redis.xadd(
  "qq.push",
  "MAXLEN",
  "~",
  "1000",
  "*",
  "event",
  JSON.stringify(event),
);
console.log(`[verify-push] 已注入 qq.push [${entryId}]`);
console.log(`[verify-push] group=${groupId} content=${content} (reply_msg_id=null → 主动消息)`);

await new Promise((r) => setTimeout(r, 3000));

// 观察 qq-gateway 消费：ACK 成功 → 指令不在 PEL；发送失败 → 留在 PEL 待重试
let pelIds = [];
try {
  const pending = await redis.xpending("qq.push", "qq-gateway", "-", "+", "20");
  pelIds = (pending ?? []).map((p) => p[0]);
} catch (err) {
  console.log(`[verify-push] ⚠️ 无法读取 PEL（qq-gateway 消费者组可能不存在）: ${err?.message ?? err}`);
}

const inPel = pelIds.includes(entryId);
console.log(`[verify-push] qq-gateway PEL 待处理条数: ${pelIds.length}`);
if (inPel) {
  console.log(`[verify-push] ❌ 指令仍在 PEL（发送失败/待重试），请查看 qq-gateway 日志`);
} else {
  console.log(`[verify-push] ✅ 指令已离开 PEL（已 ACK，发送成功；或 qq-gateway 未运行，见下方组信息）`);
}

try {
  const groups = await redis.xinfo("GROUPS", "qq.push");
  console.log(`[verify-push] qq.push 消费者组信息: ${JSON.stringify(groups)}`);
} catch (err) {
  console.log(`[verify-push] ⚠️ 读取组信息失败: ${err?.message ?? err}`);
}

// 清理本次注入（避免 mock 群反复重试污染 PEL；真实群成功 ACK 后也一并清掉）
if (!noCleanup) {
  await redis.xack("qq.push", "qq-gateway", entryId).catch(() => 0);
  await redis.xdel("qq.push", entryId).catch(() => 0);
  console.log(`[verify-push] 已清理注入条目 [${entryId}]`);
}

await redis.quit();
