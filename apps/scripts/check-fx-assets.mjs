#!/usr/bin/env node
// 对局特效素材门禁：fx-manifest.ts ↔ apps/web/public/fx 文件 ↔ 大小/尺寸。
//
// 预算：单文件 ≤120KB、总 ≤1.2MB、边长 128–768、至少 8 个效果。
// 用法：npm run check:fx-assets

import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const MANIFEST = path.join(ROOT, "apps", "web", "src", "view", "sgs", "fx-manifest.ts");
const DIR = path.join(ROOT, "apps", "web", "public", "fx");
const MAX_FILE = 120 * 1024;
const MAX_TOTAL = 1.2 * 1024 * 1024;
const MIN_COUNT = 8;

let failed = 0;
function fail(msg) {
  failed += 1;
  console.error(`❌ ${msg}`);
}
function log(msg) {
  console.log(`✅ ${msg}`);
}

if (!existsSync(MANIFEST)) {
  console.error(`❌ 缺少 ${path.relative(ROOT, MANIFEST)}（先 npm run gen:fx -- --assemble）`);
  process.exit(1);
}
const text = await fs.readFile(MANIFEST, "utf8");
const entries = [
  ...text.matchAll(
    /"([a-z_]+)":\s*\{\s*url:\s*"([^"]+)",\s*width:\s*(\d+),\s*height:\s*(\d+),\s*frames:\s*(\d+),\s*duration:\s*(\d+)/g,
  ),
].map((m) => ({
  id: m[1],
  url: m[2],
  width: Number(m[3]),
  height: Number(m[4]),
  frames: Number(m[5]),
  duration: Number(m[6]),
}));
if (entries.length < MIN_COUNT) fail(`manifest 只有 ${entries.length} 个效果（至少 ${MIN_COUNT}）`);
else log(`manifest ${entries.length} 个效果`);

let total = 0;
const seen = new Set();
for (const e of entries) {
  if (!e.url.startsWith("/fx-assets/")) fail(`${e.id}: URL 必须以 /fx-assets/ 开头`);
  const file = path.basename(e.url);
  if (seen.has(file)) fail(`${e.id}: 重复文件 ${file}`);
  seen.add(file);
  const abs = path.join(DIR, file);
  if (!existsSync(abs)) {
    fail(`${e.id}: 文件缺失 ${file}`);
    continue;
  }
  const buf = await fs.readFile(abs);
  total += buf.length;
  if (buf.length > MAX_FILE) fail(`${e.id}: ${Math.round(buf.length / 1024)}KB 超过 120KB`);
  const meta = await sharp(buf).metadata();
  if (!meta.width || !meta.height) fail(`${e.id}: 无法读取尺寸`);
  else if (meta.width < 128 || meta.height < 128 || meta.width > 768 || meta.height > 768) {
    fail(`${e.id}: 尺寸 ${meta.width}×${meta.height} 超出 128–768`);
  }
  if (!meta.hasAlpha) fail(`${e.id}: 缺少透明通道`);
  if ((meta.pages ?? 1) < 2) fail(`${e.id}: 不是动画（pages=${meta.pages ?? 1}）`);
  if (e.frames !== (meta.pages ?? 1)) fail(`${e.id}: manifest 帧数 ${e.frames} 与实际 ${meta.pages} 不一致`);
  if (e.duration < 60 || e.duration > 200) fail(`${e.id}: 帧间隔 ${e.duration}ms 超出 60–200`);
  if (meta.width !== e.width || meta.height !== e.height) {
    fail(`${e.id}: manifest 尺寸 ${e.width}×${e.height} 与实际 ${meta.width}×${meta.height} 不一致`);
  }
}
if (total > MAX_TOTAL) fail(`总大小 ${Math.round(total / 1024)}KB 超过 1200KB`);
else log(`总大小 ${Math.round(total / 1024)}KB / 上限 1200KB`);

// 反向检查：public/fx 下的孤儿 webp（不在 manifest）
if (existsSync(DIR)) {
  for (const f of await fs.readdir(DIR)) {
    if (/\.(webp|png|jpg)$/i.test(f) && !seen.has(f)) fail(`孤儿素材（未在 manifest）：${f}`);
  }
}

if (failed) {
  console.error(`\n特效素材门禁未通过（${failed} 项）`);
  process.exit(1);
}
console.log("✅ 特效素材门禁通过（文件/尺寸/预算/无孤儿）");
