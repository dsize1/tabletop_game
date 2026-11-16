//! B16–B20 内核前置（K13/K14/K16）：动作批次 / 进入弃牌堆事实 / 死亡生命周期。
//!
//! 用户 2026-11-16 裁定：行殇必须由「死亡弃牌事件」驱动，且死亡时技能（断肠/节命/追忆）
//! 先于死亡弃牌；不得在 `settle_death` 硬编码技能顺序。

mod common;

use common::{add, add_suit, game, submit};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::{flow, resolve, turn, Zone};
use sanguosha3::kernel::Suit;

// ---------------------------------------------------------------------------
// K13 单动作批次
// ---------------------------------------------------------------------------

#[test]
fn action_batches_are_cleared_between_actions() {
    let mut g = game(3);
    add(&mut g, "h1", "sha", Zone::Hand(0));
    add(&mut g, "h2", "tao", Zone::Hand(0));
    add(&mut g, "h3", "shan", Zone::Hand(2));

    g.begin_action_as(0);
    flow::discard(&mut g, 0, "h1", "测试");
    flow::discard(&mut g, 0, "h2", "测试");
    assert_eq!(g.discard_batch.get(&0).map(|v| v.len()), Some(2), "单动作弃置 2 张");
    assert_eq!(g.lost_batch.get(&0).map(|v| v.len()), Some(2), "单动作失去 2 张");
    assert!(g.mark_action_seen("once"));

    g.begin_action_as(1);
    assert!(g.discard_batch.is_empty(), "新动作清空弃置批次");
    assert!(g.lost_batch.is_empty(), "新动作清空失去批次");
    assert!(g.action_seen.is_empty(), "新动作清空去重标记");

    // 获得批次：1 号获得 0 号一张牌。
    flow::gain(&mut g, 1, "h3", 1, "测试");
    assert!(
        g.gain_batch.iter().any(|(to, from, id)| *to == 1 && *from == 2 && id == "h3"),
        "获得批次（他人 → 自己）"
    );
}

// ---------------------------------------------------------------------------
// K14 进入弃牌堆事实（任意方式）与红色计数
// ---------------------------------------------------------------------------

#[test]
fn card_to_discard_is_emitted_for_discard_and_red_counter_tracks_physical_suit() {
    let mut g = game(3);
    add_suit(&mut g, "red", "tao", Zone::Hand(0), Suit::Heart, 3);
    add_suit(&mut g, "black", "shan", Zone::Hand(0), Suit::Spade, 4);
    g.begin_action_as(0);
    flow::discard(&mut g, 0, "red", "测试");
    flow::discard(&mut g, 0, "black", "测试");

    let n = g
        .log
        .iter()
        .filter(|e| e.kind == EventKind::CardToDiscard)
        .count();
    assert_eq!(n, 2, "每张进入弃牌堆的牌都发 CardToDiscard");
    assert_eq!(g.counter(&format!("turn.{}.red_to_discard", g.turn)), 1, "只统计红色实体牌");
    // 弃置契约不变：CardDiscarded 仍只表示「弃置」。
    let d = g.log.iter().filter(|e| e.kind == EventKind::CardDiscarded).count();
    assert_eq!(d, 2, "弃置仍发 CardDiscarded");
}

// ---------------------------------------------------------------------------
// K16 死亡生命周期：Death（死亡时窗口）→ 死亡弃牌 → DeathDiscarded（行殇）
// ---------------------------------------------------------------------------

