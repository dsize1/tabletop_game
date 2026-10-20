"""CLI：shadow 阶段的启动 / 恢复 / 查看。

用法：
  uv run dev-executor run    --session-id s1 --requirement "…"
  uv run dev-executor resume --session-id s1 --decision approve
  uv run dev-executor state  --session-id s1
"""

from __future__ import annotations

import argparse
import difflib
import json
from pathlib import Path

from langgraph.types import Command

from .graph import build_graph, checkpoint_path, sqlite_saver
from .planner import SECTIONS, make_plan
from .reconcile import main_reconcile
from .worker import main_work

NEWLINE = chr(10)


def _cfg(session_id: str) -> dict:
    return {"configurable": {"thread_id": session_id}}


def _first_interrupt(result: dict) -> dict | None:
    interrupts = result.get("__interrupt__") or []
    if not interrupts:
        return None
    item = interrupts[0]
    return getattr(item, "value", item)


def cmd_plan(args: argparse.Namespace) -> int:
    """只生成计划（不进图、不写 checkpoint）：用于质量对比。"""
    md, fmt, used = make_plan(args.planner, args.requirement)  # type: ignore[arg-type]
    out = {
        "planner": used,
        "plan_format": fmt,
        "chars": len(md),
        "sections": {s: (f"## {s}" in md) for s in SECTIONS},
        "excerpt": md[:400],
    }
    if args.out:
        Path(args.out).write_text(md, encoding="utf-8")
        out["out"] = args.out
    if args.diff:
        other = Path(args.diff).read_text(encoding="utf-8")
        diff = list(
            difflib.unified_diff(
                other.splitlines(),
                md.splitlines(),
                fromfile=args.diff,
                tofile="generated",
                lineterm="",
                n=2,
            )
        )
        out["diff_lines"] = len(diff)
        out["diff_head"] = NEWLINE.join(diff[:80])
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    with sqlite_saver() as saver:
        graph = build_graph(saver)
        result = graph.invoke(
            {"session_id": args.session_id, "requirement": args.requirement},
            _cfg(args.session_id),
        )
    payload = _first_interrupt(result)
    print(json.dumps({"checkpoint": checkpoint_path(), "interrupt": payload}, ensure_ascii=False, indent=2))
    return 0


def cmd_resume(args: argparse.Namespace) -> int:
    with sqlite_saver() as saver:
        graph = build_graph(saver)
        result = graph.invoke(Command(resume=args.decision), _cfg(args.session_id))
    payload = _first_interrupt(result)
    out = {"interrupt": payload} if payload else {"summary": result.get("summary")}
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


def cmd_state(args: argparse.Namespace) -> int:
    with sqlite_saver() as saver:
        graph = build_graph(saver)
        snap = graph.get_state(_cfg(args.session_id))
    print(
        json.dumps(
            {
                "values": {k: snap.values.get(k) for k in ("session_id", "approved", "executed", "verified", "summary", "instructions")},
                "next": list(snap.next),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="dev-executor", description="dev 执行器（shadow）")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_plan = sub.add_parser("plan", help="只生成计划（stub/llm/deepagents）")
    p_plan.add_argument("--requirement", required=True)
    p_plan.add_argument("--planner", default="stub", choices=["stub", "llm", "deepagents"])
    p_plan.add_argument("--out", help="把计划写到指定文件")
    p_plan.add_argument("--diff", help="与已有计划（如 pi 生成的）做 unified diff")
    p_plan.set_defaults(func=cmd_plan)

    p_rec = sub.add_parser("reconcile", help="对账：投影↔事件↔checkpoint")
    p_rec.add_argument("--fix", action="store_true", help="按事件推导修正投影（不动 checkpoint）")
    p_rec.add_argument("--json", action="store_true", help="输出 JSON")
    p_rec.set_defaults(func=lambda a: main_reconcile(a.fix, a.json))

    p_work = sub.add_parser("work", help="按 stage 驱动 LangGraph（L3：plan/execute/verify）")
    p_work.add_argument("--session-id", required=True)
    p_work.add_argument("--stage", required=True, choices=["plan", "execute", "verify", "append"])
    p_work.add_argument("--requirement", default="")
    p_work.add_argument("--instruction", default="")
    p_work.set_defaults(func=lambda a: main_work(a.session_id, a.stage, a.requirement, a.instruction))

    p_run = sub.add_parser("run", help="启动一次 shadow 执行（到人工批准中断）")
    p_run.add_argument("--session-id", required=True)
    p_run.add_argument("--requirement", default="")
    p_run.set_defaults(func=cmd_run)

    p_resume = sub.add_parser("resume", help="恢复（approve/reject）")
    p_resume.add_argument("--session-id", required=True)
    p_resume.add_argument("--decision", default="approve", choices=["approve", "reject"])
    p_resume.set_defaults(func=cmd_resume)

    p_state = sub.add_parser("state", help="查看 checkpoint 状态")
    p_state.add_argument("--session-id", required=True)
    p_state.set_defaults(func=cmd_state)

    args = parser.parse_args(argv)
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
