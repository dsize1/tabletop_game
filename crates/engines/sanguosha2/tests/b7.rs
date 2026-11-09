//! B7 用例：李严（督粮/腹鳞）、孙登（匡弼）、刘虞（止戈/宗祚）、岑昏（极奢/链祸）、
//! 孙资刘放（瑰藻/讥谀）、黄皓（寝情/贿生）、张让（滔乱）。

use sanguosha2::cards::{DamageKind, ResponseKind};
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
        ("c5", "顺手牵羊", Suit::Spade),
        ("c6", "过河拆桥", Suit::Club),
        ("c7", "无懈可击", Suit::Club),
        ("c8", "五谷丰登", Suit::Heart),
        ("c9", "闪", Suit::Diamond),
        ("c10", "杀", Suit::Heart),
        ("c11", "桃", Suit::Club),
        ("c12", "青釭剑", Suit::Spade),
        ("c13", "八卦阵", Suit::Heart),
        ("c14", "火杀", Suit::Heart),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit)| match *name {
            "青釭剑" => Card::equip(*id, name, *suit, 6, Slot::Weapon),
            "八卦阵" => Card::equip(*id, name, *suit, 2, Slot::Armor),
            "火杀" => Card::new(*id, name, *suit, 4),
            _ => Card::new(*id, name, *suit, 5),
        })
        .collect();
    for i in 15..=80 {
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

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
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
// 督粮 / 腹鳞
// ---------------------------------------------------------------------------

#[test]
fn duliang_gains_hidden_hand_then_lets_target_watch_top_two() {
    let mut g = game(&[
        ("liyan", &["duliang", "fulin"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c3", "c4"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:duliang").unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(0).len(), 1, "随机获得其一张手牌");
    answer(&mut g, 0, "opt1");
    assert!(log_text(&g).contains("观看牌堆顶两张"), "{}", log_text(&g));
    assert!(
        !g.ids_in(|z| *z == Zone::Draw).is_empty(),
        "非基本牌放回牌堆顶"
    );
}

#[test]
fn fulin_counts_gained_but_not_draw_phase() {
    let mut g = game(&[
        ("liyan", &["fulin"], 4),
        ("p1", &[], 4),
    ]);
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    g.draw(0, 2); // 摸牌阶段
    assert_eq!(g.hand_limit(0), 4, "摸牌阶段的摸牌不计入腹鳞");
    give(&mut g, 1, &["c3"]);
    let card = g.hand(1)[0].clone();
    g.gain(&card, 0).unwrap();
    assert_eq!(g.hand_limit(0), 5, "获得 1 张 → 手牌上限 +1");
}

// ---------------------------------------------------------------------------
// 匡弼
// ---------------------------------------------------------------------------

#[test]
fn kuangbi_moves_cards_out_of_game_then_returns_next_turn() {
    let mut g = game(&[
        ("sundeng", &["kuangbi"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c3", "c4"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:kuangbi").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:c3");
    answer(&mut g, 1, "confirm");
    assert_eq!(g.zone_of(&"c3".to_string()), Some(&Zone::Void), "移出游戏");
    // 下回合开始取回
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert!(g.hand(0).contains(&"c3".to_string()), "下回合开始获得「匡弼」牌");
    assert!(g.log.iter().any(|l| l.text.contains("匡弼")));
}

// ---------------------------------------------------------------------------
// 止戈 / 宗祚
// ---------------------------------------------------------------------------

#[test]
fn zhige_requires_hand_over_hp_and_target_with_sha_or_equip() {
    let mut g = game(&[
        ("liuyu", &["zhige"], 3),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c5", "c6"]); // 手牌 3 > 体力 3? 需严格大于
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:zhige"),
        "手牌数不大于体力值 → 不可发动"
    );
    give(&mut g, 0, &["c9"]);
    give(&mut g, 1, &["c1"]); // p1 有【杀】
    assert!(skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:zhige"));
    skills::execute_active(&mut g, 0, "skill:zhige").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt1");
    answer(&mut g, 1, "choose:c1");
    assert!(log_text(&g).contains("对刘虞使用【杀】"), "{}", log_text(&g));
}

#[test]
fn zongzuo_adds_max_hp_by_kingdoms_and_drops_on_last_death() {
    let mut g = game(&[
        ("liuyu", &["zongzuo"], 3),
        ("wei", &[], 4),
        ("wu", &[], 4),
    ]);
    g.seats[1].kingdom = "wei".to_string();
    g.seats[2].kingdom = "wu".to_string();
    // 重跑开局初始化（Game::new 已调用一次；这里直接验证数值变化能力）
    let before = g.seats[0].max_hp;
    g.seats[0].max_hp = 3;
    sanguosha2::skills::b7::init_zongzuo(&mut g);
    assert!(g.seats[0].max_hp > 3, "按势力数增加体力上限（原 {before}）");
    // 某势力最后一人死亡 → 上限 -1
    let cur = g.seats[0].max_hp;
    g.seats[2].hp = 0;
    g.events.push(sanguosha2::kernel::event::Event::simple(
        EventKind::Death,
        Some(2),
        None,
    ));
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].max_hp, cur - 1, "势力最后一人死亡 → 上限 -1");
}

// ---------------------------------------------------------------------------
// 极奢 / 链祸
// ---------------------------------------------------------------------------

#[test]
fn jishe_draws_and_reduces_hand_limit() {
    let mut g = game(&[
        ("cenhun", &["jishe"], 3),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    let base = g.hand_limit(0);
    skills::execute_active(&mut g, 0, "skill:jishe").unwrap();
    assert_eq!(g.hand(0).len(), 1, "摸 1 张");
    assert_eq!(g.hand_limit(0), base - 1, "本回合手牌上限 -1");
    // 反复发动直到上限 0
    while g.hand_limit(0) > 0 {
        skills::execute_active(&mut g, 0, "skill:jishe").unwrap();
    }
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:jishe"),
        "手牌上限为 0 时不可再发动（读实际上限）"
    );
}

#[test]
fn lianhuo_bonus_only_when_fire_and_chain_origin() {
    let mut g = game(&[
        ("cenhun", &["lianhuo"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[0].chained = true;
    // 传导起点 + 火焰 → +1
    let hp = g.seats[0].hp;
    flow::damage_typed(
        &mut g,
        Some(1),
        0,
        1,
        DamageKind::Fire,
        Some(&"c14".to_string()),
        true,
    );
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, hp - 2, "传导起点火伤 +1");
    // 非起点（被传导到）→ 不加
    let hp = g.seats[0].hp;
    flow::damage_typed(
        &mut g,
        Some(1),
        0,
        1,
        DamageKind::Fire,
        Some(&"c14".to_string()),
        false,
    );
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, hp - 1, "被传导到不加伤");
    // 普通伤害 → 不加
    let hp = g.seats[0].hp;
    flow::damage_typed(
        &mut g,
        Some(1),
        0,
        1,
        DamageKind::Normal,
        Some(&"c1".to_string()),
        true,
    );
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, hp - 1);
}

// ---------------------------------------------------------------------------
// 瑰藻 / 讥谀
// ---------------------------------------------------------------------------

#[test]
fn guizao_triggers_with_two_distinct_suit_discards() {
    let mut g = game(&[
        ("sunzi", &["guizao"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c4"]);
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    g.to_discard(&"c2".to_string(), "discard_phase").unwrap(); // 红桃
    g.to_discard(&"c4".to_string(), "discard_phase").unwrap(); // 梅花
    g.end_phase("Discard").unwrap();
    assert_eq!(pend(&g).skill(), Some("guizao"));
    answer(&mut g, 0, "opt2");
    assert!(log_text(&g).contains("瑰藻：摸"));
}

#[test]
fn guizao_not_triggered_with_single_discard() {
    let mut g = game(&[
        ("sunzi", &["guizao"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    g.to_discard(&"c2".to_string(), "discard_phase").unwrap();
    g.end_phase("Discard").unwrap();
    assert!(g.pending.is_none(), "只弃 1 张不触发");
}

#[test]
fn jiyu_target_chooses_card_and_suit_gets_frozen() {
    let mut g = game(&[
        ("sunzi", &["jiyu"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c9", "c1"]); // 红桃×2 + 黑桃【杀】
    give(&mut g, 1, &["c5"]); // 黑桃【顺手牵羊】
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiyu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:c5"); // 目标自选
    assert!(!g.can_use_card(0, &"c1".to_string()), "黑桃封锁定用冻结实现");
    assert!(g.is_phase_frozen(0, &"c1".to_string()));
    assert!(g.can_use_card(0, &"c2".to_string()), "其它花色不受影响");
    // 黑桃 → 目标失去 1 点体力 + 自己翻面
    assert_eq!(g.seats[1].hp, 3);
    assert!(g.seats[0].flipped);
    assert!(g.log.iter().any(|l| l.text.contains("失去 1 点体力")));
}

// ---------------------------------------------------------------------------
// 寝情 / 贿生
// ---------------------------------------------------------------------------

#[test]
fn qinqing_discards_then_draws_and_rewards_extra() {
    let mut g = game(&[
        ("huanghao", &["qinqing"], 4),
        ("p1", &[], 4),
        ("lord", &[], 4),
    ]);
    g.seats[2].flags.insert("lord".to_string());
    g.seats[2].kingdom = "qun".to_string();
    give(&mut g, 1, &["c2", "c3", "c9"]);
    play(&mut g, 0);
    g.end_phase("End").unwrap();
    assert_eq!(pend(&g).skill(), Some("qinqing"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "hand");
    // 结算后其手牌数 > 主公（0）→ 黄皓摸 1（无候选时直接结算）
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert!(g.hand(0).len() >= 1, "{}", log_text(&g));
}

#[test]
fn huisheng_privately_shows_cards_and_source_chooses() {
    let mut g = game(&[
        ("huanghao", &["huisheng"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    give(&mut g, 1, &["c5", "c6"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("huisheng"));
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    assert!(g.is_privately_revealed(1, &"c2".to_string()), "私密给来源");
    assert!(!g.is_revealed(&"c2".to_string()), "不进公共区");
    // 来源选「获得其中一张，防止伤害」
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "opt1");
    answer(&mut g, 1, "choose:c2");
    assert_eq!(g.seats[0].hp, 4, "伤害被防止");
    assert!(g.hand(1).contains(&"c2".to_string()));
    assert!(g.counter("turn.huisheng_banned:1") > 0, "本局不能对该来源再发动");
}

#[test]
fn huisheng_opt2_requires_enough_hand() {
    let mut g = game(&[
        ("huanghao", &["huisheng"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    give(&mut g, 1, &["c5"]); // 只有 1 张手牌
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:c2");
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "confirm");
    let opts = options(&g);
    assert!(!opts.contains(&"opt2".to_string()), "手牌不足等量 → 不可选：{opts:?}");
}

// ---------------------------------------------------------------------------
// 滔乱
// ---------------------------------------------------------------------------

#[test]
fn taoluan_uses_card_as_trick_and_each_name_once() {
    let mut g = game(&[
        ("zhangrang", &["taoluan"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.ends_with(":顺手牵羊")), "普通锦囊按属性可选：{opts:?}");
    answer(&mut g, 0, "choose:c2:顺手牵羊");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt2"); // 令张让失去体力
    assert_eq!(g.seats[0].hp, 3, "滔乱：失去 1 点体力");
    // 同一牌名整局只能用一次
    give(&mut g, 0, &["c3"]);
    skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
    let opts = options(&g);
    assert!(
        !opts.iter().any(|o| o.ends_with(":顺手牵羊")),
        "用过的牌名不再提供：{opts:?}"
    );
}

#[test]
fn taoluan_response_context_only_offers_response_card() {
    let mut g = game(&[
        ("zhangrang", &["taoluan"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    for (kind, expect) in [
        (ResponseKind::Wuxie, ":无懈可击"),
        (ResponseKind::Shan, ":闪"),
        (ResponseKind::Sha, ":杀"),
    ] {
        g.response_context = Some(kind);
        skills::execute_active(&mut g, 0, "skill:taoluan").unwrap();
        let opts = options(&g);
        assert!(
            opts.iter().all(|o| o.ends_with(expect) || o == "pass"),
            "{kind:?} 只提供响应牌：{opts:?}"
        );
        answer(&mut g, 0, "pass");
        g.response_context = None;
    }
}

#[test]
fn taoluan_blocked_while_someone_is_dying() {
    let mut g = game(&[
        ("zhangrang", &["taoluan"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    g.seats[1].hp = 0;
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:taoluan"),
        "全场有人濒死时不可发动"
    );
}

// ---------------------------------------------------------------------------
// 机制门禁：伤害属性 / 响应牌
// ---------------------------------------------------------------------------

#[test]
fn damage_attributes_are_data_driven() {
    assert_eq!(
        sanguosha2::cards::def("火杀").damage_kind,
        DamageKind::Fire
    );
    assert_eq!(
        sanguosha2::cards::def("雷杀").damage_kind,
        DamageKind::Thunder
    );
    assert_eq!(sanguosha2::cards::def("杀").damage_kind, DamageKind::Normal);
    assert!(sanguosha2::cards::def("火杀").tags.sha, "火杀也是【杀】");
    assert!(sanguosha2::cards::responds_as("闪", ResponseKind::Shan));
    assert!(!sanguosha2::cards::responds_as("闪", ResponseKind::Sha));
    assert!(sanguosha2::cards::responds_as("无懈可击", ResponseKind::Wuxie));
}
