"""单集流水线：取流 → 逐分片解码 → 2fps 采样 → 左右侧栏文字检测 → OCR → 词典匹配 → 去重 → 落盘。

性能取向：
- 边下边解：分片下载到 ``src/`` 后立刻解码，处理完的采样帧不落盘
- 只有检测到文字行才 OCR；字幕不变时复用上次 OCR
- 命中帧先存 ``hits/<ep>/tmp``，统一去重后再改名
"""

from __future__ import annotations

import json
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path

import cv2
import numpy as np

from . import cctv, vision
from .dicts import Dictionary, Match, normalize
from .paths import DEDUP_HAMMING, DEDUP_WINDOW_SEC, SRC, episode_dirs


@dataclass
class Hit:
    t: float  # 全片时间（秒）
    side: str
    text: str
    conf: float
    bucket: str | None
    kind: str | None
    token: str | None
    box: tuple[int, int, int, int]  # 该侧 ROI 内的 bbox
    text_height: int
    bg_motion: float
    sharp: float
    dhash: int
    file: str = ""
    seg: int = -1
    names: list[str] = field(default_factory=list)
    orientation: str = "h"


@dataclass
class EpisodeStats:
    label: str
    page_url: str
    guid: str = ""
    title: str = ""
    duration: float = 0.0
    resolution: str = ""
    fps: float = 0.0
    segments: int = 0
    decoded_frames: int = 0
    sampled_frames: int = 0
    text_frames: int = 0
    ocr_calls: int = 0
    ocr_reused: int = 0
    hits_raw: int = 0
    hits_kept: int = 0
    unmatched: int = 0
    per_bucket: dict[str, int] = field(default_factory=dict)
    timings: dict[str, float] = field(default_factory=dict)

    def as_dict(self) -> dict:
        return {k: v for k, v in asdict(self).items()}


def _fmt_ts(t: float) -> str:
    t = max(0, int(round(t)))
    return f"{t // 60:02d}{t % 60:02d}"


