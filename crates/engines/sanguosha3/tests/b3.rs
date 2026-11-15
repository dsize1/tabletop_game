//! B3 技能门禁（5 将 / 9 技能 + 授予技 排异）。

use sanguosha3::kernel::resolve::{Ask, Stage, Step};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::{Card, Game, Phase, Seat, Suit, Zone};

fn seat_with(id: usize, skill_list: &[&str], kingdom: &str) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: skill_list.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
}

fn game(n: usize, skills0: &[&str]) -> Game {
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "wei")).collect();
    seats[0] = seat_with(0, skills0, "wei");
    Game {
        seats,
        ..Default::default()
    }
}

fn add(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add_suit(g, id, def, zone, Suit::Spade, 7);
}

fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card {
            id: id.to_string(),
            def: def.to_string(),
            suit,
            rank,
        },
    );
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        add(g, &id, "sha", Zone::Draw);
        g.deck.push(id);
    }
}

fn data(pairs: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    serde_json::Value::Object(m)
}

fn call(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler");
    h(
        g,
        &Ask {
            skill: skill.to_string(),
            stage: Stage::Choose,
            actor,
            data: d,
        },
    )
    .expect("handler 不应报错")
}

fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::Next(_, next) | Step::NeedWith(_, next, _) => {
                    data = next.clone();
                    step = call(g, skill, actor, data.clone());
                }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let pending = match &step {
            Step::Need(p) | Step::NeedAt(_, p) => p,
            _ => unreachable!(),
        };
        let opts: Vec<String> = pending.options().iter().map(|o| o.id.clone()).collect();
        assert!(pending.has_option(ans), "选项 {ans} 不在 {opts:?}");
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        step = call(g, skill, actor, data.clone());
    }
    step
}

fn marks(g: &Game, seat: usize, name: &str) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, n) if *s == seat && n == name))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

