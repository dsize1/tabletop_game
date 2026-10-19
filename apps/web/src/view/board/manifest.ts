// 素材清单（id → 图片）。由 `apps/scripts/board-assets.mjs build` 自动生成，勿手改。
//
// 约定：
// - `src` 相对 `/board-assets/`，文件名带内容哈希（可永久缓存）；
// - 清单缺失时 `resolveAsset` 返回 null，版图自动回退矢量占位（不会白屏）；
// - 本文件必须保持「纯数据」，可被浏览器 bundle 与 RN 端直接引用。

export interface AssetEntry {
  src: string;
  w: number;
  h: number;
}

export const ASSETS: Record<string, AssetEntry> = {
  "A1": { src: "tile-A1-7a3ee2f6.webp", w: 640, h: 205 },
  "A2": { src: "tile-A2-9cbd2f48.webp", w: 640, h: 198 },
  "B1": { src: "tile-B1-9ba00d3e.webp", w: 640, h: 210 },
  "B2": { src: "tile-B2-7b5decb5.webp", w: 640, h: 197 },
  "board": { src: "board-hex-c710c836.webp", w: 760, h: 760 },
  "C1": { src: "tile-C1-700f2fb4.webp", w: 640, h: 198 },
  "C2": { src: "tile-C2-632b7e61.webp", w: 640, h: 205 },
  "cover": { src: "cover-47728ec4.webp", w: 460, h: 230 },
  "D1": { src: "tile-D1-acd6e613.webp", w: 640, h: 243 },
  "D2": { src: "tile-D2-3ce50733.webp", w: 640, h: 221 },
  "E1": { src: "tile-E1-a5857479.webp", w: 640, h: 220 },
  "E2": { src: "tile-E2-84a20a79.webp", w: 640, h: 241 },
  "F1": { src: "tile-F1-161bd78b.webp", w: 640, h: 225 },
  "F2": { src: "tile-F2-15cf2d10.webp", w: 640, h: 205 },
  "G1": { src: "tile-G1-b348fdd4.webp", w: 640, h: 230 },
  "G2": { src: "tile-G2-7fe610a8.webp", w: 640, h: 217 },
  "H1": { src: "tile-H1-cf029843.webp", w: 640, h: 197 },
  "H2": { src: "tile-H2-b69b59c6.webp", w: 640, h: 250 },
};

/** 取素材；未命中返回 null（调用方负责矢量回退）。 */
export function resolveAsset(id: string): AssetEntry | null {
  return ASSETS[id] ?? null;
}

/** 素材公开 URL 前缀（同域，走 Cloudflare 缓存）。 */
export const ASSET_BASE = "/board-assets/";

export function assetUrl(entry: AssetEntry): string {
  return `${ASSET_BASE}${entry.src}`;
}
