//! 体力结算唯一出口：伤害 / 失去体力 / 回血 / 体力上限变化。
//!
//! 不变量（constitution §I2 / mechanics `damage-vs-hp-loss` / `damage-attributes`）：
//! 1. **伤害 ≠ 失去体力 ≠ 体力上限变化**：三类动作各有独立入口与事件，
//!    禁止复用 `damage` 伪造 `lose_hp`（反之亦然）；
//! 2. 伤害事件必须带**二维**（`DamageKind` 属性 × `SourceKind` 来源）——
//!    藤甲「普通杀无效」= `Slash + Normal`，火杀 = `Slash + Fire`（+1），雷杀 = `Slash + Thunder`（正常）；
//! 3. 数值链顺序：技能加成 → 藤甲 → 白银狮子**最后**（>1 改为 1）；
//! 4. 体力不得低于 0（濒死是 `resolve` 层职责，flow 只钳到 0）。

use super::event::{Event, EventKind};
use super::{ArmorKind, DamageOutcome, DamageSpec, DamageKind, Game, SourceKind};

/// 防具伤害链：技能加成已并入 `amount`，这里只做防具（藤甲 → 白银狮子）。
///
/// - 藤甲：普通杀无效（含转化普杀）；南蛮/万箭无效；火 +1；雷正常；其他正常。
/// - 白银狮子：最后结算，>1 改为 1。
pub fn apply_armor(armor: ArmorKind, spec: &DamageSpec, mut amount: i32) -> DamageOutcome {
    match armor {
        ArmorKind::Tengjia => {
            if spec.is_normal_slash()
                || matches!(spec.source, SourceKind::Nanman | SourceKind::Wanjian)
            {
                return DamageOutcome::Blocked;
            }
            if spec.kind == DamageKind::Fire {
                amount += 1;
            }
        }
        ArmorKind::Baiyin => {}
        ArmorKind::None => {}
    }
    // 白银狮子最后（恒为 1）。
    if armor == ArmorKind::Baiyin && amount > 1 {
        amount = 1;
    }
    DamageOutcome::Dealt(amount)
}

/// 造成伤害（唯一出口）。`bonus` = 技能加成（链祸/暗箭/咆哮…，M2 skills 接入）。
///
/// 返回产生的事件（可能为空 = 被免疫）。`DamageAfter` 事件带二维 `damage` 字段。
pub fn damage(g: &mut Game, spec: DamageSpec, armor: ArmorKind, bonus: i32) -> Vec<Event> {
    let base = spec.amount + bonus;
    match apply_armor(armor, &spec, base) {
        DamageOutcome::Blocked => vec![],
        DamageOutcome::Dealt(n) => {
            let seat = match g.seats.get_mut(spec.target) {
                Some(s) => s,
                None => return vec![],
            };
            seat.hp = (seat.hp - n).max(0);
            let mut ev = Event::simple(EventKind::DamageAfter, Some(spec.target), None);
            ev.amount = Some(n);
            ev.by = spec.from;
            ev.damage = Some(DamageSpec {
                from: spec.from,
                target: spec.target,
                amount: n,
                kind: spec.kind,
                source: spec.source,
            });
            vec![ev]
        }
    }
}

/// 失去（流失）体力：**不是伤害**，不产生 `DamageAfter`（mechanics `damage-vs-hp-loss`）。
pub fn lose_hp(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    seat.hp = (seat.hp - amount).max(0);
    let mut ev = Event::simple(EventKind::HpLost, Some(target), Some(cause));
    ev.amount = Some(amount);
    vec![ev]
}

/// 回复体力（唯一出口）：钳到 `max_hp`，满血不回（但不报错）。
pub fn heal(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    let healed = (seat.max_hp - seat.hp).min(amount).max(0);
    seat.hp = (seat.hp + healed).min(seat.max_hp);
    let mut ev = Event::simple(EventKind::Healed, Some(target), Some(cause));
    ev.amount = Some(healed);
    vec![ev]
}

/// 体力上限变化（第三类动作，非伤害非失去体力）：减上限时 hp 一并钳到新上限。
pub fn lose_max_hp(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    seat.max_hp = (seat.max_hp - amount).max(0);
    seat.hp = seat.hp.min(seat.max_hp);
    let mut ev = Event::simple(EventKind::MaxHpChanged, Some(target), Some(cause));
    ev.amount = Some(amount);
    vec![ev]
}