def process_segment(
    seg_path: Path,
    seg_index: int,
    seg_start: float,
    state: dict,
    ctx: dict,
) -> float:
    """解码单个分片，返回该分片结束后的全局时间。"""
    step = ctx["step"]
    dict_ = ctx["dict"]
    ocr = ctx["ocr"]
    hits: list[Hit] = ctx["hits"]
    texts_log: list[dict] = ctx["texts"]
    stats: EpisodeStats = ctx["stats"]

    container = None
    last_local = 0.0
    first_local = None
    gt = seg_start
    frame_dur = 0.04
    try:
        container = __import__("av").open(str(seg_path))
        stream = container.streams.video[0]
        stream.thread_type = "AUTO"
        try:
            stream.codec_context.thread_count = 0
        except Exception:  # noqa: BLE001
            pass
        tb = float(stream.time_base) if stream.time_base else 0.04
        if not stats.resolution:
            stats.resolution = f"{stream.codec_context.width}x{stream.codec_context.height}"
            stats.fps = float(stream.average_rate) if stream.average_rate else 0.0
        for frame in container.decode(stream):
            stats.decoded_frames += 1
            t_local = float(frame.pts * tb) if frame.pts is not None else last_local + frame_dur
            if first_local is None:
                first_local = t_local
                # CCTV 的 .ts 分片 PTS 是整片连续（绝对值）；判一次即可，避免时间戳累计错位
                if seg_index > 0 and state.get("mode") is None:
                    state["mode"] = "abs" if t_local >= state.get("gt_end", 0.0) - 2.0 else "rel"
            if t_local > last_local:
                frame_dur = min(0.2, t_local - last_local)
            last_local = t_local
            gt = t_local if state.get("mode") == "abs" else state.get("offset", 0.0) + t_local
            if gt + 1e-6 < state["next_sample"]:
                continue
            state["next_sample"] = gt + step

            rgb = frame.to_ndarray(format="rgb24")
            h, w = rgb.shape[:2]
            if ctx["mode"] == "bottom":
                spans = {"B": vision.bottom_span(w, h)}
            else:
                spans = vision.side_spans(w, h)
            stats.sampled_frames += 1
            any_text = False
            for side, (x0, y0, x1, y1) in spans.items():
                roi = np.ascontiguousarray(rgb[y0:y1, x0:x1])
                gray = cv2.cvtColor(roi, cv2.COLOR_RGB2GRAY)
                vision.mask_overlay_in_roi(gray, w, h, (x0, y0, x1, y1))
                rh, rw = gray.shape[:2]
                if ctx["mode"] == "bottom":
                    line = vision.detect_subtitle_line(gray)
                    cands = [{"img": gray, "box": line, "src": line, "ori": "h"}] if line else []
                else:
                    # 侧栏：交给 OCR 的 det（PP-OCR 的 det 会自己处理竖排框），先过廉价 gate
                    cands = []
                    if vision.roi_text_gate(gray):
                        # CLAHE 提对比：老片侧栏人名字幕偏灰，原图 det 会漏（实测 何进 需此步）
                        enh = cv2.createCLAHE(clipLimit=2.5, tileGridSize=(8, 8)).apply(gray)
                        items = ocr.recognize_boxes(cv2.cvtColor(enh, cv2.COLOR_GRAY2BGR))
                        stats.ocr_calls += 1
                        for it in items:
                            b = it["box"]
                            if b[2] < 10 or b[3] < 10:
                                continue
                            cands.append(
                                {
                                    "img": gray,
                                    "box": b,
                                    "src": b,
                                    "ori": "v" if b[3] > b[2] * 1.3 else "h",
                                    "text": it["text"],
                                    "conf": it["score"],
                                }
                            )
                small = cv2.resize(gray, (64, 48), interpolation=cv2.INTER_AREA).astype(np.int16)
                prev = state["prev_small"][side]
                motion = float(np.abs(small - prev).mean()) if prev is not None else 0.0
                state["prev_small"][side] = small
                if not cands:
                    state["last_sig"][side] = None
                    continue
                any_text = True
                for cand in cands[:3]:
                    cimg = cand["img"]
                    ch, cw = cimg.shape[:2]
                    bx = vision.union_box([cand["box"]], pad=6, w=cw, h=ch)
                    crop = cimg[bx[1] : bx[3], bx[0] : bx[2]]
                    if crop.size == 0:
                        continue
                    max_box_h = int(ch * 0.95) if ctx["mode"] == "bottom" else int(ch * 0.5)
                    if (bx[3] - bx[1]) > max_box_h:
                        continue
                    ori = cand["ori"]
                    skey = f"{side}{ori}"
                    sig = vision.mask_signature(crop)
                    if "text" in cand:
                        text, conf = cand["text"], cand["conf"]
                    elif state["last_sig"].get(skey) == sig and state["last_text"].get(skey):
                        text, conf = state["last_text"][skey]
                        stats.ocr_reused += 1
                    else:
                        scale = 1.0
                        if crop.shape[0] < 64:
                            scale = min(2.5, 64.0 / max(1, crop.shape[0]))
                        big = cv2.resize(crop, None, fx=scale, fy=scale, interpolation=cv2.INTER_CUBIC) if scale > 1.0 else crop
                        text, conf = ocr.recognize(
                            cv2.cvtColor(big, cv2.COLOR_GRAY2BGR),
                            line_mode=ctx["mode"] != "bottom" and ori == "h",
                        )
                        state["last_sig"][skey] = sig
                        state["last_text"][skey] = (text, conf)
                        stats.ocr_calls += 1
                    if not text:
                        continue
                    texts_log.append(
                        {
                            "t": round(gt, 2),
                            "side": side,
                            "ori": ori,
                            "text": text,
                            "conf": round(conf, 4),
                            "h": bx[3] - bx[1],
                        }
                    )
                    if len(normalize(text)) < 2:  # 纯英文/数字（如 CCTV8 台标）直接丢
                        continue
                    if conf < ctx["min_conf"]:
                        continue
                    if ctx["mode"] == "bottom":
                        found = dict_.find_all(text)
                        if not found:
                            stats.unmatched += 1
                            continue
                        names = [b for b, _ in found]
                        bucket, token, kind = found[0][0], found[0][1], "subtitle"
                    else:
                        m: Match | None = dict_.match(text)
                        if m is None:
                            stats.unmatched += 1
                            continue
                        names = [m.bucket]
                        bucket, token, kind = m.bucket, m.token, m.kind
                    sb = cand["src"]
                    hit = Hit(
                        t=round(gt, 2),
                        side=side,
                        text=text,
                        conf=round(conf, 4),
                        bucket=bucket,
                        kind=kind,
                        token=token,
                        box=(x0 + sb[0], y0 + sb[1], sb[2], sb[3]),
                        text_height=sb[3],
                        bg_motion=round(motion, 3),
                        sharp=round(vision.sharpness(crop), 2),
                        dhash=vision.dhash(crop),
                        seg=seg_index,
                        names=names,
                        orientation=ori,
                    )
                    stats.hits_raw += 1
                    for b in names:
                        stats.per_bucket[b] = stats.per_bucket.get(b, 0) + 1
                    hits.append(hit)
                    ctx["_pending"].append((hit, rgb.copy()))
            if any_text:
                stats.text_frames += 1
            # 及时把待存帧刷到磁盘，控制内存
            if len(ctx["_pending"]) >= 6:
                _flush_pending(ctx)
    finally:
        if container is not None:
            try:
                container.close()
            except Exception:  # noqa: BLE001
                pass
    end_gt = max(gt, seg_start) + frame_dur
    state["gt_end"] = end_gt
    if state.get("mode") != "abs":
        state["offset"] = end_gt
    return end_gt


