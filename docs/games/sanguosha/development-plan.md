# 三国杀立项：开发计划

> 状态：**v0.2 待评审**（2026-10-25，已并入用户反馈）。依据 `docs/games/sanguosha/plan.md` 制定；数据模型见 `data-model.md`，关键决策见 ADR-0016 ~ ADR-0020。
> 原则：**先规则、再中台、再引擎、再 Web（交互基座）、再工作台、最后 Q 侧与上线**；每个阶段有可验收产物，未通过不进入下一阶段。

## 1. 目标与边界

### 1.1 目标

- 复刻**标准身份局**（5–8 人）：Web 页承担局面展示与交互，Q 群负责开局设定与状态/链接查询。
- 第一阶段交付**桌游道具中台**（素材/描述/元数据/生命周期）。
- 为**国战**等后续模式预留引擎接缝（本期不实现）。
- 为武将回合/响应流程提供**设计工作台 + 预览仿真**，避免纯靠提示词开发武将。
- 机器人沿用 SystemOne「在当前合法动作里选最优」；对局可录制、可回放，为后续 AI 训练积累数据。
- 多端（Web / WebView / 未来 APP）复用同一套视图与交互，按**公共局面区 / 玩家个人资源区 / 特殊交互区**三分（ADR-0020）。

### 1.2 范围（已确认）

| 维度 | 首版 | 后续 |
|---|---|---|
| 模式 | 标准身份局 5–8 人 | 国战、双将/暗将/珠联璧合、野心家 |
| 牌堆 | **标准包**跑通整体流程；军争牌定义可先录入 | 军争玩法（酒/火杀/雷杀/藤甲/铁索等）在 P4 启用 |
| 武将 | 首批验证武将 + 标准包 25 将 | 风林火山 → 一将成名 → 界限突破 |
| 技能类型 | 一般 / 锁定 / 限定 / 觉醒 / 主公技 | 新类型按枚举扩展 |
| 端 | **Web 优先**（公共区 + 个人资源区 + 特殊区），多端 WebView 复用；Q 后置、只做开局/状态/链接 | APP / PWA 交互增强 |
| 权鉴 | 先沿用短期 token；**完整权鉴放上线阶段**（ADR-0020） | 个人资源区隔离、动作级权限 |
| AI | SystemOne 选动作 + 启发式剪枝 | 训练/自对弈强化 |

### 1.3 明确不做

- 本期不做国战玩法、不做运营系统、不做素材在线绘制。
- 工作台**不是运行时**：n8n 流程图不参与线上对局执行（ADR-0018）。
- Q 群侧不播报公共局面、不做逐动作交互（只做开局设定、轮次/状态、牌局链接）。
- 不使用官方图片素材（素材后续自行制作）。

## 2. 现状与差距

| 能力 | 现状 | 三国杀需要 |
|---|---|---|
| 引擎框架 | `game-engine-core`：`EnginePlugin` / lobby / 事件溯源 / RNG 注入 / public_state / player_view | 复用；补「技能事件总线 + 可组合回合阶段 + 大动作空间 + 视图三分区」 |
| 引擎实现 | 拉斯维加斯（完整）、猜拳（玩具） | 新增 `crates/engines/sanguosha` |
| 道具中台 | 无（只有版图素材流水线 `board-assets.mjs`） | 新增 DB + REST + SPA + seed + 定义↔物理牌关联页 + 生命周期 |
| Web | Express SSR + 纯 TS 视图 IR（ADR-0010，不引入 React/Vite） | 新增公共局面区/个人资源区/特殊区；交互前置；WebView 兼容 |
| QQ 命令 | `#开局` 桌游列表（已支持多桌游） | 后置：仅开局、状态/轮次、链接 |
| 交互原语 | 单选/多选（仅加密代码）/分区/级联/接力/异步等待语义 | 补 `many` min/max、目标多选、排序、跨玩家响应队列（**引擎先行**） |
| AI | `Policy` + SystemOne 动作选择（≤26） | 大动作空间剪枝、完整合法集日志、录制回放 |
| 工作台 | 无 | n8n 复用评估 + 预览仿真 + 模板 |

## 3. 总体架构

```
        ┌──────────────────────────────────────────────────┐
        │  Web 玩家页（公共局面区 / 个人资源区 / 特殊交互区） │
        │  ← 同一套视图契约，WebView 复用（ADR-0020）        │
        └───────────────▲──────────────────────────────────┘
                        │ HTTP（短 token，后续补权鉴）
┌──────────┐   MCP   ┌──┴───────────────────────┐  SQLite  ┌────────────────┐
│ agent-   │────────>│ game-engine-mcp          │────────->│ 真相源          │
│ gateway  │<────────│  └ sanguosha 引擎（Rust） │          │ game_state     │
└────┬─────┘         └──────────▲───────────────┘          │ game_events    │
     │ Q 侧后置（开局/状态/链接） │ 只读/仿真                  │ prop_* / sgs_* │
     ▼                          │                          └────────────────┘
  QQ 群                 apps/props-admin（Vue3）
                        apps/sgs-workbench（P5，设计/仿真）
                              │
                        n8n（图编排宿主，仅设计期）
```

- **规则内核只读**：中台与工作台都不能在运行时改规则；规则变更必须走代码 + dev 流程（ADR-0016）。
- **单写者**：道具与状态写入统一经 `crates/server`；SPA/Web 只读或走受控写接口。
- **视图三分区**：公共局面区（只读）/ 个人资源区（交互基座）/ 特殊交互区（技能声明可见性）。
- **事件溯源**：随机结果、技能触发、判定全部入事件，支持确定性回放（ADR-0004/0008）。

## 4. 阶段计划

> 工作量用相对规模标注（S/M/L/XL），具体日期在 P1 完成后按实际速率校准。

### P0 规则收口与文档落库（M）—— 文档已产出（2026-10-26，待审核）

**目标**：把「三国杀标准身份局」的规则、牌表、武将表、交互口径写成可执行的 repo 文档，消灭歧义。

交付物（`docs/games/sanguosha/`）：

- `rules.md`：身份局完整流程（发身份/选将/体力/回合阶段/判定/濒死/死亡奖惩/胜负）。
- `cards.md`：牌堆清单（已生成：标准 108 + 军争 52；社区数据待人工复核）。
- `generals.md`：武将表（**已由 `data/heroes.json` 自动生成**，按标准/风林火山等分包、每包一个表格；性别缺失待补）。
- `state-machine.md`：回合与全局状态机（含技能插入点、视图三分区）。
- `legal-actions.md`：动作 id 命名规范与合法动作清单（含动作数控制策略）。
- `rule-engine-permission.md`：校验链与权限表（沿用拉斯维加斯结构）。
- `rules-rulings.md`：已裁定规则台账（内奸/双内、主杀忠、响应顺序、主公体力 +1、无超时等；2026-10-25 已确认）。
- `special-zones.md`：特殊交互区登记表与设计模板（观星/五谷丰登/判定/濒死/无懈链）；所有特殊区域必须登记后实现。
- 规则来源与裁定台账：moegirl 参考页 / 官方说明 / 用户裁定（逐条标注）。
- **数据源结论**：官网武将数据**已全量抓取**（696 位 / 1383 技能，`data/heroes.json`），并生成 `generals.md`；官网**无公开结构化牌表**，按用户裁定**牌表人工录入**；社区开源牌堆数据已整理为 `cards.json` / `cards.md`，**待人工逐张复核**（见 `data/README.md`）。

验收：

- 文档中每个歧义点都有**明确裁定**或列入「待确定」；`data-model.md` §7.3 确认完毕。
- 用户逐条审核通过规则口径（尤其是内奸/双内、死亡奖惩、无懈可击链、距离与装备、无超时）。
- 牌表导入方案有结论（可导入 / 手工录入工作量评估）。

依赖：无。**阻塞后续所有阶段。**

### P1 道具中台（L）

**目标**：可维护三国杀全部道具元数据；图片字段先留空。

交付物：

- 存储：`prop_packs` / `prop_assets` / `sgs_identities` / `sgs_identity_setup` / `sgs_card_defs` / `sgs_cards` / `sgs_generals` / `sgs_skills` / `sgs_general_skills`（见 `data-model.md`）。
- 生命周期：所有道具/武将/技能统一 `status`（新建/测试/审核/上线/下线软删）；只读接口只取 `live`。
- server REST：`/internal/gameprops/*`（CRUD + 列表/搜索/批量导入 + 管理员白名单 + 审计字段）。
- SPA：`apps/props-admin`（Vue 3 + Vite + TS + Naive UI），页面：
  - 身份配置、牌定义、物理牌、**牌定义 ↔ 物理牌关联页**（按定义筛选、批量生成物理牌、绑定花色点数）、武将、技能、包管理；
  - 状态流转（新建→测试→审核→上线→下线）+ 审核记录（`prop_status_log`）。
- 武将初始化：用 `apps/scripts/crawl-sanguosha-heroes.mjs` 产出的 `heroes.json` 幂等导入（696 位 / 1383 技能；`external_id` 为官网 gid）。
- 牌表初始化：官网无公开结构化牌表（P0 结论），按用户裁定**人工录入**；`cards.json`/`cards.md` 作为社区参考，逐张复核后入库。
- seed：身份 + 人数配比 + 标准牌定义 + 首批武将（描述/技能来自抓取数据，人工复核类型与性别）。
- 生命周期审计：`prop_status_log` 记录每次状态迁移/审核（新建→测试→审核→上线→下线）。
- 素材：仅登记表与字段；后续接 `board-assets`（素材自行制作，不用官方图）。

验收：

- 用 SPA 完成「新增牌定义 → 批量生成物理牌 → 填花色点数 → 关联校验」与「新增武将 → 绑定技能 → 走完生命周期」。
- 写操作经 server，无直连 SQLite；seed/导入可重复执行（幂等）。
- 单元测试：REST 校验（重复 code、非法枚举、血量约束、状态流转）、迁移幂等。

依赖：P0。

### P2 引擎内核 + 标准身份局（XL）

**目标**：跑通一局无技能/少量技能的 5 人身份局（发身份、摸牌、出杀闪桃、装备、判定、濒死、胜负）。

- **P2.1 骨架**：`crates/engines/sanguosha` + `EnginePlugin` 接入 + lobby（5–8 人）+ 发身份/选将（主公 N 选 1、其他同时 M 选 1，N/M 可配置）+ 牌堆洗牌（种子入事件）+ 状态 JSON `schema_version=1` + `config`（`max_hand_cards=20` 等）+ **视图三分区投影**（`public_board` / `player_area` / `special_zones`）。
- **P2.2 回合流程**：准备/判定/摸牌/出牌/弃牌/结束；**判定区 LIFO（后进先判）**；距离（无上限；攻击距离=武器+进攻马+技能，防御距离=基础+防御马+技能；顺手/兵粮不含武器范围）；装备槽位（武器/护具/进攻马/防御马/特殊）；**持有上限 20（可配置，超出即弃）/弃牌上限=当前体力值**；基本牌（杀/闪/桃）。
- **P2.3 结算栈 + 锦囊 + 胜负**：
  - **结算栈**：`resolution: Vec<Frame>`（state schema v2），统一驱动「选目标 → 无懈链 → 效果 → 响应 → 伤害/濒死」；`pending` 为栈顶投影（对外契约不变）。
  - **无懈可击链**：询问按座位顺序的**有手牌存活玩家**（**无手牌者自动跳过**，2026-10-28 修订；有手牌但无无懈者仍显式 `pass` 以保留博弈——见 `special-zones.md` §4.1）；每次有人打出无懈从原使用者左手侧重新开始；允许同一玩家多次；层数奇偶决定效果是否被抵消；**AOE 每个目标各开一次窗口**。
  - **延时锦囊**：置入判定区**不可无懈**；判定阶段**判定前**开无懈窗口，被无懈则不判定（乐/兵进弃牌堆；**闪电流转到左手边第一个玩家**）。
  - **单目标锦囊**：顺手牵羊（距离不含武器；共用特殊区 `pick_card`，手牌随机）、过河拆桥（无距离；同一 `pick_card`）、无中生有、决斗（目标先出杀、双方交替；仅开始前一次无懈）、借刀杀人（二阶段目标：有武器者 → 其攻击范围内；**即使无杀也必须询问**并由 A 选具体杀）。
  - **AOE**：南蛮入侵（目标出杀）、万箭齐发（目标出闪）、桃园结义（仅受伤者，回血前开无懈）；逐目标、从使用者左手侧开始、死亡跳过；伤害来源 = 使用者。
  - **死亡奖惩与胜负**：主公杀忠臣弃置所有手牌和装备、杀反贼摸 3 张；内奸/双内、主公先死 → 反贼胜；死亡立即判定胜负。
  - 五谷丰登留 P2.4/P2.5（特殊区 `wugu`，释放者先选）；技能/改判/可选装备效果留 P2.4；兵粮寸断随军争 P4。
- **P2.4 技能内核 + 交互原语**：技能内核 + 触发钩子（确定性派发：技能声明序 → 左手侧座位序 → 注册序；实现口径为「注册表 + 钩子函数」而非 trait 对象，见 `skills.md` §1）+ pending 队列；**响应顺序统一从当前玩家左手侧开始**（通用原则）；**原语先行**：`many(min/max)`、多目标选择、**`ordered`（槽位 + 最终顺序，提交 `arrange:<csv>`）**、跨玩家响应队列（闪/无懈/桃）；实现首批验证武将（建议：刘备/曹操/孙权/貂蝉/黄月英/张辽/甘宁/吕蒙，含主公技）。
- **P2.5 特殊区域骨架**：`special_zones` 投影 + `guanxing` 契约（`slots` 2×5 / `rev` / `interaction.kind=ordered` / `arrange` 校验，**不含 UI**）+ `wugu` 设计稿落地准备；交互规格见 `ui-spec.md` §7。

**P2.1 实施记录（2026-10-27）**

- crate `crates/engines/sanguosha`：`state.rs`（schema v1）/ `data.rs`（自动生成：标准 108 牌 + 25 将）/ `view.rs`（三分区）/ `lib.rs`（`EnginePlugin`）/ `tests.rs`（16 用例）；已在 `crates/game-engine-mcp` 注册。
- 动作：`lobby:join/leave/kick:<id>/add_bot/start/abort/set_lord_candidate_count|set_other_candidate_count|set_max_hand_cards|set_double_spy`、`setup:choose_general:<code>`；`add_bot` 机器人自动选将。
- 牌堆：按定义表建 108 张实例（`c1..c108`）→ 用状态内 RNG 洗牌（`deck.seed` 入 `deck_shuffled` 事件）→ 每人发 4 张；`check_conservation` 断言牌数守恒。
- 视图：`get_state.data = { schema_version, game, phase, public_board, player_area: null, special_zones }`；`get_player_view` 额外带 `player_area` 与本人可见 `special_zones`；选将期间隐藏他人武将/候选，非主公身份不公开。
- 生成/校验：`node apps/scripts/gen-sanguosha-engine-data.mjs`（`--check` 纳入 CI）；MCP 冒烟：`node apps/verify-sanguosha.mjs`。
- 已知限制：内置武将池仅标准 25（多人数候选按均分降级，P4 扩池）；Q 侧 `games.ts` 注册留 P7，Web 页留 P3（P2.1 只交付数据契约）。

**P2.2 实施记录（2026-10-27）**

- `turn.rs`：阶段机 `prepare → judge → draw → play → discard → end` 自动推进；无玩家输入时连续推进，停在**出牌阶段**或 `pending`。
- 判定区 **LIFO**：乐不思蜀（非红桃 → 跳过出牌阶段）、闪电（♠2–9 → 3 点雷电伤害，否则移动到下家判定区）；判定牌与结果写入 `special_zones.judge`（`public`，保留到回合结束）并产出 `judge` 事件。
- 动作：`play:<card>`（杀/桃/装备/乐不思蜀/闪电）、`target:<player>`、`end_play`；响应 `respond:<card>` / `pass`；弃牌 `discard:<csv>`（支持一次弃满或逐张）。
- 杀：每出牌阶段 1 次（诸葛连弩无限）；目标须在攻击距离内；目标出【闪】否则受 1 点伤害；【仁王盾】黑色杀无效（被动）。
- 距离：`攻击距离 = 武器 range_value（无武器 = config.base_attack_range） + 进攻马 1`；`防御距离 = 防御马 1`；只计存活角色，顺/逆取小。
- 持有上限：摸牌后 > `max_hand_cards` 立即进入 `discard` pending；弃牌阶段上限 = 当前体力值（最低 0）。
- 濒死：`phase=dying` + `pending`（从当前玩家左手侧起、当前玩家最后，仅询问持有【桃】的玩家；可多次救援直到体力 > 0 或无人可救）；无人救援 → 死亡（弃置所有牌、身份公开）。
- 未实现（后续阶段）：锦囊（决斗/过河/顺手/无中/南蛮/万箭/桃园/五谷/借刀/无懈）、可选装备效果（八卦阵/贯石斧/青龙偃月刀/麒麟弓/寒冰剑/雌雄双股剑/丈八蛇矛/方天画戟）、死亡奖惩与胜负判定（P2.3）、改判与技能（P2.4）。
- 测试：`cargo test -p sanguosha` 32 用例；MCP 冒烟追加回合推进（约 10 回合 / 31 步，牌数守恒）。

