//! 回合驱动 + 死亡 + 终局（生命周期）。
//!
//! 不变量（constitution I2 / ADR-0042 缺陷 6「生命周期」）：
//! 1. **死亡是显式结算**：`Seat.alive` 只由 `die` 置 false；弃牌 + 身份公开 + 胜负检查在同一结算内；
//! 2. 终局后 `Game.winner` 置位，拒绝一切写动作（M2.4 之后在引擎入口校验）。

use super::event::{Event, EventKind};
use super::identity;
use super::{Game, Identity, Zone};

/// 开局：发身份（座位 0 = 主公，其余按 `identities` 顺序）+ 发初始手牌（主公 6 张，其余 `hand_cards` 张）。
/// 主公 +2 张手牌：2026-11-13 用户裁定（对拍发现 v1 主公 = 6 张，确认为正确口径）。
pub fn begin_game(g: &mut Game, identities: &[Identity], hand_cards: usize) {
    for (i, seat) in g.seats.iter_mut().enumerate() {
        seat.identity = Some(if i == 0 { Identity::Lord } else { identities[i - 1] });
    }
    for seat in 0..g.seats.len() {
        let n = if seat == 0 { hand_cards + 2 } else { hand_cards };
        super::flow::draw(g, seat, n);
    }
}

/// 死亡确认结算：`alive = false` + 弃所有牌（手牌/装备/判定区）+ 触发 Death + 检查胜负。
pub fn die(g: &mut Game, seat: usize) -> Vec<Event> {
    let Some(s) = g.seats.get_mut(seat) else {
        return vec![];
    };
    if !s.alive {
        return vec![];
    }
    s.alive = false;
    // 弃所有牌：手牌/装备/判定区（判定区栈一并清空）。
    let owned: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| {
            z.seat() == Some(seat) && matches!(z, Zone::Hand(_) | Zone::Equip(..) | Zone::Judge(_))
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in owned {
        g.cards.insert(id, Zone::Discard);
    }
    g.judge_stacks.remove(&seat);
    // 胜负检查（单一入口）。
    g.winner = identity::check_winner(&g.seats);
    vec![Event::simple(EventKind::Death, Some(seat), None)]
}

/// 检查胜负并置位（每死亡/结算后调用）。
pub fn check_finished(g: &mut Game) -> Option<super::Winner> {
    if g.winner.is_none() {
        g.winner = identity::check_winner(&g.seats);
    }
    g.winner
}
