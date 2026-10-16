//! 测试 AI：启发式贪心策略（阶段 6 初始形态）。
//!
//! 打分口径参考攻略《关于"拉斯维加斯"桌游的近乎优胜解策略研究》：
//! 以「骰子加权（期望得分/骰）」为核心，基准 ≈ **2 分/骰**（1 分 = $1 万）；
//! 由于结算前钞票面额隐藏，用**牌库分布**估第一/第二名的期望分。
//!
//! `Policy` trait 来自 `game-engine-core`，后续 AlphaZero 可替换实现；本模块同时提供
//! `run_self_play` 自对弈驱动，供测试/回归复用。

use crate::{LasVegasState, Phase, DENOMINATIONS, DENOMINATION_COPIES};
use game_engine_core::{
    Action, EngineRegistry, GameManager, GameState, GameStatus, Policy, RandomSource, SeededRng,
};
use std::collections::BTreeMap;

/// 攻略基准：每个骰子的期望得分（分；1 分 = $1 万）。
pub const BASELINE_PER_DIE: f64 = 2.0;

/// 启发式贪心策略：对每个合法动作打分，取最高（同分按 legal 顺序，确定性）。
pub struct HeuristicPolicy;

impl Policy for HeuristicPolicy {
    fn choose(
        &self,
        state: &GameState,
        player_id: &str,
        legal_actions: &[Action],
        _rng: &mut dyn RandomSource,
    ) -> Option<String> {
        let st: LasVegasState = serde_json::from_value(state.data.clone()).ok()?;
        let mut best: Option<(f64, &str)> = None;
        for a in legal_actions {
            let score = score_action(&st, player_id, &a.id);
            match best {
                Some((bs, _)) if bs >= score => {}
                _ => best = Some((score, a.id.as_str())),
            }
        }
        best.map(|(_, id)| id.to_string())
    }
}

/// 对一个动作打分（分，越大越好）。
pub fn score_action(st: &LasVegasState, player: &str, id: &str) -> f64 {
    if let Some(nstr) = id.strip_prefix("place:") {
        let Ok(n) = nstr.parse::<u8>() else {
            return f64::NEG_INFINITY;
        };
        let matched = st.roll.small.iter().filter(|&&v| v == n).count() as u32;
        let big = st.roll.big == Some(n);
        let w = matched + if big { 2 } else { 0 };
        return score_add_dice(st, player, (n - 1) as usize, w);
    }
    if id == "reroll" {
        // 花 1 筹码（=1 分）弃回合；保留骰子的期权价值约半个基准。
        return -1.0 + BASELINE_PER_DIE * 0.5;
    }
    if id.starts_with("lobby:") {
        // 自对弈/测试：优先入局/开赛。
        return 100.0;
    }
    if let Some(nstr) = id.strip_prefix("sg:D2:use:") {
        if let Ok(n) = nstr.parse::<u8>() {
            return score_add_dice(st, player, (n - 1) as usize, 1);
        }
    }
    if id.starts_with("sg:G2:move:") {
        return 4.0;
    }
    if id.starts_with("sg:H1:move:") {
        return 0.8;
    }
    if id.starts_with("sg:E1:ban:") {
        return 2.0;
    }
    if id.starts_with("sg:F1:move:") {
        return 2.0;
    }
    if id.starts_with("sg:F2:take:") {
        return if id.ends_with("money") { 3.0 } else { 1.0 };
    }
    if let Some(d) = id
        .strip_prefix("sg:H2:choose:")
        .and_then(|s| s.parse::<u32>().ok())
    {
        return match d {
            1 => 1.0,
            2 => 2.0,
            3 => 3.0,
            4 => 1.0,
            5 => 1.0,
            6 => 6.0,
            _ => 0.0,
        };
    }
    if let Some(i) = id
        .strip_prefix("sg:G1:pick:")
        .and_then(|s| s.parse::<usize>().ok())
    {
        if let Some(p) = st.sg.pending.as_ref() {
            let piles: Vec<u32> = p
                .data
                .get("piles")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            return piles.get(i).copied().unwrap_or(0) as f64;
        }
    }
    // A1 猜数 / B2 猜大小 / G1 分堆 / E1:pass / F2:pass 等：中性。
    0.0
}

