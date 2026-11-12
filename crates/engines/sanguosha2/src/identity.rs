//! 身份局（S1）：配比 / 奖惩 / 胜负。
//!
//! 口径（与 v1「标准身份配比」一致，见 `docs/games/sanguosha/rules.md` §8）：
//! - 5 人 1 主 1 忠 2 反 1 内；6 人 1 主 1 忠 3 反 1 内；7 人 1 主 2 忠 3 反 1 内；
//!   8 人 1 主 2 忠 4 反 1 内；8 人双内 1 主 1 忠 4 反 2 内（OQ8：保留开关）。
//! - 主公体力上限 +1；主公先手。
//! - 死亡奖惩：杀死反贼→摸 3 张；主公杀死忠臣→弃置全部手牌与装备。
//! - 胜负：主公死 → 反贼胜（场上仅剩内奸则内奸胜）；反贼与内奸全灭 → 主公（含忠臣）胜。
//!
//! **只在身份局生效**：`Game.identity_game` 为 false 时（单测夹具）本条规则完全不参与，
//! 否则夹具默认身份 `rebel` 会误触发摸牌/终局。

use crate::kernel::{Game, Zone};

pub const LORD: &str = "lord";
pub const LOYALIST: &str = "loyalist";
pub const REBEL: &str = "rebel";
pub const TRAITOR: &str = "traitor";

/// 标准身份配比（未洗牌，顺序固定便于测试）。
pub fn identity_list(player_count: usize, double_spy: bool) -> Result<Vec<&'static str>, String> {
    use {LOYALIST as L, LORD as O, REBEL as R, TRAITOR as T};
    let ids = match (player_count, double_spy) {
        (5, false) => vec![O, L, R, R, T],
        (6, false) => vec![O, L, R, R, R, T],
        (7, false) => vec![O, L, L, R, R, R, T],
        (8, false) => vec![O, L, L, R, R, R, R, T],
        (8, true) => vec![O, L, R, R, R, R, T, T],
        _ => {
            return Err(format!(
                "人数不合法：{player_count}（身份局 5–8 人；双内仅支持 8 人）"
            ))
        }
    };
    Ok(ids)
}

/// 主公座位（无主公时 None）。
pub fn lord_seat(g: &Game) -> Option<usize> {
    g.seats.iter().position(|s| s.identity == LORD)
}

/// 把身份洗牌后分给座位，返回主公座位。
pub fn assign(g: &mut Game, double_spy: bool) -> Result<usize, String> {
    let n = g.seats.len();
    let mut ids = identity_list(n, double_spy)?;
    shuffle(g, &mut ids);
    for (seat, id) in g.seats.iter_mut().zip(ids.into_iter()) {
        seat.identity = id.to_string();
        seat.flags.remove(LORD);
    }
    let lord = lord_seat(g).ok_or_else(|| "未分配主公身份".to_string())?;
    g.seats[lord].flags.insert(LORD.to_string());
    Ok(lord)
}

/// 身份死亡奖惩（只在身份局生效）。
pub fn on_death(g: &mut Game, victim: usize, source: Option<usize>) {
    if !g.identity_game {
        return;
    }
    let Some(killer) = source.filter(|s| *s != victim) else {
        return;
    };
    if !g.is_alive(killer) {
        return;
    }
    match g.seats[victim].identity.as_str() {
        REBEL => {
            let name = g.seats[killer].name.clone();
            let got = g.draw(killer, 3);
            g.push_log(format!(
                "{} 杀死反贼，摸 {} 张牌",
                name,
                got.len()
            ));
        }
        LOYALIST if g.seats[killer].identity == LORD => {
            // 主公杀死忠臣：弃置所有手牌与装备。
            let ids: Vec<String> = g
                .ids_in(|z| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == killer));
            for id in ids {
                let _ = g.to_discard(&id, "lord_kill_loyalist");
            }
            let name = g.seats[killer].name.clone();
            g.push_log(format!("{} 误杀忠臣，弃置所有手牌与装备", name));
        }
        _ => {}
    }
}

