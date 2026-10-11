# 里程碑记录（开发进度）

> 记录项目关键节点与验证结果，按时间倒序。重大技术决策见 `decisions/`。

## M3：架构重设计（双模型 + MCP 化 + 事件溯源，文档）✅

- **日期**：2026-10-11
- **完成**（仅文档，无业务代码）：
  - 引入双模型：主持人 LLM（远程 API，型号待定）+ 决策模型 SystemOne（StartLux-Decision-4B Q8_0 GGUF）
  - agent-gateway（Node, pi-sdk）为唯一编排者；command-router 职责并入
  - game-engine 以 MCP 形态暴露统一原语（stdio）；决策模型只建议不写状态
  - 权限/合法性由规则引擎确定性校验；置信度阈值配置化 + 兜底
  - 意图 + 动作合并为一次决策调用（单次前向多问题）
  - 事件溯源：`game_events` + `game_state` 快照；会话标识三类拆分
  - 分层回复策略（游戏模板摘要被动 / 聊天限时 / 开发回执+异步）
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

- **M5：聊天增强** — 异步主动发送（LLM 回复不走被动窗口，统一主动消息）+ 多轮对话（每群会话历史）
- **M6：决策模型接入** — decision-service 部署（llama-server + gguf_server）→ SystemOne 替代 mock 关键词，解决「猜拳规则→开桌」等误判
- 之后：拉斯维加斯引擎 → 规则查询意图路径（mock 识别游戏交互 → MCP get_rules）→ 会话历史持久化
