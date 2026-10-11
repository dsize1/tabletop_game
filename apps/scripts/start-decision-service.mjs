#!/usr/bin/env node
// 一键启动 SystemOne 决策服务（两层）：
//   llama-server                    —— GGUF 权重前向（默认 8081）
//   startlux_decision.gguf_server   —— typed-decision 读出 / 温度标定（默认 8090）
// 详见 docs/modules/decision-service.md
//
// 用法（脚本保持前台运行，Ctrl+C 同时停止两个子进程）：
//   node apps/scripts/start-decision-service.mjs
//   # 后台常驻：
//   #   nohup node apps/scripts/start-decision-service.mjs > /tmp/decision-svc.log 2>&1 &
//
// 环境变量（均可选）：
//   MODEL_DIR         模型目录（默认 <repo>/StartLux-Decision-v1-4B-Q8_0-GGUF）
//   LLAMA_SERVER_BIN  llama-server 可执行文件（默认 PATH 中的 llama-server）
//   LLAMA_PORT        llama-server 端口（默认 8081）
//   DECISION_PORT     决策服务端口（默认 8090）
//   NGL               GPU 层数（默认 99；纯 CPU 设 0）
//   CTX_SIZE          上下文长度（默认 16384）
//   PARALLEL          并行槽位数（默认 4）
//   PYTHON_BIN        Python 解释器（默认 python）

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

const MODEL_DIR = resolve(process.env.MODEL_DIR ?? resolve(repoRoot, "StartLux-Decision-v1-4B-Q8_0-GGUF"));
const LLAMA_BIN = process.env.LLAMA_SERVER_BIN ?? "llama-server";
const PYTHON_BIN = process.env.PYTHON_BIN ?? "python";
const LLAMA_PORT = Number(process.env.LLAMA_PORT ?? 8081);
const DECISION_PORT = Number(process.env.DECISION_PORT ?? 8090);
const NGL = process.env.NGL ?? "99";
const CTX = process.env.CTX_SIZE ?? "16384";
const PARALLEL = process.env.PARALLEL ?? "4";

const gguf = resolve(MODEL_DIR, "StartLux-Decision-4B-Q8_0.gguf");
if (!existsSync(gguf)) {
  console.error(`[fatal] 找不到模型文件: ${gguf}\n  MODEL_DIR=${MODEL_DIR}`);
  process.exit(1);
}

const children = [];
function launch(name, cmd, args, opts = {}) {
  const child = spawn(cmd, args, { stdio: ["ignore", "pipe", "pipe"], ...opts });
  const tag = `[${name}]`;
  const forward = (stream, out) =>
    stream.on("data", (b) => {
      for (const line of String(b).split(/\r?\n/)) if (line.trim()) out.write(`${tag} ${line}\n`);
    });
  forward(child.stdout, process.stdout);
  forward(child.stderr, process.stderr);
  child.on("error", (e) => {
    console.error(`${tag} 启动失败: ${e.message}`);
    shutdown(1);
  });
  child.on("exit", (code, sig) => {
    if (stopping) return;
    console.error(`${tag} 意外退出 code=${code} signal=${sig}`);
    shutdown(code ?? 1);
  });
  children.push(child);
  return child;
}

let stopping = false;
function shutdown(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const c of children) {
    try {
      c.kill();
    } catch {}
  }
  setTimeout(() => process.exit(code), 500);
}
process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

async function waitHealthy(url, seconds) {
  for (let i = 0; i < seconds * 2; i++) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(1500) });
      if (r.ok) return true;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  return false;
}

console.log(`[start] llama-server (${LLAMA_BIN}) -> :${LLAMA_PORT}`);
const llamaArgs = ["-m", gguf, "-c", CTX, "--parallel", PARALLEL, "--port", String(LLAMA_PORT)];
if (NGL !== "" && NGL !== "0") llamaArgs.push("-ngl", NGL);
launch("llama", LLAMA_BIN, llamaArgs, { cwd: MODEL_DIR });

if (!(await waitHealthy(`http://127.0.0.1:${LLAMA_PORT}/health`, 300))) {
  console.error("[fatal] llama-server 未就绪，退出");
  shutdown(1);
  process.exit(1);
}
console.log("[start] llama-server 就绪");

console.log(`[start] 决策服务 (${PYTHON_BIN}) -> :${DECISION_PORT}`);
launch(
  "decision",
  PYTHON_BIN,
  [
    "-m",
    "startlux_decision.gguf_server",
    "--model-dir",
    ".",
    "--llama",
    `http://127.0.0.1:${LLAMA_PORT}`,
    "--port",
    String(DECISION_PORT),
  ],
  { cwd: MODEL_DIR },
);

if (!(await waitHealthy(`http://127.0.0.1:${DECISION_PORT}/`, 120))) {
  console.error("[fatal] 决策服务未就绪，退出");
  shutdown(1);
  process.exit(1);
}

console.log("\n✅ SystemOne 决策服务已就绪");
console.log(`   llama-server : http://127.0.0.1:${LLAMA_PORT}`);
console.log(`   决策接口     : POST http://127.0.0.1:${DECISION_PORT}/v1/systemone`);
console.log("   Ctrl+C 停止\n");
