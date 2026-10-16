# ADR-0007：会话隔离与全局人物画像层（三线内容隔离 + 日批蒸馏）

- 状态：已采纳（Accepted）
- 日期：2026-10-16
- 关联：ADR-0006（时间感知与话题切换）、`docs/conventions/persistence.md`、`docs/modules/agent-gateway.md`、`docs/architecture.md` §5/§6、`docs/conventions/mcp-primitives.md`

## 背景

群聊机器人原有的上下文模型是「**每群一个共享聊天会话**」（`chat_session_id = group_id`）。多人群里不同用户的 @ 消息进入同一份历史，导致串台；ADR-0006 已注明「会话隔离是独立需求，另行设计」。

用户提供了一份《群聊机器人记忆系统技术方案》（4 层记忆：全局 / 群 / 用户 / 会话 + 会话隔离 + 摘要压缩 + 向量召回）。经评估，**对当前项目重新裁定**：

- 方案的「会话隔离」净增量很小（Redis 键下沉）；
- 用户层（人物画像）是**真实需求**：LLM 对人物的认知需**跨群**（同一人在不同群里应共享画像）；
- 群层 / 摘要压缩 / 向量召回暂不做。

### 关键 GAP 纠正

初版评估把「全场上下文」误解为「采集群内非 @ 消息作为环境」。**纠正**：任何对话的上下文就是**以主持人（机器人）为中心的那条对话线**——凡进入主持人的内容已在它的对话里，**不需要采集非 @ 消息**。据此，原「T1 群内临时环境」支线（改 bot-adapter 采集非 @、per-group 环形缓冲、隐私出域、前缀缓存等）**全部作废**。

## 决策

### A. 上下文模型：以主持人为中心的对话线

1. **上下文 = 主持人参与过的对话**，而非「群里的消息流」。不同「线」是不同池子：
   - **chat**：一个用户 ↔ 主持人的一条线；
   - **game**：一桌 ↔ 主持人的一条线；
   - **dev**：一个人 ↔ 主持人的一条线。
2. 不引入 `platform` / `topic_id`（QQ 群无话题）。

### B. 三线内容隔离

3. **chat 会话键下沉为 `(group_id, openid)`**：Redis 热缓存 `chat:history:{group_id}:{openid}`；话题边界 `chat:boundary:{group_id}:{openid}` 同步下沉。
   - 附带收益：`gap`（距上次对话）变为「该用户上次与主持人说话」，比现状更准；pending 失效逻辑更自洽。
4. **game 不隔离**（room 级）；**dev 不隔离**（`dev_session_id` 级，现状已满足）。
5. **隔离的是「对话内容」**（chat 阶段历史 vs game 局面），**不隔离「人物模型」**（画像，见 D）。

### C. 各线背景组装

6. **chat 背景**：热窗口（最近 `CHAT_HISTORY_LIMIT` = 20 条）内，按**话题阶段裁剪**——只保留最近一条 `boundary` 之后的轮次（a1）。
   - **20 条以外一律不保留**（不扩热窗口、不做摘要、不做语义召回）。
7. **game 背景**：引擎提供 `describe`/`summary` 原语（room 级紧凑局面文本视图，纳入 `mcp-primitives.md`），注入游戏叙述。
   - **关键工作在 LLM 侧**：注入格式、粒度与「只做承上启下、不逐条复述」的约束；引擎只给事实，**不灌原始 JSON**。
   - 现状缺口：`narrateGame(action, result)` 目前看不到局面（`docs/architecture.md` §6.1「`get_state`/`get_legal_actions` 尚未接入」），本 ADR 予以补上。
8. chat 与 game **内容不互相注入**（chat 有活跃对局时也不注入局面）。

### D. 全局人物画像层

9. **画像按 `openid` 隔离、跨群共享**，作为**全局背景**跨 chat / game / dev 三条线注入。
10. **纯文本画像**（`profile_text`），存入 `user_profile`。
11. **数据源 = 复活并扩展的 `chat_history`（per-user 持久对话表）**，收 **chat + game** 两条分支的用户轮与助手轮（**不含 dev**）。
    - 画像**只从 `chat_history` 蒸馏**，不去查 `group_messages`（避开 `reply_msg_id` ↔ `message_id` 的配对摩擦）。
