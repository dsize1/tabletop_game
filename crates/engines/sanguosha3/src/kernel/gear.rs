//! 装备管理 + 攻击范围/距离（单一入口）+ equip-skill 注入（O3=A）。
//!
//! 不变量（mechanics `distance-modifier` / `virtual-equip` / `equip-move`）：
//! 1. **距离只有一个计算入口**（`distance`），所有修正（进攻马/防御马）在此叠加，下限 1；
//! 2. **读装备只能走单一入口**（`armor_of` / `weapon_of` / `attack_range`），
//!    禁止各技能内联判断「有没有武器/防具」；
//! 3. **武器特效 = 特殊技能注入**（O3=A）：武器的效果由 `WeaponKind` 声明，
//!    内核/牌结算只读 `weapon_of`，不 `match` 武器名（青釭剑无视防具 / 连弩无次数 / 丈八转化…）。

use super::cards;
use super::{ArmorKind, Game, Slot, WeaponKind, Zone};

/// 座位某槽位的装备牌 id。
pub fn equip_in(g: &Game, seat: usize, slot: Slot) -> Option<&String> {
    g.cards
        .iter()
        .find(|(_, z)| matches!(z, Zone::Equip(s, sl) if *s == seat && *sl == slot))
        .map(|(id, _)| id)
}

/// 装备进槽位：同槽旧装备进弃牌堆；返回被顶掉的旧牌 id（无旧牌 = None）。
pub fn equip(g: &mut Game, seat: usize, card: &str) -> Option<String> {
    let code = g.card_defs.get(card)?.def.clone();
    let def = cards::def(&code)?;
    let slot = def.slot?;
    let old = equip_in(g, seat, slot).cloned();
    if let Some(o) = &old {
        g.cards.insert(o.clone(), Zone::Discard);
    }
    g.cards.insert(card.to_string(), Zone::Equip(seat, slot));
    old
}

/// 防具效果（单一入口）。
pub fn armor_of(g: &Game, seat: usize) -> ArmorKind {
    equip_in(g, seat, Slot::Armor)
        .and_then(|id| g.card_defs.get(id))
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.armor)
        .unwrap_or(ArmorKind::None)
}

/// 武器效果（单一入口）。
pub fn weapon_of(g: &Game, seat: usize) -> WeaponKind {
    equip_in(g, seat, Slot::Weapon)
        .and_then(|id| g.card_defs.get(id))
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.weapon)
        .unwrap_or(WeaponKind::None)
}

/// 攻击范围（默认 1 + 武器范围）。
pub fn attack_range(g: &Game, seat: usize) -> u8 {
    equip_in(g, seat, Slot::Weapon)
        .and_then(|id| g.card_defs.get(id))
        .and_then(|c| cards::def(&c.def))
        .map(|d| 1 + d.weapon_range.unwrap_or(0))
        .unwrap_or(1)
}

/// 某座位是否有某坐骑。
fn has_horse(g: &Game, seat: usize, slot: Slot) -> bool {
    equip_in(g, seat, slot).is_some()
}

/// 距离（座位环 + 进攻马 -1 + 目标防御马 +1，下限 1；自己到自己的距离 = 0）。
pub fn distance(g: &Game, from: usize, to: usize) -> usize {
    let n = g.seats.len();
    if from == to || n <= 1 {
        return 0;
    }
    let a = (from as i32 - to as i32).abs() as usize;
    let raw = a.min(n - a);
    let mut d = raw as i32;
    if has_horse(g, from, Slot::HorseOffense) {
        d -= 1;
    }
    if has_horse(g, to, Slot::HorseDefense) {
        d += 1;
    }
    d.max(1) as usize
}
