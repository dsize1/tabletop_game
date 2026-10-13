// 健康探针：读 Redis 缓存 health:status（server 后台自检每 10s 写入）。
// 用法：node apps/scripts/status.mjs（仓库根）
import { Redis } from "ioredis";

const url = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
// lazyConnect + 有限重试：Redis 不可达时快速失败并给出可读提示，
// 而不是抛出 ioredis 未处理的 error 事件 / MaxRetriesPerRequestError 堆栈。
const redis = new Redis(url, {
  lazyConnect: true,
  maxRetriesPerRequest: 1,
  retryStrategy: () => null,
});
redis.on("error", () => {
  /* 由下方 catch 统一提示；避免未处理 error 事件导致进程崩溃 */
});

try {
  await redis.connect();
  const s = await redis.get("health:status");
  if (s) {
    console.log(s);
  } else {
    console.log(
      JSON.stringify({ status: "unknown", note: "缓存不存在：server 未启动或自检尚未运行" }),
    );
  }
} catch {
  console.error(
    `[status] Redis 不可达（${url}）：server 未运行，或 Docker Desktop/Redis 未启动。\n` +
      `         请先启动 Docker Desktop 并执行 \`docker compose up -d\`（详见 docs/dev-environment.md §1/§3）。`,
  );
  process.exitCode = 1;
} finally {
  redis.disconnect();
}