12. **写入路径 B**：agent-gateway 在 chat / game 分支发 `chat.log`（Redis Stream）→ Rust `server` 消费落库（保持 SQLite 单写者；Node 不直连库）。
13. **日批蒸馏**：**凌晨 4:00（按 `CHAT_TZ`）** 异步执行：
    - 取窗口内活跃 `openid` 集合（同一窗口），再取每人窗口内对话；
    - **增量**：以 `user_profile.distilled_at` 为游标（而非固定「昨日 4 点」，任务失败重跑不漏窗）；
    - **生成与把关分离**：**生成归本地 9B**（按需起停，纯文本画像的抽取/合并/衰减由生成模型完成），**把关归 SystemOne**（`noul`/`score` 决定候选事实是否入画像、重要度与新旧覆盖，阈值化淘汰）。SystemOne 为**纯决策模型、不生成文本**（`startlux_decision/model.py`：Nothing is generated），不能独立承担蒸馏。
    - **本地蒸馏模型**：`Qwen2.5-3B-Instruct Q4_K_M`（`C:/llm/models/qwen2.5-3b-instruct-q4_k_m.gguf`，位置固定、不迁移、不进仓库，env 配置）。用**非思考型小模型**：经实测，思考型 9B（Qwythos / Qwen3.5）会“想不停”（正文为空）或复读输入，不适合本任务；3B 级抽取器 + few-shot 提示即可。4:00 按需启动 `llama-server` → 探活 → 蒸馏 → 关闭。
    - **释放确认（进程级、确定性）**：等待子进程 `exit`（超时则强制 `SIGKILL` / `taskkill /F`），复核 **PID 消失 + 端口释放 + 健康探针不可达**；`nvidia-smi` 显存检查为可选诊断，**不做快照 diff**。
    - **衰减**：「旧画像 + 新增对话 → 新画像」，事实衰减（过时/矛盾以新为准，长期未提及的可淡出）由生成提示词声明 + SystemOne 判定共同完成。
14. **跨群隐私为已知取舍**：画像全局共享，意味着「A 群说的事可能出现在 B 群回答里」。接受此取舍；`chat_history` 保留 `group_id`，未来若需分级可见性无需重建数据。
15. `group_messages` **原样保留**（群级原始归档：排查 / 审计 / web 展示），不再兼任画像数据源。

### E. 表结构

16. 复活 `chat_history`（原来只有 `group_id`）：

```sql
CREATE TABLE chat_history (
  id         INTEGER PRIMARY KEY,
  openid     TEXT NOT NULL,      -- 用户（跨群主键）
  group_id   TEXT NOT NULL,      -- 来源群
  role       TEXT NOT NULL,      -- user | assistant
  content    TEXT NOT NULL,
  created_at TEXT NOT NULL       -- ISO8601 UTC
);
CREATE INDEX idx_chat_openid_time ON chat_history(openid, created_at);
```

17. 新增 `user_profile`：

```sql
CREATE TABLE user_profile (
  openid       TEXT PRIMARY KEY,
  profile_text TEXT NOT NULL,    -- 纯文本画像
  distilled_at TEXT,             -- 上次蒸馏时刻（增量游标）
  updated_at   TEXT NOT NULL
);
```

18. 存储分层：Redis 热缓存（per-user，chat-only，仅服务**实时上下文**） / SQLite `chat_history`（冷存，chat+game，服务**画像蒸馏**） / `group_messages`（群级原始归档）。三者有意冗余，各服务不同消费方。

### F. 不做与留口子

19. **不做**：非 @ 消息采集、群层记忆、会话摘要压缩（T3）、向量/语义召回（T4）。
20. **留口子**：把「上下文来源」按线抽象（chat 来源 / game 来源各一个组装函数），为未来换实现不动调用方。

### G. 未来方向（记录，不实现）

21. 用户可能**砍掉 chat 线**并**放开非 @ 消息**，做「真正的背景」——届时 **chat 与 game 两条线会合并**。
    - 潜台词：合并成群级对话池后，闲聊可能**稀释 game 相关背景**；届时对策是以**局面状态为主锚 + game 作用域的相关性窗口**，而非把大池子整灌。
    - 本 ADR 的「按线抽象」（决策 20）即为该切换预留。

## 理由

- **以主持人为中心**：机器人的上下文边界天然是「它参与过的对话」，也避免了采集非 @ 带来的一连串成本（bot-adapter 改过滤、隐私出域、前缀缓存击穿、外部权限待验证）。
- **只下沉键 + 阶段裁剪**：T0 的隔离不需要新存储、新基建；复用已有 `boundary` 即可满足「保留话题切换 + 阶段窗口内容」。
- **画像跨群 + 全局注入**：满足「LLM 对人物的认知跨群」诉求；与「内容隔离」不冲突（隔离的是对话内容，不是人物模型）。
- **日批而非实时**：画像变化频率低，凌晨批处理不占用被动回复窗口与延迟预算；`distilled_at` 游标保证增量与可重跑。
- **写入路径 B**：遵守「SQLite 单写者 = Rust server」的既有约定，Node 侧不引入直连库。
- **纯文本画像**：注入简单；代价是 merge / 遗忘 / 衰减依赖 LLM 重写（已接受）。

