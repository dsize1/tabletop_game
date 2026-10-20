"""开发任务线 harness：接入项目技能（`.pi/skills`）与 MCP 工具。

设计边界（ADR-0015 §2.4 / B6 shadow，⚠️ 不要放宽）：

- **技能只读**：backend 虚拟根 = `<repo>/.pi/skills`，写/改/删/上传一律报错；
  仓库其余部分（`crates/`、`data/`、`.env`…）对 agent **不可见**。
- **不出站**：MCP 只读 `.pi/mcp.json` 的 stdio 配置，由 `DEV_MCP_SERVERS` 显式开启
  （默认空 = 关闭）；加载失败只降级并记原因，不阻断出计划。
- **不写业务库**：本模块不 import 任何 DB 客户端。

技能渐进披露：`SkillsMiddleware` 只把 `name/description` 注入系统提示，模型需要时
自己 `read_file` 读 `/<name>/SKILL.md`（虚拟路径，相对技能根）。

> 为什么不用 `CompositeBackend` 挂 `/skills`：deepagents 0.7.28 的
> `CompositeBackend.ls` 会把路由前缀重映射错一位（`/skills/x` → `/skill/x`），
> 导致 `SkillsMiddleware` 取不到 SKILL.md。整盘只读 backend 更简单也更安全。
"""

from __future__ import annotations

import asyncio
import json
import os
import threading
from collections.abc import Awaitable, Callable
from pathlib import Path
from typing import Any

#: 技能源（`SkillsMiddleware.sources`）：(后端路径, 展示标签)；后端根即技能目录
SKILL_SOURCES: list[tuple[str, str]] = [("/", "项目技能")]
#: MCP 配置文件（与 pi 共用，避免两套配置）
MCP_CONFIG_PATH_PARTS = (".pi", "mcp.json")
#: 允许从 `.pi/mcp.json` 透传给 MCP client 的连接字段（丢弃 description 等注释字段）
_MCP_CONN_KEYS = {"command", "args", "env", "cwd", "encoding", "transport"}


def repo_root() -> Path:
    """仓库根（`harness.py` → dev_executor → src → dev_executor(pkg) → agents → repo）。"""
    return Path(__file__).resolve().parents[4]


def skills_dir() -> Path:
    """项目技能目录（只读挂载点）。"""
    return repo_root() / ".pi" / "skills"


def mcp_config_path() -> Path:
    return repo_root().joinpath(*MCP_CONFIG_PATH_PARTS)


_SKILLS_BACKEND_CLS: type | None = None


def _build_skills_backend_cls() -> type:
    """构造只读技能 backend 子类（延迟到首次使用时才 import deepagents）。"""
    from deepagents.backends import protocol
    from deepagents.backends.filesystem import FilesystemBackend

    deny = "只读 backend：计划阶段禁止写/改/删文件"

    class SkillsBackend(FilesystemBackend):
        """根 = `.pi/skills` 的只读 backend：写接口全部返回错误。"""

        def __init__(self) -> None:
            super().__init__(root_dir=skills_dir(), virtual_mode=True)

        def write(self, file_path, content):  # noqa: ANN001, ANN201 - 与基类同签名
            return protocol.WriteResult(error=deny)

        def edit(self, file_path, old_string, new_string, replace_all=False):  # noqa: ANN001, ANN201
            return protocol.EditResult(error=deny)

        def delete(self, file_path):  # noqa: ANN001, ANN201
            return protocol.DeleteResult(error=deny)

        def upload_files(self, files):  # noqa: ANN001, ANN201
            return [protocol.FileUploadResponse(path=p, error=deny) for p, _ in files]

    return SkillsBackend


def skills_backend_cls() -> type:
    global _SKILLS_BACKEND_CLS
    if _SKILLS_BACKEND_CLS is None:
        _SKILLS_BACKEND_CLS = _build_skills_backend_cls()
    return _SKILLS_BACKEND_CLS


