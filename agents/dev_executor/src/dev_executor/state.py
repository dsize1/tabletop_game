"""执行器状态定义（LangGraph State）。"""

import operator
from typing import Annotated, TypedDict


class DevState(TypedDict, total=False):
    """一次 dev 任务的执行状态。

    只放**编排需要**的字段；业务权威状态仍在 server（ADR-0015）。
    """

    session_id: str
    requirement: str
    plan_markdown: str
    plan_format: str
    planner: str
    approved: bool
    executed: bool
    verified: bool
    summary: str
    instructions: Annotated[list[str], operator.add]
