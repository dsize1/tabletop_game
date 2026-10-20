"""shadow 骨架自测：中断/恢复、独立 checkpoint、不碰 tabletop.db。"""

from __future__ import annotations

import os
import sqlite3
import tempfile
from pathlib import Path

from langgraph.types import Command

from dev_executor.graph import build_graph, checkpoint_path, sqlite_saver

REPO = Path(__file__).resolve().parents[3]
TABLETOP_DB = REPO / "data" / "tabletop.db"


def _snapshot(p: Path):
    if not p.exists():
        return None
    st = p.stat()
    return (st.st_size, st.st_mtime_ns)


def _first_interrupt(result: dict):
    items = result.get("__interrupt__") or []
    if not items:
        return None
    it = items[0]
    return getattr(it, "value", it)


def main() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="dev-exec-checkpoint-"))
    ckpt = tmp / "checkpoints.sqlite"
    os.environ["DEV_CHECKPOINT_PATH"] = str(ckpt)
    before = _snapshot(TABLETOP_DB)

    # 1) 启动 → 计划 → 人工批准中断
    with sqlite_saver(str(ckpt)) as saver:
        g = build_graph(saver)
        r = g.invoke(
            {"session_id": "t1", "requirement": "加个小游戏"},
            {"configurable": {"thread_id": "t1"}},
        )
        intr = _first_interrupt(r)
        assert intr and intr["kind"] == "approve_plan", intr
        assert "## 待办实现" in r["plan_markdown"]

        # 2) 批准 → 执行 → 验收中断
        r = g.invoke(Command(resume="approve"), {"configurable": {"thread_id": "t1"}})
        intr = _first_interrupt(r)
        assert intr and intr["kind"] == "approve_verify", intr
        assert r.get("executed") is True

        # 3) 验收通过 → 汇总结束
        r = g.invoke(Command(resume="approve"), {"configurable": {"thread_id": "t1"}})
        assert _first_interrupt(r) is None, r
        assert r.get("verified") is True
        assert "verified=True" in (r.get("summary") or "")

        # 4) checkpoint 独立库存在且可读；thread_id 隔离
        assert ckpt.exists() and ckpt.stat().st_size > 0
        snap = g.get_state({"configurable": {"thread_id": "t1"}})
        assert snap.values.get("session_id") == "t1"
        other = g.get_state({"configurable": {"thread_id": "t2"}})
        assert not other.values

        # 5) 驳回路径：批准时 reject → 直接结束，不执行
        r = g.invoke(
            {"session_id": "t3", "requirement": "x"},
            {"configurable": {"thread_id": "t3"}},
        )
        assert _first_interrupt(r)["kind"] == "approve_plan"
        r = g.invoke(Command(resume="reject"), {"configurable": {"thread_id": "t3"}})
        assert _first_interrupt(r) is None
        assert r.get("approved") is False
        assert not r.get("executed")
        assert not r.get("summary")

    after = _snapshot(TABLETOP_DB)
    assert before == after, "shadow 阶段不得修改 tabletop.db"

    # 6) checkpoint 文件是 sqlite（可被独立工具读取）
    conn = sqlite3.connect(ckpt)
    try:
        tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    finally:
        conn.close()
    assert tables, "checkpoint 库应有表"

    print("OK test_shadow 通过（interrupt/resume、独立 checkpoint、tabletop.db 未变）")


if __name__ == "__main__":
    main()
