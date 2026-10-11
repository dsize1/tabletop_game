# bot-adapter（协议适配）

- **语言**：Rust（lib）
- **位置**：`crates/bot-adapter`

## 职责

- 订阅 Redis Stream `qq.raw`
- 将 QQ 群事件转换为内部统一消息事件（`crates/shared` 类型）
- **过滤**：只保留 @机器人文本消息，其余忽略
- 写入内部事件到 Redis Stream `internal`（供 agent-gateway 消费）

## 过滤规则

| 规则 | 处理 |
|---|---|
| 非 @消息（`GROUP_MESSAGE_CREATE`） | 忽略 |
| 非文本（`message_type != 0`） | 忽略 |
| `content` 前导/尾随空格 | trim |

## 依赖

- `shared`（内部事件类型）
- Redis Streams 客户端

## 约定

- 协议字段定义见 `docs/conventions/message-protocol.md`

## 归一化映射（实测）

| 内部字段 | 来源（qq.raw 的 `raw` 字段） |
|---|---|
| `sender_id` | `raw.author.member_openid`（注意：不是 QQ 号） |
| `sender_name` | `raw.author.username` |
| `group_id` | `raw.group_openid` |
| `message_id` | `raw.id`（被动回复的 msg_id） |
| `content` | `raw.content`（需 trim 前导/尾随空格） |
| `msg_type` | `raw.message_type` |
| `timestamp` | `raw.timestamp` |
| `event_type` | 顶层 `eventType` |