/// 胜负判定（只在身份局生效）。
pub fn check_finished(g: &mut Game) {
    if g.finished || !g.identity_game {
        return;
    }
    let Some(lord) = lord_seat(g) else {
        return;
    };
    let alive: Vec<usize> = (0..g.seats.len()).filter(|i| g.is_alive(*i)).collect();
    // I2：主公**死亡确认**（不是濒死）才判负。
    if !g.is_alive(lord) {
        g.finished = true;
        if alive.len() == 1 && g.seats[alive[0]].identity == TRAITOR {
            g.winner = Some(TRAITOR.to_string());
            g.push_log("主公阵亡且仅剩内奸：内奸获胜");
        } else {
            g.winner = Some(REBEL.to_string());
            g.push_log("主公阵亡：反贼获胜");
        }
        return;
    }
    // 主公健在：反贼与内奸全灭 → 主公方胜。
    let enemies_alive = alive
        .iter()
        .any(|i| matches!(g.seats[*i].identity.as_str(), REBEL | TRAITOR));
    if !enemies_alive {
        g.finished = true;
        g.winner = Some(LORD.to_string());
        g.push_log("反贼与内奸全灭：主公方获胜");
    }
}

/// 确定性洗牌（走 `Game::rand_below`，与技能随机源同源、可序列化）。
pub fn shuffle<T>(g: &mut Game, items: &mut [T]) {
    let n = items.len();
    if n < 2 {
        return;
    }
    for i in (1..n).rev() {
        let j = g.rand_below(i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{Game, Seat};

    fn fixture(n: usize) -> Game {
        let seats = (0..n)
            .map(|i| Seat::new(&format!("p{i}"), "", &[], 4))
            .collect();
        Game::new(seats, crate::cards::deck())
    }

    #[test]
    fn identity_table_matches_standard() {
        let counts = |n: usize, d: bool| {
            let ids = identity_list(n, d).unwrap();
            let mut out = (0, 0, 0, 0);
            for id in ids {
                match id {
                    LORD => out.0 += 1,
                    LOYALIST => out.1 += 1,
                    REBEL => out.2 += 1,
                    _ => out.3 += 1,
                }
            }
            out
        };
        assert_eq!(counts(5, false), (1, 1, 2, 1));
        assert_eq!(counts(6, false), (1, 1, 3, 1));
        assert_eq!(counts(7, false), (1, 2, 3, 1));
        assert_eq!(counts(8, false), (1, 2, 4, 1));
        assert_eq!(counts(8, true), (1, 1, 4, 2));
        assert!(identity_list(4, false).is_err());
        assert!(identity_list(5, true).is_err());
    }

    #[test]
    fn assign_is_deterministic_and_has_one_lord() {
        let mut g = fixture(5);
        let lord = assign(&mut g, false).unwrap();
        assert_eq!(g.seats[lord].identity, LORD);
        assert!(g.seats[lord].flags.contains(LORD));
        assert_eq!(g.seats.iter().filter(|s| s.identity == LORD).count(), 1);
    }

    #[test]
    fn kill_rebel_draws_three() {
        let mut g = fixture(5);
        g.identity_game = true;
        g.seats[0].identity = LORD.into();
        g.seats[1].identity = REBEL.into();
        let before = g.hand(0).len();
        on_death(&mut g, 1, Some(0));
        assert_eq!(g.hand(0).len(), before + 3);
    }

    #[test]
    fn reward_is_inactive_outside_identity_games() {
        let mut g = fixture(5);
        g.seats[1].identity = REBEL.into();
        let before = g.hand(0).len();
        on_death(&mut g, 1, Some(0));
        assert_eq!(g.hand(0).len(), before, "夹具局不应触发身份奖惩");
    }

    #[test]
    fn win_conditions() {
        // 主公阵亡 → 反贼胜。
        let mut g = fixture(5);
        g.identity_game = true;
        g.seats[0].identity = LORD.into();
        g.seats[0].hp = 0;
        g.seats[0].alive = false; // I2：死亡确认（濒死不算死亡）
        for s in g.seats.iter_mut().skip(1) {
            s.identity = REBEL.into();
        }
        check_finished(&mut g);
        assert!(g.finished);
        assert_eq!(g.winner.as_deref(), Some(REBEL));

        // 反贼全灭 → 主公胜。
        let mut g = fixture(5);
        g.identity_game = true;
        g.seats[0].identity = LORD.into();
        for s in g.seats.iter_mut().skip(1) {
            s.identity = LOYALIST.into();
        }
        check_finished(&mut g);
        assert_eq!(g.winner.as_deref(), Some(LORD));

        // 主公死 + 仅剩内奸 → 内奸胜。
        let mut g = fixture(5);
        g.identity_game = true;
        g.seats[0].identity = LORD.into();
        g.seats[0].hp = 0;
        g.seats[0].alive = false; // I2：死亡确认
        for s in g.seats.iter_mut().skip(1) {
            s.identity = TRAITOR.into();
            s.hp = 0;
            s.alive = false;
        }
        g.seats[1].hp = 3;
        g.seats[1].alive = true;
        check_finished(&mut g);
        assert_eq!(g.winner.as_deref(), Some(TRAITOR));
    }
}
