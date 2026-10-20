"""计划生成：stub / llm / deepagents（B6.2）。

约束：
- **不出站、不写业务状态**；生成结果只进 checkpoint（B7 才投影到 server）。
- 所有 planner 都必须产出**6 分区** markdown（`plan6-v1`），否则 `#进度` 会归零。
- DeepAgents 使用只读技能 backend（虚拟根 `<repo>/.pi/skills`，写降级为错误）+ 小
  `recursion_limit`，仓库其余文件不可见；MCP 工具由 `DEV_MCP_SERVERS` 显式开启（见 `harness.py`）。
"""

from __future__ import annotations

import json
import os
import urllib.request
from typing import Literal

PLAN_FORMAT = "plan6-v1"

SECTIONS = [
    "计划",
    "待办实现",
    "待确定事项",
    "act 阶段变动的文件列表",
    "验证事项",
    "harness 优化项",
]

SYSTEM_PROMPT = """你是桌游助手项目的开发计划生成器。只输出 markdown 计划，**不要写任何文件**。
系统提示里可能已列出「技能清单」（`/skills/<name>/SKILL.md`）：若某条需求与技能领域匹配，
可以先用 `read_file`（只读）读该 SKILL.md 再写计划；不需要时不要调用任何工具。
必须包含且只包含以下 6 个二级标题（顺序固定）：
## 计划
## 待办实现
## 待确定事项
## act 阶段变动的文件列表
## 验证事项
## harness 优化项

要求：
- 「待办实现」「验证事项」用 `- [ ]` 复选框条目，每条可独立验证；
- 每条待办/验证尽量写清文件路径或命令；
- 不臆造外部数值约束；不确定的写入「待确定事项」；
- 不要输出 6 个分区之外的内容。
"""

PlannerName = Literal["stub", "llm", "deepagents"]

#: DeepAgents recursion_limit：中间件 hook 也计入步数，读技能/调 MCP 需要额外轮次
RECURSION_LIMIT = 20


def stub_plan(requirement: str) -> str:
    """确定性骨架（离线/测试用）。"""
    req = requirement.strip() or "（未提供需求）"
    lines = [f"# 计划：{req}", "", "## 计划", f"- 需求：{req}", ""]
    for section in SECTIONS[1:]:
        lines += [f"## {section}", "- [ ] （待执行器填充）", ""]
    return "\n".join(lines).rstrip() + "\n"


def normalize_plan(text: str, requirement: str) -> str:
    """把任意输出规整成 plan6-v1：去代码围栏、保证 6 分区齐全且顺序固定。"""
    body = (text or "").strip()
    if body.startswith("```"):
        body = body.split("\n", 1)[1] if "\n" in body else ""
        if body.rstrip().endswith("```"):
            body = body.rstrip()[:-3]
    # 按分区切分（容忍 `## 计划 ` 之类尾随空格）
    found: dict[str, list[str]] = {}
    current: str | None = None
    preamble: list[str] = []
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith("## "):
            name = stripped[3:].strip()
            if name in SECTIONS:
                current = name
                found.setdefault(current, [])
                continue
            current = None
        if current is None:
            if stripped and not stripped.startswith("# "):
                preamble.append(line)
        else:
            found[current].append(line)

    req = requirement.strip() or "（未提供需求）"
    out: list[str] = [f"# 计划：{req}", ""]
    if preamble:
        out += [ln for ln in preamble if ln.strip()] + [""]
    for section in SECTIONS:
        out.append(f"## {section}")
        lines = [ln for ln in found.get(section, []) if ln.strip()]
        if not lines:
            lines = ["- [ ] （待补充）"]
        out += lines + [""]
    return "\n".join(out).rstrip() + "\n"


