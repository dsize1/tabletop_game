# dev-task（开发任务：桥 + 执行器 + 状态）

> 2026-10-20 重构（ADR-0015 B1–B8）：**Rust `dev-task`/`dev-task-mcp` 已删除**；开发任务改为
> 「**MCP 原语（薄桥）** + **LangGraph 执行器** + **server 状态/审计**」。

## 组成

| 层 | 位置 | 职责 |
|---|---|---|
| 入口 | `apps/agent-gateway` | 显式命令识别（`#发起/#确认/#取消/#进度/#补充/#任务/#能力`）、回执、出站通知（轮询事件 → `qq.push`） |
| 原语（桥） | `apps/dev-bridge`（Node，MCP stdio） | 对外契约；**不直连 SQLite**，全走 server；15+ 原语 |
| 状态/审计 | `crates/server`（`/internal/dev/*`） | `dev_tasks`（投影）、`dev_task_events`、`dev_runs`、`dev_artifacts`；迁移表校验 + 白名单 |
| 执行 | `agents/dev_executor`（Python + LangGraph） | 图 `plan → interrupt(approve) → execute → interrupt(verify) → summarize`；checkpoint 独立库 |
| 驱动（未来） | n8n（`dev-shadow`，当前只读） | 触发/审批 UI/CI/部署（B9） |

```
QQ → gateway（入口）→ 桥（原语）→ server（投影/审计/校验）
                        ↓ spawn
                 dev-executor work --stage plan|execute|verify
                        ↓
        LangGraph checkpoint（唯一状态权威） → 事件 → gateway 通知 → 群
```

## 原语（契约）

- 旧（11）：`list_tasks` / `get_task` / `get_progress` / `prepare_task_link` / `append_instruction` / `list_capabilities` / `create_task` / `confirm_task` / `cancel_task` / `transition_task` / `update_task_meta`
- v2：`approve_plan` / `approve_verify` / `list_events` / `check_permission` / `start_run` / `finish_run` / `cancel_run`
- 约定不变：`description` 带 `[读]/[写]`；失败文本 `错误: ...`；能力描述符 11 项（`#能力` 渲染）；**禁止用正则猜自由文本意图**。

## 状态与数据

- `dev_tasks.status`：`queued → planning → awaiting_confirm → running → verifying → done`（+ `failed`/`cancelled`），**投影**（`sync_source=langgraph`，`executor_ref=run_id`）。
- `stage`：`plan|execute|verify|ci|deploy`（区分「待确认计划」与「待验收」）。
- `dev_task_events`（append-only）：`created/status_changed/run_started/plan_ready/approved/executed/verified/run_summary/failed/cancelled`；出站通知与对账的唯一输入。
- `dev_runs`：单活跃（部分唯一索引；重复 start → 409）。
- 计划文档：`memory_palace/plan/{sessionId}.md`（`#进度` 与短链读取同一份；6 分区 `plan6-v1`）。

## 执行器命令

```bash
cd agents/dev_executor
uv run dev-executor work --session-id <sid> --stage plan --requirement "<需求>"
uv run dev-executor work --session-id <sid> --stage execute   # resume 批准，跑到验收中断
uv run dev-executor work --session-id <sid> --stage verify    # resume 批准，收尾
uv run dev-executor work --session-id <sid> --stage append --instruction "补充要求"
uv run dev-executor reconcile [--fix] [--json]                # 投影↔事件↔checkpoint 对账
```

- 计划生成：`DEV_PLANNER=stub|llm|deepagents`；失败自动降级 stub。
- 执行阶段目前 **shadow**（不改代码）；真实执行（worktree/沙箱）待后续。

## 配置

| 变量 | 作用 |
|---|---|
| `DEV_EXEC_MODE` | `langgraph`（默认）/ `entry`（只入口不执行） |
| `DEV_BRIDGE_ENTRY` | 指向 `dev-bridge/src/main.ts`（网关用桥；不设则连旧 Rust MCP，已无） |
| `DEV_WHITELIST` | 白名单判定在 **server**；dev-up 从 `apps/agent-gateway/.env` 传给 server |
| `DEV_NOTIFY_ENABLED` / `DEV_NOTIFY_INTERVAL_MS` | 事件→出站通知开关/间隔（游标 `dev:notify:cursor:{sid}` 去重） |
| `DEV_PLANNER` / `LLM_*` | 计划生成后端与模型 |
| `DEV_CHECKPOINT_PATH` | checkpoint 覆盖（默认 `data/langgraph/checkpoints.sqlite`） |

## 回滚

- 停执行器：`DEV_EXEC_MODE=entry`（只入口，不推进）。
- 回旧实现不可能（Rust 引擎已删）；如需回退走 git 历史 + 重新引入 crate。

## 已知边界

- `#补充`：执行器 `work --stage append --instruction` 写入 checkpoint（`instructions` 累加）；任务在 `awaiting_confirm` 时会**自动重新规划**（计划含「补充指令」）。
- 终态事件已收敛：`verified` 由 server 确认写入；执行器 verify 阶段不再重复写 `run_summary`（历史事件保留兼容）。
- `dev_tasks` 是投影，**不得**作为判定依据；判定走 server 迁移表 + checkpoint。