**P2.3 实施记录（2026-10-27）**

- **结算栈**：`state.resolution: Vec<Frame>`（Trick/Sha/Wuxie/PickCard/Respond/Dying/Discard/Judge）+ `outcome` 子帧结果；`pending` 为栈顶投影。`schema_version=2`。
- **无懈链**：询问所有存活玩家（不跳过无手牌者）；每次有人打出无懈 → 层数 +1 并从原使用者左手侧重新询问；同一玩家可多次；层数奇偶决定抵消；AOE 每个目标各自开窗口；无懈牌进弃牌堆。
- **延时锦囊**：置入判定区不可无懈；判定阶段**判定前**开无懈窗口，被无懈则不判定（乐/兵进弃牌堆；闪电流转到左手第一个无闪电的玩家）。
- **单目标锦囊**：顺手牵羊（`trick_distance_ok`：不含武器）、过河拆桥（共用特殊区 `pick_card`，手牌随机/装备槽/判定区）、无中生有、决斗（开始前一次无懈，之后交替出杀）、借刀杀人（二阶段目标 + 无武器禁用 + 无杀也必须询问并由 A 选具体杀）。
- **AOE**：南蛮/万箭/桃园逐目标（使用者左手侧、死亡跳过）；每目标独立无懈 → 响应（杀/闪）→ 伤害；桃园满血跳过且不询问无懈；伤害来源 = 使用者。
- **死亡奖惩/胜负**：杀反贼摸 3、主公杀忠弃置所有手牌装备；主公先死 → 反贼胜；仅剩内奸 → 内奸胜；主公存活且反/内全灭 → 主忠胜。
- **未实现**：五谷丰登（`wugu` 特殊区，P2.4/P2.5；当前不可打出）、技能/改判/可选装备效果（P2.4）、兵粮寸断与军争（P4）。
- **测试**：`cargo test -p sanguosha` 49 用例（含 400 步随机对局逐步牌数守恒）；`node apps/verify-sanguosha.mjs` 跑通至终局（含无懈链与锦囊）。

**P2.4 第一批实施记录（2026-10-27）**

- 技能内核：`skills.rs`（武将→技能注册表、固定派发顺序）+ `resolve_skill.rs`（技能帧）。
- 已实现：奸雄（`choice`）、制衡（`choose_many` + `skill:zhiheng`）、救援、闭月、集智、奇才、克己。
- 钩子：`phase_start(end)`、`card_used(锦囊)`、`damage_after`（仅存活时）、`hand_limit`、`trick_distance`、`tao_heal`、主动技。
- 视图：`player_area.skills` 改为结构化 `{code,name,type,implemented}`（Web/P5 工作台可直接用）。
- 未实现（P2.4 后续）：仁德、激将、护驾、突袭、奇袭、离间；见 `skills.md` §4。
- 测试：`src/tests/p24.rs` 7 个技能用例；全套 56 用例通过。

**P2.4 第二批实施记录（2026-10-27）**

- 新增技能：仁德（`choose_many` → `target`，累计 ≥2 张回血）、激将 / 护驾（`Requisition` 同势力接力帧，失败回到本人响应）、离间（弃 1 张 + 两名男性角色视为决斗）、突袭（摸牌阶段替代：`choice` + 至多两名目标各取 1 张手牌）、奇袭（黑色牌转化为过河拆桥）。
- 数据：`GeneralDef.gender`（标准包女性名单维护在生成脚本），供离间/装备效果使用。
- 首批 8 位武将技能**全部实现**；`cargo test -p sanguosha` 65 用例（新增 9 个技能用例，含主公技无敌对势力、突袭放弃摸牌、离间每阶段一次等边界）。
- 剩余：每武将 ≥3 条规则用例的主公技边界补齐；五谷丰登（P2.5）；其余武将（P4）。

**P2.4 收尾记录（2026-10-27）**

- 装备可选效果：青釭剑（无视防具，`ignore_armor`）、八卦阵（`skill:bagua` 判定，红色视为【闪】）、仁王盾与诸葛连弩回归验证；寒冰剑/雌雄双股剑/贯石斧/青龙偃月刀/丈八蛇矛/方天画戟/麒麟弓 **转 P4**（需 `damage_before` 窗口、多目标【杀】、双牌转化等扩展，见 `skills.md` §5）。
- 用例补齐：首批 8 将均 ≥3 条规则用例（刘备/曹操/孙权/貂蝉/黄月英/吕蒙/张辽/甘宁），含「无同势力不提供主公技」「离间每阶段一次」「克己出杀后不生效」「突袭提前确认」「奇袭转化仍有无懈窗口」等边界。
- `cargo test -p sanguosha`：**77 用例**通过；MCP 冒烟至终局；workspace 全绿、无告警。

**P2.5 实施记录（2026-10-28）**

- **五谷丰登 `wugu`（已实现）**：`resolve_zone.rs` 新增 `Frame::Wugu`。亮出 = 存活角色数 张（全部进 `deck.processing`）→ 写 `special_zones.wugu`（`public`：`cards/choosers/cursor/current/picked/interaction`）→ **释放者先选**、之后按座位左手侧依次 `pick:<card_id>`；**每个选择者各自一次无懈窗口**（被无懈者跳过）；结束把剩余亮牌与【五谷丰登】本身置入弃牌堆；获得后超过持有上限立即 `discard`（`hand_cap`）。动作：`play:<牌>` → pending `wugu` → `pick:<card_id>`。
- **观星 `guanxing`（P2 契约已实现）**：`Frame::Arrange`。拥有「观星」的玩家**准备阶段自动进入**（不询问是否发动、不可取消，每回合一次，`guanxing_done` 计数）；取 `k = min(存活角色数,5)` 张进处理区，写 `special_zones.guanxing`（`owner` 可见、`rows:5`、`cols:2`、`slots: L1..L5/R1..R5`、`rev`、`interaction = {kind:ordered, commit:arrange}`）。提交 `arrange:<csv>`（最终顺序 = 新牌堆顶 → 底），**校验张数/去重/来源/仍处理区/slots 一致**，非法与重放一律拒绝且不改状态；回写按倒序压入 `deck.draw`。`settle_slots()` 纯函数落地（跳空压缩、左列接右列）。
- 视图：`special_zones` 继续按 `visibility` 下发（`wugu` 公共、`guanxing` 仅 owner）；公共视图不含观星。回合结束时清理 `judge/wugu/guanxing` 残留。
- **响应询问口径修订（2026-10-28 用户裁定，`special-zones.md` §4.1）**：响应类询问（无懈链/闪/杀/求桃/主公技接力）——**无手牌且无替代选项的玩家自动跳过**（不生成 pending，直接按「不出」结算）；**有手牌者一律显式询问**（即使没有对应牌，唯一动作 `pass`），不代选。实现：`step_wuxie`/`step_respond`/`step_dying`/`step_requisition` 过滤 + `respond_give_up` 统一「显式 pass / 自动跳过」结算路径。
- 测试：`tests/p25.rs` 新增 12 个用例（五谷顺序/无懈/剩余弃置/可见性/守恒；观星槽位/可见性/非法与重放拒绝/回写顺序/纯函数；空手自动跳过、有手牌仍询问、无懈链空手跳过、求桃空手跳过、空手盟友不提供主公技）；全套 **89 用例**通过。
- MCP 冒烟：`apps/verify-sanguosha.mjs` 增加特殊区断言（`wugu` 为公开区且 `current == pending.actor`；`arrange` 时 owner 个人视图含 10 槽、公共视图无观星），实测覆盖 `wugu_zone/wugu_pick/guanxing_arrange`。
- 口径偏差：观星「可以发动」按 **自动进入** 实现（用户已定「不可取消」）；**空城** 留 P4。

**P2 收尾记录（2026-10-28）**

- **事件溯源重放**：`tests/replay.rs`——跑一局并记录 `(玩家, 动作)` 脚本 + 产出事件，再用同 seed 的新管理器重放同一脚本，断言**终态 JSON 与事件序列完全一致**（另有一条「重复重放稳定」用例）。补上了原验收里「事件溯源重放 = 快照重算」的空档（此前只有 setup 确定性与 400 步守恒）。
- **`check:sgs-view-contract` 门禁**：`tests/view_contract.rs`——字段级冻结三分区（`view_data` / `public_board` / `seat` / `player_area` / `card` / `pending` / `options` / `skills`）与特殊区（`wugu` / `guanxing`：`visibility`/`slots`/`interaction`/`picked` 等），并断言隐藏信息不泄漏（公共区无 `hand`、公共无观星、个人区只投影本人）。npm 脚本：`npm run check:sgs-view-contract`。
- **`check:interaction-primitives` 门禁**：`tests/interaction_contract.rs`——引擎下发的 `interaction` 只能是已登记原语 `one/many/partition/ordered` 且 `commit` 非空、`ordered` 必带 `slots`；`choose_many` 校验 `min<=max`；`pending.kind` 必须在 `state-machine.md` §2.2 登记表内。npm 脚本：`npm run check:interaction-primitives`。
- **格式化门禁**：CI 增加 `cargo fmt --check -p sanguosha`（sanguosha 包已全量格式化）。**全仓** `cargo fmt --check` 仍有 156 处历史未格式化（las-vegas 115 / server 17 / game-engine-mcp 10 / core 4 / bot-adapter 3 / docs-mcp 1 / rps 6），属既有技术债，不在 P2 范围，见计划台账 harness。
- 当前 `cargo test -p sanguosha`：**98 用例**（89 + 重放 2 + 视图契约 3 + 原语门禁 4）。

验收（P2 / M18）：

- 固定种子可复现：同 seed + 同动作序列 → 同结果。
- 无技能 5 人局可完整结束（含内奸胜利分支）；牌数守恒不变量测试。
- 视图分区断言：公共视图无手牌/未公开身份；个人视图含本人手牌；特殊区按可见性下发。
- 观星契约：`slots` 回写顺序正确（跳空压缩、左列接右列）、`arrange` 非法/重放被拒、牌数守恒（含处理区）。
- 五谷丰登：释放者先选 + 座位顺序、每选择者一次无懈、剩余牌进弃牌堆、公开可见性、牌数守恒。
- 技能武将各 ≥3 条规则用例；事件溯源重放 = 快照重算（`tests/replay.rs`：状态 + 事件序列双比对）。
- 视图/原语门禁：`check:sgs-view-contract`、`check:interaction-primitives`（CI 覆盖）。
- 锦囊用例：无懈单层/三层嵌套、同一玩家连续响应、AOE 每目标独立无懈、判定前无懈（乐/兵/闪电）、顺手/过河三区选择、借刀二阶段与「无杀仍询问」、决斗交替出杀。
- MCP 冒烟：`create_game` → lobby → `lobby:start` → 多步出牌 → 终局。

依赖：P1（道具 seed / live 数据）、P0（规则）。

### P3 Web 交互基座（L，提前，先于工作台）

**目标**：局面与交互在 Web 上跑通，成为各端（含 WebView）的统一基座。

交付物：

- `apps/web` 新增三国杀页面，按 ADR-0020 分区：
  - **公共局面区（只读）**：座位/公开身份/武将/体力/装备/判定区/弃牌堆/牌堆余量/当前轮次阶段/公共特殊区；
  - **玩家个人资源区（交互基座）**：本人手牌、技能/可用动作、个人 pending、加密选项代码、私有标记；
  - **特殊交互区**：观星、五谷丰登、判定、濒死、无懈链等；按 `special-zones.md` 登记与设计，`visibility` 支持 `owner/public`（预留 `all/team`）。
- **布局（`ui-spec.md` §3）**：左右分栏——左辅助区（出牌历史/阶段，手机可折叠）、右侧游戏区上下（公共区 + 个人武将区）；他人座位 `左/上/右` 按人数公式分配（8 人 = 2/3/2），百分比定位 + `clamp()` autosize。
- **手势（`ui-spec.md` §5）**：Pointer Events；短按=点击（判定在 `up`）、长按(≥400ms)=详情、长按拖动=观星排序；位移 >8px 取消；未选中时「确认打出」置灰；不依赖 HTML5 DnD。
- **浮层（`ui-spec.md` §6）**：所有特殊区与详情统一居中浮层（单层、遮罩、滚动锁、Esc/返回关闭）。
- **观星（`ui-spec.md` §7）**：2 列 × 5 槽；点击移至另一列首个空槽；拖动吸附（槽短边 10%）落空槽放入 / 有牌交换 / 超距回原位；一次提交 `arrange`。
- 短 token + `get_player_view` 同源；隐藏信息严格按视图契约；越权 403。
- **端侧基线**：Chromium ≥ 88 / iOS Safari ≥ 14；禁用 `:has()`/容器查询/`dvh`；响应式 + autosize；轮询刷新沿用 10s + 激活即刷；**端侧壳延后**，先用手机浏览器验收，再决定 Tauri/RN。
- 交互 API：动作提交 + pending 步进 + 取消；UI 采用「草稿 + 按 pending 逐条提交」；`execute_actions` 批量提交**无必需场景时不做**（本阶段仅预留接口）。
- 无 JS 降级文本视图（含观星文字顺序提交）。

验收：

- 同一局面下 Web 与 MCP `state_summary` / 视图契约一致（字段级快照测试）。
- 公共区只读、个人区可交互；手牌/身份/牌堆不泄露。
- 布局快照三档（PC / 手机竖屏 / 手机横屏）无溢出；手势单测（短按/长按/位移/`up` 判定）通过；观星纯函数用例通过。
- 手机浏览器（Android/iOS）+ 桌面 Chromium 各一次冒烟；端侧壳（Tauri/RN）不在本阶段验收范围。
- 权鉴：短 token 过期提示与续签路径可用（完整权鉴留 P8）。

依赖：P2.1–P2.3（状态与视图稳定）。

**P3.0 实施记录（2026-10-28，数据与动作通路）**

- **投影修复**：`server /resources/game/{gameId}` 改为按 `game_id` 分派引擎（`server` 注册 `las-vegas` + `sanguosha`），未注册引擎不再原样返回内部状态；新增 `resources_routes::ensure_room`（内存无房间时按需从 `game_state` 恢复）。
- **动作通路（方案 A，单写者）**：`server POST /internal/game/{gameId}/{roomId}/action`（仅本机）→ `GameManager::execute_action` → `game_events` + 快照（version 对齐最后 seq）；`web POST /tabletop_game/:gameId/action`（短 token 校验 `resource == game:{room_id}`）→ JSON（fetch）或 `303` PRG（无 JS 表单）。
- **本地脚手架**：`apps/scripts/sgs-web-room.mjs`（MCP 建局→选将→停在出牌 → `/auth/issue` 签 token → 打印每人链接；`--dump-state` 导出 `{public, players}` 前端 fixture）。
- **验证**：sanguosha 公共视图无 `hand`、`player_area=null`、无他人特殊区；Web JSON 提交 `end_play`/`discard:<id>` 生效（turn_index 推进、快照 version 递增、事件入库）；跨房间 token → 403；无 JS 表单（token 在 body）→ 303；`npm run sgs:web-room` 可一键起局。

**P3.1 实施记录（2026-10-28，视图层 + SSR）**

- 新增 `apps/web/src/view/sgs/`：`types.ts`（三分区契约）、`format.ts`（牌面/文案/座位公式）、`public.ts`（公共区只读 + 左侧辅助区）、`personal.ts`（个人区：手牌/装备/技能/判定/操作条/观星与五谷占位）、`text.ts`（无 JS 文本态 + pending 文本提交表单）、`page.ts`（页面骨架 + CSS，三档断点）。
- 座位布局按 `ui-spec.md` §3.2 公式：`splitSides(4..7)` = 1/2/1、1/3/1、2/3/1、2/3/2；填位顺序「左列自下而上 → 上排自左而右 → 右列自上而下」，百分比定位 + `clamp()` 尺寸。
- web 路由分派：`GET /tabletop_game/:gameId/player/:openid` 按 `data.game === "sanguosha"` 走新页（拉斯维加斯双模式不动）；客户端 bundle `public/assets/sgs.js`（es2017）负责 10s 轮询/激活刷新/退避/状态条。
- 测试：`apps/web/test-sgs.mts`（座位公式、牌面与图标、公共区无手牌、个人区手牌/技能/禁用按钮、观星/五谷占位、文本态表单与 `arrange:` 提示、整页 bootstrap/noscript、禁用特性 `:has()`/容器查询/`dvh`、昵称 XSS 转义）。
- 真机布局实测（Playwright + 系统 Edge）：1366×900、1920×1080、390×844 三档**无横向溢出**；公共区按比例撑满、个人区贴底、操作条在手机端贴底；公共区 0 张手牌正面、个人区 6 张手牌正面。

