//! M2.4 生命周期门禁：身份公开 / 死亡 / 终局判定 / 濒死不判死。

use sanguosha3::kernel::identity::{check_winner, identity_public, identity_visible};
use sanguosha3::kernel::turn::{begin_game, check_finished, die};
use sanguosha3::kernel::{Card, Game, Identity, Seat, Suit, Winner, Zone};

fn seat(id: usize, identity: Option<Identity>) -> Seat {
    Seat { id, hp: 4, max_hp: 4, alive: true, granted: vec![], skills: vec![], identity, flipped: false, ..Default::default() }
}

fn game() -> Game {
    Game {
        seats: vec![
            seat(0, Some(Identity::Lord)),
            seat(1, Some(Identity::Loyalist)),
            seat(2, Some(Identity::Rebel)),
            seat(3, Some(Identity::Rebel)),
        ],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 身份公开（I2）：主公公开 / 死亡公开 / 其余隐藏
// ---------------------------------------------------------------------------

#[test]
fn lord_identity_is_public_others_hidden() {
    let g = game();
    assert!(identity_public(&g.seats[0]), "主公公开");
    assert!(!identity_public(&g.seats[1]), "忠臣隐藏");
    assert!(!identity_public(&g.seats[2]), "反贼隐藏");
}

#[test]
fn dead_identity_is_public() {
    let mut g = game();
    die(&mut g, 2);
    assert!(identity_public(&g.seats[2]), "死亡后身份公开");
}

#[test]
fn viewer_always_knows_own_identity() {
    let g = game();
    assert!(identity_visible(&g.seats[2], 2), "自己知道自己的身份");
    assert!(!identity_visible(&g.seats[2], 1), "忠臣看不到反贼身份");
}

// ---------------------------------------------------------------------------
// 死亡（I2）：显式 alive，弃牌，触发 Death
// ---------------------------------------------------------------------------

#[test]
fn die_sets_alive_false_and_discards_cards() {
    let mut g = game();
    g.cards.insert("h1".into(), Zone::Hand(2));
    g.card_defs.insert("h1".into(), Card { id: "h1".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    let evs = die(&mut g, 2);
    assert!(!g.seats[2].alive);
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard), "死亡弃牌");
    assert!(
        evs.iter().any(|e| e.kind == sanguosha3::kernel::event::EventKind::Death),
        "死亡事件（P2：弃牌事件同时 emit，不再保证是第一项）"
    );
    assert!(g.events.iter().any(|e| e.kind == sanguosha3::kernel::event::EventKind::Death), "事件已进队列");
}

#[test]
fn die_is_idempotent() {
    let mut g = game();
    die(&mut g, 2);
    let evs = die(&mut g, 2);
    assert!(evs.is_empty(), "已死亡再 die 无事件");
}

// ---------------------------------------------------------------------------
// 终局判定（rules §3）
// ---------------------------------------------------------------------------

#[test]
fn lord_death_ends_with_rebel_win() {
    let mut g = game();
    die(&mut g, 0); // 主公死
    assert_eq!(check_finished(&mut g), Some(Winner::Rebel));
}

#[test]
fn all_rebels_dead_ends_with_lord_win() {
    let mut g = game();
    die(&mut g, 2);
    assert_eq!(check_finished(&mut g), None, "反贼未全灭不终局");
    die(&mut g, 3);
    assert_eq!(check_finished(&mut g), Some(Winner::Lord), "反贼全灭主忠胜");
}

#[test]
fn dying_is_not_dead_and_not_finished() {
    // I2：濒死（hp=0）仍存活，不判负、不公开身份。
    let mut g = game();
    g.seats[0].hp = 0;
    assert!(g.seats[0].alive, "濒死仍存活");
    assert_eq!(check_winner(&g.seats), None, "主公濒死不判负");
    assert!(identity_public(&g.seats[0]), "主公身份本就公开");
}

// ---------------------------------------------------------------------------
// 开局：发身份 + 发手牌
// ---------------------------------------------------------------------------

#[test]
fn begin_game_assigns_identity_and_deals() {
    let n = 10;
    let mut g = Game {
        seats: vec![seat(0, None), seat(1, None), seat(2, None), seat(3, None)],
        deck: (1..=n).map(|i| format!("c{i}")).collect(),
        cards: (1..=n).map(|i| (format!("c{i}"), Zone::Draw)).collect(),
        card_defs: (1..=n).map(|i| (format!("c{i}"), Card { id: format!("c{i}"), def: "sha".into(), suit: Suit::Spade, rank: 7 })).collect(),
        ..Default::default()
    };
    begin_game(&mut g, &[Identity::Loyalist, Identity::Rebel, Identity::Rebel], 2);
    assert_eq!(g.seats[0].identity, Some(Identity::Lord), "座位 0 主公");
    assert_eq!(g.seats[1].identity, Some(Identity::Loyalist));
    // 主公与其余座位**同**发 2 张（主公只 +1 体力上限，不加手牌：官方规则 + v1/v2 一致）
    let hand = |s: usize| g.cards.iter().filter(|(_, z)| **z == Zone::Hand(s)).count();
    assert_eq!(hand(0), 2, "主公发 2 张（与其他人一致）");
    for i in 1..4 {
        assert_eq!(hand(i), 2, "座位 {i} 发 2 张");
    }
}
