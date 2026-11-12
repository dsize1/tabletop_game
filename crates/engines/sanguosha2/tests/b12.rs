//! B12 用例（8 技能：急救 / 除疠 / 无双 / 利驭 / 义从 / 趫猛 / 离间 / 闭月）。
use sanguosha2::cards::{Effect, ResponseKind};
use sanguosha2::kernel::resolve::{advance, begin_phase, submit, Frame};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("s1", "杀", Suit::Spade, 7),
        ("s2", "杀", Suit::Diamond, 8),
        ("s3", "杀", Suit::Club, 9),
        ("d1", "闪", Suit::Heart, 2),
        ("d2", "闪", Suit::Heart, 3),
        ("d3", "闪", Suit::Club, 10),
        ("t1", "桃", Suit::Diamond, 3),
        ("j1", "决斗", Suit::Club, 4),
        ("sp1", "杀", Suit::Spade, 13),
        ("sp2", "闪", Suit::Spade, 11),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("e1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
    cards.push(Card::equip("e2", "的卢", Suit::Heart, 5, Slot::HorseDefense));
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    for i in 20..=90 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
    }
    cards
}

fn game(specs: &[(&str, &[&str], i32, bool)]) -> Game {
    Game::new(
        specs
            .iter()
            .map(|(n, s, hp, male)| {
                let mut seat = Seat::new(n, "gen", s, *hp);
                seat.male = *male;
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
fn pend_actor(g: &Game) -> Option<usize> {
    g.pending.as_ref().map(|p| p.actor())
}
fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
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
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}
fn end_phase(g: &mut Game, seat: usize) {
    g.phase = Phase::Discard;
    begin_phase(g, seat, Phase::End).unwrap();
}

// ---------------------------------------------------------------------------
// 急救
// ---------------------------------------------------------------------------

#[test]
fn jijiu_rescues_with_red_hand_and_equip() {
    let mut g = game(&[
        ("victim", &[], 1, true),
        ("hua", &["jijiu"], 3, true),
        ("p2", &[], 3, true),
        ("p3", &[], 3, true),
    ]);
    g.turn = 0; // 华佗的回合外
    give(&mut g, 1, &["d1"]);
    equip(&mut g, 1, "e1", Slot::HorseOffense);
    flow::damage(&mut g, Some(2), 0, 1);
    advance(&mut g).unwrap();
    // 濒死者自己无牌 → 直接轮到华佗
    assert_eq!(pend_actor(&g), Some(1));
    let opts = options(&g);
    assert!(
        opts.iter().any(|o| o == "vrespond:jijiu:d1"),
        "红色手牌可当【桃】：{opts:?}"
    );
    assert!(
        opts.iter().any(|o| o == "vrespond:jijiu:e1"),
        "红色装备区牌也可当【桃】：{opts:?}"
    );
    answer(&mut g, 1, "vrespond:jijiu:d1");
    assert_eq!(g.seats[0].hp, 1, "救起后体力 1");
    assert!(g.zone_of(&"d1".to_string()).is_some());
    g.check_conservation().unwrap();
}

#[test]
fn jijiu_is_off_outside_turn_and_red_only() {
    let mut g = game(&[
        ("victim", &[], 1, true),
        ("hua", &["jijiu"], 3, true),
        ("p2", &[], 3, true),
        ("p3", &[], 3, true),
    ]);
    give(&mut g, 1, &["d1"]);
    g.turn = 1; // 华佗自己的回合 → 不可急救
    assert!(skills::dying_conversions(&g, 1).is_empty());
    g.turn = 0; // 回合外 → 可用
    assert_eq!(skills::dying_conversions(&g, 1).len(), 1);
    // 黑色牌（黑桃【杀】）不能当【桃】
    g.to_hand(&"sp1".to_string(), 1).unwrap();
    let red_only = skills::dying_conversions(&g, 1)
        .into_iter()
        .all(|c| c.from != "sp1");
    assert!(red_only, "黑色牌不能作为急救承载牌");
}

// ---------------------------------------------------------------------------
// 除疠
// ---------------------------------------------------------------------------

#[test]
fn chuli_distinct_kingdoms_discard_and_spade_draw() {
    let mut g = game(&[
        ("hua", &["chuli"], 3, true),
        ("wei1", &[], 4, true),
        ("wu1", &[], 4, true),
        ("wei2", &[], 4, true),
    ]);
    g.seats[0].kingdom = "qun".into();
    g.seats[1].kingdom = "wei".into();
    g.seats[2].kingdom = "wu".into();
    g.seats[3].kingdom = "wei".into();
    give(&mut g, 0, &["sp1", "d1"]);
    give(&mut g, 1, &["sp2"]);
    give(&mut g, 2, &["t1"]);
    give(&mut g, 3, &["j1"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:chuli").unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("chuli"));
    let opts = options(&g);
    assert!(opts.contains(&"target:1".to_string()));
    assert!(opts.contains(&"target:2".to_string()));
    assert!(opts.contains(&"target:3".to_string()), "初始两个魏势力都可选");
    answer(&mut g, 0, "target:1");
    let opts = options(&g);
    assert!(
        !opts.contains(&"target:3".to_string()),
        "已选魏势力后同势力不可再选：{opts:?}"
    );
    answer(&mut g, 0, "confirm");
    // 参与者各自选自己的一张牌弃置
    assert_eq!(pend_actor(&g), Some(0));
    answer(&mut g, 0, "pick:sp1");
    assert_eq!(pend_actor(&g), Some(1));
    answer(&mut g, 1, "pick:sp2");
    // 华佗弃黑桃 → 摸 1；魏1 弃黑桃 → 摸 1
    assert_eq!(g.hand(0).len(), 2, "剩【闪】+ 摸 1");
    assert_eq!(g.hand(1).len(), 1, "弃光后摸 1");
    g.check_conservation().unwrap();
}

#[test]
fn chuli_requires_own_card_and_skips_judge_zone() {
    // 自己没有牌 → 不能发动
    let mut g = game(&[
        ("hua", &["chuli"], 3, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    g.seats[0].kingdom = "qun".into();
    g.seats[1].kingdom = "wei".into();
    g.seats[2].kingdom = "wu".into();
    give(&mut g, 1, &["d1"]);
    give(&mut g, 2, &["d2"]);
    play_phase(&mut g, 0);
    assert!(
        skills::execute_active(&mut g, 0, "skill:chuli").is_err(),
        "自己无牌不能发动除疠"
    );
    // 只有判定区牌的角色不可被选（判定区不参与）
    let mut g2 = game(&[
        ("hua", &["chuli"], 3, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    g2.seats[0].kingdom = "qun".into();
    g2.seats[1].kingdom = "wei".into();
    g2.seats[2].kingdom = "wu".into();
    equip(&mut g2, 0, "w1", Slot::Weapon);
    g2.to_judge(&"j1".to_string(), 1).unwrap();
    give(&mut g2, 2, &["d2"]);
    play_phase(&mut g2, 0);
    skills::execute_active(&mut g2, 0, "skill:chuli").unwrap();
    let opts = options(&g2);
    assert!(
        !opts.contains(&"target:1".to_string()),
        "判定区不算可弃牌区域：{opts:?}"
    );
    assert!(opts.contains(&"target:2".to_string()));
}

// ---------------------------------------------------------------------------
// 无双
// ---------------------------------------------------------------------------

#[test]
fn wushuang_sha_needs_two_shan() {
    let mut g = game(&[
        ("lvbu", &["wushuang"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    give(&mut g, 1, &["d1", "d2"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_actor(&g), Some(1));
    answer(&mut g, 1, "respond:d1");
    assert_eq!(pend_actor(&g), Some(1), "第一张【闪】后仍需第二张");
    answer(&mut g, 1, "respond:d2");
    assert_eq!(g.seats[1].hp, 4, "两张【闪】抵消");
    assert!(pend_skill(&g).is_none());
    g.check_conservation().unwrap();
}

#[test]
fn wushuang_single_shan_does_not_cancel() {
    let mut g = game(&[
        ("lvbu", &["wushuang"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    give(&mut g, 1, &["d1"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "respond:d1");
    assert_eq!(g.seats[1].hp, 3, "只有一张【闪】→ 未抵消");
}

#[test]
fn wushuang_duel_applies_from_both_sides() {
    // 任何人主动与吕布决斗时，其每次都要两张【杀】；吕布自己每次 1 张。
    let mut g = game(&[
        ("lvbu", &["wushuang"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["j1", "s2", "s3"]);
    give(&mut g, 0, &["s1"]);
    g.resolve_use(1, &"j1".to_string(), vec![0]).unwrap();
    drain_forced(&mut g); // I1：决斗的无懈询问先排空
    assert_eq!(pend_actor(&g), Some(0), "吕布先响应");
    answer(&mut g, 0, "respond:s1");
    assert_eq!(pend_actor(&g), Some(1), "轮到发起者");
    answer(&mut g, 1, "respond:s2");
    assert_eq!(pend_actor(&g), Some(1), "与吕布决斗者每次需两张【杀】");
    answer(&mut g, 1, "respond:s3");
    // 吕布已无【杀】→ 受到 1 点伤害（来源为决斗发起者）
    assert_eq!(g.seats[0].hp, 3);
    // 多张响应的语义入口（机制锚点）
    assert_eq!(
        skills::response_need(&g, 1, 0, ResponseKind::Sha, Effect::Duel),
        2
    );
    assert_eq!(
        skills::response_need(&g, 0, 1, ResponseKind::Sha, Effect::Duel),
        1
    );
}

// ---------------------------------------------------------------------------
// 利驭
// ---------------------------------------------------------------------------

fn liyu_setup() -> Game {
    let mut g = game(&[
        ("lvbu", &["wushuang", "liyu"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    g
}

#[test]
fn liyu_equip_card_lets_victim_designate_duel() {
    let mut g = liyu_setup();
    equip(&mut g, 1, "w1", Slot::Weapon);
    give(&mut g, 2, &["s2", "s3"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("liyu"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:w1");
    assert!(
        g.hand(0).iter().any(|x| x == "w1"),
        "装备牌进吕布手牌"
    );
    // 由受伤角色（p1）指定另一名角色
    assert_eq!(pend_actor(&g), Some(1), "决斗对象由受伤角色指定");
    assert_eq!(pend_skill(&g).as_deref(), Some("liyu"));
    answer(&mut g, 1, "target:2");
    assert_eq!(pend_actor(&g), Some(2), "视为吕布对 p2 使用【决斗】");
    g.check_conservation().unwrap();
}

#[test]
fn liyu_non_equip_makes_victim_draw() {
    let mut g = liyu_setup();
    give(&mut g, 1, &["t1"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    drain_forced(&mut g); // I1：无懈等礼貌询问先排空
    answer(&mut g, 0, "opt1");
    let before = g.hand(1).len();
    answer(&mut g, 0, "pick:hand");
    assert_eq!(g.hand(1).len(), before, "被拿走 1 张后摸 1 张");
    assert_eq!(g.hand(0).len(), 1, "吕布获得该牌（原【杀】已用掉）");
    assert!(pend_skill(&g).is_none(), "非装备牌不进入指定决斗阶段");
}

#[test]
fn liyu_ignores_judge_zone_only_victim() {
    let mut g = liyu_setup();
    g.to_judge(&"j1".to_string(), 1).unwrap();
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("liyu"), "判定区不可选 → 不发动");
}

// ---------------------------------------------------------------------------
// 义从
// ---------------------------------------------------------------------------

#[test]
fn yicong_modifies_distance_both_ways() {
    let mut g = game(&[
        ("gz", &["yicong"], 3, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
        ("p3", &[], 4, true),
    ]);
    assert_eq!(g.distance(0, 2), 1, "义从：我 → 他人 -1");
    assert_eq!(g.distance(2, 0), 2, "体力 3 → 他人算我无加成");
    g.seats[0].hp = 2;
    assert_eq!(g.distance(2, 0), 3, "体力 ≤2 → 他人算我 +1");
}

#[test]
fn yicong_distance_never_below_one() {
    let mut g = game(&[
        ("gz", &["yicong"], 3, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    assert_eq!(g.distance(0, 1), 1, "相邻距离 -1 后仍为 1");
}

// ---------------------------------------------------------------------------
// 趫猛
// ---------------------------------------------------------------------------

#[test]
fn qiaomeng_gains_mount_card() {
    let mut g = game(&[
        ("gz", &["qiaomeng"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    equip(&mut g, 1, "e1", Slot::HorseOffense);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiaomeng"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:e1");
    assert!(g.equip_in(1, Slot::HorseOffense).is_none());
    assert!(g.hand(0).iter().any(|c| c == "e1"), "坐骑牌改为获得");
    g.check_conservation().unwrap();
}

#[test]
fn qiaomeng_discards_non_mount_card() {
    let mut g = game(&[
        ("gz", &["qiaomeng"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    give(&mut g, 1, &["w1"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    drain_forced(&mut g); // I1：无懈等礼貌询问先排空
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:hand");
    assert!(!g.hand(0).iter().any(|c| c == "w1"), "非坐骑 → 弃置");
    assert_eq!(g.zone_of(&"w1".to_string()).map(|z| z.clone()), Some(sanguosha2::kernel::Zone::Discard));
    // 判定区不参与
    let mut g2 = game(&[
        ("gz", &["qiaomeng"], 4, true),
        ("p1", &[], 4, true),
        ("p2", &[], 4, true),
    ]);
    play_phase(&mut g2, 0);
    give(&mut g2, 0, &["s2"]);
    g2.to_judge(&"j1".to_string(), 1).unwrap();
    g2.resolve_use(0, &"s2".to_string(), vec![1]).unwrap();
    assert_ne!(pend_skill(&g2).as_deref(), Some("qiaomeng"));
}

// ---------------------------------------------------------------------------
// 离间
// ---------------------------------------------------------------------------

#[test]
fn lijian_discards_equip_and_duel_cannot_be_wuxied() {
    let mut g = game(&[
        ("dc", &["lijian"], 3, false),
        ("m1", &[], 4, true),
        ("m2", &[], 4, true),
        ("f1", &[], 4, false),
    ]);
    equip(&mut g, 0, "w1", Slot::Weapon);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 2, &["s3"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:lijian").unwrap();
    assert!(options(&g).iter().any(|o| o == "pick:w1"), "界离间可弃装备区");
    answer(&mut g, 0, "pick:w1");
    let opts = options(&g);
    assert!(opts.contains(&"target:1".to_string()));
    assert!(opts.contains(&"target:2".to_string()));
    assert!(!opts.contains(&"target:3".to_string()), "女性不可作发起者");
    answer(&mut g, 0, "target:1");
    let opts = options(&g);
    assert!(!opts.contains(&"target:3".to_string()), "女性不可作目标");
    answer(&mut g, 0, "target:2");
    assert_eq!(pend_actor(&g), Some(2), "目标先对发起者出【杀】");
    // 不能被【无懈可击】抵消：使用帧带 `no_wuxie`，且未出现无懈询问
    let no_wuxie = g.stack.iter().any(|f| match f {
        Frame::Flow(ff) => ff.data["no_wuxie"].as_bool() == Some(true),
        _ => false,
    });
    assert!(no_wuxie, "离间的【决斗】必须带 no_wuxie 标记");
    g.check_conservation().unwrap();
}

#[test]
fn lijian_needs_two_male_targets() {
    let mut g = game(&[
        ("dc", &["lijian"], 3, false),
        ("m1", &[], 4, true),
        ("f1", &[], 4, false),
    ]);
    give(&mut g, 0, &["d1"]);
    play_phase(&mut g, 0);
    let legal = skills::legal_actions(&g, 0);
    assert!(
        !legal.iter().any(|(a, _)| a == "skill:lijian"),
        "不足两名男性角色时不可发动"
    );
}

// ---------------------------------------------------------------------------
// 闭月
// ---------------------------------------------------------------------------

#[test]
fn biyue_draws_two_without_hand() {
    let mut g = game(&[("dc", &["biyue"], 3, false), ("p1", &[], 4, true)]);
    g.turn = 0;
    end_phase(&mut g, 0);
    assert_eq!(pend_skill(&g).as_deref(), Some("biyue"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), 2, "无手牌摸两张");
    g.check_conservation().unwrap();
}

#[test]
fn biyue_draws_one_with_hand_and_can_decline() {
    let mut g = game(&[("dc", &["biyue"], 3, false), ("p1", &[], 4, true)]);
    give(&mut g, 0, &["d1"]);
    g.turn = 0;
    end_phase(&mut g, 0);
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), 2, "有手牌摸一张");
    // 放弃发动 → 不摸
    let mut g2 = game(&[("dc", &["biyue"], 3, false), ("p1", &[], 4, true)]);
    g2.turn = 0;
    end_phase(&mut g2, 0);
    answer(&mut g2, 0, "pass");
    assert_eq!(g2.hand(0).len(), 0);
}
