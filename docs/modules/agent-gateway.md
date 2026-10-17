# agent-gateway（主持人，pi-sdk）

- **语言**：Node.js（TypeScript）
- **位置**：`apps/agent-gateway`
- **依赖**：pi-sdk（`@earendil-works/pi-coding-agent`）

## 职责

- 消费 Redis Stream `internal`（bot-adapter 归一化后的内部事件）
- 以 **pi-agent 会话**充当主持人：编排「意图 → 决策 → 执行 → 叙述」
- 会话管理：**chat 按 `(group_id, openid)` 隔离**（每人一条独立会话线，ADR-0007）；开发任务用子会话；**人物画像跨群共享**
- **MCP client**（stdio）：连接 game-engine MCP server
- **MCP client**（stdio）：连接 dev-task MCP server（`dev-task-client.ts`）
- **MCP client**（stdio）：连接 docs MCP server（`docs-client.ts`，本地开发文档预览，只读）
- **子会话管理**（pi-sdk）：`dev-session.ts` 创建/恢复开发任务子会话（`SessionManager.create/open` + `prompt`/`dispose`）
- **decision-service client**（HTTP）：调用 `/v1/systemone`
- **auth client**（HTTP）：`auth-client.ts` 调 server `/auth/issue` 签发短期 token，拼计划文档短链
- **profile client**（HTTP）：`profile/client.ts` 调 server `/internal/*` 读写对话窗口与人物画像（`profile/`）
- **本地蒸馏模型生命周期**：`profile/local-llm.ts` 按需起停 `llama-server`（Qwen2.5-3B）+ 进程级释放确认
- 置信度阈值配置化 + 兜底
- 出站：写 `qq.push`；对话归档写 `chat.log`（chat + game）

## 关键行为

### 会话与队列

- chat：**每人一条线**（`chat:history:{group_id}:{openid}`）；同一会话/对局的消息**串行**处理（per-session 队列），避免 pi-sdk 会话并发 prompt
- **阶段裁剪（a1）**：注入前只取热窗口（最近 20 条）中**最近一条 `boundary` 之后**的轮次（`chat-history.ts` 的 `cropToCurrentPhase`）；无 boundary 则全取。20 条以外不保留
- 开发任务：独立子会话，与聊天会话隔离

### 开发任务（dev）

- **显式命令硬判断**（先于决策模型，`dev-command.ts`）：`#任务 [状态]` / `#进度 <id>` / `#确认 <id>` / `#取消 <id>` / `#能力` / `#发起 <需求>`（`#开发` 别名）/ `#补充 <id> <正文>` / `#文档`。**禁止用正则/硬编码对自由文本判断意图**。
- `#文档`：白名单 → `docs-mcp.prepare_docs_link` 校验 → 网关签发 `docs:all` 短期 token → 被动回复文档预览短链（`https://{域名}/dev_docs?token=xxx`）。
- dev 意图：白名单校验 → `create_task` → 受理回执（含 sessionId）→ 异步 `spawn` 子会话规划；**仅显式 `#发起` 才建任务**，决策引擎判为 dev 但无显式命令时只提示，不自动建任务。
- 子会话（pi-sdk）跑 plan-workflow：生成计划 → `update_task_meta` 回填标题 → `transition awaiting_confirm` → 签发短期 token（server `/auth/issue`）→ 主动推送计划文档短链；子会话挂 pi MCP 扩展（`.pi/mcp.json`），可用 playwright 等 MCP 工具联网
- **LLM 思考模式**：chat / 游戏叙述默认开启（`LLM_THINKING_ENABLED=1`，不传 `thinking`），`=0` 显式关闭；源起「3s 被动超时」误判，见 ADR-0002 修订
- `#确认` → `confirm_task` → 恢复子会话执行 → `transition verifying` → `#确认` 再验收 → `done`
- `#进度`：经 `prepare_task_link` 校验归属后**重签新短链**（原 token 过期可自助重取）
- `#补充 <id> <正文>`：经 `append_instruction` 校验后 `runDevResume` 追加到已有子会话（不落 chat 兜底）
- 聊天多轮：**按 `(group, openid)` 隔离**的 Redis 热缓存最近 20 条（`CHAT_HISTORY_LIMIT` 可配）注入 `chatReply`；条目随消息存 `ts`（写入时刻）与 `boundary`（话题切换边界），旧条目无这两个字段时降级为「时间未知、无边界」。读写见 `src/chat-history.ts`，渲染见 `src/time-context.ts`，上下文组装见 `src/context.ts`

