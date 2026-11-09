//! B14 用例（16 技能；马术复用 B9）。
use sanguosha2::kernel::event::{Event, EventKind};
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("h1", "杀", Suit::Heart, 12),
        ("c1", "杀", Suit::Club, 11),
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("h2", "闪", Suit::Heart, 4),
        ("j1", "决斗", Suit::Club, 4),
        ("w1", "青釭剑", Suit::Spade, 6),
        ("e1", "赤兔", Suit::Diamond, 5),
        ("t1", "桃", Suit::Heart, 9),
        ("hd", "火攻", Suit::Heart, 2),
        ("ts1", "铁索连环", Suit::Club, 10),
        ("wj1", "万箭齐发", Suit::Heart, 9),
        ("wux1", "无懈可击", Suit::Spade, 11),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("eq1", "赤兔", Suit::Club, 5, Slot::HorseOffense));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    for i in 20..=90 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
    }
    cards
}

fn game(specs: &[(&str, &[&str], i32, &str)]) -> Game {
    Game::new(
        specs
            .iter()
            .map(|(n, s, hp, kingdom)| {
                let mut seat = Seat::new(n, "gen", s, *hp);
                seat.kingdom = kingdom.to_string();
                seat
            })
            .collect(),
        deck(),
    )
}
fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids {
        g.to_hand(&id.to_string(), seat).unwrap();
    }
}
fn equip(g: &mut Game, seat: usize, id: &str, slot: Slot) {
    g.to_equip(&id.to_string(), seat, slot).unwrap();
}
fn pend_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}
fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}
fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}
fn answer_cur(g: &mut Game, action: &str) {
    let actor = g.pending.as_ref().map(|p| p.actor()).expect("没有待办");
    answer(g, actor, action);
}
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}
fn fire(g: &mut Game, kind: EventKind, seat: usize, amount: i32, cause: Option<&str>) {
    let mut ev = Event::simple(kind, Some(seat), cause);
    ev.amount = Some(amount);
    g.events.push(ev);
    advance(g).unwrap();
}

