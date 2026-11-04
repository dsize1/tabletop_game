#!/usr/bin/env node
// 素材入中台 harness（ADR-0036）。
//
// 起临时 server + web（独立端口/SQLite/素材目录）→ 上传原图/缩略图 → 绑定武将 →
// asset-map 投影 → web `/game-assets` 静态缓存/404 → 越权 401 → 引用保护删除。
//
// 用法：npm run check:sgs-assets
// 依赖：Redis、已构建的 server；缺依赖 → 打印 skip 并以 0 退出。

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "../..");
const TMP_ROOT = await fs.mkdtemp(path.join(os.tmpdir(), "sgs_assets_e2e_"));
const SQLITE_PATH = path.join(TMP_ROOT, "e2e.db");
const ASSETS_DIR = path.join(TMP_ROOT, "assets");
const SERVER_PORT = Number(process.env.SGS_ASSETS_E2E_PORT ?? 3198);
const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`;
const WEB_PORT = Number(process.env.SGS_ASSETS_E2E_WEB_PORT ?? 18198);
const WEB_URL = `http://127.0.0.1:${WEB_PORT}`;
const REDIS_URL = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const ADMIN_TOKEN = "assets-e2e-token";
const ADMIN_ACTOR = "assets-admin";
const children = [];
let failed = 0;

