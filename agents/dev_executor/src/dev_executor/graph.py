"""LangGraph 骨架（shadow）：plan → interrupt(approve) → execute → interrupt(verify) → summarize。

设计约束（ADR-0015 §2.4 / §4）：
- **不写业务库**：checkpoint 独立文件；不 import 任何 DB 客户端。
- **不出站**：不调 qq.push / server（B4/B5 才接）。
- **无副作用节点**：`interrupt()` 恢复会从节点开头重跑，节点内不得提交/推送/改文件。
- `thread_id = sessionId`（与 dev_tasks / 计划文档统一键）。

`plan` 节点支持 `DEV_PLANNER=stub（默认）/llm/deepagents`（B6.2），失败自动降级 stub。
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Literal

from langgraph.graph import END, START, StateGraph
from langgraph.types import interrupt

from .planner import PLAN_FORMAT, SECTIONS as PLAN_SECTIONS, make_plan
from .state import DevState




def plan_node(state: DevState) -> DevState:
    """生成 6 分区计划（`DEV_PLANNER=stub|llm|deepagents`，失败降级 stub）。

    shadow：不写文件、不调 server；结果只进 checkpoint。
    """
    planner = os.environ.get("DEV_PLANNER", "stub").strip() or "stub"
    requirement = state.get("requirement") or ""
    try:
        md, fmt, used = make_plan(planner, requirement)  # type: ignore[arg-type]
    except Exception as e:  # noqa: BLE001 - 生成失败必须可降级
        md, fmt, used = make_plan("stub", requirement)
        md += chr(10) + f"> 计划生成降级：planner={planner} 失败（{e}）" + chr(10)
    return {"plan_markdown": md, "plan_format": fmt, "planner": used}


def _decision(payload: dict) -> bool:
    """把 resume 值归一化为批准/驳回（兼容字符串与 dict）。"""
    raw = payload
    if isinstance(payload, dict):
        raw = payload.get("decision", "")
    return str(raw).strip().lower() in {"approve", "approved", "y", "yes", "true"}


def await_approval(state: DevState) -> DevState:
    """人工批准计划（interrupt；恢复后从本节点重跑，无副作用）。"""
    verdict = interrupt(
        {
            "kind": "approve_plan",
            "session_id": state.get("session_id", ""),
            "plan_format": state.get("plan_format", PLAN_FORMAT),
            # 只回传摘要，完整计划在 checkpoint / 计划文档
            "plan_excerpt": (state.get("plan_markdown") or "")[:600],
        }
    )
    return {"approved": _decision(verdict)}


def execute(state: DevState) -> DevState:
    """shadow：执行阶段只置标记，不产生任何外部副作用。"""
    if not state.get("approved"):
        return {"executed": False}
    return {"executed": True}


def await_verify(state: DevState) -> DevState:
    """人工验收（interrupt）。"""
    verdict = interrupt(
        {
            "kind": "approve_verify",
            "session_id": state.get("session_id", ""),
            "executed": bool(state.get("executed")),
        }
    )
    return {"verified": _decision(verdict)}


def summarize(state: DevState) -> DevState:
    parts = [
        f"session={state.get('session_id', '')}",
        f"approved={bool(state.get('approved'))}",
        f"executed={bool(state.get('executed'))}",
        f"verified={bool(state.get('verified'))}",
    ]
    return {"summary": " ".join(parts)}


def _after_approval(state: DevState) -> Literal["execute", "__end__"]:
    return "execute" if state.get("approved") else END


def _after_verify(state: DevState) -> Literal["summarize", "__end__"]:
    return "summarize" if state.get("verified") else END


def checkpoint_path() -> str:
    """checkpoint 独立库（不碰 tabletop.db）；可用环境变量覆盖。"""
    override = os.environ.get("DEV_CHECKPOINT_PATH")
    if override:
        return override
    repo_root = Path(__file__).resolve().parents[4]  # graph.py → dev_executor → src → dev_executor(pkg) → agents → repo
    return str(repo_root / "data" / "langgraph" / "checkpoints.sqlite")


def build_graph(checkpointer=None):
    """构建图；传入 checkpointer（CLI 用 SqliteSaver，测试用内存/临时文件）。"""
    g = StateGraph(DevState)
    g.add_node("plan", plan_node)
    g.add_node("await_approval", await_approval)
    g.add_node("execute", execute)
    g.add_node("await_verify", await_verify)
    g.add_node("summarize", summarize)

    g.add_edge(START, "plan")
    g.add_edge("plan", "await_approval")
    g.add_conditional_edges("await_approval", _after_approval, {"execute": "execute", END: END})
    g.add_edge("execute", "await_verify")
    g.add_conditional_edges("await_verify", _after_verify, {"summarize": "summarize", END: END})
    g.add_edge("summarize", END)
    return g.compile(checkpointer=checkpointer)


def sqlite_saver(path: str | None = None):
    """独立 SQLite checkpointer（目录自动创建）。"""
    from langgraph.checkpoint.sqlite import SqliteSaver

    p = path or checkpoint_path()
    Path(p).parent.mkdir(parents=True, exist_ok=True)
    return SqliteSaver.from_conn_string(p)