// ---------------------------------------------------------------- 制霸
#[test]
fn zhiba_lord_pindian_and_gains_cards() {
    let mut g = game(&[
        ("ce", &["zhiba"], 4, "wu"),
        ("wu1", &[], 4, "wu"),
        ("p2", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:zhiba").unwrap();
    answer(&mut g, 0, "target:1");
    // 拼点：双方暗选
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    // 挑战者（点数 2）没赢（孙策 13）→ 孙策获得两张拼点牌
    assert_eq!(pend_skill(&g), None);
    assert!(g.hand(0).iter().any(|c| c == "d1"), "获得对手的拼点牌");
}

#[test]
fn zhiba_requires_wu_target() {
    let mut g = game(&[
        ("ce", &["zhiba"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    play_phase(&mut g, 0);
    give(&mut g, 1, &["d1"]);
    assert!(skills::execute_active(&mut g, 0, "skill:zhiba").is_err());
}

// ---------------------------------------------------------------- 激昂
#[test]
fn jiang_draws_once_per_card() {
    let mut g = game(&[
        ("ce", &["jiang"], 4, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["h1"]);
    let before = g.hand(0).len();
    g.resolve_use(0, &"h1".to_string(), vec![1]).unwrap();
    // 激昂询问（使用后）
    if pend_skill(&g).as_deref() == Some("jiang") {
        answer(&mut g, 0, "opt1");
    }
    assert!(g.hand(0).len() >= before, "激昂摸牌");
}

#[test]
fn jiang_ignores_black_sha() {
    let mut g = game(&[
        ("ce", &["jiang"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["c1"]);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("jiang"));
}

// ---------------------------------------------------------------- 魂姿
#[test]
fn hunzi_awaken_at_one_hp() {
    let mut g = game(&[
        ("ce", &["hunzi"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].hp = 1;
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 3);
    assert!(g.seats[0].granted.iter().any(|s| s == "yingzi"));
    assert!(g.seats[0].granted.iter().any(|s| s == "yinghun"));
}

#[test]
fn hunzi_needs_exactly_one_hp() {
    let mut g = game(&[
        ("ce", &["hunzi"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].hp = 2;
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 4, "hp!=1 不觉醒");
}

// ---------------------------------------------------------------- 神速
#[test]
fn shensu_skip_judge_and_draw() {
    let mut g = game(&[
        ("yuan", &["shensu"], 4, "wei"),
        ("p1", &[], 4, "wei"),
    ]);
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("shensu"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.counter("turn.skip_judge"), 1);
    assert_eq!(g.counter("turn.skip_draw"), 1);
    // 视为使用【杀】→ 选目标
    assert_eq!(pend_skill(&g).as_deref(), Some("shensu"));
    answer(&mut g, 0, "target:1");
}

#[test]
fn shensu_each_item_once() {
    let mut g = game(&[
        ("yuan", &["shensu"], 4, "wei"),
        ("p1", &[], 4, "wei"),
    ]);
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pass");
    // 再进准备阶段（下一回合）才恢复
    assert_eq!(g.counter("turn.shensu_skip_judge"), 1);
}

// ---------------------------------------------------------------- 设变
#[test]
fn shebian_moves_equip_on_flip() {
    let mut g = game(&[
        ("yuan", &["shebian"], 4, "wei"),
        ("p1", &[], 4, "wei"),
    ]);
    equip(&mut g, 1, "w1", Slot::Weapon);
    g.flip(0);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("shebian"));
    answer(&mut g, 0, "pick:w1");
    answer(&mut g, 0, "target:0");
    assert!(g.equip_in(0, Slot::Weapon).is_some(), "装备被移动（move_equip）");
    assert!(g.equip_in(1, Slot::Weapon).is_none());
}

#[test]
fn shebian_no_movable_equip() {
    let mut g = game(&[
        ("yuan", &["shebian"], 4, "wei"),
        ("p1", &[], 4, "wei"),
    ]);
    g.flip(0);
    assert_ne!(pend_skill(&g).as_deref(), Some("shebian"));
}

// ---------------------------------------------------------------- 八阵 / 藏拙
#[test]
fn bazhen_virtual_armor() {
    let mut g = game(&[
        ("zg", &["bazhen"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    assert_eq!(g.armor(0), sanguosha2::cards::ArmorKind::Bagua);
    equip(&mut g, 0, "w1", Slot::Weapon);
    assert_eq!(g.armor(0), sanguosha2::cards::ArmorKind::Bagua, "非防具不影响");
}

#[test]
fn bazhen_loses_with_real_armor() {
    let mut g = game(&[
        ("zg", &["bazhen"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    equip(&mut g, 0, "arm1", Slot::Armor);
    assert_eq!(g.armor(0), sanguosha2::cards::ArmorKind::Bagua);
    // 真实防具存在 → 八阵（虚拟防具）失效
    assert!(skills::b14::virtual_armor(&g, 0) == sanguosha2::cards::ArmorKind::None);
}

#[test]
fn cangzhuo_exempts_tricks_from_hand_count() {
    let mut g = game(&[
        ("zg", &["cangzhuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["d1", "wux1", "ts1"]); // 1 基本 + 2 锦囊
    assert_eq!(skills::hand_limit_exempt(&g, 0), 2);
    g.bump("turn.trick_used");
    assert_eq!(skills::hand_limit_exempt(&g, 0), 0);
}

// ---------------------------------------------------------------- 红颜 / 火计 / 看破 / 连环
#[test]
fn hongyan_spade_is_heart() {
    let mut g = game(&[
        ("qiao", &["hongyan"], 3, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["sp1"]);
    assert_eq!(
        skills::effective_suit(&g, 0, &"sp1".to_string()),
        Some(sanguosha2::kernel::Suit::Heart)
    );
    assert_eq!(skills::hand_limit_override(&g, 0), None);
    equip(&mut g, 0, "h1", Slot::Weapon); // 红桃装备（杀作为装备需 slot，这里用真实装备）
    let _ = &g;
}

#[test]
fn huoji_converts_red_only() {
    let mut g = game(&[
        ("zg", &["huoji"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["h1", "c1"]);
    let conv = skills::conversions(&g, 0);
    assert!(conv.iter().any(|c| c.from == "h1" && c.to == "火攻"));
    assert!(!conv.iter().any(|c| c.from == "c1" && c.to == "火攻"));
    assert!(skills::fire_attack_color_mode(&g, 0));
}

#[test]
fn kanpo_converts_black_and_is_uncounterable() {
    let mut g = game(&[
        ("zg", &["kanpo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["c1", "h1"]);
    let conv = skills::conversions(&g, 0);
    assert!(conv.iter().any(|c| c.from == "c1" && c.to == "无懈可击"));
    assert!(!conv.iter().any(|c| c.from == "h1" && c.to == "无懈可击"));
    assert!(skills::wuxie_uncounterable(&g, 0));
}

#[test]
fn lianhuan_converts_plum_and_extra_target() {
    let mut g = game(&[
        ("pang", &["lianhuan"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["ts1"]);
    assert!(skills::extra_targets(&g, 0, Some(&"ts1".to_string()), "铁索连环") == 1);
    let conv = skills::conversions(&g, 0);
    assert!(conv.iter().any(|c| c.to == "铁索连环"));
}

#[test]
fn lianhuan_recasts_plum() {
    let mut g = game(&[
        ("pang", &["lianhuan"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["ts1"]);
    let before = g.hand(0).len();
    skills::execute_active(&mut g, 0, "skill:lianhuan").unwrap();
    answer(&mut g, 0, "pick:ts1");
    assert!(g.zone_of(&"ts1".to_string()) == Some(&Zone::Discard));
    assert_eq!(g.hand(0).len(), before, "弃 1 摸 1");
}

// ---------------------------------------------------------------- 涅槃
#[test]
fn niepan_rescues_and_grants_skill() {
    let mut g = game(&[
        ("pang", &["niepan"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["d1"]);
    equip(&mut g, 0, "w1", Slot::Weapon);
    g.seats[0].flipped = true;
    g.seats[0].hp = 1;
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    assert!(options(&g).iter().any(|o| o == "niepan"), "濒死窗口出现涅槃");
    answer(&mut g, 0, "niepan");
    assert_eq!(g.seats[0].hp, 3);
    assert!(!g.seats[0].flipped);
    assert!(!g.seats[0].skills.iter().any(|s| s == "niepan"), "失去涅槃");
    assert!(options(&g).iter().any(|o| o == "opt1"), "三选一");
    answer(&mut g, 0, "opt1");
    assert!(g.seats[0].granted.iter().any(|s| s == "bazhen"));
}

// ---------------------------------------------------------------- 狂骨 / 奇谋
#[test]
fn kuanggu_heals_or_draws_within_one() {
    let mut g = game(&[
        ("wei", &["kuanggu"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].hp = 2;
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("kuanggu"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[0].hp, 3, "回复 1 点体力");
}

#[test]
fn kuanggu_needs_distance_one() {
    let mut g = game(&[
        ("wei", &["kuanggu"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
        ("p3", &[], 4, "wei"),
        ("p4", &[], 4, "wei"),
    ]);
    // 5 人局：座位 0 到座位 2 距离 2 → 不触发
    flow::damage(&mut g, Some(0), 2, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("kuanggu"));
}

#[test]
fn qimou_loses_hp_draws_and_buffs() {
    let mut g = game(&[
        ("wei", &["qimou"], 4, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qimou").unwrap();
    answer(&mut g, 0, "x:2");
    assert_eq!(g.seats[0].hp, 2);
    assert_eq!(g.counter("turn.qimou"), 2);
    assert!(g.counter("turn.qimou") as usize == skills::sha_limit_bonus(&g, 0) - 0);
}

#[test]
fn qimou_limited_once() {
    let mut g = game(&[
        ("wei", &["qimou"], 4, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qimou").unwrap();
    answer(&mut g, 0, "x:1");
    assert!(skills::execute_active(&mut g, 0, "skill:qimou").is_err(), "限定技一次");
}

// ---------------------------------------------------------------- 天香
#[test]
fn tianxiang_transfers_damage_or_hp_loss() {
    let mut g = game(&[
        ("qiao", &["tianxiang"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    give(&mut g, 0, &["h2", "t1"]); // 红桃闪
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tianxiang"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:h2");
    answer(&mut g, 0, "target:2");
    answer(&mut g, 0, "mode:lose");
    assert_eq!(g.seats[2].hp, 3, "目标失去 1 点体力");
    assert!(g.hand(2).iter().any(|c| c == "h2"), "目标获得弃置的牌");
    assert_eq!(g.seats[0].hp, 3, "天香防止了原伤害");
}

#[test]
fn tianxiang_needs_heart() {
    let mut g = game(&[
        ("qiao", &["tianxiang"], 3, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["c1"]); // 黑色
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("tianxiang"));
    assert_eq!(g.seats[0].hp, 2, "正常受伤");
}

// ---------------------------------------------------------------- 飘零 / 英魂 / 武烈
#[test]
fn piaoling_judges_and_places() {
    let mut g = game(&[
        ("qiao", &["piaoling", "hongyan"], 3, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.phase = Phase::Discard;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("piaoling"));
    answer(&mut g, 0, "opt1");
    // 牌堆顶 sp1（黑桃）→ 红颜：黑桃视为红桃 → 生效
    assert_eq!(pend_skill(&g).as_deref(), Some("piaoling"));
    answer(&mut g, 0, "target:1");
    assert!(g.hand(1).iter().any(|c| c == "sp1"));
}

#[test]
fn yinghun_draws_x_then_discard_one() {
    let mut g = game(&[
        ("jian", &["yinghun"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].hp = 2;
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("yinghun"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "target:1");
    // 摸 2 弃 1：摸到 2 张后进入弃牌流程
    assert!(g.hand(1).len() >= 1);
}

#[test]
fn wulie_grants_lie_that_prevents_damage() {
    let mut g = game(&[
        ("jian", &["wulie"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.phase = Phase::Discard;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("wulie"));
    answer(&mut g, 0, "x:1");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].counters.get("lie").copied(), Some(1));
    // 有「烈」→ 抵消该次全部伤害
    let before = g.seats[1].hp;
    flow::damage(&mut g, Some(0), 1, 3);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, before, "「烈」抵消全部伤害");
    assert_eq!(g.seats[1].counters.get("lie").copied(), Some(0));
}

// ---------------------------------------------------------------- 鞬出
#[test]
fn jiandu_basic_branch_target_gains_sha_and_continues() {
    let mut g = game(&[
        ("pang", &["jiandu"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["d1", "d2"]); // 基本牌（随机弃一张，仍留一张【闪】）
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jiandu"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:hand");
    // 目标获得此【杀】，且【杀】继续结算（目标有闪 → 询问）
    assert!(g.hand(1).iter().any(|c| c == "c1"), "目标获得此【杀】");
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"), "继续结算");
}

#[test]
fn jiandu_non_basic_branch_blocks_shan_and_bumps_limit() {
    let mut g = game(&[
        ("pang", &["jiandu"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["h1"]);
    equip(&mut g, 1, "w1", Slot::Weapon); // 非基本牌
    g.resolve_use(0, &"h1".to_string(), vec![1]).unwrap();
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:w1");
    assert_eq!(g.counter("turn.jiandu_bonus"), 1);
    // 不能抵消 → 直接受伤（没有【闪】询问）
    assert_eq!(g.seats[1].hp, 3);
}
