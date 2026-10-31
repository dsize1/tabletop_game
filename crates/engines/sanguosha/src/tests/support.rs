//! 三国杀测试公共助手（P4.4.2 批次 harness）。
//!
//! 目的：批次用例（`tests/p8x.rs`）此前各自复制 `give_card` / `give_def` / `run_phase` 等，
//! 每批 8 将 × 3 条用例会重复数百行样板。新批次请直接：
//!
//! ```ignore
//! use super::support::*;
//! ```
//!
//! 旧批次（p80–p88）保持原样，不强制迁移。

use super::take_card;
use crate::state::*;
use game_engine_core::GameManager;

/// 除 `cur` 外的存活座位（座位序）。
pub(super) fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != cur)
        .collect()
}

/// 从任意区域取一张满足条件的牌放入 `seat` 手牌，返回牌实例 id。
pub(super) fn give_card(
    st: &mut SgsState,
    seat: usize,
    pred: impl Fn(&CardInst) -> bool,
) -> String {
    let found = st
        .cards
        .iter()
        .find(|(id, c)| {
            pred(c)
                && !st.seats.iter().any(|s| s.hand.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.marks.values().flatten().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.judge_zone.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.equips.values().flatten().any(|h| h == *id))
        })
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌");
    take_card(st, &found);
    st.seats[seat].hand.push(found.clone());
    found
}

/// 把座位推进到出牌阶段并清空回合计数。
pub(super) fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

/// 按定义码与花色发牌（如黑桃【杀】、红桃【桃】）。
pub(super) fn give_def_suit(st: &mut SgsState, seat: usize, def: &str, suit: Suit) -> String {
    give_card(st, seat, |c| c.def == def && c.suit == suit)
}

/// 发一张指定槽位的装备牌并直接置入装备区。
pub(super) fn equip_slot_card(st: &mut SgsState, seat: usize, slot: EquipSlot) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(slot))
    });
    take_card(st, &card);
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

/// 按定义码发牌（如 `sha` / `wuzhong_shengyou`）。
pub(super) fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    give_card(st, seat, |c| c.def == def)
}

/// 按花色发牌。
pub(super) fn give_suit(st: &mut SgsState, seat: usize, suit: Suit) -> String {
    give_card(st, seat, |c| c.suit == suit)
}

/// 按点数发牌。
pub(super) fn give_rank(st: &mut SgsState, seat: usize, rank: u8) -> String {
    give_card(st, seat, |c| c.rank == rank)
}

/// 发一张装备牌（手牌，用于仁心/怀柔/甘露等）。
pub(super) fn give_equip(st: &mut SgsState, seat: usize) -> String {
    give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
    })
}

/// 按顺序把牌置于牌堆顶：`cards[0]` 最先被摸到。
pub(super) fn set_top(st: &mut SgsState, cards: &[String]) {
    for id in cards.iter().rev() {
        take_card(st, id);
        st.deck.draw.push(id.clone());
    }
}

/// 从牌堆/弃牌堆找点数/定义满足条件的牌 id（不发牌，仅用于牌堆顶布置）。
pub(super) fn find_cards(st: &SgsState, pred: impl Fn(&CardInst) -> bool, n: usize) -> Vec<String> {
    st.cards
        .iter()
        .filter(|(_, c)| pred(c))
        .map(|(id, _)| id.clone())
        .take(n)
        .collect()
}

/// 把座位推进到指定阶段并结算到出现 pending（或阶段等待操作）。
///
/// `clear_counters` 为 true 时清空回合计数器（跨回合复用状态时建议 true）。
pub(super) fn run_phase(
    mgr: &GameManager,
    room: &str,
    seat: usize,
    phase: TurnPhase,
    clear_counters: bool,
) {
    let full = mgr.get_state(room).expect("room");
    let mut st = SgsState::parse(&full.data).expect("parse");
    st.turn = seat;
    st.turn_phase = phase;
    if clear_counters {
        st.turn_counters.clear();
    }
    let mut ev = Vec::new();
    crate::turn::phase_step(&mut st, &mut ev).expect("phase_step");
    crate::resolve::advance(&mut st, &mut ev).expect("advance");
    let mut next = full.clone();
    next.data = serde_json::to_value(&st).expect("serialize");
    mgr.restore(next);
}

/// 造成伤害并立刻结算到 pending（伤害后技能用例的高频样板）。
pub(super) fn damage_and_advance(
    st: &mut SgsState,
    victim: usize,
    amount: i32,
    source: Option<usize>,
    dtype: crate::resolve::DamageType,
    card: Option<&str>,
) {
    crate::resolve::apply_damage(st, victim, amount, source, dtype, card);
    let mut ev = Vec::new();
    crate::resolve::advance(st, &mut ev).expect("advance");
}

/// 当前 pending 的技能 code（无 pending 时为 None）。
pub(super) fn pending_skill(st: &SgsState) -> Option<&str> {
    st.pending.as_ref().and_then(|p| p.skill.as_deref())
}

/// 按 id 前缀匹配 pending 选项（如 `targets:` / `choose:`）。
pub(super) fn pending_options_with(st: &SgsState, prefix: &str) -> Vec<String> {
    st.pending
        .as_ref()
        .map(|p| {
            p.options
                .iter()
                .filter(|o| o.id.starts_with(prefix))
                .map(|o| o.id.clone())
                .collect()
        })
        .unwrap_or_default()
}
