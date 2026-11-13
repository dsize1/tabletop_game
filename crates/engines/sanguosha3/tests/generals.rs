//! 武将名录（V3 最小字段）门禁：势力/体力/性别/主公 + 座位落地。
//!
//! 背景（2026-11-13 裁定 A）：B1 起技能需要势力（庸肆 X / 立军 吴势力 / 伪帝主公技），
//! 先把生成物 `generals_generated`（232 将）接进内核，选将/大厅切片再补技能映射。

use sanguosha3::kernel::generals;
use sanguosha3::kernel::{Game, Identity, Seat};

fn seat(id: usize, identity: Option<Identity>) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        identity,
        ..Default::default()
    }
}

#[test]
fn generals_generated_has_full_roster() {
    assert_eq!(generals::GENERALS.len(), 232, "232 位武将（标准 25 + 风林火山 48 + 一将 72 + 界 87）");
    assert!(generals::def("sgs_hero_1").is_some());
    assert!(generals::def("nope").is_none());
    assert_eq!(generals::by_gid(100).map(|g| g.code), Some("sgs_hero_100"));
}

#[test]
fn kingdom_lookup_covers_b1_generals() {
    assert_eq!(generals::kingdom("sgs_hero_100"), Some("qun"), "袁术=群");
    assert_eq!(generals::kingdom("sgs_hero_402"), Some("wu"), "陆绩=吴");
    assert_eq!(generals::kingdom("sgs_hero_403"), Some("wu"), "孙亮=吴");
    assert_eq!(generals::kingdom("sgs_hero_400"), Some("shu"), "严颜=蜀");
    assert_eq!(generals::kingdom("sgs_hero_29"), Some("wei"), "曹仁=魏");
}

#[test]
fn apply_to_seat_sets_kingdom_and_lord_bonus() {
    let mut g = Game {
        seats: vec![seat(0, Some(Identity::Lord)), seat(1, Some(Identity::Rebel))],
        ..Default::default()
    };
    generals::apply_to_seat(&mut g, 0, "sgs_hero_29").unwrap(); // 曹仁 4 体力
    assert_eq!(g.seats[0].kingdom, "wei");
    assert_eq!(g.seats[0].max_hp, 5, "主公 +1 体力上限");
    assert_eq!(g.seats[0].hp, 5);
    generals::apply_to_seat(&mut g, 1, "sgs_hero_402").unwrap(); // 陆绩吴
    assert_eq!(g.seats[1].kingdom, "wu");
    assert_eq!(g.seats[1].max_hp, 3, "陆绩 3 体力；非主公不加成");
    assert!(generals::apply_to_seat(&mut g, 0, "sgs_hero_9999").is_err());
}

#[test]
fn kingdom_count_uses_generated_data() {
    let mut g = Game {
        seats: vec![
            seat(0, None),
            seat(1, None),
            seat(2, None),
            seat(3, None),
        ],
        ..Default::default()
    };
    for (i, code) in ["sgs_hero_100", "sgs_hero_29", "sgs_hero_210", "sgs_hero_400"].iter().enumerate() {
        generals::apply_to_seat(&mut g, i, code).unwrap();
    }
    assert_eq!(g.kingdom_count(), 4, "群/魏/吴/蜀 = 4");
    g.seats[3].alive = false;
    assert_eq!(g.kingdom_count(), 3, "死亡座位不计势力");
    g.seats[0].kingdom.clear();
    assert_eq!(g.kingdom_count(), 2, "空势力不计；至少 1 的下限不受影响");
}
