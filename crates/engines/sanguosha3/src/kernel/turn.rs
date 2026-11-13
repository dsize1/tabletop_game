//! 回合驱动 + 死亡 + 终局（生命周期）。
//!
//! 不变量（constitution I2 / ADR-0042 缺陷 6「生命周期」）：
//! 1. **死亡是显式结算**：`Seat.alive` 只由 `die` 置 false；弃牌 + 身份公开 + 胜负检查在同一结算内；
//! 2. 终局后 `Game.winner` 置位，拒绝一切写动作（M2.4 之后在引擎入口校验）。

use super::event::{Event, EventKind};
use super::identity;
use super::{Game, Identity, Zone};

/// 开局：发身份（座位 0 = 主公，其余按 `identities` 顺序）+ 发初始手牌（每人 `hand_cards` 张）。
///
/// 主公**只 +1 体力上限**，初始手牌与其他人一致（官方规则 + v1/v2 一致）。
/// 2026-11-13 曾根据「v1 对拍主公=6」裁定 V3 主公 +2 张；2026-11-13 B1 对拍复核发现
/// 该 6 = **4 张初始手牌 + 摸牌阶段 2 张**（口径错位：v1 侧在出牌阶段、v3 侧在开局），
/// v1/v2 的 `initial_hand` 均为 4 → 本处回正（见 `v3-parity-baseline.md` §5.1）。
pub fn begin_game(g: &mut Game, identities: &[Identity], hand_cards: usize) {
    for (i, seat) in g.seats.iter_mut().enumerate() {
        seat.identity = Some(if i == 0 { Identity::Lord } else { identities[i - 1] });
    }
    for seat in 0..g.seats.len() {
        super::flow::draw(g, seat, hand_cards);
    }
    // 游戏开始钩子（怀橘 3 橘等）。**在发牌之后**，且不产生 Setup 事件（明任教训）。
    super::skills::fire_game_start(g);
}

/// 死亡确认结算（无击杀者上下文；保留给“非伤害死亡”与测试）。
pub fn die(g: &mut Game, seat: usize) -> Vec<Event> {
    settle_death(g, seat, None)
}

/// **通用死亡结算（唯一出口）**：弃所有牌 → 奖惩 → 胜负（constitution I2 / `death-penalty`）。
///
/// 奖惩（rules-rulings §3）：杀死反贼 → 击杀者摸 3；主公杀死忠臣 → 主公弃所有手牌与装备区牌。
/// 濒死解除后才调用（由 `FlowKind::Dying` 在无人出桃时调用）。
pub fn settle_death(g: &mut Game, seat: usize, killer: Option<usize>) -> Vec<Event> {
    let Some(s) = g.seats.get_mut(seat) else {
        return vec![];
    };
    if !s.alive {
        return vec![];
    }
    let victim_identity = s.identity;
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

    let evs = vec![Event::simple(EventKind::Death, Some(seat), killer.map(|_| "伤害"))];
    // 奖惩：只在击杀者存活且不是自己时结算。
    if let Some(k) = killer.filter(|&k| k != seat && g.seats.get(k).map(|s| s.alive).unwrap_or(false)) {
        match victim_identity {
            // 杀死反贼：摸 3 张。
            Some(Identity::Rebel) => {
                super::flow::draw(g, k, 3);
            }
            // 主公杀死忠臣：弃置所有手牌与装备区牌。
            Some(Identity::Loyalist) if g.seats[k].identity == Some(Identity::Lord) => {
                let cards: Vec<String> = g
                    .cards
                    .iter()
                    .filter(|(_, z)| {
                        z.seat() == Some(k) && matches!(z, Zone::Hand(_) | Zone::Equip(..))
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in cards {
                    super::flow::discard(g, k, &id, "主公杀忠臣");
                }
            }
            _ => {}
        }
    }
    // 胜负检查（单一入口）。
    g.winner = identity::check_winner(&g.seats);
    evs
}

/// 检查胜负并置位（每死亡/结算后调用）。
pub fn check_finished(g: &mut Game) -> Option<super::Winner> {
    if g.winner.is_none() {
        g.winner = identity::check_winner(&g.seats);
    }
    g.winner
}
