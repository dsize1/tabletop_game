//! 行为树：把「合法动作集合」变成「一个动作」。
//!
//! 结构（优先级自上而下）：
//! 1. **安全兜底**：未登记的 pending kind → 优先 `pass`（等价于
//!    `apps/verify-sanguosha.mjs` 的「安全推进」策略，保证永不死循环）；
//! 2. 逐动作打分（`handlers::score_action`）；
//! 3. 取 `(priority, score)` 最大者，同分按合法动作顺序（确定性）。

use crate::ai::eval::Evaluator;
use crate::ai::handlers;
use crate::ai::score::Score;
use crate::ai::view::SgsView;
use game_engine_core::Action;

/// 引擎已登记的 pending kind（`docs/games/sanguosha/state-machine.md` §2.2）。
pub const KNOWN_PENDING_KINDS: &[&str] = &[
    "target",
    "respond",
    "dying",
    "discard",
    "wuxie",
    "pick_card",
    "choice",
    "choose_many",
    "wugu",
    "arrange",
    "judge_rewrite",
];

/// 一次决策的结果（含分数，供日志/`--explain` 使用）。
#[derive(Debug, Clone)]
pub struct Decision {
    pub action: String,
    pub score: Score,
    /// 决策依据（`tree` 的哪条分支）。
    pub reason: &'static str,
}

/// 在合法动作里选一个（`None` = 无动作可选）。
pub fn decide(view: &SgsView, legal: &[Action]) -> Option<Decision> {
    if legal.is_empty() {
        return None;
    }
    if legal.len() == 1 {
        let a = &legal[0];
        return Some(Decision {
            action: a.id.clone(),
            score: Score::neutral(),
            reason: "single_legal_action",
        });
    }

    let kind = view.pending_kind();
    if !kind.is_empty() && !KNOWN_PENDING_KINDS.contains(&kind) {
        // 未登记的交互：安全推进（有 pass 就放弃，否则用第一个合法动作）。
        if let Some(pass) = legal.iter().find(|a| a.id == "pass") {
            return Some(Decision {
                action: pass.id.clone(),
                score: Score::neutral(),
                reason: "safe_advance_unknown_pending",
            });
        }
        let a = &legal[0];
        return Some(Decision {
            action: a.id.clone(),
            score: Score::neutral(),
            reason: "safe_advance_unknown_pending",
        });
    }

    let ev = Evaluator::new(view);
    let mut best: Option<(&Action, Score)> = None;
    for a in legal {
        let s = handlers::score_action(&ev, &a.id);
        match best {
            Some((_, bs)) if !s.better_than(&bs) => {}
            _ => best = Some((a, s)),
        }
    }
    best.map(|(a, s)| Decision {
        action: a.id.clone(),
        score: s,
        reason: if s.priority > 0 {
            "priority_rule"
        } else {
            "heuristic_score"
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_kinds_cover_engine_registry() {
        // 与 `tests/interaction_contract.rs::REGISTERED_PENDING_KINDS` 同源口径。
        for kind in [
            "target",
            "respond",
            "dying",
            "discard",
            "wuxie",
            "pick_card",
            "choice",
            "choose_many",
            "wugu",
            "arrange",
            "judge_rewrite",
        ] {
            assert!(KNOWN_PENDING_KINDS.contains(&kind), "缺少 {kind}");
        }
    }
}
