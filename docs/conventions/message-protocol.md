# 内部消息事件协议

> 草案，后续按需完善。

## 消息接收范围

- **只处理 @机器人文本消息**（`GROUP_AT_MESSAGE_CREATE` + `message_type=0`）
- 普通群消息（`GROUP_MESSAGE_CREATE`）与非文本消息在 bot-adapter 忽略

## Redis Stream 约定

| Stream | 生产者 | 消费者 | 内容 |
|---|---|---|---|
| `qq.raw` | qq-gateway | bot-adapter | 原始 QQ 群事件 |
| `internal` | bot-adapter | agent-gateway | 归一化内部事件 |
| `qq.push` | agent-gateway / push | qq-gateway | 推送指令（发群消息） |

约定：
- 消费用**消费者组**（`XREADGROUP >`）；**先写 SQLite 成功再 `XACK`**
- 写入用 `XADD ... MAXLEN ~ 1000` 做裁剪；Redis 只当过路缓冲，不留历史
- 事件带唯一 `event_id`，SQLite 侧 `event_id UNIQUE` 去重（at-least-once）
- 详见 `docs/conventions/persistence.md`

## 回复策略

- **被动回复**（带 `msg_id`）窗口有限（约 3~5s，以官方为准），仅用于窗口内必达消息
- **主动发送**（`POST /v2/groups/{group_openid}/messages`）有日配额，用于异步/补发
- 分层回复见 `docs/architecture.md` §8

## qq.raw 消息结构（实测）

qq-gateway 写入 `qq.raw` 的每条消息为单字段 `event`（JSON 字符串）：

```json
{
  "eventType": "GROUP_AT_MESSAGE_CREATE",
  "raw": { "...平台原始事件..." },
  "receivedAt": 1791568570397
}
```

`raw` 关键字段（群 @消息实测）：

| 字段 | 含义 |
|---|---|
| `raw.id` | 消息 id，**被动回复**时作为 `msg_id` 关联 |
| `raw.author.member_openid` | 发送者群内 openid，**玩家身份标识**（非 QQ 号） |
| `raw.author.username` | 昵称 |
| `raw.group_openid` | 群 openid |
| `raw.content` | 文本内容（@机器人前缀已被平台去除，但**可能带前导空格，需 trim**） |
| `raw.message_type` | 0=文本 |
| `raw.timestamp` | RFC3339（带时区） |

`eventType` 取值：`GROUP_AT_MESSAGE_CREATE`（@消息）/ `GROUP_MESSAGE_CREATE`（全量消息）。

## 事件结构（草案）

```json
{
  "id": "uuid",
  "type": "message.group.at",
  "timestamp": "2026-10-08T00:00:00Z",
  "source": "qq-gateway",
  "payload": {}
}
```

## 事件类型（草案）

- `message.group.at`：群 @ 文本消息（进入 `internal`）
- `message.group.normal`：群普通消息（bot-adapter 忽略）
- `push.global`：全局推送
- `push.personal`：个人推送

（后续按需补充）
