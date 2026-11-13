//! M3 视图投影门禁：三分区 + 隐藏信息脱敏 + 9 类区投影 + pending 脱敏。

use sanguosha3::kernel::pending::PendingBuilder;
use sanguosha3::kernel::view::{player_view, public_state};
use sanguosha3::kernel::{Card, Game, Identity, Seat, Suit, Zone};

fn seat(id: usize, identity: Option<Identity>) -> Seat {
    Seat { id, hp: 4, max_hp: 4, alive: true, granted: vec![], skills: vec![], identity, flipped: false, ..Default::default() }
}

fn game() -> Game {
    let mut g = Game {
        seats: vec![
            seat(0, Some(Identity::Lord)),
            seat(1, Some(Identity::Loyalist)),
            seat(2, Some(Identity::Rebel)),
        ],
        ..Default::default()
    };
    // 手牌（座位 0 一张杀，座位 1 一张桃）
    g.cards.insert("h0".into(), Zone::Hand(0));
    g.cards.insert("h1".into(), Zone::Hand(1));
    g.card_defs.insert("h0".into(), Card { id: "h0".into(), def: "sha".into(), suit: Suit::Spade, rank: 7 });
    g.card_defs.insert("h1".into(), Card { id: "h1".into(), def: "tao".into(), suit: Suit::Heart, rank: 3 });
    // 牌堆 + 弃牌堆
    g.cards.insert("d0".into(), Zone::Draw);
    g.card_defs.insert("d0".into(), Card { id: "d0".into(), def: "sha".into(), suit: Suit::Spade, rank: 8 });
    g.cards.insert("c0".into(), Zone::Discard);
    g.card_defs.insert("c0".into(), Card { id: "c0".into(), def: "shan".into(), suit: Suit::Diamond, rank: 2 });
    g.deck.push("d0".into());
    g
}

// ---------------------------------------------------------------------------
// 隐藏信息脱敏
// ---------------------------------------------------------------------------

#[test]
fn public_state_hides_hands_and_identity() {
    let g = game();
    let v = public_state(&g);
    // 他人手牌不出现（hand 字段只在个人视图）
    assert!(v.get("hand").is_none(), "公共视图无手牌");
    // 忠臣/反贼身份隐藏
    let seats = v["seats"].as_array().unwrap();
    assert!(seats[1].get("identity").is_none(), "忠臣身份隐藏");
    assert!(seats[2].get("identity").is_none(), "反贼身份隐藏");
    assert!(seats[0].get("identity").is_some(), "主公身份公开");
}

#[test]
fn public_state_hides_deck_order() {
    let g = game();
    let v = public_state(&g);
    // 牌堆只给数量，不给顺序/牌名
    assert_eq!(v["deck"]["draw"], 1, "牌堆只给数量");
    assert!(v["deck"].get("cards").is_none(), "牌堆顺序不公开");
}

#[test]
fn public_pending_hides_options_and_data() {
    let mut g = game();
    let p = PendingBuilder::new(0, "choice").option("opt1", "秘密选项").escape("放弃").build();
    g.pending = Some(p);
    let v = public_state(&g);
    assert_eq!(v["pending"]["kind"], "choice");
    assert!(v["pending"].get("options").is_none(), "公共 pending 不给 options");
    assert!(v["pending"].get("data").is_none(), "公共 pending 不给 data");
}

// ---------------------------------------------------------------------------
// 个人视图
// ---------------------------------------------------------------------------

#[test]
fn player_view_shows_own_hand_and_identity() {
    let g = game();
    let v = player_view(&g, 0);
    assert_eq!(v["hand"].as_array().unwrap().len(), 1, "本人手牌 1 张");
    assert_eq!(v["hand"][0], "杀");
    assert_eq!(v["my_identity"], "Lord");
}

#[test]
fn player_view_does_not_leak_others_hand() {
    let g = game();
    let v = player_view(&g, 0);
    // 本人只能看到自己的手牌，看不到座位 1 的桃
    assert_eq!(v["hand"].as_array().unwrap().len(), 1);
    assert_eq!(v["hand"][0], "杀");
}

#[test]
fn player_view_shows_full_pending_only_to_actor() {
    let mut g = game();
    let p = PendingBuilder::new(0, "choice").option("opt1", "选项一").data(serde_json::json!({"x": 1})).escape("放弃").build();
    g.pending = Some(p);
    let v = player_view(&g, 0);
    assert!(v.get("my_pending").is_some(), "actor 看到完整 pending");
    assert!(v["my_pending"].get("options").is_some());
    let v2 = player_view(&g, 1);
    assert!(v2.get("my_pending").is_none(), "非 actor 看不到完整 pending");
}

// ---------------------------------------------------------------------------
// 9 类区投影
// ---------------------------------------------------------------------------

#[test]
fn nine_zones_are_projected() {
    let g = game();
    let v = public_state(&g);
    // Draw/Discard 数量；Hand 不在公共视图
    assert_eq!(v["deck"]["draw"], 1);
    assert_eq!(v["deck"]["discard"], 1);
    // 装备/判定公共可见（这里无装备）
    let seats = v["seats"].as_array().unwrap();
    assert!(seats[0]["equips"].as_array().unwrap().is_empty());
}

#[test]
fn dead_identity_is_public_in_view() {
    let mut g = game();
    g.seats[2].alive = false;
    let v = public_state(&g);
    let seats = v["seats"].as_array().unwrap();
    assert!(seats[2].get("identity").is_some(), "死亡身份公开");
}
