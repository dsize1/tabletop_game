# dev-executor（LangGraph 执行器，shadow）

> 阶段：**B6 shadow**（ADR-0015）。只做「状态编排 + 人工中断恢复」的骨架，**不推进权威状态、不出站、不写 `tabletop.db`**。

## 边界（必须遵守）

| 允许 | 禁止 |
|---|---|
| 自己的 checkpoint：`data/langgraph/checkpoints.sqlite` | 直连/写 `data/tabletop.db`（唯一写者是 `crates/server`） |
| 读取/生成计划文本（shadow 阶段为确定性骨架） | 调 `qq.push` / 发群消息（B4/B5 才有出站） |
| `interrupt()` 等待人工裁决 | 自动批准/自动合并/自动部署 |

## 图

```
START → plan → await_approval ──reject/无批准──> END
                     └──approve──> execute → await_verify ──reject──> END
                                                     └──approve──> summarize → END
```

- `await_approval` / `await_verify` 用 `interrupt()`；恢复用 `Command(resume="approve"|"reject")`。
- **注意**：`interrupt()` 恢复会从该节点开头重跑 → 节点内不得有副作用（当前仅返回标记）。
- `thread_id = sessionId`（与 `dev_tasks.session_id` / 计划文档 id 同一键）。

## 运行

```bash
cd agents/dev_executor
uv sync                       # 首次（会生成 uv.lock 锁版本）
uv run dev-executor plan --requirement "给拉斯维加斯加一个小游戏" --planner llm --out /tmp/plan.md
uv run dev-executor plan --requirement "..." --planner deepagents      # 需 --extra agents
uv run dev-executor plan --requirement "..." --diff /tmp/other.md      # 结构/内容 diff

uv run dev-executor run --session-id s1 --requirement "给拉斯维加斯加一个小游戏"
uv run dev-executor resume --session-id s1 --decision approve
uv run dev-executor state --session-id s1
uv run dev-executor reconcile            # 对账：投影↔事件↔checkpoint（--fix 可修投影）
uv run dev-executor reconcile --json
```

- checkpoint 路径：`../../data/langgraph/checkpoints.sqlite`（可用 `DEV_CHECKPOINT_PATH` 覆盖；目录自动创建）。
- 计划生成（B6.2）：`DEV_PLANNER=stub（默认）|llm|deepagents`
  - `stub`：确定性 6 分区骨架（离线/测试）
  - `llm`：裸 HTTP 调 OpenAI 兼容端点（`LLM_API_KEY` / `LLM_BASE_URL` / `LLM_MODEL`）
  - `deepagents`：DeepAgents harness（`uv sync --extra agents`），**StateBackend 内存 + 临时 cwd + recursion_limit=12**
  - 任一 planner 失败 → 自动降级 `stub` 并在计划尾注记原因

## 自测

```bash
uv run python tests/test_shadow.py
uv run python tests/test_planner.py
```

覆盖：interrupt → resume 两次到 `summary`；checkpoint 独立库存在；**`tabletop.db` 未被创建/修改**。
