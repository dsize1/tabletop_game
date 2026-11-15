# V3 引擎 B0 前置项（P1–P6）落地说明

> 状态：**✅ 2026-11-14 验收**（`npm run check:sgs3` 全绿）。
> 对应计划：`memory_palace/plan/next-v3-rebuild.md` §0.1 / `memory_palace/plan/next-v3-collection.md` §10。
> 验收准则：**不添加任何英雄技能**，游戏即可完整对局（引擎 + 手牌 + 装备牌 + 锦囊牌）到终局。

## 1. 为什么先做前置项

上一轮（2026-11-13）把「类型骨架 + 合成测试」误报为 M1–M4 完成。实测缺陷：

- 据守永不触发（无阶段驱动）、解围 no-op、颂威硬编码座位 0、放逐无选择、不屈简化、奋激/行殇靠测试手推事件。

结论：**B0 技能不是「写 handler」，而是「handler 被真实机制驱动」**。因此先把六个基本机制做实：

| # | 前置项 | 落点 | 门禁 |
|---|---|---|---|
| P1 | 回合驱动 | `kernel/turn.rs`（阶段机 + `PHASES` + 跳阶段/额外回合/翻面/轮数） | `tests/turn_flow.rs` |
| P2 | 事件生产 | `flow.rs` / `card_play.rs` / `judge.rs` 全走 `Game::push_event`；`Game.log` 事件历史 | `tests/full_game.rs` 事件清单 |
| P3 | 交互层 | `pending` + `resolve::submit`（选目标/选牌/多选+确定/二选一）；`Step::NeedWith` | `tests/interact.rs` |
| P4 | 完整卡牌结算 | `cards::Effect` 分派 + 无懈链 + 判定区 LIFO + 濒死/酒 | `tests/cards_full.rs` |
| P5 | 装备结算 | `gear::` 单入口 + 11 武器特效 + 4 防具 + 坐骑距离 | `tests/equip_effects.rs` |
| P6 | 生命周期 | 开局（身份/选将/发牌）/ 终局 / 中断 / 5–8 人自动对局 | `tests/full_game.rs` |

合计 **68 条新用例**（turn_flow 12 / interact 11 / cards_full 21 / equip_effects 19 / full_game 5）。

## 2. P1 回合驱动

- 唯一驱动：`turn::start` → `FlowKind::Turn` 流程帧 → `turn::step_turn` 阶段机。
- 阶段表 `turn::PHASES = [Prepare, Judge, Draw, Play, Discard, End]`（唯一事实源）。
- 事件：`TurnStart` / `PhaseStart` / `PhaseEnd` / `TurnEnd`（都带 turn/phase 快照）。
- 插口（技能只调这三个）：
  - `turn::skip_phase(g, phase)`：跳阶段（乐不思蜀 / 兵粮寸断 / 技能）；
  - `turn::extra_turn(g, seat)`：额外回合（下一个回合轮到时优先消费）；
  - `Seat.flipped`：翻面 → 跳过其下一个回合（`end_turn` 内消费）。
- 轮数：绕圈回到 <= 游标的座位时 `round += 1`。
- 出牌阶段：`pending(kind="play_act")` 列可用牌/技能 + 出口 `end`；
  弃牌阶段：`pending(kind="discard_phase")`，**强制选择**（`escape_mandatory`，提交出口被拒）。
- 手牌上限：`Game::hand_limit(seat)` = 体力 + `Hook::HandLimit` 修正（单出口）。

## 3. P2 事件生产

- `Game::push_event` 是**唯一入队口**：同时写「待路由队列 `events`」与「历史日志 `log`」。
- 结算路径自动产事件（不再由测试手推）：
  - 摸牌 `CardDrawn`（与 `CardGained` 区分：摸牌不得触发「他人获得牌」类订阅，奋激教训）；
  - 使用 `CardUsed` + 失去手牌 `CardLost`；打出 `CardPlayed`（闪/无懈/响应杀）；
  - 弃置 `CardDiscarded` + `CardLost`；获得 `CardGained` + 来源方 `CardLost`；
  - 装备 `CardIntoGame` / `ArmorChanged`；判定 `JudgeRevealed` / `JudgeEffective`；
  - 延时锦囊进/出判定区 `JudgeZoneEntered` / `JudgeZoneLeft`；
  - 伤害 `DamageAfter`（二维 kind×source）/ 失体力 `HpLost` / 回血 `Healed` / 减上限 `MaxHpChanged`；
  - 死亡 `Death`（弃牌事件一并 emit）；铁索 `ChainedEnter` / `ChainedLeft`。
