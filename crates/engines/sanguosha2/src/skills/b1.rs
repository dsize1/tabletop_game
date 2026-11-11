//! B1 第一批（5 技能）：庸肆 / 奇制 / 进趋 / 怀橘 / 寸目。
//!
//! 依据 `docs/games/sanguosha/align/B1.md` 的「对齐后描述」与复验 47/51/52：
//! - 庸肆：锁定；摸牌阶段 +X（X=存活不同势力数）、弃牌阶段先弃 X；额外弃计入本阶段弃牌数。
//! - 奇制：回合内使用非装备牌**指定目标后**（走 `TargetChosen` 事件）；不限次；不含打出。
//! - 进趋：**结束阶段**摸 2 后弃至 X；X=0 不允许发动。
//! - 怀橘：3 枚「橘」（计数标记）+ 摸牌 +1 + 自动防伤。
//! - 寸目：所有摸牌从牌堆底摸。

use serde_json::json;

use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Step};
use crate::kernel::{Game, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{SkillDef, SkillKind};

// ---------------------------------------------------------------------------
// 庸肆（袁术）：纯数值钩子，无询问（锁定）
// ---------------------------------------------------------------------------

pub const YONGSI: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "yongsi",
    name: "庸肆",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 3,
    can_active: None,
    handler: yongsi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 弃牌阶段开始时先弃 X 张（摸牌加成在 `skills::draw_bonus` 里实现）。
fn yongsi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["cause"].as_str() != Some("Discard") {
        return Ok(Step::Done);
    }
    let x = super::kingdom_count(g);
    if x == 0 || g.counter("phase.yongsi_done") > 0 {
        return Ok(Step::Done);
    }
    g.bump("phase.yongsi_done");
    let n = x.min(g.hand(a.actor).len());
    for id in g.hand(a.actor).into_iter().take(n) {
        g.to_discard(&id, "yongsi")?;
    }
    g.push_log(format!("庸肆：弃牌阶段先弃 {n} 张（X={x}）"));
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 奇制（王基）：使用非装备牌指定目标后
// ---------------------------------------------------------------------------

pub const QIZHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qizhi",
    name: "奇制",
    triggers: &[EventKind::TargetChosen],
    window: Window::OwnTurn,
    priority: 20,
    can_active: None,
    handler: qizhi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qizhi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 事件主体必须是技能拥有者（自己使用牌）。仅在触发阶段校验（后续阶段 data 已换成目标）。
    if a.data.get("event").is_some()
        && a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor)
    {
        return Ok(Step::Done);
    }
    match a.stage {
        crate::kernel::resolve::Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let cands: Vec<usize> = (0..g.seats.len())
                    .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
                    .filter(|i| {
                        !g.hand(*i).is_empty()
                            || !g.judge_zone(*i).is_empty()
                            || !g.ids_in(|z| matches!(z, Zone::Equip(s, _) if s == i)).is_empty()
                    })
                    .collect();
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = super::builder(a.actor, "choice").skill("qizhi");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("奇制：弃 {} 一张牌", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动奇制").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let target: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("奇制：目标不合法 {act}"))?;
                Ok(Step::Next(
                    crate::kernel::resolve::Stage::Target,
                    json!({ "target": target }),
                ))
            }
        },
        crate::kernel::resolve::Stage::Target => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let mut cards = g.hand(target);
            cards.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == target)));
            cards.extend(g.judge_zone(target));
            let act = match g.answer.take() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("qizhi");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("弃置【{}】", g.card_label(id)));
                    }
                    return Ok(Step::Need(b.escape("不发动奇制").build()));
                }
                Some(act) => act,
            };
            if act == "pass" {
                return Ok(Step::Done);
            }
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("奇制：未知选项 {act}"))?
                .to_string();
            let owner = target;
            g.to_discard(&id, "qizhi")?;
            g.draw(owner, 1);
            g.bump("turn.qizhi_count");
            g.presentation.push(
                Presentation::new("qizhi", POutcome::Discard { n: 1 })
                    .actor(a.actor)
                    .target(owner)
                    .cards(vec![id])
                    .fx("qizhi"),
            );
            Ok(Step::Done)
        }
        _ => Err("奇制：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 进趋（王基）：结束阶段
// ---------------------------------------------------------------------------

pub const JINZOU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jinzou",
    name: "进趋",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 15,
    can_active: None,
    handler: jinzou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jinzou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 仅在触发阶段校验阶段名（后续阶段 data 已换成 x）。
    if a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("End") {
            return Ok(Step::Done);
        }
        if g.counter("turn.jinzou_done") > 0 {
            return Ok(Step::Done);
        }
        g.bump("turn.jinzou_done");
    }
    let x = g.counter("turn.qizhi_count") as usize;
    match a.stage {
        crate::kernel::resolve::Stage::Choose => match g.answer.take().as_deref() {
            None => {
                if x == 0 {
                    // X=0：不允许发动（复验 47）
                    return Ok(Step::Done);
                }
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("jinzou")
                        .option("choose:jinzou", format!("进趋：摸 2 张，然后将手牌弃至 {x} 张"))
                        .escape("不发动进趋")
                        .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:jinzou") => {
                g.draw(a.actor, 2);
                g.presentation.push(
                    Presentation::new("jinzou", POutcome::Draw { n: 2 })
                        .actor(a.actor)
                        .fx("jinzou_draw"),
                );
                Ok(Step::Next(crate::kernel::resolve::Stage::Target, json!({ "x": x })))
            }
            Some(other) => Err(format!("进趋：未知选项 {other}")),
        },
        crate::kernel::resolve::Stage::Target => {
            let x = a.data["x"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(a.actor);
            if hand.len() <= x {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("jinzou");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("弃置【{}】", g.card_label(id)));
                    }
                    Ok(Step::Need(b.escape_with("confirm", "完成弃牌").build()))
                }
                Some("confirm") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("进趋：未知选项 {act}"))?
                        .to_string();
                    g.to_discard(&id, "jinzou")?;
                    Ok(Step::Next(crate::kernel::resolve::Stage::Target, json!({ "x": x })))
                }
            }
        }
        _ => Err("进趋：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 怀橘（陆绩）：数值钩子（开局 3 橘、摸牌 +1、自动防伤）——见 skills/mod.rs
// ---------------------------------------------------------------------------

pub const HUAJU: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "huaiju",
    name: "怀橘",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 寸目（许攸）：所有摸牌从牌堆底摸——见 skills::draw_from_bottom
// ---------------------------------------------------------------------------

pub const CUNMU: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "cunmu",
    name: "寸目",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};
