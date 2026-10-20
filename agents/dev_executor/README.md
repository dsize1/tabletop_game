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
uv sync --extra agents --extra llm   # 跑 llm/deepagents planner 所需（两个 extra 要一起装，否则会被 prune）
uv run dev-executor plan --requirement "给拉斯维加斯加一个小游戏" --planner llm --out /tmp/plan.md
uv run dev-executor plan --requirement "..." --planner deepagents      # 需 --extra agents
uv run dev-executor plan --requirement "..." --diff /tmp/other.md      # 结构/内容 diff
uv run dev-executor plan --requirement "x" --print-skills              # 附加输出已加载技能名/告警

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
  - `deepagents`：DeepAgents harness（`uv sync --extra agents --extra llm`），见下节
  - 任一 planner 失败 → 自动降级 `stub` 并在计划尾注记原因（图节点与 CLI 走同一条降级路径）

## DeepAgents：技能（`.pi/skills`）与 MCP

`deepagents` planner 通过 `src/dev_executor/harness.py` 接入项目技能与 MCP（细节见模块 docstring）：

| 能力 | 接法 | 边界 |
|---|---|---|
| 技能 | `backend = SkillsBackend`（根 = `<repo>/.pi/skills`，`virtual_mode`）+ `skills=[("/", "项目技能")]` | **只读**（`write/edit/delete/upload` 直接返回错误）+ `FilesystemPermission(write, /**, deny)` 兜底；仓库其余文件不可见 |
| MCP | `langchain-mcp-adapters` 读 `.pi/mcp.json`（与 pi 同一份配置） | `DEV_MCP_SERVERS=npx 名` 显式开启，默认关闭；加载失败只注记不阻断 |

- 技能是**渐进披露**：系统提示只含 `name/description`，模型需要时自己 `read_file("/<skill>/SKILL.md")`。
- 排查：`--print-skills` 列出已加载技能名与告警；技能告警/MCP 失败也会写进计划尾的「harness 注记」。
- **MCP 工具只有 coroutine**，所以 planner 用 `ainvoke`；`recursion_limit=20`（中间件 hook 也计入步数）。
- 示例：`DEV_MCP_SERVERS=playwright DEV_PLANNER=deepagents uv run dev-executor plan --requirement ...`
  （playwright 走 `npx`，首次需联网下载；planner 默认不挂浏览器，验证类子代理才需要）。
- 不用 `CompositeBackend` 挂 `/skills` 的原因：deepagents 0.7.28 的 `CompositeBackend.ls` 路由前缀重映射错一位（`/skills/x` → `/skill/x`），会导致技能取不到 SKILL.md。

## 自测

```bash
uv run python tests/test_shadow.py
uv run python tests/test_planner.py
uv run python tests/test_harness.py        # 技能只读挂载 + MCP 配置/工具发现（离线假 MCP）
uv run python tests/test_agent_wiring.py   # 假模型：技能进系统提示 / 技能可读 / 写被拒
```

覆盖：interrupt → resume 两次到 `summary`；checkpoint 独立库存在；**`tabletop.db` 未被创建/修改**；
4 个技能全部加载且无告警、技能文件未被改写、MCP 默认关闭且可按 `.pi/mcp.json` 开启。
