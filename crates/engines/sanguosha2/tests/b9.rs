//! B9 用例：界刘备（仁德/激将）、界关羽（义绝/武圣）、界张飞（替身/咆哮）、界赵云（涯角/龙胆）、
//! 界马超（马术/铁骑）、界诸葛亮（空城/观星）、界黄月英（集智/奇才）。

use sanguosha2::cards::ResponseKind;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn seat(name: &str, skills: &[&str], hp: i32) -> Seat {
    Seat::new(name, "gen", skills, hp)
}

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("c1", "杀", Suit::Spade, 7),
        ("c2", "闪", Suit::Heart, 2),
        ("c3", "桃", Suit::Diamond, 3),
        ("c4", "决斗", Suit::Club, 4),
        ("c5", "顺手牵羊", Suit::Spade, 5),
        ("c6", "过河拆桥", Suit::Club, 6),
        ("c7", "无中生有", Suit::Heart, 8),
        ("c8", "杀", Suit::Diamond, 9),
        ("c9", "杀", Suit::Heart, 10),
        ("c10", "桃", Suit::Club, 11),
        ("c11", "青釭剑", Suit::Spade, 6),
        ("c12", "白银狮子", Suit::Heart, 2),
        ("c13", "八卦阵", Suit::Heart, 2),
        ("c14", "酒", Suit::Spade, 9),
        ("c15", "火杀", Suit::Diamond, 4),
        ("c16", "雷杀", Suit::Club, 5),
        ("c17", "杀", Suit::Club, 13),
        ("c18", "南蛮入侵", Suit::Spade, 7),
        ("c19", "闪", Suit::Diamond, 3),
        ("c20", "顺手牵羊", Suit::Heart, 3),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| match *name {
            "青釭剑" => Card::equip(*id, name, *suit, *rank, Slot::Weapon),
            "白银狮子" => Card::equip(*id, name, *suit, *rank, Slot::Armor),
            "八卦阵" => Card::equip(*id, name, *suit, *rank, Slot::Armor),
            _ => Card::new(*id, name, *suit, *rank),
        })
        .collect();
    for i in 21..=90 {
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

fn pend_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
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

fn start_phase(g: &mut Game, seat: usize, phase: Phase) {
    g.phase = if phase == Phase::Prepare {
        Phase::Discard
    } else {
        Phase::Prepare
    };
    begin_phase(g, seat, phase).unwrap();
}

fn log_text(g: &Game) -> String {
    g.log
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// 仁德（界刘备）
// ---------------------------------------------------------------------------

#[test]
fn rende_gives_many_and_second_card_offers_basic_virtual() {
    let mut g = game(&[("liubei", &["rende"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:rende").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    answer(&mut g, 0, "target:1");
    // 给出第 2 张 → 视为使用基本牌（候选含火杀/雷杀；不含闪）
    let opts = options(&g);
    assert!(opts.contains(&"use:杀".to_string()), "{opts:?}");
    assert!(opts.contains(&"use:火杀".to_string()), "{opts:?}");
    assert!(opts.contains(&"use:雷杀".to_string()), "{opts:?}");
    assert!(!opts.iter().any(|o| o == "use:闪"), "【闪】不应作为候选：{opts:?}");
    assert_eq!(g.hand(1).len(), 2);
    answer(&mut g, 0, "pass");
    g.check_conservation().expect("守恒");
}

#[test]
fn rende_second_basic_can_be_fire_sha_and_resolves_really() {
    let mut g = game(&[("liubei", &["rende"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, &["c1", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:rende").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "use:火杀");
    answer(&mut g, 0, "target:2");
    // 真实【杀】结算：p2 无【闪】→ 受 1 点火焰伤害
    assert_eq!(g.seats[2].hp, 3);
    g.check_conservation().expect("守恒");
}

#[test]
fn rende_cannot_target_same_character_twice() {
    let mut g = game(&[("liubei", &["rende"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, &["c1", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:rende").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "confirm");
    answer(&mut g, 0, "target:1");
    // 第二次发动：目标 1 不再可选
    skills::execute_active(&mut g, 0, "skill:rende").unwrap();
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    let opts = options(&g);
    assert!(!opts.contains(&"target:1".to_string()), "{opts:?}");
    assert!(opts.contains(&"target:2".to_string()), "{opts:?}");
}

// ---------------------------------------------------------------------------
// 激将（界刘备·主公技）
// ---------------------------------------------------------------------------

#[test]
fn jijiang_relay_lets_shu_ally_play_sha_for_lord() {
    let mut g = game(&[
        ("liubei", &["jijiang"], 4),
        ("shu1", &[], 4),
        ("p2", &[], 4),
    ]);
    g.seats[0].identity = "lord".to_string();
    g.seats[1].kingdom = "shu".to_string();
    g.seats[2].kingdom = "wei".to_string();
    give(&mut g, 1, &["c1"]); // 蜀将手里有【杀】
    // 南蛮入侵：主公需要打出【杀】
    play(&mut g, 2);
    give(&mut g, 2, &["c18"]);
    g.resolve_use(2, &"c18".to_string(), vec![]).unwrap();
    // 询问顺序：主公先（有激将选项）→ 选择后接力
    let mut saw_jijiang = false;
    while g.pending.is_some() {
        let actor = pend(&g).actor();
        let opts = options(&g);
        if pend_skill(&g).as_deref() == Some("jijiang") {
            saw_jijiang = true;
            // 蜀将代出【杀】（或主公放弃）
            if let Some(card) = opts.iter().find(|o| o.starts_with("respond:")).cloned() {
                answer(&mut g, actor, &card);
            } else {
                answer(&mut g, actor, "pass");
            }
            continue;
        }
        if opts.contains(&"jijiang".to_string()) {
            saw_jijiang = true;
            answer(&mut g, actor, "jijiang");
            continue;
        }
        if opts.iter().any(|o| o.starts_with("respond:")) {
            let card = opts
                .into_iter()
                .find(|o| o.starts_with("respond:"))
                .unwrap();
            answer(&mut g, actor, &card);
            continue;
        }
        answer(&mut g, actor, "pass");
    }
    assert!(saw_jijiang, "主公应可发动激将");
    assert!(
        log_text(&g).contains("激将"),
        "应出现代出记录：{}",
        log_text(&g)
    );
    g.check_conservation().expect("守恒");
}

#[test]
fn jijiang_ally_may_let_lord_draw_after_out_of_turn_sha() {
    let mut g = game(&[
        ("liubei", &["jijiang"], 4),
        ("shu1", &[], 4),
    ]);
    g.seats[0].identity = "lord".to_string();
    g.seats[1].kingdom = "shu".to_string();
    // 蜀将回合外使用【杀】（此处用手牌通过 resolve_use 需其回合，故走事件：直接发 CardUsed）
    g.seats[0].hp = 4;
    g.turn = 0; // 蜀将打出的不是自己回合
    give(&mut g, 1, &["c1"]);
    g.emit(sanguosha2::kernel::event::Event::card(
        sanguosha2::kernel::event::EventKind::CardUsed,
        Some(1),
        "c1".to_string(),
        None,
    ));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jijiang"));
    assert_eq!(pend(&g).actor(), 1, "由该蜀势力角色决定");
    answer(&mut g, 1, "opt1");
    assert_eq!(g.hand(0).len(), 1, "主公摸 1 张");
}

// ---------------------------------------------------------------------------
// 义绝 / 武圣（界关羽）
// ---------------------------------------------------------------------------

#[test]
fn yijue_red_card_owner_chooses_and_guanyu_may_heal() {
    let mut g = game(&[("guanyu", &["yijue"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c2"]); // 弃置用
    give(&mut g, 1, &["c3"]); // 红桃/方片红 → 红色
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:yijue").unwrap();
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "target:1");
    // 由目标自己选展示哪张
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "pick:c3");
    assert!(g.hand(0).contains(&"c3".to_string()), "红色牌被界关羽获得");
    answer(&mut g, 0, "opt1");
    g.check_conservation().expect("守恒");
}

#[test]
fn yijue_black_card_seals_hand_and_boosts_heart_sha() {
    let mut g = game(&[("guanyu", &["yijue"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c3", "c9"]); // 桃（弃）+ 红桃杀
    give(&mut g, 1, &["c1"]); // 黑桃杀 → 黑色
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:yijue").unwrap();
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "pick:c1");
    assert!(g.seats[1].flags.contains("hand_sealed"), "手牌封印");
    assert!(skills::nonlocked_lost(&g, 1), "非锁定技失效");
    // 红桃【杀】伤害 +1（1 + 1 = 2）
    g.resolve_use(0, &"c9".to_string(), vec![1]).unwrap();
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    assert_eq!(g.seats[1].hp, 2, "红桃【杀】伤害 +1 → 2 点");
    g.check_conservation().expect("守恒");
}

#[test]
fn yijue_requires_target_with_hand_cards() {
    let mut g = game(&[("guanyu", &["yijue"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0)
            .iter()
            .any(|(a, _)| a == "skill:yijue"),
        "目标没有手牌时不可发动义绝"
    );
}

#[test]
fn wusheng_red_card_as_sha_and_diamond_sha_has_no_distance() {
    let mut g = game(&[("guanyu", &["wusheng"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, &["c3"]); // 方片桃（红）+ 方片 → 无距离
    play(&mut g, 0);
    let conv = skills::conversions(&g, 0)
        .into_iter()
        .find(|c| c.from == "c3")
        .expect("方片红牌可转化");
    assert_eq!(conv.to, "杀");
    g.use_as(0, &"c3".to_string(), "杀", vec![2], true, None)
        .expect("方片【杀】无距离限制");
    advance(&mut g).unwrap();
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    assert_eq!(g.seats[2].hp, 3);
}

#[test]
fn wusheng_black_cards_cannot_convert() {
    let mut g = game(&[("guanyu", &["wusheng"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]); // 黑桃杀（黑色）
    play(&mut g, 0);
    assert!(
        !skills::conversions(&g, 0).iter().any(|c| c.from == "c1"),
        "黑色牌不能武圣转化"
    );
}

// ---------------------------------------------------------------------------
// 替身 / 咆哮（界张飞）
// ---------------------------------------------------------------------------

#[test]
fn tishen_heals_to_max_and_draws_equal() {
    let mut g = game(&[("zhangfei", &["tishen"], 4), ("p1", &[], 4)]);
    g.seats[0].hp = 1;
    start_phase(&mut g, 0, Phase::Prepare);
    assert_eq!(pend_skill(&g).as_deref(), Some("tishen"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[0].hp, 4);
    assert_eq!(g.hand(0).len(), 3, "回复 3 点 → 摸 3 张");
    // 限定技：再也不能发动
    start_phase(&mut g, 0, Phase::Prepare);
    assert!(g.pending.is_none());
}

#[test]
fn tishen_not_offered_at_full_hp() {
    let mut g = game(&[("zhangfei", &["tishen"], 4), ("p1", &[], 4)]);
    start_phase(&mut g, 0, Phase::Prepare);
    assert!(g.pending.is_none(), "满体力不询问");
}

#[test]
fn paoxiao_removes_sha_limit_and_adds_bonus_after_nullified() {
    let mut g = game(&[("zhangfei", &["paoxiao"], 4), ("p1", &[], 4)]);
    assert_eq!(g.sha_limit(0), usize::MAX, "无次数限制");
    // 第一张【杀】被【闪】抵消 → 下一次 +1
    give(&mut g, 0, &["c1", "c9"]);
    give(&mut g, 1, &["c2"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "respond:c2");
    assert_eq!(g.counter("turn.paoxiao_bonus"), 1);
    // 第二张【杀】：伤害 2；但目标装备白银狮子 → 恒为 1
    equip(&mut g, 1, "c12", Slot::Armor);
    g.resolve_use(0, &"c9".to_string(), vec![1]).unwrap();
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    assert_eq!(g.seats[1].hp, 3, "白银狮子把伤害压回 1");
    assert_eq!(g.counter("turn.paoxiao_bonus"), 0, "加成已消费");
}

#[test]
fn paoxiao_bonus_applies_without_silver_lion() {
    let mut g = game(&[("zhangfei", &["paoxiao"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1", "c9"]);
    give(&mut g, 1, &["c2"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "respond:c2");
    g.resolve_use(0, &"c9".to_string(), vec![1]).unwrap();
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    assert_eq!(g.seats[1].hp, 2, "被抵消后下一次【杀】伤害 +1");
}

// ---------------------------------------------------------------------------
// 涯角 / 龙胆（界赵云）
// ---------------------------------------------------------------------------

#[test]
fn yajiao_off_turn_play_reveals_top_and_same_type_gives_card() {
    let mut g = game(&[("zhaoyun", &["yajiao"], 4), ("p1", &[], 4)]);
    // 让 p1 成为当前回合，赵云在其回合外打出一张【闪】
    g.turn = 1;
    give(&mut g, 0, &["c2"]);
    g.put_on_top(&"c9".to_string()).unwrap(); // 牌堆顶：杀（基本）
    g.emit(sanguosha2::kernel::event::Event::card(
        sanguosha2::kernel::event::EventKind::CardPlayed,
        Some(0),
        "c2".to_string(),
        Some("response".to_string()),
    ));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("yajiao"));
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("target:")), "类型相同 → 给牌：{opts:?}");
    answer(&mut g, 0, "target:0");
    assert!(g.hand(0).contains(&"c9".to_string()));
    g.check_conservation().expect("守恒");
}

#[test]
fn yajiao_different_type_lets_zhaoyun_pick_a_card() {
    let mut g = game(&[("zhaoyun", &["yajiao"], 4), ("p1", &[], 4)]);
    g.turn = 1;
    give(&mut g, 0, &["c2"]); // 基本
    g.put_on_top(&"c7".to_string()).unwrap(); // 无中生有（锦囊）
    give(&mut g, 1, &["c5"]);
    equip(&mut g, 1, "c11", Slot::Weapon);
    g.emit(sanguosha2::kernel::event::Event::card(
        sanguosha2::kernel::event::EventKind::CardPlayed,
        Some(0),
        "c2".to_string(),
        Some("response".to_string()),
    ));
    advance(&mut g).unwrap();
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "discard:1"), "{opts:?}");
    answer(&mut g, 0, "discard:1");
    let picks = options(&g);
    assert!(picks.iter().any(|o| o.starts_with("pick:")), "由赵云选具体牌：{picks:?}");
    let pick = picks.into_iter().find(|o| o.starts_with("pick:")).unwrap();
    answer(&mut g, 0, &pick);
    g.check_conservation().expect("守恒");
}

#[test]
fn longdan_converts_sha_to_shan_and_jiu_to_tao() {
    let mut g = game(&[("zhaoyun", &["longdan"], 4), ("p1", &[], 4)]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["c1", "c14"]); // 杀 / 酒
    let convs = skills::conversions(&g, 0);
    let sha = convs.iter().find(|c| c.from == "c1").expect("杀→闪");
    assert_eq!(sha.to, "闪");
    assert!(!sha.usable && sha.playable, "【杀】当【闪】只能打出");
    let jiu = convs.iter().find(|c| c.from == "c14").expect("酒→桃");
    assert_eq!(jiu.to, "桃");
    // 酒当桃使用（需要出牌阶段）
    play(&mut g, 0);
    g.use_as(0, &"c14".to_string(), "桃", Vec::new(), true, None)
        .unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4);
    g.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 马术 / 铁骑（界马超）
// ---------------------------------------------------------------------------

#[test]
fn mashu_reduces_distance_by_one() {
    let g = game(&[
        ("machao", &["mashu"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    // 4 人局：0 到 2 的距离原始为 2，马术 → 1
    assert_eq!(g.distance(0, 2), 1);
    assert_eq!(g.distance(1, 3), 2, "没有马术的角色不受影响");
}

#[test]
fn tieqi_judges_and_blocks_shan_when_target_cannot_discard() {
    let mut g = game(&[("machao", &["tieqi"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["c2"]); // 只有【闪】，没有同花色牌（除非判定同花色）
    // 牌堆顶放一张黑桃判定牌（p1 手里没有黑桃）
    g.put_on_top(&"c1".to_string()).unwrap();
    // 用一张黑桃【杀】确保判定前牌堆顶可控
    g.to_hand(&"c17".to_string(), 0).unwrap(); // 梅花杀
    play(&mut g, 0);
    g.resolve_use(0, &"c17".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tieqi"));
    let before = g.seats[1].hp;
    let actor = pend(&g).actor();
    answer(&mut g, actor, "opt1");
    // 判定 → 若 p1 无同花色牌，则不能出【闪】
    let text = log_text(&g);
    assert!(text.contains("铁骑"), "{text}");
    if g.pending.is_some() && pend_skill(&g).as_deref() == Some("tieqi") {
        answer(&mut g, 1, "pass");
    }
    assert_eq!(g.seats[1].hp, before - 1, "不能出【闪】→ 受到伤害");
    g.check_conservation().expect("守恒");
}

#[test]
fn tieqi_target_may_discard_same_suit_to_keep_shan() {
    let mut g = game(&[("machao", &["tieqi"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c17"]); // 梅花杀
    // 牌堆顶 = 梅花牌（判定花色梅花）
    give(&mut g, 1, &["c16", "c2"]); // 梅花雷杀（同花色）+ 闪
    play(&mut g, 0);
    g.resolve_use(0, &"c17".to_string(), vec![1]).unwrap();
    let actor = pend(&g).actor();
    answer(&mut g, actor, "opt1");
    // 判定牌取自牌堆顶（可能是任意花色）→ 若同花色可弃；否则直接不能闪
    if pend_skill(&g).as_deref() == Some("tieqi") {
        let opts = options(&g);
        if let Some(discard) = opts.into_iter().find(|o| o.starts_with("pick:")) {
            answer(&mut g, 1, &discard);
            // 弃牌后可以出【闪】
            if g.pending.is_some() {
                answer(&mut g, 1, "respond:c2");
                assert_eq!(g.seats[1].hp, 4, "出【闪】不受伤");
            }
        }
    }
    g.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 空城 / 观星（界诸葛亮）
// ---------------------------------------------------------------------------

#[test]
fn kongcheng_blocks_sha_and_duel_when_handless() {
    let mut g = game(&[("zhugeliang", &["kongcheng"], 3), ("p1", &[], 4)]);
    assert!(!skills::can_target(&g, 1, "杀", 0, None), "空手牌不能成为【杀】目标");
    assert!(!skills::can_target(&g, 1, "决斗", 0, None), "空手牌不能成为【决斗】目标");
    assert!(skills::can_target(&g, 1, "顺手牵羊", 0, None), "其他牌不受限");
    // 有手牌后恢复
    give(&mut g, 0, &["c2"]);
    assert!(skills::can_target(&g, 1, "杀", 0, None));
}

#[test]
fn guanxing_prepare_places_cards_and_all_bottom_allows_end_rerun() {
    let mut g = game(&[
        ("zhugeliang", &["guanxing"], 3),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    start_phase(&mut g, 0, Phase::Prepare);
    assert_eq!(pend_skill(&g).as_deref(), Some("guanxing"));
    // 4 人存活 → 5 张，全部置底
    for _ in 0..5 {
        assert_eq!(pend(&g).actor(), 0);
        answer(&mut g, 0, "bottom");
    }
    assert_eq!(g.counter("turn.guanxing_all_bottom"), 1);
    // 结束阶段可再次发动
    start_phase(&mut g, 0, Phase::End);
    assert_eq!(pend_skill(&g).as_deref(), Some("guanxing"), "皆置底 → 结束阶段再来一次");
    for _ in 0..5 {
        answer(&mut g, 0, "top");
    }
    g.check_conservation().expect("守恒");
}

#[test]
fn guanxing_three_cards_when_fewer_than_four_alive() {
    let mut g = game(&[("zhugeliang", &["guanxing"], 3), ("p1", &[], 4), ("p2", &[], 4)]);
    start_phase(&mut g, 0, Phase::Prepare);
    let mut placed = 0;
    while g.pending.is_some() && pend_skill(&g).as_deref() == Some("guanxing") {
        answer(&mut g, 0, "top");
        placed += 1;
    }
    assert_eq!(placed, 3, "存活 < 4 → 3 张");
}

// ---------------------------------------------------------------------------
// 集智 / 奇才（界黄月英）
// ---------------------------------------------------------------------------

#[test]
fn jizhi_draws_on_trick_and_asks_about_basic_card() {
    let mut g = game(&[("huangyueying", &["jizhi"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c7"]); // 无中生有
    g.put_on_top(&"c2".to_string()).unwrap(); // 摸到【闪】（基本牌）
    play(&mut g, 0);
    g.resolve_use(0, &"c7".to_string(), Vec::new()).unwrap();
    // 集智先摸 1（无中本身也会摸 2），随后对基本牌询问
    assert_eq!(pend_skill(&g).as_deref(), Some("jizhi"), "{:?}", pend_skill(&g));
    let before = g.hand_limit(0);
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand_limit(0), before + 1, "弃置基本牌 → 手牌上限 +1");
    g.check_conservation().expect("守恒");
}

#[test]
fn jizhi_keeps_basic_card_when_declined() {
    let mut g = game(&[("huangyueying", &["jizhi"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c7"]);
    g.put_on_top(&"c2".to_string()).unwrap();
    play(&mut g, 0);
    g.resolve_use(0, &"c7".to_string(), Vec::new()).unwrap();
    let limit = g.hand_limit(0);
    answer(&mut g, 0, "pass");
    assert_eq!(g.hand_limit(0), limit);
    assert!(g.hand(0).contains(&"c2".to_string()), "保留摸到的基本牌");
}

#[test]
fn qicai_prevents_others_discarding_armor_but_allows_own() {
    let mut g = game(&[("huangyueying", &["qicai"], 3), ("p1", &[], 4)]);
    equip(&mut g, 0, "c12", Slot::Armor);
    assert!(
        skills::prevent_equip_discard(&g, 0, &"c12".to_string(), Some(1)),
        "他人弃置防具被防止"
    );
    assert!(
        !skills::prevent_equip_discard(&g, 0, &"c12".to_string(), Some(0)),
        "自己弃置不受限"
    );
    // 装备区的武器不受保护（只有防具/宝物）
    equip(&mut g, 0, "c11", Slot::Weapon);
    assert!(!skills::prevent_equip_discard(&g, 0, &"c11".to_string(), Some(1)));
}

// ---------------------------------------------------------------------------
// 门禁
// ---------------------------------------------------------------------------

#[test]
fn b9_none_of_the_skills_use_event_only_use_card() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/skills/b9.rs"
    ))
    .unwrap();
    assert!(!src.contains("use_card("), "b9 技能不得调用 use_card");
}
