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

// ---------------------------------------------------------------------------
// B18 其余技能
// ---------------------------------------------------------------------------

#[test]
fn zhenlie_loses_hp_and_makes_card_ineffective() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhenlie".into()];
    mk(&mut g, "c1", "sha", Suit::Spade, Zone::Processing);
    trigger(&mut g, "zhenlie", 0, data(&[("step", "ask".into()), ("user", json!(1)), ("target", json!(0)), ("card", json!("c1"))]));
    submit(&mut g, "use");
    assert_eq!(g.seats[0].hp, 3);
    assert!(g.use_opts("c1").target_ineffective.contains(&0));
}

#[test]
fn miji_draws_lost_hp_at_end_phase() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["miji".into()];
    g.seats[0].hp = 2;
    g.turn = 0;
    g.phase = Phase::End;
    for i in 0..2 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "miji", 0, data(&[("event", "PhaseStart".into())]));
    submit(&mut g, "use");
    assert_eq!(g.hand_size(0), 2);
}

#[test]
fn jianmie_discards_chosen_color_and_duels() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jianmie".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "r2", "sha", Suit::Diamond, Zone::Hand(0));
    mk(&mut g, "br", "sha", Suit::Spade, Zone::Hand(1));
    assert!(skills::use_active(&mut g, 0, "jianmie").is_ok());
    submit(&mut g, "target:1");
    submit(&mut g, "red");
    submit(&mut g, "black");
    assert_eq!(g.hand_size(0), 0, "弃光红色手牌");
    assert_eq!(g.hand_size(1), 0, "对方选黑，弃黑后无红可弃");
}

#[test]
fn zongshi_prevents_damage_and_source_gains_card() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zongshi".into()];
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    assert!(sanguosha3::kernel::skills::b18::should_ask_damage_before_victim(&g, 1, 0));
    trigger(&mut g, "zongshi", 0, data(&[("step", "ask".into()), ("target", json!(0)), ("from", json!(1))]));
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    submit(&mut g, "hand");
    assert_eq!(g.hand_size(0), 0);
    assert_eq!(g.hand_size(1), 1, "来源获得一张牌");
}

#[test]
fn xianzhou_gives_equips_and_target_heals() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["xianzhou".into()];
    g.seats[0].hp = 2;
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "e1", "qinggang_jian", Suit::Spade, Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    assert!(skills::use_active(&mut g, 0, "xianzhou").is_ok());
    submit(&mut g, "target:1");
    submit(&mut g, "heal");
    assert_eq!(g.seats[0].hp, 3, "回复 X=1");
    assert_eq!(g.cards.get("e1"), Some(&Zone::Hand(1)));
}

#[test]
fn qieting_on_other_turn_end_offers_draw() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qieting".into()];
    sanguosha3::kernel::skills::b18::on_turn_end(&mut g, 1);
    resolve::advance(&mut g).unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    submit(&mut g, "draw");
    assert_eq!(g.hand_size(0), 1);
}

#[test]
fn chengxiang_asks_pool_and_bonus_on_13() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["chengxiang".into()];
    for i in 0..4 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    trigger(&mut g, "chengxiang", 0, data(&[("event", "DamageAfter".into())]));
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    submit(&mut g, "ok");
}

#[test]
fn renxin_discards_equip_and_restores_dying_target() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["renxin".into()];
    g.seats[1].hp = 0;
    mk(&mut g, "e1", "qinggang_jian", Suit::Spade, Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    trigger(&mut g, "renxin", 0, data(&[("step", "ask".into()), ("target", json!(1))]));
    submit(&mut g, "pick:e1");
    assert_eq!(g.seats[1].hp, 1);
    assert!(g.seats[0].flipped);
}

#[test]
fn jiushi_flips_and_uses_wine() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiushi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    assert!(skills::use_active(&mut g, 0, "jiushi").is_ok());
    assert!(g.seats[0].flipped, "翻至背面");
    assert_eq!(g.counter("turn.0.jiushi_used"), 1);
}

#[test]
fn ganlu_swaps_equip_zones() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["ganlu".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "w1", "qinggang_jian", Suit::Spade, Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    mk(&mut g, "w2", "zhuge_liannu", Suit::Club, Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    assert!(skills::use_active(&mut g, 0, "ganlu").is_ok());
    submit(&mut g, "a:0");
    submit(&mut g, "b:1");
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.cards.get("w2"), Some(&Zone::Equip(0, sanguosha3::kernel::Slot::Weapon)));
}

#[test]
fn buyi_discards_non_basic_and_heals_dying() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["buyi".into()];
    g.seats[1].hp = 0;
    mk(&mut g, "s1", "sha", Suit::Spade, Zone::Hand(1));
    trigger(&mut g, "buyi", 0, data(&[("step", "ask".into()), ("target", json!(1))]));
    submit(&mut g, "pick:s1");
    assert_eq!(g.seats[1].hp, 0, "基本牌不弃置不回复");
}

#[test]
fn dangxian_extra_play_and_self_damage_without_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["dangxian".into()];
    g.turn = 0;
    assert!(sanguosha3::kernel::skills::b18::take_extra_play(&mut g, 0));
    assert!(!sanguosha3::kernel::skills::b18::take_extra_play(&mut g, 0), "每回合一次");
    sanguosha3::kernel::skills::b18::on_extra_play_end(&mut g, 0);
    assert_eq!(g.seats[0].hp, 3, "未造成伤害 → 自伤 1");
}

#[test]
fn fuli_dying_option_and_restore() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["fuli".into()];
    g.seats[0].hp = 0;
    g.seats[0].kingdom = "wei".into();
    g.seats[1].kingdom = "shu".into();
    g.seats[2].kingdom = "wu".into();
    assert!(!sanguosha3::kernel::skills::b18::dying_options(&g, 0, 0).is_empty());
    trigger(&mut g, "fuli", 0, data(&[("stage", "dying".into())]));
    assert!(g.seats[0].hp >= 1);
    assert!(skills::limited_used(&g, 0, "fuli"));
}

#[test]
fn shangshi_retriggers_each_time_hand_drops_below_lost_hp() {
    // B18 Q4=A：每次手牌数跌破 X 都询问（不再「每回合一次」）。
    let mut g = test_game(3);
    g.seats[0].skills = vec!["shangshi".into()];
    g.seats[0].hp = 2; // 已损失 2 点 → X=2
    g.phase = Phase::Play;
    for i in 0..2 {
        mk(&mut g, &format!("d{i}"), "sha", Suit::Spade, Zone::Draw);
        g.deck.push(format!("d{i}"));
    }
    mk(&mut g, "h1", "sha", Suit::Club, Zone::Hand(0));
    // 第一次：手牌 1 < 2 → 询问并摸至 2。
    skills::b18::on_hand_changed(&mut g, 0);
    resolve::advance(&mut g).expect("advance");
    submit(&mut g, "draw");
    assert_eq!(g.hand_size(0), 2);
    // 再次跌破 → 再次询问（同一回合内）。
    let card = g.hand(0)[0].clone();
    flow::discard(&mut g, 0, &card, "测试");
    skills::b18::on_hand_changed(&mut g, 0);
    resolve::advance(&mut g).expect("advance");
    assert!(g.pending.is_some(), "同回合第二次跌破仍应询问");
}
