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

/// 胜负判定（rules §3 / rules-rulings §2，用户 2026-11-14 口径细化）：
/// 1. **内奸胜**：主公死亡时场上**仅存活主公与内奸**（其余角色全部已死）→ 内奸胜；
///    双内局同理，只能有一名内奸达成（两名内奸同时存活 → 仍判反贼胜）；
/// 2. 其余「主公死亡」情形 → **反贼胜**（主公先于其他角色死亡，不区分是否有存活反贼）；
/// 3. 反贼 + 内奸全灭 → 主忠胜（主公存活）。
pub fn check_winner(seats: &[Seat]) -> Option<Winner> {
    if let Some(l) = seats.iter().find(|s| s.identity == Some(Identity::Lord)) {
        if !l.alive {
            // 主公已死：存活者只剩「一名内奸」= 内奸达成（除主公外其他玩家全部死亡后杀主）。
            let alive: Vec<&Seat> = seats.iter().filter(|s| s.alive).collect();
            if alive.len() == 1 && alive[0].identity == Some(Identity::Spy) {
                return Some(Winner::Spy);
            }
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
