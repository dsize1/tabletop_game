# dev-task（开发任务引擎，MCP 形态）

- **语言**：Rust
- **位置**：`crates/dev-task`（核心状态机）+ `crates/dev-task-mcp`（MCP stdio 暴露层）
- **配套**：`apps/agent-gateway`（子会话编排 + 消息桥）+ `apps/agent-gateway/src/dev-task-client.ts`（MCP 客户端）

## 职责

- `dev-task`（core）：开发任务状态机 + 白名单校验 + SQLite 持久化（`dev_tasks` 表）
- `dev-task-mcp`：把状态机映射为 MCP 统一原语，stdio 暴露给 agent-gateway

## 与 game-engine 的同构与差异

| 维度 | game-engine | dev-task |
|---|---|---|
| 决策模型 | SystemOne（只建议） | **人**（固定前缀命令，置信度 100%） |
| 规则 | 游戏规则 + 合法动作集 | 计划 + 计划工作流 |
| 权限 | 回合/资源归属 | 开发者白名单（`member_openid`）+ 任务归属 |
| 校验链 | 合法性 + 权限 + 执行 + 事件溯源 | 白名单 + 归属 + 确定性状态迁移表 |

**子会话只做「内容」**（规划/执行/写计划文档），任务状态机由「网关（路由人工命令）+ Rust 引擎（权威状态+校验）」驱动，子会话不能自行改状态。

## 状态机

```
queued → planning → awaiting_confirm → running → verifying → done
   └──────────────┬──────────────┬───────────────┘
                failed        cancelled        failed
```

- 迁移表确定性校验（`dev-task/src/lib.rs::is_valid_transition`）；任意非终态可进入 `failed`/`cancelled`
- `confirm_task`（人工确认）：`awaiting_confirm → running`（确认计划）或 `verifying → done`（验收通过）

## MCP 统一原语

### 只读

| 工具 | 说明 |
|---|---|
| `list_tasks(status?)` | 列出所有开发任务（可按状态过滤），返回 session_id/status/title/group_id/qq_id |
| `get_task(session_id)` | 单个任务详情 |
| `get_progress(session_id)` | 阶段 + 待办/验证完成度（读计划文档复选框） |
| `list_capabilities()` | 本服务全部 MCP 能力（能力名/简述/使用方式/读写） |

### 写入（受校验）

| 工具 | 说明 | 校验 |
|---|---|---|
| `create_task(session_id, group_id, qq_id)` | 创建任务，初始 queued | 白名单 |
| `confirm_task(session_id, qq_id)` | 人工确认计划/验收 | 白名单 + 归属 + 状态 |
| `cancel_task(session_id, qq_id)` | 人工取消 | 白名单 + 归属 |
| `transition_task(session_id, status)` | 内容里程碑迁移（网关驱动） | 确定性迁移表 |
| `update_task_meta(session_id, title?, plan_path?)` | 回填标题/计划路径 | — |

读写属性标注在各 tool 的 `description` 前缀（`[读]`/`[写]`），供 `list_capabilities` 展示。

## 统一键 sessionId

- `sessionId` = pi 子会话 id = 计划文档 id（`memory_palace/plan/{sessionId}.md`）= 后期 web 页标识
- 由 agent-gateway 生成（uuid），经 `SessionManager.create(..., { id: sessionId })` 落定到 pi 子会话；plan-memory 扩展据此写计划文档
- 受理回执（被动回复）即携带 sessionId

## 环境变量

- `SQLITE_PATH`：SQLite 路径（默认 `data/tabletop.db`）
- `DEV_WHITELIST`：开发者 `member_openid` 逗号分隔（空白名单 = 全部拒绝）
- 经 agent-gateway 的 MCP 客户端 `env` 显式传递（MCP SDK 默认只继承白名单环境变量）

## 约定

- 命令固定前缀：`#任务 [状态]` / `#进度 <id>` / `#确认 <id>` / `#取消 <id>` / `#能力`（硬判断，先于决策模型）
- 被动回复：状态同步 + 引导人工推进（不展示计划内容）；主动推送：计划六分区正文（后期替换为 web 短链）
- 子会话模型：复用主持人 LLM 凭据（DeepSeek），thinking 开启（`thinkingLevel: medium`）
