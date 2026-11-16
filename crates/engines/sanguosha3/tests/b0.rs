//! M4.2 B0 handler 门禁：据守/奋激/行殇/放逐/颂威/不屈/解围 + handler 分发。

use sanguosha3::kernel::resolve::{Ask, Stage, Step};
use sanguosha3::kernel::skills::{self, active};
use sanguosha3::kernel::{Card, Game, Seat, Suit, Zone};

fn seat(id: usize, skills: Vec<&str>) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        granted: vec![],
        skills: skills.into_iter().map(String::from).collect(),
        identity: None,
        flipped: false, ..Default::default()
    }
}

fn game() -> Game {
    Game {
        seats: vec![seat(0, vec!["jushou", "fenji", "xingshang", "fangzhu", "songwei"]), seat(1, vec![]), seat(2, vec![])],
        ..Default::default()
    }
}

fn ask(skill: &str, actor: usize, data: serde_json::Value) -> Ask {
    Ask { skill: skill.to_string(), stage: Stage::Choose, actor, data }
}

// ---------------------------------------------------------------------------
// handler 分发
// ---------------------------------------------------------------------------

#[test]
fn handler_dispatches_b0_skills() {
    for c in ["jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei"] {
        assert!(skills::handler(c).is_some(), "{c} 应有 handler");
    }
    assert!(skills::handler("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// 据守：翻面 + 摸 4
// ---------------------------------------------------------------------------

#[test]
fn jushou_asks_choice_first() {
    let mut g = game();
    for i in 0..4 {
        let id = format!("d{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
        g.deck.push(id);
    }
    let h = skills::handler("jushou").unwrap();
    let step = h(&mut g, &ask("jushou", 0, serde_json::json!({}))).unwrap();
    // 据守是「结束阶段触发 + 先询问」（对齐 B0.json：`choose:jushou` / `pass`）。
    assert!(
        matches!(step, Step::NeedAt(_, _) | Step::Need(_)),
        "据守先询问是否发动：{step:?}"
    );
    assert!(!g.seats[0].flipped, "询问阶段尚未翻面");
    // 完整流程（翻面/摸 4/弃 1/上限）见 `tests/b0_real.rs`。
}

// ---------------------------------------------------------------------------
// 奋激：失去 1 体力令失牌角色摸 2
// ---------------------------------------------------------------------------

#[test]
fn fenji_asks_then_loses_hp_and_draws_for_victim() {
    let mut g = game();
    for i in 0..2 {
        let id = format!("d{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
        g.deck.push(id);
    }
    let h = skills::handler("fenji").unwrap();
    let ask0 = ask("fenji", 0, serde_json::json!({"step": "ask", "victim": 1}));
    // 先询问（`choose:fenji` / `pass`），不再自动发动。
    let step = h(&mut g, &ask0).unwrap();
    assert!(matches!(step, Step::NeedAt(_, _) | Step::Need(_)), "奋激先询问");
    assert_eq!(g.seats[0].hp, 4, "询问阶段尚未失去体力");
    // 模拟玩家选择发动。
    g.answer = Some("choose:fenji".into());
    g.answer_skill = Some("fenji".into());
    h(&mut g, &ask0).unwrap();
    assert_eq!(g.seats[0].hp, 3, "奋激失去 1 体力");
    let hand = g.cards.iter().filter(|(_, z)| **z == Zone::Hand(1)).count();
    assert_eq!(hand, 2, "失牌角色摸 2");
}

// ---------------------------------------------------------------------------
// 行殇：获得死亡角色手牌 + 装备（不含判定区）
// ---------------------------------------------------------------------------

#[test]
fn xingshang_gains_hand_and_equip_but_not_judge() {
    let mut g = game();
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.cards.insert("e1".into(), Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    g.cards.insert("j1".into(), Zone::Judge(1));
    g.judge_stacks.entry(1).or_default().push("j1".into());
    for id in ["h1", "e1", "j1"] {
        g.card_defs.insert(id.into(), Card { id: id.into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    }
    g.seats[1].alive = false;
    let h = skills::handler("xingshang").unwrap();
    let ask0 = ask("xingshang", 0, serde_json::json!({"step": "ask", "victim": 1}));
    let step = h(&mut g, &ask0).unwrap();
    assert!(matches!(step, Step::NeedAt(_, _) | Step::Need(_)), "行殇先询问");
    g.answer = Some("choose:xingshang".into());
    g.answer_skill = Some("xingshang".into());
    h(&mut g, &ask0).unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)), "手牌被获得");
    assert_eq!(g.cards.get("e1"), Some(&Zone::Hand(0)), "装备被获得");
    assert_eq!(g.cards.get("j1"), Some(&Zone::Judge(1)), "判定区不被获得（QQ1=B）");
}

// ---------------------------------------------------------------------------
// 放逐：目标翻面摸 X（X=已损失体力）
// ---------------------------------------------------------------------------

#[test]
fn fangzhu_flips_target_and_draws_lost_hp() {
    let mut g = game();
    g.seats[0].hp = 1; // 已损失 3（无上限）
    for i in 0..3 {
        let id = format!("d{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
        g.deck.push(id);
    }
    let h = skills::handler("fangzhu").unwrap();
    let mut step = h(&mut g, &ask("fangzhu", 0, serde_json::json!({"step": "ask"}))).unwrap();
    assert!(matches!(step, Step::NeedAt(_, _) | Step::Need(_)), "放逐先选目标");
    g.answer = Some("t:1".into());
    g.answer_skill = Some("fangzhu".into());
    // 目标 → 翻面 → 摸牌 → 上限（依次推进）。
    let mut data = serde_json::json!({"step": "ask"});
    for _ in 0..4 {
        let a = ask("fangzhu", 0, data.clone());
        step = h(&mut g, &a).unwrap();
        match step {
            Step::Next(_, next) => data = next,
            _ => break,
        }
        g.answer = None;
        g.answer_skill = None;
    }
    assert!(g.seats[1].flipped, "目标翻面");
    let hand = g.cards.iter().filter(|(_, z)| matches!(z, Zone::Hand(1))).count();
    assert_eq!(hand, 3, "摸 X=3（已损失体力，无 min 5 上限）");
}

// ---------------------------------------------------------------------------
// 不屈：濒死翻创（点数重复弃，不重复回 1）
// ---------------------------------------------------------------------------

#[test]
fn buqu_moves_chuang_out_of_game_and_revives() {
    let mut g = game();
    g.seats[0].skills = vec!["buqu".into()];
    g.seats[0].hp = 0;
    let id = "d0".to_string();
    g.cards.insert(id.clone(), Zone::Draw);
    g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    g.deck.push(id.clone());
    sanguosha3::kernel::flow::enter_dying(&mut g, 0, None);
    // 不屈判定走 `quick_judge`（含鬼才/天妒窗口）：推进到判定结算完。
    sanguosha3::kernel::resolve::advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 1, "创点数不同 → 回复至 1");
    assert_eq!(g.cards.get("d0"), Some(&Zone::Void), "创移出游戏（不产生弃置）");
    assert_eq!(g.counter("buqu_total.0"), 1, "创数 1");
    assert!(!g.stack.iter().any(|f| matches!(&f.inner, sanguosha3::kernel::resolve::FrameInner::Flow(ff) if ff.kind == sanguosha3::kernel::resolve::FlowKind::Dying)), "已脱离濒死，不压濒死帧");
    assert_eq!(g.hand_limit(0), 1, "手牌上限 = 创数（QQ2=A）");
}

#[test]
fn active_reflects_own_skills_only() {
    let g = game();
    assert!(active(&g, 0, "jushou"));
    assert!(!active(&g, 1, "jushou"));
}
