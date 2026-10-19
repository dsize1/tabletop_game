#!/usr/bin/env node
// 版图素材加工（离线）：盘点 → 缩略拼版 → 压缩发布 → 生成 manifest。
//
// 用法：
//   node apps/scripts/board-assets.mjs inventory      # 生成 contact sheet（assets-src/derived/contact-sheet.jpg）
//   node apps/scripts/board-assets.mjs build         # 按 mapping.json 产出 WebP + 重写 manifest.ts
//   node apps/scripts/board-assets.mjs build --check-budget
//   node apps/scripts/board-assets.mjs build --scale 2   # 2x（960×640）
//
// 约定：
// - 原图只在 assets-src/las/（本地，含摄影师水印，**不入库**）；
// - 产物在 apps/web/public/board/（git 忽略），文件名带内容哈希 → 可 immutable 缓存；
// - mapping.json 记录「文件名 → 资产 id」，未填写/为 null 的跳过（版图自动回退矢量占位）。

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { renderBoard } from "./board-hex.mjs";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const SRC_DIR = path.join(ROOT, "assets-src", "las");
const DERIVED_DIR = path.join(ROOT, "assets-src", "derived");
const OUT_DIR = path.join(ROOT, "apps", "web", "public", "board");
const MANIFEST_TS = path.join(ROOT, "apps", "web", "src", "view", "board", "manifest.ts");
const MAPPING = path.join(SRC_DIR, "mapping.json");

// 体积预算（首版 1x）
const BUDGET = {
  tileMaxBytes: 35 * 1024,
  coverMaxBytes: 40 * 1024,
  totalBytes: 900 * 1024,
  firstPaintBytes: 280 * 1024,
};

const args = process.argv.slice(2);
const cmd = args[0] ?? "inventory";
const scale = args.includes("--scale") && Number(args[args.indexOf("--scale") + 1]) >= 2 ? 2 : 1;
const checkBudget = args.includes("--check-budget");

const TILE_W = 640 * scale;
const TILE_H = 426 * scale;
const COVER_W = 460;
const COVER_H = 230;

function log(...a) {
  console.log("[board-assets]", ...a);
}

async function listSources() {
  const names = await fs.readdir(SRC_DIR);
  return names.filter((n) => /\.(jpe?g|png|webp)$/i.test(n)).sort();
}

async function ensureDirs() {
  await fs.mkdir(DERIVED_DIR, { recursive: true });
  await fs.mkdir(OUT_DIR, { recursive: true });
}

/** 从文件名解析资产 id：A1–H2 / cover / board；解析不出返回 null。 */
function parseIdFromName(file) {
  const base = file.replace(/\.[a-z0-9]+$/i, "");
  const m = base.match(/(?:^|[^0-9a-z])([A-H])[\s_\-]?([12])(?![0-9a-z])/i);
  if (m) return `${m[1].toUpperCase()}${m[2]}`;
  if (/cover|封面|box|盒/i.test(base)) return "cover";
  if (/board|盘|版图|tray/i.test(base)) return "board";
  return null;
}

async function loadMapping() {
  if (!existsSync(MAPPING)) {
    const files = await listSources();
    const skeleton = Object.fromEntries(files.map((f) => [f, null]));
    await fs.writeFile(
      MAPPING,
      `${JSON.stringify(skeleton, null, 2)}\n`,
      "utf8",
    );
    log(`未找到 mapping.json，已生成骨架：${path.relative(ROOT, MAPPING)}（请填写资产 id）`);
    return skeleton;
  }
  return JSON.parse(await fs.readFile(MAPPING, "utf8"));
}