- 路由时机：栈顶是**回合流程帧**或栈空时按订阅派发（常驻回合帧不再阻塞技能触发；濒死中断优先结算完）。

## 4. P3 交互层

| 询问 | pending kind | 选项前缀 | 出口 |
|---|---|---|---|
| 出牌阶段行动 | `play_act` | `card:<id>` / `card:<id>:fire`（朱雀）/ `skill:<code>` / `zhangba` | `end_play`（Web「结束出牌」按钮；兼容旧 id `end`） |
| 选目标（单/多选+确定） | `sgs3_target` | `t:<seat>` / `ok` | `cancel`（取消使用，牌回手牌并回退计数） |
| 需要【闪】 | `shan_ask` | `s:<card>` / `bagua` | `pass` |
| 南蛮/万箭响应 | `aoe_ask` | `p:<card>` | `pass` |
| 决斗响应 | `duel_ask` | `s:<card>` | `pass` |
| 无懈链 | `wuxie`（映射 Web 无懈浮层） | `w:<card>` | `pass` |
| 拆/顺选牌 | `sgs3_pick` | `c:<card>` | `pass` |
| 寒冰/麒麟可选 | `hanbing_opt` / `hanbing_pick` / `qilin_opt` | `c:` / `q:` | `pass` / `跳过` |
| 五谷选牌 | `sgs3_harvest`（`special_zones.wugu` 同时投影亮牌） | `c:<card>` | 强制（占位出口） |
| 火攻（展示/弃牌） | `fire_attack_reveal` / `fire_attack_discard` | `r:` / `s:` | 强制 / `pass` |
| 借刀（出杀/交武器） | `collateral_sha` / `collateral_shan` | `s:` | `pass` |
| 雌雄双股剑二选一 | `cixiong` | `discard:<card>` / `draw` | 强制（占位出口） |
| 濒死求桃 | `dying`（映射 Web 濒死浮层） | `card:<card>`（桃 / 本人酒） | `pass` |
| 弃牌阶段 | `discard_phase` | `card:<card>` | 强制（占位出口） |

> **Web 兼容命名**：`end_play` / `dying` / `wuxie` 沿用 v2 的 pending 名以复用现有浮层；
> 其余 v3 专属询问用 `sgs3_*` 前缀，端侧回退为「选项按钮列表」（`apps/web` 未知 kind 的默认渲染）。

不变量：

1. **只有合法集为空才不询问**（I1）：目标没有【闪】→ 不产生 pending，直接结算；
2. 提交校验：`actor` / 合法选项 / 强制选择拒绝出口 → 报错且**状态不变**；
3. handler 状态持久化：`Step::Need`/`NeedAt` **不改 data**，需要存中间状态（已选目标、已亮出的牌）时用
   `Step::NeedWith(stage, data, pending)`（本地 Map 改动不会自动回写）。

## 5. P4 完整卡牌结算

- 分派只读 `CardDef.effect`（内核零牌名）；生成脚本 `gen:sgs-engine-data` 产出 `Effect`/`respond`/`lianhuan` 字段。
- 流程：`使用（use_start：事件 + 牌进处理区）` → `选目标` → `无懈链（奇偶层）` → `效果结算` → `落盘`。
- 已覆盖：杀（火/雷/酒/古锭/藤甲/青釭/铁索传导）、闪、桃、酒（加伤 + 濒死自救）、
  无中/拆/顺/决斗/南蛮/万箭/五谷/桃园/火攻/借刀/铁索、
  延时锦囊 乐（红桃免跳）/兵（梅花免跳）/闪电（黑桃 2–9 三雷，否则移给下家）+ 判定区 LIFO。
- 终局房规：`round > 100` 起桃/桃园回血无效（濒死救援不受限，`Game::endgame_no_heal`）。

## 6. P5 装备结算

