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

// ---------------------------------------------------------------------------
// 焚城 / 惴恐 / 求援 / 弓骑 / 解烦 / 父魂 / 矫诏 / 殚心 / 渐营
// ---------------------------------------------------------------------------

#[test]
fn fencheng_sequence_and_fire_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["fencheng".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    assert!(skills::use_active(&mut g, 0, "fencheng").is_ok());
    submit(&mut g, "target:1");
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(2));
    submit(&mut g, "damage");
    assert_eq!(g.seats[2].hp, 2, "2 点火焰伤害");
}

#[test]
fn zhuikong_win_blocks_turn_player_targeting_others() {
    use sanguosha3::kernel::pindian::PindianResult;
    let mut g = test_game(3);
    g.seats[0].skills = vec!["zhuikong".into()];
    g.seats[0].hp = 3;
    g.turn = 1;
    g.phase = Phase::Prepare;
    g.pindian_result = Some(PindianResult { initiator: 0, target: 1, a: "a".into(), b: "b".into(), rank_a: 9, rank_b: 3, winner: Some(0) });
    trigger(&mut g, "zhuikong", 0, data(&[("event", "PhaseStart".into()), ("stage", "wait".into())]));
    assert!(skills::b19::others_use_blocked(&g, 1));
}

#[test]
fn qiuyuan_helper_becomes_extra_target() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["qiuyuan".into()];
    mk(&mut g, "c1", "sha", Suit::Spade, Zone::Processing);
    trigger(&mut g, "qiuyuan", 2, data(&[("step", "ask".into()), ("stage", "ask".into()), ("user", json!(1)), ("target", json!(0)), ("card", json!("c1")), ("kind", json!("Basic"))]));
    // helper 2 求援
    let actor = g.pending.as_ref().map(|p| p.actor()).unwrap();
    resolve::submit(&mut g, actor, "become").unwrap();
    assert!(g.use_opts("c1").mid_targets.contains(&2));
}

#[test]
fn gongqi_discard_grants_infinite_range_and_same_suit_unlimited() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["gongqi".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "r1", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "r2", "sha", Suit::Heart, Zone::Hand(0));
    assert!(skills::use_active(&mut g, 0, "gongqi").is_ok());
    submit(&mut g, "pick:r1");
    assert!(skills::b19::infinite_range(&g, 0));
    assert!(skills::b19::sha_unlimited_card(&g, 0, "r2"));
    assert!(!skills::b19::sha_unlimited_card(&g, 0, "b1"));
}

#[test]
fn jiefan_first_round_resets_limited() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiefan".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.round = 1;
    assert!(skills::use_active(&mut g, 0, "jiefan").is_ok());
    submit(&mut g, "target:1");
    // 2 号在 1 号攻击范围内，选择令目标摸牌。
    let actor = g.pending.as_ref().map(|p| p.actor()).unwrap();
    resolve::submit(&mut g, actor, "draw").unwrap();
    assert!(skills::limited_used(&g, 0, "jiefan"));
    skills::b19::on_turn_end(&mut g, 0);
    assert!(!skills::limited_used(&g, 0, "jiefan"), "第一轮结束视为未发动");
}

#[test]
fn fuhen_pair_sets_color_lock_and_grants_after_damage() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["fuhen".into()];
    mk(&mut g, "a", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "b", "shan", Suit::Diamond, Zone::Hand(0));
    g.virtual_origins.insert("v:1".into(), vec!["a".into(), "b".into()]);
    mk(&mut g, "v:1", "sha", Suit::Heart, Zone::Processing);
    skills::b19::on_use_start_fuhen(&mut g, 0, "v:1");
    assert_eq!(g.use_opts("v:1").response_color_lock, Some(true));
    g.phase = Phase::Play;
    g.bump("turn.0.fuhen_pair");
    trigger(&mut g, "fuhen", 0, data(&[("event", "DamageAfter".into())]));
    assert!(skills::active(&g, 0, "wusheng") && skills::active(&g, 0, "paoxiao"));
    skills::b19::on_turn_end(&mut g, 0);
    assert!(!skills::active(&g, 0, "wusheng"));
}

#[test]
fn jiazhao_declares_unused_card_and_plus_display_name() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiazhao".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].add_counter("jiazhao_mod", 1);
    mk(&mut g, "h1", "sha", Suit::Spade, Zone::Hand(0));
    assert_eq!(skills::display_name(&g, 0, "jiazhao"), "矫诏plus");
    assert!(skills::use_active(&mut g, 0, "jiazhao").is_ok());
    submit(&mut g, "pick:h1");
    let opts = g.pending.as_ref().unwrap();
    assert!(opts.options().iter().any(|o| o.id.starts_with("code:")));
}

#[test]
fn danxin_draws_and_modifies_jiazhao_up_to_two() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jiazhao".into(), "danxin".into()];
    g.seats[0].add_counter("jiazhao_mod", 1);
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    trigger(&mut g, "danxin", 0, data(&[("event", "DamageAfter".into())]));
    assert_eq!(g.hand_size(0), 1);
    assert_eq!(g.seats[0].counter("jiazhao_mod"), 2);
    trigger(&mut g, "danxin", 0, data(&[("event", "DamageAfter".into())]));
    assert_eq!(g.seats[0].counter("jiazhao_mod"), 2, "最多 2 次");
}

#[test]
fn jianying_draws_on_same_suit_and_inherits_suit() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["jianying".into()];
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "a", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "b", "shan", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "d1", "sha", Suit::Spade, Zone::Draw);
    g.deck.push("d1".into());
    skills::b19::on_use_start_jianying(&mut g, 0, "a");
    skills::b19::on_use_start_jianying(&mut g, 0, "b");
    assert_eq!(g.hand_size(0), 3, "同花色摸 1");
    g.set_counter("turn.0.jianying_inherit", 1);
    assert_eq!(skills::b19::suit_for_use(&g, 0, Suit::Spade), Suit::Heart);
}

#[test]
fn fuhen_pair_response_options_and_validation() {
    let mut g = test_game(3);
    g.seats[0].skills = vec!["fuhen".into()];
    mk(&mut g, "a", "sha", Suit::Heart, Zone::Hand(0));
    mk(&mut g, "b", "shan", Suit::Spade, Zone::Hand(0));
    let opts = skills::pair_response_options(&g, 0, sanguosha3::kernel::cards::ResponseKind::Sha);
    assert!(opts.iter().any(|(id, _)| id == "pair:a,b"), "父魂两张手牌可当杀打出");
    assert!(skills::pair_response_valid(&g, 0, "a", "b"));
    sanguosha3::kernel::card_play::play_pair_response(&mut g, 0, "a", "b", "测试").unwrap();
    assert_eq!(g.cards.get("a"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("b"), Some(&Zone::Discard));
    // 反例：非父魂。
    g.seats[0].skills.clear();
    mk(&mut g, "c", "sha", Suit::Heart, Zone::Hand(0));
    assert!(skills::pair_response_options(&g, 0, sanguosha3::kernel::cards::ResponseKind::Sha).is_empty());
}
