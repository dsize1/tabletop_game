//! M4.3 事件路由门禁：事件 → 订阅（窗口+subject）→ 压技能帧 → advance → handler。

use sanguosha3::kernel::event::{Event, EventKind};
use sanguosha3::kernel::resolve::{advance, submit};
use sanguosha3::kernel::{flow, turn, Card, Game, Seat, Suit, Zone};

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
fn fenji_hook_asks_on_out_of_turn_discard() {
    // 奋激由**手牌变动钩子**驱动（B0 收口：回合外 + 手牌进弃牌堆/增加 → 询问）。
    let mut g = game();
    add_deck(&mut g, 2);
    g.started = true;
    g.turn = 2; // 他人回合（座位 1 回合外）
    let mut ev = Event::simple(EventKind::CardDiscarded, Some(1), Some("弃置"));
    ev.turn = 2;
    g.events.push(ev);
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert(
        "h1".into(),
        Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 },
    );
    flow::discard(&mut g, 1, "h1", "测试");
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into()),
        "奋激先询问（不再自动发动）"
    );
    let hand = g.cards.iter().filter(|(_, z)| **z == Zone::Hand(1)).count();
    submit(&mut g, 0, "choose:fenji").unwrap();
    assert_eq!(g.seats[0].hp, 3, "奋激失去 1 体力");
    assert_eq!(
        g.cards.iter().filter(|(_, z)| **z == Zone::Hand(1)).count(),
        hand + 2,
        "失牌角色摸 2"
    );
}

#[test]
fn fenji_not_triggered_in_changed_player_own_turn() {
    // 回合外口径：手牌变动者处于自己的回合时不触发（QQ1=A）。
    let mut g = game();
    g.started = true;
    g.turn = 1;
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert(
        "h1".into(),
        Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 },
    );
    flow::discard(&mut g, 1, "h1", "测试");
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4, "自己回合内不触发奋激");
    assert_ne!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into())
    );
}

// ---------------------------------------------------------------------------
// 死亡结算 → 行殇（清理前延后询问）
// ---------------------------------------------------------------------------

#[test]
fn death_settlement_defers_to_xingshang() {
    let mut g = game();
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert(
        "h1".into(),
        Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 },
    );
    turn::settle_death(&mut g, 1, None);
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("xingshang_ask".into()),
        "行殇在死亡清理前询问"
    );
    assert!(
        g.log.iter().all(|e| e.kind != EventKind::Death),
        "死亡事件等行殇结算后才 emit"
    );
    let hand = g.cards.iter().filter(|(_, z)| **z == Zone::Hand(1)).count();
    submit(&mut g, 0, "choose:xingshang").unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)), "行殇获得死亡角色手牌");
    assert_eq!(hand, 1);
    assert!(g.log.iter().any(|e| e.kind == EventKind::Death), "死亡事件已收口");
}

#[test]
fn fenji_is_hook_driven_not_event_driven() {
    // 直接推公共事件不再触发奋激（避免「订阅 + 钩子」双触发）。
    let mut g = game();
    let mut ev = Event::simple(EventKind::CardDiscarded, Some(1), None);
    ev.turn = 2;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert!(g.stack.is_empty());
    assert_eq!(g.seats[0].hp, 4, "事件不再直接触发奋激");
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
fn fenji_asks_once_per_hand_change() {
    let mut g = game();
    add_deck(&mut g, 2);
    g.started = true;
    g.turn = 2;
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert(
        "h1".into(),
        Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 },
    );
    flow::discard(&mut g, 1, "h1", "测试");
    advance(&mut g).unwrap();
    // 一次手牌变动 → 只有一个待办（不重复询问）。
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into())
    );
    submit(&mut g, 0, "pass").unwrap();
    assert_ne!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into()),
        "放弃后不再追问"
    );
    assert_eq!(g.seats[0].hp, 4, "放弃不失去体力");
}
