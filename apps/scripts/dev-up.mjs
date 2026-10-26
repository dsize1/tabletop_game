#!/usr/bin/env node
// 一键启动开发服务链：
//   Redis(探测/拉起) → cargo build(可选) → server → decision(可选) → web → props-admin → agent-gateway → qq-gateway(默认)
//
// 用法（在仓库根目录）：
//   node apps/scripts/dev-up.mjs [--no-qq] [--no-decision] [--no-build] [--no-props]
//   npm run dev:up            # 默认含 qq-gateway（缺 QQBOT 凭证时自动跳过）
//   npm run dev:up:noqq       # 不启动 qq-gateway
//   npm run dev:up:qq         # 兼容旧命令，等价于 dev:up
//
// 前台运行，Ctrl+C 停止全部子进程（含 MCP / llama-server 等进程树）。

import { spawn, spawnSync } from "node:child_process";
import { createConnection } from "node:net";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const args = process.argv.slice(2);
const FLAGS = {
  // qq-gateway 默认启动：忘了加 --with-qq 会导致「QQ 群收不到/发不出消息」的静默故障
  qq: !args.includes("--no-qq"),
  decision: !args.includes("--no-decision"),
  build: !args.includes("--no-build"),
  // 道具中台：默认启动，仅本机（5174）；没有安装前端依赖时自动跳过
  props: !args.includes("--no-props"),
};

const children = [];
let stopping = false;

// 从 .env 读取指定键（仅取需要的，不把全文注入所有子进程）。
// 目的：`DEV_WHITELIST` 的**判定方是 server**（ADR-0015 §2.4），但配置一直放在
// apps/agent-gateway/.env；这里显式传递给 server，避免「网关放行、server 全拒」。
function readEnvKeys(envPath, keys) {
  const out = {};
  let txt = "";
  try {
    txt = readFileSync(envPath, "utf8");
  } catch {
    return out;
  }
  for (const key of keys) {
    const m = txt.match(new RegExp(`^\\s*${key}\\s*=\\s*(.+)$`, "m"));
    if (m) out[key] = m[1].trim().replace(/^["']|["']$/g, "");
  }
  return out;
}

const sharedDevEnv = readEnvKeys(resolve(repoRoot, "apps", "agent-gateway", ".env"), [
  "DEV_WHITELIST",
]);

function launch(name, cmd, cmdArgs, opts = {}) {
  const child = spawn(cmd, cmdArgs, { stdio: ["ignore", "pipe", "pipe"], ...opts });
  const tag = `[${name}]`;
  const fwd = (stream, out) =>
    stream.on("data", (b) => {
      for (const line of String(b).split(/\r?\n/)) if (line.trim()) out.write(`${tag} ${line}\n`);
    });
  fwd(child.stdout, process.stdout);
  fwd(child.stderr, process.stderr);
  child.on("error", (e) => console.error(`${tag} 启动失败: ${e.message}`));
  child.on("exit", (code, sig) => {
    if (stopping) return;
    console.error(`${tag} 意外退出 code=${code} signal=${sig}`);
    shutdown(code ?? 1);
  });
  children.push(child);
  return child;
}

function killTree(child) {
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/F", "/T", "/PID", String(child.pid)], { stdio: "ignore" });
    } else {
      child.kill();
    }
  } catch {}
}

function shutdown(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const c of children) killTree(c);
  setTimeout(() => process.exit(code), 500);
}
process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

function oneShot(name, cmd, cmdArgs, opts = {}) {
  const r = spawnSync(cmd, cmdArgs, { stdio: "inherit", ...opts });
  if (r.error) console.warn(`[warn] ${name}: ${r.error.message}（继续）`);
  else if (r.status !== 0) console.warn(`[warn] ${name} 退出码 ${r.status}（继续）`);
  return r.status === 0;
}

function redisUp() {
  return new Promise((resolveP) => {
    const sock = createConnection({ host: "127.0.0.1", port: 6379 });
    const done = (ok) => {
      try { sock.destroy(); } catch {}
      resolveP(ok);
    };
    sock.once("connect", () => done(true));
    sock.once("error", () => done(false));
    sock.setTimeout(1000, () => done(false));
  });
}

const node = process.execPath;
const tsxCli = resolve(repoRoot, "apps", "node_modules", "tsx", "dist", "cli.mjs");
const serverBin = resolve(
  repoRoot,
  "crates",
  "target",
  "debug",
  process.platform === "win32" ? "server.exe" : "server",
);

