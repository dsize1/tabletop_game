//! 贞良（卢植）· 转换技
//! 阳：出牌阶段限一次，选攻击范围内一名其他角色，弃一张与「任」同色的牌造成 1 点伤害。
//! 阴：回合外你使用/打出的牌进入弃牌堆时，若与「任」同色，可令一名角色摸一张。
//!
//! spike 验证点：
//! - 转换态用 `Game::yang()` 单一状态源（不是 flags/turn_counters 各存一份）；
//! - 阴面触发走 `Window::OtherTurn`（反例：自己回合不触发）；
//! - 阳面 active 技的 `legal` 与 `execute` 调**同一个** `can_yang`（不会漂移）。

use serde_json::json;

use crate::kernel::event::{EventKind, Window};
use crate::kernel::pending::PendingBuilder;
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{Game, Phase};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, SkillDef, SkillKind};

/// 「任」的颜色（spike 中由测试注入 mark ；明任本身不在本批）。
pub fn ren_color(g: &Game, seat: usize) -> Option<bool> {
    g.mark_cards(seat, "ren")
        .first()
        .and_then(|id| g.cards.get(id).map(|c| c.suit.is_red()))
}

pub const DEF: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhenliang",
    name: "贞良",
    triggers: &[EventKind::CardUsed, EventKind::CardPlayed],
    window: Window::OtherTurn,
    priority: 20,
    can_active: Some(can_yang),
    handler,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn handler(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => choose_yin(g, a),
        Stage::YinTarget => yin_target(g, a),
        Stage::YangTarget => yang_target(g, a),
        Stage::YangCard => yang_card(g, a),
        _ => Err("贞良：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 阳（主动）
// ---------------------------------------------------------------------------

/// legal 与 execute **同源**的可发动判定。
pub fn can_yang(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "zhenliang") || !g.yang(seat, "zhenliang") {
        return false;
    }
    if g.turn != seat || g.phase != Phase::Play || g.counter("phase.zhenliang") > 0 {
        return false;
    }
    let Some(red) = ren_color(g, seat) else {
        return false;
    };
    let has_card = g
        .hand(seat)
        .iter()
        .any(|id| g.cards.get(id).is_some_and(|c| c.suit.is_red() == red));
    has_card && !other_alive(g, seat).is_empty()
}

fn yang_target(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands = other_alive(g, a.actor);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "target").skill("zhenliang");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("对 {} 造成 1 点伤害", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动贞良").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let seat: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("贞良：目标不合法 {act}"))?;
            if !cands.contains(&seat) {
                return Err("贞良：目标不合法".into());
            }
            Ok(Step::Next(Stage::YangCard, json!({ "target": seat })))
        }
    }
}

fn yang_card(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let red = ren_color(g, a.actor).unwrap_or(true);
    let cands: Vec<String> = g
        .hand(a.actor)
        .into_iter()
        .filter(|id| g.cards.get(id).is_some_and(|c| c.suit.is_red() == red))
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "choice").skill("zhenliang");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("弃置【{}】造成伤害", g.card_label(id)),
                );
            }
            Ok(Step::Need(b.escape("不发动贞良").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("贞良：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("贞良：该牌与「任」颜色不同".into());
            }
            g.to_discard(&id, "zhenliang")?;
            g.bump("phase.zhenliang");
            g.toggle_yang(a.actor, "zhenliang");
            super::damage(g, a.actor, target, 1);
            g.presentation.push(
                Presentation::new("zhenliang", POutcome::Damage { amount: 1 })
                    .actor(a.actor)
                    .target(target)
                    .cards(vec![id])
                    .fx("zhenliang_yang"),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 阴（回合外触发）
// ---------------------------------------------------------------------------

fn choose_yin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if active(g, a.actor, "zhenliang") && !g.yang(a.actor, "zhenliang") {
        // 阴面：判定颜色
        if let Some(card) = a.data["card"].as_str() {
            let same = g
                .cards
                .get(card)
                .zip(ren_color(g, a.actor))
                .map(|(c, red)| c.suit.is_red() == red)
                .unwrap_or(false);
            if same {
                return Ok(Step::Next(Stage::YinTarget, json!({ "card": card })));
            }
        }
    }
    Ok(Step::Done)
}

fn yin_target(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "choice").skill("zhenliang");
            for i in &cands {
                b = b.option(
                    format!("choose:draw:{i}"),
                    format!("令 {} 摸一张牌", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动贞良").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let seat: usize = act
                .strip_prefix("choose:draw:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("贞良：未知选项 {act}"))?;
            if !cands.contains(&seat) {
                return Err("贞良：目标不合法".into());
            }
            g.draw(seat, 1);
            g.toggle_yang(a.actor, "zhenliang");
            g.bump("turn.zhenliang");
            g.presentation.push(
                Presentation::new("zhenliang", POutcome::Draw { n: 1 })
                    .actor(a.actor)
                    .target(seat)
                    .fx("zhenliang_yin"),
            );
            Ok(Step::Done)
        }
    }
}