function log(msg) {
  console.log(`[sgs-assets-e2e] ${msg}`);
}
function assert(cond, msg) {
  if (cond) log(`✅ ${msg}`);
  else {
    failed += 1;
    console.error(`[sgs-assets-e2e] ❌ ${msg}`);
  }
}
function killTree(child) {
  if (!child || child.killed) return;
  try {
    if (process.platform === "win32") {
      spawnSync("taskkill", ["/F", "/T", "/PID", String(child.pid)], { stdio: "ignore" });
    } else {
      child.kill("SIGTERM");
    }
  } catch {
    /* ignore */
  }
}
async function cleanup() {
  for (const c of children) killTree(c);
  await fs.rm(TMP_ROOT, { recursive: true, force: true }).catch(() => undefined);
}
function skip(msg) {
  log(`skip: ${msg}`);
  void cleanup().then(() => process.exit(0));
}
async function tcpOk(host, port) {
  return new Promise((resolve) => {
    const s = net.connect({ host, port }, () => {
      s.destroy();
      resolve(true);
    });
    s.on("error", () => resolve(false));
    setTimeout(() => {
      s.destroy();
      resolve(false);
    }, 800);
  });
}
async function waitHealth(url, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    try {
      const r = await fetch(url);
      if (r.ok) return true;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}
function headers({ actor = ADMIN_ACTOR, token = ADMIN_TOKEN } = {}) {
  const h = { "content-type": "application/json" };
  if (token) h["x-props-admin-token"] = token;
  if (actor) h["x-props-actor"] = actor;
  return h;
}
async function api(pathname, { method = "GET", body, actor, token, contentType } = {}) {
  const h = headers({ actor, token });
  if (contentType) h["content-type"] = contentType;
  const r = await fetch(`${SERVER_URL}${pathname}`, {
    method,
    headers: h,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await r.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text };
  }
  return { status: r.status, body: json };
}

const redisPort = Number(new URL(REDIS_URL).port || 6379);
if (!(await tcpOk("127.0.0.1", redisPort))) skip(`Redis 不可用（${REDIS_URL}），auth 服务需要它`);

const serverExe = path.join(
  projectRoot,
  "crates/target/debug",
  process.platform === "win32" ? "server.exe" : "server",
);
if (!existsSync(serverExe)) skip("server 未构建（cargo build -p server）");

try {
  const server = spawn(serverExe, [], {
    cwd: projectRoot,
    env: {
      ...process.env,
      SQLITE_PATH,
      SERVER_PORT: String(SERVER_PORT),
      REDIS_URL,
      PROPS_ADMIN_TOKEN: ADMIN_TOKEN,
      DEV_WHITELIST: ADMIN_ACTOR,
      GAMEPROPS_ASSETS_DIR: ASSETS_DIR,
      RUST_LOG: "warn",
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  children.push(server);
  if (!(await waitHealth(`${SERVER_URL}/health`))) throw new Error("server 未就绪");

  // 1) 包 + 武将（绑定目标；engine 数据未同步时手工建最小行）
  const pack = await api("/internal/gameprops/prop_packs", {
    method: "POST",
    body: {
      game_id: "sanguosha",
      kind: "general",
      code: "standard",
      name: "标准",
      sort_order: 1,
      enabled: 1,
    },
  });
  assert(pack.status === 200 && pack.body?.row?.id, "创建道具包");
  const packId = pack.body.row.id;
  const gen = await api("/internal/gameprops/sgs_generals", {
    method: "POST",
    body: {
      code: "sgs_hero_1",
      name: "刘备",
      kingdom: "shu",
      pack_id: packId,
      max_hp: 4,
      status: "live",
    },
  });
  assert(gen.status === 200, "创建武将行");

  // 2) 上传原图 + 缩略图（真实 PNG，内容寻址）
  const fullPng = await sharp({
    create: {
      width: 64,
      height: 90,
      channels: 4,
      background: { r: 120, g: 40, b: 40, alpha: 1 },
    },
  })
    .png()
    .toBuffer();
  const thumbPng = await sharp(fullPng).resize(32, 45).png().toBuffer();
  const crop = JSON.stringify({ x: 0, y: 0, w: 64, h: 90, src_w: 64, src_h: 90 });
  const uploadQuery =
    `game_id=sanguosha&kind=general&width=64&height=90` +
    `&filename=${encodeURIComponent("刘备.png")}&crop=${encodeURIComponent(crop)}`;
  const up = await api(`/internal/gameprops/assets/upload?${uploadQuery}`, {
    method: "POST",
    body: fullPng,
    contentType: "image/png",
  });
  assert(up.status === 200 && up.body?.asset?.id, "上传原图（raw body）");
  const asset = up.body?.asset;
  assert(typeof asset?.sha256 === "string" && asset.sha256.length === 64, "服务端计算 sha256");
  assert(asset?.path?.startsWith("sanguosha/general/"), "内容寻址路径（不含客户端文件名）");
  assert(asset?.url === `/game-assets/${asset.path}`, "返回同名源 URL");
  assert(asset?.crop_json === crop, "crop_json 审计参数落库");

  const tq = new URLSearchParams({ width: "32", height: "45" });
  const th = await api(`/internal/gameprops/assets/${asset.id}/thumb?${tq}`, {
    method: "POST",
    body: thumbPng,
    contentType: "image/png",
  });
  assert(th.status === 200 && th.body?.asset?.thumb_path, "上传缩略图");
  const thumbPath = th.body.asset.thumb_path;
  assert(thumbPath.endsWith(".t.png"), "缩略图路径带 .t 后缀");

  // 3) 列表 + 绑定 + asset-map
  const list = await api("/internal/gameprops/assets?game_id=sanguosha&kind=general");
  assert(
    list.status === 200 && list.body.rows.some((r) => r.id === asset.id),
    "素材列表包含新素材",
  );
  const bind = await api("/internal/gameprops/bind-asset", {
    method: "POST",
    body: { table: "sgs_generals", code: "sgs_hero_1", asset_id: asset.id },
  });
  assert(
    bind.status === 200 && bind.body?.row?.image_asset_id === asset.id,
    "绑定 image_asset_id",
  );
  const map = await api("/internal/gameprops/asset-map?game_id=sanguosha&kind=general");
  assert(
    map.status === 200 && map.body?.assets?.sgs_hero_1?.thumb === `/game-assets/${thumbPath}`,
    "asset-map 投影 code → URL（缩略图优先）",
  );

  // 4) 越权：带非白名单 actor（即使有共享 token）→ 401
  const denied = await api(
    "/internal/gameprops/assets/upload?game_id=sanguosha&kind=general&width=1&height=1",
    { method: "POST", body: fullPng, actor: "not-whitelisted", contentType: "image/png" },
  );
  assert(denied.status === 401, "非白名单 actor 上传被拒（401）");

  // 5) web 静态服务：200 + immutable；未知哈希 404
  const web = spawn(
    process.platform === "win32" ? "npx.cmd" : "npx",
    ["tsx", "src/index.ts"],
    {
      cwd: path.join(projectRoot, "apps/web"),
      env: {
        ...process.env,
        WEB_PORT: String(WEB_PORT),
        SERVER_URL,
        GAMEPROPS_ASSETS_DIR: ASSETS_DIR,
        PROPS_ADMIN_TOKEN: ADMIN_TOKEN,
      },
      stdio: ["ignore", "ignore", "pipe"],
      shell: process.platform === "win32",
    },
  );
  children.push(web);
  if (!(await waitHealth(`${WEB_URL}/health`))) throw new Error("web 未就绪");
  const img = await fetch(`${WEB_URL}/game-assets/${asset.path}`);
  assert(img.status === 200, "web /game-assets 可取原图");
  assert(
    (img.headers.get("cache-control") ?? "").includes("immutable"),
    "素材响应带 immutable 长缓存",
  );
  const missing = await fetch(`${WEB_URL}/game-assets/sanguosha/general/ffffffff.webp`);
  assert(missing.status === 404, "未知素材 404");

  // 6) 引用保护 → 解绑后删除（文件随之清理）
  const delRef = await api(`/internal/gameprops/assets/${asset.id}`, { method: "DELETE" });
  assert(delRef.status === 409, "被引用素材拒绝删除（409）");
  const unbind = await api("/internal/gameprops/bind-asset", {
    method: "POST",
    body: { table: "sgs_generals", code: "sgs_hero_1", asset_id: null },
  });
  assert(unbind.status === 200 && unbind.body?.row?.image_asset_id === null, "解绑");
  const del = await api(`/internal/gameprops/assets/${asset.id}`, { method: "DELETE" });
  assert(del.status === 200 && del.body?.deleted === true, "解绑后删除素材");
  const gone = await fetch(`${WEB_URL}/game-assets/${asset.path}`);
  assert(gone.status === 404, "删除后文件不可达");

  // 7) 目录忽略（素材不落库）
  const ignored = spawnSync("git", ["check-ignore", "data/gameprops/assets"], {
    cwd: projectRoot,
    encoding: "utf8",
  });
  assert(ignored.status === 0, "data/gameprops/assets 被 .gitignore 覆盖");
} catch (e) {
  failed += 1;
  console.error(`[sgs-assets-e2e] ❌ ${e.message}`);
} finally {
  await cleanup();
}

if (failed > 0) {
  console.error(`素材入中台 harness 未通过（${failed} 项）`);
  process.exit(1);
} else {
  console.log("✅ 素材入中台 harness 通过（上传/缩略图/绑定/映射/缓存/越权/删除保护）");
}
