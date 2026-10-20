"""planner 自测（离线）：归一化 / stub / 参数校验 / 缺 key 报错。"""

from __future__ import annotations

import os

from dev_executor.planner import SECTIONS, make_plan, normalize_plan, llm_plan, stub_plan


def test_stub_has_all_sections() -> None:
    md, fmt, used = make_plan("stub", "加个小游戏")
    assert used == "stub" and fmt == "plan6-v1"
    for s in SECTIONS:
        assert f"## {s}" in md, s
    # 分区顺序固定
    positions = [md.index(f"## {s}") for s in SECTIONS]
    assert positions == sorted(positions)


def test_normalize_fills_missing_and_orders() -> None:
    raw = "# 标题\n\n## 验证事项\n- [ ] 跑测试\n\n## 计划\n- 做点事\n"
    md = normalize_plan(raw, "需求X")
    for s in SECTIONS:
        assert f"## {s}" in md
    positions = [md.index(f"## {s}") for s in SECTIONS]
    assert positions == sorted(positions)
    # 保留已有内容；缺失分区补占位
    assert "- 做点事" in md and "- [ ] 跑测试" in md
    assert md.count("（待补充）") >= 1


def test_normalize_strips_code_fence() -> None:
    raw = "```markdown\n## 计划\n- x\n```\n"
    md = normalize_plan(raw, "r")
    assert "```" not in md
    assert "## 计划" in md


def test_unknown_planner_rejected() -> None:
    try:
        make_plan("nope", "x")  # type: ignore[arg-type]
    except ValueError as e:
        assert "未知 planner" in str(e)
    else:  # pragma: no cover
        raise AssertionError("应拒绝未知 planner")


def test_llm_requires_key() -> None:
    saved = os.environ.pop("LLM_API_KEY", None)
    try:
        try:
            llm_plan("x")
        except RuntimeError as e:
            assert "LLM_API_KEY" in str(e)
        else:  # pragma: no cover
            raise AssertionError("缺 key 应报错")
    finally:
        if saved is not None:
            os.environ["LLM_API_KEY"] = saved


def test_stub_plan_deterministic() -> None:
    assert stub_plan("a") == stub_plan("a")
    assert stub_plan("a") != stub_plan("b")


def main() -> None:
    test_stub_has_all_sections()
    test_normalize_fills_missing_and_orders()
    test_normalize_strips_code_fence()
    test_unknown_planner_rejected()
    test_llm_requires_key()
    test_stub_plan_deterministic()
    print("OK test_planner 通过")


if __name__ == "__main__":
    main()
