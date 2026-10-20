"""计划生成：stub / llm / deepagents（B6.2）。

约束：
- **不出站、不写业务状态**；生成结果只进 checkpoint（B7 才投影到 server）。
- 所有 planner 都必须产出**6 分区** markdown（`plan6-v1`），否则 `#进度` 会归零。
- DeepAgents 使用 `StateBackend`（内存）+ 临时 cwd + 小 `recursion_limit`，避免写宿主文件。
"""

from __future__ import annotations

import json
import os
import tempfile
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

SYSTEM_PROMPT = """你是桌游助手项目的开发计划生成器。只输出 markdown 计划，不要调用任何工具，不要写文件。
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


def deepagents_plan(requirement: str, timeout: float = 120.0) -> str:
    """DeepAgents（StateBackend，内存；临时 cwd；小 recursion_limit）。"""
    from deepagents import create_deep_agent
    from deepagents.backends import StateBackend
    from langchain_openai import ChatOpenAI

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
    agent = create_deep_agent(
        model=model,
        tools=[],
        system_prompt=SYSTEM_PROMPT,
        backend=StateBackend(),
        name="dev-planner",
    )
    with tempfile.TemporaryDirectory(prefix="dev-exec-agent-") as tmp:
        cwd = os.getcwd()
        try:
            os.chdir(tmp)  # 双保险：即便有文件工具也不会写仓库
            result = agent.invoke(
                {"messages": [{"role": "user", "content": f"开发需求：\n{requirement.strip()}"}]},
                {"recursion_limit": 12},
            )
        finally:
            os.chdir(cwd)
    msgs = result.get("messages") or []
    for msg in reversed(msgs):
        content = getattr(msg, "content", None)
        text = content if isinstance(content, str) else json.dumps(content, ensure_ascii=False)
        if text and text.strip():
            return text
    raise RuntimeError("DeepAgents 未返回内容")


def make_plan(planner: PlannerName, requirement: str) -> tuple[str, str, str]:
    """返回 (plan_markdown, plan_format, planner_used)。失败时抛错，由调用方决定降级。"""
    if planner == "stub":
        return normalize_plan(stub_plan(requirement), requirement), PLAN_FORMAT, "stub"
    if planner == "llm":
        return normalize_plan(llm_plan(requirement), requirement), PLAN_FORMAT, "llm"
    if planner == "deepagents":
        return normalize_plan(deepagents_plan(requirement), requirement), PLAN_FORMAT, "deepagents"
    raise ValueError(f"未知 planner: {planner}")