### 游戏（拉斯维加斯，显式命令）

- **显式命令硬判断**（先于决策模型，`game-command.ts`）：确定性前缀 → MCP `execute_action`，**不经决策模型**（与「lobby 消息驱动」一致；禁止用正则猜自由文本意图）。

| 命令 | 动作 | 权限 |
|---|---|---|
| `#开局` / `#拉斯维加斯` | `create_game(las-vegas, group, sender)` | 群内无 Active |
| `#加入` / `#退出` | `lobby:join` / `lobby:leave` | 群成员 |
| `#开始` / `#中断` | `lobby:start` / `lobby:abort` | 房主 |
| `#踢 <openid>` | `lobby:kick:<openid>` | 房主 |
| `#小游戏 <3~6>` | `lobby:set_small_game_count:<n>` | 房主 |
| `#放 <1~6>`（也支持 `#放3`） | `place:<n>` | 当前玩家 |
| `#重投` / `#弃` | `reroll` | 当前玩家、筹码 ≥ 1 |
| `#加机器人 [n]` | `lobby:add_bot` × n | 房主（lobby） |
| `#单人 [n]`（默认 3 个机器人） | create + add_bot×n + start | 群内无 Active |
| `#小游戏#<id>` / `#规则 <id>`（大小写不敏感） | `get_small_game_rules` | 任意 |
| `#动作 <id>` | `execute_action("sg:<id>")`（小游戏动作 >26 时的兑底） | pending.actor |
| `#局面` / `#牌面` | `state_summary`（+ 牌面短链） | 任意 |

- **机器人补位（一个人也能测）**：`#单人` 一键创建 1 人 + N 机器人并开赛；`#加机器人` 手动补位。机器人由引擎内置启发式 AI 驱动——agent-gateway 在人类动作后循环调 MCP `bot_action`（含小游戏交互）直到人类回合/终局，并通过 `state.seats[].is_bot` 判定。
- **动作后反馈（承上启下）**：每次动作（含**每个机器人**动作）输出「结果 + 局面摘要 + 下一步指引（谁在等 / 可用动作）+ 牌面页短链」；模板 `msg_seq=1` 必达，`GAME_NARRATION_ENABLED=1` 时追加 LLM 主持人情景叙事 `msg_seq=2`（失败静默降级）。正常局引擎执行后要么提示下一位玩家、要么提示当前玩家未完成的小游戏/动作。
- `#局面` 同样输出摘要 + 下一步指引 + 牌面短链；局面摘要含每号赌场的小游戏标签（如 `3号2骰(B1)`），并区分「下注后触发」与「B1/C2 结算前触发」。
- **出站限额**（`external-constraints.md`）：同一条用户消息最多**被动回复 5 次**、`msg_seq` 递增。网关用 Redis 计数器 `msgseq:{message_id}` 递增；每个动作的「模板 + LLM 叙事」合并为**一条**回复。超过 5 条时**默认丢弃并告警**，仅 `PROACTIVE_MESSAGES_ENABLED=1`（需通过主动消息门禁）才降级为主动消息。错误/中断（`lobby:abort`）不做 LLM 叙事。
- **主动消息门禁**（ADR-0009）：默认禁用；方案含主动消息须人工确认并写 `【主动消息已人工确认】`，`npm run check:proactive-gate` 校验。

- **自然语言路径**：`start_game` 意图（无猜拳手势时）→ 创建拉斯维加斯 lobby（引导文案含上表命令）。猜拳 `rps` 保留为显式手势玩具路径。
- **动作选择（阶段 4b）**：`game_action` 意图 → `get_legal_actions(room, sender)` → SystemOne `choice`（≤26 个动作）选一个 → `execute_action`；动作数 >26 或决策不可用时回显编号清单兜底（`formatActionList`）。小游戏交互态的合法动作归 `pending.actor`，非该玩家无动作。
- 命令 → 动作串映射与解析单测见 `src/game-command.ts` / `test-game-command.mts`。

### 人物画像（全局层，ADR-0007）

