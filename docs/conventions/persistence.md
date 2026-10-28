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
| `chat.log` | agent-gateway | server | per-user 对话归档（chat + game 分支，画像蒸馏数据源，ADR-0007） | `MAXLEN ~ 1000` |

- 消费模式：**消费者组**（consumer group），`XREADGROUP >`
- ACK 时机：**处理并写 SQLite 成功后**再 `XACK`
- 失败处理：不 ACK，消息留在 PEL，重启后重投
  - 例外（`qq.push` 出站）：**永远不可能成功**的条目直接 ACK 丢弃——被动回复超 5 分钟窗口、
    永久性业务码（`40034024` / `11321`）、除 429 外的 4xx、残缺条目。
    详见 `docs/modules/push.md`「投递失败处理」、`apps/qq-gateway/src/push-policy.ts`
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
| 聊天 | `chat_session_id`（= `group_id:openid`） | 每人一条独立会话线（ADR-0007 决策 3） |
| 开发任务 | `dev_session_id` | 子会话；存 `memory_palace/plan/{id}.md` 相对路径 |
| 游戏对局 | `room_id`（配 `game_id`） | 对局；配合事件溯源 |

> 术语统一：玩家身份标识为 **openid**（= QQ `member_openid`，非 QQ 号）。表中 `qq_id` 列与 MCP 参数 `qq_id` 均存该 openid。

### 时间戳约定

- 存储层一律用 **ISO 8601 UTC**（`YYYY-MM-DDTHH:MM:SSZ`，即 `new Date().toISOString()`）；展示/注入模型时才按 `CHAT_TZ`（默认 `Asia/Shanghai`）转换。
- 进程内比较用 **epoch ms**（如 Redis 热缓存的 `chat:history` 条目 `ts`），避免字符串时区歧义。
- 容器默认时区是 UTC：**不得依赖宿主本地时区做日期/星期计算**，文案类时间格式统一走 `apps/agent-gateway/src/time-context.ts`（`CHAT_TZ` 单点）。

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

-- 开发任务子会话（session_id = pi 子会话 id = 计划文档 id）
CREATE TABLE dev_tasks (
    session_id  TEXT PRIMARY KEY,           -- 统一键（= 计划文档 id = web 页标识）
    group_id    TEXT NOT NULL,
    qq_id       TEXT NOT NULL,              -- 发起者（需在白名单内）
    title       TEXT,                        -- 子会话计划生成后回填
    plan_path   TEXT,                        -- memory_palace/plan/{session_id}.md 相对路径
    status      TEXT NOT NULL,               -- queued|planning|awaiting_confirm|running|verifying|done|failed|cancelled
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- 聊天历史（per-user 对话；画像蒸馏的数据源，ADR-0007）
-- 实时上下文热路径：Redis 列表 `chat:history:{group_id}:{openid}`（每人一条，最近 N 条）
-- 热缓存条目为 {role, content, ts, boundary?}：ts/boundary 是内部元数据，只进 LLM 上下文，不进出站消息
-- 全量冷存由 agent-gateway 发 `chat.log` → server 消费落库（chat + game；不含 dev）
CREATE TABLE chat_history (
    id         INTEGER PRIMARY KEY,
    event_id   TEXT    NOT NULL UNIQUE,      -- 幂等（at-least-once）
    openid     TEXT    NOT NULL,             -- 用户（画像跨群主键）
    group_id   TEXT    NOT NULL,             -- 来源群
    role       TEXT    NOT NULL,             -- user | assistant
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL              -- ISO8601 UTC
);
CREATE INDEX idx_chat_history_openid_time ON chat_history(openid, created_at);

-- 全局人物画像（跨群；ADR-0007）；distilled_at 兼作日批增量游标
CREATE TABLE user_profile (
    openid       TEXT PRIMARY KEY,
    profile_text TEXT NOT NULL,              -- 纯文本画像
    distilled_at TEXT,                       -- 上次蒸馏时刻
    updated_at   TEXT NOT NULL
);
```

### 道具中台表（P1，三国杀）

- 通用：`prop_packs`（包）、`prop_assets`（素材登记）、`prop_status_log`（生命周期/审核）。
- 专用：`sgs_identities`、`sgs_identity_setup`、`sgs_card_defs`、`sgs_cards`、`sgs_generals`、`sgs_skills`、`sgs_general_skills`。
- 写路径：仅 `crates/server` 的 `/internal/gameprops/*`；SPA 不直连 SQLite。
- 生命周期：`draft/testing/review/live/offline`（软删）；只读接口只取 `live`。
- 运行时不建实体表：三国杀对局仍复用 `game_state` + `game_events`。

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
        │     ├─ 聊天   → 写 group_messages（+ 按 (group,openid) 隔离的会话线）
        │     ├─ 开发   → 写 dev_tasks + plan 文件
        │     └─ 游戏   → MCP execute_action → 写 game_events + game_state（同事务）
        └─ XACK

对话归档（chat + game；不含 dev）
  agent-gateway → chat.log (MAXLEN) → server 消费 → 写 SQLite chat_history

人物画像（跨群）
  日批：server /internal/chat/* 读窗口 → 本地蒸馏模型（生成）+ SystemOne（把关）
        → server /internal/profile upsert 写 user_profile；注入时经 /internal/profile 读

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
