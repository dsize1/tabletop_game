"""agent 装配自测（离线，用假模型）：技能进系统提示 / 技能可读 / 写操作被拒。

不依赖真实 LLM：`GenericFakeChatModel` 按脚本回放消息即可验证 harness 与
`create_deep_agent` 的接线，以及只读 backend 在工具调用层的实际效果。
"""

from __future__ import annotations

import asyncio
from typing import Any, ClassVar

from langchain_core.language_models.fake_chat_models import GenericFakeChatModel
from langchain_core.messages import AIMessage, ToolMessage
from langchain_core.outputs import ChatGeneration, ChatResult

from dev_executor import harness

PLAN = (
    "## 计划\n- x\n"
    "## 待办实现\n- [ ] y\n"
    "## 待确定事项\n- z\n"
    "## act 阶段变动的文件列表\n- w\n"
    "## 验证事项\n- [ ] v\n"
    "## harness 优化项\n- h\n"
)

CAPTURED: list[list[Any]] = []


class ScriptedModel(GenericFakeChatModel):
    """按脚本回放消息（超出脚本重复最后一条），并记录每次模型输入。"""

    script: ClassVar[list[AIMessage]] = []
    idx: ClassVar[int] = 0

    def bind_tools(self, tools: Any, **kwargs: Any) -> Any:  # noqa: ANN401 - 假模型直接返回自身
        return self

    def _generate(self, messages, stop=None, run_manager=None, **kwargs):  # noqa: ANN001, ANN201
        CAPTURED.append(list(messages))
        i = ScriptedModel.idx
        ScriptedModel.idx += 1
        last = ScriptedModel.script[-1]
        return ChatResult(generations=[ChatGeneration(message=ScriptedModel.script[min(i, len(ScriptedModel.script) - 1)] if last else AIMessage(content=""))])


def _run(script: list[AIMessage]) -> dict[str, Any]:
    CAPTURED.clear()
    ScriptedModel.script = script
    ScriptedModel.idx = 0
    model = ScriptedModel(messages=iter([AIMessage(content="")]))
    agent = create_agent_for(model)
    return asyncio.run(
        agent.ainvoke({"messages": [{"role": "user", "content": "需求：给拉斯维加斯加个小游戏"}]}, {"recursion_limit": 20})
    )


def create_agent_for(model: Any):  # noqa: ANN201
    from deepagents import create_deep_agent

    return create_deep_agent(
        model=model,
        tools=[],
        system_prompt="planner",
        name="dev-planner",
        **harness.build_agent_kwargs(),
    )


def test_skills_in_system_prompt() -> None:
    _run([AIMessage(content=PLAN)])
    system_text = str(CAPTURED[0][0].content)
    for name in ("game-engine-dev", "plan-workflow", "qq-bot-dev", "rust-axum"):
        assert name in system_text, f"技能清单缺 {name}"


def test_skill_is_readable_via_tool() -> None:
    call = AIMessage(
        content="",
        tool_calls=[
            {
                "name": "read_file",
                "args": {"file_path": "/rust-axum/SKILL.md", "limit": 20},
                "id": "read1",
            }
        ],
    )
    result = _run([call, AIMessage(content=PLAN)])
    tool_msgs = [m for m in result["messages"] if isinstance(m, ToolMessage)]
    assert tool_msgs, "read_file 未产生工具结果"
    assert "Axum" in str(tool_msgs[0].content), str(tool_msgs[0].content)[:200]


def test_write_tool_is_denied() -> None:
    call = AIMessage(
        content="",
        tool_calls=[
            {
                "name": "write_file",
                "args": {"file_path": "/rust-axum/SKILL.md", "content": "覆盖尝试"},
                "id": "write1",
            }
        ],
    )
    before = (harness.skills_dir() / "rust-axum" / "SKILL.md").read_text(encoding="utf-8")
    result = _run([call, AIMessage(content=PLAN)])
    tool_msgs = [m for m in result["messages"] if isinstance(m, ToolMessage)]
    assert tool_msgs, "write_file 未产生工具结果"
    text = str(tool_msgs[0].content)
    assert "只读" in text or "permission denied" in text.lower(), text[:200]
    after = (harness.skills_dir() / "rust-axum" / "SKILL.md").read_text(encoding="utf-8")
    assert before == after, "技能文件被改写"


def main() -> None:
    test_skills_in_system_prompt()
    test_skill_is_readable_via_tool()
    test_write_tool_is_denied()
    print("OK test_agent_wiring 通过")


if __name__ == "__main__":
    main()
