"""对账：投影（dev_tasks）↔ 事件 ↔ checkpoint 巡检。

背景（ADR-0015 §2.4 第 3/6 项）：L3 下 checkpoint 是状态权威，`dev_tasks` 只是投影。
执行器直写投影、server 确认也写投影，缺少独立校验 → 本巡检回答「三者是否一致」。

用法：
  uv run dev-executor reconcile            # 只报告（退出码 1 表示有差异）
  uv run dev-executor reconcile --fix      # 按事件推导修正投影（谨慎；不改 checkpoint）
  uv run dev-executor reconcile --json     # 机器可读

判定规则（保守）：
- 以**事件**推导期望的 (status, stage)，与投影比较；
- checkpoint：awaiting_confirm 应在 `await_approval`、verifying 应在 `await_verify`；
  done 应 `verified=True` 且无 next；缺失 checkpoint 记为 warn（不判错）。
- `--fix` 只用事件推导值覆盖投影（`sync_source=reconcile`），不动 checkpoint 与任务历史。
"""

from __future__ import annotations

import json
from typing import Any

from .graph import build_graph, sqlite_saver
from .server_api import ServerApi

# 事件 → 期望投影（后出现者优先）
EXPECTED_BY_EVENT = {
    "plan_ready": ("awaiting_confirm", "plan"),
    "executed": ("verifying", "execute"),
    "run_summary": ("done", "verify"),
    "verified": ("done", "verify"),
    "cancelled": ("cancelled", "verify"),
    "failed": ("failed", "verify"),
}


def _expected(events: list[dict]) -> tuple[str, str] | None:
    exp: tuple[str, str] | None = None
    for e in sorted(events, key=lambda x: x.get("id", 0)):
        hit = EXPECTED_BY_EVENT.get(str(e.get("kind")))
        if hit:
            exp = hit
    return exp


def _checkpoint_view(session_id: str, saver_factory=sqlite_saver) -> dict[str, Any]:
    with saver_factory() as saver:
        graph = build_graph(saver)
        snap = graph.get_state({"configurable": {"thread_id": session_id}})
    if not snap or not snap.values:
        return {"exists": False}
    return {
        "exists": True,
        "approved": bool(snap.values.get("approved")),
        "executed": bool(snap.values.get("executed")),
        "verified": bool(snap.values.get("verified")),
        "next": list(snap.next),
    }


def reconcile(fix: bool = False) -> dict[str, Any]:
    api = ServerApi()
    tasks = api.list_tasks()
    issues: list[dict[str, Any]] = []
    checked = 0
    for t in tasks:
        sid = str(t.get("session_id") or "")
        if not sid:
            continue
        checked += 1
        events = api.list_events(sid, 0, 200)
        exp = _expected(events)
        cur = (str(t.get("status")), str(t.get("stage") or ""))
        cp = _checkpoint_view(sid)

        if exp and cur != exp:
            issues.append({"session_id": sid, "kind": "projection_mismatch", "expected": exp, "actual": cur})
            if fix:
                api.project(sid, exp[0], exp[1], f"reconcile:{cp.get('next') or ''}")

        # 事件与 checkpoint 的轻量一致性（只对非终态任务判）
        if cp["exists"]:
            nxt = cp["next"]
            if cur[0] == "awaiting_confirm" and "await_approval" not in nxt:
                issues.append({"session_id": sid, "kind": "checkpoint_unexpected", "detail": f"awaiting_confirm 但 next={nxt}"})
            if cur[0] == "verifying" and "await_verify" not in nxt:
                issues.append({"session_id": sid, "kind": "checkpoint_unexpected", "detail": f"verifying 但 next={nxt}"})
            if cur[0] == "done" and (not cp["verified"] or nxt):
                issues.append({"session_id": sid, "kind": "checkpoint_unexpected", "detail": f"done 但 verified={cp['verified']} next={nxt}"})
        elif cur[0] not in {"done", "failed", "cancelled"}:
            issues.append({"session_id": sid, "kind": "checkpoint_missing", "detail": f"status={cur[0]} 无 checkpoint（可能为旧任务）"})

    return {"checked": checked, "issues": issues, "fixed": bool(fix)}


def main_reconcile(fix: bool = False, as_json: bool = False) -> int:
    report = reconcile(fix=fix)
    if as_json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(f"对账完成：任务 {report['checked']} 个，差异 {len(report['issues'])} 条")
        for it in report["issues"]:
            print(f"- [{it['kind']}] {it['session_id']} {it.get('detail') or ''} "
                  f"{'expected=' + str(it.get('expected')) + ' actual=' + str(it.get('actual')) if 'expected' in it else ''}".rstrip())
    return 0 if not report["issues"] else 1


if __name__ == "__main__":
    raise SystemExit(main_reconcile())