**P3.2 实施记录（2026-10-28，浮层基座 + 指针手势）**

- **手势**（`apps/web/src/client/sgs-gestures.ts`）：`classifyPress` 纯函数（短按 <400ms、长按 ≥400ms、位移 >8px 取消、`pointercancel` 不产生点击、长按到时后 up 不再点击）；`bindPress` 事件委托（`data-press` 元素），长按到时即反馈（`.pressed` 描边）。
- **浮层**（`view/sgs/overlay.ts` 纯模板 + `client/sgs-overlay.ts` DOM 管理）：单层、遮罩、`body.overflow=hidden` 滚动锁、Esc / 遮罩 / 关闭按钮 / **浏览器返回键**关闭、焦点移入关闭按钮并在关闭后还原；打开时 `history.pushState`，关闭 `history.back()`（返回键只关浮层）。
- **详情内容**（`view/sgs/detail.ts`）：牌面（花色点数/类别/装备槽/攻击距离）、武将（玩家/武将/势力/公开身份/体力/手牌数/手牌上限/状态）、弃牌堆列表（顶→底）。为支持「弃牌堆全量列表」，引擎公共视图新增 `public_board.discard`（公共信息），`check:sgs-view-contract` 字段集同步。
- **客户端接线**：手牌短按选中/取消（`.selected` + 操作条「已选：牌名」，提交在 P3.3）；长按手牌/装备/判定小标记打开牌面详情；点他人武将打开武将详情；点弃牌堆打开列表；首帧立即拉 `/state`（详情依赖客户端数据），浮层打开时轮询不打断界面。
- **实测（Playwright + Edge，390×844）**：长按 460ms → 浮层打开、标题「桃♥8」、滚动锁生效、焦点在关闭按钮、`history.state.sgsOverlay=true`；Esc → 关闭 + 滚动恢复 + `history.back()` 生效；遮罩点击关闭；短按选中/再按取消；位移 30px 不选中。

**P3.3 实施记录（2026-10-28，出牌链路）**

- **合法动作下发给端侧**：`server /resources/game/{gameId}` 新增 `actions`（请求者本人的 `legal_actions`，引擎按权限过滤：非其回合/非其 pending 时为空）；`GameView.actions` 同步。
- **UI 由 actions 驱动**：手牌可出的牌加 `playable` 高亮；「结束出牌」仅在有 `end_play` 时可用；技能按钮在存在 `skill:*` 动作时启用（转化技 `skill:qixi:<card>` 映射到对应牌）；**pending 逐条提交**：`player_area.pending.options` 全量渲染为按钮（`data-action`），覆盖选目标 / 闪·杀·无懈·桃响应 / 显式 `pass` / 弃牌（可逐张点）/ 技能多选（choose/confirm/cancel）/ 接力 / 五谷 / 观星默认顺序。
- **提交流程**：选中 → 「确认打出」（`play:<card>`，未选中或不在 actions 内则禁用）→ `POST /tabletop_game/:gameId/action` → 成功后立即拉 `/state` 重渲染；失败显示原因并自动刷新（处理他端抢先导致的 400/403）；提交期间 `busy` 防重复。
- **实测（Playwright + Edge）**：p4 选【杀】→ 确认 → 目标按钮（p3/p5）→ 切到 p3 页面点「不出」→ HP 4→3、p3 pending 清空；回 p4 点「结束出牌」→ 回合推进到 p5；p5 结束出牌 → 回合推进到 p1（克己自动跳过弃牌）。

**P3.4 实施记录（2026-10-28，特殊区浮层）**

- 新增 `apps/web/src/view/sgs/pending.ts`（纯函数）：无懈链（root/层数/当前询问者）、濒死求桃、五谷（亮牌/已选/当前选择者）、拿牌三组（装备区/随机手牌/判定区）、目标选择（含借刀两阶段提示）、技能多选（勾选态来自引擎 `data.selected`）；`overlayKindFor` / `isBlockingPending` 决定「哪些 pending 用浮层」「哪些不可误关」。
- 浮层支持 **blocking**：无关闭按钮、遮罩点击与 Esc 无效（无懈链/濒死/五谷/拿牌，符合 ui-spec §9.1「必须显式选择」）；目标选择等非阻塞浮层可 Esc/遮罩关闭，个人区操作条仍可继续。
- 客户端：`syncPendingOverlay()` 按 pending 签名（kind+actor+options+data）自动开关/刷新浮层；浮层按钮复用 `[data-action]` 提交链路；**pending 浮层打开时轮询与提交后都会刷新**（详情浮层仍不打断）。
- 修复：`refreshNow` 之前无条件跳过「浮层打开」时的重渲染，导致浮层内提交后停在旧待办。
- 实测（Playwright + Edge）：出【杀】→ 自动弹出「选择目标」浮层（4 个目标，非阻塞、可关）；浮层内点目标 → 提交成功并继续；对方页面 inline「不出」响应正常。

**P3.5 实施记录（2026-10-28，观星 2×5 槽）**

- **纯函数**（`apps/web/src/view/sgs/guanxing.ts`）：`initialSlots`（L1..Lk）、`applyTap`（跨列首个空槽）、`applyMoveWithinColumn`（▲▼ 同列相邻交换）、`applyDrop`（空槽放入/有牌交换/同槽与空返回）、`settle`（每列跳空、左列接右列）、`arrangeAction`、`snapSlot`/`snapThreshold`（槽中心 + 短边 10% 吸附）。
- **引擎**：`special_zones.guanxing` 增加 `cards`（本次取出的牌面，owner 可见），端侧才能渲染点数/花色/牌名；`check:sgs-view-contract` 字段集同步。
- **浮层 UI**：观星作为 **blocking** pending 浮层打开（不可误关、无关闭按钮）；2 列 5 槽、牌面渲染、▲▼、重置、最终顺序预览、`arrange:<csv>` 一次性提交；个人区不再重复渲染观星占位。
- **交互**：短按跨列、▲▼ 同列、长按 380ms 激活拖拽（translate 跟手）→ 松手按吸附阈值判定：空槽放入 / 有牌交换 / 超距回原位；重置回初始分布；提交后浮层自动关闭并继续回合。
- **修复**：拖拽几何取样误把 ▲▼ 按钮（20×18）当成槽位导致阈值失真 → 限定 `.sgs-gx-slot[data-gx-slot]`；拖拽监听改为一次性绑在 `#sgs-overlay`（重渲染不再重复绑定）。
- **实测（Playwright + Edge，390×844）**：初始 L1..L5 五张牌面正确；短按 L1→R1、L2 下移；拖拽同列交换（L1↔L4）、跨列入空槽（L1→R3）、落回原槽无操作、超距回原位；重置恢复；提交 `arrange:c94,c39,c37,c2,c63` → 「观星完成（5 张）」，随后摸牌阶段摸走 **c94、c39**（证明牌堆顶顺序回写正确）。

**P3.6 实施记录（2026-10-28，降级与验收）**

- **无 JS 文本态实测**：页面 SSR 内含完整文本区（公共/个人/观星左右列文字 + `arrange:<csv>` 提示）与 `POST /tabletop_game/:gameId/action` 表单（隐藏 token/room_id + 动作输入 + datalist）。用 `curl` 以 **urlencoded 表单**提交 `arrange:c54,c106,c19,c79,c105` → `303` 回页面并带「观星完成（5 张）」；随后摸牌阶段摸走 **c54、c106**，证明无 JS 文字顺序提交可用。
- **四档布局**（Playwright + Edge）：1366×900 / 1920×1080 / 390×844 / 844×390 均**无横向溢出、无座位裁切**；桌面常驻辅助区、窄屏折叠为抽屉；新增「矮视口（≤520px）」紧凑样式：公共区 178px、个人区 135px、**操作条 fixed 贴底**（横屏手机可见）。
- **e2e 整链（浏览器点击，非仅单测）**：
  - 出牌链路：选牌 → 确认 → 目标浮层 → 对方「不出」→ 伤害 → 结束出牌 → 回合推进（P3.3 已验，本批复验）；
  - **五谷丰登**：出牌 →（无懈窗口 API 放行）→ 释放者 blocking 浮层选牌 → 拿牌 +1、剩余 4 张；其余 4 人依次选完 → 特殊区移除、无剩余牌；
  - **拿牌选择**：顺手牵羊 → 目标浮层 →（无懈窗口）→ blocking「选择一张牌」三组浮层 → 选「手牌（随机 1 张）」→ 获得【桃♥8】；
  - **无懈链**：南蛮入侵 → 无懈链浮层（root「【南蛮入侵♠K】→ 孙权」、0 层、当前询问者、显式不出）→ 打出无懈（第 1 层）→ 链重开 → 全员不出 → 日志「【南蛮入侵】被无懈可击抵消」，首个目标未受伤；
  - **观星**：P3.5 已验（点击/▲▼/拖拽/重置/提交后摸牌顺序正确）。

**P3 修复/优化记录（2026-10-28，人工试用反馈）**

- **Bug：侧栏出牌历史不随动作更新（刷新后才正常）**。根因：`#sgs-app` 只包住公共区 + 个人区，**左侧辅助区（阶段/本局/历史）在 `#sgs-app` 之外**，轮询/提交后的局部重渲染不会更新它。修复：`renderSidePanelBody()` 抽出正文，SSR 外壳为 `<aside id="sgs-side">`；客户端 `applyState` 同时刷新 `#sgs-side` 的 `innerHTML`。e2e 增加断言：点击「结束出牌」后**不刷新**页面，`#sgs-side` 历史条数与阶段条必须变化。
- **优化：他人手牌背面堆叠**。每张背面用负 margin（`-18px`）叠放、最多画 8 张，真实张数继续用 `×N` 标注，节省座位卡片宽度；e2e 在四档视口断言第二张背面 `margin-left < 0`。

**P3 修复记录（2026-10-30，第二轮用户反馈）**

- **出牌历史**：视图不再只下发最近 50 条（全量下发，引擎上限 200 → **800**）；左辅助区**倒序**（最新在上）+ 标题显示总条数；轮询/提交重渲染时保留历史列表 `scrollTop`，整页刷新用 `sessionStorage` 恢复页面与列表滚动位置。新增 `npm run sgs:history -- --room <id>`（读 SQLite 快照打印完整历史，支持 `--tail/--grep/--json/--list`）。
- **闪电**：修正为**只对自己使用**（`start_trick` 置 self-target，不再要求选目标），置入自己判定区并在下回合 LIFO 最先判定；此前误走通用「选目标」分支可指定他人。
- **顺手牵羊 / 过河拆桥**：目标必须至少有一张可拿的牌（手牌/装备/判定区）；`legal_actions` 与目标候选同步过滤，无合法目标时不可打出（此前可出现「打出后无事发生」）。
- **麒麟弓**：实现【杀】造成伤害后，攻击方可弃置目标一张坐骑（`choice`：`skill:qilin:horse_offense|horse_defense` / `pass`，仅目标存活且有坐骑；非【杀】伤害不触发）。
- **历史可读性**：无懈链结束日志带上 root（如「无懈可击：【南蛮入侵♠K】→ 孙权 效果被抵消」）；判定日志带上判定者；去掉逐张弃牌的「还需弃 N 张」中间日志。
- **八卦阵滥用（复盘日志发现）**：此前每次需要【闪】可以反复点「八卦阵判定」，直到判红为止（实测一杀判 3 次）。修复：`RespondFrame.bagua_used` 标记，一次响应只能判一次（黑色失败后只能正常出闪或不出）。
- **历史可读性（第二轮）**：历史条目显示操作者昵称前缀（`LogEntry.actor` → 前端解析），复盘时不再需要猜「谁用了这张牌」。
- 用例：`tests/p26.rs` 5 条（闪电自身/判定顺序、顺手过河目标过滤、麒麟弓弃坐骑/决斗不触发、历史全量下发）+ `p24c` 八卦阵单次判定；全套 **103** 用例通过。

### P4.5 装备效果技能化重构（ADR-0022，已立项）

**目标**：把装备效果从各时机点的内联 if/专用帧，迁移为「装备技能」注册表（`SkillOwner::Equipment` + `Trigger`），窗口仍由引擎开、效果由技能实现统一派发。

**范围（迁移清单）**：诸葛连弩 / 八卦阵 / 麒麟弓 / 寒冰剑 / 青龙偃月刀 / 贯石斧 / 雌雄双股剑 / 丈八蛇矛。

**前置**：通用判定窗口 `judge_before`（铁骑/刚烈/八卦/洛神统一走它，支持鬼才改判）先落地。

**交付**：
- `skills.rs`：`SkillOwner` + `Trigger` + `triggered()`；`has()` 同时查装备技能；
- 删除各调用点的 `has_equip` 内联分支与重复胶水，新增装备只加 `SkillDef` + 效果函数；
- 视图 `player_area.equipment_skills[]`（供 Web 武器高亮/浮层入口）+ `view_contract` 同步；
- 回归：`cargo test -p sanguosha`、`check:sgs-view-contract`、`check:interaction-primitives`。

**边界**：窗口类装备（伤害前/被闪后/判定前）不提供「提前点击」，端侧在窗口内把选项画在武器卡上；中台只读元数据（ADR-0016）。

### P4 武将 / 军争扩展（XL，持续）

**目标**：先补齐标准包 25 将，再按包扩展；军争牌玩法启用。

交付物：

- 标准 25 将技能实现 + 规则用例 + `generals.md` 更新。
- 军争内容：酒、火杀、雷杀、藤甲、铁索等牌定义与结算（P1 可先录入定义）。
- 技能类型全覆盖：主公技 / 一般 / 锁定 / 限定 / 觉醒。
- 后续包：风林火山 → 一将成名 → 界限突破（每包独立开关 + 生命周期）。

验收：

- 每武将 ≥3 用例 + 至少 1 个跨技能交互用例；`cargo test -p sanguosha` 全绿。
- 包上线不影响旧包对局（包开关 + `live` 状态）。
- 军争牌规则用例通过（若启用）。

依赖：P2.4、P3。

**P4 开工口径（2026-10-30 用户确认）**

1. **牌堆**：**标准 + 军争 = 160 张**（军争 52 张定义已在 `data/cards.json`）；`Config` 默认即此组合。
2. **包开关**：**全局 config（一局一个包组合）**，不做逐玩家/逐牌开关；旧对局按创建时的 config 固定。
3. **武将数据源**：**P4 先引擎内置多包生成**（生成脚本按 `pack` 产出）；中台 live/元数据只读 API 留 P5。
4. **改判窗口**：**判定牌亮出后自动弹 pending**；询问顺序**从判定者左手侧开始、拥有改判技能者先被询问**（没有改判技能者不询问）；可打出改判牌或 `pass`。

**P4 前置清单（2026-10-30 梳理；①②③④ 已按上表拍板）**

1. **数据/牌堆扩展（生成脚本，必做）**
   - `apps/scripts/gen-sanguosha-engine-data.mjs` 目前**只生成标准 108 张 + gid 1..25**；`docs/games/sanguosha/data/cards.json` 已有 `standard=108` / `junzheng=52`，`heroes.json` 696 将含 `pack`（标准27/风林火山48/一将成名72/界限突破87/国战110…）。
   - 需补：军争 `DEF_CODE` 映射 → 生成 `JUNZHENG_DECK`（火杀/雷杀/酒/兵粮寸断/藤甲/铁索连环/古锭刀/朱雀羽扇/骅骝/白银狮子/火攻…）；`GeneralDef.pack` 与按包生成；`--check` 门禁覆盖新数据。
   - 引擎：`Config` 增加包/牌堆模式（`standard` | `standard+junzheng`），`finish_setup` 按模式建堆；包开关不得影响旧对局。
2. **引擎机制缺口（P4 武将/装备依赖）**
   - 改判窗口 `judge_before/after`（司马懿鬼才/郭嘉天妒；`skill:<改判>` 选项 + 出牌）。
   - `damage_before` 窗口（寒冰剑、藤甲、裸衣等）；多目标【杀】（方天画戟）；双牌转化（丈八蛇矛）；被闪后追加结算（青龙偃月刀）；性别相关（雌雄双股剑）。
   - 军争结算：属性伤害/连环状态（铁索连环传递、藤甲火伤 +1/南蛮万箭无效）、酒（使用 +1 伤害 / 濒死当桃）、兵粮寸断（判定区 + 跳过摸牌）、火攻（亮牌+弃同花色）。
   - `SkillType` 已有 Limited/Awaken/Locked 枚举，但缺通用「限定次数 / 觉醒条件 / 锁定强制」执行与视图字段。
