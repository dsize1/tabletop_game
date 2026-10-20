#!/usr/bin/env node
// 本机部署器（B9）：构建 → 重启 dev-up → 健康检查 → 记录回滚点。
//
// 用法（仓库根）：
//   node apps/scripts/deploy-local.mjs status
//   node apps/scripts/deploy-local.mjs history
//   node apps/scripts/deploy-local.mjs deploy [--reason x] [--skip-build] [--dry-run] [--confirm]
//   node apps/scripts/deploy-local.mjs rollback --to <sha> [--confirm]
//   node apps/scripts/deploy-local.mjs serve            # 供 n8n 调用（需 DEPLOY_TOKEN）
//
// serve 模式：绑定 DEPLOY_HOST（默认 127.0.0.1；容器访问需 0.0.0.0）+ DEPLOY_PORT（8787）。
// 鉴权：x-deploy-token == DEPLOY_TOKEN。接口：GET /status、POST /deploy、POST /rollback。
//
// 安全：本机部署会重启服务进程树（dev-down → dev-up）。n8n 侧必须先经人工 approve（两段 webhook）。
import { spawn, spawnSync } from "node:child_process";
import { appendFileSync, closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import http from "node:http";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const dataDir = join(repoRoot, "data");
const historyFile = join(dataDir, "deploy-history.jsonl");
const DEV_UP = join(repoRoot, "apps", "scripts", "dev-up.mjs");
const DEV_DOWN = join(repoRoot, "apps", "scripts", "dev-down.mjs");

const args = process.argv.slice(2);
const cmd = args[0] ?? "status";
const flag = (name) => args.includes(`--${name}`);
const opt = (name, def = "") => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};

function sh(c, a, opts = {}) {
  const r = spawnSync(c, a, { cwd: repoRoot, encoding: "utf8", ...opts });
  return { code: r.status ?? 1, out: (r.stdout ?? "").trim(), err: (r.stderr ?? "").trim() };
}
const git = (...a) => sh("git", a).out;
const currentCommit = () => git("rev-parse", "--short", "HEAD");
const isClean = () => git("status", "--porcelain") === "";

function readHistory() {
  if (!existsSync(historyFile)) return [];
  return readFileSync(historyFile, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}
function lastRecord() {
  const h = readHistory();
  return h[h.length - 1] ?? null;
}
function record(rec) {
  mkdirSync(dataDir, { recursive: true });
  appendFileSync(historyFile, JSON.stringify({ ts: new Date().toISOString(), ...rec }) + "\n");
}

async function health(url = "http://127.0.0.1:3000/health") {
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(3000) });
    return r.ok;
  } catch {
    return false;
  }
}
async function waitHealth(timeoutMs = 180_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await health()) return true;
    await new Promise((r) => setTimeout(r, 2000));
  }
  return false;
}

function stopServices() {
  const r = sh(process.execPath, [DEV_DOWN, "--keep-redis"], { stdio: "pipe" });
  return r.code === 0 ? "stopped" : `stop-failed(${r.err || r.code})`;
}

function startServices() {
  mkdirSync(join(dataDir, "run"), { recursive: true });
  const out = openSync(join(repoRoot, "logs", `deploy-${Date.now()}.log`), "a");
  const err = openSync(join(repoRoot, "logs", `deploy-${Date.now()}.err.log`), "a");
  const child = spawn(process.execPath, [DEV_UP], {
    cwd: repoRoot,
    detached: true,
    stdio: ["ignore", out, err],
    env: { ...process.env },
  });
  child.unref();
  closeSync(out);
  closeSync(err);
  return child.pid;
}

async function deploy({ reason = "", skipBuild = false, dryRun = false } = {}) {
  const commit = currentCommit();
  const prev = lastRecord();
  const startedAt = new Date().toISOString();
  const log = [];
  if (dryRun) {
    log.push("dry-run：跳过构建与重启");
  } else {
    if (!skipBuild) {
      log.push("cargo build --workspace ...");
      const b = sh("cargo", ["build", "--workspace"], { cwd: join(repoRoot, "crates"), stdio: "inherit" });
      if (b.code !== 0) throw new Error("cargo build 失败");
      log.push("build ok");
    }
    log.push(stopServices());
    const pid = startServices();
    log.push(`dev-up pid=${pid}`);
    const ok = await waitHealth();
    if (!ok) {
      record({ action: "deploy", status: "failed", commit, prev_commit: prev?.commit ?? null, reason, log });
      throw new Error("健康检查超时");
    }
    log.push("health ok");
  }
  const rec = { action: "deploy", status: "succeeded", commit, prev_commit: prev?.commit ?? null, reason, log, started_at: startedAt };
  record(rec);
  return rec;
}