/** 加工结果拼版（人工目检用）。 */
async function previewSheet() {
  const files = (await fs.readdir(OUT_DIR)).filter((n) => n.startsWith("tile-")).sort();
  if (!files.length) {
    log("还没有加工产物，先跑 build");
    return;
  }
  const cols = 4;
  const cellW = 320;
  const cellH = 220;
  const pad = 10;
  const rows = Math.ceil(files.length / cols);
  const width = cols * cellW + (cols + 1) * pad;
  const height = rows * (cellH + 26) + (rows + 1) * pad;
  const comps = [];
  for (let i = 0; i < files.length; i += 1) {
    const f = files[i];
    const col = i % cols;
    const row = Math.floor(i / cols);
    const x = pad + col * (cellW + pad);
    const y = pad + row * (cellH + 26 + pad);
    const img = await sharp(path.join(OUT_DIR, f))
      .resize(cellW, cellH, { fit: "contain", background: { r: 20, g: 26, b: 34, alpha: 1 } })
      .jpeg({ quality: 80 })
      .toBuffer();
    comps.push({ input: img, left: x, top: y });
    const id = f.replace(/^tile-/, "").replace(/-[0-9a-f]{8}\.webp$/, "");
    comps.push({
      input: Buffer.from(
        `<svg width="${cellW}" height="24"><text x="4" y="17" font-size="15" fill="#e3b341" font-family="monospace">${id}  ${f}</text></svg>`,
      ),
      left: x,
      top: y + cellH + 1,
    });
  }
  const out = path.join(DERIVED_DIR, "preview-tiles.jpg");
  await sharp({ create: { width, height, channels: 3, background: "#0b0f14" } })
    .composite(comps)
    .jpeg({ quality: 84 })
    .toFile(out);
  log(`预览 → ${path.relative(ROOT, out)}（${files.length} 张）`);
}

/** 缩略拼版：4 列 × N 行，带序号 + 文件名尾号，便于一眼盘点。 */
async function inventory() {
  await ensureDirs();
  const files = await listSources();
  const cols = 4;
  const cellW = 380;
  const cellH = 300;
  const rows = Math.ceil(files.length / cols);
  const pad = 12;
  const width = cols * cellW + (cols + 1) * pad;
  const height = rows * (cellH + 28) + (rows + 1) * pad;

  const composites = [];
  for (let i = 0; i < files.length; i += 1) {
    const file = files[i];
    const col = i % cols;
    const row = Math.floor(i / cols);
    const x = pad + col * (cellW + pad);
    const y = pad + row * (cellH + 28 + pad);
    const buf = await sharp(path.join(SRC_DIR, file))
      .rotate()
      .resize(cellW, cellH, { fit: "contain", background: "#101418" })
      .jpeg({ quality: 78 })
      .toBuffer();
    composites.push({ input: buf, left: x, top: y });
    const label = Buffer.from(
      `<svg width="${cellW}" height="26"><rect width="${cellW}" height="26" fill="#000"/>` +
        `<text x="6" y="18" font-size="16" fill="#e3b341" font-family="monospace">` +
        `${String(i + 1).padStart(2, "0")}  ${file.slice(0, 34)}</text></svg>`,
    );
    composites.push({ input: label, left: x, top: y + cellH + 2 });
  }

  const out = path.join(DERIVED_DIR, "contact-sheet.jpg");
  await sharp({
    create: { width, height, channels: 3, background: "#0b0f14" },
  })
    .composite(composites)
    .jpeg({ quality: 82 })
    .toFile(out);
  log(`contact sheet → ${path.relative(ROOT, out)}（${files.length} 张）`);
}

function roundMask(w, h, r) {
  return Buffer.from(
    `<svg width="${w}" height="${h}"><rect x="0" y="0" width="${w}" height="${h}" rx="${r}" ry="${r}" fill="#fff"/></svg>`,
  );
}

async function renderCover(file) {
  const buf = await sharp(path.join(SRC_DIR, file))
    .rotate()
    .resize(COVER_W, COVER_H, { fit: "cover", position: "centre" })
    .composite([{ input: roundMask(COVER_W, COVER_H, 10), blend: "dest-in" }])
    .webp({ quality: 78, effort: 6 })
    .toBuffer();
  const hash = createHash("sha1").update(buf).digest("hex").slice(0, 8);
  const name = `cover-${hash}.webp`;
  await fs.writeFile(path.join(OUT_DIR, name), buf);
  return { id: "cover", name, bytes: buf.length, w: COVER_W, h: COVER_H };
}

// ---------------------------------------------------------------------------
// 板块抠图：边缘泛洪去背景 → 去斜 → 旋转摆正 → 透明底裁切 → WebP
// ---------------------------------------------------------------------------