3. **选将池与元数据（与 P5 工作台共用）**
   - 多包候选池 + `status=live` 过滤；需定「引擎内置数据 vs 中台 live 下发」边界（ADR-0016/0017）：P4 先内置多包生成，P5 再做只读元数据 API。
   - 标准 25 将余 17 个（含诸葛亮空城、司马懿改判）与本轮已实现的 6 件装备效果同批补。
4. **验收与 UI 配套**
   - 每将 ≥3 用例 + ≥1 跨技能交互；包开关回归；军争规则用例；`check:sgs-engine-data` 扩军争/多包。
   - Web 侧：技能多选/主公技接力/限定技按钮的浏览器实测（P3 已有 `choice/choose_many/target` 通道与浮层，缺实测）；新增触发窗口若无现成 pending 形态需登记 `state-machine.md` §2.2 与 `check:interaction-primitives`。

**P4 建议拆分**：P4.0 数据/牌堆(160)/包开关(config) → P4.1 标准 25 将余 17（含空城、改判 pending）→ P4.2 装备效果剩余 6 件 + 机制窗口（damage_before/多目标/转化/追加）→ P4.3 军争牌结算 + 属性/连环 → P4.4 包扩展（风林火山→…）与验收。

**P4.0 交付锚点（新会话直接照此开工）**：生成脚本支持 `junzheng` 牌 + `GeneralDef.pack`；引擎新增 `JUNZHENG_DECK` 与 `Config.packs`/牌堆模式（默认 `["standard","junzheng"]` → 160）；`finish_setup` 按 config 建堆；牌数守恒断言覆盖 160；`check:sgs-engine-data` 扩围；文档 `data-model.md` schema 同步。

**P4.0 实施记录（2026-10-30，已完成）**

- **生成脚本**（`apps/scripts/gen-sanguosha-engine-data.mjs`）：
  - 牌：`DECKS` 表驱动 `standard`(108)/`junzheng`(52)，DEF_CODE 补齐军争 11 种定义（酒/火杀/雷杀/火攻/兵粮寸断/铁索连环/白银狮子/藤甲/古锭刀/朱雀羽扇/骅骝）；跨包同名定义（桃/闪/无懈可击）字段一致性校验。
  - `PhysicalCard.pack` + `STANDARD_DECK`/`JUNZHENG_DECK` + `FULL_DECK_SIZE=160` + `deck_cards(&packs)`（未知包/空组合报错，堆内 `pack` 自检）。
  - 武将：`GENERAL_PACKS` 登记表按包生成，`GeneralDef.pack`；默认仍只生成标准包 25 将，`--general-packs` 为 P4.4 扩包预留（新包需登记性别表）。
- **引擎**：`Config.packs`（serde 默认 `["standard","junzheng"]`）+ `parse_packs`/`packs_mode`（顺序无关、去重、军争必须带标准）；`lobby:set_packs:standard|standard+junzheng`（房主、开局后不可改）；`finish_setup` 按 config 组堆，`deck_shuffled` 载荷加 `packs`；牌数守恒与 `deck_shuffled.size` 均按实际堆大小（160/108）。
- **军争未实现牌拦截**：酒/火杀/雷杀/火攻/铁索连环/兵粮寸断/古锭刀/朱雀羽扇/藤甲/白银狮子 — 定义已入库但结算未实现，`legal_actions` 不列出且 `play:` 直接报错（避免默认 160 牌堆触发内部错误或静默按普通牌结算）；骅骝走通用距离逻辑可直接装备。
- **测试**：`cargo test -p sanguosha` **108** 用例全绿（新增 `tests/p40.rs` 5 条：默认 160 / 切换 standard=108 / 包配置校验与规范化 / `deck_cards` 顺序与错误 / 未实现军争牌不可出 + 骅骝可装备）；`p23` 弃牌断言改为增量（160 牌堆下初始手牌可能含乐不思蜀副本）；`apps/verify-sanguosha.mjs` 守恒断言按 `board.config.packs` 计算（160/108）。
- **门禁**：`npm run check:sgs-engine-data` 通过（160 张：标准 108 + 军争 52 / 25 将）；`cargo fmt --check -p sanguosha` 通过。
- **文档**：`data-model.md` 新增 §3.7 引擎内置数据 + §4.1 `config.packs`；`state-machine.md` §1/§2.1 建堆口径；`legal-actions.md` §3 `lobby:set_packs` + §4.2 军争拦截；`rules.md` 牌池与建堆。
- **harness 优化（2026-10-30）**：① 新增「牌定义 ↔ 引擎结算覆盖」门禁用例（`junzheng_card_defs_are_covered_or_explicitly_pending`：每张 CardDef 必须 ∈ 标准包 / 已实现军争牌 `JUNZHENG_IMPLEMENTED` / `PENDING_JUNZHENG_CARDS`，防新牌静默落入通用分支）；② `apps/verify-sanguosha.mjs` 打印 `packs/总张数` 并断言军争未实现牌从未以 `play:` 出现（端到端守恒 + 拦截双保险）。用例总数 **109**。
- **下一阶段入口**：P4.1（标准 25 将余 17 + 空城 + 改判 pending）

**P4.1 实施记录（进行中，2026-10-30）**

- **首批（已完成）**：改判窗口 + 鬼才（司马懿）+ 空城（诸葛亮）。
  - 判定帧重构：`JudgeStage = Wuxie → WaitWuxie → Reveal → Rewrite → WaitRewrite → Effect`；判定牌亮出即进 `deck.processing`（牌数守恒）并在特殊区 `judge` 公开；改判替换的旧牌进弃牌堆。
  - `judge_rewrite` pending（已登记 `state-machine.md` §2.2 与交互原语门禁）：`respond:<任意手牌>` / `pass`；询问顺序 = 判定者左手侧起、**判定者最后**；只问拥有改判技能且手牌非空者（无手牌不询问）。
  - 空城：`skills::kongcheng_blocks` 统一过滤【杀】目标（`sha_targets`）、【决斗】目标（`legal_actions` + 锦囊目标 pending）、借刀杀人的虚拟【杀】目标、离间视为决斗的防守者；南蛮/万箭等非杀锦囊不受影响。
  - 用例：新增 `tests/p41.rs` 8 条（改判生效/放弃/询问顺序含判定者最后、空城杀/决斗/借刀/有手牌可指定/非杀锦囊不受限）；`cargo test -p sanguosha` **117** 全绿；`check:sgs-view-contract`、`check:interaction-primitives`、MCP 冒烟通过。
- **批次 B（转化/防御，部分完成 2026-10-30）**：
  - 已实现 **武圣**（红牌当杀：出牌阶段主动 `skill:wusheng:<card>` + 响应）、**龙胆**（杀闪互转，主动 + 响应）、**倾国**（黑牌当闪，响应）、**急救**（回合外红牌当桃，濒死 `dying` pending）。
  - 转化统一走 `skills::conversion_for`（实际牌 ≠ 目标牌才成立，并按有无该技能/颜色/回合外校验）；响应路径在 `resolve::respond_options` 下发选项、`finish_respond_play`/`act_dying` 重校验，未下发选项不可伪造。
  - **国色暂缓**：方块牌当【乐不思蜀】要求判定区保存「虚拟牌定义」（当前判定区按物理牌 def 结算，转换牌会被当成原牌误判），待引入 `virtual_defs` 后与军争转化（火攻/朱雀羽扇 P4.2/P4.3）一起做。
  - 用例：新增 `tests/p42.rs` 8 条；`cargo test -p sanguosha` **125** 全绿；视图/原语门禁与 MCP 冒烟通过。
- **批次 C1（锁定/触发，已完成 2026-10-30）**：咆哮（杀无次数限制）、马术（距离 -1）、英姿（摸牌 +1）、裸衣（**手动触发**：摸牌阶段 `choice` 询问，发动=少摸 1 张且杀/决斗伤害 +1，放弃=正常摸 2 张）、天妒（获得判定牌）、谦逊（不能成为顺手/过河目标）、连营（失去最后手牌摸 1）、枭姬（失去装备摸 2）。
  - 自动/锁定化实现：天妒、连营、枭姬按自动发动（口径见 `skills.md` §8）；**裸衣为手动触发**（`Frame::Luoyi` + `choice` pending，可放弃并正常摸 2 张）。
  - 用例：新增 `tests/p43.rs` 9 条（含裸衣发动/放弃）；`cargo test -p sanguosha` **134** 全绿；视图/原语门禁与 MCP 冒烟通过。
- **批次 C2-1（主动/伤害后，已完成 2026-10-30）**：苦肉（失去 1 体力摸 2，可多次；1 体力进入濒死）、青囊（弃 1 手牌令已受伤角色回复 1，每阶段一次）、结姻（弃 2 手牌，与已受伤男性各回复 1）、反馈（受伤后获得来源一张牌，`pick_card` 不含判定区）、遗计（受伤后摸 2 并逐张分配，任意角色含自己）。
  - 口径偏差：遗计按「每次伤害结算」触发一次（规则为每 1 点伤害），待 P4.2 `damage_before` 统一；分配交互已完整实现。
  - 修复要点：反馈的 `pick_card` 压在【闪】响应帧之上，不得覆盖父帧 `outcome`（已加保护）。
  - 用例：新增 `tests/p44.rs` 6 条；`cargo test -p sanguosha` **140** 全绿；视图/原语门禁与 MCP 冒烟通过。
- **批次 C2-2（判定/响应，已完成 2026-10-30）**：无双（杀需两张闪；决斗对方每次需两张杀，`RespondFrame.need/played`）、铁骑（杀指定目标后判定，红色封锁闪）、刚烈（受伤判定非红桃→来源弃2手牌或受1伤）、洛神（准备阶段反复判定黑色获得）、反间（目标选花色→获得随机手牌→花色不同受1伤）。
  - 口径偏差：铁骑/刚烈/洛神为**自动判定**（不开放改判窗口），待 P4.2 通用 `judge_before` 判定帧统一接入。
  - 用例：新增 `tests/p45.rs` 6 条；`cargo test -p sanguosha` **146** 全绿；视图/原语门禁与 MCP 冒烟通过。
- **流离（大乔，已完成 2026-10-30）**：成为【杀】目标时可弃 1 张手牌，将【杀】转移给自己攻击范围内、非使用者的其他角色；`Frame::Liuli`（Choice 选牌 / Aim 选目标），转移后重算响应（无双 need 继承、铁骑封锁不继承）。口径偏差：弃置费用限手牌（不支持弃装备）。
  - 用例：新增 `tests/p46.rs` 3 条；`cargo test -p sanguosha` **149** 全绿。
- **剩余批次（待做）**：
  - 国色（大乔）：待 `virtual_defs`（判定区虚拟牌定义）。
  - D 收尾：标准 25 将全覆盖回归（每将 ≥3 用例 + 跨技能交互）。**当前 25/25 武将所有至少 1 个已实现技能**（大乔另有国色待做）。
- **批次 D（回归收口，已完成 2026-10-30）**：
  - 新增 `tests/p47.rs` 12 条：龙胆响应决斗、马术+顺手距离、苦肉+持有上限、反间同花色免伤、流离排除使用者/空城、突袭+连营、鬼才+天妒、刚烈弃 2 张 / 红桃不触发、倾国+无双、无双+龙胆、枭姬+顺手拿装备。
  - 新增门禁 `npm run check:sgs-skill-cases`（`apps/scripts/check-sgs-skill-cases.mjs`）：扫描 `src/tests/*.rs` 的 `#[test]` 函数体，按 `sgs_hero_<gid>` / 技能 code 统计，要求标准 25 将**每人 ≥3 条**（当前最低 3）。
  - 顺手补齐：苦肉摸牌后的持有上限检查、遗计分配结束后的持有上限检查。
  - 验证：`cargo test -p sanguosha` **161** 全绿；`check:sgs-skill-cases`、`check:sgs-view-contract`、`check:interaction-primitives`、`fmt` 通过；MCP 冒烟通过。
- **国色（大乔，P4.1 收尾完成 2026-10-30）**：引入 `SgsState.virtual_defs`（牌实例 → 生效定义码）+ `effective_def_code`；方块手牌当【乐不思蜀】(`skill:guose:<card>`)，判定区同名检查与判定结算按生效定义，`virtual_defs` 仅用于判定区/判定上下文（不改变物理牌）。
  - 用例：新增 `tests/p48.rs` 3 条（国色生效/拒绝非方块/已有乐不思蜀目标排除）；`cargo test -p sanguosha` **164** 全绿。
  - **P4.1 收口（2026-10-30）**：标准 25 将 / 标准包全部技能（49 个技能 + 5 件装备效果）已实现；D 回归门禁 `check:sgs-skill-cases` 通过（25/25 每人 ≥3 用例）。
  - **下一步**：P4.2（装备效果剩余 6 件 + `damage_before`/多目标【杀】/双牌转化/被闪追加，另可用通用 `judge_before` 判定帧统一铁骑/刚烈/洛神/国色等自动判定与改判窗口）→ P4.3（军争结算：酒/属性杀/火攻/铁索/兵粮/藤甲/白银狮子 — 完成后移除 `PENDING_JUNZHENG_CARDS` 对应项）→ P4.4（包扩展）。
- **P4.2 进展（2026-10-30，部分完成）**：
  - **被闪后追加窗口**已落地：`RespondFrame.is_sha` + `Frame::AfterDodge`（`resolve_skill`）。**青龙偃月刀**：【杀】被【闪】抵消后可对该目标再使用一张【杀】（新【杀】入处理区并重算无双/铁骑，原【杀】先行结算避免 outcome 串帧）。**贯石斧**：可弃 2 张手牌使此【杀】依然造成伤害（`MultiSelect` 增加 `data` 携带 target/card 后调用 `apply_damage`）。
  - 用例：新增 `tests/p49.rs` 3 条（青龙连杀 / 贯石斧弃 2 命中 / 无武器无追加窗口）；`cargo test -p sanguosha` **167** 全绿。
  - **寒冰剑 / 雌雄双股剑（已完成 2026-10-30）**：`Hanbing` 帧走进 `respond_give_up`（damage_before，防止伤害改弃目标 2 牌，目标不足 2 牌不询问；弃置随机）；`Cixiong` 帧统一入口 `push_sha_target_response`（异性目标先二选一：弃 1 手牌 / 攻击方摸 1，再进入【闪】响应；流离转移后同样生效）。
  - 用例：新增 `tests/p50.rs` 6 条（寒冰防止/放弃/不足两张、雌雄弃牌/摸牌/同性不触发）；`cargo test -p sanguosha` **173** 全绿；门禁与 MCP 冒烟通过。
  - **丈八蛇矛（主动使用，已完成 2026-10-30）**：出牌阶段 `skill:zhangba:<c1>,<c2>` 两张手牌当【杀】（受出杀次数限制；第二张直接进弃牌堆为简化口径）；响应时双牌转化留后续。
  - 用例：新增 `tests/p51.rs` 3 条；`cargo test -p sanguosha` **176** 全绿。
  - **通用判定窗口 `judge_before`（首阶段完成 2026-10-30）**：新增 `Frame::QuickJudge`（Reveal→Rewrite→WaitRewrite→Effect，复用 `judge_rewrite` 与鬼才询问顺序，`saved_outcome` 归还父帧结果）；**铁骑、刚烈**已迁移，支持鬼才改判；洛神/八卦阵仍为直接判定（待下一阶段迁移）。
  - 用例：新增 `tests/p52.rs` 2 条（铁骑红→黑改判后可闪、刚烈非红桃→红桃改判不触发）；`cargo test -p sanguosha` **178** 全绿。
  - **方天画戟（已完成 2026-10-30）**：`ShaFrame.pending_targets` 顺序结算 + `targets:<p1,p2[,p3]>` 组合选项；仅当【杀】为使用者最后一张手牌（使用时判定 `fangtian`）可额外指定 2/3 个目标；追加目标走统一响应入口（无双/铁骑/雌雄生效，不重复触发流离）。
  - 用例：新增 `tests/p53.rs` 3 条；`cargo test -p sanguosha` **181** 全绿。
  - **洛神 / 八卦阵迁移 `QuickJudge`（完成 2026-10-30）**：八卦判定改由 `QuickJudge(reason=bagua)`，红色经 `finish_respond_dodge` 结算；洛神每次 `choose:luoshen` 走 `QuickJudge(reason=luoshen)`（黑色自动获得并重新询问，红色结束）。四类技能判定（铁骑/刚烈/洛神/八卦）全部支持鬼才改判。
  - 用例：新增 `tests/p54.rs` 2 条；`cargo test -p sanguosha` **183** 全绿。
  - **丈八蛇矛响应转化（完成 2026-10-30）**：需要【杀】时（南蛮/决斗/借刀）下发 `respond:<c1>,<c2>` 双牌选项；`act_respond` 校验并弃置第二张，复用转化日志。
  - 用例：新增 `tests/p55.rs` 2 条；`cargo test -p sanguosha` **185** 全绿。**P4.2 全部装备效果已完成**（8 件）。
  - **后续**：P4.3 军争牌结算。
