#!/usr/bin/env node
// 素材收集工作流「资源闸门」：确认项目服务全部停止、资源已释放后，才允许跑离线的
// 视频抽帧 / OCR 采集流水线；并写独占锁 data/run/media.lock，避免两条流水线并行抢 CPU。
//
// 用法（仓库根）：
//   node apps/scripts/media-gate.mjs             # 只检查（不写锁）
//   node apps/scripts/media-gate.mjs --acquire   # 检查通过后写独占锁（流水线启动前调用）
//   node apps/scripts/media-gate.mjs --release   # 释放独占锁（流水线结束时调用）
//   node apps/scripts/media-gate.mjs --force     # 跳过「服务已停」类检查，仍写锁并打印警告
//   node apps/scripts/media-gate.mjs --status    # 只打印锁状态
//
// 环境变量（可选）：
//   MEDIA_GATE_PORTS      默认 3000,18080,5174,6379,8188,4002
//   MEDIA_GATE_VRAM_MB    显存占用上限（MiB），默认 4500（无 llama-server 时本机基线 ≈2.5GB）
//   MEDIA_GATE_RAM_GB     可用物理内存下限（GiB），默认 8
//   MEDIA_GATE_DISK_GB    目标盘可用空间下限（GiB），默认 50
//   MEDIA_GATE_DISK      目标盘，默认 D:
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { statfsSync } from "node:fs";
import { createConnection } from "node:net";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const runDir = join(repoRoot, "data", "run");
const pidFile = join(runDir, "dev-up.pid");
const lockFile = join(runDir, "media.lock");

const argv = new Set(process.argv.slice(2));
const wantAcquire = argv.has("--acquire");
const wantRelease = argv.has("--release");
const wantForce = argv.has("--force");
const wantStatus = argv.has("--status");

const PORTS = (process.env.MEDIA_GATE_PORTS ?? "3000,18080,5174,6379,8188,4002")
  .split(",")
  .map((s) => Number(s.trim()))
  .filter((n) => Number.isFinite(n) && n > 0);
const VRAM_LIMIT_MB = Number(process.env.MEDIA_GATE_VRAM_MB ?? 4500);
const RAM_MIN_GB = Number(process.env.MEDIA_GATE_RAM_GB ?? 8);
const DISK_MIN_GB = Number(process.env.MEDIA_GATE_DISK_GB ?? 50);
const DISK = process.env.MEDIA_GATE_DISK ?? "D:";

/** @type {{name: string, ok: boolean, detail: string, hint?: string, blocking: boolean}[]} */
const checks = [];

function add(name, ok, detail, hint, blocking = true) {
  checks.push({ name, ok, detail, hint, blocking });
}

function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function portBusy(port) {
  return new Promise((res) => {
    const sock = createConnection({ host: "127.0.0.1", port });
    const done = (busy) => {
      sock.removeAllListeners();
      sock.destroy();
      res(busy);
    };
    sock.setTimeout(600);
    sock.once("connect", () => done(true));
    sock.once("timeout", () => done(false));
    sock.once("error", () => done(false));
  });
}

function sh(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: "utf8", windowsHide: true });
  return { ok: r.status === 0, out: `${r.stdout ?? ""}${r.stderr ?? ""}`.trim() };
}

function readLock() {
  if (!existsSync(lockFile)) return null;
  try {
    const raw = JSON.parse(readFileSync(lockFile, "utf8"));
    return { ...raw, alive: Number.isFinite(raw?.pid) ? isAlive(Number(raw.pid)) : false };
  } catch {
    return { pid: 0, alive: false, corrupt: true };
  }
}

// --- --status / --release 短路 -------------------------------------------------
if (wantStatus) {
  const lock = readLock();
  console.log(lock ? `[media-gate] 锁存在：${JSON.stringify(lock)}` : "[media-gate] 无锁（空闲）");
  process.exit(0);
}
if (wantRelease) {
  const lock = readLock();
  if (!lock) {
    console.log("[media-gate] 无锁可释放");
    process.exit(0);
  }
  if (lock.alive && Number(lock.pid) !== process.pid) {
    console.log(`[media-gate] 警告：锁被存活进程 ${lock.pid} 持有，仍按要求删除`);
  }
  unlinkSync(lockFile);
  console.log("[media-gate] 已释放独占锁 data/run/media.lock");
  process.exit(0);
}

// --- 1) dev-up 进程树 ----------------------------------------------------------
{
  let alive = false;
  let detail = "无 data/run/dev-up.pid";
  if (existsSync(pidFile)) {
    const pid = Number(readFileSync(pidFile, "utf8").trim());
    alive = Number.isFinite(pid) && pid > 0 && isAlive(pid);
    detail = alive ? `dev-up 仍在运行（PID ${pid}）` : `dev-up.pid 残留（PID ${pid} 已退出）`;
  }
  add("dev-up 进程", !alive, detail, "npm run dev:down");
}

