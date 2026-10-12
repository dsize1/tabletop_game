# qq-gateway（QQ 群接入）

- **语言**：Node.js / TypeScript
- **位置**：`apps/qq-gateway`

## 职责

- 接入 QQ 官方群机器人（官方 Node SDK `@tencent-connect/qqbot-nodejs`）
- 接收群事件：
  - `GROUP_AT_MESSAGE_CREATE`：群 @机器人消息（默认）
  - `GROUP_MESSAGE_CREATE`：群内**全量消息**（需在平台开启「接收所有消息」功能，Intent `GROUP_AND_C2C_EVENT (1<<25)`）
- 原始事件写入 Redis Stream `qq.raw`
- 消费 Redis Stream `qq.push`，调用 QQ API 发送群消息

## 依赖

- Redis（Streams）
- QQ 官方 OpenAPI

## 约定

- 接入票据（AppID/AppSecret/Token）走环境变量
- 事件原样透传，不做业务解释
- 「接收所有消息」需在 QQ 开放平台开启，否则只能收到 @消息

## 接入方式（WebSocket）

- 采用 **WebSocket 模式**（官方推荐，Node SDK 默认）：qq-gateway **主动出站**连接 QQ 网关，**无需公网 IP**（Webhook 模式才需公网回调地址）
- 网关地址：调用「获取 WSS 接入点」接口（`/gateway`、`/gateway/bot`）→ `wss://api.bot.qq.com/websocket/`
- 鉴权：`Authorization: QQBot {AccessToken}`（由 AppID/AppSecret 换取，需自动刷新）
- 鉴权参数：`token`、`intents`（订阅事件范围）、`shard`（无需分片传 `[0, 1]`）
- 连接流程：`Hello` → `Identify`(鉴权) → `Ready` → 心跳 `Heartbeat`/`Heartbeat ACK` 保活 → 断线 `Resume`
- 事件订阅：群 @消息 / 全量群消息对应 Intent `GROUP_AND_C2C_EVENT (1<<25)`；订阅超出权限会连接报错
- OpenAPI 统一请求地址：`https://api.bot.qq.com`

## 消息发送限制（主动 / 被动）

> 来源：官方「消息收发概述」（bot.q.qq.com），数值可能调整，以官方为准。

### 消息类型

| 类型 | 特征 |
|---|---|
| 主动消息 | 无任何条件，机器人主动触达用户；用户在客户端关闭「允许主动发送」后发送失败 |
| 互动召回消息 | `is_wakeup=true`，用户主动对话后每周期可下发 1 条 |
| 被动消息（回复用户） | 携带 `msg_id`，对用户消息的回复 |
| 被动消息（响应事件） | 携带 `event_id`，对事件的回复 |

### 被动消息

| 场景 | 有效期 | 每条消息可回复次数 |
|---|---|---|
| 单聊 | 60 分钟 | 4 次 |
| 群聊 | 5 分钟 | 5 次 |
| 频道 | 5 分钟 | — |

### 主动消息频控（HTTP 接口）

群聊（本项目主场景）：

| 认证类型 | Bot 维度频控 | 单关系维度频控 | 每日上限 |
|---|---|---|---|
| 企业认证 | 60/qpm | 20/qpm | 1000 条/群 |
| 个人认证 | 60/qpm | 20/qpm | 1000 条/群 |
| 未认证 | 30/qpm | 20/qpm | 1000 条/群 |

单聊：

| 认证类型 | Bot 维度频控 | 单关系维度频控 | 每日上限 |
|---|---|---|---|
| 企业/个人认证 | 10/qps | 20/qpm | 1000 条/用户 |
| 未认证 | 5/qps 且 30/qpm | 20/qpm | 1000 条/用户 |

### 其他要点

- **消息去重**：相同 `msg_id` 可能多次推送，结合 `msg_seq` 去重；被动回复相同 `msg_id + msg_seq` 重复发送会失败，递增 `msg_seq` 可对同一消息多次回复。
- **撤回**：发送超过 2 分钟不可撤回。
- **在线**：发送消息接口要求机器人保持 WebSocket 在线。

## 实现状态（MVP）

已实现（`apps/qq-gateway/src/index.ts`）：

- 连接 WebSocket 网关（SDK 自动处理 token / 心跳 / 重连）
- 接收群 `GROUP_AT_MESSAGE_CREATE`（@消息）
- 原始事件写入 Redis Stream `qq.raw`（`MAXLEN ~ 1000`，字段：`eventType` / `raw` / `receivedAt`）
- 消费 `qq.push` 发送群消息：有 `reply_msg_id` 走被动回复（显式 `msg_seq`），否则 `sendText` 主动发送；发送成功 ACK 并记录返回 `msg_id`
- SIGINT/SIGTERM 优雅退出

待实现：

- 全量群消息 `GROUP_MESSAGE_CREATE`（需平台开启「接收所有消息」）
- 主动消息频控/预算记账（qpm / 1000 群日配额，见 `docs/modules/push.md`）

验证：`apps/verify-push.mjs` 注入主动推送指令到 `qq.push` 观察出站消费/ACK（详见 `docs/modules/push.md`「测试方式」）
