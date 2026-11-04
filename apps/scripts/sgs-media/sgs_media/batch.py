"""批量跑全季：按 episodes.json 逐集跑，N 路并行，支持断点续跑。

用法（在 apps/scripts/sgs-media 下）：
    python -m sgs_media batch --from 1 --to 84 --workers 6
    python -m sgs_media batch --from 1 --to 3 --workers 3 --force   # 小批验证

续跑：某集 `out/epNN/stats.json` 存在即跳过（`--force` 可强制重跑）。
每集日志写 `out/epNN/run.log`，汇总写 `out/batch-summary.json`。
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from .paths import OUT, ROOT, ensure_dirs

EPISODES_JSON = Path(__file__).resolve().parent.parent / "episodes.json"


def load_episodes() -> list[dict]:
    data = json.loads(EPISODES_JSON.read_text(encoding="utf-8"))
    return data["episodes"]


def label_of(ep: int) -> str:
    return f"ep{ep:02d}"


def is_done(ep: int) -> bool:
    return (OUT / label_of(ep) / "stats.json").exists()


def run_one(ep: int, page: str, fps: float, force: bool) -> dict:
    label = label_of(ep)
    if not force and is_done(ep):
        return {"ep": ep, "label": label, "skipped": True}
    out_dir = OUT / label
    out_dir.mkdir(parents=True, exist_ok=True)
    log_path = out_dir / "run.log"
    env = dict(os.environ)
    env["PYTHONIOENCODING"] = "utf-8"
    env["PYTHONUTF8"] = "1"
    env.setdefault("OMP_NUM_THREADS", "2")
    cmd = [
        sys.executable,
        "-m",
        "sgs_media",
        "episode",
        "--page",
        page,
        "--label",
        label,
        "--mode",
        "side",
        "--fps",
        str(fps),
    ]
    t0 = time.perf_counter()
    with log_path.open("w", encoding="utf-8") as log:
        proc = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=str(Path(__file__).resolve().parent.parent))
    dt = time.perf_counter() - t0
    hits = 0
    stats_file = out_dir / "stats.json"
    if stats_file.exists():
        try:
            hits = int(json.loads(stats_file.read_text(encoding="utf-8")).get("hits_kept") or 0)
        except Exception:  # noqa: BLE001
            hits = 0
    return {"ep": ep, "label": label, "ok": proc.returncode == 0, "seconds": round(dt, 1), "hits": hits}


def run_batch(ep_from: int, ep_to: int, workers: int, fps: float, force: bool, log=print) -> dict:
    ensure_dirs()
    episodes = [e for e in load_episodes() if ep_from <= int(e["ep"]) <= ep_to]
    todo = episodes
    log(f"[batch] 共 {len(todo)} 集（ep{ep_from}..ep{ep_to}），workers={workers}，fps={fps}")
    results: list[dict] = []
    t0 = time.perf_counter()
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(run_one, int(e["ep"]), e["page"], fps, force): e for e in todo}
        for i, fut in enumerate(as_completed(futures), 1):
            r = fut.result()
            results.append(r)
            done = sum(1 for x in results if not x.get("skipped"))
            elapsed = time.perf_counter() - t0
            eta = (elapsed / done) * (len(todo) - i) if done else 0
            log(
                f"[batch] {i}/{len(todo)} {r['label']} "
                f"{'skip' if r.get('skipped') else ('ok' if r.get('ok') else 'FAIL')} "
                f"{r.get('seconds', 0)}s hits={r.get('hits', 0)} | elapsed {elapsed/60:.1f}min eta {eta/60:.1f}min"
            )
    summary = {
        "from": ep_from,
        "to": ep_to,
        "workers": workers,
        "fps": fps,
        "elapsed_sec": round(time.perf_counter() - t0, 1),
        "ran": len([r for r in results if not r.get("skipped")]),
        "failed": [r["label"] for r in results if not r.get("ok", True)],
        "hits_total": sum(int(r.get("hits") or 0) for r in results),
        "per_episode": sorted(results, key=lambda r: r["ep"]),
    }
    (OUT / "batch-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    return summary
