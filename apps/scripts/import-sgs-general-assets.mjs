#!/usr/bin/env node
// 三国杀武将卡面「入中台」批量导入（P1）。
//
// 输入：data/media/sgs/out/final-manifest.json（534 张 512×720 卡面，覆盖 696 个 gid）
// 流程：sharp 压成 WebP(512×720, q82) + 缩略图(256×360, q78) → 调 server 内容寻址上传
//       → 按 manifest `gids` 绑定 `sgs_generals.code = sgs_hero_<gid>`。
//
// 幂等：素材按 sha256 去重（重复上传返回同一行）；绑定是局部 UPDATE。
//
// 用法：
//   node apps/scripts/import-sgs-general-assets.mjs            # 真跑（需 server 在 127.0.0.1:3000）
//   node apps/scripts/import-sgs-general-assets.mjs --dry-run  # 只统计/校验，不联网
//   ... --limit 20 --server http://127.0.0.1:3100 --token dev-token --no-bind
//
// 环境变量：PROPS_ADMIN_TOKEN（与 server 同值）、SERVER_URL。

import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const FINAL_DIR = path.join(ROOT, "data", "media", "sgs", "out", "final");
const DEFAULT_MANIFEST = path.join(ROOT, "data", "media", "sgs", "out", "final-manifest.json");
const DEFAULT_REPORT = path.join(ROOT, "data", "media", "sgs", "out", "import-report.json");

const args = process.argv.slice(2);
const has = (flag) => args.includes(flag);
const val = (flag, fallback = null) => {
  const i = args.indexOf(flag);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};

const DRY = has("--dry-run");
const NO_BIND = has("--no-bind");
const LIMIT = Number(val("--limit", "0")) || 0;
const SERVER = (val("--server") ?? process.env.SERVER_URL ?? "http://127.0.0.1:3000").replace(/\/+$/, "");
const TOKEN = val("--token") ?? process.env.PROPS_ADMIN_TOKEN ?? "";
const MANIFEST = path.resolve(val("--manifest", DEFAULT_MANIFEST));
const REPORT = path.resolve(val("--report", DEFAULT_REPORT));
const FULL_W = 512;
const FULL_H = 720;
const THUMB_W = 256;
const THUMB_H = 360;

function log(...a) {
  console.log("[sgs-assets]", ...a);
}

async function api(pathname, { method = "GET", body, contentType } = {}) {
  const headers = {};
  if (TOKEN) headers["x-props-admin-token"] = TOKEN;
  if (contentType) headers["content-type"] = contentType;
  const r = await fetch(`${SERVER}${pathname}`, { method, headers, body });
  const text = await r.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = { raw: text };
  }
  return { status: r.status, body: json };
}

async function ensureServer() {
  try {
    const r = await fetch(`${SERVER}/health`);
    if (!r.ok) throw new Error(`health ${r.status}`);
  } catch (e) {
    throw new Error(
      `server 不可用（${SERVER}）：先 npm run dev:up 或单独起 server（${e.message}）`,
    );
  }
}

