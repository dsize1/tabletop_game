// V3 内核机制支持矩阵（单一事实源）。
//
// 口径：`docs/games/sanguosha/align/mechanics.json` 的 46 条机制词表 = 内核完备的判据。
// 每条机制必须给出：
//   - `impl`  ：内核实现锚点（相对 `crates/engines/sanguosha3/src`，形如 `kernel/flow.rs::damage`
//               或 `kernel/mod.rs::Zone`，符号可是 fn/struct/enum/const/type）；
//   - `tests` ：用例锚点（相对 `crates/engines/sanguosha3`，形如 `tests/flow.rs::lose_hp_is_not_damage`）；
//   - `note`  ：口径要点（人读；生成 `v3-kernel-audit.md`）。
// 缺任一锚点 = 机制缺口（`check:sgs3-kernel --strict` 失败）。
//
// 维护规则：
//   1. 先补内核实现 + 用例，再把锚点填进来；**不得**先填锚点后补实现（门禁会校验符号存在）；
//   2. 机制定义/不变量变更必须走 `align/mechanics.json`（bump version 会让已确认技能进入冲突阻断）；
//   3. `docs/games/sanguosha/v3-kernel-audit.md` 由本文件生成（`--write`），禁止手改。

export const MECHANICS = {
  "card-out-in-game": {
    impl: ["kernel/mod.rs::Zone", "kernel/flow.rs::gain"],
    tests: ["tests/cards_full.rs::every_card_ends_in_exactly_one_zone"],
    note: "移出/移入游戏只走 Void 区 + CardOutOfGame/CardIntoGame 事件；进出判定区不算。",
  },
  "damage-vs-hp-loss": {
    impl: ["kernel/flow.rs::damage", "kernel/flow.rs::lose_hp"],
    tests: ["tests/flow.rs::lose_hp_is_not_damage"],
    note: "伤害只走 flow::damage（DamageAfter）；失去/流失体力走 flow::lose_hp（HpLost）。",
  },
  "card-attributes": {
    impl: ["kernel/cards.rs::CardDef"],
    tests: ["tests/cards_data.rs::damage_kind_is_read_from_field_not_name"],
    note: "类别/子类/颜色/点数/标签读 CardDef（生成物），内核零牌名。",
  },
  "card-target-attribute": {
    impl: ["kernel/card_play.rs::legal_targets", "kernel/cards.rs::valid_target"],
    tests: ["tests/interact.rs::slash_asks_target_then_shan_blocks"],
    note: "目标合法性由目标模式 + 牌效应 + 距离单入口决定；目标指定发 TargetChosen。",
  },
  "chained-state": {
    impl: ["kernel/card_play.rs::toggle_chain", "kernel/flow.rs::damage_one"],
    tests: ["tests/cards_full.rs::iron_chain_toggles_and_propagates_elemental_damage"],
    note: "横置状态 + 属性伤害传导 + ChainedEnter/ChainedLeft 事件。",
  },
  "damage-trick": {
    impl: ["kernel/cards.rs::Tags"],
    tests: ["tests/cards_data.rs::damage_tricks_are_tagged"],
    note: "伤害锦囊由 CardDef.tags.damage_trick 声明（征荣等读标签）。",
  },
  "deferred-timing": {
    impl: ["kernel/deferred.rs::ORDER", "kernel/resolve.rs::advance"],
    tests: ["tests/resolve.rs::deferred_drain_follows_order_table"],
    note: "单一延迟队列 + ORDER 顺序表；TurnEndCheck 未到结束阶段不派发。",
  },
  "dying-resolution": {
    impl: ["kernel/flow.rs::enter_dying", "kernel/card_play.rs::step_dying"],
    tests: ["tests/combat.rs::dying_rescued_by_tao"],
    note: "濒死由通用 Dying 帧驱动；求救顺序与自我救援单入口。",
  },
  "hand-freeze": {
    impl: [
      "kernel/mod.rs::Seat",
      "kernel/mod.rs::Game::freeze_cards",
      "kernel/mod.rs::Game::unfreeze_phase",
      "kernel/card_play.rs::usable_hand",
    ],
    tests: [
      "tests/kernel_mechanics.rs::frozen_card_cannot_be_used_but_can_be_discarded",
      "tests/kernel_mechanics.rs::phase_freeze_expires_on_phase_switch_and_turn_freeze_on_turn_end",
    ],
    note: "冻结 = 持有者 + 牌集合 + 到期时机（回合/阶段）；阶段切换自动解冻；冻结不阻止被弃置/被获得。",
  },
  "judge-zone": {
    impl: ["kernel/card_play.rs::place_delayed", "kernel/card_play.rs::judge_zone_lifo"],
    tests: ["tests/cards_full.rs::delayed_tricks_are_judged_lifo"],
    note: "判定区 LIFO；进入/离开只发 JudgeZoneEntered/Left（不算弃置）。",
  },
  "lose-vs-discard": {
    impl: ["kernel/flow.rs::discard", "kernel/flow.rs::gain"],
    tests: ["tests/cards_full.rs::equipment_goes_to_equip_zone_and_old_card_to_discard"],
    note: "失去（CardLost）与弃置（CardDiscarded）是两个事实；获得走 flow::gain。",
  },
  "skill-card-zone": {
    impl: ["kernel/mod.rs::Zone", "kernel/mod.rs::Seat"],
    tests: ["tests/invariants.rs::conservation_every_card_in_exactly_one_zone"],
    note: "武将牌上的牌走 Zone::Mark（持牌 mark）；纯计数标记走 Seat.counters。",
  },
  "skill-status": {
    impl: ["kernel/skills.rs::active"],
    tests: ["tests/skills.rs::active_is_union_of_own_and_granted"],
    note: "唯一状态源：自带 ∪ granted ∪ 借用（伪帝）；禁止 flags/counters 另表拥有。",
  },
  "target-list": {
    impl: ["kernel/card_play.rs::targets_of"],
    tests: ["tests/interact.rs::multi_target_selection_uses_confirm_option"],
    note: "每次使用记录 targets 列表；技能只读列表长度，不按牌名反推。",
  },
  "transform-skill-windows": {
    impl: ["kernel/skills.rs::Face", "kernel/skills.rs::flip_face"],
    tests: ["tests/b1.rs::transform_skills_split_faces_with_own_windows"],
    note: "转换技拆 face，每 face 独立窗口；路由只读当前面。",
  },
  "trigger-order": {
    impl: ["kernel/event.rs::sort_subs"],
    tests: ["tests/invariants.rs::window_other_phase_gates_turn_and_phase"],
    note: "priority → 当前回合左手侧座位序 → 注册序；不依赖容器迭代顺序。",
  },
  "turn-window": {
    impl: ["kernel/event.rs::Window"],
    tests: ["tests/invariants.rs::window_other_phase_gates_turn_and_phase"],
    note: "OwnTurn/OwnPhase/OtherTurn/OtherPhase/AnyTime。",
  },
  "use-vs-play": {
    impl: ["kernel/card_play.rs::use_start", "kernel/card_play.rs::played"],
    tests: ["tests/cards_full.rs::every_card_ends_in_exactly_one_zone"],
    note: "使用进结算并计次数；打出是响应提供牌，不计使用次数。",
  },
  "virtual-card": {
    impl: [
      "kernel/card_play.rs::use_as",
      "kernel/card_play.rs::use_virtual",
      "kernel/mod.rs::Game::virtual_origins",
    ],
    tests: [
      "tests/kernel_mechanics.rs::use_as_resolves_and_discards_origin_cards",
      "tests/kernel_mechanics.rs::use_virtual_without_carrier_needs_no_origin",
    ],
    note: "生效定义与物理牌分离；origin 可追溯（结算完成整组进弃牌堆）；视为使用可无承载牌。",
  },
  "wording-can": {
    impl: ["kernel/pending.rs::PendingBuilder"],
    tests: ["tests/invariants.rs::pending_requires_escape_at_type_level"],
    note: "「可以」必须有放弃出口（类型级 HasEscape）；强制步骤用 escape_mandatory。",
  },
  "wording-then": {
    impl: ["kernel/resolve.rs::Step"],
    tests: ["tests/resolve.rs::step_next_writes_initiating_frame_not_stack_top"],
    note: "「然后」= 有序 steps（Step::Next 分段），不得同帧连做。",
  },
  "zone-ownership": {
    impl: ["kernel/mod.rs::Game::check_conservation"],
    tests: ["tests/invariants.rs::conservation_rejects_card_without_zone"],
    note: "每张牌恰好属于一个区；栈空不得有帧暂存牌。",
  },
  "distance-modifier": {
    impl: ["kernel/gear.rs::distance"],
    tests: ["tests/equip_effects.rs::horse_modifiers_change_distance"],
    note: "距离单入口：坐骑 + 技能修正叠加，下限 1。",
  },
  "dying-aftermath": {
    impl: ["kernel/flow.rs::settle_after_dying", "kernel/card_play.rs::dying_order"],
    tests: [
      "tests/kernel_mechanics.rs::dying_aftermath_damages_supporters_after_rescue",
      "tests/kernel_mechanics.rs::dying_order_left_first_puts_victim_last",
    ],
    note: "「濒死结算结束后」队列：先脱困再逐个结算；兴衰求桃顺序 = 左手侧 → 自己最后。",
  },
  "discard-contract": {
    impl: ["kernel/flow.rs::discard", "kernel/card_play.rs::remove_card"],
    tests: ["tests/tricks.rs::judged_card_goes_to_discard"],
    note: "弃置只从手牌/装备区进弃牌堆；判定区移出只发 JudgeZoneLeft。",
  },
  "reveal": {
    impl: [
      "kernel/mod.rs::Game::reveal",
      "kernel/mod.rs::Game::conceal",
      "kernel/view.rs::public_state",
    ],
    tests: ["tests/kernel_mechanics.rs::reveal_is_public_until_concealed"],
    note: "展示 = 当下进入公共可见集合 + CardRevealed；结算后 conceal；只写日志不算展示。",
  },
  "damage-attributes": {
    impl: ["kernel/mod.rs::DamageSpec", "kernel/flow.rs::damage"],
    tests: ["tests/flow.rs::tengjia_blocks_normal_slash"],
    note: "damage_kind × source_kind 二维；转化/虚拟牌按生效定义定属性。",
  },
  "response-cards": {
    impl: ["kernel/cards.rs::ResponseKind", "kernel/card_play.rs::can_respond"],
    tests: ["tests/cards_full.rs::archery_requires_shan"],
    note: "响应牌读 CardDef 标签；无懈链奇偶层判定。",
  },
  "pindian": {
    impl: [
      "kernel/pindian.rs::start",
      "kernel/pindian.rs::step_pindian",
      "kernel/pindian.rs::PindianResult",
    ],
    tests: [
      "tests/kernel_mechanics.rs::pindian_is_hidden_then_revealed_and_goes_to_discard",
      "tests/kernel_mechanics.rs::pindian_tie_has_no_winner_and_requires_cards",
    ],
    note: "双方暗选 → 同时亮牌 → 有效点数 → 平局=未赢 → 拼点牌进弃牌堆；结果单通道 take_pindian。",
  },
  "target-extra": {
    impl: ["kernel/mod.rs::UseOpts", "kernel/card_play.rs::target_bounds"],
    tests: ["tests/kernel_mechanics.rs::extra_targets_raise_bounds_and_keep_legality"],
    note: "目标数 +N（必须同时是合法目标）；本次无距离限制；legal 与 execute 同源。",
  },
  "deck-carrier": {
    impl: ["kernel/card_play.rs::use_from_pile"],
    tests: ["tests/kernel_mechanics.rs::deck_carrier_uses_public_pile_card"],
    note: "牌堆/弃牌堆承载牌走真实结算（响应/防具/属性/伤害），结算后进弃牌堆。",
  },
  "card-gained": {
    impl: ["kernel/flow.rs::gain"],
    tests: ["tests/cards_full.rs::snatch_gains_target_card"],
    note: "获得 = 事件链（CardGained）；摸牌是 CardDrawn，不算「获得他人的牌」。",
  },
  "multi-response": {
    impl: ["kernel/mod.rs::UseOpts", "kernel/card_play.rs::declare_multi_response"],
    tests: ["tests/kernel_mechanics.rs::multi_response_requires_n_cards_to_block"],
    note: "同一响应窗口连续 N 张同类响应；每张计 1；中断视为未抵消。",
  },
  "no-wuxie": {
    impl: ["kernel/card_play.rs::declare_no_wuxie", "kernel/card_play.rs::step_wuxie"],
    tests: ["tests/kernel_mechanics.rs::no_wuxie_skips_wuxie_window_but_not_other_rules"],
    note: "只关无懈窗口（本次使用），不影响闪/杀响应；视图/日志仍可见事实。",
  },
  "general-pool": {
    impl: ["generals_generated.rs::GENERALS", "kernel/mod.rs::Game::init_huashen"],
    tests: ["tests/kernel_mechanics.rs::huashen_pool_only_has_generals_with_registered_skills"],
    note: "池只含「至少一个已注册技能」的 Live 武将；随机走 rand_below；性别/势力覆盖跟随展示化身。",
  },
  "hidden-declaration": {
    impl: [
      "kernel/guhuo.rs::start",
      "kernel/guhuo.rs::step_guhuo",
      "kernel/guhuo.rs::GuhuoResult",
    ],
    tests: [
      "tests/kernel_mechanics.rs::hidden_declaration_reveals_only_after_doubts_and_settles_truth",
    ],
    note: "扣置期不公开；同时收集质疑；真假只读物理定义；缠怨为独立状态（Seat.counters）。",
  },
  "aoe-target-trim": {
    impl: ["kernel/card_play.rs::declare_trim_targets", "kernel/card_play.rs::target_bounds"],
    tests: ["tests/kernel_mechanics.rs::aoe_trim_asks_fewer_targets_before_resolution"],
    note: "群体锦囊少选 N 个目标：发生在目标确定阶段（无懈/响应之前）。",
  },
  "sha-intercept": {
    impl: [
      "kernel/mod.rs::ShaIntercept",
      "kernel/card_play.rs::declare_intercept",
      "kernel/skills.rs::fire_sha_intercept",
    ],
    tests: [
      "tests/kernel_mechanics.rs::sha_intercept_can_make_slash_immune_or_unresponsive",
    ],
    note: "成为【杀】目标后、响应窗口之前：Continue/Immune/Unresponsive/Replace。",
  },
  "judge-reversal": {
    impl: ["kernel/mod.rs::Game::reverse_judge", "kernel/turn.rs::judge_phase"],
    tests: ["tests/kernel_mechanics.rs::judge_reversal_flips_indulgence_branch_only"],
    note: "按角色绑定的判定效果反转：只改分支不改牌面花色；可视图（judge_reverse）。",
  },
  "range-override": {
    impl: ["kernel/mod.rs::UseOpts", "kernel/card_play.rs::legal_targets_with"],
    tests: ["tests/kernel_mechanics.rs::range_override_extends_reach_for_this_use_only"],
    note: "按牌属性覆盖本次距离口径（烈弓）；逐目标、不改全局攻击范围。",
  },
  "virtual-equip": {
    impl: ["kernel/skills.rs::virtual_equip", "kernel/gear.rs::virtual_card"],
    tests: ["tests/kernel_mechanics.rs::virtual_equip_is_read_only_when_real_slot_empty"],
    note: "视为装备（八阵）单入口：真实同类装备优先；不占槽、不参与守恒。",
  },
  "hand-limit": {
    impl: ["kernel/mod.rs::Game::hand_limit", "kernel/skills.rs::hand_limit_bonus"],
    tests: ["tests/b1.rs::yongsi_discard_discards_all_when_short"],
    note: "手牌上限单入口：体力 + 技能修正 + 覆盖（不屈）+ 房规上限。",
  },
  "sha-limit": {
    impl: ["kernel/card_play.rs::sha_limit_reached", "kernel/skills.rs::sha_unlimited"],
    tests: ["tests/equip_effects.rs::zhugeliannu_lifts_slash_limit"],
    note: "【杀】次数单入口：连弩/技能无限 + 技能加成。",
  },
  "limit-skill": {
    impl: [
      "kernel/skills.rs::limit_gate",
      "kernel/skills.rs::mark_limited_used",
      "kernel/skills.rs::active_actions",
    ],
    tests: ["tests/kernel_mechanics.rs::limited_skill_is_usable_once_per_game"],
    note: "限定技每局一次：已使用 → legal/选项过滤；状态存座位计数器（可回放）。",
  },
  "equip-move": {
    impl: ["kernel/gear.rs::move_equip"],
    tests: ["tests/kernel_mechanics.rs::equip_move_requires_empty_target_slot_and_emits_lost"],
    note: "场上装备移槽（设变）：非弃置非获得；目标槽必须为空；发 CardLost（失去装备钩子）。",
  },
  "armor-points": {
    impl: ["kernel/mod.rs::Seat", "kernel/flow.rs::damage_one", "kernel/mod.rs::Game::add_armor"],
    tests: [
      "tests/kernel_mechanics.rs::armor_absorbs_damage_before_hp_and_emits_event",
      "tests/kernel_mechanics.rs::armor_zero_does_not_absorb",
    ],
    note: "护甲先吸收伤害（伤害事件仍成立，amount = 原值）；ArmorChanged + 视图；溢出扣体力。",
  },
};

export const MECHANIC_IDS = Object.keys(MECHANICS);
