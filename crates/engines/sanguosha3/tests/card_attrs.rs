//! 卡牌属性比较（2026-11-16 用户裁定）：虚拟牌无属性 → 一切比较判否。

use sanguosha3::kernel::cards;
use sanguosha3::kernel::{card_play, gear, ArmorKind, Card, Game, Phase, Seat, Suit, Zone};

fn seat(id: usize) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        male: true,
        kingdom: "qun".to_string(),
        ..Default::default()
    }
}

fn game(n: usize) -> Game {
    Game {
        seats: (0..n).map(seat).collect(),
        ..Default::default()
    }
}

fn mk(g: &mut Game, id: &str, def: &str, suit: Suit, rank: u8, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card { id: id.to_string(), def: def.to_string(), suit, rank },
    );
}

#[test]
fn physical_card_keeps_attributes() {
    let mut g = game(2);
    mk(&mut g, "h1", "sha", Suit::Heart, 7, Zone::Hand(0));
    assert!(cards::is_red(&g, "h1"));
    assert!(!cards::is_black(&g, "h1"));
    assert_eq!(cards::suit_of(&g, "h1"), Some(Suit::Heart));
    assert_eq!(cards::rank_of(&g, "h1"), Some(7));
    mk(&mut g, "s1", "sha", Suit::Spade, 9, Zone::Hand(0));
    assert!(cards::is_black(&g, "s1"));
    assert!(cards::suit_is(&g, "s1", Suit::Spade));
}

#[test]
fn single_carrier_inherits_attributes() {
    // 武圣/龙胆/禁酒：一张承载牌当另一张牌使用 → 继承实体花色/点数。
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "h1", "shan", Suit::Heart, 11, Zone::Hand(0));
    let vid = card_play::use_as(&mut g, 0, "sha", &["h1".to_string()], &[], false).expect("use_as");
    assert!(cards::is_red(&g, &vid), "单承载 → 继承红桃");
    assert_eq!(cards::rank_of(&g, &vid), Some(11), "单承载 → 继承点数");
}

#[test]
fn virtual_without_carrier_has_no_attributes() {
    // 纯视为使用（无承载）→ 无属性：颜色/花色恒否、点数无。
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    let vid = card_play::use_virtual(&mut g, 0, "sha", &[], false).expect("use_virtual");
    assert_eq!(cards::suit_of(&g, &vid), None);
    assert_eq!(cards::rank_of(&g, &vid), None);
    assert!(!cards::is_red(&g, &vid));
    assert!(!cards::is_black(&g, &vid), "无属性不是黑色（仁王盾不拦）");
    assert!(!cards::suit_is(&g, &vid, Suit::Spade));
    assert!(!cards::same_color(&g, &vid, &vid), "同色判定恒否");
    assert!(!cards::same_rank(&g, &vid, &vid), "同点判定恒否");
}

#[test]
fn single_carrier_applies_skill_overrides() {
    // 单张承载读**实例属性**：禁酒「【酒】视为 K」、红颜「黑桃→红桃」都必须在比较里生效。
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].skills = vec!["jinjiu".into()];
    mk(&mut g, "j1", "jiu", Suit::Diamond, 4, Zone::Hand(0));
    let vid = card_play::use_as(&mut g, 0, "sha", &["j1".to_string()], &[], false).expect("use_as");
    assert_eq!(
        cards::rank_of(&g, &vid),
        Some(13),
        "禁酒：单承载【酒】当【杀】→ 点数视为 K（13）"
    );

    let mut g2 = game(2);
    g2.turn = 0;
    g2.phase = Phase::Play;
    g2.seats[0].skills = vec!["hongyan".into()];
    mk(&mut g2, "s1", "shan", Suit::Spade, 7, Zone::Hand(0));
    let vid2 = card_play::use_as(&mut g2, 0, "shan", &["s1".to_string()], &[], false).expect("use_as");
    assert!(cards::is_red(&g2, &vid2), "红颜：黑桃承载 → 有效花色红桃");
    assert!(!cards::is_black(&g2, &vid2));
}

#[test]
fn multi_carrier_has_no_attributes() {
    // 长标/丈八：多张承载 → 无属性（不取第一张）。
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "c2", "tao", Suit::Heart, 6, Zone::Hand(0));
    let vid = card_play::use_as(
        &mut g,
        0,
        "sha",
        &["c1".to_string(), "c2".to_string()],
        &[],
        false,
    )
    .expect("use_as");
    assert_eq!(cards::suit_of(&g, &vid), None, "多承载 → 无花色");
    assert_eq!(cards::rank_of(&g, &vid), None, "多承载 → 无点数");
    assert!(!cards::is_black(&g, &vid) && !cards::is_red(&g, &vid));
}

#[test]
fn zhangba_slash_has_no_attributes() {
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "c2", "sha", Suit::Spade, 6, Zone::Hand(0));
    card_play::begin_use_zhangba(&mut g, 0, "c1", "c2").expect("丈八");
    let vid = g.damage_card.clone().expect("虚拟杀 id");
    assert_eq!(cards::suit_of(&g, &vid), None);
    assert_eq!(cards::rank_of(&g, &vid), None);
}

#[test]
fn pindian_rank_of_virtual_card_is_zero() {
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    let vid = card_play::use_virtual(&mut g, 0, "sha", &[], false).expect("use_virtual");
    assert_eq!(
        sanguosha3::kernel::skills::pindian_rank(&g, 0, &vid),
        0,
        "无属性 → 拼点有效点数 0（判负）"
    );
}

#[test]
fn renwang_does_not_block_virtual_slash() {
    // 仁王盾「黑色【杀】无效」：虚拟杀无颜色 → 不是黑色 → **不拦**。
    let mut g = game(2);
    g.turn = 0;
    g.phase = Phase::Play;
    mk(&mut g, "c1", "sha", Suit::Spade, 5, Zone::Hand(0));
    mk(&mut g, "c2", "sha", Suit::Spade, 6, Zone::Hand(0));
    card_play::begin_use_zhangba(&mut g, 0, "c1", "c2").expect("丈八");
    let vid = g.damage_card.clone().expect("虚拟杀 id");
    assert_eq!(gear::armor_of(&g, 1), ArmorKind::None);
    // 直接复用内核判定入口：`is_black` 为 false → step_sha 的仁王盾分支不触发。
    assert!(!cards::is_black(&g, &vid), "虚拟杀不是黑色 → 仁王盾无效");
    // 对照：实体黑【杀】是黑色 → 会被拦。
    mk(&mut g, "b1", "sha", Suit::Club, 3, Zone::Hand(0));
    assert!(cards::is_black(&g, "b1"));
}
