"""worker：按 stage 驱动 LangGraph（L3：图 = 状态管理，server = 投影）。

- plan    ：新建 thread → 跑到 `approve_plan` 中断 → 写计划文件 + 投影 awaiting_confirm
- execute ：`Command(resume=approve)` → 跑到 `approve_verify` 中断 → 投影 verifying
- verify  ：`Command(resume=approve)` → 跑到 summary → 投影 done

约束：
- 业务状态只经 server（`ServerApi`）；checkpoint 独立库（`data/langgraph/checkpoints.sqlite`）。
- 计划文件写 `memory_palace/plan/{session_id}.md`（`#进度`/短链读取同一份）。
- 每个 stage 都登记 `dev_runs`（单活跃由 DB 唯一索引兼底）。
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from langgraph.types import Command

from .graph import build_graph, sqlite_saver
from .planner import SECTIONS
from .server_api import ServerApi


def repo_root() -> Path:
    return Path(__file__).resolve().parents[4]


def plan_path_for(session_id: str) -> Path:
    return repo_root() / "memory_palace" / "plan" / f"{session_id}.md"


def _first_interrupt(result: dict):
    items = result.get("__interrupt__") or []
    if not items:
        return None
    it = items[0]
    return getattr(it, "value", it)


def _cfg(session_id: str) -> dict:
    return {"configurable": {"thread_id": session_id}}


def _title(plan_md: str, requirement: str) -> str:
    first = (plan_md.splitlines() or [""])[0].lstrip("# ").strip()
    return (first or requirement or "未命名任务")[:60]


def work(session_id: str, stage: str, requirement: str = "") -> dict:
    if stage not in {"plan", "execute", "verify"}:
        raise RuntimeError(f"未知 stage: {stage}")
    api = ServerApi()
    run_id = api.start_run(session_id, stage)
    out: dict = {"session_id": session_id, "stage": stage, "run_id": run_id}
    try:
        api.set_run_status(run_id, "running")
        with sqlite_saver() as saver:
            graph = build_graph(saver)
            if stage == "plan":
                result = graph.invoke(
                    {"session_id": session_id, "requirement": requirement}, _cfg(session_id)
                )
                intr = _first_interrupt(result)
                if not intr or intr.get("kind") != "approve_plan":
                    raise RuntimeError(f"计划阶段未停在 approve_plan 中断: {intr}")
                md = result.get("plan_markdown") or ""
                path = plan_path_for(session_id)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(md, encoding="utf-8")
                rel = path.relative_to(repo_root()).as_posix()
                api.update_meta(session_id, _title(md, requirement), rel, result.get("plan_format", "plan6-v1"))
                api.event(
                    session_id,
                    "plan_ready",
                    "langgraph",
                    {
                        "run_id": run_id,
                        "plan_path": rel,
                        "chars": len(md),
                        "planner": result.get("planner", ""),
                        "sections": [s for s in SECTIONS if f"## {s}" in md],
                    },
                )
                api.project(session_id, "awaiting_confirm", "plan", run_id)
                out["interrupt"] = intr
            elif stage == "execute":
                result = graph.invoke(Command(resume="approve"), _cfg(session_id))
                intr = _first_interrupt(result)
                if not intr or intr.get("kind") != "approve_verify":
                    raise RuntimeError(f"执行阶段未停在 approve_verify 中断: {intr}")
                api.event(session_id, "executed", "langgraph", {"run_id": run_id, "shadow": True})
                api.project(session_id, "verifying", "execute", run_id)
                out["interrupt"] = intr
            else:  # verify
                result = graph.invoke(Command(resume="approve"), _cfg(session_id))
                api.event(
                    session_id,
                    "run_summary",
                    "langgraph",
                    {"run_id": run_id, "summary": result.get("summary", "")},
                )
                api.project(session_id, "done", "verify", run_id)
                out["summary"] = result.get("summary")
        api.set_run_status(run_id, "succeeded", session_id)
        out["ok"] = True
        return out
    except Exception as e:  # noqa: BLE001
        try:
            api.set_run_status(run_id, "failed", session_id, str(e)[:500])
            api.event(session_id, "failed", "langgraph", {"run_id": run_id, "stage": stage, "error": str(e)[:300]})
        except Exception:  # noqa: BLE001
            pass
        raise


def main_work(session_id: str, stage: str, requirement: str = "") -> int:
    try:
        out = work(session_id, stage, requirement)
    except Exception as e:  # noqa: BLE001
        print(json.dumps({"ok": False, "error": str(e)}, ensure_ascii=False, indent=2))
        return 1
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0