- **跨群共享**（键 = `openid`），作为全局 system 块注入 chat（`context.ts` 的 `buildChatContext`）与 game 叙述（`buildGameNarrationInput`）；**为空则跳过**
- **读取**：`profile/inject.ts` 的 `ProfileCache`（TTL 内存缓存，默认 `PROFILE_CACHE_TTL_MS=600000`）→ server `/internal/profile/{openid}`；失败静默不注入
- **日批蒸馏**（默认关；`PROFILE_DISTILL_ENABLED=1` 开启，`profile/scheduler.ts` 每日 `PROFILE_DISTILL_HOUR`（默认 4）按 `CHAT_TZ` 触发）：
  - 窗口 = `[Redis 游标 profile:distill:cursor, now)`（失败重跑不漏窗）；`server /internal/chat/*` 读活跃 openid 与对话
  - **按 openid 分开**；同一人**按字符分块 map-reduce**（`PROFILE_CHUNK_CHARS`，默认小模型用 2000），逐块生成候选事实 → 合并去重
  - **生成归本地小模型**（`profile/local-llm.ts`，Qwen2.5-3B，非思考型），**把关归 SystemOne**（`profile/gate.ts` 的 `noul`，阈值 `PROFILE_GATE_THRESHOLD`）
  - 素材只用**用户轮**发言（剔除 assistant；丢弃含 `<|…|>` 模板控制符/超长消息，防注入）；发言数 < `PROFILE_MIN_USER_MESSAGES` 则跳过
  - 组装纯文本（限 `PROFILE_MAX_CHARS`）→ server `/internal/profile` upsert；日批后缓存整体失效
  - **模型生命周期**：按需 `spawn llama-server` → 探活 → 生成 → 关闭 → **释放确认**（PID 消失 + 端口释放 + 健康探针不可达）
- 一次性全量 bootstrap（可覆盖全部历史、不写库/不推进游标）：`scripts/profile-bootstrap.mts`

### 聊天联网（Plan B 链接抓取）

- 触发：聊天消息包含 `https://` 链接（**必须有协议头**，`http://` 不算）时才走判断；无链接不做任何额外调用。
- 判断：调 SystemOne `noul`（是/否）题型「这条消息里的链接，需要抓取网页内容才能回答用户吗？」→ 只返回 P(true)，阈值化（默认 `>=0.5`）。
- 抓取：命中后抓一次链接正文（仅 https；防 SSRF 字面量：拒绝 localhost/内网 IP/带凭据；手动跟随重定向逐跳校验；限时/限长）→ 抽纯文本 → 作为 `[链接内容摘录 …]` 追加进本轮用户消息上下文，再走 `chatReply`。
- 降级：SystemOne 不可用/超时、URL 不安全、非 2xx、非文本、抓取失败 → 一律静默按普通聊天处理（历史里存的仍是原始消息，不存摘录）。
- 日志：判断命中/跳过（`[agent-gateway] 链接抓取判断：抓取|跳过（P=…）`）与抓取成功（`[agent-gateway] 链接正文已注入（N 字符）`）均打印，便于核验。
- 新增文件：`src/link-fetch.ts`（`isSafePublicHttpsUrl` / `htmlToText` / `fetchLinkText`）；`decision.ts` 新增 `extractHttpsLinks` / `decideLinkFetch`。

| 变量 | 默认 | 说明 |
|---|---|---|
| `CHAT_LINK_FETCH_ENABLED` | `1` | 聊天链接抓取总开关（`0` 关闭） |
| `CHAT_LINK_FETCH_THRESHOLD` | `0.5` | `noul` P(true) 阈值（≥ 则抓取） |
| `CHAT_LINK_TIMEOUT_MS` | `10000` | 抓取超时（覆盖重定向链） |
| `CHAT_LINK_MAX_CHARS` | `2000` | 抽取文本最大字符数 |

### 时间上下文与话题切换

**时间只作为「上下文私有层」**：让主持人知道「现在几点 / 距上次对话多久」，但它**不是消息内容**。

| 约束 | 实现 |
|---|---|
| 入模型上下文 | `chat.ts` `buildChatMessages`：`system` 人设 → 历史（簇首带 `[MM-DD HH:mm]` 锚点、带边界者前置 `--- 话题切换 ---`）→ **独立 `system` 时间块** → 当前用户消息 |
| 不落归档 | 时间块与锚点是**组装时临时渲染**，`chat:history` 存的仍是原始 `content`（与链接摘录同一模式） |
| 不出站 | 时间文本不会出现在 `qq.push` 正文；`SYSTEM_PROMPT` 另约束「用户主动问才提时间」 |

