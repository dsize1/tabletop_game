//! M2.2 锦囊 + 无懈链 + 判定区 LIFO 门禁。

use sanguosha3::kernel::card_play::{
    judge_zone_lifo, place_delayed, resolve_guohe, resolve_wuzhong, trick_cancelled_by_wuxie,
    wuxie_cancelled,
};
use sanguosha3::kernel::{Card, Game, Seat, Suit, Zone};

fn game() -> Game {
    let mut g = Game {
        seats: vec![
            Seat { id: 0, hp: 4, max_hp: 4, alive: true, granted: vec![], skills: vec![], identity: None, flipped: false },
            Seat { id: 1, hp: 4, max_hp: 4, alive: true, granted: vec![], skills: vec![], identity: None, flipped: false },
        ],
        ..Default::default()
    };
    // 牌堆 5 张 + 手牌
    for i in 0..5 {
        let id = format!("d{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.card_defs.insert(id.clone(), Card { id: id.clone(), def: "wuzhong".into(), suit: Suit::Spade, rank: 7 });
        g.deck.push(id);
    }
    g
}

// ---------------------------------------------------------------------------
// 无中生有：摸 2
// ---------------------------------------------------------------------------

#[test]
fn wuzhong_draws_two_from_deck() {
    let mut g = game();
    let drawn = resolve_wuzhong(&mut g, 0);
    assert_eq!(drawn.len(), 2);
    for id in &drawn {
        assert_eq!(g.cards.get(id), Some(&Zone::Hand(0)), "摸的牌进手牌");
    }
    assert_eq!(g.deck.len(), 3, "牌堆少 2 张");
}

// ---------------------------------------------------------------------------
// 过河拆桥：弃目标一张牌
// ---------------------------------------------------------------------------

#[test]
fn guohe_discards_target_hand_card() {
    let mut g = game();
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert("h1".into(), Card { id: "h1".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    assert!(resolve_guohe(&mut g, 1, "h1"));
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard));
}

#[test]
fn guohe_rejects_others_card() {
    let mut g = game();
    g.cards.insert("h0".into(), Zone::Hand(0)); // 是 0 的手牌，不是目标 1
    g.card_defs.insert("h0".into(), Card { id: "h0".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    assert!(!resolve_guohe(&mut g, 1, "h0"), "不能弃非目标的牌");
    assert_eq!(g.cards.get("h0"), Some(&Zone::Hand(0)), "牌未被移动");
}

// ---------------------------------------------------------------------------
// 无懈链：奇偶层
// ---------------------------------------------------------------------------

#[test]
fn wuxie_chain_parity() {
    assert!(!trick_cancelled_by_wuxie(0));
    assert!(trick_cancelled_by_wuxie(1));
    assert!(!trick_cancelled_by_wuxie(2));
    assert!(wuxie_cancelled(3));
}

// ---------------------------------------------------------------------------
// 判定区 LIFO
// ---------------------------------------------------------------------------

fn add_judge_card(g: &mut Game, id: &str, def: &str) {
    g.cards.insert(id.to_string(), Zone::Draw);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit: Suit::Spade, rank: 7 },
    );
}

#[test]
fn judge_zone_is_lifo() {
    let mut g = game();
    // 先放乐不思蜀，再放兵粮寸断 → 先判兵粮寸断
    add_judge_card(&mut g, "l1", "lesi");
    add_judge_card(&mut g, "b1", "bingliang");
    assert!(place_delayed(&mut g, 1, "l1", "lesi"));
    assert!(place_delayed(&mut g, 1, "b1", "bingliang"));
    // 栈顶是 b1（兵粮寸断），先判它
    assert_eq!(judge_zone_lifo(&mut g, 1), Some("b1".to_string()));
    assert_eq!(judge_zone_lifo(&mut g, 1), Some("l1".to_string()));
    assert_eq!(judge_zone_lifo(&mut g, 1), None);
}

#[test]
fn judge_zone_rejects_duplicate_name() {
    let mut g = game();
    add_judge_card(&mut g, "l1", "lesi");
    add_judge_card(&mut g, "l2", "lesi");
    add_judge_card(&mut g, "b1", "bingliang");
    assert!(place_delayed(&mut g, 1, "l1", "lesi"));
    assert!(!place_delayed(&mut g, 1, "l2", "lesi"), "同名延时锦囊不可重复放");
    // 不同名可放
    assert!(place_delayed(&mut g, 1, "b1", "bingliang"));
}

#[test]
fn judged_card_goes_to_discard() {
    let mut g = game();
    add_judge_card(&mut g, "l1", "lesi");
    assert!(place_delayed(&mut g, 1, "l1", "lesi"));
    let judged = judge_zone_lifo(&mut g, 1).unwrap();
    assert_eq!(g.cards.get(&judged), Some(&Zone::Discard), "判定后进弃牌堆");
}