console.log("== dev-up：一键启动开发服务链 ==");

// 1. Redis
if (!(await redisUp())) {
  console.log("[redis] 未运行，尝试 docker compose up -d ...");
  oneShot("docker", "docker", ["compose", "up", "-d"], { cwd: repoRoot });
  if (!(await redisUp())) {
    // 快速失败：Redis 是 server/agent-gateway/qq-gateway 的硬依赖。
    // 若继续启动，server 会立刻因连接被拒而退出，dev-up 视其为致命并连带停掉整条链路，
    // 表现为「已启动」后随即全部消失的迷惑故障。这里直接给出可操作指引并退出。
    console.error(
      "[error] Redis 不可达（127.0.0.1:6379），且 docker compose up -d 未能拉起。\n" +
        "        请先启动 Docker Desktop，确认 `docker compose up -d` 成功（`docker exec tabletop-game-redis redis-cli ping` 应返回 PONG）后重试。\n" +
        "        详见 docs/dev-environment.md §1/§3。",
    );
    process.exit(1);
  }
} else {
  console.log("[redis] 已运行");
}

// 2. cargo build（可选）
if (FLAGS.build) {
  console.log("[build] cargo build ...");
  oneShot("cargo", "cargo", ["build"], { cwd: resolve(repoRoot, "crates") });
}

// 3. server
if (!existsSync(serverBin)) {
  console.warn(`[warn] 找不到 server 二进制：${serverBin}\n  请先 cargo build（去掉 --no-build 会自动编译）。`);
} else {
  launch("server", serverBin, [], {
    cwd: repoRoot,
    env: { ...process.env, ...sharedDevEnv },
  });
}

// 4. decision-service（可选）
if (FLAGS.decision) {
  launch("decision", node, [resolve(repoRoot, "apps", "scripts", "start-decision-service.mjs")], {
    cwd: repoRoot,
  });
}

// 5. web
launch("web", node, [tsxCli, "src/index.ts"], { cwd: resolve(repoRoot, "apps", "web") });

// 5.5 props-admin（仅本机；不挂公网）
let propsStarted = false;
if (FLAGS.props) {
  const viteBin = resolve(repoRoot, "apps", "node_modules", "vite", "bin", "vite.js");
  if (!existsSync(viteBin)) {
    console.warn("[warn] 未安装 props-admin 前端依赖，跳过中台（先 cd apps && npm install）。");
  } else {
    launch("props", node, [viteBin], {
      cwd: resolve(repoRoot, "apps", "props-admin"),
      env: { ...process.env, SERVER_URL: process.env.SERVER_URL ?? `http://127.0.0.1:${process.env.SERVER_PORT ?? "3000"}` },
    });
    propsStarted = true;
  }
}

// 6. agent-gateway
launch("agent", node, [tsxCli, "src/index.ts"], {
  cwd: resolve(repoRoot, "apps", "agent-gateway"),
});

// 7. qq-gateway（默认启动；缺凭证则跳过，避免子进程退出连带停掉整条链路）
function qqCredsReady() {
  const envPath = resolve(repoRoot, "apps", "qq-gateway", ".env");
  try {
    const txt = readFileSync(envPath, "utf8");
    return (
      /^\s*QQBOT_APP_ID\s*=\s*\S+/m.test(txt) &&
      /^\s*QQBOT_APP_SECRET\s*=\s*\S+/m.test(txt)
    );
  } catch {
    return false;
  }
}

const startQQ = FLAGS.qq && qqCredsReady();
if (FLAGS.qq && !startQQ) {
  console.warn(
    "[warn] 需要 qq-gateway，但 apps/qq-gateway/.env 缺 QQBOT_APP_ID/QQBOT_APP_SECRET，已跳过（配好凭证后重跑，或用 --no-qq 显式关闭）。",
  );
}
if (startQQ) {
  launch("qq", node, [tsxCli, "src/index.ts"], { cwd: resolve(repoRoot, "apps", "qq-gateway") });
}

const started = [
  "server",
  ...(FLAGS.decision ? ["decision"] : []),
  "web",
  ...(propsStarted ? ["props"] : []),
  "agent",
  ...(startQQ ? ["qq"] : []),
].join(" / ");

const propsLine = propsStarted
  ? "\n  props   http://127.0.0.1:5174/props-admin/（仅本机，不挂公网）"
  : "";
console.log(`
已启动：${started}
  server  http://127.0.0.1:3000
  web     http://127.0.0.1:18080${propsLine}
Ctrl+C 停止全部。`);
