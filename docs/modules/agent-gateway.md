# agent-gateway（主持人，pi-sdk）

- **语言**：Node.js（TypeScript）
- **位置**：`apps/agent-gateway`
- **依赖**：pi-sdk（`@earendil-works/pi-coding-agent`）

## 职责

- 消费 Redis Stream `internal`（bot-adapter 归一化后的内部事件）
- 以 **pi-agent 会话**充当主持人：编排「意图 → 决策 → 执行 → 叙述」
- 会话管理：每群一个共享聊天会话；开发任务用子会话
- **MCP client**（stdio）：连接 game-engine MCP server
- **MCP client**（stdio）：连接 dev-task MCP server（`dev-task-client.ts`）
- **子会话管理**（pi-sdk）：`dev-session.ts` 创建/恢复开发任务子会话（`SessionManager.create/open` + `prompt`/`dispose`）
- **decision-service client**（HTTP）：调用 `/v1/systemone`
- **auth client**（HTTP）：`auth-client.ts` 调 server `/auth/issue` 签发短期 token，拼计划文档短链
- 置信度阈值配置化 + 兜底
- 出站：写 `qq.push`

## 关键行为

### 会话与队列

- 每群一个共享聊天会话；同一会话/对局的消息**串行**处理（per-session 队列），避免 pi-sdk 会话并发 prompt
- 开发任务：独立子会话，与聊天会话隔离

### 开发任务（dev）

- **显式命令硬判断**（先于决策模型，`dev-command.ts`）：`#任务 [状态]` / `#进度 <id>` / `#确认 <id>` / `#取消 <id>` / `#能力` / `#发起 <需求>`（`#开发` 别名）/ `#补充 <id> <正文>`。**禁止用正则/硬编码对自由文本判断意图**。
- dev 意图：白名单校验 → `create_task` → 受理回执（含 sessionId）→ 异步 `spawn` 子会话规划；**仅显式 `#发起` 才建任务**，决策引擎判为 dev 但无显式命令时只提示，不自动建任务。
- 子会话（pi-sdk）跑 plan-workflow：生成计划 → `update_task_meta` 回填标题 → `transition awaiting_confirm` → 签发短期 token（server `/auth/issue`）→ 主动推送计划文档短链
- `#确认` → `confirm_task` → 恢复子会话执行 → `transition verifying` → `#确认` 再验收 → `done`
- `#进度`：经 `prepare_task_link` 校验归属后**重签新短链**（原 token 过期可自助重取）
- `#补充 <id> <正文>`：经 `append_instruction` 校验后 `runDevResume` 追加到已有子会话（不落 chat 兜底）
- 聊天多轮：每群 Redis 热缓存最近 20 条（`CHAT_HISTORY_LIMIT` 可配）注入 `chatReply`

### 编排流程

见 `docs/architecture.md` §6。

### 意图决策（SystemOne）

- `src/systemone.ts`：`SystemOneClient`，POST `${SYSTEMONE_URL}/v1/systemone`（默认 `http://127.0.0.1:8090`），超时 `SYSTEMONE_TIMEOUT_MS`（默认 3000ms）；`fetchImpl` 可注入便于单测
- `src/decision.ts`：`decideIntent(content, client, { threshold })` **异步**；`intent ∈ {chat, start_game, game_action, dev}`，criteria 中文（实测零样本可用）
- 状态：目前 `state = { message }`；架构 §6.1 的 `get_active_game`/`get_legal_actions` 与动作选择**尚未接入**（待拉斯维加斯引擎）

### 置信度兜底

- `conf(intent)` 决定走哪条支路；`conf(action)` 决定是否自动执行
- **双通道**：决策模型为主 → 低于 `DECISION_INTENT_THRESHOLD`（默认 0.6）→ `#前缀` → 默认 `chat`
- **服务不可用**（超时/连不上/非 2xx）：`via="unavailable"` → 回「服务繁忙，请稍后重试」，不降级为 chat（避免服务故障时乱聊）
- 无效消息至少回一条

### 决策层配置

| 变量 | 默认 | 说明 |
|---|---|---|
| `SYSTEMONE_URL` | `http://127.0.0.1:8090` | 决策服务根地址（部署见 `docs/modules/decision-service.md`） |
| `SYSTEMONE_TIMEOUT_MS` | `3000` | 单次决策超时 |
| `DECISION_INTENT_THRESHOLD` | `0.6` | 意图置信度阈值 |

- 测试：`npm run test`（离线，含决策层 + 客户端层）；`npm run test:decision:live`（需本地决策服务在线）；健康探针 `node apps/scripts/check-decision-service.mjs`

### web 短链配置

| 变量 | 默认 | 说明 |
|---|---|---|
| `SERVER_URL` | `http://127.0.0.1:3000` | server 内部 HTTP（auth 签发） |
| `PUBLIC_BASE_URL` | 空 | web 公网基址（cloudflared 域名）；留空则计划推送退回长文 |

### 出站与回复

- 写 `qq.push`（遵循 event_id / MAXLEN 约定）
- 回复策略分层，见 `docs/architecture.md` §8
- 游戏交互：默认回规则模板摘要（承上启下，`msg_seq=1`）；`GAME_NARRATION_ENABLED=1` 时追加主持人 LLM 情景叙述（`msg_seq=2`），失败/超时静默降级为仅模板
- 开发任务：被动回执/状态同步（`pushReply`），计划/结果异步主动推送（`pushActive`，无 reply_msg_id）；计划推送用「标题 + 短链」替换 >1500 字长文（未配置 `PUBLIC_BASE_URL` 时退回长文）
- MCP 环境变量显式传递：MCP SDK 默认只继承白名单环境变量，`SQLITE_PATH`/`DEV_WHITELIST` 需在 `env` 显式传入

## 依赖边界

- 主持人 LLM：远程 API（provider 可配，型号待定）
- decision-service：本地 HTTP `/v1/systemone`
- game-engine：MCP stdio
