//! `Policy` 实现：把行为树接进 `game-engine-core`（MCP `bot_action` / 自对弈共用）。

use crate::ai::tree;
use crate::ai::view::SgsView;
use crate::state::SgsState;
use game_engine_core::{Action, GameState, Policy, RandomSource};

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
        let st = SgsState::parse(&state.data).ok()?;
        let view = SgsView::from_state(&st, player_id)?;
        tree::decide(&view, legal_actions).map(|d| d.action)
    }
}

/// 调试入口：返回决策 + 依据（`--explain` / 单测用）。
pub fn explain(state: &GameState, player_id: &str, legal_actions: &[Action]) -> Option<tree::Decision> {
    let st = SgsState::parse(&state.data).ok()?;
    let view = SgsView::from_state(&st, player_id)?;
    tree::decide(&view, legal_actions)
}