/// 估算「向某赌场增加 w 个等效骰」的收益（分）。
fn score_add_dice(st: &LasVegasState, player: &str, casino: usize, w: u32) -> f64 {
    if w == 0 {
        return f64::NEG_INFINITY;
    }
    let my = st.casinos[casino].dice.get(player).copied().unwrap_or(0) + w;
    let gray = st.casinos[casino].dice.get("gray").copied().unwrap_or(0);
    let mut others: Vec<u32> = st
        .seats
        .iter()
        .filter(|s| s.player_id != player)
        .map(|s| {
            st.casinos[casino]
                .dice
                .get(&s.player_id)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    others.push(gray);
    others.sort_unstable_by(|a, b| b.cmp(a));
    let best_other = others.first().copied().unwrap_or(0);
    let second_other = others.get(1).copied().unwrap_or(0);

    let (e1, e2) = expected_notes();
    let (p1, p2) = if my > best_other {
        (1.0, 0.0)
    } else if my == best_other {
        (0.0, 0.0) // 并列被剔除
    } else if my > second_other {
        (0.0, 0.7) // 大约第二
    } else {
        (0.0, 0.0)
    };
    p1 * e1 + p2 * e2 - BASELINE_PER_DIE * w as f64
}

/// 牌库分布下，「某赌场两张钞票」的期望最高/次高面额（分；万分单位）。
///
/// 假设两张独立按牌库分布抽取（近似，忽略无放回的小差异）。
pub fn expected_notes() -> (f64, f64) {
    let total: f64 = DENOMINATION_COPIES.iter().sum::<usize>() as f64;
    let mut cum = 0.0;
    let mut prev_f = 0.0;
    let mut mu = 0.0;
    let mut e_max = 0.0;
    for (d, c) in DENOMINATIONS.iter().zip(DENOMINATION_COPIES.iter()) {
        let k = *d as f64 / 10_000.0;
        let p = *c as f64 / total;
        mu += k * p;
        cum += p;
        e_max += k * (cum * cum - prev_f * prev_f);
        prev_f = cum;
    }
    let e_min = 2.0 * mu - e_max;
    (e_max, e_min)
}

// ---------------------------------------------------------------------------
// 自对弈驱动
// ---------------------------------------------------------------------------

/// 自对弈结果。
#[derive(Debug, Clone)]
pub struct SelfPlayResult {
    pub finished: bool,
    pub steps: usize,
    pub event_count: usize,
    pub winners: Vec<String>,
    pub scores: BTreeMap<String, i64>,
}

/// 让 `policies[i]` 操控 `players[i]` 打完一局（lobby→终局）。
///
/// 引擎随机源固定为 `seed`（可复现）；策略自身用派生种子。超过 `max_steps` 终止。
pub fn run_self_play(
    players: &[&str],
    policies: &[&dyn Policy],
    seed: u64,
    max_steps: usize,
) -> SelfPlayResult {
    let mut reg = EngineRegistry::new();
    reg.register(crate::LasVegasEngine);
    let mgr = GameManager::with_rng(reg, Box::new(SeededRng::new(seed)));
    let host = players[0];

    let mut policy_rng = SeededRng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    let mut events = 0usize;
    let mut steps = 0usize;

    let Ok((st, _)) = mgr.create_game("las-vegas", "selfplay", host) else {
        return finished_result(false, 0, 0, BTreeMap::new(), Vec::new());
    };
    let room = st.room_id.clone();
    for p in &players[1..] {
        let _ = mgr.execute_action(&room, p, "lobby:join");
    }
    let _ = mgr.execute_action(&room, host, "lobby:start");

    let mut state = mgr.get_state(&room).unwrap_or(st);
    while state.status == GameStatus::Active && steps < max_steps {
        let Ok(d) = serde_json::from_value::<LasVegasState>(state.data.clone()) else {
            break;
        };
        let actor = if d.phase == Phase::SmallGame {
            d.sg.pending.as_ref().map(|p| p.actor.clone())
        } else {
            d.seats.get(d.turn).map(|s| s.player_id.clone())
        };
        let Some(actor) = actor else { break };
        let Ok(legal) = mgr.legal_actions(&room, &actor) else {
            break;
        };
        let Some(pi) = players.iter().position(|p| *p == actor) else {
            break;
        };
        let Some(action) = policies[pi].choose(&state, &actor, &legal, &mut policy_rng) else {
            break;
        };
        let Ok(res) = mgr.execute_action(&room, &actor, &action) else {
            break;
        };
        events += res.events.len();
        state = res.state;
        steps += 1;
    }

    let (winners, scores) = match serde_json::from_value::<LasVegasState>(state.data.clone()) {
        Ok(d) => (d.winners, d.scores),
        Err(_) => (Vec::new(), BTreeMap::new()),
    };
    finished_result(state.status == GameStatus::Finished, steps, events, scores, winners)
}

fn finished_result(
    finished: bool,
    steps: usize,
    event_count: usize,
    scores: BTreeMap<String, i64>,
    winners: Vec<String>,
) -> SelfPlayResult {
    SelfPlayResult {
        finished,
        steps,
        event_count,
        winners,
        scores,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine_core::RandomPolicy;

    #[test]
    fn expected_notes_sane() {
        let (e_max, e_min) = expected_notes();
        // 均值 6.244 万 → 最高 > 均值 > 次高
        assert!(e_max > 6.244 && e_min < 6.244, "e_max={e_max} e_min={e_min}");
        assert!(e_max > e_min);
    }

    #[test]
    fn self_play_finishes_and_is_deterministic() {
        let players = ["a", "b", "c"];
        let p = HeuristicPolicy;
        let policies: Vec<&dyn Policy> = vec![&p, &p, &p];
        let r1 = run_self_play(&players, &policies, 7, 5000);
        let r2 = run_self_play(&players, &policies, 7, 5000);
        assert!(r1.finished, "自对弈应跑到终局");
        assert_eq!(r1.steps, r2.steps);
        assert_eq!(r1.scores, r2.scores);
        assert_eq!(r1.winners, r2.winners);
        assert!(r1.event_count > 0);
        assert_eq!(r1.scores.len(), 3);
    }

    #[test]
    fn heuristic_beats_random_over_games() {
        let h = HeuristicPolicy;
        let rnd = RandomPolicy;
        let mut h_wins = 0;
        let mut r_wins = 0;
        let mut h_total = 0i64;
        let mut r_total = 0i64;
        for seed in 1..=20u64 {
            let players = ["h", "r"];
            let policies: Vec<&dyn Policy> = vec![&h, &rnd];
            let res = run_self_play(&players, &policies, seed, 5000);
            assert!(res.finished);
            h_total += res.scores.get("h").copied().unwrap_or(0);
            r_total += res.scores.get("r").copied().unwrap_or(0);
            if res.winners.iter().any(|w| w == "h") {
                h_wins += 1;
            }
            if res.winners.iter().any(|w| w == "r") {
                r_wins += 1;
            }
        }
        // 启发式应显著占优（固定种子，确定性）
        assert!(
            h_total > r_total && h_wins >= r_wins,
            "启发式应优于随机：h_wins={h_wins} r_wins={r_wins} h_total={h_total} r_total={r_total}"
        );
    }

    #[test]
    fn four_player_self_play_finishes() {
        let players = ["a", "b", "c", "d"];
        let p = HeuristicPolicy;
        let policies: Vec<&dyn Policy> = vec![&p, &p, &p, &p];
        let r = run_self_play(&players, &policies, 99, 8000);
        assert!(r.finished, "4 人自对弈应完成：steps={}", r.steps);
        assert_eq!(r.scores.len(), 4);
    }

    #[test]
    fn policy_chooses_a_legal_action_at_turn_start() {
        let p = HeuristicPolicy;
        let mut reg = EngineRegistry::new();
        reg.register(crate::LasVegasEngine);
        let mgr = GameManager::with_rng(reg, Box::new(SeededRng::new(3)));
        let (st, _) = mgr.create_game("las-vegas", "g", "a").unwrap();
        let _ = mgr.execute_action(&st.room_id, "b", "lobby:join").unwrap();
        let started = mgr.execute_action(&st.room_id, "a", "lobby:start").unwrap().state;
        let d: LasVegasState = serde_json::from_value(started.data.clone()).unwrap();
        let cur = d.seats[d.turn].player_id.clone();
        let legal = mgr.legal_actions(&st.room_id, &cur).unwrap();
        let mut rng = SeededRng::new(1);
        let chosen = p.choose(&started, &cur, &legal, &mut rng).unwrap();
        assert!(legal.iter().any(|a| a.id == chosen), "选中动作应在合法集内");
    }
}