| 武器 | 实现点 |
|---|---|
| 青釭剑 | `gear::ignores_armor` → 伤害链/仁王盾/八卦阵都跳过防具 |
| 诸葛连弩 | `card_play::sha_limit_reached` 无限【杀】 |
| 丈八蛇矛 | `begin_use_zhangba`：两张手牌 → 虚拟【杀】（`v:` 牌进 `Void`，守恒） |
| 方天画戟 | `target_bounds`：最后一张手牌可指定最多 3 个目标 |
| 古锭刀 | 目标无手牌 → 伤害 +1 |
| 朱雀羽扇 | 出牌阶段 `card:<id>:fire` 选项（转化牌按生效定义定属性） |
| 麒麟弓 | 伤害后可选弃置目标一张坐骑 |
| 寒冰剑 | 伤害前可选「防止伤害，改为弃其两张牌」 |
| 青龙偃月刀 | 被【闪】抵消后可再使用一张【杀】 |
| 贯石斧 | 被【闪】抵消后可弃两张牌强制命中 |
| 雌雄双股剑 | 异性目标二选一（弃一张手牌 / 令使用者摸一张） |
| 防具 | 八卦阵（红判定视为【闪】，黑判定仍可出闪）/ 仁王盾（黑杀无效）/ 藤甲（普通杀·南蛮·万箭无效，火 +1）/ 白银狮子（伤害恒 1；离开装备区回 1 体力） |
| 坐骑 | `gear::distance` = 环距 − 进攻马 + 防御马 + `Hook::Distance`，下限 1 |

## 7. P6 生命周期

- `turn::new_game(n)` / `turn::begin_game(g, identities, hand_cards)`（无武将）/
  `turn::begin_game_with_generals(g, identities, generals, hand_cards)`（选将落地：势力/体力/性别/主公 +1 体力上限）。
- 初始手牌 **4 张**（`v3-parity-baseline.md` §5.1 已裁定，旧「主公 6 张」作废）。
- 终局：`identity::check_winner`（主死→反贼胜；反贼+内奸全灭→主忠胜），`Game.winner` 置位后回合帧不再推进。
- 中断：`turn::abort(g)`（清栈/待办/事件，保留牌区事实）。
- 死亡：`turn::settle_death`（弃牌事件 + 身份公开 + 奖惩 + 胜负）单一出口。
- 全流程验收：`tests/full_game.rs` 自动策略把 2/5/6 人局打到底，每步校验守恒、无帧遗留、询问完整性、事件完整性。

## 8. 端侧接入（2026-11-14，V3 接管 `sanguosha`）

> 决策：ADR-0043（V3 接管正式 id，v2 不再注册）。**真实可玩链路**：
> `#开局 三国杀` → `#加入`/`#加机器人`/`#开始` → Web 选将 → 出牌/响应 → 终局。

| 层 | 实现 | 位置 |
|---|---|---|
| 引擎插件 | `EnginePlugin`（`game_id = "sanguosha"`），`data` = `EngineState{kernel, cfg, lobby, setup}` | `sanguosha3/src/engine/mod.rs` |
| 大厅 / 选将 | 动作 id 与 v1/v2 完全一致（`lobby:join/leave/start/abort/add_bot/kick:*`、`lobby:set_{packs,general_packs,lord_candidate_count,other_candidate_count,max_hand_cards,double_spy}`、`setup:choose_general:<code>`） | `engine/lobby.rs` |
| 视图 | 输出 v1/v2 同形 `SgsViewData`（Web 零改动）；隐藏信息只在 `player_area`；`history[].seq` 单调 | `engine/view.rs`、`engine/text.rs` |
| 机器人 | 打分策略（敌我仅按公开身份；救自己/队友、不救敌人；不取消目标）+ `drive_bots`（server `/drive`） | `engine/bot.rs` |
| 注册 | MCP / server 注册 `sanguosha3::engine::SanguoshaEngine`（v2 不再注册）；网关 `GAME_OPTIONS` id 不变 | `game-engine-mcp/src/main.rs`、`server/src/main.rs` |

口径说明：

