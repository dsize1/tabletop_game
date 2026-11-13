//! M1.6 flow 门禁：伤害二维（属性 × 来源）+ 三分类（伤害/失去体力/减上限）+ 数值链。

use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::flow;
use sanguosha3::kernel::{ArmorKind, DamageKind, DamageOutcome, DamageSpec, Game, Seat, SourceKind};

fn game() -> Game {
    Game {
        seats: vec![
            Seat { id: 0, hp: 4, max_hp: 4, alive: true, granted: vec![], identity: None },
            Seat { id: 1, hp: 4, max_hp: 4, alive: true, granted: vec![], identity: None },
        ],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 藤甲二维判定（2026-11-13 裁定）
// ---------------------------------------------------------------------------

#[test]
fn tengjia_blocks_normal_slash() {
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Blocked);
}

#[test]
fn tengjia_blocks_converted_normal_slash() {
    // 武圣红牌 → 普通杀（Normal），藤甲同样无效（「含转化普杀」）。
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert!(spec.is_normal_slash());
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Blocked);
}

#[test]
fn tengjia_amplifies_fire_slash() {
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Fire, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Dealt(2));
}

#[test]
fn tengjia_lets_thunder_slash_through() {
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Thunder, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Dealt(1));
}

#[test]
fn tengjia_blocks_nanman_and_wanjian() {
    for src in [SourceKind::Nanman, SourceKind::Wanjian] {
        let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: src };
        assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Blocked, "{src:?}");
    }
}

#[test]
fn tengjia_lets_duel_through() {
    // 决斗对藤甲有效（普通伤害，非杀非南蛮万箭）。
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Duel };
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 1), DamageOutcome::Dealt(1));
}

#[test]
fn tengjia_amplifies_fire_skill_damage() {
    // 焚城 = 火焰伤害（Skill 来源，Fire 属性），藤甲 +1。
    let spec = DamageSpec { from: Some(0), target: 1, amount: 2, kind: DamageKind::Fire, source: SourceKind::Skill };
    assert_eq!(flow::apply_armor(ArmorKind::Tengjia, &spec, 2), DamageOutcome::Dealt(3));
}

// ---------------------------------------------------------------------------
// 白银狮子：最后结算，>1 改为 1
// ---------------------------------------------------------------------------

#[test]
fn baiyin_caps_damage_at_one() {
    let spec = DamageSpec { from: Some(0), target: 1, amount: 3, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(ArmorKind::Baiyin, &spec, 3), DamageOutcome::Dealt(1));
}

#[test]
fn baiyin_does_not_touch_single_damage() {
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Fire, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(ArmorKind::Baiyin, &spec, 1), DamageOutcome::Dealt(1));
}

// ---------------------------------------------------------------------------
// 三分类：伤害 / 失去体力 / 减上限 各自独立事件
// ---------------------------------------------------------------------------

#[test]
fn damage_emits_damage_after_with_2d() {
    let mut g = game();
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Fire, source: SourceKind::Slash };
    let evs = flow::damage(&mut g, spec, ArmorKind::None, 0);
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].kind, EventKind::DamageAfter);
    let d = evs[0].damage.expect("伤害事件带二维");
    assert_eq!(d.kind, DamageKind::Fire);
    assert_eq!(d.source, SourceKind::Slash);
}

#[test]
fn blocked_damage_emits_nothing_and_does_not_touch_hp() {
    let mut g = game();
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    let evs = flow::damage(&mut g, spec, ArmorKind::Tengjia, 0);
    assert!(evs.is_empty(), "被藤甲免疫不产伤害事件");
    assert_eq!(g.seats[1].hp, 4, "免疫不扣血");
}

#[test]
fn lose_hp_is_not_damage() {
    let mut g = game();
    let evs = flow::lose_hp(&mut g, 1, 1, "苦肉");
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(evs[0].kind, EventKind::HpLost, "失去体力发 HpLost，不是 DamageAfter");
    assert!(evs[0].damage.is_none(), "失去体力不带伤害二维");
}

#[test]
fn heal_caps_at_max_hp() {
    let mut g = game();
    g.seats[1].hp = 2;
    let evs = flow::heal(&mut g, 1, 5, "桃");
    assert_eq!(g.seats[1].hp, 4, "回复钳到体力上限");
    assert_eq!(evs[0].kind, EventKind::Healed);
    assert_eq!(evs[0].amount, Some(2), "只回复实际回复量");
}

#[test]
fn lose_max_hp_is_third_kind() {
    let mut g = game();
    let evs = flow::lose_max_hp(&mut g, 1, 1, "自立");
    assert_eq!(g.seats[1].max_hp, 3);
    assert_eq!(g.seats[1].hp, 3, "减上限时 hp 一并钳到新上限");
    assert_eq!(evs[0].kind, EventKind::MaxHpChanged);
    assert!(evs[0].damage.is_none());
}

#[test]
fn damage_never_drops_below_zero() {
    let mut g = game();
    let spec = DamageSpec { from: Some(0), target: 1, amount: 99, kind: DamageKind::Normal, source: SourceKind::Duel };
    flow::damage(&mut g, spec, ArmorKind::None, 0);
    assert_eq!(g.seats[1].hp, 0, "体力钳到 0，濒死由 resolve 层处理");
}
