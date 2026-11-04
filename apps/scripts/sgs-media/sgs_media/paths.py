"""路径与运行参数集中定义。

约定（用户裁定）：
- 模型权重只放 C 盘：默认 ``C:/llm/models/rapidocr``（可用 SGS_RAPIDOCR_DIR 覆盖）
- 产出物与中间产物只放 D 盘仓库内：``<repo>/data/media/sgs``（``/data/`` 已被 .gitignore 全量忽略）
"""

from __future__ import annotations

import os
from pathlib import Path

# apps/scripts/sgs-media/sgs_media/paths.py -> parents[4] == 仓库根
REPO_ROOT = Path(__file__).resolve().parents[4]

ROOT = Path(os.environ.get("SGS_MEDIA_ROOT", REPO_ROOT / "data" / "media" / "sgs"))

SRC = ROOT / "src"  # HLS 分片缓存（处理完可删）
FRAMES = ROOT / "frames"  # 采样帧临时目录（默认不落盘，保留给调试）
HITS = ROOT / "hits"  # 命中帧 PNG（全量保留）
OUT = ROOT / "out"  # hits.jsonl / unmatched.jsonl / contact sheet / report.md
STATE = ROOT / "state"  # 断点续跑状态

RAPIDOCR_DIR = Path(os.environ.get("SGS_RAPIDOCR_DIR", "C:/llm/models/rapidocr"))
HEROES_JSON = REPO_ROOT / "docs" / "games" / "sanguosha" / "data" / "heroes.json"
ALIASES_JSON = Path(__file__).resolve().parent / "aliases.json"

# 老版《三国演义》人物介绍字幕固定在左/右半区（用户确认，非底部）
ROI_LEFT = (0.00, 0.35)
ROI_RIGHT = (0.65, 1.00)

# 台词字幕带（CCTV 版实测：白字台词字幕在画面底部 ~82%–96%）
BOTTOM_BAND = (0.78, 0.99)

DEFAULT_FPS = 2.0  # 字幕停留 >2s，2fps 足够
DEDUP_WINDOW_SEC = 1.5
DEDUP_HAMMING = 5


def ensure_dirs() -> None:
    for p in (ROOT, SRC, FRAMES, HITS, OUT, STATE):
        p.mkdir(parents=True, exist_ok=True)


def episode_dirs(label: str) -> dict[str, Path]:
    hits = HITS / label
    out = OUT / label
    hits.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    return {"hits": hits, "out": out}
