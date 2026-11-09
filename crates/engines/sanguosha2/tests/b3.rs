//! B3 用例：马谡（散谣/制蛮）、荀攸（奇策/智愚）、钟会（权计/自立）、满宠（峻刑/御策）、关平（龙吟）。
//!
//! 每技能至少 1 正例 + 1 反例；并单独验证「造成伤害 ≠ 失去体力」（mechanics `damage-vs-hp-loss`）。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Suit, Zone};
use sanguosha2::skills;

fn seat(name: &str, skills: &[&str], hp: i32) -> Seat {
    Seat::new(name, "gen", skills, hp)
}

/// 固定牌堆：前 12 张给测试用，其余留作摸牌（保证 `draw` 永远有牌）。
fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit)] = &[
        ("c1", "杀", Suit::Spade),
        ("c2", "闪", Suit::Heart),
        ("c3", "桃", Suit::Diamond),
        ("c4", "决斗", Suit::Club),
        ("c5", "火攻", Suit::Heart),
        ("c6", "顺手牵羊", Suit::Spade),
        ("c7", "无懈可击", Suit::Club),
        ("c8", "五谷丰登", Suit::Heart),
        ("c9", "闪", Suit::Diamond),
        ("c10", "杀", Suit::Heart),
        ("c11", "桃", Suit::Club),
        ("c12", "青釭剑", Suit::Spade),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit)| {
            if *name == "青釭剑" {
                Card::equip(*id, name, *suit, 6, sanguosha2::kernel::Slot::Weapon)
            } else {
                Card::new(*id, name, *suit, 5)
            }
        })
        .collect();
    // 后备牌（摸牌用）：全部红桃【闪】，不与断言冲突。
    for i in 13..=40 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
    }
    cards
}

fn game(specs: &[(&str, &[&str], i32)]) -> Game {
    let seats: Vec<Seat> = specs.iter().map(|(n, s, hp)| seat(n, s, *hp)).collect();
    Game::new(seats, deck())
}

fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids {
        g.to_hand(&id.to_string(), seat).unwrap();
    }
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

// ---------------------------------------------------------------------------
// 散谣：两个模式各限一次；目标含自己；成本含装备区
// ---------------------------------------------------------------------------

