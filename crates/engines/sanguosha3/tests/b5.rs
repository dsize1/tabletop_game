//! B5 技能门禁（6 将 / 13 技能）。

use sanguosha3::kernel::resolve::{Ask, Stage, Step};
use sanguosha3::kernel::{skills, Card, Game, Phase, Seat, Suit, Zone};

fn seat_with(id: usize, list: &[&str], kingdom: &str) -> Seat {
    Seat { id, hp: 4, max_hp: 4, alive: true, skills: list.iter().map(|s| s.to_string()).collect(), kingdom: kingdom.to_string(), ..Default::default() }
}
fn game(n: usize, s0: &[&str]) -> Game {
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "wei")).collect();
    seats[0] = seat_with(0, s0, "wei");
    Game { seats, ..Default::default() }
}
fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(id.to_string(), Card { id: id.to_string(), def: def.to_string(), suit, rank });
}
fn add(g: &mut Game, id: &str, def: &str, zone: Zone) { add_suit(g, id, def, zone, Suit::Spade, 7); }
fn add_deck(g: &mut Game, n: usize) { for i in 0..n { let id = format!("d{i}"); add(g, &id, "sha", Zone::Draw); g.deck.push(id); } }
fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs { m.insert((*k).to_string(), v.clone()); }
    serde_json::Value::Object(m)
}
fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d }).expect("不应报错")
}
fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::Next(_, next) | Step::NeedWith(_, next, _) => { data = next.clone(); step = call(g, skill, actor, data.clone()); }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let p = match &step { Step::Need(p) | Step::NeedAt(_, p) => p, _ => unreachable!() };
        assert!(p.has_option(ans), "选项 {ans} 不在 {:?}", p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>());
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        step = call(g, skill, actor, data.clone());
    }
    step
}