async function rollback({ to, dryRun = false } = {}) {
  if (!to) throw new Error("缺少 --to <sha>");
  if (!isClean()) throw new Error(`工作区不干净，拒绝回滚：${git("status", "--porcelain").split("\n").slice(0, 3).join(" | ")}`);
  const check = sh("git", ["cat-file", "-e", `${to}^{commit}`]);
  if (check.code !== 0) throw new Error(`未知 commit: ${to}`);
  const from = currentCommit();
  if (!dryRun) {
    const r = sh("git", ["checkout", to]);
    if (r.code !== 0) throw new Error(`git checkout 失败: ${r.err}`);
  }
  const rec = await deploy({ reason: `rollback ${from} -> ${to}`, dryRun });
  record({ action: "rollback", status: rec.status, from, to });
  return rec;
}

function printStatus() {
  const last = lastRecord();
  console.log(
    JSON.stringify(
      {
        commit: currentCommit(),
        clean: isClean(),
        last,
        history_file: historyFile,
      },
      null,
      2,
    ),
  );
}

function json(res, code, body) {
  res.writeHead(code, { "content-type": "application/json; charset=utf-8" });
  res.end(JSON.stringify(body));
}

async function serve() {
  const token = process.env.DEPLOY_TOKEN ?? "";
  if (!token) {
    console.error("[deploy] DEPLOY_TOKEN 未配置，拒绝启动 serve");
    process.exit(2);
  }
  const host = process.env.DEPLOY_HOST ?? "127.0.0.1";
  const port = Number(process.env.DEPLOY_PORT ?? 8787);
  const dryRun = process.env.DEPLOY_DRY_RUN === "1";

  const server = http.createServer(async (req, res) => {
    if (req.headers["x-deploy-token"] !== token) return json(res, 401, { error: "bad token" });
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", async () => {
      try {
        if (req.method === "GET" && req.url === "/status") return json(res, 200, { commit: currentCommit(), last: lastRecord() });
        const data = body ? JSON.parse(body) : {};
        if (req.method === "POST" && req.url === "/deploy") {
          const rec = await deploy({ reason: data.reason ?? "", dryRun });
          return json(res, 200, rec);
        }
        if (req.method === "POST" && req.url === "/rollback") {
          const rec = await rollback({ to: data.to, dryRun });
          return json(res, 200, rec);
        }
        return json(res, 404, { error: "not found" });
      } catch (e) {
        return json(res, 500, { error: String(e?.message ?? e) });
      }
    });
  });
  server.listen(port, host, () => console.log(`[deploy] listening http://${host}:${port} dryRun=${dryRun}`));
}

async function main() {
  if (cmd === "status") return printStatus();
  if (cmd === "history") return console.log(JSON.stringify(readHistory().slice(-10), null, 2));
  if (cmd === "serve") return serve();
  if (cmd === "deploy") {
    if (!flag("confirm") && !flag("dry-run")) {
      console.error("拒绝：deploy 需 --confirm（或用 --dry-run 演练）");
      process.exit(1);
    }
    const rec = await deploy({ reason: opt("reason"), skipBuild: flag("skip-build"), dryRun: flag("dry-run") });
    return console.log(JSON.stringify(rec, null, 2));
  }
  if (cmd === "rollback") {
    if (!flag("confirm") && !flag("dry-run")) {
      console.error("拒绝：rollback 需 --confirm（或用 --dry-run 演练）");
      process.exit(1);
    }
    const rec = await rollback({ to: opt("to"), dryRun: flag("dry-run") });
    return console.log(JSON.stringify(rec, null, 2));
  }
  console.error(`未知命令: ${cmd}`);
  process.exit(1);
}

main().catch((e) => {
  console.error("[deploy] 失败:", e?.message ?? e);
  process.exit(1);
});
