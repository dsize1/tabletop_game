//! M2.1 基本牌结算门禁：杀→闪→伤害→濒死求桃 完整循环 + 酒。

use sanguosha3::kernel::card_play::{resolve_sha, use_jiu, ShaResult};
use sanguosha3::kernel::flow;
use sanguosha3::kernel::{ArmorKind, Game, Seat};

fn game() -> Game {
    Game {
        seats: vec![
            Seat { id: 0, hp: 4, max_hp: 4, alive: true, granted: vec![], identity: None },
            Seat { id: 1, hp: 1, max_hp: 4, alive: true, granted: vec![], identity: None },
        ],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 杀 → 闪 → 伤害
// ---------------------------------------------------------------------------

#[test]
fn slash_blocked_by_shan_no_damage() {
    let mut g = game();
    let r = resolve_sha(&mut g, 0, 1, true, ArmorKind::None);
    assert_eq!(r, ShaResult::Blocked);
    assert_eq!(g.seats[1].hp, 1, "出闪抵消，不受伤");
}

#[test]
fn slash_hits_without_shan() {
    let mut g = game();
    let r = resolve_sha(&mut g, 0, 1, false, ArmorKind::None);
    assert_eq!(r, ShaResult::Hit { damage: 1 });
    assert_eq!(g.seats[1].hp, 0, "无闪受 1 点伤害");
    assert!(flow::is_dying(&g.seats[1]), "hp=0 进入濒死");
}

// ---------------------------------------------------------------------------
// 酒：加伤 + 消费
// ---------------------------------------------------------------------------

#[test]
fn jiu_bonus_adds_damage_and_is_consumed() {
    let mut g = game();
    use_jiu(&mut g);
    let r = resolve_sha(&mut g, 0, 1, false, ArmorKind::None);
    assert_eq!(r, ShaResult::Hit { damage: 2 }, "酒加成后杀 2 伤");
    // 酒已消费：下一张杀回 1 伤
    let r2 = resolve_sha(&mut g, 0, 1, false, ArmorKind::None);
    assert_eq!(r2, ShaResult::Hit { damage: 1 });
}

#[test]
fn jiu_bonus_is_consumed_even_when_blocked() {
    let mut g = game();
    use_jiu(&mut g);
    let r = resolve_sha(&mut g, 0, 1, true, ArmorKind::None);
    assert_eq!(r, ShaResult::Blocked);
    assert_eq!(g.sha_bonus, 0, "酒已消费，即使被闪抵消");
    let r2 = resolve_sha(&mut g, 0, 1, false, ArmorKind::None);
    assert_eq!(r2, ShaResult::Hit { damage: 1 }, "下一张杀不再有加成");
}

// ---------------------------------------------------------------------------
// 濒死求桃
// ---------------------------------------------------------------------------

#[test]
fn dying_rescued_by_tao() {
    let mut g = game();
    g.seats[1].hp = 0;
    assert!(flow::is_dying(&g.seats[1]));
    flow::rescue(&mut g, 1, 1, "桃");
    assert_eq!(g.seats[1].hp, 1);
    assert!(!flow::is_dying(&g.seats[1]), "脱离濒死");
}

#[test]
fn deep_dying_needs_multiple_rescues() {
    let mut g = game();
    g.seats[1].hp = -2; // 溢出伤害
    flow::rescue(&mut g, 1, 1, "桃");
    assert_eq!(g.seats[1].hp, -1);
    assert!(flow::is_dying(&g.seats[1]), "仍濒死");
    flow::rescue(&mut g, 1, 1, "酒自救");
    assert_eq!(g.seats[1].hp, 0);
    assert!(flow::is_dying(&g.seats[1]));
    flow::rescue(&mut g, 1, 1, "桃");
    assert_eq!(g.seats[1].hp, 1);
    assert!(!flow::is_dying(&g.seats[1]), "三次救援脱离濒死");
}

#[test]
fn dying_does_not_imply_dead() {
    // I2：濒死（hp≤0）仍是存活，alive 不变。
    let mut g = game();
    g.seats[1].hp = 0;
    assert!(g.seats[1].alive, "濒死仍存活");
    assert!(flow::is_dying(&g.seats[1]));
}

#[test]
fn slash_then_tao_full_loop() {
    // 完整循环：杀 → 无闪 → 伤害 → 濒死 → 桃 → 脱离濒死。
    let mut g = game();
    let r = resolve_sha(&mut g, 0, 1, false, ArmorKind::None);
    assert_eq!(r, ShaResult::Hit { damage: 1 });
    assert!(flow::is_dying(&g.seats[1]));
    flow::rescue(&mut g, 1, 1, "桃");
    assert!(!flow::is_dying(&g.seats[1]));
    assert_eq!(g.seats[1].hp, 1);
}