#[test]
fn zanhui_helper_choice() {
    let mut g = game(3, &["zanhui"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "c", "sha", Zone::Processing, Suit::Spade, 7);
    g.use_targets.insert("c".into(), vec![1]);
    add(&mut g, "h", "tao", Zone::Hand(2));
    let step = run_steps(&mut g, "zanhui", 0, data(&[("card", "c".into())]), &["seat:2", "extra"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.use_opts("c").extra_targets, 1, "协助者成为额外目标");
}

#[test]
fn jiaojin_reduces_damage_by_one() {
    let mut g = game(3, &["jiaojin"]);
    g.seats[1].male = true;
    add(&mut g, "e", "qinggang_jian", Zone::Hand(0));
    let step = run_steps(&mut g, "jiaojin", 0, data(&[("from", 1.into()), ("amount", 2.into())]), &["use", "card:e"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3, "伤害 2-1=1");
    // 反例：无装备牌则直接受伤。
    let mut g = game(3, &["jiaojin"]);
    g.seats[1].male = true;
    call(&mut g, "jiaojin", 0, data(&[("from", 1.into()), ("amount", 2.into())]));
    assert_eq!(g.seats[0].hp, 2);
}

#[test]
fn fenli_skips_phase_and_counts_for_pingkou() {
    let mut g = game(3, &["fenli", "pingkou"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    for i in 0..3 { add(&mut g, &format!("c{i}"), "sha", Zone::Hand(0)); }
    let step = run_steps(&mut g, "fenli", 0, data(&[]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.skipped.contains(&Phase::Draw), "跳过摸牌阶段");
    assert_eq!(g.counter("turn.0.pingkou"), 1, "平寇计数 +1");
    // 反例：非严格最大不提供。
    let mut g = game(3, &["fenli"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    assert!(matches!(call(&mut g, "fenli", 0, data(&[])), Step::Done));
}

#[test]
fn pingkou_damages_per_skipped_phase() {
    let mut g = game(3, &["pingkou"]);
    g.turn = 0;
    g.set_counter("turn.0.pingkou", 2);
    let step = run_steps(&mut g, "pingkou", 0, data(&[]), &["seat:1", "pass"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 3, "第一次造成 1 点伤害");
    // 反例：计数 0 不询问。
    let mut g = game(3, &["pingkou"]);
    g.turn = 0;
    assert!(matches!(call(&mut g, "pingkou", 0, data(&[])), Step::Done));
}

#[test]
fn shenxing_discard_two_draw_one() {
    let mut g = game(2, &["shenxing"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 1);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "shan", Zone::Hand(0));
    let step = run_steps(&mut g, "shenxing", 0, data(&[]), &["card:a", "card:b"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 1, "弃两张摸一张");
    // 反例：不足两张不可发动。
    let mut g = game(2, &["shenxing"]);
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    assert!(!skills::active_actions(&g, 0).contains(&"shenxing"));
}

#[test]
fn bingyi_same_color_draws_for_others() {
    let mut g = game(3, &["bingyi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "c1", "sha", Zone::Discard);
    add_deck(&mut g, 1);
    add_suit(&mut g, "h1", "sha", Zone::Hand(0), Suit::Spade, 7);
    add_suit(&mut g, "h2", "shan", Zone::Hand(0), Suit::Club, 7);
    let step = run_steps(&mut g, "bingyi", 0, data(&[("card", "c1".into())]), &["use", "seat:1", "ok"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 1, "同色 → 令其摸一张");
}

#[test]
fn huituo_judges_red_for_heal() {
    let mut g = game(2, &["huituo"]);
    g.seats[0].hp = 3;
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Heart, 7);
    g.deck.push("top".into());
    let step = run_steps(&mut g, "huituo", 0, data(&[("amount", 1.into())]), &["seat:0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 4, "红色 → 回复 1 点");
    // 反例：黑色 → 摸伤害值张。
    let mut g = game(2, &["huituo"]);
    add_suit(&mut g, "top", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("top".into());
    add_deck(&mut g, 2);
    let step = run_steps(&mut g, "huituo", 0, data(&[("amount", 2.into())]), &["seat:0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 2, "黑色 → 摸 2 张");
}

#[test]
fn mingjian_gives_all_hand_and_buffs_next_turn() {
    let mut g = game(3, &["mingjian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    let limit = g.hand_limit(1);
    let step = run_steps(&mut g, "mingjian", 0, data(&[]), &["seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(0).is_empty(), "所有手牌交给其");
    assert!(g.hand(1).contains(&"a".to_string()));
    assert_eq!(g.hand_limit(1), limit + 1, "其下回合手牌上限 +1");
    assert_eq!(skills::sha_extra(&g, 1), 1, "其【杀】次数 +1");
}

#[test]
fn xingshuai_supporters_heal_then_take_damage() {
    let mut g = game(3, &["xingshuai"]);
    g.seats[0].identity = Some(sanguosha3::kernel::Identity::Lord);
    g.seats[0].hp = 0;
    g.seats[1].kingdom = "wei".into();
    g.seats[2].kingdom = "shu".into();
    let step = run_steps(&mut g, "xingshuai", 0, data(&[]), &["use"]);
    assert!(matches!(step, Step::Finish(_) | Step::Next(..)));
    assert_eq!(g.seats[0].hp, 1, "魏势力支援回复 1 点（0→1）");
    // 支援者代价在「濒死结算结束」后派发：推进到收口分支。
    let _ = call(&mut g, "xingshuai", 0, data(&[("step", "ask".into()), ("cursor", 1.into()), ("helped", 1.into())]));
    assert!(!g.after_dying.is_empty(), "支援者代价延后结算");
    // 反例：非主公不触发。
    let mut g = game(3, &["xingshuai"]);
    g.seats[1].kingdom = "wei".into();
    assert!(matches!(call(&mut g, "xingshuai", 0, data(&[])), Step::Done));
}

#[test]
fn qianju_distance_penalty_per_lost_hp() {
    let mut g = game(4, &["qianju"]);
    let before = sanguosha3::kernel::gear::distance(&g, 0, 2);
    g.seats[0].hp = 1;
    let after = sanguosha3::kernel::gear::distance(&g, 0, 2);
    assert!(after < before, "每损失 1 点体力距离 -1");
    // 反例：无千驹不受影响。
    let mut g = game(4, &[]);
    g.seats[0].hp = 1;
    assert_eq!(sanguosha3::kernel::gear::distance(&g, 0, 2), 2);
}

#[test]
fn qingxi_target_choice_discard_or_plus_one() {
    let mut g = game(3, &["qingxi"]);
    add(&mut g, "w", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "c1", "sha", Zone::Hand(1));
    let step = run_steps(&mut g, "qingxi", 0, data(&[("target", 1.into()), ("amount", 1.into())]), &["use", "plus"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 2, "伤害 +1（2 点）");
    // 反例：无武器不触发。
    let mut g = game(3, &["qingxi"]);
    assert!(matches!(call(&mut g, "qingxi", 0, data(&[("target", 1.into())])), Step::Done));
}

#[test]
fn huomo_uses_black_nonbasic_as_basic() {
    let mut g = game(2, &["huomo"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "c", "wuxie_keji", Zone::Hand(0), Suit::Spade, 7);
    let step = run_steps(&mut g, "huomo", 0, data(&[]), &["card:c", "use:sha"]);
    assert!(matches!(step, Step::Done));
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("c"), "置于牌堆顶");
    assert!(g.card_defs.values().any(|c| c.id.starts_with("v:")), "视为使用基本牌");
    // 反例：红色非基本牌不可用。
    let mut g = game(2, &["huomo"]);
    g.phase = Phase::Play;
    add_suit(&mut g, "c", "wuxie_keji", Zone::Hand(0), Suit::Heart, 7);
    assert!(!skills::active_actions(&g, 0).contains(&"huomo"));
}

#[test]
fn zuoding_lets_target_draw_on_spade() {
    let mut g = game(3, &["zuoding"]);
    g.turn = 1;
    g.phase = Phase::Play;
    add_deck(&mut g, 1);
    add_suit(&mut g, "c", "sha", Zone::Processing, Suit::Spade, 7);
    g.use_targets.insert("c".into(), vec![2]);
    let step = run_steps(&mut g, "zuoding", 0, data(&[("card", "c".into())]), &["seat:2"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(2), 1, "令其中一个目标摸一张");
    // 反例：红桃不触发。
    let mut g = game(3, &["zuoding"]);
    g.turn = 1;
    g.phase = Phase::Play;
    add_suit(&mut g, "c", "sha", Zone::Processing, Suit::Heart, 7);
    assert!(matches!(call(&mut g, "zuoding", 0, data(&[("card", "c".into())])), Step::Done));
}
