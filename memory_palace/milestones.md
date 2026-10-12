# 里程碑记录（开发进度）

> 记录项目关键节点与验证结果，按时间倒序。重大技术决策见 `decisions/`。

## M9：web 页呈现（代码链路 + cloudflared 公网可达打通）✅

- **日期**：2026-10-12
- **完成（代码侧）**：
  - `crates/auth`：短期 token 签发/校验/撤销（Redis 存储，uuid v4 + 绑定 openid + 资源范围，TTL 8h，局终态 `revoke_resource` 清空）；`cargo test -p auth` 8/8（含 Redis 集成：issue→verify / revoke / revoke_resource / 空 openid 拒绝）
  - `crates/server`：新增 `POST /auth/issue`、`POST /auth/verify`、`GET /resources/dev_task/{sessionId}/plan`（uuid 校验 + token 鉴权 + 资源范围匹配 → 401/403/404/400）、`GET /resources/game/{gameId}`（占位「暂未开放」）
  - `apps/web`：Express BFF（`/dev_task/{sessionId}?token=`、`/tabletop_game/{gameId}/player/{openid}` 占位、`/health`）+ markdown(GFM, marked) 渲染 + trust proxy
  - `apps/agent-gateway`：`auth-client.ts` 调 server 签发 token，拼短链替换 >1500 字计划长文推送（`PUBLIC_BASE_URL` 未配置退回长文）
  - 术语统一：openid（= member_openid，非 QQ 号）；路由/文档 `{qqid}`→`{openid}`
- **验证**：`cargo check --workspace` 通过；`cargo test -p auth` 8/8；server API 冒烟（issue→verify/401/403/400/占位）；web 本地 curl 冒烟（渲染/401/游戏占位）；agent-gateway `tsc` + `test:short-link` 通过
- **cloudflared（已完成）**：域名 `20221219.xyz` 切 Cloudflare NS；`cloudflared tunnel login/create/route dns` 本地管理隧道 `tabletop-web`（tunnel id `562666ab-...`）；Windows 服务常驻（`deploy/cloudflared/install-service.ps1`，ImagePath `tunnel --config ... run`）；公网 HTTPS `https://tabletopgame.20221219.xyz/health` → 200（本机大陆网络实测可达）
- **未做（后续）**：端到端 QQ 实测（#开发 → 计划生成 → 推短链 → 手机点开）；手机端大陆可达性复核
- **详见**：`plan/ad7f436a-1443-49ad-b229-220c44c17e80.md`

## M8：主动推送测试（push 单测 + qq.push 出站验证脚本）✅

- **日期**：2026-10-12
- **完成**：
  - `crates/push`：抽取 `ensure_event_id` 纯函数并补单测（event_id 自动补全 / 序列化 round-trip），`cargo test -p push` 3/3
  - `apps/verify-push.mjs`：注入一条「主动消息」指令（`reply_msg_id=null`）到 `qq.push`，观察 qq-gateway PEL/ACK 与消费者组信息，支持 `--no-cleanup` / `PUSH_TEST_GROUP`
  - `apps/qq-gateway`：`sendGroupMessage` 返回发送结果并在成功时记录 `msg_id`，便于验证主动/被动发送结果
- **验证**：`cargo test -p push` 3/3；`apps/qq-gateway` `tsc --noEmit` 通过；`verify-push.mjs` `node --check` 通过
- **未做（后续）**：真机向真实群发主动消息（需 Redis+QQ 在线+测试群 openid）；主动消息频控/预算记账（push.md 设计要求）；`at_member_id`(@提及) 出站使用、`PushEvent.msg_seq` 字段补齐

## M7：开发任务（多轮对话 + 子会话 plan-workflow 复用 + MCP 化）✅

- **日期**：2026-10-11
- **完成**：
  - `crates/dev-task`（core）：确定性状态机（queued→planning→awaiting_confirm→running→verifying→done/failed/cancelled）+ 白名单校验；`crates/dev-task-mcp`（stdio MCP）：9 个读写原语 + `list_capabilities`
  - `storage`：`dev_tasks` + `chat_history` 表落地
  - `agent-gateway`：`dev-task-client.ts`（MCP 客户端）、`dev-session.ts`（pi-sdk 子会话创建/恢复，复用 plan-workflow）、`dev-command.ts`（固定前缀命令硬判断）、消息桥（被动回执/状态同步 + 主动推送计划）
  - 聊天多轮：每群 Redis 热缓存最近 20 条（`CHAT_HISTORY_LIMIT` 可配）注入 `chatReply`
  - **关键修正**：MCP SDK `StdioClientTransport` 默认只继承白名单环境变量，`SQLITE_PATH`/`DEV_WHITELIST` 需显式 `env` 传入