function colorDist(d, i, j) {
  const dr = d[i] - d[j];
  const dg = d[i + 1] - d[j + 1];
  const db = d[i + 2] - d[j + 2];
  return Math.sqrt(dr * dr + dg * dg + db * db);
}

/** 从四边泛洪标记木纹背景 → 取最大连通块（去掉杂散卡片/阴影） → bbox。 */
function segment(data, w, h, ch) {
  let rs = 0, gs = 0, bs = 0, n = 0;
  const sample = (x, y) => {
    const i = (y * w + x) * ch;
    rs += data[i]; gs += data[i + 1]; bs += data[i + 2]; n += 1;
  };
  for (let x = 0; x < w; x += 3) { sample(x, 0); sample(x, h - 1); }
  for (let y = 0; y < h; y += 3) { sample(0, y); sample(w - 1, y); }
  const seed = [rs / n, gs / n, bs / n];

  const bg = new Uint8Array(w * h);
  const queue = new Int32Array(w * h);
  let qh = 0, qt = 0;
  const push = (p) => { if (!bg[p]) { bg[p] = 1; queue[qt++] = p; } };
  for (let x = 0; x < w; x += 1) { push(x); push((h - 1) * w + x); }
  for (let y = 0; y < h; y += 1) { push(y * w); push(y * w + w - 1); }

  const tolSeed = 95;
  const tolStep = 52;
  while (qh < qt) {
    const p = queue[qh++];
    const x = p % w;
    const y = (p - x) / w;
    const i = p * ch;
    if (x > 0) { const q = p - 1; if (!bg[q] && colorDist(data, q * ch, i) <= tolStep) { const j=q*ch; if (Math.hypot(data[j]-seed[0], data[j+1]-seed[1], data[j+2]-seed[2]) <= tolSeed) { bg[q]=1; queue[qt++]=q; } } }
    if (x < w - 1) { const q = p + 1; if (!bg[q] && colorDist(data, q * ch, i) <= tolStep) { const j=q*ch; if (Math.hypot(data[j]-seed[0], data[j+1]-seed[1], data[j+2]-seed[2]) <= tolSeed) { bg[q]=1; queue[qt++]=q; } } }
    if (y > 0) { const q = p - w; if (!bg[q] && colorDist(data, q * ch, i) <= tolStep) { const j=q*ch; if (Math.hypot(data[j]-seed[0], data[j+1]-seed[1], data[j+2]-seed[2]) <= tolSeed) { bg[q]=1; queue[qt++]=q; } } }
    if (y < h - 1) { const q = p + w; if (!bg[q] && colorDist(data, q * ch, i) <= tolStep) { const j=q*ch; if (Math.hypot(data[j]-seed[0], data[j+1]-seed[1], data[j+2]-seed[2]) <= tolSeed) { bg[q]=1; queue[qt++]=q; } } }
  }

  // 最大连通块（前景）
  const label = new Int32Array(w * h).fill(-1);
  const sizes = [];
  const touches = [];
  let cur = 0;
  for (let p = 0; p < w * h; p += 1) {
    if (bg[p] || label[p] !== -1) continue;
    let head = 0, tail = 0;
    const q = new Int32Array(w * h);
    q[tail++] = p;
    label[p] = cur;
    let size = 0;
    let minX = w, minY = h, maxX = -1, maxY = -1;
    while (head < tail) {
      const c = q[head++];
      size += 1;
      const x = c % w;
      const y = (c - x) / w;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
      const nb = [];
      if (x > 0) nb.push(c - 1);
      if (x < w - 1) nb.push(c + 1);
      if (y > 0) nb.push(c - w);
      if (y < h - 1) nb.push(c + w);
      for (const d of nb) {
        if (!bg[d] && label[d] === -1) { label[d] = cur; q[tail++] = d; }
      }
    }
    sizes.push(size);
    touches.push([minX, minY, maxX, maxY]);
    cur += 1;
  }
  if (!sizes.length) return null;
  let best = 0;
  for (let i = 1; i < sizes.length; i += 1) if (sizes[i] > sizes[best]) best = i;
  if (sizes[best] < w * h * 0.05) return null;

  const fg = new Uint8Array(w * h);
  for (let p = 0; p < w * h; p += 1) if (label[p] === best) fg[p] = 1;
  // 5x5 多数滤波，去锯齿/残留
  const smooth = new Uint8Array(fg);
  for (let y = 2; y < h - 2; y += 1) {
    for (let x = 2; x < w - 2; x += 1) {
      let ones = 0;
      for (let dy = -2; dy <= 2; dy += 1) for (let dx = -2; dx <= 2; dx += 1) ones += fg[(y + dy) * w + (x + dx)];
      smooth[y * w + x] = ones >= 13 ? 1 : 0;
    }
  }
  const [minX, minY, maxX, maxY] = touches[best];
  return { bg: smooth, minX, minY, maxX, maxY, keep: best };
}