async function main() {
  const manifest = JSON.parse(await fs.readFile(MANIFEST, "utf8"));
  const images = Array.isArray(manifest.images) ? manifest.images : [];
  if (images.length === 0) throw new Error(`manifest 无 images：${MANIFEST}`);
  const picked = LIMIT > 0 ? images.slice(0, LIMIT) : images;

  const report = {
    manifest: MANIFEST,
    dry_run: DRY,
    started_at: new Date().toISOString(),
    images: picked.length,
    uploaded: 0,
    reused: 0,
    thumbs: 0,
    thumb_failed: 0,
    bind_ok: 0,
    bind_missing: 0,
    bind_errors: 0,
    /** 已绑定 image_asset_id 的武将数（asset-map 条目；未实现武将不在库内）。 */
    bound_generals: 0,
    missing_sources: [],
    gids: 0,
    skipped_no_bind: NO_BIND,
  };

  if (!DRY) await ensureServer();

  // 预处理：源文件存在性 + 真实尺寸
  let idx = 0;
  for (const img of picked) {
    idx += 1;
    const src = path.join(FINAL_DIR, img.file);
    const gids = Array.isArray(img.gids) ? img.gids : [];
    report.gids += gids.length;
    if (!existsSync(src)) {
      report.missing_sources.push(img.file);
      log(`跳过（缺文件）：${img.file}`);
      continue;
    }
    const meta = await sharp(src).metadata();
    if (meta.width !== FULL_W || meta.height !== FULL_H) {
      log(`注意：${img.file} 尺寸 ${meta.width}×${meta.height}（将缩放为 ${FULL_W}×${FULL_H}）`);
    }
    if (DRY) {
      if (idx % 50 === 0 || idx === picked.length) log(`dry-run 校验 ${idx}/${picked.length}`);
      continue;
    }

    const fullBuf = await sharp(src)
      .resize(FULL_W, FULL_H, { fit: "cover" })
      .webp({ quality: 82, effort: 5 })
      .toBuffer();
    const thumbBuf = await sharp(src)
      .resize(THUMB_W, THUMB_H, { fit: "cover" })
      .webp({ quality: 78, effort: 5 })
      .toBuffer();

    const qs = new URLSearchParams({
      game_id: "sanguosha",
      kind: "general",
      width: String(FULL_W),
      height: String(FULL_H),
      filename: img.file,
    });
    const up = await api(`/internal/gameprops/assets/upload?${qs}`, {
      method: "POST",
      body: fullBuf,
      contentType: "image/webp",
    });
    if (up.status !== 200 || !up.body?.asset?.id) {
      log(`上传失败：${img.file} → ${up.status} ${JSON.stringify(up.body)}`);
      continue;
    }
    let asset = up.body.asset;
    if (asset.thumb_path) report.reused += 1;
    else report.uploaded += 1;

    if (!asset.thumb_path) {
      const tq = new URLSearchParams({ width: String(THUMB_W), height: String(THUMB_H) });
      const t = await api(`/internal/gameprops/assets/${asset.id}/thumb?${tq}`, {
        method: "POST",
        body: thumbBuf,
        contentType: "image/webp",
      });
      if (t.status === 200) {
        asset = t.body.asset;
        report.thumbs += 1;
      } else {
        report.thumb_failed += 1;
        log(`缩略图失败：#${asset.id} ${img.file}`);
      }
    }

    if (!NO_BIND) {
      for (const gid of gids) {
        const b = await api("/internal/gameprops/bind-asset", {
          method: "POST",
          body: JSON.stringify({
            table: "sgs_generals",
            code: `sgs_hero_${gid}`,
            asset_id: asset.id,
          }),
          contentType: "application/json",
        });
        if (b.status === 200) report.bind_ok += 1;
        else if (b.status === 400) report.bind_missing += 1;
        else report.bind_errors += 1;
      }
    }

    if (idx % 25 === 0 || idx === picked.length) {
      log(
        `${idx}/${picked.length} 张 · 上传 ${report.uploaded}（复用 ${report.reused}）· ` +
          `缩略图 ${report.thumbs} · 绑定 ${report.bind_ok}`,
      );
    }
  }

  if (!DRY && !NO_BIND) {
    const map = await api("/internal/gameprops/asset-map?game_id=sanguosha&kind=general");
    report.bound_generals = Object.keys(map.body?.assets ?? {}).length;
  }
  report.finished_at = new Date().toISOString();
  await fs.mkdir(path.dirname(REPORT), { recursive: true });
  await fs.writeFile(REPORT, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  log("完成：", JSON.stringify(report));
  log(`报告：${REPORT}`);
  if (report.missing_sources.length > 0) {
    log(`缺源文件 ${report.missing_sources.length} 个（见报告）`);
  }
}

main().catch((e) => {
  console.error(`[sgs-assets] ❌ ${e.message}`);
  process.exit(1);
});