- 时区单点：`CHAT_TZ`（默认 `Asia/Shanghai`；容器默认 UTC，故显式固定）。间隔为纯数字紧凑格式（`刚刚` / `32m` / `14h32m` / `1d14h`），**不做「昨晚 / 该睡了」这类内容推断**。
- `handle` 开头统一读一次 `chat:history`（算 gap 供门控与 pending 失效用），并传给 `chatFlow` 复用，**同一轮不重复 LRANGE**。
- 同一「发言簇」（间隔 < `CLUSTER_GAP_MS` = 5min）只在簇首打时间锚点。
- **话题切换（A）**：距上次对话 ≥ `TOPIC_GAP_MINUTES`（默认 30）时，在**同一次** `decideIntent` 请求里追加第 2 问 `topic_switch`（`noul`，`state.previous_message` = 上一条用户消息）——**零额外 RTT**。`P(true) ≥ TOPIC_SWITCH_THRESHOLD`（默认 0.7）→ 在本轮 history 条目上写 `boundary: true`（决策一次、可复用，前缀稳定），后续渲染出 `--- 话题切换 ---`。
- **边界落位（非 chat 分支）**：`chat:history` 只在 chat 分支写入，所以被路由为 game/dev 的那条消息只能把边界暂存到 `chat:boundary:{group_id}`（TTL 1h），由「空档后第一条进入聊天历史的用户消息」取用并落位。因为 `gap` 取 `chat:history` 最后一条的 `ts`，未写入时门控会持续命中（同一次请求内，无额外 RTT）而自愈。门控命中时以**本次**判定为准，丢弃旧暂存。
- **pending 失效（C）**：同一门控命中时，先**确定性丢弃**该 `(group_id, openid)` 的未决意图确认——确认提示的语境有效期就等于这个间隔，否则跨 30 分钟的一句「早」会被当成 `y/n` 消费。
- 门控可整体关闭：`TOPIC_GAP_MINUTES=0`（此时不做话题判定，也不使 pending 失效）；`SystemOne` 不可用时 `topicSwitch` 为 `undefined` → 按「未切换」安全降级。
- 仅 **chat 路径**注入时间上下文；`narrateGame`（游戏情景叙述）不带。
- 日志：`话题切换判定 gap=14h32m P=0.88 → 新话题|延续`、`话题边界暂存（intent=… 不写 history）`、`话题切换：取用待落位边界`、`pending 失效丢弃 gap=… cid=… intent=…`。

### 编排流程

见 `docs/architecture.md` §6。

### 意图决策（SystemOne）

- `src/systemone.ts`：`SystemOneClient`，POST `${SYSTEMONE_URL}/v1/systemone`（默认 `http://127.0.0.1:8090`），超时 `SYSTEMONE_TIMEOUT_MS`（默认 3000ms）；`fetchImpl` 可注入便于单测
- `src/decision.ts`：`decideIntent(content, client, { threshold })` **异步**；`intent ∈ {chat, start_game, game_action, dev}`，criteria 中文（实测零样本可用）
- **分类前屏蔽 URL**：送模型的 `state.message` 把 `http(s)://…` 换成 `[链接]`（`maskUrls`）——链接文本本身是强 dev 信号（如 `/dev_docs`），中英文均会被误判为 dev；`#前缀` 兜底与链接抓取判断仍用原文
- 状态：`state = { message }`；启用话题门控时额外带 `previous_message`（当前用户消息仍走 `maskUrls`）。架构 §6.1 的 `get_active_game`/`get_legal_actions` 与动作选择**尚未接入**（待拉斯维加斯引擎）

### 置信度兜底（三档 + 意图确认）

决策模型输出 `choice + confidence`，按**双阈值**分三档（`decision.ts` `confidenceBand`，约定 `[low, high)` 为 confirm）：

| 档位 | 处理 | via |
|---|---|---|
| `conf ≥ DECISION_INTENT_THRESHOLD_HIGH`（默认 0.8） | 直接按决策结果路由 | `decision` |
| `[DECISION_INTENT_THRESHOLD, DECISION_INTENT_THRESHOLD_HIGH)` | **人工二次确认**：回 y=按**原消息**执行候选意图；n=原消息走低阈值兜底；候选=chat 免确认（等同兜底） | `confirm` |
| `conf < DECISION_INTENT_THRESHOLD`（默认 0.6） | `#前缀` → 默认 `chat` | `prefix` / `default` |