- **不挂武将技能**（`Seat.skills` 为空）：21 批次完成前也能完整对局；武将只提供势力/体力/性别/名字；
- 选将池 = **Live 口径 147 位**（与 v2 / 全武将验证批次 / 中台同源）：
  - 生成器登记的名录是 **232**（标准 25 + 风林火山 48 + 一将成名 72 + 界限突破 87）；
  - 其中 **85 位是被「界/将」同名去重排除的原版**（如标准关羽 `sgs_hero_2` → `Offline`），
    选将池只取 `Status::Live` → **147**（风林火山 19 + 一将 41 + 界 87；标准包 25 位全部有界版 → 单 `standard` 可用 0 位）；
  - 仍**不按技能实现度过滤**（技能按批次接入，未接入不影响开局）。
- 卡池/身份/发牌：5–8 人，标准身份配比（8 人可双内）；初始手牌 4 张；主公 +1 体力上限；
- 牌堆耗尽时弃牌堆洗回（内核 `Game.rng`，可序列化回放）。

## 8.1 收尾补齐（2026-11-14，第二轮审计发现并修复）

| 缺口 | 修复 | 回归 |
|---|---|---|
| 内奸独胜未实现（主死一律判反贼胜） | `identity::check_winner`（用户 2026-11-14 口径）：主死时**仅存活主公+内奸**→ `Winner::Spy`；其余主死→反贼胜；双内同时存活不达成 | `tests/prereq_gaps.rs`（5 情形） |
| 【桃】在第 100 轮后仍回血 | 出牌阶段 `Effect::Tao` 走 `endgame_no_heal` 判定（濒死救援不受限） | 同上 |
| 【兵粮寸断】缺「距离 1 以内」 | `legal_targets` 增加 `SupplyShortage` 距离过滤 | 同上 |
| 【铁索连环】**重铸**未实现 | 出牌阶段 `recast:<id>` 选项 → `card_play::recast`（弃此牌摸一张，不计「使用」） | 同上 |
| 装备被替换的旧牌只发 `CardLost` | 同时 emit `CardDiscarded`（进弃牌堆事实完整） | 同上 |
| 当前回合角色阵亡后仍继续其回合 | `turn::step_turn` 前置守卫：死者回合立即 `end_turn`（终局则停止） | 同上 |
| 五谷剩余牌进弃牌堆无事件 | `discard_processed` 补 `CardDiscarded` | 同上 |

> P5 口径更正：装备特效武器为 **11 件**（标准 9 + 军争 2：寒冰剑/朱雀羽扇），防具 4 件，
> 坐骑按距离规则；`木牛流马`（宝物）仅入表，未做运输交互。

## 8.2 与既有工具的接线

| 工具 | 现状 | 说明 |
|---|---|---|
| 决策回放 `server --bin sgs-replay` | ✅ **已支持 V3** | 按快照形态分派引擎（`data = EngineState` → V3；旧 v2 快照仍可回放），digest 用 `engine::bot::state_digest_of`（与 server 决策日志同源） |
| 道具中台 `/internal/gameprops/*` | ⚠️ **仍读 v2 数据** | 目录/「技能已实现」口径绑定 v2 注册表（147 上线将 + 296 技能）；切 V3 会改变语义（232 全量 + 按批次接入的技能状态），需产品口径确认后再迁 |

## 9. 本切片明确**未做**（下一片）

1. **B0 武将技能**（据守/解围/不屈/奋激/行殇/放逐/颂威）：按 `align/B0.json` 断言逐条收口。
   - **进度（2026-11-14 起）**：`jushou（据守）` ✅ 已收口并挂载 —— 结束阶段真实触发（`PhaseStart`+自身 End 窗口）、
     Q1=B 取反翻面、摸 4 → 选 1 张手牌（装备牌改为使用）→ 按上限弃牌收尾；
     其余 6 个（解围/不屈/奋激/行殇/放逐/颂威）仍为骨架，**不挂载**；
   - 挂载开关：`kernel::skills::ENABLED_SKILLS`（逐条收口后追加），选将只挂「已注册 + 已收口」技能；
   - 选将中文名 → code 映射：`skills::code_by_name`（V3 注册表）。
2. **未完杀 / 技能救援**（濒死扩展）：随 B0 技能 / B3 批次。
3. **v2 专属浮层**（拼点/观星/蛊惑/化身/勤王）：V3 尚无对应 pending 形态，随对应武将批次补特殊区。
4. `木牛流马`（宝物）仅入表，未做运输交互。
