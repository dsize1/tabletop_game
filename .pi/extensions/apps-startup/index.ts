/**
 * apps-startup：在 Pi TUI 中一键启动 / 停止本地服务链。
 *
 *   /apps-startup start [--no-build] [--no-qq]   启动 dev-up（server/web/agent/decision/MCP，默认含 qq-gateway）
 *   /apps-startup stop  [--keep-redis]           停止全部（dev-up 树 + 独立 qq-gateway + 残留服务；默认停 Redis 容器）
 *   /apps-startup status                         查看进程与 PID 文件状态
 *
 * 与 `npm run dev:up` / `apps/scripts/dev-up.mjs` 等价。
 * start 以 detached 后台进程 + PID 文件（data/run/dev-up.pid）运行；
 * stop 优先按 PID 树结束，再兜底按「进程名 + 可执行路径 / 命令行」清理本项目残留。
 */

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(HERE, "../../..");
const RUN_DIR = path.join(PROJECT_ROOT, "data", "run");
const LOG_DIR = path.join(PROJECT_ROOT, "logs");
const PID_FILE = path.join(RUN_DIR, "dev-up.pid");
const LOG_FILE = path.join(LOG_DIR, "dev-up.log");
const ERR_FILE = path.join(LOG_DIR, "dev-up.err.log");
const DEV_UP = path.join(PROJECT_ROOT, "apps", "scripts", "dev-up.mjs");
const REDIS_CONTAINER = "tabletop-game-redis";

const USAGE =
  "用法：\n" +
  "/apps-startup start [--no-build] [--no-qq] — 启动全部服务\n" +
  "/apps-startup stop  [--keep-redis] — 停止全部服务\n" +
  "/apps-startup status — 查看状态";

function ensureDirs() {
  fs.mkdirSync(RUN_DIR, { recursive: true });
  fs.mkdirSync(LOG_DIR, { recursive: true });
}

function readPid(): number | null {
  try {
    const n = Number(fs.readFileSync(PID_FILE, "utf8").trim());
    return Number.isFinite(n) && n > 0 ? n : null;
  } catch {
    return null;
  }
}

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** 结束进程及其子进程树（Windows: taskkill /F /T）。 */
function killTree(pid: number) {
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

/** 发现本项目相关服务进程（优先按可执行路径/命令行匹配，避免误杀同名进程）。 */
function findProjectProcs(): Array<{ pid: number; name: string }> {
  if (process.platform !== "win32") {
    const r = spawnSync("ps", ["-eo", "pid=,comm=,args="], { encoding: "utf8" });
    return String(r.stdout ?? "")
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l && (l.includes("dev-up.mjs") || l.includes(PROJECT_ROOT)))
      .map((l) => {
        const m = l.match(/^(\d+)\s+(\S+)/);
        return { pid: m ? Number(m[1]) : NaN, name: m ? m[2] : "?" };
      })
      .filter((x) => Number.isFinite(x.pid));
  }
  const root = PROJECT_ROOT.replace(/'/g, "''");
  const ps = [
    "$ErrorActionPreference='SilentlyContinue'",
    `$root='${root}'`,
    "$names=@('server.exe','game-engine-mcp.exe','llama-server.exe')",
    // 匹配：① node 进程命令行含项目根（tsx 路径）或 dev-up.mjs；② 本项目下的服务 exe（按路径或命令行）
    "Get-CimInstance Win32_Process | Where-Object { ($_.Name -eq 'node.exe' -and ($_.CommandLine -like \"*$root*\" -or $_.CommandLine -like '*dev-up.mjs*')) -or (($names -contains $_.Name) -and ($_.ExecutablePath -like \"$root*\" -or $_.CommandLine -like \"*$root*\")) } | ForEach-Object { \"$($_.ProcessId)|$($_.Name)\" }",
  ].join("; ");
  const r = spawnSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" });
  return String(r.stdout ?? "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      const [pid, name] = l.split("|");
      return { pid: Number(pid), name: name ?? "?" };
    })
    .filter((x) => Number.isFinite(x.pid));
}

function doStart(ctx: any, flags: string[]) {
  const existing = readPid();
  if (existing && isAlive(existing)) {
    ctx.ui.notify(`服务链已在运行（dev-up pid=${existing}）。如需重启请先 /apps-startup stop`, "info");
    return;
  }
  const argv = [DEV_UP];
  if (flags.includes("--no-build")) argv.push("--no-build");
  if (flags.includes("--no-qq")) argv.push("--no-qq");
  ensureDirs();
  const out = fs.openSync(LOG_FILE, "w");
  const err = fs.openSync(ERR_FILE, "w");
  const child = spawn(process.execPath, argv, {
    cwd: PROJECT_ROOT,
    detached: true,
    stdio: ["ignore", out, err],
    windowsHide: true,
  });
  child.unref();
  fs.writeFileSync(PID_FILE, String(child.pid));
  ctx.ui.notify(
    `已启动服务链（dev-up pid=${child.pid}）。\n日志：logs/dev-up.log\n稍后用 /apps-startup status 确认就绪`,
    "info",
  );
}

function doStop(ctx: any, flags: string[]) {
  const stopped: string[] = [];
  // 自我保护：不杀当前进程及其父进程（如 pi/tsx 的命令行也含项目根）
  const self = new Set([process.pid, process.ppid]);
  const pid = readPid();
  if (pid && isAlive(pid) && !self.has(pid)) {
    killTree(pid);
    stopped.push(`dev-up(${pid})`);
  }
  // 兜底：清理独立 qq-gateway 及残留服务进程
  for (const p of findProjectProcs()) {
    if (self.has(p.pid)) continue;
    if (pid && p.pid === pid) continue;
    killTree(p.pid);
    stopped.push(`${p.name}(${p.pid})`);
  }
  try {
    fs.unlinkSync(PID_FILE);
  } catch {
    /* ignore */
  }
  if (!flags.includes("--keep-redis")) {
    const r = spawnSync("docker", ["stop", REDIS_CONTAINER], { encoding: "utf8", cwd: PROJECT_ROOT });
    if (r.status === 0) stopped.push(`redis(${REDIS_CONTAINER})`);
  }
  ctx.ui.notify(stopped.length ? `已停止：\n${stopped.join("\n")}` : "未发现运行中的服务链", "info");
}

function doStatus(ctx: any) {
  const pid = readPid();
  const procs = findProjectProcs();
  const lines = [`dev-up PID 文件：${pid ?? "无"}${pid ? (isAlive(pid) ? "（存活）" : "（已退出）") : ""}`];
  if (procs.length) {
    lines.push("相关进程：", ...procs.map((p) => `  ${p.name}  pid=${p.pid}`));
  } else {
    lines.push("相关进程：无");
  }
  ctx.ui.notify(lines.join("\n"), "info");
}

export default function appsStartup(pi: ExtensionAPI) {
  pi.registerCommand("apps-startup", {
    description: "一键启动/停止本地服务链（start|stop|status）",
    getArgumentCompletions: (prefix: string) => {
      const subs = ["start", "stop", "status"];
      const f = subs.filter((s) => s.startsWith(prefix));
      return f.length ? f.map((s) => ({ value: s, label: s })) : null;
    },
    handler: async (args: string, ctx: any) => {
      const [sub, ...rest] = args.trim().split(/\s+/);
      switch (sub) {
        case "start":
          doStart(ctx, rest);
          break;
        case "stop":
          doStop(ctx, rest);
          break;
        case "status":
          doStatus(ctx);
          break;
        default:
          ctx.ui.notify(USAGE, "info");
      }
    },
  });
}
