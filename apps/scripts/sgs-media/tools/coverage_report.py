"""全季覆盖率报表 + 跨集拼版（独立脚本，不被 sgs_media 主流程导入）。

用法（仓库根）：
    PY=data/media/sgs/venv/Scripts/python.exe
    $PY apps/scripts/sgs-media/tools/coverage_report.py --from 1 --to 84

产出：
    data/media/sgs/out/coverage.md      覆盖率报表（按包分组 + 每集命中数）
    data/media/sgs/out/coverage.json    机器可读
    data/media/sgs/out/contact-all.jpg  跨集命中拼版
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parents[4]
MEDIA = REPO / "data" / "media" / "sgs"
OUT = MEDIA / "out"
HITS = MEDIA / "hits"
HEROES = REPO / "docs" / "games" / "sanguosha" / "data" / "heroes.json"

PREFIXES = ("SP", "sp", "Sp", "sP", "界", "谋", "神", "魔", "族")
FONT_CANDIDATES = ["C:/Windows/Fonts/simhei.ttf", "C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simsun.ttc"]


def strip_prefix(name: str) -> str:
    out = name.strip()
    changed = True
    while changed:
        changed = False
        for p in PREFIXES:
            if out.startswith(p) and len(out) > len(p) + 1:
                out = out[len(p) :]
                changed = True
    return out


def font(size: int):
    for p in FONT_CANDIDATES:
        if Path(p).exists():
            try:
                return ImageFont.truetype(p, size)
            except Exception:  # noqa: BLE001
                pass
    return ImageFont.load_default()


def load_hits(ep_from: int, ep_to: int) -> list[dict]:
    rows: list[dict] = []
    for n in range(ep_from, ep_to + 1):
        label = f"ep{n:02d}"
        f = OUT / label / "hits.jsonl"
        if not f.exists():
            continue
        for line in f.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                h = json.loads(line)
            except json.JSONDecodeError:
                continue
            h["ep"] = n
            h["label"] = label
            rows.append(h)
    return rows


def bucket_stats(heroes: list[dict]) -> tuple[dict[str, list[str]], dict[str, str]]:
    """桶 -> 武将名列表；桶 -> 包"""
    buckets: dict[str, list[str]] = {}
    pack_of: dict[str, str] = {}
    for h in heroes:
        b = strip_prefix(h["name"])
        buckets.setdefault(b, []).append(h["name"])
        pack_of.setdefault(b, h.get("pack") or "?")
    return buckets, pack_of


def _mmss(t: float) -> str:
    t = max(0, int(round(t)))
    return f"{t // 60:02d}:{t % 60:02d}"


def crop_of(img: Image.Image, box: list[int], pad: int = 6) -> Image.Image:
    x, y, w, h = box
    return img.crop((max(0, x - pad), max(0, y - pad), min(img.width, x + w + pad), min(img.height, y + h + pad)))


def contact_sheet(rows: list[dict], path: Path, cols: int = 6, cell: int = 300, max_items: int = 60) -> None:
    picks = sorted(rows, key=lambda r: (r["bucket"] or "", r["ep"]))[:max_items]
    cells = []
    f = font(13)
    for r in picks:
        p = HITS / r["label"] / r.get("file", "")
        if not p.exists():
            continue
        try:
            with Image.open(p) as im:
                img = im.convert("RGB")
                c = crop_of(img, r.get("box") or [0, 0, img.width, img.height])
        except Exception:  # noqa: BLE001
            continue
        ratio = cell / max(1, c.width)
        c = c.resize((cell, max(1, int(c.height * ratio))), Image.LANCZOS)
        cells.append((c, r))
    if not cells:
        return
    row_h = max(c.height for c, _ in cells) + 34
    rows_n = (len(cells) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * cell, rows_n * row_h), (16, 16, 16))
    d = ImageDraw.Draw(sheet)
    for i, (c, r) in enumerate(cells):
        x = (i % cols) * cell
        y = (i // cols) * row_h
        sheet.paste(c, (x, y))
        d.text((x + 3, y + row_h - 30), f'{r["bucket"]}  ep{r["ep"]:02d} {_mmss(r["t"])}', fill=(255, 230, 120), font=f)
        d.text((x + 3, y + row_h - 15), (r.get("text") or "")[:26], fill=(190, 190, 190), font=f)
    path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(path, quality=88)
    print(f"[sheet] {path} ({len(cells)} 张)")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from", dest="ep_from", type=int, default=1)
    ap.add_argument("--to", dest="ep_to", type=int, default=84)
    ap.add_argument("--sheet", type=int, default=60, help="拼版最多取多少张")
    args = ap.parse_args()

    heroes = json.loads(HEROES.read_text(encoding="utf-8"))["heroes"]
    buckets, pack_of = bucket_stats(heroes)
    rows = load_hits(args.ep_from, args.ep_to)

    per_bucket: dict[str, list[dict]] = {}
    for r in rows:
        for name in r.get("names") or [r.get("bucket")]:
            if not name:
                continue
            per_bucket.setdefault(name, []).append(r)

    covered = {b: v for b, v in per_bucket.items() if b in buckets}
    unknown = {b: v for b, v in per_bucket.items() if b not in buckets}
    missing = sorted(b for b in buckets if b not in covered)

    by_pack: dict[str, dict[str, int]] = {}
    for b in buckets:
        p = pack_of.get(b, "?")
        d = by_pack.setdefault(p, {"total": 0, "covered": 0})
        d["total"] += 1
        if b in covered:
            d["covered"] += 1

    per_ep: dict[int, int] = {}
    for r in rows:
        per_ep[r["ep"]] = per_ep.get(r["ep"], 0) + 1

    ep_done = sorted(int(p.name[2:]) for p in OUT.glob("ep[0-9][0-9]") if (p / "stats.json").exists())

    contact_sheet(rows, OUT / "contact-all.jpg", max_items=args.sheet)

    lines: list[str] = []
    a = lines.append
    a(f"# 武将素材覆盖率（ep{args.ep_from}–ep{args.ep_to}）\n")
    a(f"- 已完成集：**{len(ep_done)}**（{ep_done[0] if ep_done else '-'}–{ep_done[-1] if ep_done else '-'}）")
    a(f"- 命中帧：**{len(rows)}**　覆盖武将桶：**{len(covered)} / {len(buckets)}**（{len(covered)/max(1,len(buckets))*100:.1f}%）")
    a(f"- 未在词典中的文本命中：{len(unknown)} 个（多为片头/片尾的编剧、作曲、演员名）")
    a("")
    a("## 按包覆盖")
    a("")
    a("| 包 | 桶数 | 已覆盖 | 覆盖率 |")
    a("|---|---|---|---|")
    for p, d in sorted(by_pack.items(), key=lambda kv: -kv[1]["total"]):
        a(f"| {p} | {d['total']} | {d['covered']} | {d['covered']/max(1,d['total'])*100:.0f}% |")
    a("")
    a("## 命中数 Top 40")
    a("")
    a("| 武将桶 | 帧数 | 集 |")
    a("|---|---|---|")
    for b, v in sorted(covered.items(), key=lambda kv: -len(kv[1]))[:40]:
        eps = sorted({r["ep"] for r in v})
        a(f"| {b} | {len(v)} | {','.join(str(e) for e in eps[:8])}{'…' if len(eps) > 8 else ''} |")
    a("")
    a("## 每集命中数")
    a("")
    a("| 集 | 命中 |")
    a("|---|---|")
    for n in ep_done:
        a(f"| ep{n:02d} | {per_ep.get(n, 0)} |")
    a("")
    a(f"## 未覆盖的桶（{len(missing)} 个，按包）")
    a("")
    cur = None
    for b in sorted(missing, key=lambda x: (pack_of.get(x, "?"), x)):
        p = pack_of.get(b, "?")
        if p != cur:
            cur = p
            a(f"\n**{p}**：")
        a(f"- {b}")
    a("")
    (OUT / "coverage.md").write_text("\n".join(lines), encoding="utf-8")
    (OUT / "coverage.json").write_text(
        json.dumps(
            {
                "episodes_done": ep_done,
                "hits": len(rows),
                "covered": {k: len(v) for k, v in covered.items()},
                "missing": missing,
                "unknown_texts": {k: len(v) for k, v in unknown.items()},
                "per_episode": per_ep,
            },
            ensure_ascii=False,
            indent=1,
        ),
        encoding="utf-8",
    )
    print(f"[report] {OUT/'coverage.md'}")
    print(f"覆盖 {len(covered)}/{len(buckets)} 桶，命中 {len(rows)} 帧，已完成 {len(ep_done)} 集")
    return 0


if __name__ == "__main__":
    sys.exit(main())
