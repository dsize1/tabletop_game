# 三国杀 AI（P6）

> 状态：**P6.1 已落地（Batch-1 / 1.5 / 1.6，2026-11-01）**。范围：规则引擎 + 行为树 + 启发式评估的 Baseline。
> **门禁口径 = 全将（149 位去重）+ 全卡牌（160 张）100 局：100/100 终局、0 非法动作、0 回环、0 卡死**；
> `standard`（25 将）为快速自检口径（同样 100/100）。
> 代码：`crates/engines/sanguosha/src/ai/`；门禁：`npm run check:sgs-ai`。
> 相关：ADR-0019（AI/录制回放）、ADR-0032（AI 分层与可见信息不变量）、`development-plan.md` P6。

## 1. 分层与数据流

```text
GameState(引擎私有 state)
   │  SgsState::parse
   ▼
SgsView::from_state(&SgsState, viewer)        ← 只投影「可见信息」（ADR-0032）
   │  公共：体力/手牌数/装备/判定区/标记/存活/公开身份/弃牌顶/牌堆余量/特殊区
   │  私有：本人手牌明细、本人身份、本人 pending 选项
   ▼
Evaluator { view, ThreatModel }               ← 资源价值 / 威胁压力 / 盟友救援
   ▼
handlers::score_action(id) → Score(priority, score)   ← 按动作 id 家族
   ▼
tree::decide(view, legal) → Decision          ← 取 (priority, score) 最大；同分按合法顺序
   ▼
Policy::choose → execute_action               ← 引擎无条件重校验（AI 只建议）
```

### 1.1 可见信息不变量（硬约束）

- handler **只能**经 `SgsView` 读状态：拿不到他人手牌、他人身份、牌堆顺序；
- 主公身份、已死亡角色身份按规则公开 → 允许；
- 守卫用例：`ai::tests::ai_view_hides_other_hands_and_identities`、
  `ai::tests::hidden_info_invariance`（篡改他人手牌/牌堆顺序后决策**必须**不变）。
- 该不变量是后续「信念建模 / 强化学习」训练数据不泄漏的前提。

## 2. 模块结构

| 文件 | 职责 |
|---|---|
| `view.rs` | `SgsView` / `SeatView` / `CardView` / `PendingView`：可见信息投影 |
| `card.rs` | 牌型判定 + 基准价值表（弃牌/选牌/权衡用，纯调参入口） |
| `score.rs` | `Score(priority, score)` 口径与优先级常量 |
| `eval.rs` | 座位资源价值、敌人压力、盟友救援、自身危险度 |
| `threat.rs` | 敌友嫌疑模型（身份先验 + 公开观测后验） |
| `handlers/play.rs` | 出牌阶段（`play:*`） |
| `handlers/respond.rs` | `respond:*` / `pass`（濒死求桃、杀闪、无懈、改判） |
| `handlers/target.rs` | `target:*`（选目标；增益技能反转成「选盟友」） |
| `handlers/pick.rs` | `discard:*` / `choose:*` / `pick:*` / `confirm`（选牌与多选） |
| `handlers/misc.rs` | `skill:*` / `setup:choose_general:*` / `lobby:*` |
| `tree.rs` | 分派 + 未登记 kind 的「安全推进」兜底 |
| `prune.rs` | 候选剪枝（≤26，保底 `pass`/`end_play`/最高分动作） |
| `policy.rs` | `SgsPolicy`（`game_engine_core::Policy`）+ `explain()` 调试入口 |
| `selfplay.rs` | 自对弈驱动：统计 + 空转守卫（回环破环器） |

## 3. 打分口径

- `priority`：硬优先级（越大越优先）——`MUST(10)` 自救 > `URGENT(8)` 救队友/必选响应 >
  `COMMIT(6)` 提交 > `STRONG(4)` 强收益 > `NORMAL(0)` 常规 > `IDLE(-2)` 空过。
- `score`：同优先级内的期望收益；选动作 = `(priority, score)` 字典序最大，同分按合法动作顺序（确定性）。
- 关键取舍：
  - **没有明确敌人就不出攻击牌**（未知身份 `enemy_score = 0.5` < 门槛 0.6）；
  - 残血时必出闪/必用桃（`self_danger()` 提升闪的分数与优先级）；
  - 濒死不救敌方、必救自己与明确盟友；
  - 无懈只在「冲着自己/明确盟友来」时用；
  - 装备空槽优先，同槽只在更值钱时换装。

