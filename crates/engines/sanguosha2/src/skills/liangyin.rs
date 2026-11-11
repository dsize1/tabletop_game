//! 良姻（周妃）：每回合首次有牌移出/移入游戏后 → 与一名其他角色各摸/弃一张
//! → 然后可令「手牌数 == 箜声牌数」的一名角色回复 1 点体力。
//!
//! 本次 spike 要验证的 4 个点：
//! 1. **窗口**：`Window::OwnTurn`（用户裁定：只在周妃回合内生效）→ 反例断言覆盖「他人回合不触发」；
//! 2. **延后派发**：回血判定进单一 `Deferred` 队列（顺序表 `LiangYinHeal` 先于 `KongShengUse`）；
//! 3. **候选为空不询问**（复验 25）；
//! 4. **每回合首次**（移出 / 移入各自独立计数，切换回合清空）。

use serde_json::json;

use crate::kernel::deferred::{Deferred, DeferredKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::pending::PendingBuilder;
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::Game;
use crate::presentation::{POutcome, Presentation};

use super::{other_alive, SkillDef};

pub const DEF: SkillDef = SkillDef {
    code: "liangyin",
    name: "良姻",
    triggers: &[EventKind::CardOutOfGame, EventKind::CardIntoGame],
    window: Window::OwnTurn,
    priority: 10,
    can_active: None,
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
        Stage::Choose => choose(g, a),
        Stage::Target => target(g, a),
        Stage::DiscardOwner => discard_owner(g, a),
        Stage::DiscardTarget => discard_target(g, a),
        Stage::Heal => heal(g, a),
        _ => Err("良姻：未知阶段".into()),
    }
}

/// 每回合首次（移出/移入各自独立）→ 询问「各摸一张 / 各弃一张」。
fn choose(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let event = a
        .data["event"]
        .as_str()
        .unwrap_or("CardIntoGame")
        .to_string();
    let flag = format!("turn.liangyin.{event}");
    if g.counter(&flag) > 0 {
        return Ok(Step::Done); // 每回合首次：该方向已消耗
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            PendingBuilder::new(a.actor, "choice")
                .skill("liangyin")
                .option("choose:draw", "与一名其他角色各摸一张牌")
                .option("choose:discard", "与一名其他角色各弃置一张牌")
                .escape("不发动良姻")
                .build(),
        )),
        Some("pass") => {
            // 放弃也消耗「本回合首次」（触发点一次性，避免同一回合反复骚扰）。
            g.bump(&flag);
            Ok(Step::Done)
        }
        Some(act @ ("choose:draw" | "choose:discard")) => {
            g.bump(&flag);
            let mode = if act == "choose:draw" { "draw" } else { "discard" };
            Ok(Step::Next(Stage::Target, json!({ "mode": mode })))
        }
        Some(other) => Err(format!("良姻：未知选项 {other}")),
    }
}

/// 选一名其他角色。
fn target(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let mode = a.data["mode"].as_str().unwrap_or("draw").to_string();
    let cands: Vec<usize> = other_alive(g, a.actor)
        .into_iter()
        .filter(|i| mode == "draw" || !g.hand(*i).is_empty())
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "target").skill("liangyin");
            for i in &cands {
                let verb = if mode == "draw" { "各摸一张" } else { "各弃一张" };
                b = b.option(
                    format!("target:{i}"),
                    format!("与 {} {verb}", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动良姻").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let seat: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("良姻：目标不合法 {act}"))?;
            if !cands.contains(&seat) {
                return Err("良姻：目标不合法".into());
            }
            if mode == "draw" {
                g.draw(a.actor, 1);
                g.draw(seat, 1);
                g.presentation.push(
                    Presentation::new("liangyin", POutcome::Draw { n: 1 })
                        .actor(a.actor)
                        .target(seat)
                        .fx("liangyin_draw"),
                );
                g.push_log(format!(
                    "良姻：{} 与 {} 各摸一张",
                    g.seats[a.actor].name, g.seats[seat].name
                ));
                // 回血判定延后到栈空（先让其他钩子改变手牌数）。
                g.deferred.push(Deferred::ask(
                    DeferredKind::LiangYinHeal,
                    "liangyin",
                    Stage::Heal,
                    a.actor,
                    json!({ "target": seat }),
                ));
                Ok(Step::Done)
            } else {
                Ok(Step::Next(Stage::DiscardOwner, json!({ "target": seat })))
            }
        }
    }
}