- **意图确认**（与 dev 的 `#确认` 计划确认区分）：未决状态存 Redis `intent:pending:{group_id}:{openid}`，TTL `INTENT_CONFIRM_TTL_SEC`（默认 300s，对齐群被动回复窗口）；按 openid 隔离，同 openid 再来中带请求则**覆盖**，旧「原消息+确认消息」写入异常记录（`[意图确认][异常]` 结构化日志）；回复非 y/n → 追问一次，仍非 y/n → 判 n。
- **一轮确认 = 2 次被动对话**（并非一次回复内完成）：

  | 轮 | 入站 | 出站 | 语义 |
  |---|---|---|---|
  | ① | 原问题 A | 确认提示（`reply_msg_id=A`） | **原问题的答复就是二次确认提示** |
  | ② | y/n（消息 B） | 按**原问题 A**执行的答复（`reply_msg_id=B`） | y 的答复为执行结果；n 时为低阈值兜底结果 |

  内容层面可在 `group_messages` 完整还原（两对 in/out；`raw_payload.reply_msg_id` 可证配对）；`chat_history`（LLM 上下文）**不记**确认提示与 y/n，仅记「原问题 + 执行答复」，保持对话语义干净。
- **日志可回溯**：两轮用原问题 `message_id` 作关联 id `cid` 串联；每条消息的终结行统一带决策模型原始输出 `model=<候选意图>:<置信度>`（无模型回答时 `-`）——**由 `via` + `model` 即可反推是否进过确认带**（`via=confirm*` 必在中带；`via=decision` 在高带；`via=prefix|default` 在低带）。`cid` 后置于 `←` 之前，正文仍在 `←` 之后：
  - 发起：`intent=<候选> via=confirm model=<候选>:<conf> cid=A`
  - 通过：`intent=<候选> via=confirm_yes model=<候选>:<conf> cid=A`
  - 否决/判否：`intent=<低阈值结果> via=confirm_no fallback=prefix|default model=<候选>:<conf> cid=A`
  - 高带/低带：`intent=<最终> via=decision|prefix|default model=<候选>:<conf> cid=A`
  - 追问/判否：`意图确认=unknown|n cid=A`（异常另落 `[意图确认][异常]`）
  - dev `#前缀` 硬路由：`intent=dev via=prefix cmd=… model=- cid=…`

  答复正文不入 agent 日志（真相在 `group_messages` / `qq.log`）。
- **阈值标定**：基于 205 条历史回放（与线上 `intent=` 日志一致率 100%）——非 chat 正确预测 `conf=0.840~0.965`，非 chat 错判 `conf=0.388~0.815`（≥0.6 的 11 条落在 0.610~0.815）；确认集 `[0.6,0.8)` 内非 chat 预测 9 条 9 错（精确率 100%）。引擎可自带双阈值（dev 建议 0.9、game 0.80，缺省回落全局）。
- `conf(action)` 决定游戏动作是否自动执行
- **服务不可用**（超时/连不上/非 2xx）：`via="unavailable"` → 回「服务繁忙，请稍后重试」，不降级为 chat（避免服务故障时乱聊）
- 无效消息至少回一条；确认轨迹日志：`意图确认=y|n|unknown`，每条终结行带 `model=<候选>:<置信度>`

### 决策层配置

| 变量 | 默认 | 说明 |
|---|---|---|
| `SYSTEMONE_URL` | `http://127.0.0.1:8090` | 决策服务根地址（部署见 `docs/modules/decision-service.md`） |
| `SYSTEMONE_TIMEOUT_MS` | `3000` | 单次决策超时 |
| `DECISION_INTENT_THRESHOLD` | `0.6` | 意图置信度低阈值（低于 → `#前缀` → 默认 chat） |
| `DECISION_INTENT_THRESHOLD_HIGH` | `0.8` | 意图置信度高阈值（`[低, 高)` 走「意图确认」；必须 ≥ 低阈值，倒挂自动夹紧） |
| `INTENT_CONFIRM_TTL_SEC` | `300` | 未决「意图确认」存活时长（秒），对齐群被动回复窗口 |
| `TOPIC_GAP_MINUTES` | `30` | 话题切换门控：距上次对话 ≥ 该间隔才做判定（`0` 关闭门控）。**经验值，非外部约束** |
| `TOPIC_SWITCH_THRESHOLD` | `0.7` | `topic_switch` 的 P(true) 阈值（≥ 判新话题）。**经验值，非外部约束** |
| `CHAT_TZ` | `Asia/Shanghai` | 时间上下文时区（容器默认 UTC，故显式固定） |

