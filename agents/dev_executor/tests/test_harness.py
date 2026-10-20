"""harness 自测（离线）：技能只读挂载 / 技能清单 / MCP 配置与工具发现 / 仓库不被写。"""

from __future__ import annotations

import asyncio
import os
import sys
from pathlib import Path

from dev_executor import harness

EXPECTED_SKILLS = {"game-engine-dev", "plan-workflow", "qq-bot-dev", "rust-axum"}
FAKE_MCP = Path(__file__).resolve().parent / "fake_mcp_server.py"


def _err(result: object) -> str:
    """backend 结果为 dataclass 或 dict，统一取 error 文本。"""
    if isinstance(result, dict):
        return str(result.get("error") or "")
    return str(getattr(result, "error", None) or "")


def test_skills_loaded() -> None:
    names, warnings = harness.describe_skills()
    assert warnings == [], f"技能加载告警: {warnings}"
    assert set(names) == EXPECTED_SKILLS, names


def test_backend_is_read_only() -> None:
    backend = harness.build_backend()
    assert "只读" in _err(backend.write("/rust-axum/SKILL.md", "x"))
    assert "只读" in _err(backend.edit("/rust-axum/SKILL.md", "a", "b"))
    assert "只读" in _err(backend.delete("/rust-axum/SKILL.md"))
    up = backend.upload_files([("/rust-axum/SKILL.md", b"x")])
    assert up and "只读" in _err(up[0])


def test_backend_hides_repo() -> None:
    """虚拟根之外（仓库其它文件）不可读；技能文件可读。"""
    backend = harness.build_backend()
    ok = backend.read("/rust-axum/SKILL.md", 0, 5)
    assert not _err(ok), ok
    outside = backend.read("/AGENTS.md", 0, 5)
    assert _err(outside), "仓库文件不该被 agent 读到"


def _snapshot_skills() -> list[tuple[str, int, int]]:
    root = harness.repo_root() / ".pi" / "skills"
    return sorted(
        (str(p.relative_to(root)), p.stat().st_mtime_ns, p.stat().st_size)
        for p in root.rglob("*")
        if p.is_file()
    )


def test_repo_not_written() -> None:
    before = _snapshot_skills()
    backend = harness.build_backend()
    backend.write("/rust-axum/SKILL.md", "覆盖尝试")
    harness.describe_skills()
    assert _snapshot_skills() == before


def test_mcp_disabled_by_default() -> None:
    saved = os.environ.pop("DEV_MCP_SERVERS", None)
    try:
        assert harness.mcp_server_names() == []
        assert harness.mcp_connections() == {}
        assert harness.load_mcp_tools() == []
    finally:
        if saved is not None:
            os.environ["DEV_MCP_SERVERS"] = saved


def test_mcp_config_reads_pi_json() -> None:
    conns = harness.mcp_connections(["playwright"])
    assert set(conns) == {"playwright"}
    cfg = conns["playwright"]
    assert cfg["command"] == "npx"
    assert any("@playwright/mcp" in a for a in cfg["args"])
    assert "description" not in cfg  # 注释字段不透传给 MCP client


def test_mcp_tool_discovered_and_callable() -> None:
    conns = {"fake": {"command": sys.executable, "args": [str(FAKE_MCP)], "transport": "stdio"}}
    tools = harness.load_mcp_tools(connections=conns)
    assert [t.name for t in tools] == ["echo"], [t.name for t in tools]
    out = harness.run_coro_sync(lambda: tools[0].ainvoke({"text": "hi"}))
    assert "echo:hi" in str(out), out


def test_run_coro_sync_inside_event_loop() -> None:
    async def inner() -> int:
        return harness.run_coro_sync(lambda: asyncio.sleep(0, result=7))

    assert asyncio.run(inner()) == 7


def test_permissions_deny_write() -> None:
    rules = harness.permissions()
    assert rules, "必须至少一条 deny 规则"
    rule = rules[0]
    operations = rule["operations"] if isinstance(rule, dict) else rule.operations
    paths = rule["paths"] if isinstance(rule, dict) else rule.paths
    mode = rule["mode"] if isinstance(rule, dict) else rule.mode
    assert "write" in operations and mode == "deny"
    assert "/" in paths and "/**" in paths


def test_cli_print_skills() -> None:
    """`dev-executor plan --print-skills` 输出技能名（排查用）。"""
    import argparse
    import contextlib
    import io
    import json

    from dev_executor.cli import cmd_plan

    args = argparse.Namespace(planner="stub", requirement="x", out=None, diff=None, print_skills=True)
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        assert cmd_plan(args) == 0
    payload = json.loads(buf.getvalue())
    assert set(payload["skills"]["names"]) == EXPECTED_SKILLS
    assert payload["skills"]["warnings"] == []


def main() -> None:
    test_skills_loaded()
    test_backend_is_read_only()
    test_backend_hides_repo()
    test_repo_not_written()
    test_mcp_disabled_by_default()
    test_mcp_config_reads_pi_json()
    test_mcp_tool_discovered_and_callable()
    test_run_coro_sync_inside_event_loop()
    test_permissions_deny_write()
    test_cli_print_skills()
    print("OK test_harness 通过")


if __name__ == "__main__":
    main()
