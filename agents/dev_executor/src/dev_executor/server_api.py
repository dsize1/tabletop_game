"""server 内部 API 客户端（执行器 → server `/internal/dev/*`）。

执行器**只经 server** 读写业务状态（不直连 tabletop.db）；
所有失败转 `RuntimeError`，由 CLI 输出可读原因。
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any

DEFAULT_BASE = "http://127.0.0.1:3000"


class ServerApi:
    def __init__(self, base: str | None = None, timeout: float = 10.0) -> None:
        self.base = (base or os.environ.get("SERVER_URL") or DEFAULT_BASE).rstrip("/")
        self.timeout = timeout

    def _req(self, method: str, path: str, payload: dict | None = None) -> Any:
        data = None if payload is None else json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base}{path}",
            data=data,
            headers={"content-type": "application/json"} if data else {},
            method=method,
        )
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", "ignore")
            try:
                reason = json.loads(body).get("error")
            except Exception:  # noqa: BLE001
                reason = body[:200]
            raise RuntimeError(f"{method} {path} -> HTTP {e.code}: {reason}") from e
        except Exception as e:  # noqa: BLE001
            raise RuntimeError(f"{method} {path} 失败: {e}") from e
        return json.loads(raw) if raw else None

    # ---- run ----
    def start_run(self, session_id: str, stage: str) -> str:
        out = self._req(
            "POST",
            "/internal/dev/runs",
            {"session_id": session_id, "stage": stage, "executor": "langgraph"},
        )
        return str(out["run_id"])

    def set_run_status(self, run_id: str, status: str, ref: str | None = None, error: str | None = None) -> None:
        self._req(
            "POST",
            f"/internal/dev/runs/{run_id}/status",
            {
                "status": status,
                **({"executor_ref": ref} if ref else {}),
                **({"last_error": error} if error else {}),
            },
        )

    # ---- 任务/投影/事件 ----
    def list_tasks(self) -> list[dict]:
        return self._req("GET", "/internal/dev/tasks") or []

    def list_events(self, session_id: str, since_id: int = 0, limit: int = 200) -> list[dict]:
        return self._req(
            "GET",
            f"/internal/dev/events?session_id={session_id}&since_id={since_id}&limit={limit}",
        ) or []

    def get_task(self, session_id: str) -> dict:
        return self._req("GET", f"/internal/dev_task/{session_id}")["task"]

    def update_meta(self, session_id: str, title: str, plan_path: str, plan_format: str) -> None:
        self._req(
            "POST",
            "/internal/dev/meta",
            {
                "session_id": session_id,
                "title": title,
                "plan_path": plan_path,
                "plan_format": plan_format,
            },
        )

    def project(self, session_id: str, status: str, stage: str, ref: str) -> None:
        self._req(
            "POST",
            "/internal/dev/projection",
            {
                "session_id": session_id,
                "status": status,
                "stage": stage,
                "executor": "langgraph",
                "executor_ref": ref,
                "sync_source": "langgraph",
            },
        )

    def event(self, session_id: str, kind: str, actor: str, payload: dict) -> None:
        import uuid

        self._req(
            "POST",
            "/internal/dev/events",
            {
                "event_id": str(uuid.uuid4()),
                "session_id": session_id,
                "kind": kind,
                "actor": actor,
                "payload": json.dumps(payload, ensure_ascii=False),
            },
        )