/** 用左边界线性拟合做去斜（只取中间 50% 行，避开圆角）。 */
function estimateSkew(seg, w, h) {
  const pts = [];
  const y0 = Math.floor(h * 0.25);
  const y1 = Math.floor(h * 0.75);
  for (let y = y0; y < y1; y += 2) {
    let x = 0;
    while (x < w && !seg.bg[y * w + x]) x += 1;
    if (x < w) pts.push([y, x]);
  }
  if (pts.length < 20) return 0;
  let sy = 0, sx = 0;
  for (const [y, x] of pts) { sy += y; sx += x; }
  const my = sy / pts.length;
  const mx = sx / pts.length;
  let num = 0, den = 0;
  for (const [y, x] of pts) { num += (y - my) * (x - mx); den += (y - my) ** 2; }
  if (den === 0) return 0;
  const slope = num / den; // dx/dy
  const deg = (Math.atan(slope) * 180) / Math.PI;
  return Math.max(-14, Math.min(14, deg));
}

async function renderTile(file, id, rot = 0) {
  const srcPath = path.join(SRC_DIR, file);
  const base = sharp(srcPath).rotate().resize({ width: 1200, withoutEnlargement: true });
  const { data, info } = await base.clone().ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const { width: w, height: h, channels: ch } = info;

  const seg = segment(data, w, h, ch);
  if (!seg) throw new Error("抠图失败（背景占比过高）");
  const skew = estimateSkew(seg, w, h);

  // 生成带 alpha 的 RGBA raw
  const rgba = Buffer.alloc(w * h * 4);
  for (let p = 0; p < w * h; p += 1) {
    const i = p * ch;
    rgba[p * 4] = data[i];
    rgba[p * 4 + 1] = data[i + 1];
    rgba[p * 4 + 2] = data[i + 2];
    rgba[p * 4 + 3] = seg.bg[p] ? 255 : 0;
  }

  const cut = sharp(rgba, { raw: { width: w, height: h, channels: 4 } })
    .rotate(-skew, { background: { r: 0, g: 0, b: 0, alpha: 0 } });
  const upright = rot ? cut.clone().rotate(rot, { background: { r: 0, g: 0, b: 0, alpha: 0 } }) : cut;

  // 紧贴主体裁剪后再等比缩放到固定宽度（不留透明边，锚点坐标即图上坐标）
  const trimmed = await upright
    .trim({ background: { r: 0, g: 0, b: 0, alpha: 0 }, threshold: 6 })
    .png()
    .toBuffer({ resolveWithObject: true });
  const fitted = await sharp(trimmed.data)
    .resize({ width: TILE_W, withoutEnlargement: false })
    .webp({ quality: 74, alphaQuality: 85, effort: 6 })
    .toBuffer();
  const outW = TILE_W;
  const outH = Math.round((trimmed.info.height / trimmed.info.width) * TILE_W);

  const hash = createHash("sha1").update(fitted).digest("hex").slice(0, 8);
  const name = `tile-${id}-${hash}.webp`;
  await fs.writeFile(path.join(OUT_DIR, name), fitted);
  return { id, name, bytes: fitted.length, w: outW, h: outH, skew: Number(skew.toFixed(1)) };
}

