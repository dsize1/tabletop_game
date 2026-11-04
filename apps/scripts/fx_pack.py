#!/usr/bin/env python3
"""把单张 ComfyUI 黑底 VFX 打包成「透明背景 + 低分辨率」的动画 WebP。

用法：
  python fx_pack.py <src.png> <out.webp> --kind slash --size 256 --quality 52

输出（stdout JSON）：{"width":256,"height":256,"frames":3,"duration":110}

设计：
- alpha = max(R,G,B) → 黑底透明；
- 裁掉透明留白；
- 3 帧：轻微缩放/旋转/位移（不依赖 AnimateDiff）；
- Pillow 保存 animated WebP（带 alpha），循环播放；运行时按 cue 时长移除。
"""
import argparse
import json
import math
from PIL import Image, ImageChops


def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("src", nargs="+", help="单张源图，或多帧源图（按顺序）")
    p.add_argument("--out", required=True)
    p.add_argument("--kind", default="slash")
    p.add_argument("--size", type=int, default=256)
    p.add_argument("--quality", type=int, default=52)
    p.add_argument("--frames", type=int, default=5)
    p.add_argument("--duration", type=int, default=80)
    return p.parse_args()


def key_alpha(im: Image.Image) -> Image.Image:
    im = im.convert("RGBA")
    r, g, b, _ = im.split()
    alpha = ImageChops.lighter(ImageChops.lighter(r, g), b)
    im.putalpha(alpha)
    return im


def trim(im: Image.Image, margin: int = 4) -> Image.Image:
    bbox = im.getbbox()
    if not bbox:
        return im
    l, t, r, b = bbox
    return im.crop(
        (
            max(0, l - margin),
            max(0, t - margin),
            min(im.width, r + margin),
            min(im.height, b + margin),
        )
    )


def motion(kind: str, i: int, n: int):
    """返回 (scale, rotate_deg, dx, dy)；i∈[0,n)。"""
    t = 0 if n <= 1 else (i / (n - 1)) * 2 - 1  # -1..1
    if kind in ("slash", "dodge"):
        return (1.0 + 0.14 * t, 18 * t, 12 * t, 0)
    if kind in ("thunder", "thunder_flash"):
        return (1.04 - 0.16 * abs(t), 3 * t, 0, 3 * t)
    if kind == "chain":
        return (1.0 + 0.08 * abs(t), 8 * t, 0, 0)
    if kind in ("peach", "wine", "heal"):
        return (0.86 + 0.28 * (t + 1) / 2, 0, 0, 22 - 22 * ((t + 1) / 2))
    return (1.0 + 0.12 * t, 0, 0, 8 * t)


def main():
    a = parse_args()
    size = a.size
    canvas_size = size
    frames = []

    def fit_canvas(im: Image.Image, scale: float = 0.92) -> Image.Image:
        im = trim(key_alpha(im))
        k = min((size * scale) / max(1, im.width), (size * scale) / max(1, im.height))
        return im.resize(
            (max(1, round(im.width * k)), max(1, round(im.height * k))),
            Image.Resampling.LANCZOS,
        )

    if len(a.src) > 1:
        # 逐帧原画模式：每张源图就是一帧（保持顺序）。
        for src in a.src:
            f = fit_canvas(Image.open(src))
            canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
            canvas.alpha_composite(
                f,
                (
                    round((canvas_size - f.width) / 2),
                    round((canvas_size - f.height) / 2),
                ),
            )
            frames.append(canvas)
    else:
        base = fit_canvas(Image.open(a.src[0]))
        for i in range(a.frames):
            scale, rot, dx, dy = motion(a.kind, i, a.frames)
            f = base.resize(
                (max(1, round(base.width * scale)), max(1, round(base.height * scale))),
                Image.Resampling.LANCZOS,
            )
            if rot:
                f = f.rotate(rot, resample=Image.Resampling.BICUBIC, expand=True)
            canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
            canvas.alpha_composite(
                f,
                (
                    round((canvas_size - f.width) / 2 + dx),
                    round((canvas_size - f.height) / 2 + dy),
                ),
            )
            frames.append(canvas)
    frames[0].save(
        a.out,
        save_all=True,
        append_images=frames[1:],
        duration=a.duration,
        loop=0,
        lossless=False,
        quality=a.quality,
        method=6,
    )
    print(
        json.dumps(
            {
                "width": canvas_size,
                "height": canvas_size,
                "frames": len(frames),
                "duration": a.duration,
            }
        )
    )


if __name__ == "__main__":
    main()
