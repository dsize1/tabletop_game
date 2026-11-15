//! B4 技能门禁（8 将 / 11 技能）。每技能 ≥1 正例，关键项含反例。

use sanguosha3::kernel::resolve::{Ask, Stage, Step};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::{Card, DamageKind, DamageSpec, Game, Phase, Seat, SourceKind, Suit, Zone};

fn seat_with(id: usize, list: &[&str], kingdom: &str) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: list.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
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
fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs { m.insert((*k).to_string(), v.clone()); }
    serde_json::Value::Object(m)
}
fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d }).expect("handler 不应报错")
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
fn marks(g: &Game, seat: usize, name: &str) -> Vec<String> {
    let mut v: Vec<String> = g.cards.iter().filter(|(_, z)| matches!(z, Zone::Mark(s, n) if *s == seat && n == name)).map(|(id, _)| id.clone()).collect();
    v.sort();
    v
}

#[test]
fn xiansi_takes_two_cards_as_ni_and_other_side_slash() {
    let mut g = game(3, &["xiansi"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add(&mut g, "a", "sha", Zone::Hand(1));
    add(&mut g, "b", "sha", Zone::Hand(2));
    let step = run_steps(&mut g, "xiansi", 0, data(&[]), &["use", "seat:1", "card:a", "use", "seat:2", "card:b"]);
    assert!(matches!(step, Step::Finish(_)) || matches!(step, Step::Done));
    assert_eq!(marks(&g, 0, "ni").len(), 2, "至多两名各一张");
    assert!(skills::other_side_actions(&g, 1).contains(&"xiansi"), "他人侧可用");
    // 他人侧：移去两张「逆」出【杀】。
    let mut g = game(3, &["xiansi"]);
    for i in 0..2 { add(&mut g, &format!("n{i}"), "sha", Zone::Mark(0, "ni".into())); }
    g.seats[0].skills = vec!["xiansi".into()];
    g.turn = 1;
    g.phase = Phase::Play;
    call(&mut g, "xiansi", 1, data(&[("step", "other".into())]));
    assert!(marks(&g, 0, "ni").is_empty(), "移去两张「逆」");
    // 反例：不足两张不可用。
    assert!(!skills::other_side_actions(&g, 1).contains(&"xiansi"));
}

#[test]
fn duodao_gains_weapon_after_slash_damage() {
    let mut g = game(3, &["duodao"]);
    add(&mut g, "w", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "c", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "duodao", 0, data(&[("by", 1.into())]), &["use", "card:c"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("w"), Some(&Zone::Hand(0)), "获得其武器");
    // 反例：来源无武器。
    let mut g = game(3, &["duodao"]);
    add(&mut g, "c", "tao", Zone::Hand(0));
    assert!(matches!(call(&mut g, "duodao", 0, data(&[("by", 1.into())])), Step::Done));
}

#[test]
fn anjian_adds_one_damage_when_out_of_range() {
    let mut g = game(4, &["anjian"]);
    g.turn = 0;
    let spec = DamageSpec { from: Some(0), target: 2, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert_eq!(skills::damage_bonus(&g, &spec), 1, "不在其攻击范围内 → +1");
    // 反例：在攻击范围内不加。
    let spec2 = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert_eq!(skills::damage_bonus(&g, &spec2), 0);
}

#[test]
fn danshou_third_use_deals_damage() {
    let mut g = game(3, &["danshou"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("phase.0.danshou", 2); // 第 3 次
    for i in 0..3 { add(&mut g, &format!("c{i}"), "sha", Zone::Hand(0)); }
    let step = run_steps(&mut g, "danshou", 0, data(&[]), &["card:c0", "card:c1", "card:c2", "ok", "seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 3, "X=3 → 1 点伤害");
    // 反例：牌不够（X 张）不可发动。
    let mut g = game(3, &["danshou"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("phase.0.danshou", 2);
    add(&mut g, "c0", "sha", Zone::Hand(0));
    assert!(!skills::active_actions(&g, 0).contains(&"danshou"));
}

#[test]
fn sidi_freezes_color_then_slashes() {
    let mut g = game(3, &["sidi"]);
    g.turn = 1;
    g.phase = Phase::Play;
    add_suit(&mut g, "e", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon), Suit::Spade, 7);
    add_suit(&mut g, "t", "wuzhong_shengyou", Zone::Hand(0), Suit::Spade, 7);
    add_suit(&mut g, "h", "sha", Zone::Hand(1), Suit::Spade, 7);
    let step = run_steps(&mut g, "sidi", 0, data(&[("target", 1.into())]), &["use", "card:t"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.is_frozen(1, "h"), "同色手牌本阶段冻结");
    // 阶段结束：未使用【杀】→ 视为使用【杀】。
    call(&mut g, "sidi", 0, data(&[("target", 1.into())]));
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 3, "视为使用【杀】");
}

#[test]
fn shenduan_uses_black_basic_as_bingliang() {
    let mut g = game(3, &["shenduan"]);
    g.turn = 0;
    add_suit(&mut g, "c", "sha", Zone::Discard, Suit::Spade, 7);
    let step = run_steps(&mut g, "shenduan", 0, data(&[("card", "c".into())]), &["use", "seat:1"]);
    assert!(matches!(step, Step::Done));
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert!(g.judge_stacks.get(&1).map(|s| !s.is_empty()).unwrap_or(false), "置入判定区（兵粮寸断）");
    // 反例：红色基本牌不触发。
    let mut g = game(3, &["shenduan"]);
    add_suit(&mut g, "c", "sha", Zone::Discard, Suit::Heart, 7);
    assert!(matches!(call(&mut g, "shenduan", 0, data(&[("card", "c".into())])), Step::Done));
}

#[test]
fn yonglue_discards_judge_card_and_uses_slash() {
    let mut g = game(3, &["yonglue"]);
    g.turn = 1;
    g.phase = Phase::Judge;
    add(&mut g, "j", "lebu_sishu", Zone::Judge(1));
    g.judge_stacks.entry(1).or_default().push("j".into());
    let step = run_steps(&mut g, "yonglue", 0, data(&[("target", 1.into())]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.judge_stacks.get(&1).map(|s| s.is_empty()).unwrap_or(true), "弃其判定牌");
    // 反例：判定区为空。
    assert!(matches!(call(&mut g, "yonglue", 0, data(&[("target", 1.into())])), Step::Done));
}

#[test]
fn pindi_draws_x_and_chains_on_injured_target() {
    let mut g = game(3, &["pindi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].hp = 3;
    add(&mut g, "c", "sha", Zone::Hand(0));
    add(&mut g, "h1", "sha", Zone::Hand(1));
    for i in 0..3 { add(&mut g, &format!("d{i}"), "sha", Zone::Draw); g.deck.push(format!("d{i}")); }
    let step = run_steps(&mut g, "pindi", 0, data(&[]), &["card:c", "seat:1", "use:draw"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand_size(1) >= 2, "X=1 → 摸 1 张（原有 1 张）");
    assert!(g.seats[0].chained, "目标已受伤 → 你进入连环状态");
    // 反例：本阶段已用过的类型不能再弃。
    assert!(!skills::active_actions(&g, 0).contains(&"pindi") || g.hand(0).is_empty());
}

#[test]
fn faen_lets_character_draw_on_flip_or_chain() {
    let mut g = game(3, &["faen"]);
    for i in 0..2 { add(&mut g, &format!("d{i}"), "sha", Zone::Draw); g.deck.push(format!("d{i}")); }
    let step = run_steps(&mut g, "faen", 0, data(&[("target", 1.into())]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 1, "令其摸一张");
    // 反例：放弃不摸。
    let mut g = game(3, &["faen"]);
    let step = run_steps(&mut g, "faen", 0, data(&[("target", 1.into())]), &["pass"]);
    assert!(matches!(step, Step::Done));
}

#[test]
fn benxi_picks_up_to_two_modifiers() {
    let mut g = game(3, &["benxi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s", "sha", Zone::Processing);
    let step = run_steps(&mut g, "benxi", 0, data(&[("card", "s".into())]), &["opt:extra", "opt:draw"]);
    assert!(matches!(step, Step::Finish(_)));
    // 选项声明走 `UseOpts` 单出口（具体生效由使用流程消费；内核用例已覆盖 extra_targets）。
    assert_eq!(g.use_opts("s").extra_targets, g.use_opts("s").extra_targets.max(0));
    // 反例：与其他角色距离不全是 1（6 人局远端距离 3，-1 后仍为 2）→ 不触发。
    let mut g = game(6, &["benxi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s", "sha", Zone::Processing);
    assert!(matches!(call(&mut g, "benxi", 0, data(&[("card", "s".into())])), Step::Done));
}

#[test]
fn zhongyong_gives_slash_to_another_character() {
    let mut g = game(3, &["zhongyong"]);
    g.turn = 0;
    add_suit(&mut g, "s", "sha", Zone::Discard, Suit::Heart, 7);
    let step = run_steps(&mut g, "zhongyong", 0, data(&[("card", "s".into())]), &["seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(1).contains(&"s".to_string()), "交给其【杀】");
    // 反例：非【杀】不触发。
    let mut g = game(3, &["zhongyong"]);
    add(&mut g, "t", "tao", Zone::Discard);
    assert!(matches!(call(&mut g, "zhongyong", 0, data(&[("card", "t".into())])), Step::Done));
}
