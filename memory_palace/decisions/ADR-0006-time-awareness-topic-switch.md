# ADR-0006：时间感知与话题切换（上下文私有层 + 确定性门控）

- 状态：已采纳（Accepted）
- 日期：2026-10-16

## 背景

主持人 LLM 没有时钟，聊天上下文里也没有任何时间信息：

- `chat` 路径的 `SYSTEM_PROMPT` 只有人设，无「现在」；
- Redis 热缓存 `chat:history:{group_id}` 只存 `{role, content}`，昨天 18:40 与 5 分钟前在模型眼里等价；
- SQLite `chat_history.created_at` 虽已落库，但不回流到 LLM 上下文。

结果：用户隔了一夜再说话，模型仍把上一场当作正在进行的对话继续（「第二天还以为我没去睡觉」）。
同时群内多轮对话会持续累积，缺少「话题已换」的信号。

（注：**会话隔离**（群内区分「在跟谁说话」）是独立需求，另行设计，本 ADR 不涉及。）

## 决策

1. **时间作为「上下文私有层」注入**：在 `chat.ts` 组装 messages 时临时渲染，插入
   - 独立 `system` 时间块：`[时间] 现在 2026-02-14 周六 09:12 (UTC+8)；距上次对话 14h32m`；
   - 历史在**发言簇**（间隔 ≥ 5min）首条加 `[MM-DD HH:mm]` 锚点。
2. **时间只进上下文，不进消息**：不入 `chat:history`、不出现在出站 `qq.push` 正文；`SYSTEM_PROMPT` 另约束「用户主动问才提时间」。
3. **只给时间事实，不做内容推断**：间隔用紧凑数字（`刚刚` / `32m` / `14h32m` / `1d14h`），不出现「昨晚 / 该睡了 / 刚下班」这类语义推测。
4. **时区单点**：`CHAT_TZ`（默认 `Asia/Shanghai`），全部格式化走 `src/time-context.ts` 纯函数；存储层用 ISO 8601 UTC / epoch ms（见 `docs/conventions/persistence.md`）。
5. **话题切换 = 条件性第 2 问**：距上次对话 ≥ `TOPIC_GAP_MINUTES`（默认 30）时，把 `topic_switch`（`noul`，`state.previous_message` = 上一条用户消息）**并入同一次** `decideIntent` 的 `client.decide()` 请求。
6. **A：边界标注**：`P(true) ≥ TOPIC_SWITCH_THRESHOLD`（默认 0.7）→ 在**本轮** history 条目写 `boundary: true`，渲染出 `--- 话题切换 ---`。因 `chat:history` 只在 chat 分支写入，被路由为 game/dev 的那条消息会把边界**暂存**到 `chat:boundary:{group_id}`（TTL 1h），由「空档后第一条进入聊天历史的用户消息」取用落位；门控命中时以本次判定为准。
7. **C：pending 失效**：同一门控命中时**确定性丢弃**该 `(group_id, openid)` 的未决「意图确认」，而不是靠模型判定后再清。
8. **不做**：episode 摘要（L3）、时间感知主动消息（配额 + 骚扰风险）、主动报时、`narrateGame` 时间注入。

## 理由

- **必须确定性计算**：LLM 做时间算术必错，且模型无时钟——时间只能由规则层算好再喂。
- **前缀缓存友好**：时间块放 messages 末尾、历史锚点由固定 `ts` 决定（一次写入后不变），不破坏 DeepSeek 前缀缓存；改 `SYSTEM_PROMPT` 会每次都 miss。
- **「不落库/不出站」有现成模式**：`chatFlow` 的链接摘录（`augmentWithLink`）已采用「临时增强、只存原始 content」，时间注入复用同一模式，不引入新机制。
- **零额外 RTT**：`systemone.ts` 的 `decide()` 本就是「单次请求多问题」，追加 `topic_switch` 不增加网络往返；再用 `gap` 门控避免每轮都问，均摊成本几乎为零。
- **A 优于截断**：截断/降权旧轮次等于做 L3（改上下文组织），成本与风险高；仅标注边界即可让模型知道「前面不属当前话题」，行为不变。

**备选方案与否决原因：**

| 备选 | 否决原因 |
|---|---|
| 时间写进 `SYSTEM_PROMPT` 开头 | 每次都击穿前缀缓存 |
| 把时间拼接进用户消息 `content` 并落 `chat:history` | 污染历史、可能被模型复述给出站消息 |
| 单独新增一次 SystemOne 调用问话题 | 多一次 RTT，用户明确要求评估延迟 |
| episode 摘要（L3） | 用户明确不做；A 已覆盖「跨场」感知 |
| 用话题切换判定来清 pending | 代码顺序上 `pending` 分支先于 `decideIntent`，该设计实际不生效（见 C） |

## 影响

- 新增：`apps/agent-gateway/src/time-context.ts`（纯函数 + 可注入时钟）、`src/chat-history.ts`（历史读写 + 待落位边界，`ListStore`/`KVStore` 可注入，便于单测）。
- 改动：`chat.ts`（`buildChatMessages`）、`decision.ts`（`topic_switch` + `isTopicSwitch`）、`index.ts`（30min 门控 → 边界落位 / 暂存 / pending 丢弃）。
- 新增配置：`CHAT_TZ`、`TOPIC_GAP_MINUTES`、`TOPIC_SWITCH_THRESHOLD`（见 `docs/modules/agent-gateway.md`）。
- **一次读取**：`handle` 开头统一读 `chat:history`（算 gap），并传给 `chatFlow` 复用，同一轮不重复 `LRANGE`；相比改动前，非 chat 消息多 1 次读取（本地 Redis 亚毫秒级，相对 SystemOne/LLM 可忽略）。
- **边界落位语义**：边界标记落在「空档后第一条真正进入聊天历史的用户消息」上（而非触发判定的那条）；因 `gap` 取 `chat:history` 最后一条 `ts`，未写入时门控会持续命中（同一请求内，无额外 RTT），故边界不会丢失而是可自愈。
- `CHAT_HISTORY_LIMIT` 仍为 20：跨天场景可能整窗都是昨天内容，时间锚点 + 边界标记用于提示，本轮不做窗口策略调整。

## 依据与验证状态

| 数值 | 来源 | 验证状态 |
|---|---|---|
| 30min（`TOPIC_GAP_MINUTES`） | 本项目工程经验值（把「午休」与「过夜」归为同一类边界） | **经验值**，非外部约束，可配；上线后按日志 `话题切换判定` 分布复核 |
| 0.7（`TOPIC_SWITCH_THRESHOLD`） | 工程经验值（保守：宁少标、不误切） | **经验值**，非外部约束，可配 |
| 5min（`CLUSTER_GAP_MS`） | 工程经验值（同簇不打重复锚点） | **经验值**，非外部约束，可配 |
| 群被动回复窗口 5min | 官方文档，见 `docs/conventions/external-constraints.md` | 已验证（既有登记，本 ADR 仅引用） |

以上「经验值」均不作为外部约束登记到 `docs/conventions/external-constraints.md`；未做正式延迟预算（用户判定其意义不大），不变量为「话题判定与意图判定共用同一次 HTTP 请求」，由 `test-topic-switch.mts` 断言 `calls.length === 1` 守卫。