- **P4.3 首批（已完成 2026-10-30）**：**酒**（出牌阶段限一次，下一张【杀】+1；濒死可自救）、**火杀/雷杀**（`ShaFrame.damage_type` 传递火/雷属性）、**藤甲**（火焰 +1；普通杀/南蛮/万箭免疫）、**白银狮子**（伤害改为 1）、**古锭刀**（目标无手牌时杀 +1）；军争装备已登记 `EQUIPMENT_SKILLS`。`PENDING_JUNZHENG_CARDS` 缩减为 火攻/铁索连环/兵粮寸断/朱雀羽扇。
  - 用例：新增 `tests/p62.rs` 5 条；`cargo test -p sanguosha` **201** 全绿；门禁/前端单测/MCP 冒烟通过。
  - **兵粮寸断（完成 2026-10-30）**：延时锦囊，距离 ≤1，判定非黑桃跳过摸牌阶段；`PENDING_JUNZHENG_CARDS` 再缩减为 火攻/铁索连环/朱雀羽扇。用例 `tests/p63.rs` 2 条。
  - **朱雀羽扇（完成 2026-10-30）**：PlayPhase 注册表新增 `zhuque_yushan`（普通杀 → 火杀，`ShaFrame.damage_type=Fire`）；`PENDING_JUNZHENG_CARDS` 缩减为 火攻/铁索连环。用例 `tests/p64.rs` 2 条；`cargo test -p sanguosha` **205** 全绿。
  - **火攻（完成 2026-10-30）**：目标亮一张手牌 → 使用者可弃同花色手牌造成 1 点火焰伤害（`Frame::HuoGong`，可放弃；无懈窗口/藤甲按火焰伤害处理）；`PENDING_JUNZHENG_CARDS` 仅剩 铁索连环。用例 `tests/p65.rs` 2 条；`cargo test -p sanguosha` **207** 全绿。
  - **铁索连环（完成 2026-10-30）**：`Seat.chained` 横置/重置；重铸 `recast:<card>`（弃置摸 1）；火/雷属性伤害在同横置角色间传导（传导前全部重置，避免级联）。`PENDING_JUNZHENG_CARDS` **清空**，军争 52 张全部接入结算。用例 `tests/p66.rs` 3 条；`cargo test -p sanguosha` **210** 全绿。
  - **P4.3 状态：完成**（酒/属性杀/火攻/铁索/兵粮/藤甲/白银狮子/古锭刀/朱雀羽扇 全部实现；16 件装备效果 + 9 军争牌）。
- **P4.5 阶段一（已完成 2026-10-30）**：`skills.rs` 新增 `Trigger` / `EquipmentSkillDef` / `EQUIPMENT_SKILLS`（17 件）/ `has_equipment_skill()` / `triggered()`；所有装备判断调用点由 `has_equip` 统一改为 `has_equipment_skill`（8 武器 + 2 防具 + 坐骑）。用例 `tests/p56.rs` 3 条（注册表覆盖 / 装备后生效 / 按时机收集确定性）。
  - **阶段二（已完成 2026-10-30）**：`player_area.equipment_skills[] = {slot, code, name, card, trigger}`（仅本人视图，公共区不下发）；AfterDodge（青龙/贯石）与 DamageBefore（寒冰剑）窗口改为 `triggered(Trigger)` 派发；`view_contract` 已同步冻结字段。
  - 用例：新增 `tests/p57.rs` 2 条；`cargo test -p sanguosha` **190** 全绿；`npm --prefix apps run test:web` 通过。
  - **阶段三（已完成 2026-10-30）**：AfterDodge 窗口改为**效果函数注册表** `AFTER_DODGE_HANDLERS`（`{code, options, act}`，青龙/贯石），窗口按 `triggered(Trigger)` 收集处理器并顺序执行；新增装备只需登记 handler。用例 `tests/p58.rs` 2 条（注册表覆盖 / 无第二张杀不开窗口）。
  - **阶段三（续，完成 2026-10-30）**：`DAMAGE_BEFORE_HANDLERS`（寒冰剑：`available` + `effect`，选项与效果均由注册表提供）与 `TARGET_CHOSEN_HANDLERS`（雌雄双股剑：异性目标可用性）；窗口统一经 `triggered(Trigger)` 收集处理器。用例 `tests/p59.rs` 2 条；`cargo test -p sanguosha` **194** 全绿。
  - **阶段四（收尾，完成 2026-10-30）**：**PlayPhase** 注册表 `PLAY_PHASE_HANDLERS`（丈八蛇矛 options/act），出牌阶段选项按 `triggered(PlayPhase)` 派发；方天画戟仍由 `triggered(TargetChosen)` + `ShaFrame.fangtian` 驱动（多目标修正不属于选项类 handler）。用例 `tests/p60.rs` 1 条；`cargo test -p sanguosha` **195** 全绿。
  - **Web 武器高亮（完成 2026-10-30）**：窗口 pending 增加 `data.option_sources`（选项 → 装备 code）；个人区武器卡按 `equipment_skills` + `option_sources` 渲染 `data-equipment-skill` 与 `.sgs-equip.active`；丈八在出牌阶段按 action 前缀高亮。前端单测覆盖。
  - **P4.5 状态：全部完成**（注册表 17 件 + 六类 Trigger + 四个 handler 表 + `equipment_skills` 视图 + 武器高亮）。

### P4.4 包扩展（下一会话开工口径，2026-10-30 确认在新会话完成）

**目标**：在 P4.0–P4.3 已完成（160 牌堆、标准 25 将全技能、标准 8 件 + 军争装备、军争 52 张结算、装备技能化）的基础上，扩展武将包并验收。

**现状/前置**：
- 生成脚本已有 `GENERAL_PACKS` 登记表（`--general-packs`）与 `GeneralDef.pack`；默认只生成标准 25 将。
- `heroes.json` 包分布：风林火山 48 / 一将成名 72 / 界限突破 87 / 神将 16 / 谋 27 / 门阀士族 34 / 魔 6 / 国战 110 / 其他 269。
- `skills.rs` 目前只注册标准 25 将技能；新包技能需注册表 + 实现（或先 `implemented:false` 进池）。

**建议拆分**：
- **P4.4.0 数据/开关**：生成脚本登记「风林火山」包（`gids` + `femaleGids`）；`GENERAL_PACKS` 支持多包；`Config.general_packs`（默认 `["standard"]`，全局开关，旧局固定）；选将池按包过滤（`all_general_codes`/`offer_to_lord`/`distribute_offers_to_others`）；`lobby:set_general_packs:standard|standard+wind_fire_forest_mountain`；用例：池子过滤、包切换、旧局不受影响；`check:sgs-skill-cases` 扩到新将（每将 ≥3 用例前先按 `implemented:false` 允许进池不触发）。
- **P4.4.1 风林火山技能**：按武将逐批注册 + 实现（每将 ≥3 用例 + 跨技能交互）；优先锁定/被动类。
- **P4.4.2 一将成名 / 界限突破**：同模式扩包；界限突破与标准同名武将的替换口径需在 ADR/数据模型确认。
- **P4.4.3 验收**：包开关回归（旧包对局不受影响）、`cargo test` 全绿、`check:sgs-*` 全套、MCP 冒烟、对应文档（`skills.md`、`generals.md`、`data-model.md`）同步。

**已知口径**：包开关是一局一个组合（与牌堆 `Config.packs` 同思路）；中台 live/元数据只读 API 仍留 P5；军争/标准装备不再改动。

**P4.4.0 实施记录（2026-10-30，已完成）**：
- 生成脚本 `GENERAL_PACKS` 登记 `wind_fire_forest_mountain`（48 位，官网标签全量），默认生成 standard + 风林火山 = **73 将**；`data.rs` 新增 `WIND_FIRE_FOREST_MOUNTAIN_GENERALS`、`GENERAL_PACK_KEYS`、`general_codes_in_packs()`。
- `Config.general_packs`（默认 `["standard"]`，旧快照 serde 兼容）+ `parse_general_packs` + `general_packs_mode`；选将池按包过滤（`offer_to_lord`/`distribute_offers_to_others`），未实现武将不进池（`skills::general_ready`）。
- lobby 新增 `lobby:set_general_packs:standard|standard+wind_fire_forest_mountain`（描述带当前可用武将数）；开局后不可改。
- `check:sgs-skill-cases` 改为解析 `skills.rs`，只对「注册齐全且全部 implemented」的武将要求 ≥3 用例；新增契约测试 `general_skill_registry_matches_generated_data`。
- 用例：`tests/p67.rs` 5 条；`cargo test -p sanguosha` 全绿；`check:sgs-engine-data` / MCP 冒烟（含包开关）通过。

**P4.4.1 风包 8 将（已完成 2026-10-30）**：
- 机制：`Seat.marks`（不屈「创」，牌数守恒覆盖标记区）、`Seat.flipped`（翻面跳过回合/翻回）、`apply_damage` 天香窗口（`apply_damage_now` 绕过）、`Frame::Guhuo` 蛊惑质疑、`ShaFrame.no_distance/virtual_card`（神速虚拟杀）、鬼道扩展改判窗口。
- 技能：魏延（狂骨/奇谋）、夏侯渊（神速）、曹仁（据守/解围）、小乔（红颜/天香）、黄忠（烈弓）、周泰（不屈/奋激）、于吉（蛊惑 + 缠怨）、张角（鬼道/黄天/雷击）。
- 视图：公共/个人区下发 `marks`、`flipped`；`view_contract` 期望集同步；Web 座位/个人区展示翻面与标记。
- 用例：`tests/p68.rs` 21 条、`tests/p69.rs` 5 条；`cargo test -p sanguosha` **242** 全绿；`check:sgs-*` 全套 + `test:web` + MCP 冒烟通过。
- 已知边界（ADR-0023 §3）：鬼道/鬼才仅手牌、解围移动只能到自己区域、奋激触发点有限、神速②仅手牌装备、不屈无牌可翻时不发动。

**P4.4.1 火包 8 将（已完成 2026-10-30）**：
- 机制（ADR-0025）：通用拼点帧 `PindianFrame`（双方选牌→比点→`Outcome::Pindian`，同点发起者未赢）；`virtual_equip`（八阵防具槽空时视为八卦阵，青釭剑可无视）；天义无距离/多目标（`ShaFrame.no_distance/tianyi`）；涅槃限定自救（弃置所有区域 + 复原武将牌 + 摸 3 + 回复至 3）；鞬出目标指定后弃牌分支；双雄判定获得 + 异色当决斗；血裔主公技手牌上限。
- 技能：典韦（强袭）、荀彧（驱虎/节命）、庞统（连环/涅槃）、卧龙诸葛亮（八阵/火计/看破）、太史慈（天义）、庞德（马术/鞬出）、颜良文丑（双雄）、袁绍（乱击/血裔）。
- 用例：`tests/p70.rs` 13 条、`tests/p71.rs` 12 条；`cargo test -p sanguosha` **267** 全绿；`check:sgs-*` + `test:web` + MCP 冒烟通过。
- 已知边界（ADR-0025 §3）：天义与方天组合不做叠加（最多 3 目标）；鞬出遇流离时流离优先；涅槃不重置铁索/限定/觉醒标记；八阵不产生牌实例。

**P4.4.1 林包 8 将（已完成 2026-10-30）**：
- 机制（ADR-0026）：行殇死亡接管（死亡清理前询问，终局不触发）；南蛮免疫/祸首伤害来源/巨象获得；再起/好施摸牌替代；崩坏体力上限减少；帷幕黑色锦囊目标过滤；放逐翻面+摸牌；烈刃拼点父帧 outcome 保护；乱武限定多角色顺序；缔盟交换手牌；完杀/颂威/暴虐/酒池/肉林/截辎。
- 技能：徐晃（断粮/截辎）、曹丕（行殇/放逐/颂威）、孙坚（英魂）、董卓（酒池/肉林/崩坏/暴虐）、祝融（巨象/烈刃）、孟获（祸首/再起）、贾诩（完杀/乱武/帷幕）、鲁肃（好施/缔盟）。
- 用例：`tests/p72.rs` 14 条、`tests/p73.rs` 12 条；`cargo test -p sanguosha` **293** 全绿；`check:sgs-*` + `test:web` + MCP 冒烟通过。
- 已知边界（ADR-0026 §3）：行殇终局不触发；乱武自动取第一张杀/最近目标且不触发目标指定后窗口；好施先选牌再选目标；帷幕只过滤目标枚举。

**P4.4.1 山包 8 将（已完成 2026-10-30）**：
- 机制（ADR-0027）：觉醒/额外技能（`Seat.extra_skills` + `grant_skill`）；屯田标记与距离；巧变阶段跳过；享乐反制；放权额外回合（`SgsState.extra_turn`）；制霸拼点夺牌（Pindian 带牌 id + 魂姿拒绝）；固政弃牌回收（`phase_discarded`）；化身/新生（`huashen` + 技能替换）；悲歌/断肠（`skills_lost`）。
- 技能：张郃（巧变）、邓艾（屯田/凿险）、姜维（挑衅/志继）、刘禅（享乐/放权/若愚）、孙策（激昂/魂姿/制霸）、张昭张纮（直谏/固政）、左慈（化身/新生）、蔡文姬（悲歌/断肠）。
- 用例：`tests/p74.rs` 12 条、`tests/p75.rs` 12 条；`cargo test -p sanguosha` **317** 全绿；`check:sgs-*` + `test:web` + MCP 冒烟通过。
- 已知边界（ADR-0027 §3）：放权自动弃第一张；巧变移动复用解围；化身只取第一个已实现技能且准备/结束各替换一次；固政自动分配；悲歌致命伤害不触发；制霸仅魂姿后可拒绝。

**P4.4.1 SP-1 8 将（已完成 2026-10-30）**：
- 机制（ADR-0028）：转换技（`Seat.flags["yj:<code>"]`，阳默认、发动后切换）；计数标记（`Seat.mark_counts`，怀橘「橘」+ 视图 `marks_count`）；牌堆底摸牌（寸目）/ 置牌堆顶（恃才）；主公技借用（伪帝，仅 kind=Lord 已实现技能）；回合内禁止指定目标（`turn_counters["deny:<user>:<target>"]`）；出牌阶段伤害防止（掣政）；出牌阶段用牌计数（`play_cards_used`）；私有「观看手牌」（审时阴，写 `shenshi_marks` 供回合结束校验）。
- 技能：袁术（庸肆/伪帝）、王基（奇制/进趋）、严颜（拒战·转换）、王平（飞军/兵略）、陆绩（怀橘/遗礼/整论）、孙亮（溃诛/掣政/立军）、蒯越蒯良（荐降/审时·转换）、许攸（成略·转换/恃才/寸目）。
- 用例：`tests/p76.rs` 12 条、`tests/p77.rs` 15 条；`cargo test -p sanguosha` **344** 全绿。
- 门禁（harness，2026-10-30）：`check:sgs-skill-cases` 增加「转换技阳/阴两面均有用例」检查；新增 `check:sgs-skill-docs`（`apps/scripts/check-sgs-skill-docs.mjs`）比对 `skills.md` 技能表 ↔ `skills.rs` 注册（code/name/kind），并补齐首批遗漏行（刘备/曹操护驾/张辽/甘宁/貂蝉离间）；`test:web` 增加视图契约快照（`marks_count` / 转换技阳阴 / 旧快照兼容）。`check:sgs-*` + `test:web` + `test:sgs-web` 通过。
- 已知边界（ADR-0028 §3）：奇制按「每次使用非装备牌」结算一次（不按目标数叠加）；恃才只对基本牌/锦囊触发、装备牌不触发；飞军代价只弃手牌（不支持装备区）；兵略/飞军交牌由目标随机给一张；审时阴「观看手牌」仅写入历史日志；掣政结束弃牌由孙亮选目标、牌随机；立军收益自动生效（未询问孙亮）。