def build_backend():
    """只读技能 backend 实例（虚拟根 = `.pi/skills`）。"""
    return skills_backend_cls()()


def permissions():
    """兜底防线：即便将来换回可写 backend，`write` 也一律拒绝。"""
    from deepagents import FilesystemPermission

    return [FilesystemPermission(["write"], ["/", "/**"], "deny")]


def build_agent_kwargs() -> dict[str, Any]:
    """`create_deep_agent` 的技能侧关键字参数（backend / skills / permissions）。"""
    return {
        "backend": build_backend(),
        "skills": SKILL_SOURCES,
        "permissions": permissions(),
    }


def describe_skills() -> tuple[list[str], list[str]]:
    """同步列出技能名与加载告警（离线可测，不需要模型）。"""
    from deepagents.middleware.skills import _list_skills_with_errors

    backend = build_backend()
    names: list[str] = []
    warnings: list[str] = []
    for source_path, _label in SKILL_SOURCES:
        skills, error = _list_skills_with_errors(backend, source_path)
        names.extend(str(s.get("name", "")) for s in skills)
        if error:
            warnings.append(error)
    return names, warnings


def mcp_server_names() -> list[str]:
    """`DEV_MCP_SERVERS` 指定的 MCP server 名（逗号分隔，默认空 = 关闭）。"""
    raw = os.environ.get("DEV_MCP_SERVERS", "")
    return [x.strip() for x in raw.split(",") if x.strip()]


def mcp_connections(names: list[str] | None = None) -> dict[str, dict[str, Any]]:
    """从 `.pi/mcp.json` 取指定 server 的 stdio 连接配置。"""
    want = names if names is not None else mcp_server_names()
    if not want:
        return {}
    path = mcp_config_path()
    if not path.is_file():
        raise RuntimeError(f"MCP 配置不存在: {path}")
    data = json.loads(path.read_text(encoding="utf-8"))
    servers = data.get("mcpServers") or {}
    missing = [n for n in want if n not in servers]
    if missing:
        raise RuntimeError(f"MCP server 未配置: {', '.join(missing)}")
    out: dict[str, dict[str, Any]] = {}
    for name in want:
        conn = {k: v for k, v in (servers[name] or {}).items() if k in _MCP_CONN_KEYS}
        if "command" not in conn:
            raise RuntimeError(f"MCP server 缺少 command: {name}")
        conn.setdefault("transport", "stdio")
        out[name] = conn
    return out


async def aload_mcp_tools(
    names: list[str] | None = None,
    connections: dict[str, dict[str, Any]] | None = None,
) -> list[Any]:
    """加载 MCP 工具为 LangChain tool。

    MCP 工具是 coroutine-only（没有同步实现），所以 agent 必须走 `ainvoke`。
    `connections` 显式给出时优先（测试/临时 server 用），否则按 `names` 读 `.pi/mcp.json`。
    """
    conns = connections if connections is not None else mcp_connections(names)
    if not conns:
        return []
    from langchain_mcp_adapters.client import MultiServerMCPClient

    client = MultiServerMCPClient(conns)
    return list(await client.get_tools())


def run_coro_sync(factory: Callable[[], Awaitable[Any]]) -> Any:
    """在同步上下文运行协程；若已在事件循环中，则退到独立线程的新循环。"""
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        return asyncio.run(factory())

    box: dict[str, Any] = {}

    def _runner() -> None:
        try:
            box["value"] = asyncio.run(factory())
        except BaseException as e:  # noqa: BLE001 - 原样抛回调用线程
            box["error"] = e

    thread = threading.Thread(target=_runner, name="dev-executor-coro")
    thread.start()
    thread.join()
    if "error" in box:
        raise box["error"]
    return box.get("value")


def load_mcp_tools(
    names: list[str] | None = None,
    connections: dict[str, dict[str, Any]] | None = None,
) -> list[Any]:
    """`aload_mcp_tools` 的同步封装。"""
    return run_coro_sync(lambda: aload_mcp_tools(names, connections))
