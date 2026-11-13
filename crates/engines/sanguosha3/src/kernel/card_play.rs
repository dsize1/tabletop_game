//! 牌结算单一流程：使用 → 目标 → 响应 → 无懈 → 结算 → 弃牌堆。
//!
//! 不变量（mechanics `use-vs-play` / `target-list` / `response-cards` / `no-wuxie`）：
//! 1. **使用 ≠ 打出**：「使用」= 主动进入结算；「打出」= 响应提供牌（不进结算）。
//!    闪只能响应、不能主动使用；杀在响应里是「打出」，不计入使用次数。
//! 2. **目标列表**：每次使用记录 `targets: Vec<seat>`，技能只读列表，不按牌名反推目标模式。
//! 3. **响应牌**读 `CardDef.tags`（`responds_as`），不按牌名枚举。
//! 4. **无懈链**：奇偶层判定（奇数 = 锦囊被抵消）；「不能被无懈」是牌属性的独立事实。

use super::cards::{CardDef, ResponseKind, valid_target};
use super::event::Event;
use super::flow;
use super::{Game, Zone};

/// 出牌阶段可主动使用的手牌（闪/桃满血等被排除；响应牌不出现在主动使用里）。
///
/// `defs` = 牌定义表查表（生成物；内核不按牌名字面量判定）。
pub fn legal_uses(g: &Game, seat: usize, defs: &dyn Fn(&str) -> Option<CardDef>) -> Vec<String> {
    let mut out = vec![];
    for (id, zone) in &g.cards {
        if *zone != Zone::Hand(seat) {
            continue;
        }
        let Some(c) = g.card_defs.get(id) else { continue };
        let Some(d) = defs(&c.def) else { continue };
        // 闪不能主动使用（只能响应）；桃满血不能使用。
        if d.tags.shan {
            continue;
        }
        if d.tags.tao && g.seats[seat].hp >= g.seats[seat].max_hp {
            continue;
        }
        out.push(id.clone());
    }
    out
}

/// 使用【桃】→ 回血（唯一出口 `flow::heal`，满血回 0）。
pub fn resolve_tao(g: &mut Game, seat: usize) -> Vec<Event> {
    flow::heal(g, seat, 1, "桃")
}

/// 无懈链：奇数层 = 锦囊被抵消（`layers` = 已打出的无懈张数）。
pub fn wuxie_cancelled(layers: usize) -> bool {
    layers % 2 == 1
}

/// 一张牌能否指定该目标（默认规则；技能覆盖走钩子，M2 接入）。
pub fn can_target(def: &CardDef, self_seat: usize, target: usize, alive: bool) -> bool {
    valid_target(def.target_mode, self_seat, target, alive)
}

/// 牌进弃牌堆（唯一出口，`lose-vs-discard`：弃置是离开手牌/装备区进弃牌堆）。
pub fn to_discard(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), Zone::Discard);
}

/// 响应牌判定：读标签，不按牌名。
pub fn can_respond(def: &CardDef, kind: ResponseKind) -> bool {
    def.responds_as(kind)
}