**P4.4.1 SP-2 8 将（已完成 2026-10-30，风林火山 48 将全部落地）**：
- 机制（ADR-0029）：武将牌标记「任/荣/箜声」（复用 `Seat.marks`）；废除装备栏（`Seat.abolished_slots`，永久、弃置原装备、装备技能失效）与废除判定区（`judge_abolished`）；延迟结算队列（`SgsState.scheduled`，镇骨两次触发）；牌移出/移入游戏钩子（`turn::to_void_card`/`from_void_card`）；「不可响应」（`ShaFrame.no_respond` / `TrickFrame.no_wuxie`）与「本阶段不能再使用牌」（`turn_counters["no_card_use"]`）/「本回合手牌被封」（`hand_sealed`）；本回合统计 `damage_dealt`/`discard_done`；特殊区 `zuilun`（观看牌堆顶 3 张）。
- 技能：卢植（明任/贞良·转换）、郝昭（镇骨）、陈到（往烈）、诸葛瞻（罪论/父荫）、周妃（良姻/箜声）、毌丘俭（征荣/鸿举）、陆抗（谦节/决堰/破势）、张绣（雄乱·限定/从谏）。
- 用例：`tests/p78.rs` 13 条、`tests/p79.rs` 13 条；`cargo test -p sanguosha` **369** 全绿。
- 已知边界（ADR-0029 §3）：觉醒授予「清侧」（毌丘俭）与「怀柔」（陆抗）仓库无官方文本 → `implemented:false`（可授予但不生效）；良姻仅响应 `void` 区进出（当前牌堆无移出游戏的牌，实测不触发）；往烈仅支持【杀】与单目标普通锦囊（AOE/决斗不可用）；雄乱封手牌不封装备区与技能（八卦阵/主公技仍可用）；镇骨「至多摸至 5」按 `min(郝昭手牌, 5)` 调整。

**P4.4.2 批次 1（已完成 2026-10-30）**：
- 口径（ADR-0024）：同名武将 code 独立、技能 code 复用、选将池「界优先去重」（`prefer_jiexian` 纯函数 + 单测）、旧局 `general_packs` 不变。
- 数据扩包：生成脚本登记 `yijiang`(72) / `jiexian`(87)（含 femaleGids），`data.rs` 全量 **232** 位；`GENERAL_PACKS` / `GENERAL_PACK_KEYS` 覆盖；包组合菜单新增 `+yijiang` / `+jiexian`。
- 机制：弃牌堆来源钩子 `on_card_discarded`（落英/旋风）、`SgsState.void_return`（破军移出游戏→回合结束回置）、陷阵（`xz_*` 计数：无距离/无次数/无视防具/追加目标/杀不计上限/封锁杀）、制蛮（伤害防止获牌）、补益（濒死展示手牌）、甘露（装备区交换）。
- 技能：曹植（落英/酒诗）、高顺（陷阵/禁酒）、陈宫（明策/智迟）、凌统（旋风）、马谡（散谣/制蛮）、吴国太（甘露/补益）、徐盛（破军）、于禁（镇军）。
- 用例：`tests/p78.rs` 13 条（SP-2 沿用）、`tests/p80.rs` 13 条、`tests/p81.rs` 13 条、`tests/p67.rs` 新增 6 条数据扩包/去重/池子门禁；`cargo test -p sanguosha` **406** 全绿。
- harness（2026-10-30）：新增 `check:sgs-pack-pool`（`cargo test … pack_pool`：包过滤/仅已实现进池/界优先去重/包解析规范化）；`check:sgs-skill-docs` 增加「data.rs 技能名 ↔ 文档技能名」一致性（防 gid 错位）；视图契约新增 `deck.void` 与甘露换装后装备区字段（Rust + Web「移出游戏」计数）。
- 已知边界（ADR-0024 §3 / `rules-rulings.md` §16）：界标 25 将待实现（去重纯函数已就绪）；禁酒不提供响应转化；破军移出游戏期间牌不可被其他效果取得。

**P4.4.2 批次 2（界标·蜀 7 将，已完成 2026-10-30）**：
- 机制：`nonlocked_lost`（本回合非锁定技失效）+ `hand_sealed`（不能使用/打出手牌）组合（义绝）、`paoxiao_bonus` 追击伤害、界仁德「每目标每阶段限一次 + 第二张视为基本牌」（`Frame::RendeBasic`）、界涯角展示牌堆顶并二选一（`Frame::YaJiao`）、界铁骑弃同花色牌否则不能【闪】（`Frame::TieQi`）、界观星张数/牌堆底与结束阶段重复观星（`guanxing_bottom`）、界集智弃基本牌换手牌上限（`jizhi_hand_limit`）、界奇才防止防具/宝物被弃、界武圣方片杀无距离、界龙胆酒/桃互转、界激将代出杀主公摸 1。
- 技能：界刘备（仁德/激将）、界关羽（义绝/武圣）、界张飞（替身/咆哮）、界赵云（涯角/龙胆）、界马超（马术/铁骑）、界诸葛亮（空城/观星）、界黄月英（集智/奇才）；新增模块 `resolve_jiexian.rs`。
- 用例：`tests/p82.rs` 21 条；`cargo test -p sanguosha` **427** 全绿；`tests/p67.rs` 界优先去重断言改为「蜀 7 已替换原版、池仍 25 位」。
- 已知边界（ADR-0024 §3 追加）：义绝「非锁定技失效」按 `skills::has` 统一屏蔽（装备技能不受影响）；观星「全部置底」以特殊区右列为判据（端侧仍按有序提交）；界激将只覆盖「代出【杀】」路径；涯角只覆盖响应打出与【无懈可击】，不覆盖濒死【桃】。

**P4.4.2 批次 3（界标·吴 8 将，已完成 2026-10-30）**：
- 机制：`Frame::FenWei`（多目标锦囊无效子集）、界勤学觉醒 + 授予`攻心`（`Frame::QinXue`/`Frame::GongXin`）、博图额外回合（`discard_suits` 花色掩码 + `mark_counts["botu:<round>"]` + `extra_turn` 允许自己）、界苦肉弃牌版、诈降（失去体力摸 3 / 红杀无距离不可抵消）、界英姿手牌上限=体力上限、界反间（`Frame::JieFanJian`）、界国色限一次+选项二（`Frame::GuoSe`）、界谦逊手牌移出游戏（复用 `void_return`）、界连营栈空延迟询问（`Frame::LianYing`）、界制衡弃光额外摸 1、界救援转移回复、界结姻体力比较版。
- 技能：界甘宁（奇袭/奋威）、界吕蒙（克己/勤学/博图）、界黄盖（苦肉/诈降）、界周瑜（英姿/反间）、界大乔（流离/国色）、界陆逊（谦逊/连营）、界孙权（制衡/救援）、界孙尚香（枭姬/结姻）。
- 用例：`tests/p83.rs` 24 条；`cargo test -p sanguosha` **451** 全绿。
- 已知边界（ADR-0024 §3 追加）：奋威在无懈窗口前询问；博图按 `round` 计次（额外回合不重置）；诈降红杀「不能被抵消」按目标不能响应实现；界谦逊自动发动（不询问）；界连营 X 按失去最后一张 =1；界救援仅覆盖自救【桃】。

**P4.4.2 批次 4（界标·魏 8 将，已完成 2026-10-30）**：
- 机制：界奸雄「摸 1+获得」、界护驾代出闪主公摸 1、鬼才装备改判、界刚烈红伤/黑弃、清俭（`Frame::QingJian`，手牌上限 +1）、界突袭少摸等量、界裸衣亮 3 取基本/武器/决斗 + `luoyi_bonus`（跨回合，准备阶段清除）、恂恂（`Frame::XunXun`）、忘隙（`Frame::WangXi`）、界洛神 `luoshen_free` 不计手牌上限。
- 技能：界曹操（奸雄/护驾）、界司马懿（反馈/鬼才）、界夏侯惇（刚烈/清俭）、界张辽（突袭）、界许褚（裸衣）、界郭嘉（天妒/遗计）、界李典（恂恂/忘隙）、界甄姬（倾国/洛神）。
- 用例：`tests/p84.rs` 24 条；`cargo test -p sanguosha` **475** 全绿。
- 已知边界（ADR-0024 §3 追加）：清俭「任意张」简化为 1 张且每回合只询问一次；界突袭 X ≤ 2；界裸衣亮牌后的其余牌进弃牌堆；界遗计按逐张分配；忘隙仅在 1 点伤害事件触发；界洛神 `luoshen_free` 按「获得张数」直接加手牌上限。

**P4.4.2 批次 5（界标·群 4 将，已完成 2026-10-30，界标 25 将收尾）**：
- 机制：除疠（`Frame::ChuLi`，势力去重多选 + 弃牌/黑桃摸牌）、利驭（`Frame::LiYu`：拿牌 → 装备则目标指定决斗；`pending_liyu` 栈空后结算）、义从（双向距离修正）、趫猛（弃牌/坐骑获得）、界离间可弃装备且无无懈、界闭月空手摸 2。
- 技能：界华佗（急救/除疠）、界吕布（无双/利驭）、界公孙瓒（义从/趫猛）、界貂蝉（离间/闭月）。
- 用例：`tests/p85.rs` 12 条；`cargo test -p sanguosha` **487** 全绿；`tests/p67.rs` 界优先去重断言更新为「标准 25 全替换 + 李典/公孙瓒额外 → 27 位」。
- 已知边界（ADR-0024 §3 追加）：除疠弃牌由引擎自动选牌；利驭「由目标指定」由界面驱动（actor 为吕布）；趫猛自动发动（不询问）。

**P4.4.2 批次 6（一将成名批次 2：8 将，已完成 2026-10-30）**：
- 机制：绝情（伤害转失去体力）、伤逝（失去手牌后补至已损失体力）、将驰（`Frame::JiangChi` 二选一 + 出杀封锁/无距离/次数）、奇策（`Frame::Qice` 全手牌当锦囊）、智愚（受伤摸 1 + 同色来源弃牌）、当先（`dangxian_extra` 额外出牌阶段）、伏枥（濒死回复至势力数并翻面）、潜袭（`Frame::QianXi` 颜色封锁）、安恤（`Frame::AnXu`）、追忆（`Frame::ZhuiYi`）、疠火（`Frame::LiHuo` 火杀判定 + 失去体力）、醇醪（`Frame::ChunLao` + 濒死自动【酒】）、弓骑（`Frame::GongQi`）、解烦（`Frame::JieFan`）。
- 技能：张春华（绝情/伤逝）、曹彰（将驰）、荀攸（奇策/智愚）、廖化（当先/伏枥）、马岱（马术/潜袭）、步练师（安恤/追忆）、程普（疠火/醇醪）、韩当（弓骑/解烦）。
- 用例：`tests/p86.rs` 24 条；`cargo test -p sanguosha` **511** 全绿；`tests/p67.rs` 一将成名已实现数更新为 16。
- 已知边界：智愚/伤逝/醇醪/解烦/趫猛类「可选」按自动口径结算；奇策仅首牌为实体（其余进弃牌堆）；当先额外出牌阶段结束时跳过弃牌阶段；潜袭只封锁「颜色」手牌（技能转化仍受同色判断约束）。

**P4.4.2 批次 Y7（一将成名 388–395：8 将，已完成 2026-10-31，一将成名收尾）**：
- 机制：忠鉴（展示对照）/才识（摸牌阶段二选一）、福绵（准备阶段数值递增二选一）/怠宴（红桃基本牌 + 连续目标失体力）、问卦（交牌置顶/底 + 双方摸另一端）/伏诛（牌堆【杀】连击 + 洗牌）、守玺（声明牌名二选一）/惠民（按缺口摸牌分配）、清弦（受伤后来源二选一）/绝响（死亡授予清弦 + 梅花保护）、谏征（取消/转移【杀】目标）/专对（拼点封锁响应）/天辩（牌堆顶拼点 + 红桃=K）、复难（响应牌交换 + 禁牌）/诫训（方片数摸牌 + 递增弃牌）、辟撰（「书」标记 + 手牌上限）/通博（「书」替换 + 四花色分配）。
- 技能：辛宪英（忠鉴/才识）、吴苋（福绵/怠宴）、徐氏（问卦/伏诛）、曹节（守玺/惠民）、嵇康（清弦/绝响）、秦宓（谏征/专对/天辩）、薛综（复难/诫训）、蔡邕（辟撰/通博）。
- 用例：`tests/p95.rs` 24 条；`cargo test -p sanguosha` **685** 全绿；`tests/p67.rs` 一将成名已实现数更新为 **72（全部）**。
- 已知边界（rules-rulings.md §29）：伏诛不开放响应；福绵锦囊多目标未实现；清弦「回复体力后」半部分未实现；绝响以「清弦」代替四个变体技能；诫训对复难的修改未实现；通博每次仅交换 1 张。

**P4.4.2 批次 Y6（一将成名 380–387：8 将，已完成 2026-10-31）**：
- 机制：矫诏（展示牌转化 + 最近角色声明）/殚心（摸牌或修改矫诏）、督粮（获牌 + 二选一）/腹鳞（获得牌不计上限）、匡弼（武将牌寄存 + 下回合回收）、止戈（令范围含己者出杀或交装备）/宗祚（开局势力数上限、势力全灭减上限）、极奢（摸牌换手牌上限、结束阶段横置）/链祸（火焰传导起点 +1）、瑰藻（异花色弃牌回血/摸牌）、讥谀（弃牌封锁花色，黑桃失体力+翻面）、寝情（范围含主公者弃牌摸牌，手牌差额外摸牌）、贿生（展示牌换伤害防止/弃牌）、滔乱（任意基本牌/锦囊 + 每名限一次 + 事后二选一）。
- 技能：郭皇后（矫诏/殚心）、李严（督粮/腹鳞）、孙登（匡弼）、刘虞（止戈/宗祚）、岑昏（极奢/链祸）、孙资刘放（瑰藻/讥谀）、黄皓（寝情/贿生）、张让（滔乱）。
- 用例：`tests/p94.rs` 24 条；`cargo test -p sanguosha` **661** 全绿；`tests/p67.rs` 一将成名已实现数更新为 64。
- 已知边界（rules-rulings.md §28）：矫诏「闪」仅响应未实现；腹鳞仅统计督粮获得；宗祚死亡减上限用「势力最后一人」判定；滔乱锦囊选项为 6 种普通锦囊。

**P4.4.2 批次 Y5（一将成名 335–342：8 将，已完成 2026-10-31）**：
- 机制：战绝（全手牌当【决斗】+ 受伤双方摸牌，摸满 2 张失效）/勤王（主公技弃牌发动激将，响应者摸 1）、樵拾（手牌相等各摸 1）/燕语（重铸【杀】，≥2 张令男性摸 2）、怃戎（同时展示手牌分支）/矢志（体力 1 时【闪】当【杀】）、宴诛（夺装备或弃牌 + 修改兴学）/兴学（按体力次数摸牌置顶）/诏缚（主公限定：吴势力攻击范围覆盖）、安国（全场最少三项 + 补做）、邀名（伤害后弃/摸，每回合一次）、怀异（弃一色按张数获牌，≥2 失体力）、急攻（摸 2 + 手牌上限=伤害值）/饰非（响应窗口视为出【闪】）。
- 技能：刘谌（战绝/勤王）、夏侯氏（樵拾/燕语）、张嶷（怃戎/矢志）、孙休（宴诛/兴学/诏缚）、朱治（安国）、全琮（邀名）、公孙渊（怀异）、郭图逢纪（急攻/饰非）。
- 用例：`tests/p93.rs` 24 条；`cargo test -p sanguosha` **637** 全绿；`tests/p67.rs` 一将成名已实现数更新为 56。
- 已知边界（rules-rulings.md §27）：战绝按本回合【决斗】伤害近似识别（无「仅本技能」标记）；兴学/安国选牌按随机/自动口径；诏缚只影响攻击范围（不改变距离数值）。

**P4.4.2 批次 Y4（一将成名 327–334：8 将，已完成 2026-10-31）**：
- 机制：谮毁（`Frame::ZanHui` 代用/额外目标）、骄矜（伤害 -1）、奋励（按条件跳阶段）+ 平寇（按跳过数伤害）、慎行/秉壹（类型展示摸牌）、渐营（花色点数连锁）+ 矢北（首次受伤回复、再受伤失体力）、窃听/献州（限定）、恢拓（判定回复/摸牌）/明鉴（全手牌 + 下回合加成）/兴衰（主公限定濒死救援）、千驹（损失体力距离 -1）/倾袭（伤害前二选一）、活墨（黑非基本→基本）/佐定（黑桃目标摸牌）。
- 技能：孙鲁班（谮毁/骄矜）、朱桓（奋励/平寇）、顾雍（慎行/秉壹）、沮授（渐营/矢北）、蔡夫人（窃听/献州）、曹叡（恢拓/明鉴/兴衰）、曹休（千驹/倾袭）、钟繇（活墨/佐定）。
- 用例：`tests/p92.rs` 24 条；`cargo test -p sanguosha` **613** 全绿；`tests/p67.rs` 一将成名已实现数更新为 48。
- 已知边界（rules-rulings.md §26）：谮毁仅【杀】；活墨不含【闪】；佐定仅【杀】目标窗口；兴衰自动结算不询问；秉壹/献州目标自动选取。