#[test]
fn sanyao_discards_then_damages_max_hp_target() {
    let mut g = game(3, &["sanyao"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[1].hp = 3;
    add(&mut g, "c0", "sha", Zone::Hand(0));
    let step = run_steps(&mut g, "sanyao", 0, data(&[]), &["mode:hp", "card:c0", "seat:0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3, "自己也是「体力最大」的合法目标");
    // 反例：取消不消耗次数。
    let mut g = game(3, &["sanyao"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "c0", "sha", Zone::Hand(0));
    let step = run_steps(&mut g, "sanyao", 0, data(&[]), &["pass"]);
    assert!(matches!(step, Step::Done));
    assert_eq!(g.counter("phase.0.sanyao_hp"), 0);
}

#[test]
fn zhiman_prevents_damage_and_steals_card() {
    let mut g = game(3, &["zhiman"]);
    g.turn = 0;
    add(&mut g, "x", "tao", Zone::Hand(1));
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Slash,
    };
    sanguosha3::kernel::flow::damage(&mut g, spec, sanguosha3::kernel::ArmorKind::None, 0);
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert_eq!(g.pending.as_ref().and_then(|p| p.skill()), Some("zhiman"));
    let step = run_steps(&mut g, "zhiman", 0, data(&[("target", 1.into()), ("amount", 1.into())]), &["use", "card:x"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 4, "伤害被防止");
    assert!(g.hand(0).contains(&"x".to_string()), "获得其一张牌");
    // 反例：放弃 → 伤害照常结算。
    let mut g = game(3, &["zhiman"]);
    g.turn = 0;
    add(&mut g, "x", "tao", Zone::Hand(1));
    let step = run_steps(
        &mut g,
        "zhiman",
        0,
        data(&[("target", 1.into()), ("amount", 1.into())]),
        &["pass"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 3, "未发动则不防伤");
}

#[test]
fn qice_uses_all_hand_as_trick() {
    let mut g = game(3, &["qice"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "shan", Zone::Hand(0));
    let step = run_steps(&mut g, "qice", 0, data(&[]), &["use:wuzhong_shengyou"]);
    assert!(matches!(step, Step::Done));
    assert!(g.hand(0).is_empty(), "全部手牌作为承载牌");
    assert!(
        g.card_defs.values().any(|c| c.id.starts_with("v:")),
        "生成虚拟牌"
    );
    // 反例：无手牌时不可发动。
    let mut g = game(3, &["qice"]);
    g.turn = 0;
    g.phase = Phase::Play;
    assert!(!skills::active_actions(&g, 0).contains(&"qice"));
}

#[test]
fn zhiyu_draws_and_reveals() {
    let mut g = game(3, &["zhiyu"]);
    g.turn = 0;
    add_deck(&mut g, 1);
    add_suit(&mut g, "r1", "sha", Zone::Hand(0), Suit::Heart, 7);
    let step = run_steps(
        &mut g,
        "zhiyu",
        0,
        data(&[("by", 1.into()), ("amount", 1.into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 2, "摸一张");
    assert!(g.log.iter().any(|e| e.kind == sanguosha3::kernel::event::EventKind::CardRevealed));
    // 反例：放弃不摸牌。
    let mut g = game(3, &["zhiyu"]);
    add_suit(&mut g, "r1", "sha", Zone::Hand(0), Suit::Heart, 7);
    let step = call(&mut g, "zhiyu", 0, data(&[("by", 1.into())]));
    let step = run_steps(&mut g, "zhiyu", 0, data(&[("by", 1.into())]), &["pass"]);
    assert!(matches!(step, Step::Done));
    assert_eq!(g.hand_size(0), 1);
    let _ = step;
}

#[test]
fn quanji_marks_per_damage_point_and_raises_hand_limit() {
    let mut g = game(3, &["quanji"]);
    g.turn = 0;
    add_deck(&mut g, 4);
    add(&mut g, "h0", "sha", Zone::Hand(0));
    add(&mut g, "h1", "sha", Zone::Hand(0));
    let limit_before = g.hand_limit(0);
    let step = run_steps(
        &mut g,
        "quanji",
        0,
        data(&[("amount", 2.into())]),
        &["use", "card:h0", "use", "card:h1"],
    );
    assert!(matches!(step, Step::Finish(_)) || matches!(step, Step::Done));
    assert_eq!(marks(&g, 0, "quan").len(), 2, "每点伤害各可发动一次");
    assert_eq!(g.hand_limit(0), limit_before + 2, "每张「权」手牌上限 +1");
}

#[test]
fn zili_awakens_and_grants_paiyi() {
    let mut g = game(3, &["quanji", "zili"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    for i in 0..3 {
        add(&mut g, &format!("q{i}"), "sha", Zone::Mark(0, "quan".into()));
    }
    g.seats[0].hp = 3;
    let max_before = g.seats[0].max_hp;
    let step = run_steps(&mut g, "zili", 0, data(&[]), &["use:draw"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].max_hp, max_before - 1, "减 1 点体力上限");
    assert!(skills::active(&g, 0, "paiyi"), "授予排异");
    // 反例：权不足 3 不触发。
    let mut g = game(3, &["zili"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add(&mut g, "q0", "sha", Zone::Mark(0, "quan".into()));
    assert!(matches!(call(&mut g, "zili", 0, data(&[])), Step::Done));
}

#[test]
fn junxing_forces_flip_or_discard() {
    let mut g = game(3, &["junxing"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "c0", "sha", Zone::Hand(0));
    add(&mut g, "c1", "sha", Zone::Hand(0));
    add_deck(&mut g, 2);
    let step = run_steps(&mut g, "junxing", 0, data(&[]), &["card:c0", "ok", "seat:1", "flip"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.seats[1].flipped, "选择翻面");
    assert_eq!(g.cards.get("c0"), Some(&Zone::Discard), "代价已支付");
    // 反例：无手牌不可发动。
    let mut g = game(3, &["junxing"]);
    g.turn = 0;
    g.phase = Phase::Play;
    assert!(!skills::active_actions(&g, 0).contains(&"junxing"));
}

#[test]
fn yuce_heals_when_source_has_no_hand() {
    let mut g = game(3, &["yuce"]);
    g.turn = 0;
    g.seats[0].hp = 3;
    add(&mut g, "m", "sha", Zone::Hand(0));
    let step = run_steps(
        &mut g,
        "yuce",
        0,
        data(&[("by", 1.into())]),
        &["card:m"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 4, "来源无手牌 → 直接回复 1 点");
    // 反例：来源有不同类型手牌时可以选择弃牌（由来源决定）。
    let mut g = game(3, &["yuce"]);
    g.seats[0].hp = 3;
    add_suit(&mut g, "m", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "s", "wuzhong_shengyou", Zone::Hand(1), Suit::Spade, 7);
    let step = run_steps(
        &mut g,
        "yuce",
        0,
        data(&[("by", 1.into())]),
        &["card:m", "discard", "card:s"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3, "来源弃牌 → 不回血");
    assert_eq!(g.cards.get("s"), Some(&Zone::Discard));
}

#[test]
fn longyin_uncounts_slash_and_draws_on_red() {
    let mut g = game(3, &["longyin"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "s0", "sha", Zone::Processing, Suit::Heart, 7);
    g.set_counter("phase.sha_used.0", 1);
    add(&mut g, "c0", "tao", Zone::Hand(0));
    add_deck(&mut g, 1);
    let step = run_steps(
        &mut g,
        "longyin",
        0,
        data(&[("by", 0.into()), ("card", "s0".into())]),
        &["use", "card:c0"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.counter("phase.sha_used.0"), 0, "不计入本阶段次数");
    assert_eq!(g.hand_size(0), 1, "红色【杀】→ 摸一张");
    // 反例：无牌可弃 → 不提供发动。
    let mut g = game(3, &["longyin"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s0", "sha", Zone::Processing);
    assert!(matches!(
        call(&mut g, "longyin", 0, data(&[("by", 0.into()), ("card", "s0".into())])),
        Step::Done
    ));
}

#[test]
fn paiyi_removes_quan_draws_two_and_damages_if_ahead() {
    let mut g = game(3, &["paiyi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "q0", "sha", Zone::Mark(0, "quan".into()));
    add_deck(&mut g, 2);
    add(&mut g, "h0", "sha", Zone::Hand(0));
    let step = run_steps(&mut g, "paiyi", 0, data(&[]), &["seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(marks(&g, 0, "quan").is_empty(), "移去一张「权」");
    assert_eq!(g.hand_size(1), 2, "目标摸两张");
    assert_eq!(g.seats[1].hp, 3, "其手牌数大于你 → 造成 1 点伤害");
    // 反例：没有「权」不可发动。
    let mut g = game(3, &["paiyi"]);
    g.phase = Phase::Play;
    assert!(!skills::active_actions(&g, 0).contains(&"paiyi"));
}
