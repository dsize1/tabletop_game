//! 选目标（`target:<pid|pid,csv>`）打分。
//!
//! 默认「选最有价值的敌人」；若当前 pending 的技能是增益类（仁德/结姻/青囊等），
//! 则反转为「选最需要帮助的盟友」。

use crate::ai::eval::Evaluator;
use crate::ai::score::{prio, Score};

/// 技能是「对别人好」的（选目标应选盟友）。
const BENEFICIAL_SKILLS: &[&str] = &[
    "rende", "jieyin", "qingnang", "taoyuan_jieyi", "jiuyuan", "fangquan", "mingce",
];

fn beneficial(ev: &Evaluator) -> bool {
    ev.view
        .pending
        .as_ref()
        .and_then(|p| p.skill.as_deref())
        .map(|s| BENEFICIAL_SKILLS.contains(&s))
        .unwrap_or(false)
}

fn seat_score(ev: &Evaluator, seat: usize, want_ally: bool) -> f64 {
    let Some(s) = ev.view.seats.get(seat) else {
        return f64::NEG_INFINITY;
    };
    if !s.alive {
        return f64::NEG_INFINITY;
    }
    if want_ally {
        if seat == ev.view.me {
            // 青囊/结姻也可能选自己：残血时可选。
            return ev.threat.friendly_score(seat) * (1.0 - s.health()) * 10.0;
        }
        ev.threat.friendly_score(seat) * (1.0 - s.health()) * 10.0
    } else {
        ev.enemy_pressure(seat) * 10.0
    }
}

/// 选目标打分（多目标取其中最高者）。
pub fn score_target(ev: &Evaluator, rest: &str) -> Score {
    let want_ally = beneficial(ev);
    let mut best = f64::NEG_INFINITY;
    for raw in rest.split(',') {
        let pid = raw.trim();
        if pid.is_empty() {
            continue;
        }
        let Some(seat) = ev.view.seat_of(pid) else {
            continue;
        };
        let v = seat_score(ev, seat.index, want_ally);
        if v > best {
            best = v;
        }
    }
    if !best.is_finite() {
        return Score::new(prio::IDLE, 0.5);
    }
    if best <= 0.0 {
        // 没有值得选的目标（全是友方 / 全是未知）：低优先级，让别的动作有机会。
        return Score::new(prio::IDLE, 1.0);
    }
    Score::new(prio::NORMAL, best)
}
