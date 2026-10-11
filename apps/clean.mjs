// 清理 qq.raw / internal / qq.push（DEL 会连带删除其消费者组）。
import { Redis } from "ioredis";

const redis = new Redis("redis://127.0.0.1:6379");
await redis.del("qq.raw", "internal", "qq.push");
await redis.quit();
console.log("[clean] 已清理三个 stream");
