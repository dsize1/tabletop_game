//! M2.3 装备数值链门禁：装备槽位/攻击范围/距离/防具武器单入口 + equip-skill 注入。

use sanguosha3::cards_generated::DEFS;
use sanguosha3::kernel::cards::def;
use sanguosha3::kernel::flow;
use sanguosha3::kernel::gear;
use sanguosha3::kernel::{ArmorKind, Card, DamageKind, DamageSpec, Game, Seat, SourceKind, Suit, WeaponKind, Zone};

fn game(n: usize) -> Game {
    let seats = (0..n)
        .map(|i| Seat { id: i, hp: 4, max_hp: 4, alive: true, granted: vec![], identity: None })
        .collect();
    Game { seats, ..Default::default() }
}

fn add_card(g: &mut Game, id: &str, name: &str, zone: Zone) {
    let code = DEFS.iter().find(|d| d.name == name).unwrap().code;
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(id.to_string(), Card { id: id.to_string(), def: code.to_string(), suit: Suit::Spade, rank: 7 });
}

// ---------------------------------------------------------------------------
// 装备槽位：进槽位 + 顶旧装备
// ---------------------------------------------------------------------------

#[test]
fn equip_into_slot_and_replace_old() {
    let mut g = game(2);
    add_card(&mut g, "w1", "方天画戟", Zone::Hand(0));
    add_card(&mut g, "w2", "青釭剑", Zone::Hand(0));
    assert_eq!(gear::equip(&mut g, 0, "w1"), None, "首件装备无旧牌");
    assert_eq!(g.cards.get("w1"), Some(&Zone::Equip(0, sanguosha3::kernel::Slot::Weapon)));
    // 顶掉方天画戟
    let old = gear::equip(&mut g, 0, "w2");
    assert_eq!(old, Some("w1".to_string()), "旧装备进弃牌堆");
    assert_eq!(g.cards.get("w1"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("w2"), Some(&Zone::Equip(0, sanguosha3::kernel::Slot::Weapon)));
}

// ---------------------------------------------------------------------------
// 攻击范围：默认 1 + 武器范围
// ---------------------------------------------------------------------------

#[test]
fn attack_range_defaults_to_one() {
    let g = game(2);
    assert_eq!(gear::attack_range(&g, 0), 1);
}

#[test]
fn attack_range_reads_weapon_range() {
    let mut g = game(2);
    add_card(&mut g, "w1", "方天画戟", Zone::Hand(0));
    gear::equip(&mut g, 0, "w1");
    assert_eq!(gear::attack_range(&g, 0), 5, "方天画戟范围 4 + 1");
}

// ---------------------------------------------------------------------------
// 距离：座位环 + 坐骑
// ---------------------------------------------------------------------------

#[test]
fn distance_is_ring_distance_with_lower_bound_one() {
    let g = game(4);
    assert_eq!(gear::distance(&g, 0, 1), 1);
    assert_eq!(gear::distance(&g, 0, 2), 2);
    assert_eq!(gear::distance(&g, 0, 3), 1, "座位环取较近一侧");
    assert_eq!(gear::distance(&g, 0, 0), 0);
}

#[test]
fn horse_modifies_distance() {
    let mut g = game(4);
    // 进攻马：0 装备赤兔 → 0→2 从 2 减到 1
    add_card(&mut g, "h1", "赤兔", Zone::Hand(0));
    gear::equip(&mut g, 0, "h1");
    assert_eq!(gear::distance(&g, 0, 2), 1, "进攻马 -1");
    assert_eq!(gear::distance(&g, 0, 3), 1, "距离下限 1");

    // 防御马：2 装备的卢 → 0→2 从 1 加回 2
    add_card(&mut g, "h2", "的卢", Zone::Hand(2));
    gear::equip(&mut g, 2, "h2");
    assert_eq!(gear::distance(&g, 0, 2), 2, "进攻马 -1 + 防御马 +1");
}

// ---------------------------------------------------------------------------
// 防具/武器单入口（equip-skill 注入）
// ---------------------------------------------------------------------------

#[test]
fn armor_of_reads_equipped_armor() {
    let mut g = game(2);
    add_card(&mut g, "a1", "藤甲", Zone::Hand(0));
    gear::equip(&mut g, 0, "a1");
    assert_eq!(gear::armor_of(&g, 0), ArmorKind::Tengjia);
    assert_eq!(gear::armor_of(&g, 1), ArmorKind::None);
}

#[test]
fn weapon_of_reads_equipped_weapon() {
    let mut g = game(2);
    add_card(&mut g, "w1", "青釭剑", Zone::Hand(0));
    gear::equip(&mut g, 0, "w1");
    assert_eq!(gear::weapon_of(&g, 0), WeaponKind::Qinggang);
    assert_eq!(gear::weapon_of(&g, 1), WeaponKind::None);
}

// ---------------------------------------------------------------------------
// 藤甲通过 armor_of → apply_armor 生效（equip-skill 端到端）
// ---------------------------------------------------------------------------

#[test]
fn tengjia_blocks_normal_slash_via_armor_of() {
    let mut g = game(2);
    add_card(&mut g, "a1", "藤甲", Zone::Hand(1));
    gear::equip(&mut g, 1, "a1");
    let armor = gear::armor_of(&g, 1);
    let spec = DamageSpec { from: Some(0), target: 1, amount: 1, kind: DamageKind::Normal, source: SourceKind::Slash };
    assert_eq!(flow::apply_armor(armor, &spec, 1), sanguosha3::kernel::DamageOutcome::Blocked);
}

#[test]
fn def_lookup_by_code_not_name() {
    // 内核按 code 查表（生成物），不按牌名。
    let sha = def("sha").unwrap();
    assert_eq!(sha.name, "杀");
    assert_eq!(def("huo_sha").unwrap().damage_kind, DamageKind::Fire);
}
