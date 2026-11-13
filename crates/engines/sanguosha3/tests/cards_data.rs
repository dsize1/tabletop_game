//! M2 生成物接入门禁：V3 `cards_generated.rs` 数据完整性 + 属性字段正确。

use sanguosha3::cards_generated::{DEFS, DECK, DECK_SIZE};
use sanguosha3::kernel::cards::Category;
use sanguosha3::kernel::DamageKind;

fn def(name: &str) -> &sanguosha3::kernel::cards::CardDef {
    DEFS.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("牌定义缺失：{name}"))
}

#[test]
fn deck_has_160_physical_cards() {
    assert_eq!(DECK_SIZE, 160, "标准 108 + 军争 52");
    assert_eq!(DECK.len(), 160);
}

#[test]
fn deck_is_ordered_standard_then_junzheng() {
    // 建堆顺序固定：standard → junzheng（同 seed 下牌实例 id 稳定）。
    let standard = DECK.iter().filter(|c| c.pack == "standard").count();
    let junzheng = DECK.iter().filter(|c| c.pack == "junzheng").count();
    assert_eq!(standard, 108);
    assert_eq!(junzheng, 52);
}

#[test]
fn damage_kind_is_read_from_field_not_name() {
    assert_eq!(def("火杀").damage_kind, DamageKind::Fire);
    assert_eq!(def("雷杀").damage_kind, DamageKind::Thunder);
    assert_eq!(def("杀").damage_kind, DamageKind::Normal);
}

#[test]
fn slash_tags_include_converted_sources() {
    // 杀/火杀/雷杀都是 tags.sha = true（藤甲「普通杀无效」靠 source_kind + damage_kind 合判）。
    for n in ["杀", "火杀", "雷杀"] {
        assert!(def(n).tags.sha, "{n} 应是 sha 标签");
    }
}

#[test]
fn response_and_basic_tags() {
    assert!(def("闪").tags.shan);
    assert!(def("桃").tags.tao);
    assert!(def("酒").tags.jiu);
    assert!(def("无懈可击").tags.wuxie);
    assert!(def("决斗").tags.duel);
}

#[test]
fn delayed_tricks_are_delayed_category() {
    for n in ["乐不思蜀", "兵粮寸断", "闪电"] {
        assert_eq!(def(n).category, Category::DelayedTrick, "{n} 应是延时锦囊");
    }
}

#[test]
fn damage_tricks_are_tagged() {
    for n in ["决斗", "南蛮入侵", "万箭齐发", "火攻"] {
        assert!(def(n).tags.damage_trick, "{n} 应是伤害锦囊");
    }
}

#[test]
fn armor_cards_exist() {
    for n in ["八卦阵", "仁王盾", "藤甲", "白银狮子"] {
        assert_eq!(def(n).category, Category::Equip, "{n} 应是装备");
    }
}
