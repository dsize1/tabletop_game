// 六边形底盘加工：从实物照裁出六边形、掏空中央骰盘、去台面、摆正。
//
// 坐标基于 **900px 宽**的底图，用 `assets-src/derived/board-*-grid.jpg`（网格标定图）量取。
// 换底图时重新量一次 OUTER / INNER 即可（见 CALIB）。

import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";

const W = 900;

/** 每张底图一套标定：outer 为外轮廓，inner 为中央骰盘开口，rot 为去斜角度。 */
const CALIB = {
  // ComfyUI 清理后的赌场盘（去骰子袋；AI 生成补全）
  "board-clean.png": {
    rot: -1.5,
    outer: [
      [470, 45],
      [735, 200],
      [735, 485],
      [470, 600],
      [200, 480],
      [200, 195],
    ],
    inner: [
      [470, 240],
      [585, 300],
      [585, 415],
      [470, 470],
      [355, 415],
      [355, 300],
    ],
  },
  // ComfyUI 二次清理版（开口放大到木框内沿并外扩，吃掉黑色残边）
  "board.png": {
    rot: -1.5,
    outer: [
      [470, 45],
      [735, 200],
      [735, 485],
      [470, 600],
      [200, 480],
      [200, 195],
    ],
    inner: [
      [470, 203],
      [592, 279],
      [592, 416],
      [470, 487],
      [348, 416],
      [348, 279],
    ],
  },
  // 旧照片（保留兜底）
  default: {
    rot: -2.5,
    outer: [
      [455, 57],
      [740, 205],
      [740, 450],
      [450, 595],
      [195, 450],
      [195, 205],
    ],
    inner: [
      [462, 243],
      [578, 300],
      [578, 375],
      [462, 432],
      [346, 375],
      [346, 300],
    ],
  },
};

function subpath(points, k) {
  const pts = points.map(([x, y]) => `${(x * k).toFixed(1)},${(y * k).toFixed(1)}`).join(" L ");
  return `M ${pts} Z`;
}

/**
 * @param {string} srcPath 照片路径
 * @param {string} outDir  发布目录
 */
export async function renderBoard(srcPath, outDir) {
  const calib = CALIB[path.basename(srcPath)] ?? CALIB.default;

  const { data, info } = await sharp(srcPath)
    .rotate()
    .resize({ width: W })
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  const w = info.width;
  const h = info.height;
  const k = w / W;

  const maskSvg =
    `<svg width="${w}" height="${h}">` +
    `<path d="${subpath(calib.outer, k)} ${subpath(calib.inner, k)}" fill="#fff" fill-rule="evenodd"/>` +
    `</svg>`;
  const mask = await sharp(Buffer.from(maskSvg)).png().toBuffer();

  const masked = await sharp(data, { raw: { width: w, height: h, channels: 4 } })
    .composite([{ input: mask, blend: "dest-in" }])
    .png()
    .toBuffer();

  const buf = await sharp(masked)
    .rotate(calib.rot, { background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .trim({ background: { r: 0, g: 0, b: 0, alpha: 0 }, threshold: 6 })
    .resize(760, 760, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .webp({ quality: 80, alphaQuality: 90, effort: 6 })
    .toBuffer();

  const hash = createHash("sha1").update(buf).digest("hex").slice(0, 8);
  const name = `board-hex-${hash}.webp`;
  await fs.writeFile(path.join(outDir, name), buf);
  return { id: "board", name, bytes: buf.length, w: 760, h: 760 };
}
