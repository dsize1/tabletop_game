//! B5（孙鲁班 / 朱桓 / 顾雍 / 曹叡 / 曹休 / 钟繇）——按 `docs/games/sanguosha/align/B5.json` 第 7 段实现。
//!
//! 本批落实的裁定：
//! - 谮毁接【杀】与**黑色普通锦囊**，且「唯一目标」读**目标列表**长度 = 1；
//! - 奋励「全场最多」必须**严格大于**（并列不算）；
//! - 平寇的「不发动」只放弃**本次**；
//! - 兴衰：逐个询问魏势力 + 支援者**濒死结算结束后受到 1 点伤害**（读伤害事件，不是失去体力）+ 求桃从左手方开始；
//! - 千驹并入统一距离入口（已损失体力 X，与进攻马叠加）；
//! - 佐定：一张牌只触发一次（由钟繇选其中一个目标），覆盖所有指定目标的牌；
//! - 秉壹：弃置 ≠ 使用、不含判定区牌、各阶段都可触发（每阶段一次），展示仅日志摘要。

use serde_json::json;

use crate::cards::{self, CardKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Suit, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 通用小工具
// ---------------------------------------------------------------------------

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn kind_bit(g: &Game, id: &CardId) -> u32 {
    match g.def_of(id).kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    }
}

fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}

fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

/// 目标列表（`TargetChosen` 的 cause）。
fn target_list(a: &Ask) -> Vec<usize> {
    a.data
        .get("cause")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default()
}

fn is_black_card(g: &Game, id: &CardId) -> bool {
    !g.is_red(id)
}

/// 黑色普通锦囊（非延迟、非无懈）。
fn is_black_trick(g: &Game, id: &CardId) -> bool {
    let def = g.def_of(id);
    def.kind == CardKind::Trick
        && !def.tags.delayed
        && !def.tags.wuxie
        && is_black_card(g, id)
}

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 谮毁（孙鲁班）：使用【杀】或黑色普通锦囊指定唯一目标时，可令另一角色代用/成为额外目标
// ---------------------------------------------------------------------------

