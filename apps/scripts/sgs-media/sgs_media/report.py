"""单集报告：特征分布表 + 按桶命中排行 + 样本拼版（contact sheet）。

「特征分布」是给人工定过滤规则用的：片头/片尾的字幕与正文人物介绍在
字号 / 位置 / 背景运动 上大概率分得开，报表把每档都配样板缩略图。
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

from .paths import episode_dirs

FONT_CANDIDATES = [
    "C:/Windows/Fonts/simhei.ttf",
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/simsun.ttc",
]


def _font(size: int) -> ImageFont.FreeTypeFont:
    for p in FONT_CANDIDATES:
        if Path(p).exists():
            try:
                return ImageFont.truetype(p, size)
            except Exception:  # noqa: BLE001
                continue
    return ImageFont.load_default()


def _load_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return out


def _bucketize(values: list[float], edges: list[tuple[str, float, float]]) -> list[tuple[str, int]]:
    out = []
    for name, lo, hi in edges:
        n = sum(1 for v in values if lo <= v < hi)
        out.append((name, n))
    return out


def _crop_box(img: Image.Image, box: dict, pad: int = 4) -> Image.Image:
    w, h = img.size
    bx, by, bw, bh = box["box"]
    left = max(bx - pad, 0)
    top = max(by - pad, 0)
    right = min(bx + bw + pad, w)
    bottom = min(by + bh + pad, h)
    return img.crop((left, top, right, bottom))


def contact_sheet(hits: list[dict], hits_dir: Path, out_path: Path, cols: int = 4, cell_w: int = 420) -> Path:
    items = [h for h in hits if (hits_dir / h["file"]).exists()]
    if not items:
        return out_path
    rows = min(6, (len(items) + cols - 1) // cols)
    picks = items[: cols * rows]
    font = _font(15)
    cells = []
    for h in picks:
        try:
            with Image.open(hits_dir / h["file"]) as im:
                crop = _crop_box(im.convert("RGB"), h).copy()
        except Exception:  # noqa: BLE001
            continue
        ratio = cell_w / max(1, crop.width)
        crop = crop.resize((cell_w, max(1, int(crop.height * ratio))), Image.LANCZOS)
        cells.append((crop, h))
    if not cells:
        return out_path
    cell_h = max(c.height for c, _ in cells) + 40
    sheet = Image.new("RGB", (cols * cell_w, rows * cell_h), (18, 18, 18))
    draw = ImageDraw.Draw(sheet)
    for i, (crop, h) in enumerate(cells):
        cx = (i % cols) * cell_w
        cy = (i // cols) * cell_h
        sheet.paste(crop, (cx, cy))
        label = f'{h["bucket"] or "?"} {_mmss(h["t"])} h={h["text_height"]} c={h["conf"]:.2f} {h["side"]}'
        draw.text((cx + 4, cy + cell_h - 34), label, fill=(255, 235, 120), font=font)
        draw.text((cx + 4, cy + cell_h - 17), (h["text"] or "")[:34], fill=(200, 200, 200), font=font)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out_path, quality=88)
    return out_path


def _mmss(t: float) -> str:
    t = max(0, int(round(t)))
    return f"{t // 60:02d}:{t % 60:02d}"


def build(label: str) -> Path:
    dirs = episode_dirs(label)
    out = dirs["out"]
    stats_path = out / "stats.json"
    stats = json.loads(stats_path.read_text(encoding="utf-8")) if stats_path.exists() else {}
    hits = _load_jsonl(out / "hits.jsonl")
    texts = _load_jsonl(out / "texts.jsonl")
    unmatched = _load_jsonl(out / "unmatched.jsonl")

    sheet = contact_sheet(hits, dirs["hits"], out / "contact-sheet.jpg")

    height_bins = _bucketize([h["text_height"] for h in hits], [("h<20", 0, 20), ("20-29", 20, 30), ("30-39", 30, 40), ("40-59", 40, 60), ("60+", 60, 10**6)])
    conf_bins = _bucketize([h["conf"] for h in hits], [("c<0.6", 0, 0.6), ("0.6-0.8", 0.6, 0.8), ("0.8-0.9", 0.8, 0.9), ("0.9+", 0.9, 1.01)])
    motion_bins = _bucketize([h["bg_motion"] for h in hits], [("m<0.5 静止", 0, 0.5), ("0.5-2", 0.5, 2.0), ("2-5", 2.0, 5.0), ("5+ 剧烈", 5.0, 10**6)])
    kind_counts: dict[str, int] = {}
    for h in hits:
        kind_counts[h["kind"] or "?"] = kind_counts.get(h["kind"] or "?", 0) + 1
    side_counts: dict[str, int] = {}
    for h in hits:
        side_counts[h["side"]] = side_counts.get(h["side"], 0) + 1

    per_bucket = sorted(((k, v) for k, v in (stats.get("per_bucket") or {}).items()), key=lambda kv: -kv[1])

    duration = float(stats.get("duration") or 0)
    total = float((stats.get("timings") or {}).get("total") or 0)
    est_all = total / duration * (84 * 2605) / 3600 if duration else 0.0

    lines: list[str] = []
    add = lines.append
    add(f"# ep 素材采集报告：{label}\n")
    add(f"- 片源：{stats.get('page_url', '')}")
    add(f"- 标题：{stats.get('title', '')}　guid：`{stats.get('guid', '')}`")
    add(f"- 时长：{duration:.0f}s　分辨率：{stats.get('resolution', '?')}　帧率：{stats.get('fps', '?')}")
    add(f"- 分片：{stats.get('segments')}　解码帧：{stats.get('decoded_frames')}　采样帧：{stats.get('sampled_frames')}（2fps）")
    add(f"- 含文字帧：{stats.get('text_frames')}　OCR 调用：{stats.get('ocr_calls')}（复用 {stats.get('ocr_reused')}）")
    add(f"- 命中：原始 {stats.get('hits_raw')} → 去重后 **{stats.get('hits_kept')}**　未匹配文本：{stats.get('unmatched')}")
    add(f"- 耗时：{total:.1f}s（resolve {stats.get('timings', {}).get('resolve')}s / 解码+OCR {stats.get('timings', {}).get('decode_ocr')}s / 去重 {stats.get('timings', {}).get('dedup')}s）")
    if est_all:
        add(f"- **全季外推**（84 集 × 43.4min，串行）：≈ {est_all:.1f} 小时（3 路并行约 {est_all / 2.5:.1f} 小时）")
    add("")
    add("## 样本拼版")
    add(f"![contact sheet]({sheet.name})")
    add("")
    add("## 特征分布（用于定片头/片尾过滤规则）")
    add("")
    add("| 维度 | 分档 | 帧数 |")
    add("|---|---|---|")
    for name, n in height_bins:
        add(f"| 字号(bbox 高) | {name} | {n} |")
    for name, n in conf_bins:
        add(f"| OCR 置信度 | {name} | {n} |")
    for name, n in motion_bins:
        add(f"| 背景运动 | {name} | {n} |")
    for k, v in side_counts.items():
        add(f"| 位置 | {k}（左/右） | {v} |")
    for k, v in kind_counts.items():
        add(f"| 匹配方式 | {k} | {v} |")
    add("")
    add("## 按桶命中排行 Top 20")
    add("")
    add("| # | 武将桶 | 命中帧 |")
    add("|---|---|---|")
    for i, (k, v) in enumerate(per_bucket[:20], 1):
        add(f"| {i} | {k} | {v} |")
    add("")
    top_texts = {}
    for t in texts:
        top_texts[t["text"]] = top_texts.get(t["text"], 0) + 1
    add("## OCR 文本 Top 25（含未匹配）")
    add("")
    add("| 文本 | 次数 |")
    add("|---|---|")
    for k, v in sorted(top_texts.items(), key=lambda kv: -kv[1])[:25]:
        add(f"| {k} | {v} |")
    add("")
    add(f"## 未匹配文本（去重去停用，共 {len(unmatched)} 条，供扩别名）")
    add("")
    for u in unmatched[:40]:
        add(f"- {_mmss(u['t'])} {u['side']} conf={u['conf']:.2f} `{u['text']}`")
    add("")
    path = out / "report.md"
    path.write_text("\n".join(lines), encoding="utf-8")
    return path
