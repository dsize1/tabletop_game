//! B20 技能门禁（J8：4 将 / 8 技能，验证批次收官）。

use serde_json::json;
use sanguosha3::kernel::resolve::{self, Stage};
use sanguosha3::kernel::{skills, Card, Game, Phase, Seat, Suit, Zone};

fn seat_with(id: usize, list: &[&str]) -> Seat {
    Seat { id, hp: 4, max_hp: 4, alive: true, male: true,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: "qun".to_string(), ..Default::default() }
}
fn test_game(n: usize) -> Game {
    let seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[])).collect();
    Game { seats, ..Default::default() }
}
fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs { m.insert((*k).to_string(), v.clone()); }
    serde_json::Value::Object(m)
}
fn trigger(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) {
    resolve::push_ask(g, skill, Stage::Choose, actor, d);
    resolve::advance(g).expect("advance");
}
fn submit(g: &mut Game, ans: &str) {
    let p = g.pending.clone().expect("期望有待办");
    let actor = p.actor();
    assert!(p.has_option(ans), "选项 {ans} 不在 {:?}", p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>());
    resolve::submit(g, actor, ans).expect("submit");
}
fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(id.to_string(), Card { id: id.to_string(), def: def.to_string(), suit, rank: 7 });
}

#[test]
fn qiaoshi_draws_both_and_disables_on_unequal_hands() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiaoshi".into()];
    g.turn = 1;
    g.phase = Phase::End;
    for i in 0..2 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "qiaoshi", 0, data(&[("event", "PhaseStart".into()), ("stage", "ask".into())]));
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 1);
    assert_eq!(g.hand_size(1), 1);
}

#[test]
fn yanyu_recast_sha_and_end_draws_two_for_male() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["yanyu".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "s1", "sha", Suit::Spade, Zone::Hand(0));
    assert!(skills::b20::recast_allowed(&g, 0, "s1"));
    g.bump("phase.0.yanyu_sha");
    g.bump("phase.0.yanyu_sha");
    for i in 0..2 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    skills::b20::on_phase_end(&mut g, 0, Phase::Play);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 2);
}

#[test]
fn qiangshi_reveals_and_draws_on_same_category() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiangshi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "a1", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    trigger(&mut g, "qiangshi", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    submit(&mut g, "pick:h1");
    assert_eq!(g.mark("turn.0.qiangshi_category"), Some("basic"));
    skills::b20::on_use_start(&mut g, 0, "a1");
    assert_eq!(g.hand_size(0), 2, "同类别使用摸 1");
}

#[test]
fn xiantu_gives_cards_and_penalizes_insufficient_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xiantu".into()];
    g.turn = 1;
    g.phase = Phase::Play;
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "h2", "shan", Suit::Heart, Zone::Hand(0));
    for i in 0..2 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "xiantu", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "use");
    submit(&mut g, "n:2");
    submit(&mut g, "pick:h1");
    submit(&mut g, "pick:h2");
    assert_eq!(g.hand_size(1), 2);
    skills::b20::on_phase_end(&mut g, 1, Phase::Play);
    assert_eq!(g.seats[0].hp, 3, "伤害不足 → 张松失去 1 体力");
}

#[test]
fn zhenjun_discards_x_and_self_discards_non_equip_count() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhenjun".into()];
    g.turn = 0;
    g.phase = Phase::Prepare;
    mk(&mut g, "t1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    trigger(&mut g, "zhenjun", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    submit(&mut g, "hand");
    submit(&mut g, "self");
    assert_eq!(g.hand_size(1), 0);
}

#[test]
fn yizhong_black_slash_ineffective_and_shan_blocked() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["yizhong".into()];
    g.seats[0].hp = 4;
    g.seats[1].hp = 4;
    mk(&mut g, "b1", "sha", Suit::Spade, Zone::Processing);
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Processing);
    assert!(skills::b20::card_ineffective_for(&mut g, 1, 0, "b1"));
    assert!(!skills::b20::card_ineffective_for(&mut g, 1, 0, "r1"));
    g.seats[1].hp = 3; // 于禁 hp 更高 → 不触发
    assert!(!skills::b20::card_ineffective_for(&mut g, 1, 0, "b1"));
    assert!(skills::b20::shan_blocked_for(&g, 0, 1, "b1"));
}

#[test]
fn anxu_takes_two_cards_then_gives_one_to_lower_hand() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["anxu".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "a1", "sha", Suit::Spade, Zone::Hand(1));
    mk(&mut g, "b1", "shan", Suit::Heart, Zone::Hand(2));
    assert!(skills::use_active(&mut g, 0, "anxu").is_ok());
    submit(&mut g, "a:1");
    submit(&mut g, "b:2");
    // grab auto, then give
    submit(&mut g, "give:a1");
    assert_eq!(g.hand_size(0), 1, "另一张获得，交出一张");
}

#[test]
fn zhuixi_end_and_death_repeat() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhuixi".into()];
    g.turn = 0;
    g.phase = Phase::End;
    for i in 0..3 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "zhuixi", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.hand_size(1), 3);
    // 死亡时再次发动
    for i in 3..6 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "zhuixi", 0, data(&[("event", "Death".into())]));
    assert_eq!(g.hand_size(1), 6, "死亡时再次摸三张");
}