**P4.4.2 批次 Y3（一将成名 8 将：187/188/321–326，已完成 2026-10-31）**：
- 机制：贞烈（`Frame::ZhenLie` 取消【杀】/锦囊目标 + 弃使用者牌）、秘计（结束阶段摸已损失体力并可交牌）、父魂（两张手牌当【杀】+ 伤害后暂授武圣/咆哮）、司敌（同色非基本牌封锁颜色 + 阶段末虚拟【杀】）、慎断（黑基本牌弃置→兵粮寸断）、勇略（判定阶段弃判定牌虚拟【杀】，未伤害摸 1）、品第（类型位图 + 每角色一次 + X 递增 + 自横置）、法恩（翻面/横置摸 1）、奔袭（每牌距离 -1 + 单目标【杀】四选二强化）、忠勇（交出【杀】/【闪】，红牌可代出【杀】）、强识（展示牌类型 + 同类摸 1）、献图（摸 2 交 2 + 阶段末失体力）。
- 技能：王异（贞烈/秘计）、关兴张苞（父魂）、曹真（司敌）、韩浩史涣（慎断/勇略）、陈群（品第/法恩）、吴懿（奔袭）、周仓（忠勇）、张松（强识/献图）。
- 用例：`tests/p91.rs` 24 条；`cargo test -p sanguosha` **589** 全绿；`tests/p67.rs` 一将成名已实现数更新为 40。
- 已知边界（rules-rulings.md §25）：贞烈仅覆盖普通锦囊；父魂虚拟杀不触发按牌名判定的加成；奔袭暂只强化【杀】（锦囊部分未实现）；强识只统计 `play:` 主动使用；忠勇取最后一个目标的响应牌。

**P4.4.2 批次 Y2（一将成名 8 将：179–186，已完成 2026-10-31）**：
- 机制：陷嗣（`Frame::XianSi`「逆」标记 + 其他角色移两「逆」虚拟【杀】）、夺刀（弃牌夺武器）、暗箭（攻击范围反向判定加伤）、纵玄（弃置牌置顶，同次合并询问）、直言（摸 1 亮牌，装备则使用并回复）、胆守（`Frame::DanShou` X=发动次数 1/2/3/≥4 分支）、惴恐（回合开始拼点 + `deny:*` 封锁 / `zhuikong:*` 距离覆盖）、求援（交【闪】或成为额外目标）、灭计（黑锦囊置顶 + 目标弃牌分支）、绝策（无手牌角色 1 伤）、焚城（限定，依次弃更多牌或 2 火伤）、恩怨（受伤来源二选一 + 眩惑获牌摸 1）、眩惑（摸牌阶段替代 + 虚拟杀/获得两张）、无言（锦囊伤害防止）、举荐（弃非基本牌三选一）。
- 技能：刘封（陷嗣）、潘璋马忠（夺刀/暗箭）、虞翻（纵玄/直言）、朱然（胆守）、伏皇后（惴恐/求援）、李儒（灭计/绝策/焚城）、法正（恩怨/眩惑）、徐庶（无言/举荐）。
- 用例：`tests/p90.rs` 24 条；`cargo test -p sanguosha` **565** 全绿；`tests/p67.rs` 一将成名已实现数更新为 32。
- 已知边界（rules-rulings.md §24）：恩怨获牌半只覆盖眩惑；纵玄/直言/灭计/焚城的选牌按自动口径；求援额外目标加入同一【杀】的后续目标队列。

**P4.4.2 批次 Y1（一将成名 8 将：171–178，已完成 2026-10-31）**：
- 机制：恃勇（红杀/酒杀 → 减体力上限，`jiu_boost:<seat>`）、自守（摸牌阶段多摸势力数 + 本回合防止自己造成的伤害）、宗室（手牌上限 + 势力数）、权计（每点伤害摸 1 置「权」，手牌上限 +权数）、自立（准备阶段「权」≥3 觉醒：回复 1 或摸 2，减上限 + 获得排异）、排异（移「权」令一名角色摸 2，手牌多于你则 1 伤）、称象（亮 4 取点数和 ≤13）、仁心（体力 1 的其他角色受伤时弃装备翻面防止）、精策（花色位图手牌上限 + 牌类型位图结束摸牌）、峻刑（弃任意张 → 目标弃异类型牌或翻面摸等量）、御策（展示手牌 → 来源弃异类型牌或回复 1）、龙吟（弃一牌使【杀】不计次数，红杀摸 1）、巧说（拼点赢 → 下一张【杀】多目标；没赢 → 禁锦囊）、纵适（拼点牌回收）。
- 技能：将华雄（恃勇）、刘表（自守/宗室）、钟会（权计/自立 + 授予排异）、曹冲（称象/仁心）、郭淮（精策）、满宠（峻刑/御策）、关平（龙吟）、简雍（巧说/纵适）。
- 用例：`tests/p89.rs` 24 条；`cargo test -p sanguosha` **541** 全绿；`tests/p67.rs` 一将成名已实现数更新为 24。
- 已知边界（rules-rulings.md §23）：权计「置一张手牌」自动随机；巧说赢只实现「多指定一个目标」（不含少指定），且只作用于下一张【杀】；精策花色/类型按 `play:` 使用记录（技能转化牌不计）；称象亮出的牌暂存处理区（帧内 `check_conservation` 仍守恒）。

**P4.4.2 一将成名批次表（Y1–Y7 全部完成）**：

- **状态**：一将成名 **72/72 全部实现**（Y1–Y7 完成，2026-10-31）；下一步转为界限突破其余 60 位 → P4.4.3 验收。
- **数据/包开关**：`data.rs` 已全量生成 72 位、`GENERAL_PACKS` 已含 `yijiang`，无需再跑生成脚本；未实现的武将 `general_ready=false` 不进池。
- **每批流程**：① `skills.rs` 注册（code/name/kind/implemented）→ ② 实现技能帧/钩子（`resolve_yijiangN.rs` 按批次续号）→ ③ 每将 ≥3 条用例（`tests/p95.rs` 起按批次续号，复用 `tests/support.rs` 助手）→ ④ `skills.md` 新增 `§2.7.x` 表 + `rules-rulings.md` 新增一节 → ⑤ `p67` 的「一将成名已实现数」断言 +8（`npm run sgs:batch-status` 校验）→ ⑥ `npm run sgs:check` 全套门禁 → ⑦ 提交。**模板/坑位/DoD 见 `docs/games/sanguosha/batch-playbook.md`**。
- **批前必读**：`skills.md` §2.7/§2.7.8、`rules-rulings.md` §27/§28/§29、`docs/conventions/coding-standards.md`、`docs/conventions/message-protocol.md`（若涉及交互原语）。
- **门禁**：`npm run sgs:check`（= `cargo test -p sanguosha` + skill-cases/skill-docs/engine-data/batch-status/pack-pool/view-contract/interaction + `test:web`/`test:sgs-web`）；`npm run sgs:check:fast` 快速自检；`npm run sgs:check:full` 追加 MCP 冒烟 `verify:sanguosha`。

| 批次 | 武将（gid 名称） |
|---|---|
| Y1 ✅ | 171 将华雄、172 刘表、173 钟会、174 曹冲、175 郭淮、176 满宠、177 关平、178 简雍（已完成 2026-10-31） |
| Y2 ✅ | 179 刘封、180 潘璋马忠、181 虞翻、182 朱然、183 伏皇后、184 李儒、185 法正、186 徐庶（已完成 2026-10-31） |
| Y3 ✅ | 187 王异、188 关兴张苞、321 曹真、322 韩浩史涣、323 陈群、324 吴懿、325 周仓、326 张松（已完成 2026-10-31） |
| Y4 ✅ | 327 孙鲁班、328 朱桓、329 顾雍、330 沮授、331 蔡夫人、332 曹叡、333 曹休、334 钟繇（已完成 2026-10-31） |
| Y5 ✅ | 335 刘谌、336 夏侯氏、337 张嶷、338 孙休、339 朱治、340 全琮、341 公孙渊、342 郭图逢纪（已完成 2026-10-31） |
| Y6 ✅ | 380 郭皇后、381 李严、382 孙登、383 刘虞、384 岑昏、385 孙资刘放、386 黄皓、387 张让（已完成 2026-10-31） |
| Y7 ✅ | 388 辛宪英、389 吴苋、390 徐氏、391 曹节、392 嵇康、393 秦宓、394 薛综、395 蔡邕（已完成 2026-10-31） |

> 顺序口径：按 gid 升序每批 8 将；每批独立提交，便于中断后续接。

**P4.4.2 批次 J1（界限突破 8 将：26/304/446–451，已完成 2026-10-31）**：
- 机制：界烈弓（点数距离 + 双效果）、`dealt:<seat>` 回合内伤害记录（诛害）、觉醒授予（潜心→荐言）、耀武（伤害牌颜色分派摸牌）、势斩（`RespondFrame(Duel)`，华雄先出【杀】）、界化身（3 张 + 换牌二选一 + 势力/性别覆盖）、界雷击（使用【闪电】也触发；判定后选目标，`Frame::LeijiJie`）、界鬼道（♠2~9 摸 1）、界黄天（黑桃手牌）、界蛊惑（假→质疑者摸 1；真→弃 1 或失 1）、界乱击（少选一个目标，`Frame::LuanJiPick`）、界血裔（「裔」标记，`mark_counts["yi"]`）、界放权（弃牌阶段开始时）、界若愚（回复至 3 + 授予思蜀）、思蜀（`sishu_reverse` 判定反转）。
- 技能：界黄忠（烈弓）、界徐庶（诛害/潜心 + 荐言）、界华雄（耀武/势斩）、界左慈（化身/新生）、界张角（雷击/鬼道/黄天）、界于吉（蛊惑 + 界缠怨）、界袁绍（乱击/血裔）、界刘禅（享乐/放权/若愚 + 思蜀）。
- 用例：`tests/p96.rs` 26 条；`cargo test -p sanguosha` **711** 全绿；`tests/p67.rs`「standard+jiexian 池」断言 27 → 35。
- 已知边界（rules-rulings.md §30）：诛害按项目数据「视为使用【杀】」（不采用「将手牌当过河拆桥」版本）；荐言按「出牌阶段限一次」且只给男性角色；界蛊惑「弃一张牌或失去 1 点体力」为自动口径（有手牌弃随机一张）；界化身换牌固定移去非展示的至多 2 张；界雷击可选自己为目标；界乱击最多少选 1 个目标。

**P4.4.2 批次 J2（界限突破 8 将：452/453/454/455/456/457/458/462，已完成 2026-10-31）**：
- 机制：界激昂（弃牌回收、`Frame::JiangJie`）、界魂姿（觉醒当回合结束阶段摸 2 或回复 1，`Frame::HunziEnd`）、界制霸（主公主动拼点，`Frame::ZhiBaJie`）、设变（翻面移动装备，`Frame::SheBian`）、藏拙（`trick_used` 计数 + 锦囊不计手牌上限）、界火计（装备区红牌 + 同颜色弃牌）、界看破（装备区黑牌 + 无懈不可响应）、界连环（装备区梅花 + 多指定一个目标，`Frame::LianHuanPick`）、界涅槃（授予八阵/火计/看破，`Frame::NiePanSkill`）、界奇谋（摸 X 张）、界红颜（装备区红桃 → 手牌上限=体力上限）、界天香（装备区红桃）、飘零（`Frame::PiaoLingPlace`）、武烈（「烈」标记，`Frame::WuLie` + 伤害防止）、界鞬出（基本/非基本分支 + `jiandu_sha_bonus`）。
- 技能：界孙策（激昂/魂姿/制霸）、界夏侯渊（神速/设变）、界卧龙诸葛亮（八阵/藏拙/火计/看破）、界庞统（连环/涅槃）、界魏延（狂骨/奇谋）、界小乔（红颜/天香/飘零）、界孙坚（英魂/武烈）、界庞德（马术/鞬出）。
- 用例：`tests/p97.rs` 25 条；`cargo test -p sanguosha` **736** 全绿；`tests/p67.rs`「standard+jiexian 池」断言 35 → 43。
- 已知边界（rules-rulings.md §31）：界连环的装备区梅花牌「重铸」未实现；设变移动目标自动选择第一个同槽位为空者；武烈按座位序取目标且不询问；界涅槃三选一为必选；界制霸主动拼点要求双方均有手牌。

**P4.4.2 批次 J3（界限突破 8 将：463/470/471/479/480/484/485/486，已完成 2026-10-31）**：
- 机制：酣战（拼点随机 + 获得最大点数【杀】）、界酒池（【酒】无限 + 酒【杀】后崩坏失效）、界暴虐（+获得判定牌）、界屯田（回合内弃【杀】判定）、界凿险（额外回合）、长标（任意张手牌当无距离【杀】+阶段末摸牌）、界挑衅（伤害条件 + 限两次，`Frame::TiaoxinWait`）、界帷幕（回合内防止伤害并摸 2 倍）、界乱武（结算后视为使用【杀】）、界好施（回礼者自动交牌）、界巧变（「变」标记 / 代替弃牌 / 结束阶段增长）。
- 技能：界太史慈（天义/酣战）、界董卓（肉林/崩坏/酒池/暴虐）、界邓艾（屯田/凿险）、界祝融（巨象/烈刃/长标）、界姜维（挑衅/志继）、界贾诩（帷幕/乱武/完杀）、界鲁肃（好施/缔盟）、界张郃（巧变）。
- 用例：`tests/p98.rs` 24 条；`cargo test -p sanguosha` **760** 全绿；`tests/p67.rs`「standard+jiexian 池」断言 43 → 51。
- 已知边界（rules-rulings.md §32）：酣战/帷幕/好施/乱武均为自动口径；屯田仅覆盖经 `discard_by` 的弃置；长标取所选第一张为实体、其余置入弃牌堆；挑衅「未造成伤害」分支自动弃牌。

**P4.4.2 界限突破剩余 60 位（新会话开工口径，2026-10-31 确认）**：

- **状态**：界限突破 87 位中已实现 **51 位**（标准 25 将界版 + 界李典/界公孙瓒 + J1/J2/J3 各 8 将）；剩余 36 位按 J4–J8 分批（J8 为 4 将）。
- **数据/包开关**：`data.rs` 已全量生成 87 位、`GENERAL_PACKS` 已含 `jiexian`；界面同名去重由 `prefer_jiexian` 处理（ADR-0024 口径 A：界版优先）。
- **每批流程**：与一将成名一致（`docs/games/sanguosha/batch-playbook.md`）：`npm run sgs:batch-scaffold -- --pack jiexian` 取下一批 → skills.rs 注册（界版同名技能**复用 code 并按 `is_jiexian` 分支**）→ 帧实现（新文件 `resolve_jiexian2.rs` 起按批次命名）→ 每将 ≥3 用例（`tests/p96.rs` 起）→ `p67` 计数 +8 → 文档（`skills.md` §2.8.x + `rules-rulings.md` 新节）→ `npm run sgs:check`。
- **注意**：界版与标准同名技能共用 code，公共逻辑须在 `skills::is_jiexian(st, seat)` 分支中区分；不要改标准版行为（回归 `tests/p46`–`p55` 与 `check:sgs-skill-cases`）。

| 批次 | 武将（gid 名称） |
|---|---|
| J1 ✅ | 26 界黄忠、304 界徐庶、446 界华雄、447 界左慈、448 界张角、449 界于吉、450 界袁绍、451 界刘禅（已完成 2026-10-31） |
| J2 ✅ | 452 界孙策、453 界夏侯渊、454 界卧龙诸葛亮、455 界庞统、456 界魏延、457 界小乔、458 界孙坚、462 界庞德（已完成 2026-10-31） |
| J3 ✅ | 463 界太史慈、470 界董卓、471 界邓艾、479 界祝融、480 界姜维、484 界贾诩、485 界鲁肃、486 界张郃（已完成 2026-10-31） |
| J4 | 492 界孟获、496 界徐晃、505 界蔡文姬、515 界荀彧、526 界典韦、550 界颜良文丑、563 界张昭张纮、587 界徐盛 |
| J5 | 588 界凌统、597 界陈宫、599 界曹彰、600 界简雍、603 界虞翻、604 界高顺、610 界法正、620 界程普 |
| J6 | 621 界王异、625 界张春华、626 界刘表、627 界蔡夫人、628 界曹冲、629 界曹植、640 界吴国太、641 界廖化 |
| J7 | 642 界李儒、658 界郭淮、671 界伏皇后、676 界韩当、690 界关兴张苞、696 界马岱、726 界郭皇后、749 界沮授 |
| J8 | 752 界夏侯氏、753 界张松、756 界于禁、775 界步练师 |

