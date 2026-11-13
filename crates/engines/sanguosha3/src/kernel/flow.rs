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
use super::{ArmorKind, CardId, DamageOutcome, DamageSpec, DamageKind, Game, SourceKind};

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
        // 八卦阵（响应拦截）/仁王盾（黑色杀拦截）不在伤害链，拦截走 M2.3 gear 的响应钩子。
        ArmorKind::Bagua | ArmorKind::Renwang | ArmorKind::None => {}
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
    // 锁定技能防伤（怀橘：移除 1 枚「橘」防止该次伤害）——技能防伤先于防具数值链。
    if super::skills::prevent_damage(g, spec.target) {
        return vec![];
    }
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
            enter_dying(g, spec.target, spec.from);
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
    enter_dying(g, target, None);
    vec![ev]
}

/// **濒死入口（唯一）**：hp ≤ 0 且存活 → 压入通用 `FlowKind::Dying` 帧。
///
/// 同一座位同时只允许一个濒死帧（重复伤害/多段结算不得叠帧）。
pub fn enter_dying(g: &mut Game, seat: usize, by: Option<usize>) {
    let dying = g
        .seats
        .get(seat)
        .map(|s| s.alive && s.hp <= 0)
        .unwrap_or(false);
    if !dying {
        return;
    }
    let already = g.stack.iter().any(|f| {
        matches!(
            &f.inner,
            super::resolve::FrameInner::Flow(ff)
                if ff.kind == super::resolve::FlowKind::Dying
                    && ff.data.get("target").and_then(|v| v.as_u64()) == Some(seat as u64)
        )
    });
    if already {
        return;
    }
    super::resolve::push_flow(
        g,
        super::resolve::FlowKind::Dying,
        seat,
        super::resolve::Stage::DyingAsk,
        serde_json::json!({ "target": seat, "by": by, "idx": 0 }),
    );
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

/// 濒死判定（constitution I2）：hp ≤ 0 且仍存活 = 濒死（等待救援，**不是死亡**）。
pub fn is_dying(seat: &super::Seat) -> bool {
    seat.alive && seat.hp <= 0
}

/// 濒死救援（唯一入口，走 `heal` 语义）：桃/酒从 hp ≤ 0 回血；hp > 0 即脱离濒死。
pub fn rescue(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    heal(g, target, amount, cause)
}

/// 摸牌：从牌堆顶抽 n 张进手牌（牌堆不足时摸到几张某）。
///
/// **摸牌方向单出口**：寸目（锁定）改从牌堆底摸；所有摸牌（摸牌阶段/技能摸牌）都走这里。
pub fn draw(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    let bottom = super::skills::draw_from_bottom(g, seat);
    let mut drawn = vec![];
    for _ in 0..n {
        let id = if bottom {
            if g.deck.is_empty() {
                None
            } else {
                Some(g.deck.remove(0))
            }
        } else {
            g.deck.pop()
        };
        let Some(id) = id else { break };
        g.cards.insert(id.clone(), super::Zone::Hand(seat));
        drawn.push(id);
    }
    drawn
}

/// **摸牌阶段单出口**：基础 2 张 + 技能加成（庸肆 +X / 怀橘 +1）；整论可整段跳过。
pub fn draw_phase(g: &mut Game, seat: usize) -> Vec<CardId> {
    if super::skills::skip_draw(g, seat) {
        return vec![];
    }
    let n = 2 + super::skills::draw_bonus(g, seat);
    draw(g, seat, n)
}

/// 弃置（唯一出口，`lose-vs-discard`）：只能从手牌/装备区弃置（判定区离开**不算弃置**）。
///
/// 记账：进弃牌堆 + 本阶段弃牌计数（`phase.discarded.<Phase>`，溃诛 X 的唯一来源）+ 事件（供订阅路由）。
pub fn discard(g: &mut Game, by: usize, id: &str, cause: &str) -> Option<Event> {
    let zone = g.cards.get(id)?.clone();
    let owner = zone.seat()?;
    if !matches!(zone, super::Zone::Hand(_) | super::Zone::Equip(..)) {
        return None;
    }
    g.cards.insert(id.to_string(), super::Zone::Discard);
    let key = format!("phase.discarded.{:?}", g.phase);
    g.bump(&key);
    let mut ev = Event::card(EventKind::CardDiscarded, Some(owner), id.to_string(), Some(cause.to_string()));
    ev.by = Some(by);
    ev.from = Some(owner);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.events.push(ev.clone());
    Some(ev)
}

/// 获得（唯一出口）：牌进手牌；事件记「谁失去 → 谁获得」。
pub fn gain(g: &mut Game, to: usize, id: &str, by: usize, cause: &str) -> Option<Event> {
    let from = g.cards.get(id)?.seat();
    g.cards.insert(id.to_string(), super::Zone::Hand(to));
    let mut ev = Event::card(EventKind::CardGained, Some(to), id.to_string(), Some(cause.to_string()));
    ev.by = Some(by);
    ev.from = from;
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.events.push(ev.clone());
    Some(ev)
}

/// 置于牌堆顶（恃才）：进 Draw 区并压到牌堆顶。
pub fn to_deck_top(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), super::Zone::Draw);
    g.deck.push(id.to_string());
}

/// 手牌摸至 n 张（审时「摸至 4 张」）：只摸不弃（超过 n 不处理）。
pub fn fill_hand(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    let cur = g.hand_size(seat);
    if cur >= n {
        return vec![];
    }
    draw(g, seat, n - cur)
}
