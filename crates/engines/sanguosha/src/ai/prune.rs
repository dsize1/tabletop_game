//! 候选剪枝：完整合法集 → ≤26 个候选（`GAME_ACTION_CHOICE_LIMIT`）。
//!
//! 用途：
//! - 决策日志的 `candidates` 字段（训练/复盘）；
//! - 后续接 SystemOne/Web 建议时的选项上限；
//! - 中期「关键决策一层搜索」的分支控制。
//!
//! **保底项不可剪**：`pass`（放弃响应）、`end_play`（结束出牌阶段）、以及打分最高的那个动作。

use crate::ai::eval::Evaluator;
use crate::ai::handlers;
use crate::ai::score::Score;
use crate::ai::view::SgsView;
use game_engine_core::Action;
use std::cmp::Ordering;

/// 默认候选上限（与 `GAME_ACTION_CHOICE_LIMIT` 一致）。
pub const DEFAULT_LIMIT: usize = 26;

/// 打分并排序（返回 `(原下标, 动作, 分数)`，按优先级降序）。
pub fn rank(view: &SgsView, legal: &[Action]) -> Vec<(usize, Action, Score)> {
    let ev = Evaluator::new(view);
    let mut out: Vec<(usize, Action, Score)> = legal
        .iter()
        .enumerate()
        .map(|(i, a)| (i, a.clone(), handlers::score_action(&ev, &a.id)))
        .collect();
    out.sort_by(|a, b| {
        let (pa, sa) = a.2.sort_key();
        let (pb, sb) = b.2.sort_key();
        pb.partial_cmp(&pa)
            .unwrap_or(Ordering::Equal)
            .then_with(|| sb.partial_cmp(&sa).unwrap_or(Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

/// 剪枝到 `limit` 个候选，返回**保持原合法集顺序**的子集。
pub fn prune(view: &SgsView, legal: &[Action], limit: usize) -> Vec<Action> {
    if legal.len() <= limit {
        return legal.to_vec();
    }
    let ranked = rank(view, legal);
    let mut picked: Vec<usize> = Vec::new();
    // 保底：pass / end_play
    for (i, a, _) in &ranked {
        if (a.id == "pass" || a.id == "end_play") && !picked.contains(i) {
            picked.push(*i);
        }
    }
    // 保底：打分最高的动作
    if let Some((i, _, _)) = ranked.first() {
        if !picked.contains(i) {
            picked.push(*i);
        }
    }
    for (i, _, _) in &ranked {
        if picked.len() >= limit {
            break;
        }
        if !picked.contains(i) {
            picked.push(*i);
        }
    }
    picked.sort_unstable();
    picked.into_iter().map(|i| legal[i].clone()).collect()
}

// 剪枝的真实断言（需真实局面）见 `ai::tests::prune_keeps_bound_and_must_keep`。
