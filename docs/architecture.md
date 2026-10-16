# 架构总览

> 本文是架构详情，配套 `.pi/prompts/architecture.md`（rule）与 `AGENTS.md`（索引）。

## 1. 目标场景

- QQ **群**机器人（官方 bot.q.qq.com API v2 群事件能力）
- Web **独立站点**，短期 token 鉴权
  - 单页整合：`/tabletop_game/{gameId}/player/{openid}?token=xxx`（同页展示全局资源与个人资源）

## 2. 技术栈

- Node.js (TypeScript)：`qq-gateway`、`agent-gateway`（pi-sdk 主持人）、`web`
- Rust + Axum（单进程多模块）：`bot-adapter`、`game-engine-core`/`engines`（含 MCP 暴露层）、`auth`、`push`、`server`
- Redis（Streams 总线 + 缓存）、SQLite（持久化真相源）
- 模型：主持人 LLM（远程 API，型号待定）+ 决策模型 SystemOne（本地 StartLux-Decision-4B，Q8_0 GGUF）

## 3. 双模型 + 规则引擎（三层分工）

| 层 | 实现 | 部署 | 职责 | 性质 |
|---|---|---|---|---|
| 主持人 | 远程 LLM（型号待定） | 远程 API | 叙述、闲聊、开发任务、编排 | 生成式 |
| 决策模型（SystemOne） | StartLux-Decision-4B Q8_0 | 本地 `/v1/systemone`（GGUF） | 意图分类 + 动作选择 | 概率（只建议） |
| 规则引擎 | game-engine（Rust） | 本地 | 合法性/权限校验、状态机、事件溯源 | 确定性 |

- **决策模型只建议，不写状态**：所有动作必须经规则引擎无条件校验（合法性 + 权限）后才执行。
- **权限/合法性是确定性规则**，由规则引擎计算，不用 LLM 判断。

### 3.1 LLM 能力矩阵（思考 / 联网）

> 三类 LLM 路径集中在 `apps/agent-gateway`。此处固定「思考开关」与「联网形式」，避免遗忘（思考曾被「被动回复 3s 超时」误判关闭，实为 5 分钟，见 `docs/conventions/external-constraints.md`、ADR-0002）。

| LLM 路径 | 实现 | 思考（thinking） | 联网形式 |
|---|---|---|---|
| **chat**（闲聊/摘要压缩） | `chat.ts` `callLlm`（裸 HTTP DeepSeek） | **默认开启**：`LLM_THINKING_ENABLED=1`（不传 `thinking`）；`=0` 显式关闭 | **需用户主动提供链接**：消息含 `https://` 时，SystemOne `noul`（是/否）判断是否抓取该链接，命中则朴素 `fetch` 抽正文注入（Plan B 单跳）。**无链接不联网、不支持自主搜索** |
| **game**（游戏情景叙述） | `chat.ts` `narrateGame`（复用 `callLlm`） | **默认开启**（同 `LLM_THINKING_ENABLED`） | **不支持**（纯文本润色，无链接/浏览器） |
| **dev-task**（开发子会话） | `dev-session.ts`（pi-sdk 会话） | **开启**：`thinkingLevel: "medium"`（对 `deepseek-v4-pro` 实际夹到 `high`） | **playwright MCP 由模型自行选择抓取**：pi MCP 扩展读 `.pi/mcp.json`，`bindExtensions` 后可用 25 个 `mcp__playwright__browser_*`（codemode 暴露）；另有 `bash` 可 `curl` |

- 配置项与触发细节见 `docs/modules/agent-gateway.md`（聊天联网）、`docs/modules/dev-task.md`（子会话工具）。
- 切换抓取源（如 chat 改用 playwright）时，同步更新本矩阵。

## 4. 模块拓扑

```
QQ 群 ──> qq-gateway(Node) ──> Redis Stream[qq.raw] ──> server(Rust)
                                                            │ bot-adapter：归一化 + 过滤（只 @消息、非文本忽略）
                                                            ▼
                                                Redis Stream[internal]
                                                            │
                                                            ▼
                                              agent-gateway(Node, pi-sdk)  ←─ 主持人
                                                 │  ├─ 会话队列（per-session 串行）
                                                 │  ├─ MCP(stdio) ──> game-engine(Rust, MCP server)
                                                 │  │                    └─ 规则引擎校验 + 状态机 + 事件溯源 ──> SQLite
                                                 │  ├─ HTTP ──> decision-service(/v1/systemone, StartLux-4B GGUF)
                                                 │  └─ 出站：写 Redis Stream[qq.push] ──> qq-gateway ──> QQ 群
                                                 ▼
                                            SQLite（真相源）

web(Express) ──HTTP──> server/auth + 资源查询 ──> SQLite / Redis
```

## 5. 消息流（核心链路）

1. QQ 群事件 → `qq-gateway` 写入 `qq.raw`
2. `bot-adapter` 订阅 `qq.raw`：**只保留 @机器人文本消息**（`GROUP_AT_MESSAGE_CREATE` + `message_type=0`），归一化为内部事件写入 `internal`
3. `agent-gateway` 消费 `internal`，按会话排队（per-session 串行）
4. 主持人编排（见 §6 决策流程）
5. 出站回复写入 `qq.push` → `qq-gateway` 调 QQ API 发群消息

