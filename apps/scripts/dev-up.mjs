#!/usr/bin/env node
// 一键启动开发服务链：
//   Redis(探测/拉起) → cargo build(可选) → server → decision(可选) → web → agent-gateway → qq-gateway(可选)
//
// 用法（在仓库根目录）：
//   node apps/scripts/dev-up.mjs [--with-qq] [--no-decision] [--no-build]
//   npm run dev:up            # 同上
//   npm run dev:up:qq         # 含 qq-gateway
//
// 前台运行，Ctrl+C 停止全部子进程（含 MCP / llama-server 等进程树）。

import { spawn, spawnSync } from "node:child_process";
import { createConnection } from "node:net";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const args = process.argv.slice(2);
const FLAGS = {
  qq: args.includes("--with-qq"),
  decision: !args.includes("--no-decision"),
  build: !args.includes("--no-build"),
};

const children = [];
let stopping = false;

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
    console.warn("[warn] Redis 仍不可达（127.0.0.1:6379）。请确认 Docker Desktop 已启动，否则 server/agent-gateway 无法工作。");
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
  launch("server", serverBin, [], { cwd: repoRoot });
}

// 4. decision-service（可选）
if (FLAGS.decision) {
  launch("decision", node, [resolve(repoRoot, "apps", "scripts", "start-decision-service.mjs")], {
    cwd: repoRoot,
  });
}

// 5. web
launch("web", node, [tsxCli, "src/index.ts"], { cwd: resolve(repoRoot, "apps", "web") });

// 6. agent-gateway
launch("agent", node, [tsxCli, "src/index.ts"], {
  cwd: resolve(repoRoot, "apps", "agent-gateway"),
});

// 7. qq-gateway（可选）
if (FLAGS.qq) {
  launch("qq", node, [tsxCli, "src/index.ts"], { cwd: resolve(repoRoot, "apps", "qq-gateway") });
}

const started = [
  "server",
  ...(FLAGS.decision ? ["decision"] : []),
  "web",
  "agent",
  ...(FLAGS.qq ? ["qq"] : []),
].join(" / ");

console.log(`
已启动：${started}
  server  http://127.0.0.1:3000
  web     http://127.0.0.1:18080
Ctrl+C 停止全部。`);