> 顺序口径：按 gid 升序每批 8 将（J8 为 4 将）；每批独立提交。

**P4.4.2 下一步**：界限突破其余 36 位（J4–J8，按 gid 分批）→ P4.4.3 包开关/全套门禁验收。

**授予技能补齐（已完成 2026-10-30）**：SP-2 觉醒授予 **清侧**（毌丘俭·鸿举）、**怀柔**（陆抗·破势）已取得官方文本并实现：
- 清侧：出牌阶段移去一张「荣」，弃置其他角色区域里一张装备牌或延时锦囊牌（不能弃置自己的牌）。
- 怀柔：出牌阶段可重铸一张装备牌（弃置并摸一张），不限次数。
- 用例：`tests/p87.rs` 4 条（清侧装备/延时锦囊/不能弃自己 + 怀柔重铸）；`cargo test -p sanguosha` **515** 全绿。

### P5 武将流程设计工作台 + 预览（L）

**目标**：用有向图设计武将回合内/外流程，并在 Web 预览页跑通一轮 5 人局。

交付物：

- n8n 复用评估（**撤退点**：若自定义节点无法注入/逐步仿真，则改用 Vue Flow 自研，见 ADR-0018）。
- 工作台页面（合并进 `props-admin` 或独立 `apps/sgs-workbench`）：节点=阶段/技能触发/条件/结算/选择；边=时机流。
- 只读依赖：引擎「合法动作 + 规则校验 + 武将/技能元数据」API（不可在中台改规则）。
- 预览：加载 `game_state_templates` → 逐步执行 → 显示公共区/个人区/特殊区 → 保存/复制模板。
- 模板管理页（中台）：局面与玩家资源模板 CRUD。

验收：

- 至少 1 个武将（建议多技能武将）用工作台完成设计并生成可执行实现（人工落到代码 + 用例）。
- 预览页跑通 1 轮完整 5 人局（覆盖回合内 + 回合外技能），复用 P3 的视图组件。
- 模板可保存/复制/回放。

依赖：P1（元数据）、P2（引擎/合法动作）、**P3（Web 预览基座）**。

### P6 AI 决策 + 录制回放（M）

**目标**：机器人能打完整局；对局可导出训练数据。

交付物：

- 引擎候选剪枝（完整合法集 → ≤26 候选）+ SystemOne 选择；超限兜底。
- `game_decision_log` 落库（可采样/关闭）；导出 JSONL（openid 哈希）。
- 回放工具：`game_state` 快照 + `game_events` 重放 + decision log 关联；断点续放。
- 自对弈脚本（`RandomPolicy` vs `HeuristicPolicy`）与基础平衡指标。

验收：

- 机器人 5 人局 100 局无非法动作、无卡死；回放与原始对局逐步一致。
- 导出数据字段完整（状态摘要/合法集/候选/选择/置信度）。

依赖：P2、P3。

### P7 Q 群接入（M，后置）

**目标**：Q 群只承担「开局设定 + 轮次/状态 + 牌局链接」，不做局面播报与逐动作交互。

交付物：

- `apps/agent-gateway/src/games.ts` 增加 `sanguosha`（lobby 模式）。
- 命令：`#加入` / `#退出` / `#开始` / `#中断` / `#局面`（只回轮次/阶段/链接，不叙述公共局面）。
- 牌局链接：签发在 Web 个人资源区操作的短 token；`#恢复` 重签。
- 关闭三国杀的 LLM 局面叙述（与拉斯维加斯不同）。
- 交互原语已在 P2.4 实现，本阶段只做网关命令映射。

验收：

- 群内完成开局 → 玩家点链接进入 Web 个人资源区 → 动作在 Web 完成；`#局面` 只回轮次/状态/链接。
- 命令解析单测；无自由文本意图猜测；群消息不泄露隐藏信息。

依赖：P2、P3（Web 交互）、P4（可玩武将）。

### P8 上线验证 + 权鉴（M）

**目标**：灰度上线，补齐权鉴、可观测、可回滚。

交付物：

- **游戏权鉴**：梳理个人资源区/特殊区的数据范围，评估短 token scope 是否足够；补动作级校验（服务端 + 引擎双重）。
- 灰度开关：`games.ts` 启用 + 环境变量；失败回退到「仅拉斯维加斯」。
- 观测：对局数、平均时长、动作失败率、决策延迟、MCP 错误日志。
- 运维手册：中断/恢复、卡局排查、事件重放校验。
- 验收脚本：`apps/verify-sanguosha.mjs`（5 人/8 人局全流程冒烟 + 视图越权用例）。

验收：

- 真实群 ≥10 局 5 人局 + 1 局 8 人局；无严重规则错误；回放可复现。
- 权鉴用例通过（个人资源区越权 403；公共区只读）。
- 用户人工验收通过，里程碑记入 `memory_palace/milestones.md`。

依赖：P2–P7。

## 5. 依赖与里程碑

```
P0 ──> P1 ──> P2.1 ──> P2.2 ──> P2.3 ──> P2.4 ──> P2.5 ──> P3 ──> P5 ──> P7 ──> P8
                              └──> P4（可与 P3/P5 并行）──┘
P2 + P3 ──> P6（可与 P5 并行）
```

> 关键顺序（用户 2026-10-25 确认）：**Web（P3）先于工作台（P5）**；**Q 侧（P7）后置**；原语在 P2.4 先行添加。

建议里程碑（写入 `milestones.md` 的时机）：

| 里程碑 | 内容 | 判定 |
|---|---|---|
| M17 | 道具中台 MVP（含生命周期与关联页） | P1 验收 |
| M18 | 无技能 5 人局引擎 + 视图三分区 + 特殊区骨架 | P2.1–P2.5 验收 |
| M19 | 技能内核 + 首批武将 + 原语 | P2.4 验收 |
| M20 | Web 公共区 + 个人资源区交互 | P3 验收 |
| M21 | 标准 25 将 + 军争牌 | P4 部分验收 |
| M22 | 工作台 + 预览 | P5 验收 |
| M23 | AI 完整局 + 数据导出 | P6 验收 |
| M24 | Q 群开局/状态/链接 | P7 验收 |
| M25 | 灰度上线 + 权鉴 | P8 验收 |

## 6. 技能与回合内核设计要点（摘要）

> 完整决策见 ADR-0016；视图分区见 ADR-0020；此处只列实现约束。

1. **回合 = 可组合阶段序列**：`prepare → judge → draw → play → discard → end`；武将技能通过钩子**插入/跳过/重复**阶段，核心流程不写武将分支。
2. **武将上下文**：`TurnContext { seat, general, skills, phase, once_flags }`；所有技能判定都能拿到「谁的回合/什么阶段/是否自己回合」。
3. **技能事件注册**：`SkillRegistry` 按事件订阅；事件例：`turn_start`、`phase_start/end`、`card_used`、`card_targeted`、`damage_before/after`、`dying`、`death`、`judge_before/after`、`draw_count`、`hand_limit`（弃牌上限）、`hand_cap`（持有上限）。
4. **确定性排序**：时机优先级（技能声明）→ 距离当前回合玩家的座位顺序 → 注册顺序；禁止依赖 HashMap 迭代顺序。
5. **嵌套触发**：统一结算栈；触发中产生的新询问进 `pending_queue`，逐个解决，不递归改状态。
6. **规则校验**：`execute_action` 无条件重校验（合法动作 + 权限 + 资源归属）；中台/工作台数据不参与校验。
7. **动作 id 规范**：`lobby:*`、`play:<card_instance>`、`skill:<skill_code>[:arg]`、`respond:<card_instance>`、`discard:<csv>`、`choose:<n|codes>`、`pass`；具体命名 P0 定稿。
8. **视图三分区**：`public_board`（只读）/ `player_area`（交互）/ `special_zones`（可见性由技能声明）；引擎内部状态完整，对外投影脱敏。
9. **响应顺序**：所有响应（闪/无懈/桃/技能）从**当前玩家左手侧**开始，按座位顺序循环；死亡跳过、无超时。
10. **主公体力**：`max_hp + 1`、`initial_hp + 1`（武将初始体力按 `initial_hp = max_hp` 处理）。
11. **判定区顺序**：延时锦囊按**后进先出（LIFO）**判定；`judge_zone` 末尾为栈顶，从末尾向前遍历（例：先乐不思蜀、后兵粮寸断 → 先判兵粮寸断）。
12. **选将**：主公候选=全部主公将 + N 张非主公（默认 5）；其他玩家同时获得 M 张候选（默认 5）；N/M 配置化。
13. **手牌**：持有上限默认 20（`config.max_hand_cards`，超出立即弃至上限）；弃牌阶段上限 = 当前体力值（技能可改）。
14. **装备区**：武器/护具/进攻马/防御马/特殊（木牛流马预留）各 1，后装备顶掉旧装备。
15. **距离**：无上限；攻击距离=1+武器+进攻马+技能；防御距离=1+防御马(+1)+技能；顺手/兵粮使用不含武器范围的距离（陆逊等例外）。
16. **无懈链**：从原锦囊使用者左手侧开始，每次有人打出无懈重新从该使用者左手侧开始，允许同一玩家多次响应。

## 7. 交互原语与端侧分工

> 用户 2026-10-25 裁定：**原语先行添加**（P2.4），Q 群侧交互后置；Web 是交互基座；多端走 WebView 复用。

| 需求 | 现有原语 | 缺口 | 提案 |
|---|---|---|---|
| 出牌（牌 → 目标 → 额外选择） | 单 `execute_action` / 多轮 `#选 N` | 无「同一主体连续多步」的批量提交 | 引擎 pending 链；Web 逐帧交互，后续 `execute_actions` 批量 |
| 弃多张牌（手牌上限/技能） | 仅 G1 加密代码多选 | 明文多选 + min/max | `many` 补 `min/max`；Web 复选列表 |
| 选择多个目标（AOE/方天画戟等） | 仅单目标 | 目标多选与去重 | 引擎 `many` + 目标列表；Web 多选，Q 后置 |
| 跨玩家响应（闪/无懈可击/桃） | 语义上的「异步等待」 | 响应队列与 actor 展示 | 引擎 pending + 响应栈；Web 等待态；Q 后置 |
| 同时多玩家选择（五谷丰登） | 接力/异步 | 固定顺序与队列 | 按座位顺序逐个询问，特殊区展示 |
| 排序（观星等） | `partition` 无顺序语义 | 需要「有序多选」 | 新增 `ordered` 变体；Web 拖拽排序；Q 后置 |
| 动作数 >26 | `GAME_ACTION_CHOICE_LIMIT=26` + 清单兜底 | 出牌空间远大于 26 | 引擎候选剪枝 + 粗动作两级；完整合法集只进日志 |
| 隐藏信息 | `masked` + 短 token 个人视图 | 手牌量大、刷新频繁 | 个人资源区 + 5 分钟 token + `#恢复` 重签 |
| 消息频控 | 5 条/消息、5 分钟被动窗口 | Q 侧不做逐动作，本项风险降低 | Q 只回状态/链接；Web 不占用群消息 |

**端侧分工**：

| 端 | 职责 |
|---|---|
| Web | 公共局面区展示 + 个人资源区交互 + 特殊区交互（**主交互面**） |
| WebView（Tauri/原生壳） | 加载同一 Web 页，不启用平台专有 API；后续按需桥接 |
| Q 群 | 开局设定、加入/开始/中断、轮次/状态查询、牌局链接；**不播报局面、不做逐动作** |

## 8. 测试策略

| 层 | 内容 |
|---|---|
| 规则单测 | 每张牌、每个技能、每个时机；边界（无牌可弃、装备替换、距离 1 与 +1 马） |
| 不变量 | 牌数守恒、体力上下限、手牌上限、死亡后无动作、身份唯一 |
| 黄金局面 | 固定 seed + 状态 JSON fixture → 动作 → 期望事件序列（防回归） |
| 回放一致性 | 快照重放 = 原对局；事件 payload 含全部随机 |
| 交互链 | pending 队列、多步选择、取消/无合法动作 |
| 视图契约 | 三分区字段快照；公共区无隐藏信息；个人区越权 403；特殊区可见性 |
| Web / WebView | 渲染快照、轮询刷新、无 JS 降级、WebView 冒烟 |
| 端到端 | `verify-sanguosha.mjs`：5 人/8 人全流程、中断/恢复、链接进入 |
| AI | 100 局无非法动作；决策日志完整性 |

## 9. 风险与阻碍（提前确认）

| # | 风险/待确认 | 影响 | 处置 |
|---|---|---|---|
| 1 | 规则歧义（持有上限超出弃置、顺手距离例外等） | 返工 | 已裁定部分落 `rules-rulings.md` / `rules.md`；剩余 P2/P4 细化 |
| 2 | **官网数据质量**（696 位中 1 条空技能、gender 全缺失、54 位国战半体力） | P1 导入返工 | 导入脚本按 `data/README.md` 处理；人工复核 `type_guess` 与 gender |
| 2b | **牌表无公开结构化接口** | P1 初始化工作量 | 官网 sitemap 仅 `/hero/*` 等页面；默认人工/社区表，后续再找官方数据 |
| 3 | 牌池范围：军争内容多 | 复杂度 | 首版标准包跑通；军争定义 P1 录入、玩法 P4 启用 |
| 4 | 武将包范围（标准 25 → 100+ 技能） | 排期失控 | 按包交付、每包独立验收；技能走代码 + 工作台设计 |
| 5 | 动作空间 >26 / 多步交互 | 体验 | 两级动作 + 候选剪枝 + pending 链；Web 逐帧交互 |
| 6 | **多端 WebView 兼容** | 返工 | 视图三分区契约 + 响应式 + 无平台 API；P3 做 WebView 冒烟 |
| 7 | **视图分区信息泄露** | 严重 | 服务端投影 + 字段级测试 + 越权用例；权鉴 P8 补齐 |
| 8 | **权鉴后置**（短期 token，拉斯维加斯信息公开） | 上线风险 | P8 专项评估 token scope / 动作级校验；未通过不上线 |
| 9 | **Q 侧后置**导致群体验弱化 | 产品预期 | 明确 Q 只做开局/状态/链接；主交互在 Web（用户已确认） |
| 10 | 长对局与离线 | 卡局 | 沿用「无超时、可恢复」约定 |
| 11 | n8n 是否胜任工作台 | P5 返工 | ADR-0018 设撤退点（Vue Flow） |
| 12 | **数据/素材版权** | 上线风险 | 武将文本仅内部开发/初始化；图片自行制作，不用官方素材；上线前确认授权 |
| 13 | 状态体积与快照频率（108 牌 + 手牌） | SQLite 写放大 | 每次动作一次快照；P2 测量，必要时压缩/差量（不改事件模型） |
| 14 | 国战兼容 | 未来重构 | ADR-0016 的 mode 接缝 + 隐藏信息/双将预留字段，不在 MVP 实现 |

**需要在 P0 结束前由用户明确确认**：风险 1–4、6–9；其余在对应阶段评审。

## 10. 交付清单（按阶段）

- P0：`rules.md` / `cards.md` / `generals.md` / `state-machine.md` / `legal-actions.md` / `rule-engine-permission.md` + `rules-rulings.md` / `special-zones.md` + 数据模型最终确认 + 数据源结论（武将可爬、牌表人工）。**以上文档已产出，待审核。**
- P1：`crates/storage` 迁移（含 `prop_status_log`）+ `crates/server` `/internal/gameprops/*` + `apps/props-admin`（含定义↔物理牌关联页、生命周期、审核记录）+ `heroes.json` 导入 + seed + 测试。
- P2：`crates/engines/sanguosha` + 视图三分区 + 交互原语 + 单测/黄金局面 + MCP 冒烟。
- P3：`apps/web` 三国杀三分区页面 + 交互 API + WebView 兼容 + 测试。
- P4：标准 25 将 + 军争牌 + 各武将包 + 用例 + 文档。
- P5：工作台/预览 + 模板表 + n8n 或 Vue Flow 选型结论。
- P6：候选剪枝 + 决策日志 + 回放/导出工具。
- P7：`games.ts` 注册 + Q 命令（开局/状态/链接）+ 关闭局面叙述 + 单测。
- P8：权鉴 + 灰度开关 + 观测 + 验收脚本 + 里程碑记录。