def llm_plan(requirement: str, timeout: float = 90.0) -> str:
    """裸 HTTP 调 OpenAI 兼容端点（默认 DeepSeek），与 chat.ts 同风格。"""
    api_key = os.environ.get("LLM_API_KEY", "").strip()
    if not api_key:
        raise RuntimeError("LLM_API_KEY 未配置")
    base = (os.environ.get("LLM_BASE_URL") or "https://api.deepseek.com").rstrip("/")
    model = os.environ.get("LLM_MODEL") or "deepseek-flash"
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": f"开发需求：\n{requirement.strip()}"},
        ],
        "temperature": 0.2,
    }
    req = urllib.request.Request(
        f"{base}/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json", "authorization": f"Bearer {api_key}"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except Exception as e:  # noqa: BLE001 - 统一转可读错误
        raise RuntimeError(f"LLM 调用失败: {e}") from e
    try:
        content = data["choices"][0]["message"]["content"]
    except (KeyError, IndexError) as e:
        raise RuntimeError(f"LLM 返回结构异常: {json.dumps(data)[:300]}") from e
    return str(content or "")


async def adeepagents_plan(requirement: str, timeout: float = 120.0) -> str:
    """DeepAgents（只读技能 backend + 可选 MCP 工具；小 recursion_limit）。

    - 技能：`harness.build_agent_kwargs()` 提供只读 backend（虚拟根 = `.pi/skills`）、
      技能源与 `write` deny；模型只在系统提示看到 `name/description`，需要时自己读
      `/<skill>/SKILL.md`。
    - MCP：`DEV_MCP_SERVERS` 非空才加载（读 `.pi/mcp.json`）；MCP 工具是 coroutine-only，
      所以必须走 `ainvoke`，不能退回 `invoke`。
    - MCP 加载失败只注记、不阻断出计划（技能/工具的失败不得让 plan 阶段挂掉）。
    """
    from deepagents import create_deep_agent
    from langchain_openai import ChatOpenAI

    from . import harness

    api_key = os.environ.get("LLM_API_KEY", "").strip()
    if not api_key:
        raise RuntimeError("LLM_API_KEY 未配置")
    model = ChatOpenAI(
        model=os.environ.get("LLM_MODEL") or "deepseek-flash",
        base_url=(os.environ.get("LLM_BASE_URL") or "https://api.deepseek.com").rstrip("/"),
        api_key=api_key,
        temperature=0.2,
        timeout=timeout,
    )
    notes: list[str] = []
    tools: list[object] = []
    try:
        skill_names, skill_warnings = harness.describe_skills()
        if skill_warnings:
            notes.append("技能加载告警：" + "；".join(skill_warnings))
        elif not skill_names:
            notes.append("未加载到任何技能（检查 .pi/skills）")
    except Exception as e:  # noqa: BLE001 - 技能不可用也要出计划
        notes.append(f"技能清单读取失败（{type(e).__name__}: {e}）")
    try:
        tools = await harness.aload_mcp_tools()
    except Exception as e:  # noqa: BLE001 - MCP 不可用必须降级
        notes.append(f"MCP 工具未加载（{type(e).__name__}: {e}）")
    agent = create_deep_agent(
        model=model,
        tools=tools,  # type: ignore[arg-type]
        system_prompt=SYSTEM_PROMPT,
        name="dev-planner",
        **harness.build_agent_kwargs(),
    )
    result = await agent.ainvoke(
        {"messages": [{"role": "user", "content": f"开发需求：\n{requirement.strip()}"}]},
        {"recursion_limit": RECURSION_LIMIT},
    )
    msgs = result.get("messages") or []
    text = ""
    for msg in reversed(msgs):
        content = getattr(msg, "content", None)
        candidate = content if isinstance(content, str) else json.dumps(content, ensure_ascii=False)
        if candidate and candidate.strip():
            text = candidate
            break
    if not text:
        raise RuntimeError("DeepAgents 未返回内容")
    if notes:
        text += "\n\n> harness 注记：" + "；".join(notes) + "\n"
    return text


def deepagents_plan(requirement: str, timeout: float = 120.0) -> str:
    """`adeepagents_plan` 的同步封装（可在事件循环内调用）。"""
    return harness_run(lambda: adeepagents_plan(requirement, timeout))


def harness_run(factory):
    """延迟 import harness，避免 `planner` 在未装 `agents` extra 时导入失败。"""
    from .harness import run_coro_sync

    return run_coro_sync(factory)


def make_plan(planner: PlannerName, requirement: str) -> tuple[str, str, str]:
    """返回 (plan_markdown, plan_format, planner_used)。失败时抛错，由调用方决定降级。"""
    if planner == "stub":
        return normalize_plan(stub_plan(requirement), requirement), PLAN_FORMAT, "stub"
    if planner == "llm":
        return normalize_plan(llm_plan(requirement), requirement), PLAN_FORMAT, "llm"
    if planner == "deepagents":
        return normalize_plan(deepagents_plan(requirement), requirement), PLAN_FORMAT, "deepagents"
    raise ValueError(f"未知 planner: {planner}")


def plan_with_fallback(planner: PlannerName | str, requirement: str) -> tuple[str, str, str]:
    """生成计划；**任何失败降级 stub** 并在计划尾注记原因（图节点与 CLI 共用同一条路径）。"""
    try:
        return make_plan(planner, requirement)  # type: ignore[arg-type]
    except Exception as e:  # noqa: BLE001 - 生成失败必须可降级
        md, fmt, used = make_plan("stub", requirement)
        md += chr(10) + f"> 计划生成降级：planner={planner} 失败（{e}）" + chr(10)
        return md, fmt, used