## 4. 威胁 / 身份模型（P6.1 先验 + 一步后验）

| 本人身份 | 主公 | 其他未知 |
|---|---|---|
| 主公 / 忠臣 | 友（-1） | 未知（0） |
| 反贼 | 敌（+1） | 未知（0） |
| 内奸 | 轻敌（+0.15） | 未知（0） |

- `enemy_score = (suspicion + 1) / 2`；`> ATTACK_THRESHOLD(0.6)` 才主动出手。
- **公开观测后验**：引擎在造成伤害时写一条 `Observation{kind:"damage", actor, target, amount}`
  （`SgsState.observations`，公开信息、有上限 240 条）。据此：
  - 打主公 +0.45/点（最强反贼信号）；
  - 打自己 +0.25/点；
  - 主公打他 +0.30/次。
  只上调不下调（不抵消主公锚点）。
- 效果：主忠方不再「永不还手」，自对弈最大步数从 11707 降到 859，胜率分布从「反贼 19/20」
  变为「26 主 / 69 反 / 5 内」（100 局 `standard`）。

## 5. 回归与指标

```bash
npm run check:sgs-ai              # 10 局全将（CI，release，约 5s）
npm run check:sgs-ai:100          # 100 局全将（验收）
npm run check:sgs-ai:standard     # 100 局标准 25 将（快速自检）
npm run check:sgs-ai:full         # 100 局全将（与 check:sgs-ai:100 等价的显式口径）
cargo test -p sanguosha ai::    # 用例（视图/不变量/11 种 kind/剪枝/自对弈）
```

硬指标（`--strict`）：终局率 100%、非法动作 0、状态回环 0、卡死 0、pending 覆盖 ≥ 8 种。

**实测（2026-11-01，seed=1）**：

| 口径 | 局数 | 终局 | 非法动作 | 回环 | 卡死 | 最大步数 | 耗时 | 胜率 |
|---|---|---|---|---|---|---|---|---|
| **全将（149 位，门禁）** | 100 | **100** | **0** | **0** | **0** | 3235 | 64s | 主 18 / 反 78 / 内 4 |
| `standard`（25 将，自检） | 100 | 100 | 0 | 0 | 0 | 1326 | 59s | 主 26 / 反 69 / 内 5 |

pending 覆盖（`standard` 100 局）：`arrange` `choice` `choose_many` `discard` `dying`
`judge_rewrite` `pick_card` `respond` `target` `wugu` `wuxie`（11/11）。

## 6. 驱动宿主

| 宿主 | 用途 | 状态 |
|---|---|---|
| `game-engine-mcp::bot_action` | 测试/机器人补位（单动作） | ✅ 已按 `game_id` 分派 `SgsPolicy` |
| `sanguosha::ai::selfplay` | 自对弈回归 / 数据采集（操控**全部座位**） | ✅ |
| `sanguosha::ai::driver::drive_bots` | **只推进机器人座位**（server 用；回落/破环器/步数上限与自对弈同口径） | ✅ P6.3 |
| `server::game_routes`（Web 动作后自动推进） | 灰度主通路（单写者；房间锁 + `POST /internal/game/.../drive` 兜底） | ✅ P6.3 |
| `agent-gateway::driveBots` | 三国杀**不**使用（避免 MCP/server 双写，已在网关侧显式跳过） | ✅ P6.3 |

- 机器人动作与人类动作走**同一条落库链路**（`game_events` + 快照，同一事务语义）；
- 单次请求最多推进 200 步（`DRIVE_MAX_STEPS`）；停因写入日志与响应（`human_turn` / `finished` / `max_steps` / `loop` / `no_legal_actions` / `engine_error`）；
- e2e：`npm run test:sgs-bot`（1 真人 + 4 机器人：Web 动作 → 自动推进 → 每次返回都不是机器人回合）。

## 7. 已知问题（引擎侧；Batch-1.5 已修 E1–E5 + 同类，剩 1 类）

全将自对弈暴露「合法集与结算不一致 / 交互不收敛」。**Batch-1.5 已修**：

