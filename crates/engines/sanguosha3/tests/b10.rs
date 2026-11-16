//! B10 技能门禁（界标·吴：8 将 / 17 技能）。

use sanguosha3::kernel::resolve::{self, Ask, Stage, Step};
use sanguosha3::kernel::skills::b10;
use sanguosha3::kernel::{flow, gear, skills, turn, Card, Game, Identity, Phase, Seat, Slot, Suit, Zone};

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
    let mut seats: Vec<Seat> = (0..n).map(|i| seat_with(i, &[], "wu")).collect();
    seats[0] = seat_with(0, s0, "wu");
    Game { seats, ..Default::default() }
}

fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank },
    );
}

fn add(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add_suit(g, id, def, zone, Suit::Club, 7);
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        add(g, &id, "shan", Zone::Draw);
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
    h(g, &Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data: d })
        .expect("不应报错")
}

fn pending_of(step: &Step) -> &sanguosha3::kernel::pending::Pending {
    match step {
        Step::Need(p) | Step::NeedAt(_, p) | Step::NeedWith(_, _, p) => p,
        _ => panic!("期望待办，实际 {step:?}"),
    }
}

fn run_steps(g: &mut Game, skill: &str, actor: usize, d: serde_json::Value, answers: &[&str]) -> Step {
    let mut data = d;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        loop {
            match &step {
                Step::Next(_, next) => {
                    data = next.clone();
                    step = call(g, skill, actor, data.clone());
                }
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::NeedWith(_, next, _) => {
                    data = next.clone();
                    break;
                }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let p = pending_of(&step);
        assert!(
            p.has_option(ans),
            "选项 {ans} 不在 {:?}",
            p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
        );
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        step = call(g, skill, actor, data.clone());
    }
    step
}

fn drain(g: &mut Game, skill: &str, actor: usize, step: Step) -> Step {
    let mut step = step;
    loop {
        match step {
            Step::Next(_, data) => step = call(g, skill, actor, data),
            other => return other,
        }
    }
}

// ---------------------------------------------------------------------------
// 奇袭 / 奋威
// ---------------------------------------------------------------------------

#[test]
fn qixi_black_card_as_dismantle() {
    let mut g = game(2, &["qixi"]);
    add_suit(&mut g, "b", "sha", Zone::Hand(0), Suit::Spade, 7);
    add_suit(&mut g, "r", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "e", "qinggang_jian", Zone::Equip(0, Slot::Weapon), Suit::Club, 7);
    assert_eq!(skills::use_alias(&g, 0, "b"), Some("guohe_chaiqiao"));
    assert_eq!(skills::use_alias(&g, 0, "e"), Some("guohe_chaiqiao"), "装备区黑牌也可");
    assert_eq!(skills::use_alias(&g, 0, "r"), None, "红牌不能奇袭");
}

#[test]
fn fenwei_invalidates_chosen_targets() {
    let mut g = game(3, &["fenwei"]);
    let step = run_steps(
        &mut g,
        "fenwei",
        0,
        data(&[("step", "ask".into()), ("card", "c".into()), ("targets", serde_json::json!([1, 2]))]),
        &["use", "t:1", "pass"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(b10::take_multi_invalid(&mut g, "c"), Some(vec![1]));
    assert!(skills::limited_used(&g, 0, "fenwei"), "限定技已使用");
    // 反例：用过的奋威不再询问。
    assert!(!b10::fire_multi_target(&mut g, 1, &[0, 2], "c2"));
}

// ---------------------------------------------------------------------------
// 克己 / 勤学 / 博图
// ---------------------------------------------------------------------------

#[test]
fn keji_skips_discard_without_sha() {
    let mut g = game(2, &["keji"]);
    assert!(b10::skip_discard(&g, 0), "未使用【杀】→ 跳过弃牌阶段");
    g.bump("turn.0.sha_played");
    assert!(!b10::skip_discard(&g, 0));
}

#[test]
fn qinxue_awaken_reduces_max_hp_and_draws() {
    let mut g = game(2, &["qinxue"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    for i in 0..6 {
        add(&mut g, &format!("h{i}"), "sha", Zone::Hand(0));
    }
    add_deck(&mut g, 5);
    let step = run_steps(&mut g, "qinxue", 0, data(&[]), &["opt2"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].max_hp, 3, "减 1 点体力上限");
    assert_eq!(g.hand_size(0), 8, "摸两张");
    // 反例：已觉醒不再询问。
    assert!(matches!(call(&mut g, "qinxue", 0, data(&[])), Step::Done));
}

#[test]
fn botu_grants_extra_turn_with_four_suits() {
    let mut g = game(2, &["botu"]);
    g.turn = 0;
    g.round = 1;
    g.set_mark("turn.0.discard_suits", "Spade,Heart,Club,Diamond");
    b10::on_turn_end(&mut g, 0);
    assert!(g.extra_turns.contains(&0), "四花色 → 额外回合");
    assert_eq!(g.counter("botu.extra.0"), 1);
    // 额外回合结束不再触发。
    g.set_mark("turn.0.discard_suits", "Spade,Heart,Club,Diamond");
    b10::on_turn_end(&mut g, 0);
    assert_eq!(g.extra_turns.len(), 1, "额外回合内不再触发");
}

// ---------------------------------------------------------------------------
// 苦肉 / 诈降 / 英姿
// ---------------------------------------------------------------------------

#[test]
fn kurou_discards_and_loses_hp() {
    let mut g = game(2, &["kurou"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add(&mut g, "e", "qinggang_jian", Zone::Equip(0, Slot::Weapon));
    let step = run_steps(&mut g, "kurou", 0, data(&[]), &["card:e"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[0].hp, 3, "失去 1 点体力");
    assert!(gear::equip_in(&g, 0, Slot::Weapon).is_none(), "弃置装备区的牌");
    assert_eq!(g.counter("phase.0.kurou"), 1);
}

#[test]
fn zhaxiang_draws_three_and_boosts_sha() {
    let mut g = game(2, &["zhaxiang"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 5);
    flow::lose_hp(&mut g, 0, 1, "苦肉");
    let step = call(&mut g, "zhaxiang", 0, data(&[("victim", 0.into()), ("amount", 1.into())]));
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 3, "摸三张");
    assert_eq!(b10::sha_extra(&g, 0), 1, "【杀】次数 +1");
    add_suit(&mut g, "rs", "sha", Zone::Processing, Suit::Heart, 7);
    assert!(b10::no_real_shan(&g, 0, "rs"), "红【杀】不能被真实【闪】抵消");
    b10::on_use_start(&mut g, 0, "rs");
    assert!(g.use_opts("rs").no_distance, "红【杀】无距离");
}

#[test]
fn yingzi_draws_extra_and_hand_limit_equals_max_hp() {
    let mut g = game(2, &["yingzi"]);
    g.seats[0].hp = 1;
    assert_eq!(skills::draw_bonus(&mut g, 0), 1);
    assert_eq!(g.hand_limit(0), 4, "手牌上限 = 体力上限（覆盖体力值）");
}

// ---------------------------------------------------------------------------
// 反间 / 流离 / 国色
// ---------------------------------------------------------------------------

#[test]
fn fanjian_gives_card_and_target_chooses() {
    let mut g = game(2, &["fanjian"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "h", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g, "t1", "tao", Zone::Hand(1), Suit::Heart, 3);
    add_suit(&mut g, "t2", "shan", Zone::Hand(1), Suit::Spade, 4);
    let step = run_steps(&mut g, "fanjian", 0, data(&[]), &["t:1", "card:h", "opt1"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(1), 1, "同花色（含刚交给的那张）全部弃置");
    // 反例：opt2 失去体力。
    let mut g2 = game(2, &["fanjian"]);
    g2.turn = 0;
    g2.phase = Phase::Play;
    add_suit(&mut g2, "h", "sha", Zone::Hand(0), Suit::Heart, 7);
    add_suit(&mut g2, "t1", "tao", Zone::Hand(1), Suit::Spade, 3);
    let step = run_steps(&mut g2, "fanjian", 0, data(&[]), &["t:1", "card:h", "opt2"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g2.seats[1].hp, 3);
}

#[test]
fn liuli_transfers_slash_by_discarding() {
    let mut g = game(3, &["liuli"]);
    add(&mut g, "c", "tao", Zone::Hand(0));
    add(&mut g, "s", "sha", Zone::Processing);
    let step = run_steps(
        &mut g,
        "liuli",
        0,
        data(&[("by", 1.into()), ("card", "s".into())]),
        &["use", "card:c", "t:2"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(
        g.use_opts("s").intercept,
        Some(sanguosha3::kernel::ShaIntercept::Replace(2))
    );
}

#[test]
fn guose_option1_uses_indulgence_and_draws() {
    let mut g = game(2, &["guose"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 3);
    add_suit(&mut g, "d", "sha", Zone::Hand(0), Suit::Diamond, 7);
    let step = run_steps(&mut g, "guose", 0, data(&[]), &["opt1", "card:d"]);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 1, "摸一张（承载牌进处理区）");
    assert_eq!(g.counter("phase.0.guose"), 1);
}

#[test]
fn guose_option2_discards_first_indulgence() {
    let mut g = game(2, &["guose"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_suit(&mut g, "d", "sha", Zone::Hand(0), Suit::Diamond, 7);
    add(&mut g, "le", "lebu_sishu", Zone::Judge(1));
    g.judge_stacks.entry(1).or_default().push("le".into());
    let step = run_steps(&mut g, "guose", 0, data(&[]), &["opt2", "card:d"]);
    assert!(matches!(step, Step::Finish(_)));
    assert!(g.judge_stacks.get(&1).map(|s| s.is_empty()).unwrap_or(true), "弃置场上的乐");
}

// ---------------------------------------------------------------------------
// 谦逊 / 连营
// ---------------------------------------------------------------------------

#[test]
fn qianxun_moves_hand_out_and_returns_at_turn_end() {
    let mut g = game(2, &["qianxun"]);
    g.turn = 1;
    add(&mut g, "c", "guohe_chaiqiao", Zone::Processing);
    g.use_targets.insert("c".into(), vec![0]);
    add(&mut g, "h", "sha", Zone::Hand(0));
    let step = run_steps(
        &mut g,
        "qianxun",
        0,
        data(&[("card", "c".into()), ("by", 1.into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 0, "手牌移出游戏");
    b10::on_turn_end(&mut g, 0);
    assert_eq!(g.hand_size(0), 1, "回合结束归还手牌");
}

#[test]
fn lianying_distributes_draws_after_hand_emptied() {
    let mut g = game(3, &["lianying"]);
    add_deck(&mut g, 5);
    b10::on_hand_empty(&mut g, 0, 2);
    resolve::advance(&mut g).expect("压入连营询问");
    assert!(g.pending.is_some());
    resolve::submit(&mut g, 0, "t:1").expect("令其摸牌");
    resolve::submit(&mut g, 0, "t:2").expect("再令其摸牌");
    assert_eq!(g.hand_size(1), 1);
    assert_eq!(g.hand_size(2), 1);
}

// ---------------------------------------------------------------------------
// 制衡 / 救援 / 枭姬 / 结姻
// ---------------------------------------------------------------------------

#[test]
fn zhiheng_discards_and_draws_equal_plus_bonus() {
    let mut g = game(2, &["zhiheng"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, 5);
    add(&mut g, "a", "sha", Zone::Hand(0));
    add(&mut g, "b", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "zhiheng", 0, data(&[]), &["card:a", "card:b"]);
    let step = drain(&mut g, "zhiheng", 0, step);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 3, "弃 2 摸 2 + 手牌空额外摸 1");
    assert_eq!(g.counter("phase.0.zhiheng"), 1);
}

#[test]
fn jiuyuan_redirects_heal_to_lord() {
    let mut g = game(2, &["jiuyuan"]);
    g.seats[0].identity = Some(Identity::Lord);
    g.seats[0].skills = vec!["jiuyuan".to_string()];
    g.seats[1].kingdom = "wu".into();
    g.seats[1].hp = 3;
    g.seats[0].hp = 3;
    g.turn = 1;
    add_deck(&mut g, 3);
    flow::heal(&mut g, 1, 1, "桃");
    assert_eq!(g.seats[0].hp, 4, "改为令主公回复");
    assert_eq!(g.seats[1].hp, 3, "其自身未回复");
    assert_eq!(g.hand_size(1), 1, "其摸一张");
    // 反例：非吴势力不触发。
    let mut g2 = game(2, &["jiuyuan"]);
    g2.seats[0].identity = Some(Identity::Lord);
    g2.seats[1].kingdom = "wei".into();
    g2.seats[1].hp = 3;
    g2.seats[0].hp = 3;
    g2.turn = 1;
    flow::heal(&mut g2, 1, 1, "桃");
    assert_eq!(g2.seats[1].hp, 4);
}

#[test]
fn xiaoji_draws_on_equip_lost() {
    let mut g = game(2, &["xiaoji"]);
    add_deck(&mut g, 5);
    add(&mut g, "e", "qinggang_jian", Zone::Equip(0, Slot::Weapon));
    flow::discard(&mut g, 1, "e", "拆").expect("弃置装备");
    resolve::advance(&mut g).expect("枭姬询问");
    resolve::submit(&mut g, 0, "use").expect("发动枭姬");
    assert_eq!(g.hand_size(0), 2);
}

#[test]
fn jieyin_compares_hp_after_payment() {
    let mut g = game(2, &["jieyin"]);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 4;
    g.seats[1].hp = 2;
    g.seats[1].male = true;
    add_deck(&mut g, 5);
    add(&mut g, "h", "tao", Zone::Hand(0));
    let step = run_steps(&mut g, "jieyin", 0, data(&[]), &["t:1", "hand:h"]);
    let step = drain(&mut g, "jieyin", 0, step);
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.hand_size(0), 1, "体力较大者（孙尚香）摸一张");
    assert_eq!(g.seats[1].hp, 3, "体力较小者回复 1 点");
    assert_eq!(g.counter("phase.0.jieyin"), 1);
}
