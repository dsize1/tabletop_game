//! 身份与胜负（I2 状态语义的「身份公开」部分）。
//!
//! 不变量（constitution I2 / rules §2–§3）：
//! 1. 身份公开 = 主公（公开），或 `alive == false` 之后；
//! 2. 存活/死亡由 `Seat.alive` 显式决定，不由 hp 推导。

use super::{Identity, Seat, Winner};

/// 身份是否公开（主公 / 已死亡）。
pub fn identity_public(seat: &Seat) -> bool {
    match seat.identity {
        Some(Identity::Lord) => true,
        _ => !seat.alive,
    }
}

/// 身份是否对某观察者可见（含「自己永远知道自己身份」）。
pub fn identity_visible(seat: &Seat, viewer: usize) -> bool {
    seat.id == viewer || identity_public(seat)
}

/// 胜负判定（rules §3）：主公先死 → 反贼胜；反贼+内奸全灭 → 主忠胜。
/// 内奸单独胜利（最后 1v1 杀主）在 M2.4 之后细化。
pub fn check_winner(seats: &[Seat]) -> Option<Winner> {
    let lord = seats.iter().find(|s| s.identity == Some(Identity::Lord));
    // 主公死亡 → 反贼胜（不要求清场）。
    if let Some(l) = lord {
        if !l.alive {
            return Some(Winner::Rebel);
        }
    }
    // 反贼 + 内奸全灭 → 主忠胜（主公存活）。
    let rebel_alive = seats.iter().any(|s| s.identity == Some(Identity::Rebel) && s.alive);
    let spy_alive = seats.iter().any(|s| s.identity == Some(Identity::Spy) && s.alive);
    if !rebel_alive && !spy_alive {
        return Some(Winner::Lord);
    }
    None
}
