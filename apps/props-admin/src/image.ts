// 浏览器端裁剪/压缩工具（P1 入中台）：Canvas 完成裁剪 + WebP 导出 + 缩略图。
//
// 约定：武将卡面固定 512×720、缩略图 256×360；其他 kind 用 3:4。
// 不保留原始大图，只把裁剪参数写入 `prop_assets.crop_json` 供审计。

export interface CropRect {
  x: number;
  y: number;
  w: number;
  h: number;
  src_w: number;
  src_h: number;
}

export interface AssetExport {
  full: Blob;
  thumb: Blob;
  thumbW: number;
  thumbH: number;
  cropJson: string;
  mime: string;
}

export interface KindPreset {
  w: number;
  h: number;
  label: string;
}

export const KIND_PRESETS: Record<string, KindPreset> = {
  general: { w: 512, h: 720, label: "武将卡面 512×720" },
  identity: { w: 384, h: 512, label: "身份图 384×512" },
  card: { w: 384, h: 512, label: "卡牌图 384×512" },
};

export function presetOf(kind: string): KindPreset {
  return KIND_PRESETS[kind] ?? KIND_PRESETS.general;
}

export function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("图片读取失败（仅支持浏览器可解码的 png/jpeg/webp）"));
    };
    img.src = url;
  });
}

/** 覆盖式缩放：保证图片至少铺满框。 */
export function coverScale(imgW: number, imgH: number, frameW: number, frameH: number): number {
  return Math.max(frameW / Math.max(1, imgW), frameH / Math.max(1, imgH));
}

/** 限制平移范围（图片不能露出框外空白）。 */
export function clampOffset(
  imgW: number,
  imgH: number,
  frameW: number,
  frameH: number,
  zoom: number,
  offsetX: number,
  offsetY: number,
): { x: number; y: number } {
  const scale = coverScale(imgW, imgH, frameW, frameH) * zoom;
  const maxX = Math.max(0, (imgW * scale - frameW) / 2);
  const maxY = Math.max(0, (imgH * scale - frameH) / 2);
  return {
    x: Math.min(maxX, Math.max(-maxX, offsetX)),
    y: Math.min(maxY, Math.max(-maxY, offsetY)),
  };
}

/** 当前视口对应的原图裁剪矩形。 */
export function cropRect(
  imgW: number,
  imgH: number,
  frameW: number,
  frameH: number,
  zoom: number,
  offsetX: number,
  offsetY: number,
): CropRect {
  const { x, y } = clampOffset(imgW, imgH, frameW, frameH, zoom, offsetX, offsetY);
  const scale = coverScale(imgW, imgH, frameW, frameH) * zoom;
  const left = (frameW - imgW * scale) / 2 + x;
  const top = (frameH - imgH * scale) / 2 + y;
  const sx = Math.max(0, -left / scale);
  const sy = Math.max(0, -top / scale);
  const sw = Math.min(imgW - sx, frameW / scale);
  const sh = Math.min(imgH - sy, frameH / scale);
  return { x: sx, y: sy, w: sw, h: sh, src_w: imgW, src_h: imgH };
}

/** 把裁剪区域画到指定尺寸的 canvas。 */
export function drawCropped(
  canvas: HTMLCanvasElement,
  img: HTMLImageElement,
  rect: CropRect,
  outW: number,
  outH: number,
): void {
  canvas.width = outW;
  canvas.height = outH;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("浏览器不支持 Canvas 2D");
  ctx.clearRect(0, 0, outW, outH);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, rect.x, rect.y, rect.w, rect.h, 0, 0, outW, outH);
}

/** 预览绘制（把当前视口按 cover 画到预览框）。 */
export function drawPreview(
  canvas: HTMLCanvasElement,
  img: HTMLImageElement,
  frameW: number,
  frameH: number,
  zoom: number,
  offsetX: number,
  offsetY: number,
): void {
  const rect = cropRect(img.width, img.height, frameW, frameH, zoom, offsetX, offsetY);
  drawCropped(canvas, img, rect, Math.round(frameW), Math.round(frameH));
}

function canvasToBlob(canvas: HTMLCanvasElement, type: string, quality: number): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => (blob ? resolve(blob) : reject(new Error("导出图片失败"))),
      type,
      quality,
    );
  });
}

/** 优先 WebP；浏览器不支持编码时回退 PNG。 */
async function exportCanvas(
  canvas: HTMLCanvasElement,
  prefer: string,
  quality: number,
): Promise<{ blob: Blob; mime: string }> {
  let blob = await canvasToBlob(canvas, prefer, quality);
  if (blob.type !== prefer) blob = await canvasToBlob(canvas, "image/png", quality);
  return { blob, mime: blob.type || "image/png" };
}

/** 裁剪 + 导出原图/缩略图（含审计参数）。 */
export async function buildAssetExport(
  img: HTMLImageElement,
  frameW: number,
  frameH: number,
  zoom: number,
  offsetX: number,
  offsetY: number,
  outW: number,
  outH: number,
): Promise<AssetExport> {
  const rect = cropRect(img.width, img.height, frameW, frameH, zoom, offsetX, offsetY);
  const fullCanvas = document.createElement("canvas");
  drawCropped(fullCanvas, img, rect, outW, outH);
  const full = await exportCanvas(fullCanvas, "image/webp", 0.82);

  const thumbW = Math.max(1, Math.round(outW / 2));
  const thumbH = Math.max(1, Math.round(outH / 2));
  const thumbCanvas = document.createElement("canvas");
  drawCropped(thumbCanvas, img, rect, thumbW, thumbH);
  const thumb = await exportCanvas(thumbCanvas, "image/webp", 0.78);

  return {
    full: full.blob,
    thumb: thumb.blob,
    thumbW,
    thumbH,
    cropJson: JSON.stringify({
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      w: Math.round(rect.w),
      h: Math.round(rect.h),
      src_w: rect.src_w,
      src_h: rect.src_h,
    }),
    mime: full.mime,
  };
}
