//! `Policy` 实现：把行为树接进 `game-engine-core`（MCP `bot_action` / 自对弈共用）。

use crate::ai::view::SgsView;
use crate::ai::{prune, selfplay, tree};
use crate::state::SgsState;
use game_engine_core::{Action, GameState, Policy, RandomSource};

/// 一次决策的可记录轨迹（P6.4：决策日志/回放/训练数据）。
#[derive(Debug, Clone)]
pub struct DecisionTrace {
    /// 完整合法集（引擎下发顺序）。
    pub legal: Vec<Action>,
    /// 预剪枝候选（≤26，保持合法集顺序）。
    pub candidates: Vec<Action>,
    /// 实际选中的动作 id。
    pub action: String,
    pub priority: i32,
    pub score: f64,
    /// 决策依据（tree 分支 / 单步 / 破环器等）。
    pub reason: String,
    /// 动作前状态摘要（回放对齐用）。
    pub state_digest: String,
}

/// 三国杀启发式策略（确定性：同分按合法动作顺序）。
#[derive(Debug, Default, Clone, Copy)]
pub struct SgsPolicy;

impl Policy for SgsPolicy {
    fn choose(
        &self,
        state: &GameState,
        player_id: &str,
        legal_actions: &[Action],
        _rng: &mut dyn RandomSource,
    ) -> Option<String> {
        if legal_actions.is_empty() {
            return None;
        }
        if legal_actions.len() == 1 {
            return Some(legal_actions[0].id.clone());
        }
        decide_traced(state, player_id, legal_actions).map(|t| t.action)
    }
}

/// 决策 + 可记录轨迹；与 `Policy::choose` 同源（禁止两套打分）。
pub fn decide_traced(
    state: &GameState,
    player_id: &str,
    legal_actions: &[Action],
) -> Option<DecisionTrace> {
    if legal_actions.is_empty() {
        return None;
    }
    let st = SgsState::parse(&state.data).ok()?;
    let digest = selfplay::state_digest(&st);
    let view = SgsView::from_state(&st, player_id)?;
    let decision = tree::decide(&view, legal_actions)?;
    let candidates = prune::prune(&view, legal_actions, prune::DEFAULT_LIMIT);
    Some(DecisionTrace {
        legal: legal_actions.to_vec(),
        candidates,
        action: decision.action,
        priority: decision.score.priority,
        score: decision.score.score,
        reason: decision.reason.to_string(),
        state_digest: digest,
    })
}

/// 调试入口：返回决策 + 依据（`--explain` / 单测用）。
pub fn explain(
    state: &GameState,
    player_id: &str,
    legal_actions: &[Action],
) -> Option<tree::Decision> {
    let st = SgsState::parse(&state.data).ok()?;
    let view = SgsView::from_state(&st, player_id)?;
    tree::decide(&view, legal_actions)
}