pub const ZANHUI: SkillDef = SkillDef {
    code: "zanhui",
    name: "谮毁",
    triggers: &[EventKind::TargetChosen],
    window: Window::OwnPhase(Phase::Play),
    priority: 30,
    can_active: None,
    handler: zanhui,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zanhui(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("TargetChosen") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if g.counter("turn.zanhui_used") > 0 {
            return Ok(Step::Done);
        }
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()).map(str::to_string) else {
            return Ok(Step::Done);
        };
        let def = g.def_of(&card);
        let kind_ok = def.tags.sha || is_black_trick(g, &card);
        if !kind_ok {
            return Ok(Step::Done);
        }
        // 「唯一目标」读目标列表长度（裁定 Q2=A）
        let targets = target_list(a);
        if targets.len() != 1 {
            return Ok(Step::Done);
        }
        let target = targets[0];
        if target == a.actor {
            return Ok(Step::Done);
        }
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let target = target_list(a).first().copied().unwrap_or(0);
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| *i != target)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("zanhui");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 选择（代用或成为额外目标）", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动谮毁").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let helper: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("谮毁：未知选项 {act}"))?;
                    if !cands.contains(&helper) {
                        return Err("谮毁：协助者不合法".into());
                    }
                    g.bump("turn.zanhui_used");
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "helper": helper, "card": card, "target": target }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
            let can_give = !hand_and_equip(g, helper).is_empty();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(helper, "choice").skill("zanhui");
                    if can_give {
                        b = b.option(
                            "opt1",
                            format!("交给 {} 一张牌，代替其成为此牌使用者", g.seats[a.actor].name),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_with("opt2", "成为此牌的额外目标").build(),
                    ))
                }
                Some("opt1") => {
                    if !can_give {
                        return Err("谮毁：没有可交出的牌".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "helper": helper, "card": card }),
                    ))
                }
                Some("opt2") => {
                    g.bump("turn.zanhui_extra_target");
                    g.push_log(format!(
                        "谮毁：{} 成为【{}】的额外目标",
                        g.seats[helper].name,
                        g.name_of(&card)
                    ));
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("谮毁：未知选项 {other}")),
            }
        }
        Stage::PickOwn => {
            let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
            let cards = hand_and_equip(g, helper);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(helper, "choice").skill("zanhui");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("交给 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不交出（改为成为额外目标）").build()))
                }
                Some("pass") => {
                    g.bump("turn.zanhui_extra_target");
                    g.push_log(format!("谮毁：{} 成为额外目标", g.seats[helper].name));
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("谮毁：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("谮毁：只能交出自己的牌".into());
                    }
                    let label = label(g, &id);
                    g.gain(&id, a.actor)?;
                    // 使用者变更：记录为「本牌的使用者已改为协助者」（v2 无【杀】结算引擎，先记状态）。
                    g.counters
                        .insert("turn.zanhui_switch_user".to_string(), helper as u32 + 1);
                    g.presentation.push(
                        Presentation::new("zanhui", POutcome::Activated {
                            skill: "谮毁".to_string(),
                        })
                        .actor(helper)
                        .target(a.actor)
                        .cards(vec![id.clone()])
                        .fx("zanhui_switch_user"),
                    );
                    g.push_log(format!(
                        "谮毁：{} 交出【{label}】，代替 {} 成为此牌使用者",
                        g.seats[helper].name, g.seats[a.actor].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("谮毁：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 骄矜（孙鲁班）：男性角色对你造成伤害时，可弃一张装备牌令伤害 -1
// ---------------------------------------------------------------------------

pub const JIAOJIN: SkillDef = SkillDef {
    code: "jiaojin",
    name: "骄矜",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiaojin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 「造成伤害时」受害者侧拦截：返回 true = 已压入询问帧。
pub fn jiaojin_before(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) -> bool {
    if amount <= 0 || !active(g, victim, "jiaojin") {
        return false;
    }
    let Some(src) = source else { return false };
    if src == victim || !g.seats[src].is_male() {
        return false;
    }
    if hand_and_equip(g, victim).is_empty() {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "jiaojin",
        Stage::Choose,
        victim,
        json!({ "amount": amount, "source": src, "card": card.map(|c| c.to_string()) }),
    );
    true
}

fn jiaojin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let amount = a.data["amount"].as_i64().unwrap_or(0) as i32;
    let source = a.data["source"].as_u64().map(|n| n as usize);
    let card = a.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string());
    match a.stage {
        Stage::Choose => {
            let cards = hand_and_equip(g, a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("jiaojin");
                    for id in &cards {
                        if g.def_of(id).kind != CardKind::Equip {
                            continue;
                        }
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置装备牌 {}，伤害 -1", label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape(format!("不发动骄矜（受到 {amount} 点伤害）"))
                            .build(),
                    ))
                }
                Some("pass") => {
                    resume_damage(g, source, a.actor, amount, card.as_deref(), "jiaojin");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("骄矜：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) || g.def_of(&id).kind != CardKind::Equip {
                        return Err("骄矜：只能弃置一张装备牌".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "jiaojin")?;
                    g.presentation.push(
                        Presentation::new("jiaojin", POutcome::Activated {
                            skill: "骄矜".to_string(),
                        })
                        .actor(a.actor)
                        .cards(vec![id.clone()])
                        .fx("jiaojin"),
                    );
                    g.push_log(format!("骄矜：弃置【{label}】，伤害 -1"));
                    let left = (amount - 1).max(0);
                    if left > 0 {
                        resume_damage(g, source, a.actor, left, card.as_deref(), "jiaojin");
                    } else {
                        g.push_log("骄矜：伤害减为 0，本次伤害不发生".to_string());
                    }
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("骄矜：未知阶段".into()),
    }
}

/// 改完数值后继续走**剩余**的「造成伤害时」钩子（自抑制，避免递归）。
fn resume_damage(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&str>,
    own_hook: &str,
) {
    let card = card.map(|c| c.to_string());
    g.suppress_hook(own_hook);
    flow::damage_with(g, source, victim, amount, card.as_ref());
    g.release_hook(own_hook);
}

// ---------------------------------------------------------------------------
// 奋励（朱桓）：按条件跳过摸牌 / 出牌 / 弃牌阶段
// ---------------------------------------------------------------------------

pub const FENLI: SkillDef = SkillDef {
    code: "fenli",
    name: "奋励",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 20,
    can_active: None,
    handler: fenli,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 严格大于其他所有存活角色（裁定 Q1=B：并列不算）。
fn strictly_most(g: &Game, seat: usize, key: impl Fn(usize) -> usize) -> bool {
    let mine = key(seat);
    other_alive(g, seat).into_iter().all(|i| key(i) < mine)
}

fn fenli_condition(g: &Game, seat: usize, phase: Phase) -> bool {
    match phase {
        Phase::Draw => strictly_most(g, seat, |i| g.hand(i).len()),
        Phase::Play => strictly_most(g, seat, |i| g.seats[i].hp.max(0) as usize),
        Phase::Discard => {
            strictly_most(g, seat, |i| equip_ids(g, i).len()) && !equip_ids(g, seat).is_empty()
        }
        _ => false,
    }
}

fn fenli(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("PhaseStart") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let phase = g.phase;
        if !matches!(phase, Phase::Draw | Phase::Play | Phase::Discard) {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.fenli_{phase:?}_asked")) > 0 || !fenli_condition(g, a.actor, phase) {
            return Ok(Step::Done);
        }
    }
    let phase = g.phase;
    let key = format!("turn.fenli_{phase:?}_asked");
    let what = match phase {
        Phase::Draw => "摸牌",
        Phase::Play => "出牌",
        Phase::Discard => "弃牌",
        _ => return Ok(Step::Done),
    };
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("fenli")
                    .option("opt1", format!("奋励：跳过{what}阶段（平寇可造成 1 点伤害）"))
                    .escape("不跳过")
                    .build(),
            )),
            Some("pass") => {
                g.bump(&key);
                Ok(Step::Done)
            }
            Some("opt1") => {
                g.bump(&key);
                match phase {
                    Phase::Draw => {
                        g.bump("turn.skip_draw");
                    }
                    Phase::Play => {
                        g.bump("turn.skip_play");
                    }
                    _ => {
                        g.bump("turn.fenli_skip_discard");
                    }
                }
                g.bump("turn.fenli_skipped");
                g.presentation.push(
                    Presentation::new("fenli", POutcome::Activated {
                        skill: "奋励".to_string(),
                    })
                    .actor(a.actor)
                    .fx("fenli_skip"),
                );
                g.push_log(format!("奋励：跳过{what}阶段"));
                Ok(Step::Done)
            }
            Some(other) => Err(format!("奋励：未知选项 {other}")),
        },
        _ => Err("奋励：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 平寇（朱桓）：回合结束后，每跳过一个阶段对一名其他角色造成 1 点伤害
// ---------------------------------------------------------------------------

pub const PINGKOU: SkillDef = SkillDef {
    code: "pingkou",
    name: "平寇",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 15,
    can_active: None,
    handler: pingkou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn pingkou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("PhaseEnd") {
            return Ok(Step::Done);
        }
        if a.data["cause"].as_str() != Some("End") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if g.counter("turn.fenli_skipped") == 0 {
            return Ok(Step::Done);
        }
    }
    let remaining = a
        .data
        .get("remaining")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or_else(|| g.counter("turn.fenli_skipped") as usize);
    if remaining == 0 {
        return Ok(Step::Done);
    }
    let cands = other_alive(g, a.actor);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "target").skill("pingkou");
                for i in &cands {
                    b = b.option(
                        format!("target:{i}"),
                        format!("平寇：对 {} 造成 1 点伤害（剩 {remaining} 次）", g.seats[*i].name),
                    );
                }
                Ok(Step::Need(b.escape("放弃本次（仍可继续剩余次数）").build()))
            }
            Some("pass") => {
                // 裁定 Q1=B：只放弃本次
                if remaining > 1 {
                    Ok(Step::Next(Stage::Choose, json!({ "remaining": remaining - 1 })))
                } else {
                    Ok(Step::Done)
                }
            }
            Some(act) => {
                let target: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("平寇：目标不合法 {act}"))?;
                if !cands.contains(&target) {
                    return Err("平寇：目标不合法".into());
                }
                g.push_log(format!("平寇：对 {} 造成 1 点伤害", g.seats[target].name));
                flow::damage(g, Some(a.actor), target, 1);
                if remaining > 1 {
                    Ok(Step::Next(Stage::Choose, json!({ "remaining": remaining - 1 })))
                } else {
                    Ok(Step::Done)
                }
            }
            Some(other) => Err(format!("平寇：未知选项 {other}")),
        },
        _ => Err("平寇：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 慎行（顾雍）：弃两张牌（手牌或装备区）摸一张
// ---------------------------------------------------------------------------

pub const SHENXING: SkillDef = SkillDef {
    code: "shenxing",
    name: "慎行",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_shenxing),
    handler: shenxing,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_shenxing(g: &Game, seat: usize) -> bool {
    active(g, seat, "shenxing") && play_phase(g, seat) && hand_and_equip(g, seat).len() >= 2
}

fn shenxing(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cards = hand_and_equip(g, a.actor);
    if cards.len() < 2 {
        return Ok(Step::Done);
    }
    let selected: Vec<CardId> = a.data["selected"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "choose_many").skill("shenxing");
            for id in &cards {
                let mark = if selected.contains(id) { "✔ " } else { "" };
                b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
            }
            if selected.len() == 2 {
                b = b.option("confirm", "确认弃置 2 张，摸 1 张");
            }
            Ok(Step::Need(b.escape("不发动慎行").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            if selected.len() != 2 {
                return Err("慎行：需要弃置 2 张牌".into());
            }
            for id in &selected {
                g.to_discard(id, "shenxing")?;
            }
            let n = g.draw(a.actor, 1).len();
            g.presentation.push(
                Presentation::new("shenxing", POutcome::Draw { n: n as u32 })
                    .actor(a.actor)
                    .cards(selected.clone())
                    .fx("shenxing"),
            );
            g.push_log(format!("慎行：弃置 2 张，摸 {n} 张"));
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("慎行：未知选项 {act}"))?
                .to_string();
            if !cards.contains(&id) {
                return Err("慎行：只能选自己的牌".into());
            }
            let mut next = selected.clone();
            match next.iter().position(|c| c == &id) {
                Some(pos) => {
                    next.remove(pos);
                }
                None => {
                    if next.len() >= 2 {
                        return Err("慎行：只需弃置 2 张".into());
                    }
                    next.push(id);
                }
            }
            Ok(Step::Next(Stage::Choose, json!({ "selected": next })))
        }
    }
}

