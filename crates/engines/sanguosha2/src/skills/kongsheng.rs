//! 箜声（周妃）：准备阶段可将任意张牌置于武将牌上；
//! 结束阶段取回其中非装备牌，然后令一名角色使用剩余牌并失去 1 点体力。
//!
//! spike 验证点：多阶段步骤顺序（取回 → 良姻回血 → 使用剩余装备）由**单一延迟队列的
//! 顺序表**保证，而不是靠代码位置。

use serde_json::json;

use crate::kernel::deferred::{Deferred, DeferredKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::pending::PendingBuilder;
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::flow;
use crate::kernel::Game;
use crate::presentation::{POutcome, Presentation};

use super::{SkillDef, SkillKind};

/// 箜声牌数（良姻回血判定的 X）。
pub fn count(g: &Game, seat: usize) -> usize {
    g.mark_cards(seat, "kongsheng").len()
}

pub const DEF: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "kongsheng",
    name: "箜声",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 5,
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
        Stage::Choose => {
            // PhaseStart 事件：按阶段名分流（Prepare / End）。
            match a.data["cause"].as_str().unwrap_or("") {
                "Prepare" => Ok(Step::Next(Stage::PreparePick, json!({ "picked": [] }))),
                "End" => Ok(Step::Next(Stage::EndReturn, json!({}))),
                _ => Ok(Step::Done),
            }
        }
        Stage::PreparePick => prepare_pick(g, a),
        Stage::EndReturn => end_return(g, a),
        Stage::UseEquip => use_equip(g, a),
        _ => Err("箜声：未知阶段".into()),
    }
}

/// 准备阶段：可反复置牌（多选）；`confirm` 为出口。
fn prepare_pick(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let picked: Vec<String> = a.data["picked"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let hand = g.hand(a.actor);
    let remaining: Vec<String> = hand.into_iter().filter(|id| !picked.contains(id)).collect();
    match g.answer.take().as_deref() {
        None => {
            if remaining.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(a.actor, "choose_many").skill("kongsheng");
            for id in &remaining {
                b = b.option(
                    format!("choose:{id}"),
                    format!("置于武将牌上【{}】", g.card_label(id)),
                );
            }
            let exit = if picked.is_empty() {
                "不置牌（放弃箜声）"
            } else {
                "完成置牌"
            };
            Ok(Step::Need(b.escape_with("confirm", exit).build()))
        }
        Some("confirm") => {
            if picked.is_empty() {
                Ok(Step::Done)
            } else {
                g.presentation.push(
                    Presentation::new("kongsheng", POutcome::MarkPlaced {
                        n: picked.len() as u32,
                    })
                    .actor(a.actor)
                    .cards(picked.clone())
                    .fx("kongsheng_place"),
                );
                g.push_log(format!(
                    "箜声：{} 将 {} 张牌置于武将牌上",
                    g.seats[a.actor].name,
                    picked.len()
                ));
                Ok(Step::Done)
            }
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("箜声：未知选项 {act}"))?
                .to_string();
            g.to_mark(&id, a.actor, "kongsheng")?;
            let mut picked = picked;
            picked.push(id);
            Ok(Step::Next(Stage::PreparePick, json!({ "picked": picked })))
        }
    }
}

/// 结束阶段：非装备牌回手；剩余装备进延迟队列（顺序表保证在良姻回血之后）。
fn end_return(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let marks = g.mark_cards(a.actor, "kongsheng");
    if marks.is_empty() {
        return Ok(Step::Done);
    }
    let mut returned = 0u32;
    let mut equips = Vec::new();
    for id in marks {
        if g.is_equip(&id) {
            equips.push(id);
        } else {
            g.to_hand(&id, a.actor)?;
            returned += 1;
        }
    }
    if returned > 0 {
        g.presentation.push(
            Presentation::new("kongsheng", POutcome::Draw { n: returned })
                .actor(a.actor)
                .fx("kongsheng_return"),
        );
        g.push_log(format!("箜声：{} 取回 {returned} 张非装备牌", g.seats[a.actor].name));
    }
    if equips.is_empty() {
        Ok(Step::Done)
    } else {
        g.deferred.push(Deferred::ask(
            DeferredKind::KongShengUse,
            "kongsheng",
            Stage::UseEquip,
            a.actor,
            json!({ "cards": equips }),
        ));
        Ok(Step::Done)
    }
}

/// 令一名角色使用剩余装备，然后其失去 1 点体力。
fn use_equip(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cards: Vec<String> = a.data["cards"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    let alive: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = PendingBuilder::new(a.actor, "target").skill("kongsheng");
            for i in &alive {
                b = b.option(
                    format!("target:{i}"),
                    format!("令 {} 使用剩余 {} 张装备", g.seats[*i].name, cards.len()),
                );
            }
            Ok(Step::Need(b.escape("不使用剩余牌").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let seat: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("箜声：目标不合法 {act}"))?;
            if !alive.contains(&seat) {
                return Err("箜声：目标不合法".into());
            }
            let mut used = 0u32;
            for id in &cards {
                let slot = g.cards.get(id).and_then(|c| c.slot);
                match slot {
                    Some(slot) => {
                        // 已有同槽装备 → 旧装备进弃牌堆。
                        let old = g.ids_in(|z| *z == crate::kernel::Zone::Equip(seat, slot));
                        if let Some(old_id) = old.first() {
                            g.to_discard(old_id, "kongsheng_replace")?;
                        }
                        g.to_equip(id, seat, slot)?;
                        used += 1;
                    }
                    None => g.to_discard(id, "kongsheng")?,
                }
            }
            g.presentation.push(
                Presentation::new("kongsheng", POutcome::Activated { skill: "箜声".into() })
                    .actor(a.actor)
                    .target(seat)
                    .cards(cards.clone())
                    .fx("kongsheng_use"),
            );
            flow::lose_hp(g, seat, 1, "kongsheng");
            g.push_log(format!(
                "箜声：{} 使用 {used} 张装备并失去 1 点体力（{}）",
                g.seats[seat].name, g.seats[seat].hp
            ));
            Ok(Step::Done)
        }
    }
}