- **验证**：Rust 单测 4/4；Node 命令解析单测；dev-task-mcp 集成验证；子会话 spawn 冒烟 + **计划工作流复用验证**（子会话生成 6 分区计划并落盘）
- **详见**：`plan/01a12ba0-c415-75b9-a5a5-1f74888e401d.md`
- **未做（后续）**：端到端 QQ 实测（需 Redis+LLM+QQ 在线）、web 页呈现（cloudflare tunnel）、聊天历史 SQLite 归档与 Redis 热缓存同步

## M6：决策模型接入（SystemOne 替代 mock 意图决策）✅（意图部分）

- **日期**：2026-10-11
- **完成**：
  - decision-service 部署（llama-server + gguf_server；一键启动 `apps/scripts/start-decision-service.mjs`）
  - agent-gateway 新增 `SystemOneClient`（HTTP `/v1/systemone`，超时可配/可注入）；`decideIntent` 改异步调真实模型，**mock 关键词彻底移除**
  - criteria 中文；双通道兜底：决策 → 低置信 `#前缀` → 默认 chat；服务不可用 → 「服务繁忙」
  - 配置：`SYSTEMONE_URL` / `SYSTEMONE_TIMEOUT_MS` / `DECISION_INTENT_THRESHOLD`
  - 单测改注入桩（离线）+ 真实服务集成验证脚本
  - 顺带修复既有前缀规则 bug（`#开发` 误判为 start_game）
- **验证**：`tsc` build 通过；`npm run test:decision` 通过；`npm run test:decision:live` 5/5；「这局规则是什么」→ chat(0.649)、「猜拳怎么玩」→ chat(0.822)，不再误判开桌
- **未做（后续）**：动作选择接入（架构 §6.1：先 `get_active_game`/`get_legal_actions` 再双问 `intent`+`action`）—— 留待拉斯维加斯引擎落地

## M3：架构重设计（双模型 + MCP 化 + 事件溯源，文档）✅

- **日期**：2026-10-11
- **完成**（仅文档，无业务代码）：
  - 引入双模型：主持人 LLM（远程 API，型号待定）+ 决策模型 SystemOne（StartLux-Decision-4B Q8_0 GGUF）
  - agent-gateway（Node, pi-sdk）为唯一编排者；command-router 职责并入
  - game-engine 以 MCP 形态暴露统一原语（stdio）；决策模型只建议不写状态
  - 权限/合法性由规则引擎确定性校验；置信度阈值配置化 + 兜底
  - 意图 + 动作合并为一次决策调用（单次前向多问题）
  - 事件溯源：`game_events` + `game_state` 快照；会话标识三类拆分
  - 分层回复策略（游戏模板摘要默认 + 可配置 LLM 情景叙述 / 聊天 LLM 叙述 / 开发回执+异步）
- **详见**：`decisions/ADR-0002-dual-model-mcp-eventsourcing.md`

## M2：qq-gateway 接入（群 → qq.raw 链路跑通）✅

- **日期**：2026-10-10
- **完成**：
  - 接入 QQ 官方 WebSocket 网关（官方 SDK `@tencent-connect/qqbot-nodejs`）
  - 接收 `GROUP_AT_MESSAGE_CREATE`（群 @消息）
  - 原始事件写入 Redis Stream `qq.raw`（`MAXLEN ~ 1000`，单字段 `event`）
- **验证**：群里 @机器人 → 日志 `→ qq.raw [GROUP_AT_MESSAGE_CREATE]`；`XRANGE qq.raw` 确认 XLEN=1
- **关键结论**（已补进 `docs/conventions/message-protocol.md`、`docs/modules/bot-adapter.md`）：
  - 玩家身份 = `member_openid`（**非 QQ 号**）
  - `content` 带前导空格，bot-adapter 需 trim
  - 被动回复用 `raw.id` 作为 `msg_id`
- **环境**：Docker Desktop（WSL2）+ Redis 7（镜像加速 `docker.m.daocloud.io`）

## M1：项目初始化（脚手架）✅

- **日期**：2026-10-08
- **完成**：目录结构、AGENTS.md 渐进披露、skills/rules、docs、Node/Rust 空骨架、拉斯维加斯空引擎
- **验收**：`npm install`、`cargo metadata`、`cargo check --workspace` 通过
- **详见**：`plan/01a11c33-43ef-7462-9d16-2ffb5822fe8a.md`

## 下一里程碑（待办）

- **M8：端到端实测** — 真机验证开发任务链路（受理→规划→确认→执行→进度查询）+ 聊天多轮
- **M9 残留**：端到端 QQ 实测（#开发 → 计划生成 → 推短链 → 手机点开）
- **M5 残留**：聊天历史 SQLite 归档与 Redis 热缓存同步；游戏路径 LLM 情景叙述（`GAME_NARRATION_ENABLED`）
- 之后：拉斯维加斯引擎 → 规则查询意图路径（mock 识别游戏交互 → MCP get_rules）
