"""命令行入口：``python -m sgs_media <subcommand>``。"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import cv2

from . import cctv, dicts, pipeline, report
from .paths import RAPIDOCR_DIR, ROOT, SRC, ensure_dirs

EP1_PAGE = "https://tv.cctv.com/2010/05/07/VIDE1355918475742338.shtml"


def cmd_dict(args: argparse.Namespace) -> int:
    d = dicts.load_dictionary()
    print(dicts.summary(d))
    multi = [(k, v) for k, v in d.buckets.items() if len(v) > 1]
    multi.sort(key=lambda kv: -len(kv[1]))
    print("\n多版本合并示例（Top 10）：")
    for k, v in multi[:10]:
        print(f"  {k} <- gid {v}")
    print(f"\n词条样例：{list(d.tokens.items())[:12]}")
    return 0


def cmd_probe(args: argparse.Namespace) -> int:
    t0 = time.perf_counter()
    resolved = cctv.resolve(args.page, prefer=args.prefer_br)
    print(f"title   : {resolved['title']}")
    print(f"guid    : {resolved['guid']}")
    print(f"duration: {resolved['total']:.0f}s")
    print(f"m3u8    : {resolved['m3u8']}")
    print(f"segments: {len(resolved['segments'])}（首片 {resolved['segments'][0].duration:.1f}s）")
    seg = resolved["segments"][0]
    path = cctv.download_segment(seg.url, ROOT / "src" / resolved["guid"] / "probe_0000.ts")
    print(f"seg0    : {path} {path.stat().st_size / 1e6:.2f} MB")
    import av

    container = av.open(str(path))
    stream = container.streams.video[0]
    width = stream.codec_context.width
    height = stream.codec_context.height
    fps_v = float(stream.average_rate) if stream.average_rate else 0.0
    pix_fmt = stream.codec_context.pix_fmt
    codec = stream.codec_context.name
    n = 0
    for _frame in container.decode(stream):
        n += 1
        if n >= 60:
            break
    container.close()
    print(f"video   : {width}x{height} fps={fps_v:.3f} pix_fmt={pix_fmt} codec={codec}")
    print(f"decoded : {n} 帧 in {time.perf_counter() - t0:.1f}s（含下载）")
    return 0


def cmd_episode(args: argparse.Namespace) -> int:
    ensure_dirs()
    d = dicts.load_dictionary()
    print(f"[dict] {dicts.summary(d)}")
    print(f"[model] rapidocr -> {RAPIDOCR_DIR}")
    stats = pipeline.run_episode(
        page_url=args.page,
        label=args.label,
        dictionary=d,
        fps=args.fps,
        prefer_br=args.prefer_br,
        max_segments=args.max_segments,
        mode=args.mode,
        min_conf=args.min_conf,
    )
    print(json.dumps(stats.as_dict(), ensure_ascii=False, indent=2))
    if not args.no_report:
        p = report.build(args.label)
        print(f"[report] {p}")
    return 0


def cmd_report(args: argparse.Namespace) -> int:
    p = report.build(args.label)
    print(p)
    return 0


def cmd_batch(args: argparse.Namespace) -> int:
    from . import batch

    s = batch.run_batch(args.ep_from, args.ep_to, args.workers, args.fps, args.force)
    print(json.dumps({k: v for k, v in s.items() if k != "per_episode"}, ensure_ascii=False, indent=2))
    return 0 if not s["failed"] else 1


def cmd_frames(args: argparse.Namespace) -> int:
    """导出指定时间点的整帧 PNG（调 ROI / 台标遮罩用）。"""
    import av

    ensure_dirs()
    resolved = cctv.resolve(args.page, prefer=args.prefer_br)
    targets = sorted({float(x) for x in args.at.split(",") if x.strip()})
    starts: list[float] = []
    acc = 0.0
    for seg in resolved["segments"]:
        starts.append(acc)
        acc += seg.duration
    out_dir = Path(args.out) if args.out else ROOT / "out" / "frames"
    out_dir.mkdir(parents=True, exist_ok=True)
    saved = []
    for t in targets:
        idx = 0
        for i, s in enumerate(starts):
            if s <= t:
                idx = i
            else:
                break
        seg = resolved["segments"][idx]
        path = cctv.download_segment(seg.url, SRC / resolved["guid"] / f"dump_{idx:04d}.ts")
        container = av.open(str(path))
        stream = container.streams.video[0]
        tb = float(stream.time_base) if stream.time_base else 0.04
        best = None
        best_d = 1e9
        for frame in container.decode(stream):
            lt = float(frame.pts * tb) if frame.pts is not None else 0.0
            # CCTV 的 .ts 分片 PTS 是整片绝对值；若 PTS 不足本段起点则视为相对时间
            abs_t = lt if lt >= starts[idx] - 2.0 else starts[idx] + lt
            d = abs(abs_t - t)
            if d < best_d:
                best_d = d
                best = frame.to_ndarray(format="rgb24")
            if abs_t > t + 1.0:
                break
        container.close()
        if best is not None:
            dst = out_dir / f"t{int(t):05d}.png"
            ok, buf = cv2.imencode(".png", cv2.cvtColor(best, cv2.COLOR_RGB2BGR))
            if ok:
                buf.tofile(str(dst))
                saved.append(str(dst))
    for s in saved:
        print(s)
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="sgs_media", description="三国演义人物介绍字幕素材采集（CCTV 源）")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("dict", help="打印武将桶/词典统计")
    p.set_defaults(func=cmd_dict)

    p = sub.add_parser("probe", help="探测片源（guid/时长/分辨率/码率变体）")
    p.add_argument("--page", default=EP1_PAGE)
    p.add_argument("--prefer-br", type=int, default=2000)
    p.set_defaults(func=cmd_probe)

    p = sub.add_parser("episode", help="跑一集：抽帧 + OCR + 匹配 + 去重 + 报告")
    p.add_argument("--page", default=EP1_PAGE)
    p.add_argument("--label", default="ep01")
    p.add_argument("--fps", type=float, default=2.0)
    p.add_argument("--prefer-br", type=int, default=2000)
    p.add_argument("--max-segments", type=int, default=None, help="只跑前 N 个分片（调试用）")
    p.add_argument("--mode", choices=("side", "bottom"), default="bottom", help="side=左右侧栏（人物介绍）；bottom=底部台词字幕")
    p.add_argument("--min-conf", type=float, default=0.5)
    p.add_argument("--no-report", action="store_true")
    p.set_defaults(func=cmd_episode)

    p = sub.add_parser("report", help="对已有结果重建报告")
    p.add_argument("--label", default="ep01")
    p.set_defaults(func=cmd_report)

    p = sub.add_parser("batch", help="批量跑多集（N 路并行 + 断点续跑）")
    p.add_argument("--from", dest="ep_from", type=int, default=1)
    p.add_argument("--to", dest="ep_to", type=int, default=84)
    p.add_argument("--workers", type=int, default=6)
    p.add_argument("--fps", type=float, default=1.0)
    p.add_argument("--force", action="store_true", help="忽略已完成标记，重跑")
    p.set_defaults(func=cmd_batch)

    p = sub.add_parser("frames", help="导出指定时间点整帧（调 ROI / 台标遮罩）")
    p.add_argument("--page", default=EP1_PAGE)
    p.add_argument("--at", default="100,800,1600,2200")
    p.add_argument("--out", default=None)
    p.add_argument("--prefer-br", type=int, default=2000)
    p.set_defaults(func=cmd_frames)

    args = ap.parse_args(argv)
    return int(args.func(args))


if __name__ == "__main__":
    sys.exit(main())
