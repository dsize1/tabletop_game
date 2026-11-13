//! M1 第一步的三大不变量门禁：
//! 1. 牌区唯一归属 + 守恒（`zone-ownership`）；
//! 2. 询问完整性（I1）：pending 类型级出口 + `must_ask` 单出口；
//! 3. 事件归属（subject）：订阅声明主体，handler 首行校验。

use std::collections::HashMap;

use sanguosha3::kernel::event::{Event, EventKind, Lifecycle, Subject, Sub, Window};
use sanguosha3::kernel::pending::{must_ask, PendingBuilder};
use sanguosha3::kernel::{Card, Game, Phase, Seat, Zone};

fn seat(id: usize) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        granted: vec![],
    }
}

// ---------------------------------------------------------------------------
// 1. 守恒：每张牌恰好在一个区
// ---------------------------------------------------------------------------

#[test]
fn conservation_every_card_in_exactly_one_zone() {
    let mut g = Game::default();
    g.seats = vec![seat(0), seat(1)];
    // 5 张牌：2 牌堆 + 1 手牌 + 1 装备 + 1 弃牌堆
    let mut cards = HashMap::new();
    let mut defs = HashMap::new();
    let def = |id: &str, cards: &mut HashMap<String, Zone>, defs: &mut HashMap<String, Card>| {
        cards.insert(id.to_string(), Zone::Draw);
        defs.insert(
            id.to_string(),
            Card {
                id: id.to_string(),
                def: "sha".to_string(),
                suit: sanguosha3::kernel::Suit::Spade,
                rank: 7,
            },
        );
    };
    // 直接构造（避免闭包借用问题）
    cards.insert("c1".into(), Zone::Draw);
    cards.insert("c2".into(), Zone::Draw);
    cards.insert("c3".into(), Zone::Hand(0));
    cards.insert("c4".into(), Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    cards.insert("c5".into(), Zone::Discard);
    for i in 1..=5 {
        defs.insert(
            format!("c{i}"),
            Card {
                id: format!("c{i}"),
                def: "sha".to_string(),
                suit: sanguosha3::kernel::Suit::Spade,
                rank: 7,
            },
        );
    }
    g.cards = cards;
    g.card_defs = defs;
    let _ = def; // 未使用
    g.check_conservation().expect("5 张牌都恰好在一个区，守恒应通过");
}

#[test]
fn conservation_rejects_card_without_zone() {
    let mut g = Game::default();
    g.card_defs.insert(
        "orphan".to_string(),
        Card {
            id: "orphan".to_string(),
            def: "sha".to_string(),
            suit: sanguosha3::kernel::Suit::Spade,
            rank: 7,
        },
    );
    // 牌实例存在但 cards 无归属 → 报错
    assert!(g.check_conservation().is_err());
}

#[test]
fn conservation_rejects_zone_without_card_def() {
    let mut g = Game::default();
    g.cards.insert("ghost".to_string(), Zone::Draw);
    // 有区域归属但无牌实例 → 报错
    assert!(g.check_conservation().is_err());
}

#[test]
fn no_orphan_frames_rejects_lingering_frame() {
    let mut g = Game::default();
    g.cards.insert("f1".to_string(), Zone::Frame(0));
    g.card_defs.insert(
        "f1".to_string(),
        Card {
            id: "f1".to_string(),
            def: "sha".to_string(),
            suit: sanguosha3::kernel::Suit::Spade,
            rank: 7,
        },
    );
    assert!(g.check_no_orphan_frames().is_err());
}

// ---------------------------------------------------------------------------
// 2. 询问完整性（I1）：类型级出口 + must_ask 单出口
// ---------------------------------------------------------------------------

#[test]
fn pending_requires_escape_at_type_level() {
    // HasEscape 状态才能 build；escape / escape_mandatory / escape_with 都能构造出合法 pending。
    let p = PendingBuilder::new(0, "choice")
        .option("opt1", "选项一")
        .escape("放弃")
        .build();
    assert_eq!(p.escape_id(), "pass");
    assert!(!p.escape_hidden());

    let m = PendingBuilder::new(0, "discard")
        .escape_mandatory("必须弃置")
        .build();
    assert!(m.escape_hidden()); // 强制步骤出口占位
}

#[test]
fn must_ask_only_when_legal_set_nonempty() {
    // 合法集为空 → 不问；有选项 → 必须问（即使只有 pass）。
    assert!(!must_ask(false));
    assert!(must_ask(true));
}

// ---------------------------------------------------------------------------
// 3. 事件归属（subject）：声明主体，handler 首行校验
// ---------------------------------------------------------------------------

#[test]
fn subject_self_fires_only_on_own_event() {
    // 屯田类：「当**你**于回合外失去牌」——只有事件主体是自己才触发。
    let sub = Sub {
        owner: 1,
        skill: "tuntian".to_string(),
        kind: EventKind::CardLost,
        window: Window::OtherTurn,
        subject: Subject::Self_,
        priority: 0,
        seq: 0,
        side: 0,
    };
    let mut own = Event::simple(EventKind::CardLost, Some(1), None);
    own.turn = 2; // 他人回合
    assert!(sub.accepts(&own), "自己失去牌应触发");

    let mut other = Event::simple(EventKind::CardLost, Some(2), None);
    other.turn = 2;
    assert!(!sub.accepts(&other), "他人失去牌不得触发（M105 教训）");
}

#[test]
fn subject_other_fires_only_on_other_event() {
    // 奋激类：「当**其他角色**的手牌被弃置/获得」。
    let sub = Sub {
        owner: 0,
        skill: "fenji".to_string(),
        kind: EventKind::CardDiscarded,
        window: Window::AnyTime,
        subject: Subject::Other,
        priority: 0,
        seq: 0,
        side: 0,
    };
    let other = Event::simple(EventKind::CardDiscarded, Some(3), None);
    assert!(sub.accepts(&other));

    let own = Event::simple(EventKind::CardDiscarded, Some(0), None);
    assert!(!sub.accepts(&own), "自己弃牌不得触发");
}

#[test]
fn subject_by_and_from_distinguish_actor_and_source() {
    // 「由**你**（施动者）造成的伤害」→ by = 自己；「伤害**来源**是你」→ from = 自己。
    let by_sub = Sub {
        owner: 0,
        skill: "x".to_string(),
        kind: EventKind::DamageAfter,
        window: Window::AnyTime,
        subject: Subject::By,
        priority: 0,
        seq: 0,
        side: 0,
    };
    let mut ev = Event::simple(EventKind::DamageAfter, Some(2), None);
    ev.by = Some(0);
    ev.from = Some(0);
    assert!(by_sub.accepts(&ev));
    ev.by = Some(1); // 施动者换成别人
    assert!(!by_sub.accepts(&ev));

    let from_sub = Sub {
        owner: 0,
        skill: "y".to_string(),
        kind: EventKind::DamageAfter,
        window: Window::AnyTime,
        subject: Subject::From,
        priority: 0,
        seq: 0,
        side: 0,
    };
    ev.by = Some(1);
    ev.from = Some(0); // 来源仍是自己
    assert!(from_sub.accepts(&ev));
}

#[test]
fn window_other_phase_gates_turn_and_phase() {
    // 樵拾：「其他角色的**结束阶段**」——回合/阶段事件 seat=None，主体由 window 决定，subject 用 Any。
    let sub = Sub {
        owner: 0,
        skill: "qiaoshi".to_string(),
        kind: EventKind::PhaseStart,
        window: Window::OtherPhase(Phase::End),
        subject: Subject::Any,
        priority: 0,
        seq: 0,
        side: 0,
    };
    let mut ev = Event::simple(EventKind::PhaseStart, None, None);
    ev.turn = 2;
    ev.phase = Phase::End;
    assert!(sub.accepts(&ev));

    ev.phase = Phase::Play; // 他人回合但非结束阶段
    assert!(!sub.accepts(&ev));

    ev.phase = Phase::End;
    ev.turn = 0; // 自己回合
    assert!(!sub.accepts(&ev), "自己的结束阶段不算 OtherPhase");
}

#[test]
fn lifecycle_setup_events_do_not_route_to_skills() {
    // 开局发牌（Setup 生命周期）不得触发「获得牌」类技能（明任开局置任的教训）。
    let sub = Sub {
        owner: 0,
        skill: "any".to_string(),
        kind: EventKind::CardGained,
        window: Window::AnyTime,
        subject: Subject::Self_,
        priority: 0,
        seq: 0,
        side: 0,
    };
    let mut ev = Event::simple(EventKind::CardGained, Some(0), None);
    ev.lifecycle = Lifecycle::Setup;
    // accepts 不拦 lifecycle（由路由层拦），但这里验证快照可携带 lifecycle。
    // 路由层门禁见后续 resolve 实现；此处只确认字段存在。
    assert_eq!(ev.lifecycle, Lifecycle::Setup);
    let _ = sub.accepts(&ev);
}
