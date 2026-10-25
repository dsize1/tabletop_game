# ADR-0016：三国杀引擎 —— 模式无关内核 + 武将技能事件总线 + 刚性规则内核

- **状态**：提案·待评审（2026-10-25）
- **相关**：`docs/games/sanguosha/plan.md`（立项需求）、`docs/games/sanguosha/development-plan.md`、`docs/games/sanguosha/data-model.md`；ADR-0004（状态序列化）、ADR-0008（事件/RNG/lobby/视图）、ADR-0012（交互原语）、ADR-0017（道具中台）、ADR-0018（设计工作台）、ADR-0019（AI 与录制）、ADR-0020（多端与视图三分区）
- **结论一句话**：新建 `crates/engines/sanguosha`，用「模式无关内核（牌/区域/事件总线/结算栈）+ 模式插件（identity / guozhan）+ 武将技能代码实现」承载三国杀；**规则与合法动作只在 Rust 代码里**，中台与工作台只提供元数据与设计，不能运行时改规则。

## 背景

1. 立项文档要求复刻标准身份局（5–8 人），并为国战（双将、暗将、珠联璧合、野心家）预留兼容，本期不实现国战。
2. 三国杀与拉斯维加斯不是一个量级：108 张牌、几十个武将、上百个技能、跨玩家响应（闪/无懈可击/桃）、大量时机与嵌套结算。
3. 立项文档明确：武将描述不严谨、规则复杂，**不能靠提示词让 AI 直接写武将**；需要设计工作台 + 引擎提供合法动作与规则校验。
4. 现有 `EnginePlugin` 契约（`new_game` / `legal_actions` / `execute_action` / `public_state` / `player_view` / `summarize`）已具备事件溯源与视图脱敏，可复用；缺的是「技能事件注册、可组合回合阶段、大动作空间」。
5. 用户 2026-10-25 裁定：**Q 群侧后置**，Web 为交互基座；三国杀的多步出牌靠 pending 链承载，Q 只做开局/状态/链接（ADR-0020）。

## 决策

### 1. 模块与状态分层

```
crates/engines/sanguosha/
├─ lib.rs          # EnginePlugin 实现 + 引擎注册
├─ mode/
│  ├─ identity.rs  # 标准身份局（本期唯一实现）
│  └─ guozhan.rs   # 国战接缝（仅 trait/占位，不实现）
├─ kernel/         # 模式无关：Card / Zone / Deck / Player / Distance / Damage / Judge
├─ flow/           # 回合阶段机（可组合）+ 结算栈 + pending 队列
├─ skill/          # Skill trait + SkillRegistry + 触发事件总线
├─ generals/       # 各武将技能实现（按包分文件）
└─ view.rs         # public_state / player_view / summarize
```

- `GameState.data` 强类型化，沿用「信封 + 引擎私有 JSON」两段式（ADR-0004），`data.schema_version` 提供迁移入口。
- `mode` 是策略对象：身份局与国战共享牌/区域/伤害/判定内核，差异在**身份与胜利条件、选将/亮将、体力与势力规则**。
- 不做「一个模式一个 crate」：规则内核会重复，且跨模式复用（牌、技能、伤害）是主要工作量。

### 2. 规则内核刚性（不可配置）

- 合法性、权限、距离、伤害结算、濒死、胜负判定**全部在 Rust 代码**；`execute_action` 无条件重校验（沿用 ADR-0008）。
- 中台（ADR-0017）只存元数据：名称、描述、图片、花色/点数、分类、距离值、血量、技能类型。
- 工作台（ADR-0018）只产出**设计稿 + 测试夹具 + 代码任务**，其流程图**不参与线上执行**；武将落地必须走代码评审 + 测试。
- 理由：三国杀规则相互作用强、边界多；数据驱动/脚本化规则会带来调试黑洞与不可控的安全面。拉斯维加斯的「声明式时机表」只适合其封闭小游戏集合，不能推广到武将技能。

### 3. 回合 = 可组合阶段 + 可插入钩子

- 基础阶段序列：`prepare → judge → draw → play → discard → end`。
- `TurnContext { seat, general, skills, phase, round, once_flags }` 随回合传递；技能可：
  - 在阶段前后插入额外阶段（如额外判定/额外出牌）；
  - 跳过/重复某阶段；
  - 修改摸牌数/手牌上限/使用次数。
- 核心流程**不写武将名分支**：所有武将差异经 `Skill` 钩子进入；禁止 `if general == "xxx"` 散落在流程代码里。

### 4. 技能事件总线（确定性）

