//! B4 用例：刘封（陷嗣）、潘璋马忠（夺刀/暗箭）、朱然（胆守）、曹真（司敌）、
//! 韩浩史涣（慎断/勇略）、陈群（品第/法恩）、吴懿（奔袭）、周仓（忠勇）。
//!
//! 每技能 ≥1 正例 + ≥1 反例；并验证本批新增机制：
//! 「逆」移出游戏、「不能使用和打出」= 手牌冻结、`chained_enter` 事件、奔袭距离 -1、目标列表口径。

use sanguosha2::kernel::event::EventKind;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn seat(name: &str, skills: &[&str], hp: i32) -> Seat {
    Seat::new(name, "gen", skills, hp)
}

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
        ("c13", "八卦阵", Suit::Heart),
        ("c14", "赤兔", Suit::Diamond),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit)| match *name {
            "青釭剑" => Card::equip(*id, name, *suit, 6, Slot::Weapon),
            "八卦阵" => Card::equip(*id, name, *suit, 2, Slot::Armor),
            "赤兔" => Card::equip(*id, name, *suit, 5, Slot::HorseOffense),
            _ => Card::new(*id, name, *suit, 5),
        })
        .collect();
    for i in 15..=60 {
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

fn equip(g: &mut Game, seat: usize, id: &str, slot: Slot) {
    g.to_equip(&id.to_string(), seat, slot).unwrap();
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

/// I1 询问完整性（v1 口径 2026-10-28）：有手牌就必须显式询问。
/// 这里把「无牌可打」的三类响应询问（闪/杀、无懈、濒死）按唯一出口放弃，
/// 避免每个用例重复处理（引擎的询问本身仍会产生，见 cards.rs::ask_* 用例）。
fn drain_forced(g: &mut Game) {
    while let Some(p) = g.pending.clone() {
        if !matches!(p.kind(), "card_response" | "wuxie" | "dying") {
            break;
        }
        if p.options().iter().any(|o| o.id != p.escape_id()) {
            break;
        }
        let (actor, escape) = (p.actor(), p.escape_id().to_string());
        submit(g, actor, &escape).expect("放弃唯一出口");
    }
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    // 该询问若只剩出口（有手牌但无对应牌），drain 已按唯一出口处理 → 只允许放弃类动作。
    if g.pending.is_none() {
        assert!(
            matches!(action, "pass" | "cancel"),
            "没有待办却提交了实质动作：{action}"
        );
        return;
    }
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    // I1：作答后可能立刻产生新的「无牌可打」礼貌询问（如锦囊的无懈窗口）→ 继续排空。
    drain_forced(g);
}

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

fn log_text(g: &Game) -> String {
    g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

// ---------------------------------------------------------------------------
// 陷嗣：准备阶段取牌为「逆」（移出游戏）；其他角色移去两张视为【杀】（不限次数）
// ---------------------------------------------------------------------------

#[test]
fn xiansi_takes_card_to_void_as_ni() {
    let mut g = game(&[
        ("liufeng", &["xiansi"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c1", "c2"]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend(&g).skill(), Some("xiansi"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "confirm");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("choose:")), "应能选具体哪张牌：{opts:?}");
    let card = opts[0].trim_start_matches("choose:").to_string();
    answer(&mut g, 0, &opts[0]);
    assert_eq!(skills::ni(&g, 0), 1, "「逆」+1");
    assert_eq!(g.zone_of(&card), Some(&Zone::Void), "「逆」= 移出游戏");
    assert_eq!(g.ids_in(|z| *z == Zone::Discard).len(), 0, "未进弃牌堆");
}

#[test]
fn xiansi_other_player_can_use_two_ni_without_phase_limit() {
    let mut g = game(&[
        ("liufeng", &["xiansi"], 4),
        ("p1", &[], 4),
    ]);
    // 直接构造 4 张「逆」（移出游戏 + 归属标记）
    for id in ["c1", "c2", "c3", "c4"] {
        g.to_void(&id.to_string()).unwrap();
        skills::tag_void(&mut g, &id.to_string(), 0, "ni");
    }
    play(&mut g, 1);
    assert!(
        skills::legal_actions(&g, 1).iter().any(|(id, _)| id == "skill:xiansi"),
        "「逆」≥2 时其他角色可用"
    );
    skills::execute_active(&mut g, 1, "skill:xiansi").unwrap();
    assert_eq!(skills::ni(&g, 0), 2, "移去两张「逆」");
    assert_eq!(g.ids_in(|z| *z == Zone::Void).len(), 2, "还剩 2 张「逆」");
    assert!(log_text(&g).contains("视为对"), "视为使用【杀】：{}", log_text(&g));
    // M2：真实结算（刘封无闪 → 受到 1 点伤害）。
    assert_eq!(g.seats[0].hp, 3, "视为使用的【杀】真实造成伤害");
    // 不限次数：剩余 2 张仍可再用
    assert!(skills::legal_actions(&g, 1).iter().any(|(id, _)| id == "skill:xiansi"));
}

#[test]
fn xiansi_not_offered_without_targets() {
    let mut g = game(&[
        ("liufeng", &["xiansi"], 4),
        ("p1", &[], 4),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert!(g.pending.is_none(), "没有任何可取的牌 → 不询问");
}

// ---------------------------------------------------------------------------
// 夺刀：读伤害事件（转化杀也触发）；成本含装备区；武器没了不发动
// ---------------------------------------------------------------------------

#[test]
fn duodao_gains_weapon_with_equipment_cost() {
    let mut g = game(&[
        ("panzhang", &["duodao"], 4),
        ("src", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    equip(&mut g, 0, "c13", Slot::Armor); // 成本用装备区牌
    flow::damage_with(&mut g, Some(1), 0, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("duodao"));
    answer(&mut g, 0, "choose:duodao");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c13".to_string()), "成本含装备区：{opts:?}");
    answer(&mut g, 0, "choose:c13");
    assert!(g.hand(0).contains(&"c12".to_string()), "获得武器");
    assert!(log_text(&g).contains("夺刀"), "写日志");
}

#[test]
fn duodao_triggers_on_virtual_sha_damage_not_on_hp_loss() {
    let mut g = game(&[
        ("panzhang", &["duodao"], 4),
        ("src", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    give(&mut g, 0, &["c2"]);
    // 虚拟【杀】：承载牌设生效定义
    g.set_virtual(&"c1".to_string(), "杀", vec!["c1".to_string()]);
    flow::damage_with(&mut g, Some(1), 0, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("duodao"), "转化/虚拟【杀】也触发");
    answer(&mut g, 0, "pass");

    // 失去体力不触发
    let mut g = game(&[
        ("panzhang", &["duodao"], 4),
        ("src", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    give(&mut g, 0, &["c2"]);
    flow::lose_hp(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "失去体力不是伤害");
}

#[test]
fn duodao_dropped_when_weapon_gone_before_activation() {
    let mut g = game(&[
        ("panzhang", &["duodao"], 4),
        ("src", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    give(&mut g, 0, &["c2"]);
    flow::damage_with(&mut g, Some(1), 0, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:duodao");
    // 武器在询问期间被移走 → 真正弃牌前再校验一次，取消且不付成本
    g.to_discard(&"c12".to_string(), "test").unwrap();
    let cost = options(&g)[0].clone();
    answer(&mut g, 0, &cost);
    assert!(g.pending.is_none(), "武器没了 → 取消");
    assert!(g.hand(0).contains(&"c2".to_string()), "成本未支付");
    assert!(log_text(&g).contains("来源已无武器"), "{}", log_text(&g));
}

// ---------------------------------------------------------------------------
// 暗箭：不在其攻击范围内时【杀】伤害 +1
// ---------------------------------------------------------------------------

#[test]
fn anjian_adds_one_damage_when_out_of_attack_range() {
    let mut g = game(&[
        ("panzhang", &["anjian"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    // 4 人局：0 与 2 距离 2 → p2 的攻击范围 1 不含 p0 → +1
    let hp = g.seats[2].hp;
    flow::damage_with(&mut g, Some(0), 2, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(g.seats[2].hp, hp - 2, "暗箭 +1");
}

#[test]
fn anjian_not_applied_when_in_range_or_not_sha() {
    let mut g = game(&[
        ("panzhang", &["anjian"], 4),
        ("p1", &[], 4),
    ]);
    let hp = g.seats[1].hp;
    // 2 人局距离 1，在攻击范围内 → 不加
    flow::damage_with(&mut g, Some(0), 1, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, hp - 1, "在攻击范围内不加伤");
    // 非【杀】不加
    flow::damage_with(&mut g, Some(0), 1, 1, Some(&"c4".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, hp - 2, "非【杀】不加伤");
}

// ---------------------------------------------------------------------------
// 胆守：第 X 次发动弃 X 张牌（自动支付，含装备区）；没有目标不能发动
// ---------------------------------------------------------------------------

#[test]
fn danshou_x_costs_and_effects() {
    let mut g = game(&[
        ("zhuran", &["danshou"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    give(&mut g, 1, &["c1", "c4"]);
    play(&mut g, 0);
    // X=1：弃 1 张，弃置 p1 一张牌
    skills::execute_active(&mut g, 0, "skill:danshou").unwrap();
    assert_eq!(g.discard_pile().len(), 1, "自动支付 1 张（无需选牌交互）");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.counter("turn.danshou_used"), 1);
    // X=2：弃 2 张（手牌 1 + 装备区 1）——手牌只留 1 张，逼出装备区支付
    if let Some(rest) = g.hand(0).first().cloned() {
        g.to_discard(&rest, "test").unwrap();
    }
    equip(&mut g, 0, "c12", Slot::Weapon);
    give(&mut g, 0, &["c9"]);
    assert!(skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:danshou"));
    skills::execute_active(&mut g, 0, "skill:danshou").unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.counter("turn.danshou_used"), 2, "本阶段第二次发动 → X=2");
    assert!(!g.hand(0).contains(&"c9".to_string()), "手牌成本已弃");
    assert_eq!(g.zone_of(&"c12".to_string()), Some(&Zone::Discard), "装备区牌也可作为成本");
}

#[test]
fn danshou_needs_target_in_range() {
    let mut g = game(&[
        ("zhuran", &["danshou"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    // p1 与 p2 都在范围内；把 p1/p2 都排除 → 用 4 人局可让「范围内的其他人」为空
    let mut g4 = game(&[
        ("zhuran", &["danshou"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    give(&mut g4, 0, &["c2"]);
    play(&mut g4, 0);
    // 4 人局攻击范围 1：p1、p3 在范围内 → 仍可发动
    assert!(skills::legal_actions(&g4, 0).iter().any(|(id, _)| id == "skill:danshou"));
    // 把范围内的人都移走（只剩距离 2 的 p2）
    for id in g4.hand(1).clone() {
        g4.to_discard(&id, "test").unwrap();
    }
    g4.seats[1].hp = 0;
    for id in g4.hand(3).clone() {
        g4.to_discard(&id, "test").unwrap();
    }
    g4.seats[3].hp = 0;
    assert!(
        !skills::legal_actions(&g4, 0).iter().any(|(id, _)| id == "skill:danshou"),
        "没有攻击范围内的目标 → 不能发动"
    );
    let _ = (&mut g, "");
}

// ---------------------------------------------------------------------------
// 司敌：弃同色非基本牌 → 冻结该色手牌（到期 = 阶段结束）；未用【杀】则视为【杀】
// ---------------------------------------------------------------------------

#[test]
fn sidi_freezes_hand_color_for_phase() {
    let mut g = game(&[
        ("caozhen", &["sidi"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 0, "c13", Slot::Armor); // 红装备
    give(&mut g, 0, &["c4"]); // 黑色锦囊（不可用：与红装备不同色）
    give(&mut g, 1, &["c2", "c1"]); // 红【闪】+ 黑【杀】
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(pend(&g).skill(), Some("sidi"), "装备区牌本身即满足同色条件");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c13".to_string()), "装备区牌可选：{opts:?}");
    answer(&mut g, 0, "choose:c13");
    assert!(!g.can_use_card(1, &"c2".to_string()), "红色手牌被冻结（不能使用/打出）");
    assert!(g.can_use_card(1, &"c1".to_string()), "黑色手牌不受影响");
}

#[test]
fn sidi_end_of_phase_virtual_sha_when_no_sha_used() {
    let mut g = game(&[
        ("caozhen", &["sidi"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 0, "c13", Slot::Armor);
    give(&mut g, 0, &["c5"]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    answer(&mut g, 0, "choose:c13");
    // 出牌阶段结束：未使用【杀】→ 视为使用【杀】
    g.end_phase("Play").unwrap();
    assert!(log_text(&g).contains("视为对其使用【杀】"), "{}", log_text(&g));
    // M2：真实结算（p1 无手牌 → 受到 1 点伤害）。
    assert_eq!(g.seats[1].hp, 3, "司敌【杀】真实造成伤害");
}

#[test]
fn sidi_no_sha_when_player_used_sha() {
    let mut g = game(&[
        ("caozhen", &["sidi"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 0, "c13", Slot::Armor);
    give(&mut g, 0, &["c5"]);
    give(&mut g, 1, &["c1"]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    answer(&mut g, 0, "choose:c13");
    g.use_card(1, &"c1".to_string(), vec![0]);
    g.end_phase("Play").unwrap();
    assert!(log_text(&g).contains("本阶段使用过【杀】，不触发"), "{}", log_text(&g));
}

// ---------------------------------------------------------------------------
// 慎断：黑色基本牌被弃置后当【兵粮寸断】；同一张只问一次
// ---------------------------------------------------------------------------

#[test]
fn shenduan_uses_discarded_black_basic_as_bingliang() {
    let mut g = game(&[
        ("hanhao", &["shenduan"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 黑桃【杀】
    play(&mut g, 0);
    let card = "c1".to_string();
    g.to_discard(&card, "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("shenduan"));
    answer(&mut g, 0, "choose:shenduan");
    answer(&mut g, 0, "target:1");
    assert!(log_text(&g).contains("兵粮寸断"), "{}", log_text(&g));
    // M2：真实结算 → 进入目标判定区。
    assert_eq!(g.judge_zone(1), vec![card.clone()], "置于判定区");
}

#[test]
fn shenduan_not_triggered_for_red_or_trick() {
    let mut g = game(&[
        ("hanhao", &["shenduan"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c4"]); // 红色【闪】+ 黑色锦囊
    play(&mut g, 0);
    g.to_discard(&"c2".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "红色基本牌不触发");
    g.to_discard(&"c4".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "黑色锦囊不触发");
}

// ---------------------------------------------------------------------------
// 勇略：弃判定牌视为【杀】；未造成伤害才摸 1
// ---------------------------------------------------------------------------

#[test]
fn yonglue_uses_judge_card_then_draws_when_no_damage() {
    let mut g = game(&[
        ("hanhao", &["yonglue"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c2"]); // p1 有【闪】
    g.to_judge(&"c6".to_string(), 1).unwrap();
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    assert_eq!(pend(&g).skill(), Some("yonglue"));
    let before = g.hand(0).len();
    answer(&mut g, 0, "choose:c6");
    assert_eq!(g.zone_of(&"c6".to_string()), Some(&Zone::Discard), "判定牌被弃置");
    assert!(
        log_text(&g).contains("视为对其使用【杀】"),
        "{}",
        log_text(&g)
    );
    // M2：真实结算 → p1 打出【闪】抵消 → 未造成伤害 → 勇略摸 1。
    assert_eq!(pend(&g).actor(), 1, "视为使用的【杀】要求 p1 响应");
    let shan = options(&g)
        .into_iter()
        .find(|o| o.starts_with("respond:"))
        .expect("p1 有闪");
    answer(&mut g, 1, &shan);
    assert_eq!(g.seats[1].hp, 4, "闪抵消，未造成伤害");
    assert_eq!(g.hand(0).len(), before + 1, "未造成伤害 → 勇略摸 1");
}

#[test]
fn yonglue_no_draw_when_damage_happened() {
    let mut g = game(&[
        ("hanhao", &["yonglue"], 4),
        ("p1", &[], 4),
    ]);
    g.to_judge(&"c6".to_string(), 1).unwrap();
    begin_phase(&mut g, 1, Phase::Judge).unwrap();
    let before = g.hand(0).len();
    answer(&mut g, 0, "choose:c6");
    // M2：真实结算 → p1 无闪 → 受到 1 点伤害 → 不摸牌。
    assert!(g.pending.is_none(), "p1 无闪，直接结算");
    assert_eq!(g.seats[1].hp, 3, "造成伤害");
    assert_eq!(g.hand(0).len(), before, "造成伤害则不摸牌");
}

#[test]
fn yonglue_requires_target_in_attack_range_and_judge_zone() {
    let mut g = game(&[
        ("hanhao", &["yonglue"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    // 4 人局：0 与 2 距离 2 → 不在韩浩史涣攻击范围内 → 不触发
    g.to_judge(&"c6".to_string(), 2).unwrap();
    begin_phase(&mut g, 2, Phase::Judge).unwrap();
    assert!(g.pending.is_none(), "不在攻击范围内不触发");
}

// ---------------------------------------------------------------------------
// 品第：弃未用过的类型牌 → 摸/弃 X；已受伤目标 → 陈群进入连环（chained_enter）
// ---------------------------------------------------------------------------

#[test]
fn pindi_discards_unused_type_and_target_draws() {
    let mut g = game(&[
        ("chenqun", &["pindi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c4"]); // 基本 + 锦囊
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:pindi").unwrap();
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    assert_eq!(g.counter("turn.pindi_count"), 1, "X=1");
    assert!(!g.hand(1).is_empty(), "目标摸 1 张");
    // 同类型牌不再可选（只列锦囊）
    skills::execute_active(&mut g, 0, "skill:pindi").unwrap();
    let opts = options(&g);
    let picks: Vec<&String> = opts.iter().filter(|o| o.starts_with("choose:")).collect();
    assert_eq!(picks, vec![&"choose:c4".to_string()], "基本牌类型已用过：{opts:?}");
    answer(&mut g, 0, "pass");
}

#[test]
fn pindi_chains_chenqun_when_target_wounded() {
    let mut g = game(&[
        ("chenqun", &["pindi"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[1].hp = 3; // 已受伤
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:pindi").unwrap();
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    assert!(g.seats[0].chained, "陈群进入连环状态");
    assert!(log_text(&g).contains("进入连环状态"), "{}", log_text(&g));
}

#[test]
fn pindi_target_can_only_be_used_once_per_turn() {
    let mut g = game(&[
        ("chenqun", &["pindi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c4"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:pindi").unwrap();
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    skills::execute_active(&mut g, 0, "skill:pindi").unwrap();
    answer(&mut g, 0, "choose:c4");
    let opts = options(&g);
    assert!(!opts.contains(&"target:1".to_string()), "p1 本回合已被选过：{opts:?}");
    assert!(opts.contains(&"target:2".to_string()));
}

// ---------------------------------------------------------------------------
// 法恩：任意角色进入连环/翻至正面 → 陈群可令其摸 1（可放弃）
// ---------------------------------------------------------------------------

#[test]
fn faen_asks_owner_when_other_character_chains() {
    let mut g = game(&[
        ("chenqun", &["faen"], 4),
        ("p1", &[], 4),
    ]);
    flow::chain_enter(&mut g, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("faen"));
    assert_eq!(pend(&g).actor(), 0, "由陈群（拥有者）决定");
    let before = g.hand(1).len();
    answer(&mut g, 0, "choose:faen");
    assert_eq!(g.hand(1).len(), before + 1, "受益者是进入连环的角色");
}

#[test]
fn faen_declined_or_without_skill_does_nothing() {
    let mut g = game(&[
        ("chenqun", &["faen"], 4),
        ("p1", &[], 4),
    ]);
    flow::chain_enter(&mut g, 1);
    advance(&mut g).unwrap();
    let before = g.hand(1).len();
    answer(&mut g, 0, "pass");
    assert_eq!(g.hand(1).len(), before, "放弃则不摸牌");

    // 没有法恩 → 完全不询问
    let mut g = game(&[("a", &[], 4), ("b", &[], 4)]);
    flow::chain_enter(&mut g, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none());
}

#[test]
fn faen_triggers_on_flip_to_face_up() {
    let mut g = game(&[
        ("chenqun", &["faen"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[1].flipped = true;
    g.flip(1); // 翻至正面
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("faen"));
    assert_eq!(pend(&g).actor(), 0);
}

// ---------------------------------------------------------------------------
// 奔袭：距离固定 -1（可与进攻马叠加）；单目标【杀】/普通锦囊的至多两项强化
// ---------------------------------------------------------------------------

#[test]
fn benxi_distance_is_minus_one_not_per_card() {
    let mut g = game(&[
        ("wuyi", &["benxi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    play(&mut g, 0);
    let base = g.distance(0, 2); // 4 人局距离 2 → 奔袭 -1 → 1
    assert_eq!(base, 1, "回合内距离 -1");
    // 再用两张牌也不继续减
    give(&mut g, 0, &["c2", "c3"]);
    g.use_card(0, &"c2".to_string(), vec![]);
    g.use_card(0, &"c3".to_string(), vec![]);
    assert_eq!(g.distance(0, 2), 1, "不是每张牌累加");
    // 装备进攻马可叠加（距离下限 1）
    equip(&mut g, 0, "c14", Slot::HorseOffense);
    assert_eq!(g.distance(0, 2), 1, "距离下限为 1");
}

#[test]
fn benxi_offers_options_for_single_target_sha() {
    let mut g = game(&[
        ("wuyi", &["benxi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    play(&mut g, 0);
    give(&mut g, 0, &["c1"]);
    g.use_card(0, &"c1".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("benxi"));
    let opts = options(&g);
    assert!(opts.contains(&"opt_multi".to_string()), "有额外合法目标时可加目标：{opts:?}");
    answer(&mut g, 0, "opt_draw");
    answer(&mut g, 0, "confirm");
    assert_eq!(g.counter("turn.benxi_draw"), 1, "{}", log_text(&g));
    let before = g.hand(0).len();
    skills::on_sha_finished(&mut g, 0, Some(1), Some(&"c1".to_string()), None, true);
    assert_eq!(g.hand(0).len(), before + 1, "造成伤害后摸 1");
}

#[test]
fn benxi_skipped_for_multi_target_or_other_player() {
    let mut g = game(&[
        ("wuyi", &["benxi"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    give(&mut g, 0, &["c1"]);
    // 双目标 → 目标列表长度 2 → 不触发
    g.use_card(0, &"c1".to_string(), vec![1, 0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非单一目标不触发");
    // 别人的回合 → 不触发
    let mut g = game(&[
        ("wuyi", &["benxi"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 1);
    give(&mut g, 1, &["c1"]);
    g.use_card(1, &"c1".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "只有吴懿自己的回合才触发");
}

// ---------------------------------------------------------------------------
// 忠勇：使用【杀】后交牌；红色则接收者可使用【杀】（询问接收者）
// ---------------------------------------------------------------------------

#[test]
fn zhongyong_gives_sha_and_asks_receiver_for_red_card() {
    let mut g = game(&[
        ("zhoucang", &["zhongyong"], 4),
        ("receiver", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c10"]); // 红桃【杀】
    play(&mut g, 0);
    g.use_card(0, &"c10".to_string(), vec![1]);
    // 模拟【杀】结算完毕：该【杀】进弃牌堆
    g.to_discard(&"c10".to_string(), "sha").unwrap();
    skills::on_sha_finished(&mut g, 0, Some(1), Some(&"c10".to_string()), None, true);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zhongyong"));
    answer(&mut g, 0, "give:sha");
    answer(&mut g, 0, "target:1");
    assert!(g.hand(1).contains(&"c10".to_string()), "走「获得」语义");
    assert!(log_text(&g).contains("交给"), "写日志");
    // 红色 → 由**接收者**决定是否使用【杀】
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "choose:zhongyong");
    let opts = options(&g);
    assert!(!opts.contains(&"target:1".to_string()), "不能对接收者自己使用：{opts:?}");
    answer(&mut g, 1, "target:2");
    assert!(log_text(&g).contains("视为对"), "接收者视为使用【杀】：{}", log_text(&g));
    // M2：真实结算（p2 无闪 → 受到 1 点伤害）。
    assert_eq!(g.seats[2].hp, 3, "接收者的【杀】真实造成伤害");
}

#[test]
fn zhongyong_not_offered_when_card_gone() {
    let mut g = game(&[
        ("zhoucang", &["zhongyong"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.use_card(0, &"c1".to_string(), vec![1]);
    // 牌已被别的技能拿走（不在弃牌堆）
    skills::on_sha_finished(&mut g, 0, Some(1), Some(&"c1".to_string()), None, true);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "牌不在弃牌堆 → 不询问");
}

// ---------------------------------------------------------------------------
// 守恒 / 无帧遗留
// ---------------------------------------------------------------------------

#[test]
fn b4_flows_keep_conservation_and_no_orphan_frames() {
    let mut g = game(&[
        ("liufeng", &["xiansi"], 4),
        ("chenqun", &["pindi", "faen"], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 1, &["c2", "c4"]);
    give(&mut g, 2, &["c1", "c3"]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    if pend(&g).skill() == Some("xiansi") {
        answer(&mut g, 0, "target:1");
        answer(&mut g, 0, "confirm");
        let card = options(&g)[0].clone();
        answer(&mut g, 0, &card);
    }
    play(&mut g, 1);
    if skills::legal_actions(&g, 1).iter().any(|(id, _)| id == "skill:pindi") {
        skills::execute_active(&mut g, 1, "skill:pindi").unwrap();
        let card = options(&g)[0].clone();
        answer(&mut g, 1, &card);
        answer(&mut g, 1, "target:0");
        answer(&mut g, 1, "opt1");
        if g.pending.is_some() {
            answer(&mut g, 1, "pass");
        }
    }
    g.check_conservation().expect("牌区守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}
