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
    // 已废除的装备栏不可再装备（决堰/破势/雄乱）。
    if g.slot_abolished(seat, slot) {
        return None;
    }
    let old = equip_in(g, seat, slot).cloned();
    if let Some(o) = &old {
        g.cards.insert(o.clone(), Zone::Discard);
    }
    g.cards.insert(card.to_string(), Zone::Equip(seat, slot));
    old
}

/// 防具效果（单一入口）：**真实装备优先**；无真实防具时才读虚拟装备（八阵）。
pub fn armor_of(g: &Game, seat: usize) -> ArmorKind {
    if let Some(id) = equip_in(g, seat, Slot::Armor) {
        return g
            .card_defs
            .get(id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.armor)
            .unwrap_or(ArmorKind::None);
    }
    virtual_card(g, seat, Slot::Armor)
        .and_then(cards::def)
        .map(|d| d.armor)
        .unwrap_or(ArmorKind::None)
}

/// 武器效果（单一入口）：真实装备优先，其次虚拟装备。
pub fn weapon_of(g: &Game, seat: usize) -> WeaponKind {
    if let Some(id) = equip_in(g, seat, Slot::Weapon) {
        return g
            .card_defs
            .get(id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.weapon)
            .unwrap_or(WeaponKind::None);
    }
    virtual_card(g, seat, Slot::Weapon)
        .and_then(cards::def)
        .map(|d| d.weapon)
        .unwrap_or(WeaponKind::None)
}

/// 攻击范围（默认 1 + 武器范围；虚拟武器同样参与）。
pub fn attack_range(g: &Game, seat: usize) -> u8 {
    if let Some(id) = equip_in(g, seat, Slot::Weapon) {
        return g
            .card_defs
            .get(id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| 1 + d.weapon_range.unwrap_or(0))
            .unwrap_or(1);
    }
    virtual_card(g, seat, Slot::Weapon)
        .and_then(cards::def)
        .map(|d| 1 + d.weapon_range.unwrap_or(0))
        .unwrap_or(1)
}

/// 虚拟装备牌定义 code（mechanics `virtual-equip`；不占槽、不参与牌数守恒）。
pub fn virtual_card(g: &Game, seat: usize, slot: Slot) -> Option<&'static str> {
    if equip_in(g, seat, slot).is_some() {
        return None;
    }
    super::skills::virtual_equip(g, seat, slot)
}

/// 装备移动（mechanics `equip-move`）：场上一张装备移到另一名角色的同槽位（目标槽必须为空）。
///
/// 移动 ≠ 弃置，也不属于「获得」：只发 `CardLost`（离开原持有者装备区）+ 失去装备钩子。
pub fn move_equip(g: &mut Game, from: usize, card: &str, to: usize) -> Result<(), String> {
    let zone = g
        .cards
        .get(card)
        .cloned()
        .ok_or_else(|| format!("牌 {card} 不存在"))?;
    let Zone::Equip(owner, slot) = zone else {
        return Err("只能移动装备区的牌".into());
    };
    if owner != from {
        return Err("来源座位与装备归属不一致".into());
    }
    if to == from {
        return Err("不能移回原持有者".into());
    }
    if equip_in(g, to, slot).is_some() {
        return Err("目标槽位已有装备".into());
    }
    g.cards.insert(card.to_string(), Zone::Equip(to, slot));
    let mut lost = super::event::Event::card(
        super::event::EventKind::CardLost,
        Some(from),
        card.to_string(),
        Some("装备移动".to_string()),
    );
    lost.by = Some(from);
    lost.from = Some(from);
    let ev = lost.snapshot(g);
    g.push_event(ev);
    Ok(())
}