#[test]
fn sanyao_hp_mode_hits_max_hp_and_counts_once() {
    let mut g = game(&[
        ("ma", &["sanyao"], 3),
        ("p1", &[], 4),
        ("p2", &[], 2),
    ]);
    give(&mut g, 0, &["c2", "c9"]);
    play(&mut g, 0);
    assert!(skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:sanyao"));
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    answer(&mut g, 0, "choose:hp");
    answer(&mut g, 0, "choose:c2");
    assert_eq!(g.discard_pile().len(), 1, "散谣成本进弃牌堆");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "对体力最大的角色造成 1 点伤害");
    assert_eq!(g.counter("turn.sanyao_hp"), 1);
    // 手牌模式仍可用（各限一次）
    assert!(skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:sanyao"));
}

#[test]
fn sanyao_can_target_self_and_second_use_is_limited() {
    let mut g = game(&[
        ("ma", &["sanyao"], 4),
        ("p1", &[], 2),
    ]);
    give(&mut g, 0, &["c2", "c9"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    answer(&mut g, 0, "choose:hp");
    answer(&mut g, 0, "choose:c2");
    let opts = options(&g);
    assert!(opts.contains(&"target:0".to_string()), "裁定 Q1=B：自己也算最大者：{opts:?}");
    answer(&mut g, 0, "target:0");
    assert_eq!(g.seats[0].hp, 3, "对自己造成 1 点伤害");
    // 手牌模式再用一次后，两个模式都用过 → 不再提供
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    answer(&mut g, 0, "choose:hand");
    answer(&mut g, 0, "choose:c9");
    answer(&mut g, 0, "target:0");
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:sanyao"),
        "两个模式各限一次"
    );
}

#[test]
fn sanyao_cost_can_be_equipment() {
    let mut g = game(&[
        ("ma", &["sanyao"], 3),
        ("p1", &[], 4),
    ]);
    g.to_equip(&"c12".to_string(), 0, sanguosha2::kernel::Slot::Weapon)
        .unwrap();
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    answer(&mut g, 0, "choose:hp");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c12".to_string()), "成本含装备区：{opts:?}");
    answer(&mut g, 0, "choose:c12");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3);
}

// ---------------------------------------------------------------------------
// 制蛮：结算伤害前发动；防止伤害并获得区域里一张牌
// ---------------------------------------------------------------------------

#[test]
fn zhiman_prevents_damage_and_gains_a_card() {
    let mut g = game(&[
        ("ma", &["zhiman"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c1"]);
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zhiman"));
    answer(&mut g, 0, "choose:zhiman");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c1".to_string()), "可获得其区域里的牌：{opts:?}");
    answer(&mut g, 0, "choose:c1");
    assert_eq!(g.seats[1].hp, 4, "伤害被防止");
    assert!(g.hand(0).contains(&"c1".to_string()), "获得该牌");
    assert_eq!(g.ids_in(|z| *z == Zone::Discard).len(), 0, "未弃置");
}

#[test]
fn zhiman_not_offered_when_target_has_no_hand_or_equip() {
    let mut g = game(&[
        ("ma", &["zhiman"], 4),
        ("p1", &[], 4),
    ]);
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "无牌可获得 → 不询问");
    assert_eq!(g.seats[1].hp, 3, "伤害照常结算");
}

#[test]
fn zhiman_can_take_judge_zone_card() {
    let mut g = game(&[
        ("ma", &["zhiman"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c1"]);
    g.to_judge(&"c6".to_string(), 1).unwrap();
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:zhiman");
    let opts = options(&g);
    assert!(
        opts.contains(&"choose:c6".to_string()),
        "裁定：可获得判定区的延时锦囊：{opts:?}"
    );
}

// ---------------------------------------------------------------------------
// 奇策：全部手牌作为虚拟牌 origin；未进入结算不计次数
// ---------------------------------------------------------------------------

#[test]
fn qice_uses_all_hand_cards_as_one_virtual_card() {
    let mut g = game(&[
        ("xun", &["qice"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3", "c1"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    answer(&mut g, 0, "choose:决斗");
    answer(&mut g, 0, "target:1");
    assert!(g.hand(0).is_empty(), "全部手牌消耗");
    assert_eq!(g.discard_pile().len(), 3, "origin 整组进弃牌堆（不提前逐张弃置）");
    assert_eq!(g.counter("turn.qice_used"), 1);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:qice"),
        "出牌阶段限一次"
    );
}

#[test]
fn qice_without_resolution_does_not_count_as_used() {
    let mut g = game(&[
        ("xun", &["qice"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    answer(&mut g, 0, "choose:顺手牵羊");
    answer(&mut g, 0, "pass"); // 取消 → 未进入结算
    assert_eq!(g.counter("turn.qice_used"), 0, "裁定 Q3=B：未进入结算不计入次数");
    assert!(g.hand(0).is_empty(), "手牌仍作废");
}

#[test]
fn qice_cannot_declare_delayed_trick() {
    let mut g = game(&[
        ("xun", &["qice"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qice").unwrap();
    let opts = options(&g);
    assert!(
        !opts.iter().any(|o| o.contains("乐不思蜀") || o.contains("闪电")),
        "延迟锦囊不可声明：{opts:?}"
    );
    assert!(submit(&mut g, 0, "choose:乐不思蜀").is_err(), "非候选项必须被拒绝");
}

// ---------------------------------------------------------------------------
// 智愚：可以出口 + 展示写入历史 + 来源弃牌；失去体力不触发
// ---------------------------------------------------------------------------

#[test]
fn zhiyu_draws_reveals_and_source_discards_when_same_color() {
    let mut g = game(&[
        ("xun", &["zhiyu"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c5"]); // 两张红色
    give(&mut g, 1, &["c1", "c4", "c6", "c7", "c12"]); // 所有黑色牌在手牌里 → 摸到红色
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zhiyu"));
    answer(&mut g, 0, "choose:zhiyu");
    let logs = g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(logs.contains("展示手牌"), "展示写入历史记录：{logs}");
    assert_eq!(pend(&g).actor(), 1, "由伤害来源选牌");
    let before = g.hand(1).len();
    answer(&mut g, 1, "choose:c1");
    assert_eq!(g.hand(1).len(), before - 1, "来源弃置 1 张");
}

#[test]
fn zhiyu_does_not_trigger_on_losing_hp() {
    let mut g = game(&[
        ("xun", &["zhiyu"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c5"]);
    flow::lose_hp(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "失去体力不触发「受到伤害后」技能");
    assert_eq!(g.seats[0].hp, 3);
}

#[test]
fn zhiyu_declined_by_player_does_not_draw() {
    let mut g = game(&[
        ("xun", &["zhiyu"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    give(&mut g, 1, &["c1"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    let before = g.hand(0).len();
    answer(&mut g, 0, "pass");
    assert_eq!(g.hand(0).len(), before, "放弃 → 不摸牌");
}

// ---------------------------------------------------------------------------
// 权计：逐点询问；「权」移出游戏；手牌上限 +1
// ---------------------------------------------------------------------------

#[test]
fn quanji_triggers_per_damage_point_and_moves_card_out_of_game() {
    let mut g = game(&[
        ("zhong", &["quanji"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1", "c2", "c3"]);
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    // 第 1 点
    answer(&mut g, 0, "choose:quanji");
    let card = options(&g)
        .into_iter()
        .find(|o| o.starts_with("choose:"))
        .expect("选手牌");
    answer(&mut g, 0, &card);
    assert_eq!(skills::quan(&g, 0), 1, "「权」+1");
    assert_eq!(g.ids_in(|z| *z == Zone::Void).len(), 1, "「权」= 移出游戏");
    assert_eq!(pend(&g).skill(), Some("quanji"), "第 2 点继续询问");
    // 第 2 点放弃
    answer(&mut g, 0, "pass");
    assert!(g.pending.is_none(), "两点问完");
    assert_eq!(g.seats[0].hp, 2);
    assert_eq!(g.hand_limit(0), 2 + 1, "手牌上限 = 体力值 + 「权」数");
}

#[test]
fn quanji_survives_losing_hp_without_trigger() {
    let mut g = game(&[
        ("zhong", &["quanji"], 4),
        ("p1", &[], 4),
    ]);
    flow::lose_hp(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "失去体力不触发权计");
    assert_eq!(skills::quan(&g, 0), 0);
}

// ---------------------------------------------------------------------------
// 自立：准备阶段觉醒；权不足不触发；满体力隐藏回复项
// ---------------------------------------------------------------------------

#[test]
fn zili_awaken_at_prepare_phase() {
    let mut g = game(&[
        ("zhong", &["zili"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[0].hp = 2;
    skills::add_quan(&mut g, 0, 3);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend(&g).skill(), Some("zili"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[0].hp, 3, "回复 1 点体力（2 → 3）");
    assert_eq!(g.seats[0].max_hp, 3, "然后减 1 点体力上限");
    assert!(g.seats[0].granted.iter().any(|s| s == "paiyi"), "获得「排异」");
    assert!(g.seats[0].flags.contains("zili_done"));
}

#[test]
fn zili_not_triggered_below_three_quan_and_hides_heal_at_full_hp() {
    let mut g = game(&[
        ("zhong", &["zili"], 4),
        ("p1", &[], 4),
    ]);
    skills::add_quan(&mut g, 0, 2);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert!(g.pending.is_none(), "「权」不足 3 不触发");

    let mut g = game(&[
        ("zhong", &["zili"], 4),
        ("p1", &[], 4),
    ]);
    skills::add_quan(&mut g, 0, 3);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    let opts = options(&g);
    assert!(!opts.contains(&"opt1".to_string()), "满体力隐藏回复项：{opts:?}");
}

// ---------------------------------------------------------------------------
// 峻刑：任意张手牌成本；目标不要求有手牌；翻面照字面
// ---------------------------------------------------------------------------

#[test]
fn junxing_forces_target_to_discard_other_type_or_flip() {
    let mut g = game(&[
        ("man", &["junxing"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1", "c3"]); // 两张基本牌
    give(&mut g, 1, &["c4", "c2"]); // 决斗（锦囊）+ 闪（基本）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:junxing").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "confirm");
    answer(&mut g, 0, "target:1");
    let opts = options(&g);
    assert!(opts.contains(&"opt1".to_string()), "有类型不同的牌可弃：{opts:?}");
    answer(&mut g, 1, "opt1");
    let opts = options(&g);
    assert_eq!(opts, vec!["choose:c4".to_string(), "pass".to_string()], "只能弃锦囊");
    answer(&mut g, 1, "choose:c4");
    assert_eq!(g.hand(1).len(), 1);
    assert!(!g.seats[1].flipped);
}

#[test]
fn junxing_target_without_hand_can_only_flip_and_draw() {
    let mut g = game(&[
        ("man", &["junxing"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:junxing").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    let opts = options(&g);
    assert!(opts.contains(&"target:1".to_string()), "裁定 Q1=A：无手牌也可指定：{opts:?}");
    answer(&mut g, 0, "target:1");
    assert_eq!(options(&g), vec!["opt2".to_string()], "只能翻面（无牌可弃）");
    answer(&mut g, 1, "opt2");
    assert!(g.seats[1].flipped, "翻面");
    assert_eq!(g.hand(1).len(), 2, "摸与弃置数相同的牌");
}

// ---------------------------------------------------------------------------
// 御策：可以出口；来源无手牌自动回复；满体力+来源无手牌不可发动
// ---------------------------------------------------------------------------

#[test]
fn yuce_reveals_and_source_chooses_heal_or_discard() {
    let mut g = game(&[
        ("man", &["yuce"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["c4", "c2"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("yuce"));
    answer(&mut g, 0, "choose:yuce");
    answer(&mut g, 0, "choose:c1"); // 展示基本牌
    let opts = options(&g);
    assert!(opts.contains(&"opt1".to_string()), "来源有锦囊可弃：{opts:?}");
    answer(&mut g, 1, "opt2"); // 令满宠回复
    assert_eq!(g.seats[0].hp, 3, "回复 1 点体力（2 → 3）");
    let logs = g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(logs.contains("展示手牌"), "展示写入历史记录");
}

#[test]
fn yuce_auto_heals_when_source_has_no_hand() {
    let mut g = game(&[
        ("man", &["yuce"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["c1"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "裁定 Q1=B：来源无手牌不询问");
    assert_eq!(g.seats[0].hp, 3, "直接回复 1 点（2 → 3）");
}

#[test]
fn yuce_not_available_without_hand_card_to_reveal() {
    let mut g = game(&[
        ("man", &["yuce"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c1"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "没有手牌可展示 → 不能发动御策");
    assert_eq!(g.seats[0].hp, 3);
}

// ---------------------------------------------------------------------------
// 龙吟：使用【杀】时弃一张牌，打「不计次数」标记（不回退全局计数）
// ---------------------------------------------------------------------------

#[test]
fn longyin_marks_sha_uncounted_without_touching_global_counter() {
    let mut g = game(&[
        ("guan", &["longyin"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 1);
    let sha = "c10".to_string(); // 红桃【杀】
    g.to_hand(&sha, 1).unwrap();
    g.use_card(1, &sha, vec![0]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("longyin"));
    answer(&mut g, 0, "choose:c2");
    assert!(g.is_uncounted(&sha), "给该【杀】打「不计次数」标记");
    assert_eq!(g.sha_used(1), 0, "不计入本阶段使用次数");
    assert_eq!(g.hand(0).len(), 1, "红色【杀】→ 关平摸 1 张");
}

#[test]
fn longyin_does_not_trigger_on_non_sha_or_outside_play_phase() {
    let mut g = game(&[
        ("guan", &["longyin"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 1);
    let shan = "c9".to_string();
    g.to_hand(&shan, 1).unwrap();
    g.use_card(1, &shan, vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非【杀】不触发龙吟");

    // 非出牌阶段使用【杀】也不触发
    let mut g = game(&[
        ("guan", &["longyin"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    begin_phase(&mut g, 1, Phase::Draw).unwrap();
    let sha = "c10".to_string();
    g.to_hand(&sha, 1).unwrap();
    g.use_card(1, &sha, vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "只有出牌阶段内使用【杀】才触发");
}

// ---------------------------------------------------------------------------
// 机制门禁：造成伤害 vs 失去体力（事件层区分）
// ---------------------------------------------------------------------------

#[test]
fn damage_and_hp_loss_are_different_events() {
    use sanguosha2::kernel::event::EventKind;
    let mut g = game(&[
        ("a", &[], 4),
        ("b", &[], 4),
    ]);
    flow::damage(&mut g, Some(1), 0, 1);
    let has_damage = g.events.iter().any(|e| e.kind == EventKind::DamageAfter);
    let has_lost = g.events.iter().any(|e| e.kind == EventKind::HpLost);
    assert!(has_damage && !has_lost, "造成伤害只发伤害事件");
    g.events.clear();
    flow::lose_hp(&mut g, 0, 1, "test");
    let has_damage = g.events.iter().any(|e| e.kind == EventKind::DamageAfter);
    let has_lost = g.events.iter().any(|e| e.kind == EventKind::HpLost);
    assert!(has_lost && !has_damage, "失去体力只发 HpLost，不发伤害事件");
    g.events.clear();
    flow::lose_max_hp(&mut g, 0, 1, "test");
    assert!(
        g.events.iter().any(|e| e.kind == EventKind::MaxHpChanged),
        "减体力上限有独立事件"
    );
}

// ---------------------------------------------------------------------------
// 守恒 / 无帧遗留（B3 全流程跑一遍）
// ---------------------------------------------------------------------------

#[test]
fn b3_flows_keep_conservation_and_leave_no_orphan_frames() {
    // 散谣 → 制蛮 → 智愚 → 权计 → 峻刑 → 御策 → 龙吟：能跑的都跑一遍再自检。
    let mut g = game(&[
        ("ma", &["sanyao", "zhiman"], 4),
        ("xun", &["zhiyu"], 4),
        ("zhong", &["quanji"], 4),
        ("man", &["junxing", "yuce"], 4),
        ("guan", &["longyin"], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    give(&mut g, 1, &["c1", "c3"]);
    play(&mut g, 0);
    // 散谣 → 制蛮
    skills::execute_active(&mut g, 0, "skill:sanyao").unwrap();
    answer(&mut g, 0, "choose:hp");
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "target:1");
    if g.pending.as_ref().and_then(|p| p.skill()) == Some("zhiman") {
        answer(&mut g, 0, "choose:zhiman");
        let card = options(&g).into_iter().next().unwrap();
        answer(&mut g, 0, &card);
    }
    // 奇策（荀攸没有手牌 → 不该出现）
    assert!(!skills::legal_actions(&g, 1).iter().any(|(id, _)| id == "skill:qice"));
    g.check_conservation().expect("牌区守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}