| # | 现象 | 修法 |
|---|---|---|
| E1 | `内部错误：缺少【闪】结算结果` | ①主公技（激将/护驾）无人代出时显式回传「未响应」；②专对拼点赢/未赢都回填 outcome（并修「此杀对其无效」仍造成伤害）；③`step_respond` 对过期 outcome（牌已不在手）按「未响应」处理 |
| E2 | `respond:<card>` 但牌不在手 | 同 E1③（过期 outcome）+ 离间/胆守等 `choose_many` 的 `pending.data.selected` 补齐 |
| E3 | `sha_used` 与「无距离」混用（合法集给杀、结算拒） | `legal_actions` 与 `execute` 统一拆成 `count_free`（成略）/ `dist_free`（成略或 `no_distance_turn`）；目标范围用 `dist_free` |
| E4 | 陷嗣选满 2 人仍给第三人 | 选满后只保留已选项；`pending.data` 补 `selected/min/max` |
| E5 | 界仁德可重复对同一角色发动 | Target 阶段过滤 `rende_target:*` 已用角色 |
| E6 | 技能帧不自行消解（誓战/激将/护驾） | AI 侧：同技能本局 ≥2 次则不再发动（引擎待修「失败后仍合法」） |
| E7 | 贯石斧多选确认后残留旧 `selected` | `act_multiselect` 确认时剔除已失效牌并重问（通用） |
| E8 | 离间多选 `pending.data` 缺 `selected` | 补 `selected`（Batch-1） |
| E9 | 酒（界酒池）合法集允许但结算拒绝 | `execute` 与 `legal_actions` 同源（`jiu_unlimited`） |
| E10 | 排异无「权」仍给目标 | 无「权」时不询问 |
| E11 | 天香/多选在无牌可选时挂空 pending → 无合法动作 | 多选无选项且 `min>0` → 跳过；白名单牌（界天香装备）也进选项 |
| E12 | 从谏交牌后仍被重复询问 | 牌已不在手 → 结束该帧 |
| E13 | 濒死阶段栈空仍停在 `Dying` → 无合法动作 | `advance` 自愈：`Dying && 栈空` → 回到 `Turn` |

**Batch-1.6（2026-11-01，F 系列）**：

| # | 现象 | 修法 |
|---|---|---|
| F1 | 除疠（`chuli`）多步选目标 `pending.data` 无 `selected` → 反复点选/取消，技能仍合法 | 补 `selected/min`；AI 侧 `kind=choice` 也按 `selected` 跳过已选项 |
| F2 | 伤逝（张春华）在**弃牌阶段**补牌 → 「弃 1 摸 1」无限弃牌（seed=54/75 跑满 20000 步） | 按牌面「除弃牌阶段外」在 `TurnPhase::Discard` 不触发 |
| F3 | 破环器误判：无懈链/五谷逐人询问、以及「当先」额外出牌阶段（可见状态与前一阶段相同） | 只在**有 pending 的交互中**做回环检测，并跳过 `wuxie`/`wugu`；digest 纳入待选状态与选项集 |

当前 `check:sgs-ai`（全将 100 局 strict）**0 回环**。

**AI 侧兜底（与引擎缺陷解耦）**：
1. 未登记 `pending.kind` → 优先 `pass`（安全推进）；
2. 同一技能本局已被本人发动 ≥2 次 → 不再发动（防 E6）；
3. 过期选项过滤：动作载荷里的牌必须在本人的可见牌里；
4. 驱动层「回环破环器」：状态摘要回环时换一个未试过的动作；
5. 引擎拒绝动作 → 退到 `pass`/`confirm`/`end_play`/首个其他动作（记录「回落」，不中断全局）。

## 8. 后续路线

| 阶段 | 内容 | 状态 |
|---|---|---|
| P6.2 全覆盖 | 11 种 pending kind handler + 逐 kind 用例 | ✅ |
| P6.3 灰度通路 | server 自动推进 + `#加机器人` + `#武将包` + Web 等待态 | 待做 |
| P6.4 决策日志 | `game_decision_log` + JSONL 导出 + 回放 | 待做 |
| P6.5 中期智能 | 结构化历史（技能/牌使用）、手牌信念、关键决策一层搜索、选将细化 | 待做 |
| P6.6 终极 AI | 自对弈数据管线 → 完美信息蒸馏 → 策略网络（ONNX，可选 feature） | 待做 |
| 全将修复 | E1–E13 + F1–F3 全部修复；门禁已切全将（strict） | ✅ |