/// **手牌装备置入他人装备区（K17；直谏/窃听）**：替换旧装备（进弃牌堆 + 失去装备钩子）。
///
/// 与 `move_equip` 区别：来源是**手牌**、目标槽位**可被替换**（旧装备进弃牌堆）。
/// 返回被替换的旧装备 id（无则 None）。
pub fn place_from_hand(
    g: &mut Game,
    by: usize,
    card: &str,
    to: usize,
) -> Result<Option<String>, String> {
    let zone = g
        .cards
        .get(card)
        .cloned()
        .ok_or_else(|| format!("牌 {card} 不存在"))?;
    if !matches!(zone, Zone::Hand(s) if s == by) {
        return Err("只能把该角色手牌中的装备置入装备区".into());
    }
    let def = g
        .card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
        .ok_or_else(|| format!("牌 {card} 无定义"))?;
    let Some(slot) = def.slot else {
        return Err("该牌不是装备牌".into());
    };
    if g.slot_abolished(to, slot) {
        return Err("目标装备栏已被废除".into());
    }
    super::flow::note_lost(g, by, card, false);
    let mut lost = super::event::Event::card(
        super::event::EventKind::CardLost,
        Some(by),
        card.to_string(),
        Some("置入装备区".to_string()),
    );
    lost.by = Some(by);
    lost.from = Some(by);
    let ev = lost.snapshot(g);
    g.push_event(ev);
    let old = equip(g, to, card);
    if let Some(o) = &old {
        super::flow::note_lost(g, to, o, true);
        super::flow::record_to_discard(g, o, "装备被替换", Some(by));
        let mut lost = super::event::Event::card(
            super::event::EventKind::CardLost,
            Some(to),
            o.clone(),
            Some("装备被替换".to_string()),
        );
        lost.by = Some(by);
        lost.from = Some(to);
        let ev = lost.snapshot(g);
        g.push_event(ev);
        let mut discarded = super::event::Event::card(
            super::event::EventKind::CardDiscarded,
            Some(to),
            o.clone(),
            Some("装备被替换".to_string()),
        );
        discarded.by = Some(by);
        discarded.from = Some(to);
        let ev = discarded.snapshot(g);
        g.push_event(ev);
        let _ = super::flow::leave_equip(g, to, o, "装备被替换");
    }
    Ok(old)
}

/// **交换两名角色的装备区（K17；甘露）**：逐槽交换；双方都触发失去装备钩子。
pub fn swap_zones(g: &mut Game, a: usize, b: usize) -> Result<(), String> {
    if a == b {
        return Err("不能与自己交换装备区".into());
    }
    let ea = equips_of(g, a);
    let eb = equips_of(g, b);
    // 先快照槽位，再逐张移动（避免边遍历边改）。
    let mut moves: Vec<(String, usize, Slot)> = Vec::new();
    for id in &ea {
        if let Some(Zone::Equip(_, slot)) = g.cards.get(id) {
            moves.push((id.clone(), b, *slot));
        }
    }
    for id in &eb {
        if let Some(Zone::Equip(_, slot)) = g.cards.get(id) {
            moves.push((id.clone(), a, *slot));
        }
    }
    for (id, to, slot) in moves {
        let owner = g.cards.get(&id).and_then(|z| z.seat()).unwrap_or(a);
        g.cards.insert(id.clone(), Zone::Equip(to, slot));
        let mut lost = super::event::Event::card(
            super::event::EventKind::CardLost,
            Some(owner),
            id.clone(),
            Some("装备区交换".to_string()),
        );
        lost.by = Some(owner);
        lost.from = Some(owner);
        let ev = lost.snapshot(g);
        g.push_event(ev);
        let _ = super::flow::leave_equip(g, owner, &id, "装备区交换");
        let mut gained = super::event::Event::card(
            super::event::EventKind::CardGained,
            Some(to),
            id,
            Some("装备区交换".to_string()),
        );
        gained.by = Some(to);
        gained.from = Some(owner);
        let ev = gained.snapshot(g);
        g.push_event(ev);
    }
    Ok(())
}

/// 某座位是否有某坐骑。
fn has_horse(g: &Game, seat: usize, slot: Slot) -> bool {
    equip_in(g, seat, slot).is_some()
}

/// 距离（座位环 + 进攻马 -1 + 目标防御马 +1 + 技能修正，下限 1；自己到自己的距离 = 0）。
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
    // 技能距离修正（马术 -1）：`Hook::Distance` 单出口。
    d += super::skills::distance_modifier(g, from) + super::skills::distance_bonus(g, from);
    // 目标侧距离修正（义从：体力 ≤2 时他人到其 +1）。
    d += super::skills::distance_to_bonus(g, to);
    d.max(1) as usize
}

/// 「能否打到」单入口（距离 ≤ 攻击范围，或技能声明视为在范围内：诏缚）。
pub fn within_attack_range(g: &Game, from: usize, to: usize, reach: usize) -> bool {
    if from == to {
        return false;
    }
    distance(g, from, to) <= reach || super::skills::in_attack_range(g, from, to)
}

/// 使用【杀】是否无视目标防具（青釭剑）。
pub fn ignores_armor(g: &Game, seat: usize) -> bool {
    weapon_of(g, seat) == WeaponKind::Qinggang
}

/// 装备区全部牌（按牌 id 排序 = 确定性）。
pub fn equips_of(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

/// 坐骑（进攻/防御）牌 id（麒麟弓「弃置其一张坐骑牌」唯一查询）。
pub fn horses_of(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| {
            matches!(z, Zone::Equip(s, Slot::HorseOffense) | Zone::Equip(s, Slot::HorseDefense) if *s == seat)
        })
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}