def _flush_pending(ctx: dict) -> None:
    pending = ctx["_pending"]
    if not pending:
        return
    tmp_dir: Path = ctx["hits_tmp"]
    for hit, rgb in pending:
        name = f"{hit.seg:04d}_{int(hit.t * 100):07d}_{hit.side}.png"
        path = tmp_dir / name
        bgr = cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
        ok, buf = cv2.imencode(".png", bgr)
        if ok:
            # 文件名含中文，cv2.imwrite 在 Windows 上会失败；用 tofile 走宽字符路径
            buf.tofile(str(path))
            hit.file = name
    pending.clear()


def dedup_and_finalize(hits: list[Hit], hits_tmp: Path, hits_dir: Path) -> tuple[list[Hit], int]:
    """同镜头去重：同 bucket/side、时间窗内、dHash 相近 -> 只留最清晰一帧。"""
    hits = [h for h in hits if h.file]
    hits.sort(key=lambda x: x.t)
    keep: list[Hit] = []
    last_idx: dict[tuple[str, str], int] = {}
    dropped = 0
    for h in hits:
        key = (h.bucket or normalize(h.text), h.side, h.orientation)
        j = last_idx.get(key)
        if j is not None:
            prev = keep[j]
            same_text = normalize(h.text) == normalize(prev.text)
            if h.t - prev.t <= DEDUP_WINDOW_SEC and (
                same_text or vision.hamming(h.dhash, prev.dhash) <= DEDUP_HAMMING
            ):
                dropped += 1
                if h.sharp > prev.sharp:  # 换上更清晰的一帧
                    keep[j] = h
                continue
        last_idx[key] = len(keep)
        keep.append(h)
    # 落最终文件
    kept_files = {h.file for h in keep}
    for h in keep:
        src = hits_tmp / h.file
        final = f"{h.bucket or 'unknown'}_{_fmt_ts(h.t)}_{h.side}_{Path(h.file).stem.split('_')[0]}.png"
        dst = hits_dir / final
        if src.exists():
            if dst.exists():
                dst.unlink()
            src.replace(dst)
            h.file = final
    for h in hits:
        if h.file not in kept_files:
            p = hits_tmp / h.file
            if p.exists():
                p.unlink()
    return keep, dropped


