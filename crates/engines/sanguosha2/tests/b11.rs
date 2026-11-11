//! B11 用例（14 技能）。
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("c1", "杀", Suit::Spade, 7), ("c2", "闪", Suit::Heart, 2), ("c3", "桃", Suit::Diamond, 3),
        ("c4", "决斗", Suit::Club, 4), ("c5", "过河拆桥", Suit::Spade, 5), ("c7", "无中生有", Suit::Heart, 8),
        ("c8", "杀", Suit::Diamond, 9), ("c10", "青釭剑", Suit::Spade, 6), ("c13", "黑桃杀", Suit::Spade, 13),
    ];
    let mut cards: Vec<Card> = spec.iter().map(|(id, name, suit, rank)| match *name {
        "青釭剑" => Card::equip(*id, name, *suit, *rank, Slot::Weapon),
        "黑桃杀" => Card::new(*id, "杀", Suit::Spade, *rank),
        _ => Card::new(*id, name, *suit, *rank),
    }).collect();
    for i in 20..=90 { cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2)); }
    cards
}
fn game(specs: &[(&str, &[&str], i32)]) -> Game {
    Game::new(specs.iter().map(|(n, s, hp)| Seat::new(n, "gen", s, *hp)).collect(), deck())
}
fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids { g.to_hand(&id.to_string(), seat).unwrap(); }
}
fn pend_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}
fn options(g: &Game) -> Vec<String> {
    g.pending.as_ref().map(|p| p.options().iter().map(|o| o.id.clone()).collect()).unwrap_or_default()
}
fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}
fn start_phase(g: &mut Game, seat: usize, phase: Phase) {
    g.phase = if phase == Phase::Prepare { Phase::Discard } else { Phase::Prepare };
    begin_phase(g, seat, phase).unwrap();
}

#[test]
fn jianxiong_draws_one_and_can_gain_damage_card() {
    let mut g = game(&[("caocao", &["jianxiong"], 4), ("p1", &[], 4)]);
    give(&mut g, 1, &["c1"]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let before = g.hand(0).len();
    g.resolve_use(1, &"c1".to_string(), vec![0]).unwrap();
    while g.pending.is_some() {
        let a = g.pending.as_ref().unwrap().actor();
        let gain = options(&g).iter().any(|o| o == "opt1") && pend_skill(&g).as_deref() == Some("jianxiong");
        answer(&mut g, a, if gain { "opt1" } else { "pass" });
    }
    assert!(g.hand(0).len() >= before + 1);
    g.check_conservation().expect("守恒");
}

#[test]
fn hujia_offers_relay_for_shan_only() {
    let mut g = game(&[("caocao", &["hujia"], 4), ("wei1", &[], 4)]);
    g.seats[0].identity = "lord".to_string();
    g.seats[1].kingdom = "wei".to_string();
    give(&mut g, 1, &["c2"]);
    assert!(skills::relay_options(&g, 0, sanguosha2::cards::ResponseKind::Shan).iter().any(|(i, _)| i == "hujia"));
    assert!(skills::relay_options(&g, 0, sanguosha2::cards::ResponseKind::Sha).is_empty());
}

#[test]
fn fankui_takes_one_per_damage_point() {
    let mut g = game(&[("sima", &["fankui"], 4), ("p1", &[], 4)]);
    give(&mut g, 1, &["c1", "c8"]);
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("fankui"));
    answer(&mut g, 0, "opt1");
    if pend_skill(&g).as_deref() == Some("fankui") { answer(&mut g, 0, "opt1"); }
    assert_eq!(g.hand(0).len(), 2);
    g.check_conservation().expect("守恒");
}

