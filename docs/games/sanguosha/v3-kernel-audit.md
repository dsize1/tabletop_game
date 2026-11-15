# V3 内核机制完备审计（`align/mechanics.json` 46 条）

> **生成物，勿手改**：`npm run check:sgs3-kernel -- --write`。
> 事实源：`apps/scripts/lib/sgs3-mechanics.mjs`（机制 → 实现锚点 + 用例锚点）。
> 门禁：`npm run check:sgs3-kernel`（锚点存在性 + 词表双向一致）。

## 为什么有这份表（2026-11-15 用户裁定）

`B0 前置项 P1–P6` 只覆盖「回合/事件/交互/卡牌/装备/生命周期」的**基础流程**，
不等于「46 条机制词表全部进内核」。上一轮把「P1–P6 完成」当成「内核完备」并开始武将批次，
属于**严重漂移**。V3 的口径是：**内核未完备不开始武将批次**；本表就是完备判据。

## 汇总

- 词表机制：**46**
- ✅ 已具备（实现 + 用例锚点）：**46**
- 🟡 部分（只有实现锚点）：**0**
- ❌ 缺失：**0**

## 明细

| # | 机制 | 名称 | 状态 | 实现锚点 | 用例锚点 | 口径要点 |
|---|---|---|---|---|---|---|
| 1 | `card-out-in-game` | 移出/移入游戏 | ✅ | `kernel/mod.rs::Zone`<br>`kernel/flow.rs::gain` | `tests/cards_full.rs::every_card_ends_in_exactly_one_zone` | 移出/移入游戏只走 Void 区 + CardOutOfGame/CardIntoGame 事件；进出判定区不算。 |
| 2 | `damage-vs-hp-loss` | 体力扣除：造成伤害 vs 失去/流失体力 | ✅ | `kernel/flow.rs::damage`<br>`kernel/flow.rs::lose_hp` | `tests/flow.rs::lose_hp_is_not_damage` | 伤害只走 flow::damage（DamageAfter）；失去/流失体力走 flow::lose_hp（HpLost）。 |
| 3 | `card-attributes` | 卡牌属性（类别 / 子类 / 颜色 / 点数） | ✅ | `kernel/cards.rs::CardDef` | `tests/cards_data.rs::damage_kind_is_read_from_field_not_name` | 类别/子类/颜色/点数/标签读 CardDef（生成物），内核零牌名。 |
| 4 | `card-target-attribute` | 牌的目标属性 / 目标指定事件 | ✅ | `kernel/card_play.rs::legal_targets`<br>`kernel/cards.rs::valid_target` | `tests/interact.rs::slash_asks_target_then_shan_blocks` | 目标合法性由目标模式 + 牌效应 + 距离单入口决定；目标指定发 TargetChosen。 |
| 5 | `chained-state` | 连环状态（横置） | ✅ | `kernel/card_play.rs::toggle_chain`<br>`kernel/flow.rs::damage_one` | `tests/cards_full.rs::iron_chain_toggles_and_propagates_elemental_damage` | 横置状态 + 属性伤害传导 + ChainedEnter/ChainedLeft 事件。 |
| 6 | `damage-trick` | 伤害锦囊 | ✅ | `kernel/cards.rs::Tags` | `tests/cards_data.rs::damage_tricks_are_tagged` | 伤害锦囊由 CardDef.tags.damage_trick 声明（征荣等读标签）。 |
| 7 | `deferred-timing` | 延迟派发与顺序 | ✅ | `kernel/deferred.rs::ORDER`<br>`kernel/resolve.rs::advance` | `tests/resolve.rs::deferred_drain_follows_order_table` | 单一延迟队列 + ORDER 顺序表；TurnEndCheck 未到结束阶段不派发。 |
| 8 | `dying-resolution` | 濒死结算 | ✅ | `kernel/flow.rs::enter_dying`<br>`kernel/card_play.rs::step_dying` | `tests/combat.rs::dying_rescued_by_tao` | 濒死由通用 Dying 帧驱动；求救顺序与自我救援单入口。 |
| 9 | `hand-freeze` | 手牌冻结 | ✅ | `kernel/mod.rs::Seat`<br>`kernel/mod.rs::Game::freeze_cards`<br>`kernel/mod.rs::Game::unfreeze_phase`<br>`kernel/card_play.rs::usable_hand` | `tests/kernel_mechanics.rs::frozen_card_cannot_be_used_but_can_be_discarded`<br>`tests/kernel_mechanics.rs::phase_freeze_expires_on_phase_switch_and_turn_freeze_on_turn_end` | 冻结 = 持有者 + 牌集合 + 到期时机（回合/阶段）；阶段切换自动解冻；冻结不阻止被弃置/被获得。 |
| 10 | `judge-zone` | 判定区 | ✅ | `kernel/card_play.rs::place_delayed`<br>`kernel/card_play.rs::judge_zone_lifo` | `tests/cards_full.rs::delayed_tricks_are_judged_lifo` | 判定区 LIFO；进入/离开只发 JudgeZoneEntered/Left（不算弃置）。 |
| 11 | `lose-vs-discard` | 失去牌 / 弃置 | ✅ | `kernel/flow.rs::discard`<br>`kernel/flow.rs::gain` | `tests/cards_full.rs::equipment_goes_to_equip_zone_and_old_card_to_discard` | 失去（CardLost）与弃置（CardDiscarded）是两个事实；获得走 flow::gain。 |
| 12 | `skill-card-zone` | 技能牌堆 / 计数标记 | ✅ | `kernel/mod.rs::Zone`<br>`kernel/mod.rs::Seat` | `tests/invariants.rs::conservation_every_card_in_exactly_one_zone` | 武将牌上的牌走 Zone::Mark（持牌 mark）；纯计数标记走 Seat.counters。 |
| 13 | `skill-status` | 技能状态单一事实源 | ✅ | `kernel/skills.rs::active` | `tests/skills.rs::active_is_union_of_own_and_granted` | 唯一状态源：自带 ∪ granted ∪ 借用（伪帝）；禁止 flags/counters 另表拥有。 |
| 14 | `target-list` | 目标列表（取代 无/单/多 枚举） | ✅ | `kernel/card_play.rs::targets_of` | `tests/interact.rs::multi_target_selection_uses_confirm_option` | 每次使用记录 targets 列表；技能只读列表长度，不按牌名反推。 |
| 15 | `transform-skill-windows` | 转换技窗口 | ✅ | `kernel/skills.rs::Face`<br>`kernel/skills.rs::flip_face` | `tests/b1.rs::transform_skills_split_faces_with_own_windows` | 转换技拆 face，每 face 独立窗口；路由只读当前面。 |
| 16 | `trigger-order` | 同时机结算顺序 | ✅ | `kernel/event.rs::sort_subs` | `tests/invariants.rs::window_other_phase_gates_turn_and_phase` | priority → 当前回合左手侧座位序 → 注册序；不依赖容器迭代顺序。 |
| 17 | `turn-window` | 回合/阶段窗口 | ✅ | `kernel/event.rs::Window` | `tests/invariants.rs::window_other_phase_gates_turn_and_phase` | OwnTurn/OwnPhase/OtherTurn/OtherPhase/AnyTime。 |
| 18 | `use-vs-play` | 使用 / 打出 | ✅ | `kernel/card_play.rs::use_start`<br>`kernel/card_play.rs::played` | `tests/cards_full.rs::every_card_ends_in_exactly_one_zone` | 使用进结算并计次数；打出是响应提供牌，不计使用次数。 |
| 19 | `virtual-card` | 虚拟牌 / 转化牌 | ✅ | `kernel/card_play.rs::use_as`<br>`kernel/card_play.rs::use_virtual`<br>`kernel/mod.rs::Game::virtual_origins` | `tests/kernel_mechanics.rs::use_as_resolves_and_discards_origin_cards`<br>`tests/kernel_mechanics.rs::use_virtual_without_carrier_needs_no_origin` | 生效定义与物理牌分离；origin 可追溯（结算完成整组进弃牌堆）；视为使用可无承载牌。 |
| 20 | `wording-can` | 「可以」措辞约定 | ✅ | `kernel/pending.rs::PendingBuilder` | `tests/invariants.rs::pending_requires_escape_at_type_level` | 「可以」必须有放弃出口（类型级 HasEscape）；强制步骤用 escape_mandatory。 |
| 21 | `wording-then` | 「然后」措辞约定 | ✅ | `kernel/resolve.rs::Step` | `tests/resolve.rs::step_next_writes_initiating_frame_not_stack_top` | 「然后」= 有序 steps（Step::Next 分段），不得同帧连做。 |
| 22 | `zone-ownership` | 牌区唯一归属 | ✅ | `kernel/mod.rs::Game::check_conservation` | `tests/invariants.rs::conservation_rejects_card_without_zone` | 每张牌恰好属于一个区；栈空不得有帧暂存牌。 |
| 23 | `distance-modifier` | 距离修正（叠加与下限） | ✅ | `kernel/gear.rs::distance` | `tests/equip_effects.rs::horse_modifiers_change_distance` | 距离单入口：坐骑 + 技能修正叠加，下限 1。 |
| 24 | `dying-aftermath` | 濒死后的延后结算（兴衰式） | ✅ | `kernel/flow.rs::settle_after_dying`<br>`kernel/card_play.rs::dying_order` | `tests/kernel_mechanics.rs::dying_aftermath_damages_supporters_after_rescue`<br>`tests/kernel_mechanics.rs::dying_order_left_first_puts_victim_last` | 「濒死结算结束后」队列：先脱困再逐个结算；兴衰求桃顺序 = 左手侧 → 自己最后。 |
| 25 | `discard-contract` | 「弃置」契约（弃置 ≠ 使用；判定区不算弃置） | ✅ | `kernel/flow.rs::discard`<br>`kernel/card_play.rs::remove_card` | `tests/tricks.rs::judged_card_goes_to_discard` | 弃置只从手牌/装备区进弃牌堆；判定区移出只发 JudgeZoneLeft。 |
| 26 | `reveal` | 展示（立即公共可见） | ✅ | `kernel/mod.rs::Game::reveal`<br>`kernel/mod.rs::Game::conceal`<br>`kernel/view.rs::public_state` | `tests/kernel_mechanics.rs::reveal_is_public_until_concealed` | 展示 = 当下进入公共可见集合 + CardRevealed；结算后 conceal；只写日志不算展示。 |
| 27 | `damage-attributes` | 伤害属性（普通 / 火 / 雷）与连环传导 | ✅ | `kernel/mod.rs::DamageSpec`<br>`kernel/flow.rs::damage` | `tests/flow.rs::tengjia_blocks_normal_slash` | damage_kind × source_kind 二维；转化/虚拟牌按生效定义定属性。 |
| 28 | `response-cards` | 响应牌（无懈链 / 万箭 / 南蛮…） | ✅ | `kernel/cards.rs::ResponseKind`<br>`kernel/card_play.rs::can_respond` | `tests/cards_full.rs::archery_requires_shan` | 响应牌读 CardDef 标签；无懈链奇偶层判定。 |
| 29 | `pindian` | 拼点（同时暗选 → 亮牌比点数） | ✅ | `kernel/pindian.rs::start`<br>`kernel/pindian.rs::step_pindian`<br>`kernel/pindian.rs::PindianResult` | `tests/kernel_mechanics.rs::pindian_is_hidden_then_revealed_and_goes_to_discard`<br>`tests/kernel_mechanics.rs::pindian_tie_has_no_winner_and_requires_cards` | 双方暗选 → 同时亮牌 → 有效点数 → 平局=未赢 → 拼点牌进弃牌堆；结果单通道 take_pindian。 |
| 30 | `target-extra` | 目标数修正（多指定目标 / 无距离） | ✅ | `kernel/mod.rs::UseOpts`<br>`kernel/card_play.rs::target_bounds` | `tests/kernel_mechanics.rs::extra_targets_raise_bounds_and_keep_legality` | 目标数 +N（必须同时是合法目标）；本次无距离限制；legal 与 execute 同源。 |
| 31 | `deck-carrier` | 牌堆承载使用（使用牌堆中的牌） | ✅ | `kernel/card_play.rs::use_from_pile` | `tests/kernel_mechanics.rs::deck_carrier_uses_public_pile_card` | 牌堆/弃牌堆承载牌走真实结算（响应/防具/属性/伤害），结算后进弃牌堆。 |
| 32 | `card-gained` | 「获得」语义（获得 = 事件链） | ✅ | `kernel/flow.rs::gain` | `tests/cards_full.rs::snatch_gains_target_card` | 获得 = 事件链（CardGained）；摸牌是 CardDrawn，不算「获得他人的牌」。 |
| 33 | `multi-response` | 多张响应（响应计数 N） | ✅ | `kernel/mod.rs::UseOpts`<br>`kernel/card_play.rs::declare_multi_response` | `tests/kernel_mechanics.rs::multi_response_requires_n_cards_to_block` | 同一响应窗口连续 N 张同类响应；每张计 1；中断视为未抵消。 |
| 34 | `no-wuxie` | 不能被【无懈可击】抵消 | ✅ | `kernel/card_play.rs::declare_no_wuxie`<br>`kernel/card_play.rs::step_wuxie` | `tests/kernel_mechanics.rs::no_wuxie_skips_wuxie_window_but_not_other_rules` | 只关无懈窗口（本次使用），不影响闪/杀响应；视图/日志仍可见事实。 |
| 35 | `general-pool` | 武将牌池（化身 / 新生） | ✅ | `generals_generated.rs::GENERALS`<br>`kernel/mod.rs::Game::init_huashen` | `tests/kernel_mechanics.rs::huashen_pool_only_has_generals_with_registered_skills` | 池只含「至少一个已注册技能」的 Live 武将；随机走 rand_below；性别/势力覆盖跟随展示化身。 |
| 36 | `hidden-declaration` | 扣置声明 + 质疑链（蛊惑） | ✅ | `kernel/guhuo.rs::start`<br>`kernel/guhuo.rs::step_guhuo`<br>`kernel/guhuo.rs::GuhuoResult` | `tests/kernel_mechanics.rs::hidden_declaration_reveals_only_after_doubts_and_settles_truth` | 扣置期不公开；同时收集质疑；真假只读物理定义；缠怨为独立状态（Seat.counters）。 |
| 37 | `aoe-target-trim` | 群体锦囊少选目标 | ✅ | `kernel/card_play.rs::declare_trim_targets`<br>`kernel/card_play.rs::target_bounds` | `tests/kernel_mechanics.rs::aoe_trim_asks_fewer_targets_before_resolution` | 群体锦囊少选 N 个目标：发生在目标确定阶段（无懈/响应之前）。 |
| 38 | `sha-intercept` | 【杀】结算前介入（目标锁定技） | ✅ | `kernel/mod.rs::ShaIntercept`<br>`kernel/card_play.rs::declare_intercept`<br>`kernel/skills.rs::fire_sha_intercept` | `tests/kernel_mechanics.rs::sha_intercept_can_make_slash_immune_or_unresponsive` | 成为【杀】目标后、响应窗口之前：Continue/Immune/Unresponsive/Replace。 |
| 39 | `judge-reversal` | 判定效果反转（思蜀） | ✅ | `kernel/mod.rs::Game::reverse_judge`<br>`kernel/turn.rs::judge_phase` | `tests/kernel_mechanics.rs::judge_reversal_flips_indulgence_branch_only` | 按角色绑定的判定效果反转：只改分支不改牌面花色；可视图（judge_reverse）。 |
| 40 | `range-override` | 距离/范围覆盖（按牌属性） | ✅ | `kernel/mod.rs::UseOpts`<br>`kernel/card_play.rs::legal_targets_with` | `tests/kernel_mechanics.rs::range_override_extends_reach_for_this_use_only` | 按牌属性覆盖本次距离口径（烈弓）；逐目标、不改全局攻击范围。 |
| 41 | `virtual-equip` | 虚拟装备（视为装备） | ✅ | `kernel/skills.rs::virtual_equip`<br>`kernel/gear.rs::virtual_card` | `tests/kernel_mechanics.rs::virtual_equip_is_read_only_when_real_slot_empty` | 视为装备（八阵）单入口：真实同类装备优先；不占槽、不参与守恒。 |
| 42 | `hand-limit` | 手牌上限（叠加 / 赋值 / 豁免） | ✅ | `kernel/mod.rs::Game::hand_limit`<br>`kernel/skills.rs::hand_limit_bonus` | `tests/b1.rs::yongsi_discard_discards_all_when_short` | 手牌上限单入口：体力 + 技能修正 + 覆盖（不屈）+ 房规上限。 |
| 43 | `sha-limit` | 【杀】次数上限修正 | ✅ | `kernel/card_play.rs::sha_limit_reached`<br>`kernel/skills.rs::sha_unlimited` | `tests/equip_effects.rs::zhugeliannu_lifts_slash_limit` | 【杀】次数单入口：连弩/技能无限 + 技能加成。 |
| 44 | `limit-skill` | 限定技（每局一次） | ✅ | `kernel/skills.rs::limit_gate`<br>`kernel/skills.rs::mark_limited_used`<br>`kernel/skills.rs::active_actions` | `tests/kernel_mechanics.rs::limited_skill_is_usable_once_per_game` | 限定技每局一次：已使用 → legal/选项过滤；状态存座位计数器（可回放）。 |
| 45 | `equip-move` | 装备移动（场上一张装备牌） | ✅ | `kernel/gear.rs::move_equip` | `tests/kernel_mechanics.rs::equip_move_requires_empty_target_slot_and_emits_lost` | 场上装备移槽（设变）：非弃置非获得；目标槽必须为空；发 CardLost（失去装备钩子）。 |
| 46 | `armor-points` | 护甲（独立防护值） | ✅ | `kernel/mod.rs::Seat`<br>`kernel/flow.rs::damage_one`<br>`kernel/mod.rs::Game::add_armor` | `tests/kernel_mechanics.rs::armor_absorbs_damage_before_hp_and_emits_event`<br>`tests/kernel_mechanics.rs::armor_zero_does_not_absorb` | 护甲先吸收伤害（伤害事件仍成立，amount = 原值）；ArmorChanged + 视图；溢出扣体力。 |

## 机制词表来源

- `docs/games/sanguosha/align/mechanics.json` version=2 updated_at=2026-11-09
- 机制定义/不变量变更必须走该文件（bump version 会让已确认技能进入冲突阻断）。