def run_episode(
    page_url: str,
    label: str,
    dictionary: Dictionary,
    fps: float = 2.0,
    prefer_br: int = 2000,
    max_segments: int | None = None,
    mode: str = "side",
    min_conf: float = 0.5,
    log=print,
) -> EpisodeStats:
    dirs = episode_dirs(label)
    stats = EpisodeStats(label=label, page_url=page_url)

    t0 = time.perf_counter()
    resolved = cctv.resolve(page_url, prefer=prefer_br)
    stats.guid = resolved["guid"]
    stats.title = resolved["title"]
    stats.duration = resolved["total"]
    segments = resolved["segments"]
    if max_segments:
        segments = segments[:max_segments]
    stats.segments = len(segments)
    stats.timings["resolve"] = round(time.perf_counter() - t0, 2)
    log(f"[{label}] guid={stats.guid} title={stats.title!r} 时长={stats.duration:.0f}s 分片={len(segments)}")

    ocr = vision.OcrEngine()
    hits_tmp = dirs["hits"] / "tmp"
    hits_tmp.mkdir(parents=True, exist_ok=True)
    ctx = {
        "step": 1.0 / fps,
        "dict": dictionary,
        "ocr": ocr,
        "hits": [],
        "texts": [],
        "stats": stats,
        "_pending": [],
        "hits_tmp": hits_tmp,
        "mode": mode,
        "min_conf": min_conf,
    }
    state = {
        "next_sample": 0.0,
        "offset": 0.0,
        "gt_end": 0.0,
        "mode": None,
        "prev_small": {"L": None, "R": None, "B": None},
        "last_sig": {"L": None, "R": None, "B": None},
        "last_text": {"L": ("", 0.0), "R": ("", 0.0), "B": ("", 0.0)},
    }

    seg_start = 0.0
    t_dec = time.perf_counter()
    for seg in segments:
        seg_path = SRC / stats.guid / f"{seg.index:04d}.ts"
        cctv.download_segment(seg.url, seg_path)
        seg_start = process_segment(seg_path, seg.index, seg_start, state, ctx)
        if seg.index % 10 == 0:
            log(
                f"[{label}] seg {seg.index + 1}/{len(segments)} t={seg_start:.0f}s "
                f"样本={stats.sampled_frames} OCR={stats.ocr_calls} 命中={stats.hits_raw}"
            )
        try:
            seg_path.unlink()  # 边处理边删，控制磁盘
        except OSError:
            pass
    _flush_pending(ctx)
    stats.timings["decode_ocr"] = round(time.perf_counter() - t_dec, 2)

    t_dd = time.perf_counter()
    keep, dropped = dedup_and_finalize(ctx["hits"], hits_tmp, dirs["hits"])
    stats.timings["dedup"] = round(time.perf_counter() - t_dd, 2)
    stats.hits_kept = len(keep)
    log(f"[{label}] 去重：{len(ctx['hits'])} -> {len(keep)}（丢弃 {dropped}）")

    t_w = time.perf_counter()
    with (dirs["out"] / "hits.jsonl").open("w", encoding="utf-8") as f:
        for h in keep:
            f.write(json.dumps(asdict(h), ensure_ascii=False) + "\n")
    with (dirs["out"] / "texts.jsonl").open("w", encoding="utf-8") as f:
        for t in ctx["texts"]:
            f.write(json.dumps(t, ensure_ascii=False) + "\n")
    with (dirs["out"] / "unmatched.jsonl").open("w", encoding="utf-8") as f:
        seen: dict[str, int] = {}
        for t in ctx["texts"]:
            if dictionary.match(t["text"]) is None:
                norm = normalize(t["text"])
                if norm and norm not in seen:
                    seen[norm] = 1
                    f.write(json.dumps(t, ensure_ascii=False) + "\n")
    (dirs["out"] / "stats.json").write_text(
        json.dumps(stats.as_dict(), ensure_ascii=False, indent=2), encoding="utf-8"
    )
    stats.timings["write"] = round(time.perf_counter() - t_w, 2)
    stats.timings["total"] = round(time.perf_counter() - t0, 2)
    (dirs["out"] / "stats.json").write_text(
        json.dumps(stats.as_dict(), ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return stats
