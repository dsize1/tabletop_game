//! B19 技能门禁（J7：8 将 / 15 技能；马术复用 B9）。

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
fn mieji_puts_trick_on_top_and_target_discards_two() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["mieji".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "t1", "shunshou_qianyang", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "h1", "sha", Suit::Heart, Zone::Hand(1));
    mk(&mut g, "h2", "shan", Suit::Club, Zone::Hand(1));
    assert!(skills::use_active(&mut g, 0, "mieji").is_ok());
    submit(&mut g, "pick:t1");
    submit(&mut g, "target:1");
    submit(&mut g, "two");
    assert_eq!(g.hand_size(1), 0);
    assert_eq!(g.cards.get("t1"), Some(&Zone::Draw));
}

#[test]
fn juece_end_phase_damages_low_hand_target() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["juece".into()];
    g.turn = 0;
    g.phase = Phase::End;
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    trigger(&mut g, "juece", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "target:1");
    assert_eq!(g.seats[1].hp, 3);
}

#[test]
fn jingce_counts_suits_types_and_draws_at_play_end() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jingce".into()];
    g.turn = 0;
    mk(&mut g, "a", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "b", "shan", Suit::Heart, Zone::Hand(0));
    skills::b19::on_use_start(&mut g, 0, "a");
    skills::b19::on_use_start(&mut g, 0, "b");
    assert_eq!(skills::b19::hand_limit_bonus(&g, 0), 2, "两种花色 +2");
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    skills::b19::on_phase_end(&mut g, 0, Phase::Play);
    assert_eq!(g.hand_size(0), 3, "原 2 张 + 摸类型数 1");
}

#[test]
fn shibei_starts_with_armor_and_first_damage_heals() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shibei".into()];
    g.seats[0].hp = 3;
    skills::fire_game_start(&mut g);
    assert_eq!(g.seats[0].armor, 3);
    trigger(&mut g, "shibei", 0, data(&[("event", "DamageAfter".into())]));
    assert_eq!(g.seats[0].hp, 4, "首次受伤回复 1");
    trigger(&mut g, "shibei", 0, data(&[("event", "DamageAfter".into())]));
    assert_eq!(g.seats[0].hp, 3, "再次受伤失去 1");
}

#[test]
fn qianxi_reveals_card_and_blocks_same_color_at_distance_one() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qianxi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Hand(0));
    trigger(&mut g, "qianxi", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "pick:r1");
    assert!(g.is_revealed("r1"));
    assert!(skills::b19::hand_color_blocked(&g, 1, "r1"), "距离 1 同色手牌封锁");
}