fn discard_owner(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let hand = g.hand(a.actor);
    if hand.is_empty() {
        return Ok(Step::Next(Stage::DiscardTarget, a.data.clone()));
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "choice").skill("liangyin");
            for id in &hand {
                b = b.option(
                    format!("choose:{id}"),
                    format!("弃置【{}】", g.card_label(id)),
                );
            }
            Ok(Step::Need(b.escape("放弃良姻").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("良姻：未知选项 {act}"))?
                .to_string();
            g.to_discard(&id, "liangyin")?;
            g.presentation.push(
                Presentation::new("liangyin", POutcome::Discard { n: 1 })
                    .actor(a.actor)
                    .cards(vec![id]),
            );
            Ok(Step::Next(Stage::DiscardTarget, a.data.clone()))
        }
    }
}

fn discard_target(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let hand = g.hand(target);
    if hand.is_empty() {
        return Ok(Step::Next(Stage::Heal, a.data.clone()));
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(target, "choice").skill("liangyin");
            for id in &hand {
                b = b.option(
                    format!("choose:{id}"),
                    format!("弃置【{}】", g.card_label(id)),
                );
            }
            Ok(Step::Need(b.escape("放弃良姻").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("良姻：未知选项 {act}"))?
                .to_string();
            g.to_discard(&id, "liangyin")?;
            g.presentation.push(
                Presentation::new("liangyin", POutcome::Discard { n: 1 })
                    .actor(target)
                    .cards(vec![id]),
            );
            Ok(Step::Next(Stage::Heal, a.data.clone()))
        }
    }
}

/// 回血：候选 = 手牌数 == 箜声牌数 且未满血的角色（自己 + 目标）。
/// 候选为空 → **直接跳过，不询问**（复验 25）。
fn heal(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let x = super::kongsheng::count(g, a.actor);
    let mut cands: Vec<usize> = vec![a.actor, target];
    cands.dedup();
    let cands: Vec<usize> = cands
        .into_iter()
        .filter(|i| {
            g.seats[*i].hp > 0 && g.seats[*i].hp < g.seats[*i].max_hp && g.hand(*i).len() == x
        })
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "choice").skill("liangyin");
            for i in &cands {
                b = b.option(
                    format!("choose:heal:{i}"),
                    format!(
                        "令 {} 回复 1 点体力（手牌 {} = 箜声 {x}）",
                        g.seats[*i].name,
                        g.hand(*i).len()
                    ),
                );
            }
            Ok(Step::Need(b.escape("不回血").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let seat: usize = act
                .strip_prefix("choose:heal:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("良姻：未知选项 {act}"))?;
            if !cands.contains(&seat) {
                return Err("良姻：回血目标不合法".into());
            }
            g.seats[seat].hp = (g.seats[seat].hp + 1).min(g.seats[seat].max_hp);
            g.presentation.push(
                Presentation::new("liangyin", POutcome::Heal { amount: 1 })
                    .actor(a.actor)
                    .target(seat)
                    .fx("liangyin_heal"),
            );
            g.push_log(format!("良姻：{} 回复 1 点体力", g.seats[seat].name));
            Ok(Step::Done)
        }
    }
}

/// 供测试使用：判断某事件是否会命中本技能窗口。
pub fn accepts(kind: EventKind, window: Window, turn: usize, phase: crate::kernel::Phase) -> bool {
    window.allows(turn, phase, 0) && DEF.triggers.contains(&kind)
}
