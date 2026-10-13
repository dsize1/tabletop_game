#!/usr/bin/env node
// 安全停止本地开发服务链（按 data/run/dev-up.pid 结束整棵进程树）。
//
// 为什么需要它：dev-up 视「子进程意外退出」为致命，会连带停掉 web/agent/qq/decision。
// 因此**不要**直接 taskkill 单个子进程（如 server.exe）；统一走本脚本或 /apps-startup stop。
//
// 用法（仓库根）：node apps/scripts/dev-down.mjs [--keep-redis]
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, unlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const pidFile = join(repoRoot, "data", "run", "dev-up.pid");
const keepRedis = process.argv.includes("--keep-redis");

function killTree(pid) {
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/F", "/T", "/PID", String(pid)], { stdio: "ignore" });
  } else {
    try {
      process.kill(-pid, "SIGTERM");
    } catch {
      try {
        process.kill(pid, "SIGTERM");
      } catch {
        /* ignore */
      }
    }
  }
}

const stopped = [];
if (existsSync(pidFile)) {
  const pid = Number(readFileSync(pidFile, "utf8").trim());
  if (Number.isFinite(pid) && pid > 0) {
    killTree(pid);
    stopped.push(`dev-up(${pid})`);
  }
  try {
    unlinkSync(pidFile);
  } catch {
    /* ignore */
  }
} else {
  console.log("[dev-down] 无 PID 文件：未发现由 dev-up 启动的服务链");
}

if (!keepRedis) {
  const r = spawnSync("docker", ["stop", "tabletop-game-redis"], {
    encoding: "utf8",
    cwd: repoRoot,
  });
  if (r.status === 0) stopped.push("redis(tabletop-game-redis)");
}

console.log(stopped.length ? `[dev-down] 已停止：${stopped.join(", ")}` : "[dev-down] 未发现运行中的服务链");