// --- 2) 关键进程（server / llama-server / qq / vite）---------------------------
{
  const r = sh("tasklist", ["/FO", "CSV", "/NH"]);
  const out = r.out;
  const wanted = ["server.exe", "llama-server.exe", "esbuild.exe"];
  const hits = wanted.filter((name) => new RegExp(name.replace(".", "\\."), "i").test(out));
  const projectNode = /tabletop_game\\.*node\.exe/i.test(out);
  add(
    "项目进程",
    hits.length === 0,
    hits.length ? `发现 ${hits.join(", ")}` : "无 server.exe / llama-server.exe / esbuild.exe",
    "npm run dev:down（若仍残留，用 taskkill /F /T /PID <pid>）",
  );
  add("项目 node 进程", !projectNode, projectNode ? "发现以仓库为 cwd 的 node.exe" : "未发现", "npm run dev:down");
}

// --- 3) 端口 ------------------------------------------------------------------
{
  const busy = [];
  for (const port of PORTS) {
    if (await portBusy(port)) busy.push(port);
  }
  add("端口占用", busy.length === 0, busy.length ? `占用：${busy.join(", ")}` : `${PORTS.join("/")} 全部空闲`, "npm run dev:down");
}

// --- 4) Redis 容器 ------------------------------------------------------------
{
  const r = sh("docker", ["ps", "--filter", "name=tabletop-game-redis", "--format", "{{.Names}} {{.Status}}"]);
  if (!r.ok && /not recognized|no such|无法|不是内部/i.test(r.out)) {
    add("Redis 容器", true, "docker 不可用，跳过（Redis 未运行视为已停）", undefined, false);
  } else {
    const running = r.out.split("\n").filter((l) => l.trim().length > 0);
    add("Redis 容器", running.length === 0, running.length ? running.join("; ") : "tabletop-game-redis 未运行", "docker stop tabletop-game-redis");
  }
}

// --- 5) 显存 ------------------------------------------------------------------
{
  const r = sh("nvidia-smi", ["--query-gpu=memory.used", "--format=csv,noheader,nounits"]);
  if (!r.ok) {
    add("显存", true, "nvidia-smi 不可用，跳过", undefined, false);
  } else {
    const used = r.out
      .split("\n")
      .map((s) => Number(s.trim()))
      .filter((n) => Number.isFinite(n));
    const max = used.length ? Math.max(...used) : 0;
    add("显存占用", max <= VRAM_LIMIT_MB, `${max} MiB / 上限 ${VRAM_LIMIT_MB} MiB`, "确认 llama-server / ComfyUI 已退出");
  }
}

// --- 6) 内存 / 磁盘 ------------------------------------------------------------
{
  const freeGb = os.freemem() / 1024 ** 3;
  add("可用内存", freeGb >= RAM_MIN_GB, `${freeGb.toFixed(1)} GiB / 下限 ${RAM_MIN_GB} GiB`, "关闭浏览器等大内存程序");
}
{
  let freeGb = 0;
  try {
    const st = statfsSync(DISK + "\\");
    freeGb = (st.bavail * st.bsize) / 1024 ** 3;
  } catch (e) {
    freeGb = -1;
  }
  add("目标盘空间", freeGb >= DISK_MIN_GB, `${DISK} 可用 ${freeGb.toFixed(1)} GiB / 下限 ${DISK_MIN_GB} GiB`, "清理磁盘");
}

// --- 7) 独占锁 ----------------------------------------------------------------
const lock = readLock();
{
  const blocked = Boolean(lock && lock.alive);
  add(
    "独占锁",
    !blocked,
    blocked ? `已被 PID ${lock.pid}（${lock.started_at ?? "?"}）持有` : lock ? "残留锁（持有进程已退出）" : "空闲",
    "node apps/scripts/media-gate.mjs --release",
  );
}

// --- 输出 ---------------------------------------------------------------------
const blockingFails = checks.filter((c) => c.blocking && !c.ok);
const softFails = checks.filter((c) => !c.blocking && !c.ok);

console.log("=== 素材收集资源闸门 ===");
for (const c of checks) {
  const tag = c.ok ? "PASS" : c.blocking ? "FAIL" : "WARN";
  console.log(`[${tag}] ${c.name.padEnd(10, " ")} ${c.detail}`);
}

if (wantForce && blockingFails.length > 0) {
  console.log(`\n[--force] 忽略 ${blockingFails.length} 项阻断：${blockingFails.map((c) => c.name).join(", ")}`);
}

const effectiveFails = wantForce ? [] : blockingFails;
if (effectiveFails.length > 0) {
  console.log("\n闸门未通过，建议处理：");
  for (const c of effectiveFails) console.log(`  - ${c.name}：${c.hint ?? "见上"}`);
  process.exit(1);
}

if (wantAcquire) {
  mkdirSync(runDir, { recursive: true });
  if (lock?.alive && Number(lock.pid) !== process.pid) {
    console.log(`\n闸门：锁被 PID ${lock.pid} 持有，拒绝获取`);
    process.exit(1);
  }
  writeFileSync(
    lockFile,
    `${JSON.stringify({ pid: process.pid, started_at: new Date().toISOString(), host: os.hostname(), cmd: `python -m sgs_media` }, null, 2)}\n`,
  );
  console.log("\n闸门通过 → 已获取独占锁 data/run/media.lock（流水线结束时务必 --release）");
} else {
  console.log("\n闸门通过（未写锁；流水线请用 --acquire）");
  if (softFails.length > 0) console.log(`（有 ${softFails.length} 项 WARN，不阻断）`);
}
