# 持久化与消息总线约定

> 核心分工：**Redis Streams 负责「传输」，SQLite 负责「真相」**。
> 本文定义两者的边界与关键约定，配套 `docs/conventions/message-protocol.md`。

## 1. 核心原则

- Redis Streams 是**过路缓冲**：只保留「尚未处理完的消息」与「热缓存」，不当历史档案。
- SQLite 是**持久化真相源**：所有需要留档的数据（群消息、玩家执行记录、游戏状态/事件）最终写入 SQLite。
- 消费端「**先写 SQLite 成功，再 ACK**」；重投靠 `event_id` 唯一键去重。
- 游戏状态变更走**事件溯源**：append-only 事件 + 快照。

## 2. Redis Stream 约定

| Stream | 生产者 | 消费者 | 用途 | 保留策略 |
|---|---|---|---|---|
| `qq.raw` | qq-gateway | bot-adapter | 入站原始 QQ 群事件 | `MAXLEN ~ 1000`（消费后即裁） |
| `internal` | bot-adapter | agent-gateway | 归一化内部事件 | 短保留 |
| `qq.push` | agent-gateway / push | qq-gateway | 出站推送指令 | `MAXLEN ~ 1000` |

- 消费模式：**消费者组**（consumer group），`XREADGROUP >`
- ACK 时机：**处理并写 SQLite 成功后**再 `XACK`
- 失败处理：不 ACK，消息留在 PEL，重启后重投
- 写入：`XADD <stream> MAXLEN ~ 1000 * ...`

## 3. 幂等与去重

- Redis Streams 是 **at-least-once** 语义，消费可能重复（PEL 重投）。
- 每条事件携带全局唯一 `event_id`（由写入方生成，如 uuid）。
- SQLite 侧用 `event_id UNIQUE` 约束 + `INSERT OR IGNORE`（或 `ON CONFLICT DO NOTHING`）防重。

## 4. SQLite 约定

- 打开参数：**WAL 模式** + `busy_timeout`（如 5000ms）+ `foreign_keys=ON`
- **单写者**：写入收敛到 Rust `server` 单点；web / qq-gateway 不直连 SQLite，只读经 server 的查询 API
- 事务边界：一次事件处理 = 一个事务（状态更新 + 记录追加原子提交）

### 会话标识（三类）

| 场景 | 标识 | 说明 |
|---|---|---|
| 聊天 | `chat_session_id`（= `group_id`） | 每群一个共享会话 |
| 开发任务 | `dev_session_id` | 子会话；存 `memory_palace/plan/{id}.md` 相对路径 |
| 游戏对局 | `room_id`（配 `game_id`） | 对局；配合事件溯源 |

### 表结构草案

```sql
-- 群消息留档（进/出都存）
CREATE TABLE group_messages (
    id          INTEGER PRIMARY KEY,
    event_id    TEXT    UNIQUE NOT NULL,
    direction   TEXT    NOT NULL,            -- 'in' | 'out'
    group_id    TEXT    NOT NULL,
    qq_id       TEXT,                        -- 发送者（out 时为机器人）
    content     TEXT,
    raw_payload TEXT,                        -- 原始事件 JSON
    created_at  TEXT    NOT NULL
);

-- 玩家执行记录（人类可读的动作流水）
CREATE TABLE player_actions (
    id         INTEGER PRIMARY KEY,
    event_id   TEXT    UNIQUE NOT NULL,
    game_id    TEXT    NOT NULL,
    room_id    TEXT    NOT NULL,
    qq_id      TEXT    NOT NULL,
    action     TEXT    NOT NULL,             -- 指令/动作名
    payload    TEXT,                         -- 参数 JSON
    created_at TEXT    NOT NULL
);

-- 游戏事件溯源（append-only，真相）
CREATE TABLE game_events (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id   TEXT    UNIQUE NOT NULL,
    game_id    TEXT    NOT NULL,
    room_id    TEXT    NOT NULL,
    event_type TEXT    NOT NULL,             -- 状态变更事件类型
    actor      TEXT,                         -- 触发者（玩家/系统/决策建议）
    payload    TEXT,                         -- 事件 JSON（含动作、结果）
    created_at TEXT    NOT NULL
);

-- 游戏状态快照（真相；version 对齐 game_events.seq）
CREATE TABLE game_state (
    game_id    TEXT NOT NULL,
    room_id    TEXT NOT NULL,
    state      TEXT NOT NULL,                -- 状态 JSON
    version    INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (game_id, room_id)
);

-- 开发任务子会话
CREATE TABLE dev_tasks (
    id              INTEGER PRIMARY KEY,
    dev_session_id  TEXT NOT NULL,
    group_id        TEXT NOT NULL,
    qq_id           TEXT NOT NULL,           -- 发起者（需在白名单内）
    plan_path       TEXT,                    -- memory_palace/plan/{sessionId}.md 相对路径
    status          TEXT NOT NULL,           -- queued | running | done | failed
    created_at      TEXT NOT NULL
);
```

### 事件溯源约定

- 一次状态变更 = 一个事务：`INSERT game_events` + `UPDATE game_state`（version 自增）原子提交。
- 复盘 = 读快照 + 重放 `version` 之后的 events。
- 中断对局可放宽：仅要求「最近操作可追溯」（快照 + 增量事件已满足）。

## 5. 数据流

```
QQ 群事件进
  → qq.raw (MAXLEN) → bot-adapter 消费
        ├─ 过滤（只 @文本）
        ├─ 写 SQLite group_messages（event_id 去重）
        ├─ 归一化 → internal → agent-gateway 消费
        │     ├─ 聊天   → 写 group_messages（+ 共享会话关联）
        │     ├─ 开发   → 写 dev_tasks + plan 文件
        │     └─ 游戏   → MCP execute_action → 写 game_events + game_state（同事务）
        └─ XACK

回复 → qq.push (MAXLEN) → qq-gateway 消费
        ├─ 写 SQLite group_messages（我方发出的也留档）
        ├─ 调 QQ API 发群消息
        └─ XACK
```

## 6. 升级信号

| 信号 | 说明 | 应对 |
|---|---|---|
| 群消息吞吐持续上千/秒 | Streams + SQLite 单写者成瓶颈 | 总线换 Kafka/NATS，SQLite 换 Postgres |
| 需要「写库 + 发消息」原子一致 | 需要事务性 outbox | SQLite 建 outbox 表，轮询/触发器投递 |
| 要保留全量历史可重放审计 | Redis 不适合当历史库 | 直接读 SQLite（已是真相源） |
| 多进程并发写 SQLite | 单写者限制 | 保持单点写，或升级 PG |