## 6. 决策流程（agent 编排）

### 6.1 意图识别与动作选择合并为一次决策调用

- 主持人先**确定性**查 MCP：`get_active_game`（是否有活跃对局）
- 无对局：一次决策调用（单问）`intent ∈ {chat, dev, start_game}`
- 有对局：先查 `get_state` + `get_legal_actions`，再一次决策调用（双问）`intent ∈ {chat, dev, game_action}` + `action ∈ {合法动作}`
- 决策调用 = `/v1/systemone` **单次请求多问题**（单次前向），意图与动作针对同一份 state 快照
- **置信度三档**：`≥ 高阈值` 直接采用决策结果；`[低阈值, 高阈值)` 走「意图确认」（人工 y/n，y=按原消息执行、n=回落低阈值）；`< 低阈值` 走 `#前缀` → 默认 chat。阈值与状态机见 `docs/modules/agent-gateway.md`「置信度兜底（三档 + 意图确认）」

### 6.2 分支配

| intent | 处理 |
|---|---|
| chat | 主持人直接 LLM 叙述（见 §8 回复策略） |
| dev | 管理员白名单（`member_openid`）校验；游戏激活时丢弃并告知；子会话异步处理 |
| start_game | 主持人列出可用游戏，确定性开桌 |
| game_action | 见 §6.3 |

### 6.3 游戏动作路径

1. 决策返回 `action` + `confidence`；主持人按配置阈值兜底：低于阈值 → 澄清 / 回显合法动作，不执行
2. 阈值通过 → MCP `execute_action(action)` → 规则引擎**无条件**做合法性 + 权限校验
3. 无权限 / 不合法 → 返回错误原因 → 主持人回模板摘要（`msg_seq=1`）+ 可选 LLM 叙述给玩家
4. 合法 → 执行 + 事件溯源（写 events + 更新快照，同一事务）→ 返回结果 → 主持人先回规则模板摘要（承上启下，`msg_seq=1`），可选追加 LLM 情景叙述（`msg_seq=2`，配置开关，失败/超时静默降级）

## 7. 安全模型

- **决策模型只建议**：其输出仅作为"建议"，不视为已通过。
- **规则引擎最后防线**：即使 agent 直连 `execute_action`，引擎也必须重新校验合法性与权限（defense in depth）。
- **置信度兜底**：阈值配置化；无效消息至少给用户一条反馈。
- **事件溯源**：所有状态变更落 append-only events + 快照，可复盘。

## 8. 回复策略（分层）

> 被动回复有效期 **5 分钟**（群聊；单聊 60 分钟），每条消息最多回复 **5 次**（递增 `msg_seq`）；
> 主动消息有日配额：群 **1000 条/群/天**、未认证 30/qpm（企业/个人认证 60/qpm）、单关系 20/qpm、每秒最多 5 条。
> 以上数值约束的来源与验证状态见 `docs/conventions/external-constraints.md`。

| 消息类型 | 被动回复（5 分钟内必达） | 补充 |
|---|---|---|
| 游戏交互 | 规则引擎模板摘要（承上启下，<1s，`msg_seq=1`）+ 可选 LLM 情景叙述（`msg_seq=2`，配置开关，可静默降级） | LLM 只做承上启下/情景带入，不逐条复述结果（结果详情走 web 页） |
| 聊天 | LLM 叙述 | 超过 5 分钟才需主动补发 |
| 开发任务 | 受理回执（含 sessionId）+ 状态同步/引导（被动） | 计划/结果异步主动推送（后期换 web 短链） |

## 9. 数据存储

- **Redis**：Streams 传输缓冲（`qq.raw` / `internal` / `qq.push`）+ 热缓存
- **SQLite**：真相源。聊天/开发/游戏三类会话 + 游戏事件溯源，见 `docs/conventions/persistence.md`
- 分工与 ACK/幂等/裁剪见 `docs/conventions/persistence.md`、`docs/conventions/message-protocol.md`

## 10. 鉴权

- `auth` 签发/校验短期 token：uuid v4 + 绑定 openid + 资源范围，存 Redis，TTL 8 小时（可配 `AUTH_TOKEN_TTL`），局终态清除
- token 绑定玩家身份 = `member_openid`（openid，非 QQ 号）；个人资源归属一致（越权 403）
- Web 不直连 SQLite，统一经 server 资源查询 API
- 开发任务走管理员白名单（`member_openid`）

## 11. 扩展

- 新增桌游引擎：`crates/engines/<name>/` 实现 EnginePlugin（映射到 MCP 统一原语），并在 server 注册
- 统一原语见 `docs/modules/game-engine.md`
- 开发任务：`crates/dev-task` + `crates/dev-task-mcp`（MCP 原语，状态机 + 白名单校验）；子会话编排/消息桥见 `docs/modules/dev-task.md`
- 开发文档预览：`crates/docs-mcp`（只读 MCP 原语）+ `config/dev-docs.json` + server `/resources/docs/*` + web `/dev_docs`；见 `docs/modules/docs-mcp.md`
