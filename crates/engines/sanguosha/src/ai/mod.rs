//! 三国杀 AI（P6）：规则引擎 + 行为树 + 启发式评估的 Baseline 实现。
//!
//! 分层（详见 `docs/games/sanguosha/ai.md`）：
//!
//! ```text
//! GameState ── SgsState::parse ── SgsView::from_state（只投影可见信息，ADR-0032）
//!                                     │
//!                          Evaluator（资源/威胁/盟友评估 + ThreatModel 先验）
//!                                     │
//!                        handlers（按动作 id 家族打分）
//!                                     │
//!                       tree::decide（优先级 + 分数取最优，确定性）
//!                                     │
//!                     Policy::choose → execute_action（引擎无条件重校验）
//! ```
//!
//! 约束：
//! - **只用可见信息**：handler 拿不到他人手牌/身份/牌堆顺序；
//! - 不做 SystemOne 走子（延迟/确定性不适合 100 局自对弈），也不是纯 MCTS；
//! - 输出只是「建议动作」，合法性仍由引擎保证。
//!
//! 回归：`cargo test -p sanguosha ai_`（单测）/ `npm run check:sgs-ai`（自对弈）/ 
//! `cargo run -p sanguosha --example selfplay -- --games 100`。

pub mod card;
pub mod driver;
pub mod eval;
pub mod handlers;
pub mod policy;
pub mod prune;
pub mod score;
pub mod selfplay;
pub mod threat;
pub mod tree;
pub mod view;

#[cfg(test)]
mod tests;

pub use driver::{drive_bots, DriveReport, StopReason, MAX_STEPS as DRIVE_MAX_STEPS};
pub use policy::{explain, SgsPolicy};
pub use selfplay::{
    run_batch, run_self_play, run_self_play_traced, run_self_play_with, BatchResult, SelfPlayResult,
    FULL_GENERALS,
};
pub use view::SgsView;