### 人物画像配置（ADR-0007）

| 变量 | 默认 | 说明 |
|---|---|---|
| `PROFILE_CACHE_TTL_MS` | `600000` | 画像内存缓存 TTL（ms） |
| `PROFILE_DISTILL_ENABLED` | `0` | 日批蒸馏总开关（`1` 开启） |
| `PROFILE_DISTILL_HOUR` | `4` | 日批触发小时（按 `CHAT_TZ`） |
| `PROFILE_LLM_MODEL` | 空 | 本地蒸馏模型 GGUF 路径（固定位置，不迁移）；未配则跳过 |
| `PROFILE_LLM_BIN` | `llama-server` | llama-server 可执行文件 |
| `PROFILE_LLM_PORT` | `8082` | 蒸馏服务的 llama-server 端口 |
| `PROFILE_LLM_NGL` / `PROFILE_LLM_CTX` | `99` / `8192` | GPU 层数 / 上下文长度 |
| `PROFILE_LLM_TEMPERATURE` | `0.2` | 生成温度 |
| `PROFILE_LLM_DISABLE_THINKING` | `0` | `1` 禁用思考（仅对支持 `enable_thinking` 的模型生效） |
| `PROFILE_LLM_MAX_TOKENS` | `4096` | 单次生成 token 上限 |
| `PROFILE_CHUNK_CHARS` | `6000` | 分块字符上限（小模型建议 2000） |
| `PROFILE_MIN_USER_MESSAGES` | `3` | 低于该用户发言条数不蒸馏 |
| `PROFILE_GATE_THRESHOLD` | `0.5` | SystemOne 把关阈值（P(值得写入) ≥ 阈值才保留） |
| `PROFILE_MAX_CHARS` | `1200` | 画像文本上限（字符） |
| `CHAT_LOG_MAXLEN` | `1000` | `chat.log` 归档条数上限 |

- 测试：`npm run test`（离线，统一 runner `test-all.mts`：显式清单 + 逐文件计时 + 汇总，失败/超时非零退出；新增测试需在列或入 EXCLUDED 附原因）；`npm run test:decision:live`（需本地决策服务在线）；健康探针 `node apps/scripts/check-decision-service.mjs`

### web 短链配置

| 变量 | 默认 | 说明 |
|---|---|---|
| `SERVER_URL` | `http://127.0.0.1:3000` | server 内部 HTTP（auth 签发） |
| `PUBLIC_BASE_URL` | 空 | web 公网基址（cloudflared 域名）；留空则计划推送退回长文 |
| `DOCS_MCP` | `crates/target/debug/docs-mcp(.exe)` | docs-mcp 二进制路径（本地开发文档预览） |

### 出站与回复

- 写 `qq.push`（遵循 event_id / MAXLEN 约定）
- 回复策略分层，见 `docs/architecture.md` §8
- 游戏交互：默认回规则模板摘要（承上启下，`msg_seq=1`）；`GAME_NARRATION_ENABLED=1` 时追加主持人 LLM 情景叙述（`msg_seq=2`，注入**局面摘要**（引擎 `state_summary`）+ **玩家画像**，见 `context.ts`），失败/超时静默降级为仅模板
- 开发任务：被动回执/状态同步（`pushReply`），计划/结果异步主动推送（`pushActive`，无 reply_msg_id）；计划推送用「标题 + 短链」替换 >1500 字长文（未配置 `PUBLIC_BASE_URL` 时退回长文）
- MCP 环境变量显式传递：MCP SDK 默认只继承白名单环境变量，`SQLITE_PATH`/`DEV_WHITELIST` 需在 `env` 显式传入

## 依赖边界

- 主持人 LLM：远程 API（provider 可配，型号待定）
- 蒸馏模型：本地 `llama-server` + Qwen2.5-3B GGUF（按需起停）
- decision-service：本地 HTTP `/v1/systemone`
- game-engine：MCP stdio
- dev-task：MCP stdio（状态机 + 白名单）
- docs-mcp：MCP stdio（本地开发文档预览，只读）
