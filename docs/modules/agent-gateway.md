# agent-gateway（主持人，pi-sdk）

- **语言**：Node.js（TypeScript）
- **位置**：`apps/agent-gateway`
- **依赖**：pi-sdk（`@earendil-works/pi-coding-agent`）

## 职责

- 消费 Redis Stream `internal`（bot-adapter 归一化后的内部事件）
- 以 **pi-agent 会话**充当主持人：编排「意图 → 决策 → 执行 → 叙述」
- 会话管理：每群一个共享聊天会话；开发任务用子会话
- **MCP client**（stdio）：连接 game-engine MCP server
- **decision-service client**（HTTP）：调用 `/v1/systemone`
- 置信度阈值配置化 + 兜底
- 出站：写 `qq.push`

## 关键行为

### 会话与队列

- 每群一个共享聊天会话；同一会话/对局的消息**串行**处理（per-session 队列），避免 pi-sdk 会话并发 prompt
- 开发任务：独立子会话，与聊天会话隔离

### 编排流程

见 `docs/architecture.md` §6。

### 置信度兜底

- `conf(intent)` 决定走哪条支路；`conf(action)` 决定是否自动执行
- 低于阈值 → 澄清 / 回显合法动作；无效消息至少回一条

### 出站与回复

- 写 `qq.push`（遵循 event_id / MAXLEN 约定）
- 回复策略分层，见 `docs/architecture.md` §8

## 依赖边界

- 主持人 LLM：远程 API（provider 可配，型号待定）
- decision-service：本地 HTTP `/v1/systemone`
- game-engine：MCP stdio
