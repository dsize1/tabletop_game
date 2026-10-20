"""dev 任务执行器（shadow）。

只做 LangGraph 状态编排与人工中断恢复；不写 tabletop.db、不出站。
"""

__all__ = ["graph", "state", "cli"]
