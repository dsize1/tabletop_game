//! M1.8 card_play 门禁：使用 vs 打出 / 目标列表 / 响应牌 / 无懈链。

use std::collections::HashMap;

use sanguosha3::kernel::card_play;
use sanguosha3::kernel::cards::{CardDef, Category, ResponseKind, Tags, TargetMode};
use sanguosha3::kernel::{Card, DamageKind, Game, Seat, Suit, Zone};

fn defs(code: &str) -> Option<CardDef> {
    // 最小测试牌表（M2 换成生成物 cards_generated）。
    let table: &[CardDef] = &[
        CardDef { code: "sha", name: "杀", category: Category::Basic, target_mode: TargetMode::Single, tags: Tags { sha: true, ..Default::default() }, damage_kind: DamageKind::Normal },
        CardDef { code: "shan", name: "闪", category: Category::Basic, target_mode: TargetMode::None, tags: Tags { shan: true, ..Default::default() }, damage_kind: DamageKind::Normal },
        CardDef { code: "tao", name: "桃", category: Category::Basic, target_mode: TargetMode::SelfOnly, tags: Tags { tao: true, ..Default::default() }, damage_kind: DamageKind::Normal },
        CardDef { code: "wuxie", name: "无懈可击", category: Category::Trick, target_mode: TargetMode::None, tags: Tags { wuxie: true, ..Default::default() }, damage_kind: DamageKind::Normal },
        CardDef { code: "huosha", name: "火杀", category: Category::Basic, target_mode: TargetMode::Single, tags: Tags { sha: true, ..Default::default() }, damage_kind: DamageKind::Fire },
    ];
    table.iter().find(|d| d.code == code).copied()
}

fn game_with_hand(seat: usize, cards: &[(&str, &str)]) -> Game {
    // cards: (牌实例 id, 牌定义 code)
    let mut g = Game {
        seats: vec![
            Seat { id: 0, hp: 4, max_hp: 4, alive: true, granted: vec![] },
            Seat { id: 1, hp: 3, max_hp: 4, alive: true, granted: vec![] },
        ],
        ..Default::default()
    };
    for (id, code) in cards {
        g.cards.insert(id.to_string(), Zone::Hand(seat));
        g.card_defs.insert(
            id.to_string(),
            Card { id: id.to_string(), def: code.to_string(), suit: Suit::Spade, rank: 7 },
        );
    }
    g
}

// ---------------------------------------------------------------------------
// 使用 vs 打出：闪不能主动使用
// ---------------------------------------------------------------------------

#[test]
fn shan_is_not_a_legal_active_use() {
    let g = game_with_hand(0, &[("c1", "shan"), ("c2", "sha")]);
    let uses = card_play::legal_uses(&g, 0, &defs);
    assert_eq!(uses, vec!["c2".to_string()], "闪只能响应，不能主动使用");
}

#[test]
fn tao_is_not_legal_at_full_hp() {
    let g = game_with_hand(0, &[("c1", "tao")]);
    let uses = card_play::legal_uses(&g, 0, &defs);
    assert!(uses.is_empty(), "满血桃不能主动使用");
}

// ---------------------------------------------------------------------------
// 响应牌：读标签，不按牌名
// ---------------------------------------------------------------------------

#[test]
fn respond_reads_tags_not_name() {
    let shan = defs("shan").unwrap();
    let sha = defs("sha").unwrap();
    let wuxie = defs("wuxie").unwrap();
    assert!(card_play::can_respond(&shan, ResponseKind::Shan));
    assert!(card_play::can_respond(&sha, ResponseKind::Sha));
    assert!(card_play::can_respond(&wuxie, ResponseKind::Wuxie));
    assert!(!card_play::can_respond(&sha, ResponseKind::Shan), "杀不能当闪");
}

// ---------------------------------------------------------------------------
// 目标列表：目标合法性
// ---------------------------------------------------------------------------

#[test]
fn single_target_must_be_other_alive() {
    let sha = defs("sha").unwrap();
    assert!(card_play::can_target(&sha, 0, 1, true));
    assert!(!card_play::can_target(&sha, 0, 0, true), "杀不能指定自己");
    assert!(!card_play::can_target(&sha, 0, 1, false), "不能指定死亡角色");
}

#[test]
fn self_only_target() {
    let tao = defs("tao").unwrap();
    assert!(card_play::can_target(&tao, 0, 0, true));
    assert!(!card_play::can_target(&tao, 0, 1, true));
}

// ---------------------------------------------------------------------------
// 无懈链：奇偶层
// ---------------------------------------------------------------------------

#[test]
fn wuxie_odd_layers_cancel() {
    assert!(!card_play::wuxie_cancelled(0), "0 层 = 锦囊生效");
    assert!(card_play::wuxie_cancelled(1), "1 层 = 被抵消");
    assert!(!card_play::wuxie_cancelled(2), "2 层 = 反无懈，锦囊生效");
    assert!(card_play::wuxie_cancelled(3), "3 层 = 被抵消");
}

// ---------------------------------------------------------------------------
// 桃：回血唯一出口
// ---------------------------------------------------------------------------

#[test]
fn tao_heals_through_flow() {
    let mut g = game_with_hand(0, &[]);
    g.seats[1].hp = 2;
    let evs = card_play::resolve_tao(&mut g, 1);
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(evs[0].kind, sanguosha3::kernel::event::EventKind::Healed);
}

// ---------------------------------------------------------------------------
// 弃牌：唯一出口进弃牌堆
// ---------------------------------------------------------------------------

#[test]
fn to_discard_moves_to_discard_zone() {
    let mut g = game_with_hand(0, &[("c1", "sha")]);
    card_play::to_discard(&mut g, "c1");
    assert_eq!(g.cards.get("c1"), Some(&Zone::Discard));
}