**备选方案与否决原因：**

| 备选 | 否决原因 |
|---|---|
| 采集非 @ 消息作「群内临时环境」（原 T1） | 与「以主持人为中心」的 GAP 纠正冲突；带来跨模块改动、隐私出域、前缀缓存与外部权限问题 |
| 画像按 `(group, openid)` 隔离 | 不满足「跨群认知」诉求 |
| 画像结构化事实表 | 用户选定纯文本；结构化留作将来可选升级 |
| 画像从 `group_messages` 现算 | `reply_msg_id` ↔ `message_id` 配对摩擦；无 `role` 结构 |
| 固定「昨日 4 点 ~ 今日 4 点」窗口 | 任务失败会漏窗；改用 `distilled_at` 游标 |
| 实时蒸馏画像 | 占用被动回复延迟与成本；日批已满足需求 |
| 会话摘要压缩（T3）/ 向量召回（T4） | 用户明确不做；a1 阶段裁剪已覆盖当前诉求 |

## 影响

- **改动**：
  - `apps/agent-gateway`：`chat-history.ts`（键加 `openid`）、`index.ts`（读/写带 `sender_id`；chat 组装按 `boundary` 裁阶段 a1；chat/game 分支发 `chat.log`）、新增日批调度器与蒸馏管道、`narrateGame` 注入局面摘要、上下文来源按线抽象。
  - `crates/storage`：`chat_history` 表迁移（加 `openid`/`group_id`/`role`/`created_at` 索引）、新增 `user_profile`。
  - `crates/server`：新增 `chat.log` 消费者 + 画像 upsert / 读窗口 API。
  - `crates/game-engine-mcp` / 引擎：新增 `describe`/`summary` 原语。
  - `crates/bot-adapter`：**不改**（非 @ 采集确认不做）。
- **文档**：`persistence.md`（会话标识、表结构、存储分层）、`architecture.md` §5/§6、`agent-gateway.md`、`mcp-primitives.md`、`external-constraints.md`（如需登记日批时区）。
- **测试**：`test-topic-switch` / `test-time-context` / `test-intent-confirm`（键下沉）；新增阶段裁剪、`chat.log` 落库、蒸馏增量与画像注入的测试。
- **风险**：跨群画像隐私（已知取舍）；纯文本 merge 的质量依赖 LLM；Redis 热缓存与 SQLite `chat_history` 双写的一致性（由路径 B 的 Stream at-least-once + 幂等保证）。
- **数值约束**：无新增外部数值约束；「凌晨 4 点」为工程约定（可配）。

## 依据与验证状态

| 事项 | 来源 | 验证状态 |
|---|---|---|
| `chat_session_id = group_id`（现状） | `docs/conventions/persistence.md` | 已核实（本次据 ADR 修订为 per-user） |
| `get_state(room_id)` 原语存在 | `docs/modules/game-engine.md`、`crates/game-engine-mcp` | 已核实 |
| `messages` 非 @ 消息当前被 bot-adapter 丢弃 | `crates/bot-adapter/src/lib.rs::process_one` | 已核实（本次决定仍不采集） |
| `group_messages` 出站 `qq_id = NULL`、入站 = openid | `crates/bot-adapter/src/lib.rs::archive_one` | 已核实 |
| SQLite 单写者 = Rust server | `docs/conventions/persistence.md` | 已核实（写入路径 B 据此） |
| 凌晨 4 点 / 纯文本画像 / 跨群共享 | 用户裁定 | 已确认 |
| 蒸馏生成用本地小模型（Qwen2.5-3B-Instruct） | 用户裁定 + 实测 | 已确认（9B 思考型不适合，见下） |
| SystemOne 为纯决策模型、不生成文本 | `StartLux-Decision-v1-4B-Q8_0-GGUF/startlux_decision/model.py` | 已核实（故生成须用生成模型） |
| 本地 9B GGUF 存在于 `/c/llm/llama.cpp-master/models/` | 本机文件系统 | 已核实（`HauhauCSQwen3.5-9B…`、`Qwythos-9B…Q4_K_M`） |
| 蒸馏模型改用 Qwen2.5-3B-Instruct（`/c/llm/models/`） | 实测：9B 思考型想不停/复读；3B+few-shot 可用 | 已验证 |
| 未来「砍 chat + 放开非 @ → 两线合并」 | 用户裁定 | 记录为方向，未实现 |