// ---------------------------------------------------------------------------
// 秉壹（顾雍）：自己的牌因弃置进弃牌堆后展示手牌，同色则可令至多 X 名角色各摸 1
// ---------------------------------------------------------------------------

pub const BINGYI: SkillDef = SkillDef {
    code: "bingyi",
    name: "秉壹",
    triggers: &[EventKind::CardDiscarded],
    window: Window::AnyTime,
    priority: 20,
    can_active: None,
    handler: bingyi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn bingyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("CardDiscarded") {
            return Ok(Step::Done);
        }
        // 「你的牌」：手牌/装备区的牌；**判定区的牌不算**（裁定：弃置不含判定区牌）。
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()) else {
            return Ok(Step::Done);
        };
        if g.counter("phase.bingyi") > 0 {
            return Ok(Step::Done);
        }
        // 判定区的牌移出不发 `CardDiscarded`（mechanics `discard-contract`），因此这里只需 case 存在。
        let _ = card;
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("bingyi")
                    .option("opt1", "秉壹：展示所有手牌（颜色均相同则令 0..X 名其他角色各摸一张）")
                    .escape("不发动秉壹")
                    .build(),
            )),
            Some("pass") => {
                g.bump("phase.bingyi");
                Ok(Step::Done)
            }
            Some("opt1") => {
                g.bump("phase.bingyi");
                let hand = g.hand(a.actor);
                let reds: Vec<bool> = hand.iter().map(|id| g.is_red(id)).collect();
                let same = !reds.is_empty() && reds.iter().all(|r| *r == reds[0]);
                for id in &hand {
                    g.reveal(id, Some(a.actor), "bingyi");
                }
                let shown: Vec<String> = hand.iter().map(|id| label(g, id)).collect();
                // 展示 = 立即进公共可见区（mechanics `reveal`）；日志摘要仅为文案。
                g.push_log(format!(
                    "秉壹：{} 展示所有手牌（{}）",
                    g.seats[a.actor].name,
                    if same {
                        "颜色均相同"
                    } else {
                        "颜色不同"
                    }
                ));
                if !same {
                    for id in &hand {
                        g.conceal(id, "bingyi_done");
                    }
                    return Ok(Step::Done);
                }
                let x = hand.len();
                let n = g.draw(a.actor, 1).len();
                g.presentation.push(
                    Presentation::new("bingyi", POutcome::Draw { n: n as u32 })
                        .actor(a.actor)
                        .cards(hand)
                        .fx("bingyi_reveal"),
                );
                g.push_log(format!(
                    "秉壹：摸 {n} 张；可令至多 {x} 名其他角色各摸 1 张（展示：{}）",
                    shown.join("、")
                ));
                Ok(Step::Next(Stage::Target, json!({ "x": x, "picked": [] })))
            }
            Some(other) => Err(format!("秉壹：未知选项 {other}")),
        },
        Stage::Target => {
            let x = a.data["x"].as_u64().unwrap_or(0) as usize;
            let picked: Vec<usize> = a.data["picked"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !picked.contains(i))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("bingyi");
                    for i in &cands {
                        if picked.len() >= x {
                            break;
                        }
                        b = b.option(format!("target:{i}"), format!("令 {} 摸一张牌", g.seats[*i].name));
                    }
                    if !picked.is_empty() {
                        b = b.option("confirm", format!("确认（{} 名）", picked.len()));
                    }
                    Ok(Step::Need(
                        b.escape("不再选（放弃剩余名额）").build(),
                    ))
                }
                Some("pass") => Ok(Step::Done),
                Some("confirm") => {
                    for i in &picked {
                        g.draw(*i, 1);
                    }
                    for id in g.hand(a.actor) {
                        g.conceal(&id, "bingyi_done");
                    }
                    g.push_log(format!("秉壹：{} 名角色各摸 1 张", picked.len()));
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("秉壹：目标不合法 {act}"))?;
                    if !cands.contains(&t) || picked.len() >= x {
                        return Err("秉壹：目标不合法（或不大于 X）".into());
                    }
                    let mut next = picked.clone();
                    next.push(t);
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "x": x, "picked": next }),
                    ))
                }
            }
        }
        _ => Err("秉壹：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 恢拓（曹叡）：受到伤害后令一名角色判定，红色回血 / 黑色摸伤害值张
// ---------------------------------------------------------------------------

pub const HUITUO: SkillDef = SkillDef {
    code: "huituo",
    name: "恢拓",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 35,
    can_active: None,
    handler: huituo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn huituo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if a.data["amount"].as_i64().unwrap_or(0) <= 0 || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
    }
    let amount = a.data["amount"].as_i64().unwrap_or(1) as i32;
    match a.stage {
        Stage::Choose => {
            let cands = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect::<Vec<_>>();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("huituo");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("令 {} 判定", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动恢拓").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("恢拓：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("恢拓：目标不合法".into());
                    }
                    // 判定：亮出牌堆顶 → 按颜色结算 → 进弃牌堆。
                    let Some(judge) = flow::judge_card(g, t) else {
                        return Ok(Step::Done);
                    };
                    let label = label(g, &judge);
                    if g.is_red(&judge) {
                        // 裁定 Q2=B：满体力直接跳过红色分支
                        if g.seats[t].hp < g.seats[t].max_hp {
                            g.seats[t].hp += 1;
                            g.presentation.push(
                                Presentation::new("huituo", POutcome::Heal { amount: 1 })
                                    .actor(a.actor)
                                    .target(t)
                                    .fx("huituo_red"),
                            );
                            g.push_log(format!(
                                "恢拓：判定【{label}】红色，{} 回复 1 点体力（体力 {}）",
                                g.seats[t].name, g.seats[t].hp
                            ));
                        } else {
                            g.push_log(format!(
                                "恢拓：判定【{label}】红色，但 {} 体力已满（不回复）",
                                g.seats[t].name
                            ));
                        }
                    } else {
                        let n = g.draw(t, amount.max(0) as usize).len();
                        g.presentation.push(
                            Presentation::new("huituo", POutcome::Draw { n: n as u32 })
                                .actor(a.actor)
                                .target(t)
                                .fx("huituo_black"),
                        );
                        g.push_log(format!(
                            "恢拓：判定【{label}】黑色，{} 摸 {n} 张",
                            g.seats[t].name
                        ));
                    }
                    flow::judge_to_discard(g, &judge)?;
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("恢拓：未知选项 {other}")),
            }
        }
        _ => Err("恢拓：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 明鉴（曹叡）：交给一名其他角色所有手牌；其下回合手牌上限 +1、杀次数 +1
// ---------------------------------------------------------------------------

pub const MINGJIAN: SkillDef = SkillDef {
    code: "mingjian",
    name: "明鉴",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_mingjian),
    handler: mingjian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_mingjian(g: &Game, seat: usize) -> bool {
    active(g, seat, "mingjian")
        && play_phase(g, seat)
        && g.counter("turn.mingjian_used") == 0
        && !g.hand(seat).is_empty() // 裁定 Q1=B：手牌为 0 不能发动
        && !other_alive(g, seat).is_empty()
}

fn mingjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands = other_alive(g, a.actor);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("mingjian");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("将全部手牌交给 {}（其下回合手牌上限/杀次数 +1）", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动明鉴").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("明鉴：目标不合法 {act}"))?;
            if !cands.contains(&t) {
                return Err("明鉴：目标不合法".into());
            }
            let cards = g.hand(a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            // 「交给」= 获得语义（裁定 Q2=A）
            for id in &cards {
                g.gain(id, t)?;
            }
            g.bump("turn.mingjian_used");
            g.counters.insert(format!("mingjian:{t}"), 1);
            g.presentation.push(
                Presentation::new("mingjian", POutcome::Activated {
                    skill: "明鉴".to_string(),
                })
                .actor(a.actor)
                .target(t)
                .cards(cards.clone())
                .fx("mingjian"),
            );
            g.push_log(format!(
                "明鉴：交给 {} {} 张手牌；其下回合手牌上限 +1、【杀】次数 +1",
                g.seats[t].name,
                cards.len()
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 兴衰（曹叡·主公技/限定）：濒死时逐个询问魏势力；支援者脱困后受到 1 点伤害
// ---------------------------------------------------------------------------

pub const XINGSHUAI: SkillDef = SkillDef {
    code: "xingshuai",
    name: "兴衰",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: xingshuai,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 兴衰是否可用：主公 + 拥有技 + 未用过 + 有存活魏势力其他人。
pub fn xingshuai_available(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "xingshuai") || !g.seats[seat].flags.contains("lord") {
        return false;
    }
    if g.seats[seat].flags.contains("xingshuai_used") {
        return false;
    }
    !xingshuai_helpers(g, seat).is_empty()
}

fn xingshuai_helpers(g: &Game, seat: usize) -> Vec<usize> {
    let mut order = g.seats_from_left_of(seat);
    order.retain(|i| *i != seat && g.seats[*i].hp > 0 && g.seats[*i].kingdom == "wei");
    order
}

/// 濒死流程调用：压入兴衰询问（逐个询问支援者）。
pub fn start_xingshuai(g: &mut Game, victim: usize) {
    g.seats[victim].flags.insert("xingshuai_used".to_string());
    crate::kernel::resolve::push_ask(
        g,
        "xingshuai",
        Stage::Choose,
        victim,
        json!({ "helpers": xingshuai_helpers(g, victim), "cursor": 0, "accepted": [] }),
    );
}

fn xingshuai(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let helpers: Vec<usize> = a.data["helpers"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let mut accepted: Vec<usize> = a.data["accepted"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    if cursor >= helpers.len() {
        // 结算完毕：记录「脱困后再结算支援者伤害」。
        if !accepted.is_empty() {
            g.after_dying_damage.push((a.actor, accepted.clone()));
        }
        return Ok(Step::Done);
    }
    let helper = helpers[cursor];
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(helper, "choice")
                .skill("xingshuai")
                .option(
                    "opt1",
                    format!(
                        "兴衰：令 {} 回复 1 点体力（你之后受到 1 点伤害）",
                        g.seats[a.actor].name
                    ),
                )
                .escape("不支援")
                .build(),
        )),
        Some("opt1") => {
            if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                g.seats[a.actor].hp += 1;
            }
            accepted.push(helper);
            g.push_log(format!(
                "兴衰：{} 支援，{} 回复 1 点体力（体力 {}）",
                g.seats[helper].name, g.seats[a.actor].name, g.seats[a.actor].hp
            ));
            let mut data = a.data.clone();
            data["cursor"] = json!(cursor + 1);
            data["accepted"] = json!(accepted);
            Ok(Step::Next(Stage::Choose, data))
        }
        Some("pass") => {
            let mut data = a.data.clone();
            data["cursor"] = json!(cursor + 1);
            Ok(Step::Next(Stage::Choose, data))
        }
        Some(other) => Err(format!("兴衰：未知选项 {other}")),
    }
}

/// 脱离濒死后：支援者各受到 1 点伤害（裁定 Q3=A：结算结束后才受伤）。
pub fn settle_after_dying(g: &mut Game, victim: usize) {
    let idx = g
        .after_dying_damage
        .iter()
        .position(|(v, _)| *v == victim);
    let Some(idx) = idx else { return };
    let (_, helpers) = g.after_dying_damage.remove(idx);
    for h in helpers {
        if g.seats[h].hp <= 0 {
            continue;
        }
        g.push_log(format!("兴衰：{} 受到 1 点伤害", g.seats[h].name));
        flow::damage_with(g, None, h, 1, None);
    }
}

// ---------------------------------------------------------------------------
// 千驹（曹休，锁定）：每损失 1 点体力距离 -1 —— 数值在 `Game::distance` 统一入口生效
// ---------------------------------------------------------------------------

pub const QIANJU: SkillDef = SkillDef {
    code: "qianju",
    name: "千驹",
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
};

// ---------------------------------------------------------------------------
// 倾袭（曹休）：【杀】造成伤害时，令目标二选一（弃等量手牌+弃武器 / 伤害 +1）
// ---------------------------------------------------------------------------

pub const QINGXI: SkillDef = SkillDef {
    code: "qingxi",
    name: "倾袭",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qingxi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn weapon_of(g: &Game, seat: usize) -> Option<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, Slot::Weapon) if *s == seat))
        .into_iter()
        .next()
}

