//! 动作打分入口：按动作 id 家族分派到各 handler。
//!
//! 分派顺序按**前缀族**（`respond:` / `play:` / `target:` ...），而不是只按 `pending.kind`：
//! 引擎里 `choice` / `target` 两个 kind 覆盖了大量技能语义，只按 kind 会漏判。

pub mod misc;
pub mod pick;
pub mod play;
pub mod respond;
pub mod target;

use super::eval::Evaluator;
use super::score::Score;

/// 动作载荷里的牌是否真的在本人的可见牌里（手牌 / 装备 / 已知牌）。
///
/// 用途：引擎偶发下发「过期选项」（如响应帧未按当前手牌重建），
/// AI 侧先按可见信息过滤，避免选中必然被拒的动作。
fn card_available(ev: &Evaluator, payload: &str) -> bool {
    for id in payload.split(',') {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let in_hand = ev.view.hand.iter().any(|c| c.id == id);
        let in_equip = ev.view.equips.iter().any(|c| c.id == id);
        let in_known = ev.view.known_cards.contains_key(id);
        if !in_hand && !in_equip && !in_known {
            return false;
        }
    }
    true
}

/// 给一个合法动作打分。
pub fn score_action(ev: &Evaluator, id: &str) -> Score {
    let kind = ev.view.pending_kind().to_string();

    if id == "pass" {
        return respond::score_pass(ev);
    }
    if id == "confirm" {
        return pick::score_confirm(ev);
    }
    if id == "end_play" {
        return Score::new(super::score::prio::NORMAL, 0.5);
    }
    if let Some(rest) = id.strip_prefix("respond:") {
        return respond::score_respond(ev, &kind, rest);
    }
    if let Some(rest) = id.strip_prefix("discard:") {
        if !card_available(ev, rest) {
            return Score::new(super::score::prio::IDLE, 0.0);
        }
        return pick::score_discard(ev, rest);
    }
    if let Some(rest) = id.strip_prefix("play:") {
        return play::score_play(ev, rest);
    }
    if let Some(rest) = id.strip_prefix("target:") {
        return target::score_target(ev, rest);
    }
    if let Some(rest) = id.strip_prefix("targets:") {
        return target::score_target(ev, rest);
    }
    if let Some(rest) = id.strip_prefix("choose:") {
        return pick::score_choose(ev, &kind, rest);
    }
    if let Some(rest) = id.strip_prefix("pick:") {
        return pick::score_pick(ev, rest);
    }
    if id.starts_with("arrange:") {
        // 观星：引擎只给出「当前 slots 顺序」的默认提交项，直接提交。
        return Score::new(super::score::prio::COMMIT, 20.0);
    }
    if let Some(rest) = id.strip_prefix("skill:") {
        return misc::score_skill(ev, rest);
    }
    if let Some(rest) = id.strip_prefix("setup:choose_general:") {
        return misc::score_general(ev, rest);
    }
    if id.starts_with("lobby:") {
        return misc::score_lobby(id);
    }
    if id == "setup:confirm" {
        return Score::new(super::score::prio::COMMIT, 15.0);
    }
    // 未登记前缀（`opt1`/`opt2` 等技能帧）：按 pending 选项的 label 做关键词打分，
    // 否则所有选项同分 → 只能按合法顺序取第一个，容易在技能帧里选到「弃牌」类坏选项。
    if let Some(label) = ev
        .view
        .pending
        .as_ref()
        .and_then(|p| p.options.iter().find(|(oid, _)| oid == id))
        .map(|(_, label)| label.clone())
    {
        return pick::score_label(&label);
    }
    Score::neutral()
}
