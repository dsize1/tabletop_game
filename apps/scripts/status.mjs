// 健康探针：读 Redis 缓存 health:status（server 后台自检每 10s 写入）。
// 用法：node apps/scripts/status.mjs
import { Redis } from "ioredis";

const redis = new Redis(process.env.REDIS_URL ?? "redis://127.0.0.1:6379");

const s = await redis.get("health:status");
if (s) {
  console.log(s);
} else {
  console.log(
    JSON.stringify({ status: "unknown", note: "缓存不存在：server 未启动或自检尚未运行" }),
  );
}

await redis.quit();