- `Skill` trait：`fn triggers(&self) -> &[Trigger]; fn on_event(&mut self, ctx: &mut SkillCtx) -> Option<SkillOutcome>`。
- 触发事件（首期）：`game_start`、`turn_start/end`、`phase_start/end`、`card_use_before/after`、`card_targeted`、`damage_before/after`、`dying`、`death`、`judge_before/after`、`draw_count`、`hand_limit`、`equip_change`、`card_lost`。
- **确定性排序**（必须显式实现，禁止依赖容器迭代顺序）：
  1. 技能声明的时机优先级（数值小先结算）；
  2. 距离当前回合玩家的座位顺序（顺时针）；
  3. 注册顺序（同优先级、同座位时）。
- 嵌套触发统一进**结算栈**；技能产生的询问进 `pending_queue`，逐个解决；任何一步都不允许"递归直接改状态"。
- 事件注册表随状态保存（`skill_subscriptions`），用于重放与调试；注册/注销在技能获得/失去时发生。
- 随机（洗牌/判定/随机弃牌）必须入事件（ADR-0004/0008）。

### 5. 合法动作与交互链

- **两级动作**：
  1. 粗动作（≤26，给决策模型/Web 列表）：`play:<card_instance>`、`skill:<code>[:arg]`、`end_play`、`respond:<card_instance>`、`pass`；
  2. 子交互（引擎 pending）：选目标、选弃牌、选牌堆顶排序、二次确认等。
- **Web 为交互基座**（ADR-0020）：动作只能基于 `player_area` / 授权 `special_zones` 发起；公共区只读；`execute_actions` 批量提交作为后续优化，不阻塞首版。
- **Q 群后置**（P7）：只做开局设定、轮次/状态、牌局链接；不播报公共局面、不做逐动作交互；交互原语在 P2.4 先行实现。
- 手牌/牌堆等隐藏信息走 `player_view` + 一次性代码（沿用拉斯维加斯机制）；`public_state` 抹除未公开身份与手牌明细；视图固定三分区（`public_board` / `player_area` / `special_zones`）。
- 动作空间超限时：引擎提供**预剪枝候选**（≤26）给 SystemOne/Web 列表；完整合法集只进日志（ADR-0019），不改变 `legal_actions` 的「当前步骤」语义。
- 装备槽位显式化：`equip_slot`（武器/护具/进攻马/防御马）进入牌定义与状态，距离/替换判定不靠猜牌名。

### 6. 模式接缝（国战预留，不实现）

- `mode` 层预留：暗将/双将/势力胜利/野心家判定所需的字段（`revealed`、`second_general`、`faction_team`）；
- 身份局 MVP **不写入**这些字段的语义，只保证状态 JSON 可演进（`schema_version`）；
- 国战立项时新增 `mode/guozhan.rs`，不得改动身份局规则代码。

## 理由（取舍）

| 方案 | 结论 |
|---|---|
| 数据驱动规则 DSL（中台配置触发/结算） | 否：调试困难、边界爆炸、无法覆盖嵌套触发；与「中台只管元数据」冲突 |
| 内嵌脚本（Lua/WASM）写武将 | 否（首期）：多一套运行时与沙箱，收益在武将数量极大后才显现 |
| 每个模式独立 crate | 否：牌/技能/伤害内核重复，跨模式无法复用 |
| 在 `game-engine-core` 里直接实现三国杀 | 否：污染通用契约，拉低其他引擎的编译与测试速度 |
| **独立 crate + 模式插件 + 代码技能 + 事件总线** | **采纳**：可测试、可回放、可逐步扩包；与现有 MCP/事件溯源契约一致 |

## 影响

- 新增 crate 需在 `crates/game-engine-mcp` 注册；Q 侧 `games.ts` 注册放 P7（后置）。
- 需扩充交互能力：`many(min/max)`、多目标选择、跨玩家响应队列、有序选择（观星）——**P2.4 先行落地**，见开发计划 §7。
- 视图三分区（ADR-0020）：`public_board` / `player_area` / `special_zones`；Web P3 消费，工作台 P5 复用。
- 状态 JSON 体积增大：每次动作一次快照；P2 测量写放大（风险表 #10）。
- 武将开发成本仍在代码侧：工作台只降低设计/验证成本，不替代实现（ADR-0018）。
- 反向约束：任何「在中台加个字段就能改规则」的需求，默认拒绝；确需数据化时另立 ADR。

## 依据与验证状态

- 复用事实来源：`crates/game-engine-core/src/lib.rs`（EnginePlugin 契约）、`crates/engines/las-vegas/src/sg.rs`（pending 队列/加密选项）、`docs/conventions/interaction-primitives.md`（原语），均为本仓库现状，**已验证**。
- Q 群被动回复 5 条/消息、5 分钟窗口：来源 `docs/conventions/external-constraints.md`（官方文档 + 实测），**已验证**；本 ADR 不新增外部数值约束。
- 国战规则细节（双将/暗将/珠联璧合/野心家）来源为立项文档 + 参考页，**待 P0 规则落库时核对**；本 ADR 只承诺接缝，不承诺实现。