#[test]
fn guicai_rewrites_judge() {
    let mut g = game(&[("sima", &["guicai"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c13"]);
    sanguosha2::kernel::card_play::start_quick_judge(&mut g, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("guicai"));
    answer(&mut g, 0, "pick:c13");
    advance(&mut g).unwrap();
    assert_eq!(g.judge_result.take(), Some("c13".to_string()));
    g.check_conservation().expect("守恒");
}

#[test]
fn ganglie_black_judge_discards_source_card() {
    let mut g = game(&[("xiahou", &["ganglie"], 4), ("p1", &[], 4)]);
    give(&mut g, 1, &["c1"]);
    g.put_on_top(&"c13".to_string()).unwrap();
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("ganglie"));
    answer(&mut g, 0, "opt1");
    advance(&mut g).unwrap();
    assert!(g.hand(1).is_empty(), "黑色 → 弃置来源一张牌");
    g.check_conservation().expect("守恒");
}

#[test]
fn qingjian_counts_card_kinds() {
    let mut g = game(&[("xiahou", &["qingjian"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1", "c7"]);
    g.turn = 1;
    g.emit(sanguosha2::kernel::event::Event::card(
        sanguosha2::kernel::event::EventKind::CardGained, Some(0), "c3".to_string(), None));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qingjian"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "pick:c1");
    answer(&mut g, 0, "pick:c7");
    answer(&mut g, 0, "pass");
    assert_eq!(g.counter("turn.qingjian_limit:1"), 2);
    g.check_conservation().expect("守恒");
}

#[test]
fn tuxi_draws_x_and_takes_from_rest() {
    let mut g = game(&[("zhangliao", &["tuxi"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 1, &["c1"]);
    give(&mut g, 2, &["c8"]);
    start_phase(&mut g, 0, Phase::Draw);
    assert_eq!(pend_skill(&g).as_deref(), Some("tuxi"));
    answer(&mut g, 0, "draw:0");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "target:2");
    assert_eq!(g.hand(0).len(), 2);
    g.check_conservation().expect("守恒");
}

#[test]
fn luoyi_may_give_up_and_draw_two() {
    let mut g = game(&[("xuchu", &["luoyi"], 4), ("p1", &[], 4)]);
    start_phase(&mut g, 0, Phase::Draw);
    assert_eq!(pend_skill(&g).as_deref(), Some("luoyi"));
    answer(&mut g, 0, "opt1");
    let before = g.hand(0).len();
    answer(&mut g, 0, "pass");
    assert_eq!(g.hand(0).len(), before + 2);
    g.check_conservation().expect("守恒");
}

#[test]
fn tiandu_gains_judge_card() {
    let mut g = game(&[("guojia", &["tiandu"], 3), ("p1", &[], 4)]);
    g.put_on_top(&"c2".to_string()).unwrap();
    sanguosha2::kernel::card_play::start_quick_judge(&mut g, 0, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tiandu"));
    answer(&mut g, 0, "opt1");
    assert!(g.hand(0).contains(&"c2".to_string()));
    g.check_conservation().expect("守恒");
}

#[test]
fn yiji_gives_cards_to_others() {
    let mut g = game(&[("guojia", &["yiji"], 3), ("p1", &[], 4)]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("yiji"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "target:1");
    if pend_skill(&g).as_deref() == Some("yiji") { answer(&mut g, 0, "pass"); }
    assert_eq!(g.hand(1).len(), 1);
    g.check_conservation().expect("守恒");
}

#[test]
fn xunxun_places_top_and_bottom() {
    let mut g = game(&[("lidian", &["xunxun"], 4), ("p1", &[], 4)]);
    start_phase(&mut g, 0, Phase::Draw);
    assert_eq!(pend_skill(&g).as_deref(), Some("xunxun"));
    let mut guard = 0;
    while pend_skill(&g).as_deref() == Some("xunxun") && guard < 8 {
        guard += 1;
        answer(&mut g, 0, if guard <= 2 { "top" } else { "bottom" });
    }
    assert!(guard >= 4, "4 张都要放置（实际 {guard}）");
    g.check_conservation().expect("守恒");
}

#[test]
fn wangxi_draws_two_and_gives_one() {
    let mut g = game(&[("lidian", &["wangxi"], 4), ("p1", &[], 4)]);
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("wangxi"));
    answer(&mut g, 0, "opt1");
    let pick = options(&g).into_iter().find(|o| o.starts_with("pick:")).expect("选一张交出");
    answer(&mut g, 0, &pick);
    assert_eq!(g.hand(1).len(), 1);
    g.check_conservation().expect("守恒");
}

#[test]
fn qingguo_converts_black_cards_including_equip() {
    let mut g = game(&[("zhenji", &["qingguo"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    g.to_equip(&"c10".to_string(), 0, Slot::Weapon).unwrap();
    let convs = skills::conversions(&g, 0);
    assert!(convs.iter().any(|c| c.from == "c1" && c.to == "闪"));
    assert!(convs.iter().any(|c| c.from == "c10"), "equip zone converts");
}

#[test]
fn luoshen_gains_black_and_stops_on_red() {
    let mut g = game(&[("zhenji", &["luoshen"], 3), ("p1", &[], 4)]);
    g.put_on_top(&"c2".to_string()).unwrap();
    g.put_on_top(&"c13".to_string()).unwrap();
    start_phase(&mut g, 0, Phase::Prepare);
    assert_eq!(pend_skill(&g).as_deref(), Some("luoshen"), "luoshen should trigger");
    answer(&mut g, 0, "opt1");
    assert!(g.hand(0).contains(&"c13".to_string()), "black judge card gained");
    if pend_skill(&g).as_deref() == Some("luoshen") {
        answer(&mut g, 0, "opt1");
        assert!(!g.hand(0).contains(&"c2".to_string()), "red judge card not gained");
    }
    g.check_conservation().expect("conservation");
}