#[test]
fn death_order_is_death_then_discard_then_death_discarded_and_xingshang_gains_from_discard() {
    let mut g = game(4);
    g.seats[0].skills = vec!["xingshang".into()];
    add(&mut g, "h1", "tao", Zone::Hand(2));
    add(&mut g, "e1", "qinggang_jian", Zone::Equip(2, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "j1", "lebu_sishu", Zone::Judge(2));
    g.judge_stacks.entry(2).or_default().push("j1".into());

    turn::settle_death(&mut g, 2, Some(1));
    // 有死亡弃牌订阅者（行殇）→ 延后清理。
    assert!(g.pending.is_none());
    resolve::advance(&mut g).expect("推进死亡事件");
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("xingshang_ask".into()),
        "行殇在 DeathDiscarded 后询问"
    );

    let idx = |kind: EventKind| g.log.iter().position(|e| e.kind == kind).unwrap();
    let death = idx(EventKind::Death);
    let to_discard = idx(EventKind::CardToDiscard);
    let dd = idx(EventKind::DeathDiscarded);
    assert!(death < to_discard && to_discard < dd, "死亡时窗口 → 死亡弃牌 → DeathDiscarded");

    // 死亡弃牌已先入弃牌堆（行殇从弃牌堆获得）；判定区不进 DeathDiscarded。
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("e1"), Some(&Zone::Discard));
    let batch = g
        .log
        .iter()
        .find(|e| e.kind == EventKind::DeathDiscarded)
        .expect("DeathDiscarded");
    assert!(batch.cards.iter().any(|c| c == "h1") && batch.cards.iter().any(|c| c == "e1"));
    assert!(!batch.cards.iter().any(|c| c == "j1"), "判定区不进行殇批次");
    assert!(!g.log.iter().any(|e| e.kind == EventKind::CardToDiscard && e.card.as_deref() == Some("j1")));

    submit(&mut g, 0, "choose:xingshang").expect("行殇发动");
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)));
    assert_eq!(g.cards.get("e1"), Some(&Zone::Hand(0)));
    assert_eq!(g.cards.get("j1"), Some(&Zone::Discard), "判定区仍进弃牌堆");
}

// ---------------------------------------------------------------------------
// K17 装备区单出口：手牌置入他人装备区 / 两角色装备区交换
// ---------------------------------------------------------------------------

#[test]
fn gear_place_from_hand_replaces_and_emits_events() {
    let mut g = game(3);
    add(&mut g, "w1", "qinggang_jian", Zone::Hand(0));
    add(&mut g, "w2", "zhuge_liannu", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    let old = sanguosha3::kernel::gear::place_from_hand(&mut g, 0, "w1", 1).unwrap();
    assert_eq!(old.as_deref(), Some("w2"));
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.cards.get("w2"), Some(&Zone::Discard));
    assert!(g.log.iter().any(|e| e.kind == EventKind::CardDiscarded && e.card.as_deref() == Some("w2")));
}

#[test]
fn gear_swap_zones_moves_both_sides() {
    let mut g = game(3);
    add(&mut g, "w1", "qinggang_jian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "w2", "zhuge_liannu", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    sanguosha3::kernel::gear::swap_zones(&mut g, 0, 1).unwrap();
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.cards.get("w2"), Some(&Zone::Equip(0, sanguosha3::kernel::Slot::Weapon)));
    assert_eq!(g.log.iter().filter(|e| e.kind == EventKind::CardLost).count(), 2);
    assert_eq!(g.log.iter().filter(|e| e.kind == EventKind::CardGained).count(), 2);
}

// ---------------------------------------------------------------------------
// K18 牌级标记 / 回合末归还
// ---------------------------------------------------------------------------

#[test]
fn card_mark_clears_when_leaving_hand() {
    let mut g = game(3);
    add(&mut g, "h1", "sha", Zone::Hand(0));
    g.set_card_mark("h1", "luoying");
    assert_eq!(g.card_mark("h1"), Some("luoying"));
    flow::discard(&mut g, 0, "h1", "测试");
    assert_eq!(g.card_mark("h1"), None, "离开手牌即清除");
}

#[test]
fn void_return_happens_at_turn_end() {
    let mut g = game(3);
    add(&mut g, "h1", "sha", Zone::Hand(1));
    g.turn_seq = 5;
    flow::exile_until_turn_end(&mut g, 1, "h1", "破军").unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Void));
    assert_eq!(g.void_return.len(), 1);
    let _ = flow::return_due_void(&mut g);
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(1)), "同回合末归还");
    assert!(g.void_return.is_empty());
}

// ---------------------------------------------------------------------------
// K19 伤害事实：DamageAfter 携带牌 id + 本回合/本阶段「此牌造成伤害」计数
// ---------------------------------------------------------------------------

#[test]
fn damage_event_carries_card_and_counters() {
    use sanguosha3::kernel::{DamageKind, DamageSpec, SourceKind};
    let mut g = game(3);
    g.turn = 0;
    g.damage_card = Some("c1".into());
    let spec = DamageSpec {
        from: Some(0),
        target: 1,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    flow::damage(&mut g, spec, sanguosha3::kernel::ArmorKind::None, 0);
    let ev = g.log.iter().find(|e| e.kind == EventKind::DamageAfter).unwrap();
    assert_eq!(ev.card.as_deref(), Some("c1"));
    assert_eq!(g.counter("turn.0.damage_card.c1"), 1);
    assert_eq!(g.counter(&format!("phase.0.damage_card.c1")), 1);
}
