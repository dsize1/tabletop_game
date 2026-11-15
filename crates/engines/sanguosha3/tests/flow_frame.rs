//! M4.3b 门禁：流程帧 handler（杀/桃）+ 归一化摘要（差分对拍口径）。

use sanguosha3::kernel::card_play::begin_use;
use sanguosha3::kernel::resolve::advance;
use sanguosha3::kernel::view::summary;
use sanguosha3::kernel::{Card, Game, Seat, Suit, Zone};
/// 提交一个选项（内部走 `resolve::submit`；无待办时视为无事）。
fn submit(g: &mut Game, seat: usize, option: &str) {
    sanguosha3::kernel::resolve::submit(g, seat, option).unwrap();
}


fn seat(id: usize) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        granted: vec![],
        skills: vec![],
        identity: None,
        flipped: false, ..Default::default()
    }
}

fn game() -> Game {
    Game { seats: vec![seat(0), seat(1)], ..Default::default() }
}

// ---------------------------------------------------------------------------
// 流程帧：杀 → 伤害
// ---------------------------------------------------------------------------

#[test]
fn flow_card_slash_deals_damage() {
    let mut g = game();
    g.cards.insert("s1".into(), Zone::Hand(0));
    g.card_defs.insert("s1".into(), Card { id: "s1".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    // P3：使用入口 → 结算流程帧 → 目标无【闪】时同步结算（合法集为空不询问）。
    begin_use(&mut g, 0, "s1", false).unwrap();
    advance(&mut g).unwrap();
    submit(&mut g, 0, "t:1");
    assert_eq!(g.seats[1].hp, 3, "杀造成 1 伤");
    assert_eq!(g.cards.get("s1"), Some(&Zone::Discard), "杀结算后进弃牌堆");
}

#[test]
fn flow_card_tao_heals() {
    let mut g = game();
    g.seats[0].hp = 2;
    g.cards.insert("t1".into(), Zone::Hand(0));
    g.card_defs.insert("t1".into(), Card { id: "t1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 });
    begin_use(&mut g, 0, "t1", false).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 3, "桃回 1");
}

// ---------------------------------------------------------------------------
// 归一化摘要（对拍口径）
// ---------------------------------------------------------------------------

#[test]
fn summary_normalizes_hp_and_hand_and_zones() {
    let mut g = game();
    g.cards.insert("h0".into(), Zone::Hand(0));
    g.card_defs.insert("h0".into(), Card { id: "h0".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    g.cards.insert("d0".into(), Zone::Draw);
    g.card_defs.insert("d0".into(), Card { id: "d0".into(), def: "shan".into(), suit: Suit::Diamond, rank: 2 });
    g.cards.insert("c0".into(), Zone::Discard);
    g.card_defs.insert("c0".into(), Card { id: "c0".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 });

    let s = summary(&g);
    assert_eq!(s["seats"][0]["hand"], 1, "座位 0 手牌数 1");
    assert_eq!(s["seats"][0]["hp"], 4);
    assert_eq!(s["zones"]["draw"], 1);
    assert_eq!(s["zones"]["discard"], 1);
    // 手牌明细不在摘要里（只有数量，屏蔽牌 id 差异）
    assert!(s["seats"][0].get("cards").is_none(), "摘要不含牌明细");
}
