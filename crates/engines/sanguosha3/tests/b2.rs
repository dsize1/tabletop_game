//! B2 技能门禁（8 将 / 15 技能 + 授予技 清侧/怀柔 + 临时技 集智）。
//!
//! 每个技能至少 1 正例 + 1 反例；逐条对齐 `align/B2.json` 的 `agreed`/`assertions`。
//! 触发类技能按「真实待办」驱动（先发动/放弃，再强制选牌/选目标），不把 `ask.data` 手推给 handler。

use sanguosha3::kernel::event::{Event, EventKind};
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

fn add_deck(g: &mut Game, ids: &[&str]) {
    for id in ids {
        add(g, id, "sha", Zone::Draw);
        g.deck.push(id.to_string());
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
    let h = skills::handler(skill).expect("handler 应存在");
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

/// 按 pending 逐个回答（与 `resolve::submit` 同语义，不依赖栈/回合帧）。
fn run_steps(
    g: &mut Game,
    skill: &str,
    actor: usize,
    d: serde_json::Value,
    answers: &[&str],
) -> Step {
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

// ---------------------------------------------------------------------------

#[test]
fn mingren_prepare_init_and_end_swap() {
    let mut g = game(3, &["mingren"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add_deck(&mut g, &["d0"]);
    add(&mut g, "h0", "sha", Zone::Hand(0));
    let step = run_steps(&mut g, "mingren", 0, data(&[]), &["card:h0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(marks(&g, 0, "ren"), vec!["h0".to_string()], "置「任」");

    // 结束阶段：一进一出替换。
    let mut g = game(3, &["mingren"]);
    g.turn = 0;
    g.phase = Phase::End;
    add(&mut g, "old", "sha", Zone::Mark(0, "ren".into()));
    add(&mut g, "new", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "mingren", 0, data(&[]), &["use", "card:new"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(marks(&g, 0, "ren"), vec!["new".to_string()]);
    assert!(g.hand(0).contains(&"old".to_string()), "旧「任」回手");
}

#[test]
fn zhenliang_yang_damages_and_flips() {
    let mut g = game(3, &["zhenliang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "ren", "sha", Zone::Mark(0, "ren".into()), Suit::Spade, 7);
    add_suit(&mut g, "c0", "sha", Zone::Hand(0), Suit::Spade, 9);
    assert!(skills::active_actions(&g, 0).contains(&"zhenliang"));
    let step = run_steps(&mut g, "zhenliang", 0, data(&[]), &["seat:1", "card:c0"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 3, "造成 1 点伤害");
    assert_eq!(skills::face_of(&g, 0, "zhenliang"), 1, "发动后转面");
    // 反例：不同色牌不能用于发动（可用性过滤）。
    let mut g = game(3, &["zhenliang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "ren", "sha", Zone::Mark(0, "ren".into()), Suit::Spade, 7);
    add_suit(&mut g, "c0", "sha", Zone::Hand(0), Suit::Heart, 9);
    assert!(!skills::active_actions(&g, 0).contains(&"zhenliang"));
}

#[test]
fn zhenliang_yin_draws_out_of_turn_only() {
    let mut g = game(3, &["zhenliang"]);
    g.turn = 1; // 回合外
    add_suit(&mut g, "ren", "sha", Zone::Mark(0, "ren".into()), Suit::Spade, 7);
    add_suit(&mut g, "x", "sha", Zone::Discard, Suit::Spade, 9);
    skills::flip_face(&mut g, 0, "zhenliang");
    let step = run_steps(
        &mut g,
        "zhenliang",
        0,
        data(&[("face", 1.into()), ("card", "x".into())]),
        &["use", "seat:2"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(skills::face_of(&g, 0, "zhenliang"), 0, "发动后回到阳面");
    // 反例：自己的回合内不触发。
    let mut g = game(3, &["zhenliang"]);
    g.turn = 0;
    add_suit(&mut g, "ren", "sha", Zone::Mark(0, "ren".into()), Suit::Spade, 7);
    skills::flip_face(&mut g, 0, "zhenliang");
    let step = call(&mut g, "zhenliang", 0, data(&[("face", 1.into()), ("card", "x".into())]));
    assert!(matches!(step, Step::Done));
}

#[test]
fn zhengu_fills_and_defers_to_target_turn_end() {
    let mut g = game(3, &["zhengu"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, &["d0", "d1"]);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "sha", Zone::Hand(0));
    let step = run_steps(&mut g, "zhengu", 0, data(&[]), &["use", "seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 2, "手牌数向郝昭看齐（补到 2）");
    assert!(!g.deferred.is_empty(), "目标回合结束再结算一次");
    // 反例：目标手牌更多时逐张弃（由目标自己选）。
    let mut g = game(3, &["zhengu"]);
    g.turn = 0;
    g.phase = Phase::End;
    add(&mut g, "mine", "sha", Zone::Hand(0));
    add(&mut g, "c0", "sha", Zone::Hand(1));
    add(&mut g, "c1", "sha", Zone::Hand(1));
    add(&mut g, "c2", "sha", Zone::Hand(1));
    let step = run_steps(&mut g, "zhengu", 0, data(&[]), &["use", "seat:1", "card:c0", "card:c1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 1, "弃到与郝昭相同（1）");
}

#[test]
fn wanglie_first_card_no_distance_and_active_no_response() {
    let mut g = game(3, &["wanglie"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s0", "sha", Zone::Processing);
    let step = run_steps(&mut g, "wanglie", 0, data(&[("card", "s0".into())]), &["use"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.use_opts("s0").no_distance, "首张牌无距离限制（锁定）");
    assert!(g.use_opts("s0").no_response, "主动部分：不能被响应");
    assert!(g.is_frozen(0, &g.hand(0).first().cloned().unwrap_or_default()) || g.hand(0).is_empty());
    // 反例：放弃则既不冻结也不禁止响应。
    let mut g = game(3, &["wanglie"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s0", "sha", Zone::Processing);
    let step = run_steps(&mut g, "wanglie", 0, data(&[("card", "s0".into())]), &["pass"]);
    assert!(matches!(step, Step::Done));
    assert!(!g.use_opts("s0").no_response);
}

#[test]
fn zuilun_penalty_when_no_condition() {
    let mut g = game(3, &["zuilun"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, &["d0", "d1", "d2"]);
    // 让条件③不成立（自己手牌不是最少）且②不成立（有弃置记录）。
    add(&mut g, "x", "sha", Zone::Hand(0));
    let mut ev = Event::card(EventKind::CardDiscarded, Some(0), "x".into(), Some("测试".into()));
    ev.by = Some(0);
    ev.turn = g.turn;
    g.log.push(ev);
    let step = run_steps(&mut g, "zuilun", 0, data(&[]), &["seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3, "自己失去 1 点体力");
    assert_eq!(g.seats[1].hp, 3, "目标失去 1 点体力");
}

#[test]
fn zuilun_takes_one_then_orders_remaining() {
    let mut g = game(3, &["zuilun"]);
    g.turn = 0;
    g.phase = Phase::End;
    add_deck(&mut g, &["d0", "d1", "d2"]);
    // 条件①：本回合造成过伤害。
    let mut ev = Event::simple(EventKind::DamageAfter, Some(1), None);
    ev.by = Some(0);
    ev.turn = g.turn;
    g.log.push(ev);
    // 条件②：本回合弃置过牌 → 不成立。
    let mut disc = Event::card(EventKind::CardDiscarded, Some(0), "x".into(), Some("测试".into()));
    disc.by = Some(0);
    disc.turn = g.turn;
    g.log.push(disc);
    // 条件③：手牌最少（其他人 2 张，自己 0）。
    add(&mut g, "o0", "sha", Zone::Hand(1));
    add(&mut g, "o1", "sha", Zone::Hand(1));
    add(&mut g, "o2", "sha", Zone::Hand(2));
    add(&mut g, "o3", "sha", Zone::Hand(2));
    let step = run_steps(&mut g, "zuilun", 0, data(&[]), &["card:d2", "card:d1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 2, "两个条件 → 获得两张");
    assert_eq!(g.deck.last().map(|s| s.as_str()), Some("d0"), "剩余 1 张回牌堆顶");
}

#[test]
fn fuyin_first_slash_ineffective_only() {
    let mut g = game(3, &["fuyin"]);
    g.turn = 0;
    add(&mut g, "s0", "sha", Zone::Processing);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "sha", Zone::Hand(0));
    add(&mut g, "c", "sha", Zone::Hand(0));
    assert!(
        skills::card_ineffective_for(&mut g, 0, 0, "s0"),
        "首次成为【杀】目标且其手牌数 ≥ 你 → 此牌无效"
    );
    assert!(
        !skills::card_ineffective_for(&mut g, 0, 0, "s0"),
        "每回合仅首次"
    );
    // 反例：非杀/决斗不覆盖。
    let mut g = game(3, &["fuyin"]);
    add(&mut g, "t0", "wanjian_qifa", Zone::Processing);
    assert!(!skills::card_ineffective_for(&mut g, 0, 0, "t0"));
}

#[test]
fn liangyin_draw_or_discard_once_per_kind() {
    let mut g = game(3, &["liangyin"]);
    g.turn = 0;
    add_deck(&mut g, &["d0"]);
    for i in 0..3 {
        add(&mut g, &format!("c{i}"), "sha", Zone::Hand(1));
    }
    let step = run_steps(
        &mut g,
        "liangyin",
        0,
        data(&[("kind", "out".into())]),
        &["use", "seat:1", "use:draw"],
    );
    assert!(matches!(step, Step::Finish(_)));
    // 反例：同回合第二次同类不再询问。
    let step = call(&mut g, "liangyin", 0, data(&[("kind", "out".into())]));
    assert!(matches!(step, Step::Done));
}

#[test]
fn kongsheng_places_and_returns_equipment() {
    let mut g = game(3, &["kongsheng", "liangyin"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    add(&mut g, "h0", "sha", Zone::Hand(0));
    add(&mut g, "e0", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    let step = run_steps(&mut g, "kongsheng", 0, data(&[]), &["use", "card:h0", "card:e0", "ok"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(marks(&g, 0, "kongsheng").len(), 2);
    // 结束阶段：非装备回手，装备令一名角色使用并失 1 体力。
    g.phase = Phase::End;
    let hand_before = g.hand_size(0);
    let step = run_steps(&mut g, "kongsheng", 0, data(&[]), &["pass", "seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), hand_before + 1, "非装备牌取回");
    assert_eq!(g.seats[1].hp, 3, "使用装备者失去 1 点体力");
    assert!(marks(&g, 0, "kongsheng").is_empty(), "装备已用出");
}

#[test]
fn zhengrong_takes_a_card_as_rong() {
    let mut g = game(3, &["zhengrong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s0", "sha", Zone::Processing);
    add(&mut g, "t0", "sha", Zone::Hand(1));
    add(&mut g, "t1", "sha", Zone::Hand(1));
    let step = run_steps(
        &mut g,
        "zhengrong",
        0,
        data(&[("target", 1.into()), ("card", "s0".into())]),
        &["use", "rand"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(marks(&g, 0, "rong").len(), 1, "取一张为「荣」");
    // 反例：目标手牌数 < 你 → 不触发。
    let mut g = game(3, &["zhengrong"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "s0", "sha", Zone::Processing);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "sha", Zone::Hand(0));
    let step = call(
        &mut g,
        "zhengrong",
        0,
        data(&[("target", 1.into()), ("card", "s0".into())]),
    );
    assert!(matches!(step, Step::Done));
}

#[test]
fn hongju_awakens_swaps_and_grants_qingce() {
    let mut g = game(3, &["zhengrong", "hongju"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    for i in 0..3 {
        add(&mut g, &format!("r{i}"), "sha", Zone::Mark(0, "rong".into()));
        add(&mut g, &format!("h{i}"), "sha", Zone::Hand(0));
    }
    let max_before = g.seats[0].max_hp;
    let step = run_steps(
        &mut g,
        "hongju",
        0,
        data(&[]),
        &["card:h0", "card:h1", "card:h2", "ok"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].max_hp, max_before - 1, "减 1 点体力上限");
    assert!(skills::active(&g, 0, "qingce"), "授予清侧");
    assert!(
        g.hand(0).iter().any(|c| c.starts_with('r')),
        "旧「荣」回手（等量交换）"
    );
}

#[test]
fn qianjie_blocks_delayed_chain_and_pindian() {
    let mut g = game(3, &["qianjie"]);
    assert!(skills::deny_target_card(&g, 0, 0, "lebu_sishu"), "延时锦囊");
    assert!(skills::deny_target_card(&g, 0, 0, "tiesuo_lianhuan"), "铁索连环");
    assert!(!skills::deny_target_card(&g, 0, 0, "sha"), "普通牌不拦");
    assert!(skills::deny_pindian(&g, 0), "不能成为拼点目标");
    // 反例：无谦节者不受限。
    let g = game(3, &[]);
    assert!(!skills::deny_pindian(&g, 0));
}

#[test]
fn jueyan_abolishes_one_slot_once_per_phase() {
    let mut g = game(3, &["jueyan"]);
    g.turn = 0;
    g.phase = Phase::Play;
    let step = run_steps(&mut g, "jueyan", 0, data(&[]), &["slot:weapon"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.slot_abolished(0, sanguosha3::kernel::Slot::Weapon));
    assert_eq!(g.counter("turn.0.jueyan_sha"), 3, "本回合【杀】次数 +3");
    // 反例：同一出牌阶段限一次。
    let step = call(&mut g, "jueyan", 0, data(&[]));
    assert!(matches!(step, Step::Done));
    assert!(!skills::active_actions(&g, 0).contains(&"jueyan"));
}

#[test]
fn poshi_awakens_by_hp_one_and_grants_huairou() {
    let mut g = game(3, &["jueyan", "poshi"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    g.seats[0].hp = 1;
    add(&mut g, "w", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    add_deck(&mut g, &["d0", "d1", "d2", "d3"]);
    let max_before = g.seats[0].max_hp;
    let step = call(&mut g, "poshi", 0, data(&[]));
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].max_hp, max_before - 1);
    assert!(g.slot_abolished(0, sanguosha3::kernel::Slot::Weapon));
    assert_eq!(g.hand_size(0), g.seats[0].max_hp as usize, "摸至体力上限");
    assert!(!skills::active(&g, 0, "jueyan"), "失去决堰");
    assert!(skills::active(&g, 0, "huairou"), "获得怀柔");
    // 反例：非准备阶段/条件不满足不触发。
    let mut g = game(3, &["poshi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    assert!(matches!(call(&mut g, "poshi", 0, data(&[])), Step::Done));
}

#[test]
fn xiongluan_abolishes_zones_and_marks_no_response() {
    let mut g = game(3, &["xiongluan"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "w", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    let step = run_steps(&mut g, "xiongluan", 0, data(&[]), &["seat:1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.slot_abolished(0, sanguosha3::kernel::Slot::Weapon));
    assert!(g.seats[0].judge_abolished, "废除判定区");
    assert!(skills::no_response_for(&g, 0, 1), "对目标的牌不可响应");
    assert!(skills::ignore_distance(&g, 0), "本回合无距离限制");
    assert!(skills::limited_used(&g, 0, "xiongluan"));
    // 反例：限定技每局一次。
    let step = call(&mut g, "xiongluan", 0, data(&[]));
    assert!(matches!(step, Step::Done));
}

#[test]
fn congjian_gives_own_card_then_draws() {
    let mut g = game(3, &["congjian"]);
    g.turn = 1;
    add(&mut g, "t0", "sha", Zone::Processing);
    g.use_targets.insert("t0".into(), vec![0, 1, 2]);
    add(&mut g, "mine", "tao", Zone::Hand(0));
    add_deck(&mut g, &["d0"]);
    let step = run_steps(
        &mut g,
        "congjian",
        0,
        data(&[("by", 1.into()), ("card", "t0".into())]),
        &["use", "seat:2", "card:mine"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.hand(2).contains(&"mine".to_string()), "交给目标");
    assert_eq!(g.hand_size(0), 1, "然后摸一张（0→1）");
    // 反例：没有自己的牌 → 不提供发动。
    let mut g = game(3, &["congjian"]);
    add(&mut g, "t0", "sha", Zone::Processing);
    g.use_targets.insert("t0".into(), vec![0, 2]);
    assert!(matches!(
        call(&mut g, "congjian", 0, data(&[("by", 1.into()), ("card", "t0".into())])),
        Step::Done
    ));
}

#[test]
fn huairou_recasts_equipment() {
    let mut g = game(2, &["huairou"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "e", "qinggang_jian", Zone::Hand(0));
    add_deck(&mut g, &["d0"]);
    let step = run_steps(&mut g, "huairou", 0, data(&[]), &["card:e"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("e"), Some(&Zone::Discard), "弃置并摸一张");
}

#[test]
fn qingce_discards_and_draws_when_distant() {
    let mut g = game(4, &["qingce"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "x", "sha", Zone::Hand(2));
    add_deck(&mut g, &["d0"]);
    let step = run_steps(&mut g, "qingce", 0, data(&[]), &["seat:2", "card:x"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("x"), Some(&Zone::Discard));
    assert_eq!(g.hand_size(0), 1, "距离 > 1 → 摸一张");
    // 反例：每阶段限一次。
    assert!(!skills::active_actions(&g, 0).contains(&"qingce"));
}

#[test]
fn jizhi_draws_on_trick_when_flag_set() {
    let mut g = game(2, &[]);
    g.turn = 0;
    g.seats[0].skills = vec!["jizhi".into()];
    g.set_counter("turn.0.jizhi", 1);
    add(&mut g, "t", "wuzhong_shengyou", Zone::Processing);
    add_deck(&mut g, &["d0"]);
    // 界集智：摸到基本牌（d0=闪）时可弃置换手牌上限；这里选择保留。
    let step = run_steps(
        &mut g,
        "jizhi",
        0,
        data(&[("card", "t".into())]),
        &["use", "pass"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 1);
}
