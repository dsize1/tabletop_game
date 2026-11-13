//! M4.3 事件路由门禁：事件 → 订阅（窗口+subject）→ 压技能帧 → advance → handler。

use sanguosha3::kernel::event::{Event, EventKind};
use sanguosha3::kernel::resolve::advance;
use sanguosha3::kernel::skills::{self};
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
        seats: vec![seat(0, vec!["fenji", "xingshang"]), seat(1, vec![]), seat(2, vec![])],
        ..Default::default()
    }
}

fn add_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("d{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
        g.deck.push(id);
    }
}

// ---------------------------------------------------------------------------
// 事件 → 订阅路由 → handler（奋激）
// ---------------------------------------------------------------------------

#[test]
fn card_discarded_by_other_routes_to_fenji() {
    let mut g = game();
    add_deck(&mut g, 2);
    // 其他角色（座位 1）手牌被弃置 → 奋激（subject=Other）触发
    let mut ev = Event::simple(EventKind::CardDiscarded, Some(1), Some("弃置"));
    ev.turn = 2; // 他人回合（窗口 AnyTime 不限制，但快照要真实）
    g.events.push(ev);
    advance(&mut g).unwrap();
    // 奋激：座位 0 失 1 体力，座位 1 摸 2
    assert_eq!(g.seats[0].hp, 3, "奋激失去 1 体力");
    let hand = g.cards.iter().filter(|(_, z)| **z == Zone::Hand(1)).count();
    assert_eq!(hand, 2, "失牌角色摸 2");
}

#[test]
fn own_card_discarded_does_not_route_to_fenji() {
    let mut g = game();
    // 自己（座位 0）弃牌 → 奋激 subject=Other 不触发
    let mut ev = Event::simple(EventKind::CardDiscarded, Some(0), Some("弃置"));
    ev.turn = 2;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4, "自己弃牌不触发奋激");
    let hand = g.cards.iter().filter(|(_, z)| **z == Zone::Hand(0)).count();
    assert_eq!(hand, 0, "没人摸牌");
}

// ---------------------------------------------------------------------------
// 死亡事件 → 行殇
// ---------------------------------------------------------------------------

#[test]
fn death_routes_to_xingshang() {
    let mut g = game();
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert("h1".into(), Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 });
    let mut ev = Event::simple(EventKind::Death, Some(1), None);
    ev.turn = 2;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)), "行殇获得死亡角色手牌");
}

// ---------------------------------------------------------------------------
// 事件快照窗口过滤（良姻教训：全局事件 + 未定义窗口 = 误触发）
// ---------------------------------------------------------------------------

#[test]
fn setup_lifecycle_event_does_not_route() {
    // 开局发牌事件（lifecycle=Setup）不路由给技能——明任开局置任的教训（M4.3b 已补拦截）。
    let mut g = game();
    let mut ev = Event::simple(EventKind::CardGained, Some(1), None);
    ev.lifecycle = sanguosha3::kernel::event::Lifecycle::Setup;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4, "Setup 事件不触发奋激");
    assert!(g.stack.is_empty(), "Setup 事件不压帧");
}

#[test]
fn route_event_only_once_per_skill_per_event() {
    let mut g = game();
    add_deck(&mut g, 2);
    let mut ev = Event::simple(EventKind::CardDiscarded, Some(1), None);
    ev.turn = 2;
    g.events.push(ev);
    advance(&mut g).unwrap();
    // 奋激只触发一次（stack 清空，events 清空）
    assert!(g.stack.is_empty());
    assert!(g.events.is_empty());
    assert_eq!(g.seats[0].hp, 3, "奋激只触发一次");
    let _ = skills::handler("fenji");
}
