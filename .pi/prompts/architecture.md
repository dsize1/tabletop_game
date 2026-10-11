---
description: 加载桌游助手项目架构上下文（模块、消息链路、技术栈）
---

# 架构上下文

先读 `docs/architecture.md` 了解整体架构与消息链路。

关键约定：

- QQ 群事件 → `apps/qq-gateway`(Node) → Redis Stream `qq.raw` → `crates/bot-adapter`(Rust)：归一化 + 过滤（只 @文本，非文本忽略）→ 写 `internal`
- `apps/agent-gateway`(Node, pi-sdk) 消费 `internal`，为**唯一编排者/主持人**
- agent-gateway 三路：MCP(stdio) → game-engine（规则引擎校验/状态机/事件溯源）；HTTP → decision-service（`/v1/systemone`，StartLux-Decision-4B）；出站写 `qq.push`
- 双模型：主持人 LLM（远程 API，型号待定）+ 决策模型 SystemOne（本地 StartLux-Decision-4B Q8_0 GGUF）
- 决策模型只建议不写状态；动作经规则引擎无条件校验（合法性 + 权限）；权限确定性计算，不用 LLM
- 意图 + 动作合并为一次决策调用（`/v1/systemone` 单次前向多问题）
- 分层回复：游戏交互被动回规则模板摘要（无 LLM 叙述）；聊天 LLM 限时 + 超时回执补发；开发任务回执 + 异步推送
- 事件溯源：`game_events` append-only + `game_state` 快照；会话标识三类（聊天=群、开发=子会话、游戏=room）
- 持久化约定：Redis Streams 只做传输缓冲，SQLite 为真相源（ACK 在写库后、event_id 去重、MAXLEN 裁剪），见 `docs/conventions/persistence.md`
- Web（Express）→ HTTP 调用 server 的 `auth` 与资源查询 API（短期 token）
