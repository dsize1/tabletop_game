//! B18 技能门禁（J6：8 将 / 17 技能；本文件随批次推进增长）。

use serde_json::json;
use sanguosha3::kernel::resolve::{self, Stage};
use sanguosha3::kernel::{flow, skills, Card, Game, Phase, Seat, Suit, Zone};

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
fn jueqing_turns_damage_into_hp_loss() {
    use sanguosha3::kernel::{ArmorKind, DamageKind, DamageSpec, SourceKind};
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jueqing".into()];
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Skill };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    assert!(!g.log.iter().any(|e| e.kind == sanguosha3::kernel::event::EventKind::DamageAfter), "不产生伤害事件");
    assert!(g.log.iter().any(|e| e.kind == sanguosha3::kernel::event::EventKind::HpLost));
    assert_eq!(g.seats[1].hp, 3);
}

#[test]
fn zishou_draw_bonus_and_end_phase_discard() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zishou".into()];
    g.seats[0].kingdom = "wei".into();
    g.seats[1].kingdom = "shu".into();
    g.seats[2].kingdom = "wu".into();
    g.turn = 0;
    g.phase = Phase::Draw;
    assert_eq!(sanguosha3::kernel::skills::b18::draw_bonus(&mut g, 0), 3);
    g.phase = Phase::End;
    g.bump("turn.0.zishou_damage_card");
    for i in 0..3 {
        mk(&mut g, &format!("h{i}"), "sha", Suit::Spade, Zone::Hand(0));
    }
    trigger(&mut g, "zishou", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "pick:h0");
    submit(&mut g, "pick:h1");
    submit(&mut g, "pick:h2");
    assert_eq!(g.hand_size(0), 0, "弃 X 张");
}

#[test]
fn luoying_gains_other_club_card_and_marks_it() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["luoying".into()];
    mk(&mut g, "c1", "sha", Suit::Club, Zone::Discard);
    trigger(&mut g, "luoying", 0, data(&[("event", "CardToDiscard".into()), ("card", json!("c1")), ("victim", json!(1)), ("cause", json!("测试"))]));
    submit(&mut g, "gain");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Hand(0)));
    assert_eq!(g.card_mark("c1"), Some("luoying"));
}

#[test]
fn shangshi_fills_hand_to_lost_hp_on_hand_change() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shangshi".into()];
    g.seats[0].hp = 2; // 已损失 2
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    mk(&mut g, "d2", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    g.deck.push("d2".into());
    sanguosha3::kernel::skills::b18::on_hand_changed(&mut g, 0);
    resolve::advance(&mut g).unwrap();
    submit(&mut g, "draw");
    assert_eq!(g.hand_size(0), 2, "摸至 X=2");
}