/// 「造成伤害时」来源侧拦截：返回 true = 已压入询问帧。
pub fn qingxi_before(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) -> bool {
    let Some(src) = source else { return false };
    if src == victim || amount <= 0 || !active(g, src, "qingxi") {
        return false;
    }
    let Some(card) = card else { return false };
    if !g.def_of(card).tags.sha {
        return false;
    }
    let Some(weapon) = weapon_of(g, src) else {
        return false;
    };
    if g.hook_suppressed("qingxi") {
        return false;
    }
    let range = cards::weapon_range(&g.name_of(&weapon)).unwrap_or(1) as i32;
    crate::kernel::resolve::push_ask(
        g,
        "qingxi",
        Stage::PickOther,
        victim,
        json!({
            "source": src,
            "amount": amount,
            "card": card,
            "range": range.max(1),
            "weapon": weapon,
        }),
    );
    true
}

fn qingxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let source = a.data["source"].as_u64().unwrap_or(0) as usize;
    let amount = a.data["amount"].as_i64().unwrap_or(0) as i32;
    let card = a
        .data
        .get("card")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let range = a.data["range"].as_i64().unwrap_or(1) as usize;
    let weapon = a
        .data
        .get("weapon")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    match a.stage {
        Stage::PickOther => {
            let hand = g.hand(a.actor);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("qingxi");
                    if hand.len() >= range {
                        b = b.option(
                            "opt1",
                            format!("弃置 {range} 张手牌，然后弃置其武器"),
                        );
                    }
                    Ok(Step::Need(b.escape_with("opt2", "令此【杀】伤害 +1").build()))
                }
                Some("opt1") => {
                    if hand.len() < range {
                        return Err("倾袭：手牌不足".into());
                    }
                    Ok(Step::Next(
                        Stage::DiscardTarget,
                        json!({ "source": source, "amount": amount, "card": card, "range": range, "weapon": weapon }),
                    ))
                }
                Some("opt2") => {
                    g.push_log(format!("倾袭：令此【杀】伤害 +1（{}）", amount + 1));
                    resume_damage(g, Some(source), a.actor, amount + 1, card.as_deref(), "qingxi");
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("倾袭：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let hand = g.hand(a.actor);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("qingxi");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("弃置 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不弃置（手牌不足）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("倾袭：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("倾袭：只能弃置自己的手牌".into());
                    }
                    g.to_discard(&id, "qingxi")?;
                    let left = a.data["left"]
                        .as_u64()
                        .map(|n| n as usize)
                        .unwrap_or(range)
                        .saturating_sub(1);
                    if left > 0 && !g.hand(a.actor).is_empty() {
                        let mut data = a.data.clone();
                        data["left"] = json!(left);
                        return Ok(Step::Next(Stage::DiscardTarget, data));
                    }
                    // 弃够后弃武器，再按原伤害结算（继续走剩余「造成伤害时」钩子）。
                    if let Some(w) = weapon {
                        if g.zone_of(&w) == Some(&Zone::Equip(source, Slot::Weapon)) {
                            let wlabel = label(g, &w);
                            g.to_discard(&w, "qingxi")?;
                            g.push_log(format!("倾袭：弃置其武器【{wlabel}】"));
                        }
                    }
                    resume_damage(g, Some(source), a.actor, amount, card.as_deref(), "qingxi");
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("倾袭：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 活墨（钟繇）：黑色非基本牌置牌堆顶，视为使用牌名不同的基本牌
// ---------------------------------------------------------------------------

pub const HUOMO: SkillDef = SkillDef {
    code: "huomo",
    name: "活墨",
    triggers: &[EventKind::CardUsed],
    window: Window::OwnPhase(Phase::Play),
    priority: 60,
    can_active: Some(can_huomo),
    handler: huomo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 本回合用过的基本牌位图（sha=1 / tao=2 / jiu=4）。
fn huomo_used(g: &Game) -> u32 {
    g.counter("turn.huomo_used")
}

fn basic_bit(name: &str) -> Option<u32> {
    match name {
        "杀" => Some(1),
        "桃" => Some(2),
        "酒" => Some(4),
        _ => None,
    }
}

fn can_huomo(g: &Game, seat: usize) -> bool {
    active(g, seat, "huomo") && play_phase(g, seat) && !huomo_cards(g, seat).is_empty()
}

fn huomo_cards(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| {
            let def = g.def_of(id);
            def.kind != CardKind::Basic && !g.is_red(id)
        })
        .collect()
}

fn huomo_names(g: &Game) -> Vec<&'static str> {
    let used = huomo_used(g);
    ["杀", "桃", "酒"]
        .into_iter()
        .filter(|n| {
            let bit = basic_bit(n).unwrap_or(0);
            used & bit == 0
        })
        .collect()
}

fn huomo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cards = huomo_cards(g, a.actor);
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    let names = huomo_names(g);
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "choice").skill("huomo");
                for id in &cards {
                    for n in &names {
                        // 裁定 Q3=A：满血不提供【桃】；【酒】每回合一次。
                        if *n == "桃" && g.seats[a.actor].hp >= g.seats[a.actor].max_hp {
                            continue;
                        }
                        if *n == "酒" && g.counter("turn.huomo_jiu") > 0 {
                            continue;
                        }
                        b = b.option(
                            format!("choose:{id}:{n}"),
                            format!("将 {} 置于牌堆顶，视为使用【{n}】", label(g, id)),
                        );
                    }
                }
                Ok(Step::Need(b.escape("不发动活墨").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let rest = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("活墨：未知选项 {act}"))?;
                let (id, name) = rest
                    .rsplit_once(':')
                    .ok_or_else(|| format!("活墨：未知选项 {act}"))?;
                let id = id.to_string();
                let name = name.to_string();
                if !cards.contains(&id) || !names.contains(&name.as_str()) {
                    return Err("活墨：该组合不可用".into());
                }
                // 置于牌堆顶
                g.put_on_top(&id)?;
                let bit = basic_bit(&name).unwrap_or(0);
                let used = huomo_used(g) | bit;
                g.counters.insert("turn.huomo_used".to_string(), used);
                let label = label(g, &id);
                let outcome = match name.as_str() {
                    "杀" => {
                        g.use_virtual(a.actor, "杀", vec![]);
                        format!("活墨：将【{label}】置于牌堆顶，视为使用【杀】")
                    }
                    "桃" => {
                        if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                            g.seats[a.actor].hp += 1;
                        }
                        format!("活墨：将【{label}】置于牌堆顶，视为使用【桃】（体力 {}）", g.seats[a.actor].hp)
                    }
                    _ => {
                        g.bump("turn.huomo_jiu");
                        g.counters.insert("jiu".to_string(), 1);
                        format!("活墨：将【{label}】置于牌堆顶，视为使用【酒】")
                    }
                };
                g.presentation.push(
                    Presentation::new("huomo", POutcome::Activated {
                        skill: "活墨".to_string(),
                    })
                    .actor(a.actor)
                    .fx("huomo"),
                );
                g.push_log(outcome);
                Ok(Step::Done)
            }
            Some(other) => Err(format!("活墨：未知选项 {other}")),
        },
        _ => Err("活墨：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 佐定（钟繇）：其他角色出牌阶段使用黑桃牌指定目标后（本阶段无伤害）令其中一个目标摸 1
// ---------------------------------------------------------------------------

pub const ZUODING: SkillDef = SkillDef {
    code: "zuoding",
    name: "佐定",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 35,
    can_active: None,
    handler: zuoding,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zuoding(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("TargetChosen") {
            return Ok(Step::Done);
        }
        // 窗口：其他角色 + 其出牌阶段
        let user = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
        if user == a.actor || user != g.turn || g.phase != Phase::Play {
            return Ok(Step::Done);
        }
        // 本阶段没有角色受到过伤害（裁定：读 phase.damage）
        if g.counter("phase.damage") > 0 {
            return Ok(Step::Done);
        }
        let Some(card) = a.data.get("card").and_then(|v| v.as_str()).map(str::to_string) else {
            return Ok(Step::Done);
        };
        if g.cards.get(&card).map(|c| c.suit) != Some(Suit::Spade) {
            return Ok(Step::Done);
        }
        // 一张牌只触发一次（裁定 Q1=A）
        if g.counter(&format!("phase.zuoding:{card}")) > 0 {
            return Ok(Step::Done);
        }
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let targets = target_list(a);
    let cands: Vec<usize> = targets
        .into_iter()
        .filter(|t| *t != a.actor && g.seats[*t].hp > 0)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            g.bump(&format!("phase.zuoding:{card}"));
            let mut b = super::builder(a.actor, "target").skill("zuoding");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("佐定：令 {} 摸一张牌", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动佐定").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("佐定：目标不合法 {act}"))?;
            if !cands.contains(&t) {
                return Err("佐定：只能令其中一个目标摸牌".into());
            }
            let n = g.draw(t, 1).len();
            g.presentation.push(
                Presentation::new("zuoding", POutcome::Draw { n: n as u32 })
                    .actor(a.actor)
                    .target(t)
                    .fx("zuoding"),
            );
            g.push_log(format!("佐定：令 {} 摸 {n} 张", g.seats[t].name));
            Ok(Step::Done)
        }
        Some(other) => Err(format!("佐定：未知选项 {other}")),
    }
}