async function writeManifest(entries) {
  const byId = new Map();
  for (const e of entries) byId.set(e.id, e);
  entries = [...byId.values()];
  const lines = entries
    .sort((a, b) => a.id.localeCompare(b.id))
    .map(
      (e) =>
        `  "${e.id}": { src: "${e.name}", w: ${e.w}, h: ${e.h} },`,
    )
    .join("\n");
  const ts = `// 素材清单（id → 图片）。由 \`apps/scripts/board-assets.mjs build\` 自动生成，勿手改。
//
// 约定：
// - \`src\` 相对 \`/board-assets/\`，文件名带内容哈希（可永久缓存）；
// - 清单缺失时 \`resolveAsset\` 返回 null，版图自动回退矢量占位（不会白屏）；
// - 本文件必须保持「纯数据」，可被浏览器 bundle 与 RN 端直接引用。

export interface AssetEntry {
  src: string;
  w: number;
  h: number;
}

export const ASSETS: Record<string, AssetEntry> = {
${lines}
};

/** 取素材；未命中返回 null（调用方负责矢量回退）。 */
export function resolveAsset(id: string): AssetEntry | null {
  return ASSETS[id] ?? null;
}

/** 素材公开 URL 前缀（同域，走 Cloudflare 缓存）。 */
export const ASSET_BASE = "/board-assets/";

export function assetUrl(entry: AssetEntry): string {
  return \`\${ASSET_BASE}\${entry.src}\`;
}
`;
  await fs.writeFile(MANIFEST_TS, ts, "utf8");
  log(`manifest → ${path.relative(ROOT, MANIFEST_TS)}（${entries.length} 条）`);
}

async function build() {
  await ensureDirs();
  // 清理旧产物（文件名带内容哈希，避免堆积）
  if (process.env.BOARD_DEBUG) console.log("[dbg] outdir", OUT_DIR, (await fs.readdir(OUT_DIR)).length);
  for (const f of await fs.readdir(OUT_DIR)) {
    if (/^(tile|cover|board-hex)-.*[.]webp$/.test(f)) await fs.unlink(path.join(OUT_DIR, f));
  }
  const mapping = await loadMapping();
  const entries = [];
  const problems = [];
  let total = 0;

  for (const [file, spec] of Object.entries(mapping)) {
    const explicit = typeof spec === "object" && spec !== null && "id" in spec ? spec.id : undefined;
    const id = explicit !== undefined ? explicit : parseIdFromName(file);
    const rot = typeof spec === "string" ? 0 : Number(spec?.rot ?? 0);
    if (!id) continue;
    const srcPath = path.join(SRC_DIR, file);
    if (!existsSync(srcPath)) {
      problems.push(`缺文件：${file}`);
      continue;
    }
    try {
      if (id === "cover") {
        const e = await renderCover(file);
        if (e.bytes > BUDGET.coverMaxBytes) problems.push(`超预算 cover ${(e.bytes / 1024).toFixed(0)}KB`);
        entries.push(e);
        total += e.bytes;
        log(`cover ← ${file} → ${e.name} ${(e.bytes / 1024).toFixed(1)}KB`);
        continue;
      }
      if (id === "board") {
        const e = await renderBoard(srcPath, OUT_DIR);
        entries.push(e);
        total += e.bytes;
        log();
        continue;
      }
      const e = await renderTile(file, id, rot);
      if (e.bytes > BUDGET.tileMaxBytes) {
        problems.push(`超预算 ${id} ${(e.bytes / 1024).toFixed(0)}KB`);
      }
      entries.push(e);
      total += e.bytes;
      log(`${id.padEnd(3)} ← ${file} → ${e.name} ${(e.bytes / 1024).toFixed(1)}KB（斜 ${e.skew}° rot${rot}）`);
    } catch (err) {
      problems.push(`${id} 失败：${err.message}`);
    }
  }

  await writeManifest(entries);

  log(
    `合计 ${(total / 1024).toFixed(1)}KB / 预算 ${(BUDGET.totalBytes / 1024).toFixed(0)}KB` +
      `（首屏目标 ≤${(BUDGET.firstPaintBytes / 1024).toFixed(0)}KB）`,
  );
  if (problems.length) {
    console.error("[board-assets] 问题：");
    for (const p of problems) console.error("  -", p);
    if (checkBudget) process.exit(1);
  }
}

if (cmd === "inventory") {
  await inventory();
} else if (cmd === "build") {
  await build();
} else if (cmd === "preview") {
  await previewSheet();
} else {
  console.error(`未知命令：${cmd}（可用：inventory | build | preview）`);
  process.exit(2);
}
