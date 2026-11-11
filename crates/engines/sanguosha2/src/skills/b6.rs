//! B6（刘谌 / 张嶷 / 孙休 / 朱治 / 全琮 / 公孙渊 / 郭图逢纪）——按 `align/B6.json` 第 7 段实现。
//!
//! 本批落实的裁定：
//! - 战绝：失效按**发动次数**（2 次）；同一角色因该【决斗】受伤只摸 1 张；
//! - 勤王：刘谌自选牌（手牌/装备区）、**自己不能响应**、激将代出杀的**伤害来源视为刘谌**；
//! - 怃戎：真「同时展示」（选择期不公开，结算时一起亮）；
//! - 矢志：**替代**用法（体力 1 时【闪】只能当【杀】）；
//! - 诏缚：可选自己、**统一攻击范围入口**（`Game::in_attack_range`）；
//! - 安国：按**条件**判定补做；装备从**牌堆**（无则弃牌堆）随机取；
//! - 邀名：造成/受到**各限一次**（含回合外）；
//! - 怀异：走 `flow::lose_hp`；展示立即进公共区；获得张数 = min(弃置数, 其手牌+装备)；
//! - 急攻：本**出牌阶段**造成伤害值赋值手牌上限；可不发动；
//! - 饰非：覆盖「使用」与「打出」两条入口。

use serde_json::json;

use crate::cards::{self, CardKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
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

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

/// 同势力（含自己判断由调用方处理）。
fn same_kingdom(g: &Game, a: usize, b: usize) -> bool {
    g.seats[a].kingdom == g.seats[b].kingdom
}

// ---------------------------------------------------------------------------
// 战绝（刘谌）：所有手牌当【决斗】；与受伤者各摸 1；发动 2 次后本阶段失效
// ---------------------------------------------------------------------------

pub const ZHANJUE: SkillDef = SkillDef {
    code: "zhanjue",
    name: "战绝",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhanjue),
    handler: zhanjue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_zhanjue(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhanjue")
        && play_phase(g, seat)
        && !g.hand(seat).is_empty()
        && g.counter("turn.zhanjue_uses") < 2
        && !other_alive(g, seat).is_empty()
}

fn zhanjue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.stage == Stage::Target {
        return zhanjue_target(g, a);
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("zhanjue")
                .option("choose:zhanjue", "战绝：将所有手牌当【决斗】使用")
                .escape("不发动战绝")
                .build(),
        )),
        Some("pass") => Ok(Step::Done),
        Some("choose:zhanjue") => {
            // M2：真实结算需要选【决斗】目标。
            Ok(Step::Next(Stage::Target, json!({})))
        }
        Some(other) => Err(format!("战绝：未知选项 {other}")),
    }
}

fn zhanjue_target(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands = other_alive(g, a.actor);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("zhanjue");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("对 {} 使用【决斗】", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("取消（不发动战绝）").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let target: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("战绝：目标不合法 {act}"))?;
            if !cands.contains(&target) {
                return Err("战绝：目标不合法".into());
            }
            let origins = g.hand(a.actor);
            if origins.is_empty() {
                return Ok(Step::Done);
            }
            let base = origins[0].clone();
            // 全部手牌当【决斗】；origins 在结算后一并进弃牌堆。
            g.set_virtual(&base, "决斗", origins.clone());
            g.use_as(a.actor, &base, "决斗", vec![target], true, None)?;
            g.bump("turn.zhanjue_uses");
            g.bump("turn.zhanjue_active");
            g.presentation.push(
                Presentation::new("zhanjue", POutcome::Activated {
                    skill: "战绝".to_string(),
                })
                .actor(a.actor)
                .cards(origins.clone())
                .fx("zhanjue"),
            );
            g.push_log(format!(
                "战绝：将 {} 张手牌当【决斗】对 {} 使用（本阶段第 {} 次）",
                origins.len(),
                g.seats[target].name,
                g.counter("turn.zhanjue_uses")
            ));
            Ok(Step::Done)
        }
    }
}

/// 战绝造成伤害后：刘谌与受伤者**各摸 1 张**（同一角色只摸 1 次）。
/// 由【决斗】结算引擎（未来）与用例调用。
pub fn zhanjue_on_damage(g: &mut Game, owner: usize, victim: usize) {
    if !active(g, owner, "zhanjue") || g.counter("turn.zhanjue_active") == 0 {
        return;
    }
    let mut drawn = Vec::new();
    let a = g.draw(owner, 1).len();
    if a > 0 {
        drawn.push(g.seats[owner].name.clone());
    }
    if victim != owner {
        let b = g.draw(victim, 1).len();
        if b > 0 {
            drawn.push(g.seats[victim].name.clone());
        }
    }
    g.push_log(format!("战绝：{} 各摸 1 张", drawn.join("、")));
}

// ---------------------------------------------------------------------------
// 勤王（刘谌·主公技）：弃一张牌（手牌/装备区）发动激将；响应者摸 1；伤害来源=刘谌
// ---------------------------------------------------------------------------

pub const QINWANG: SkillDef = SkillDef {
    code: "qinwang",
    name: "勤王",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_qinwang),
    handler: qinwang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qinwang_allies(g: &Game, seat: usize) -> Vec<usize> {
    // 自己不能响应（裁定 Q3=B）；且必须有【杀】可代出。
    other_alive(g, seat)
        .into_iter()
        .filter(|i| same_kingdom(g, seat, *i))
        .filter(|i| g.hand(*i).iter().any(|id| g.def_of(id).tags.sha))
        .collect()
}

fn can_qinwang(g: &Game, seat: usize) -> bool {
    active(g, seat, "qinwang")
        && g.seats[seat].flags.contains("lord")
        && play_phase(g, seat)
        && !hand_and_equip(g, seat).is_empty()
        && !qinwang_allies(g, seat).is_empty()
}

fn qinwang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let costs = hand_and_equip(g, a.actor);
            if costs.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("qinwang");
                    for id in &costs {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {} 发动激将", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动勤王").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("勤王：未知选项 {act}"))?
                        .to_string();
                    if !costs.contains(&id) {
                        return Err("勤王：只能弃置自己的手牌或装备区牌".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "qinwang")?;
                    g.push_log(format!("勤王：弃置【{label}】发动激将"));
                    Ok(Step::Next(Stage::Target, json!({})))
                }
            }
        }
        Stage::Target => {
            let cands = qinwang_allies(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("qinwang");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 代出【杀】（其摸一张）", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("无人代出（激将取消）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let ally: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("勤王：目标不合法 {act}"))?;
                    if !cands.contains(&ally) {
                        return Err("勤王：只能令同势力其他角色代出".into());
                    }
                    g.push_log(format!("勤王：令 {} 代出【杀】", g.seats[ally].name));
                    Ok(Step::Next(Stage::CardOpt, json!({ "ally": ally })))
                }
            }
        }
        Stage::CardOpt => {
            let ally = a.data["ally"]
                .as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| "勤王：缺少代出角色".to_string())?;
            let shas: Vec<CardId> = g
                .hand(ally)
                .into_iter()
                .filter(|id| g.def_of(id).tags.sha)
                .collect();
            let cands: Vec<usize> = other_alive(g, ally)
                .into_iter()
                .filter(|i| g.in_attack_range(ally, *i))
                .collect();
            if shas.is_empty() || cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(ally, "qinwang_sha").skill("qinwang");
                    for id in &shas {
                        for t in &cands {
                            b = b.option(
                                format!("choose:{id}:{t}"),
                                format!(
                                    "用【{}】对 {} 使用【杀】",
                                    label(g, id),
                                    g.seats[*t].name
                                ),
                            );
                        }
                    }
                    Ok(Step::Need(b.escape("不出【杀】（激将取消）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let rest = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("勤王：未知选项 {act}"))?;
                    let (id, target) = rest
                        .rsplit_once(':')
                        .ok_or_else(|| format!("勤王：未知选项 {act}"))?;
                    let id = id.to_string();
                    let target: usize = target
                        .parse()
                        .map_err(|_| format!("勤王：目标不合法 {act}"))?;
                    if !shas.contains(&id) || !cands.contains(&target) {
                        return Err("勤王：该组合不可用".into());
                    }
                    // 响应此激将的角色摸一张；伤害来源视为刘谌（裁定 Q3）。
                    let n = g.draw(ally, 1).len();
                    g.use_as(
                        ally,
                        &id,
                        "杀",
                        vec![target],
                        false,
                        Some(a.actor),
                    )?;
                    g.presentation.push(
                        Presentation::new("qinwang", POutcome::Activated {
                            skill: "勤王".to_string(),
                        })
                        .actor(a.actor)
                        .target(ally)
                        .fx("qinwang"),
                    );
                    g.push_log(format!(
                        "勤王：{} 代出【杀】（摸 {n} 张）对 {}；伤害来源视为 {}",
                        g.seats[ally].name,
                        g.seats[target].name,
                        g.seats[a.actor].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("勤王：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 怃戎（张嶷）：与一名其他角色同时展示一张手牌（选择期不公开）
// ---------------------------------------------------------------------------

pub const WURONG: SkillDef = SkillDef {
    code: "wurong",
    name: "怃戎",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_wurong),
    handler: wurong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_wurong(g: &Game, seat: usize) -> bool {
    active(g, seat, "wurong")
        && play_phase(g, seat)
        && g.counter("turn.wurong_used") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat)
            .into_iter()
            .any(|i| !g.hand(i).is_empty())
}

fn wurong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let hand = g.hand(a.actor);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("wurong");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("选择 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不发动怃戎").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("怃戎：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("怃戎：只能展示自己的手牌".into());
                    }
                    g.bump("turn.wurong_used");
                    // 选择期不公开（裁定 Q1）：不 reveal、不写牌面日志
                    g.push_log("怃戎：你已选好展示牌（待同时亮牌）".to_string());
                    Ok(Step::Next(Stage::Target, json!({ "own": id })))
                }
            }
        }
        Stage::Target => {
            let own = a.data["own"].as_str().unwrap_or_default().to_string();
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !g.hand(*i).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("wurong");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("与 {} 同时展示", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消（展示牌不公开）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("怃戎：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("怃戎：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "own": own, "target": t }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let own = a.data["own"].as_str().unwrap_or_default().to_string();
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(target);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(target, "choice").skill("wurong");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("选择 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不展示（怃戎取消）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let theirs = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("怃戎：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&theirs) {
                        return Err("怃戎：只能展示自己的手牌".into());
                    }
                    // 结算时**同时亮牌**（裁定 Q3）
                    g.reveal(&own, Some(a.actor), "wurong");
                    g.reveal(&theirs, Some(target), "wurong");
                    resolve_wurong(g, a.actor, target, &own, &theirs)?;
                    g.conceal(&own, "wurong_done");
                    g.conceal(&theirs, "wurong_done");
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("怃戎：未知阶段".into()),
    }
}

fn resolve_wurong(
    g: &mut Game,
    actor: usize,
    target: usize,
    own: &CardId,
    theirs: &CardId,
) -> Result<(), String> {
    let own_sha = g.def_of(own).tags.sha;
    let their_shan = g.def_of(theirs).tags.shan;
    g.push_log(format!(
        "怃戎：张嶷展示【{}】，{} 展示【{}】",
        label(g, own),
        g.seats[target].name,
        label(g, theirs)
    ));
    if own_sha && !their_shan {
        g.to_discard(own, "wurong")?;
        g.push_log("怃戎：弃置此【杀】并造成 1 点伤害".to_string());
        flow::damage(g, Some(actor), target, 1);
        return Ok(());
    }
    if !own_sha && their_shan {
        g.to_discard(own, "wurong")?;
        // 由张嶷选牌（裁定 Q2）
        let cands: Vec<CardId> = {
            let mut v = g.judge_zone(target);
            v.extend(equip_ids(g, target));
            v.extend(g.hand(target));
            v
        };
        if let Some(card) = cands.first().cloned() {
            g.gain(&card, actor)?;
            g.push_log(format!(
                "怃戎：弃置【{}】并获得 {} 的【{}】",
                label(g, own),
                g.seats[target].name,
                label(g, &card)
            ));
        }
        return Ok(());
    }
    g.push_log("怃戎：双方展示组合不满足结算条件（无事发生）".to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// 矢志（张嶷·锁定）：体力 1 时【闪】视为【杀】（替代用法）
// ---------------------------------------------------------------------------

pub const SHIZHI: SkillDef = SkillDef {
    code: "shizhi",
    name: "矢志",
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

/// 矢志：该牌是否**不能**再当【闪】使用/打出（体力 1 时【闪】被替代为【杀】）。
pub fn shizhi_blocks_shan(g: &Game, seat: usize, id: &CardId) -> bool {
    active(g, seat, "shizhi") && g.seats[seat].hp == 1 && g.def_of(id).tags.shan
}

// ---------------------------------------------------------------------------
// 宴诛（孙休）：目标二选一（交出装备并修改兴学 / 弃一张牌）
// ---------------------------------------------------------------------------

pub const YANZHU: SkillDef = SkillDef {
    code: "yanzhu",
    name: "宴诛",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_yanzhu),
    handler: yanzhu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_yanzhu(g: &Game, seat: usize) -> bool {
    active(g, seat, "yanzhu")
        && play_phase(g, seat)
        && g.counter("turn.yanzhu_used") == 0
        && other_alive(g, seat).into_iter().any(|i| {
            !g.hand(i).is_empty() || !equip_ids(g, i).is_empty()
        })
}

fn yanzhu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !g.hand(*i).is_empty() || !equip_ids(g, *i).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("yanzhu");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("令 {} 二选一", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动宴诛").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("宴诛：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("宴诛：目标不合法".into());
                    }
                    g.bump("turn.yanzhu_used");
                    Ok(Step::Next(Stage::PickOther, json!({ "target": t })))
                }
            }
        }
        Stage::PickOther => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let equips = equip_ids(g, t);
            let has_hand = !g.hand(t).is_empty() || !equips.is_empty();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("yanzhu");
                    if !equips.is_empty() {
                        b = b.option(
                            "opt1",
                            format!("交出装备区所有牌（{} 张），孙休失去宴诛并修改兴学", equips.len()),
                        );
                    }
                    Ok(Step::Need(if has_hand {
                        b.escape_with("opt2", "弃置一张牌（手牌或装备区）").build()
                    } else {
                        b.escape("没有可弃置的牌").build()
                    }))
                }
                Some("opt1") => {
                    if equips.is_empty() {
                        return Err("宴诛：其装备区没有牌".into());
                    }
                    for id in &equips {
                        g.gain(id, a.actor)?;
                    }
                    // 失去宴诛（技能状态走 active 单一源）
                    g.seats[a.actor].skills.retain(|s| s != "yanzhu");
                    g.seats[a.actor].granted.retain(|s| s != "yanzhu");
                    g.seats[a.actor].flags.insert("xingxue_upgraded".to_string());
                    crate::skills::rebuild_subs(g);
                    g.push_log(format!(
                        "宴诛：{} 交出 {} 张装备牌；孙休失去宴诛并修改兴学",
                        g.seats[t].name,
                        equips.len()
                    ));
                    Ok(Step::Done)
                }
                Some("opt2") => Ok(Step::Next(Stage::DiscardTarget, json!({ "target": t }))),
                Some(other) => Err(format!("宴诛：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let cards = hand_and_equip(g, t);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("yanzhu");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("弃置 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不弃置（无牌可弃）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("宴诛：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("宴诛：只能弃置自己的牌".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "yanzhu")?;
                    g.push_log(format!("宴诛：{} 弃置【{label}】", g.seats[t].name));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("宴诛：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 兴学（孙休）：结束阶段，每点体力可令一名角色摸 1 并置顶一张（由其自选）
// ---------------------------------------------------------------------------

pub const XINGXUE: SkillDef = SkillDef {
    code: "xingxue",
    name: "兴学",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 20,
    can_active: None,
    handler: xingxue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn xingxue(g: &mut Game, a: &Ask) -> Result<Step, String> {
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
    }
    let remaining = a
        .data
        .get("remaining")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or_else(|| {
            if g.seats[a.actor].flags.contains("xingxue_upgraded") {
                1
            } else {
                g.seats[a.actor].hp.max(1) as usize
            }
        });
    if remaining == 0 {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("xingxue");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 摸一张并置于牌堆顶（剩 {remaining} 次）", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动兴学（可放弃剩余次数）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("兴学：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("兴学：目标不合法".into());
                    }
                    let n = g.draw(t, 1).len();
                    if n == 0 || g.hand(t).is_empty() {
                        return next_xingxue(g, remaining - 1);
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "target": t, "remaining": remaining }),
                    ))
                }
            }
        }
        Stage::PickOwn => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let remaining = a.data["remaining"].as_u64().unwrap_or(1) as usize;
            let hand = g.hand(t);
            if hand.is_empty() {
                return next_xingxue(g, remaining - 1);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("xingxue");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("将 {} 置于牌堆顶", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不置顶（跳过剩余）").build()))
                }
                Some("pass") => next_xingxue(g, remaining - 1),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("兴学：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("兴学：只能置顶自己的手牌".into());
                    }
                    let label = label(g, &id);
                    g.put_on_top(&id)?;
                    g.push_log(format!(
                        "兴学：{} 摸一张并将【{label}】置于牌堆顶",
                        g.seats[t].name
                    ));
                    next_xingxue(g, remaining - 1)
                }
            }
        }
        _ => Err("兴学：未知阶段".into()),
    }
}

fn next_xingxue(g: &mut Game, remaining: usize) -> Result<Step, String> {
    let _ = g;
    if remaining > 0 {
        Ok(Step::Next(Stage::Choose, json!({ "remaining": remaining })))
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 诏缚（孙休·主公技 / 限定）：至多两名角色（含自己）视为在吴势力攻击范围内
// ---------------------------------------------------------------------------

pub const ZHAOFU: SkillDef = SkillDef {
    code: "zhaofu",
    name: "诏缚",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhaofu),
    handler: zhaofu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_zhaofu(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhaofu")
        && g.seats[seat].flags.contains("lord")
        && play_phase(g, seat)
        && !g.seats[seat].flags.contains("zhaofu_used")
}

fn zhaofu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let picked: Vec<usize> = a.data["picked"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    let cands: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .filter(|i| picked.len() < 2 || picked.contains(i))
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "choice").skill("zhaofu");
            for i in &cands {
                let mark = if picked.contains(i) { "✔ " } else { "" };
                b = b.option(format!("target:{i}"), format!("{mark}{}", g.seats[*i].name));
            }
            if !picked.is_empty() {
                b = b.option("confirm", format!("确认（{} 名）", picked.len()));
            }
            Ok(Step::Need(b.escape("不发动诏缚").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            if picked.is_empty() {
                return Err("诏缚：至少选择 1 名角色".into());
            }
            for i in &picked {
                g.seats[*i].flags.insert("zhaofu".to_string());
            }
            g.seats[a.actor].flags.insert("zhaofu_used".to_string());
            g.presentation.push(
                Presentation::new("zhaofu", POutcome::Activated {
                    skill: "诏缚".to_string(),
                })
                .actor(a.actor)
                .fx("zhaofu"),
            );
            g.push_log(format!(
                "诏缚：{} 名角色视为在所有其他吴势力角色的攻击范围内",
                picked.len()
            ));
            Ok(Step::Done)
        }
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("诏缚：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("诏缚：目标不合法（至多两名）".into());
            }
            let mut next = picked.clone();
            match next.iter().position(|x| *x == t) {
                Some(pos) => {
                    next.remove(pos);
                }
                None => next.push(t),
            }
            Ok(Step::Next(Stage::Choose, json!({ "picked": next })))
        }
    }
}

// ---------------------------------------------------------------------------
// 安国（朱治）：按全场最少的三项执行，然后补做自己满足的未满足项
// ---------------------------------------------------------------------------

pub const ANGUO: SkillDef = SkillDef {
    code: "anguo",
    name: "安国",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_anguo),
    handler: anguo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_anguo(g: &Game, seat: usize) -> bool {
    active(g, seat, "anguo")
        && play_phase(g, seat)
        && g.counter("turn.anguo_used") == 0
        && !other_alive(g, seat).is_empty()
}

fn min_of(g: &Game, key: impl Fn(usize) -> usize) -> usize {
    (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .map(key)
        .min()
        .unwrap_or(0)
}

fn anguo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands = other_alive(g, a.actor);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("anguo");
            for i in &cands {
                b = b.option(format!("target:{i}"), format!("对 {} 发动安国", g.seats[*i].name));
            }
            Ok(Step::Need(b.escape("不发动安国").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("安国：目标不合法 {act}"))?;
            if !cands.contains(&t) {
                return Err("安国：目标不合法".into());
            }
            g.bump("turn.anguo_used");
            resolve_anguo(g, a.actor, t)?;
            Ok(Step::Done)
        }
    }
}

/// 从牌堆找第一张装备牌（无则弃牌堆）；返回使用的牌。
fn take_equip_from_piles(g: &mut Game) -> Option<CardId> {
    let from_draw = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.def_of(id).kind == CardKind::Equip);
    if let Some(id) = from_draw {
        return Some(id);
    }
    g.ids_in(|z| *z == Zone::Discard)
        .into_iter()
        .find(|id| g.def_of(id).kind == CardKind::Equip)
}

fn use_equip_from_piles(g: &mut Game, seat: usize) -> bool {
    let Some(id) = take_equip_from_piles(g) else {
        return false;
    };
    let Some(slot) = g.cards.get(&id).and_then(|c| c.slot) else {
        return false;
    };
    if g.is_slot_abolished(seat, slot) {
        // 该栏已废除：换一个空栏
        let alt = [Slot::Weapon, Slot::Armor, Slot::HorseOffense, Slot::HorseDefense, Slot::Treasure]
            .into_iter()
            .find(|s| !g.is_slot_abolished(seat, *s));
        let Some(alt) = alt else { return false };
        if g.to_equip(&id, seat, alt).is_err() {
            return false;
        }
        return true;
    }
    if g.to_equip(&id, seat, slot).is_err() {
        return false;
    }
    g.push_log(format!("安国：{} 随机使用装备【{}】", g.seats[seat].name, label(g, &id)));
    true
}

fn resolve_anguo(g: &mut Game, actor: usize, target: usize) -> Result<(), String> {
    let min_hand = min_of(g, |i| g.hand(i).len());
    let min_hp = min_of(g, |i| g.seats[i].hp.max(0) as usize);
    let min_equip = min_of(g, |i| equip_ids(g, i).len());
    let t_hand_min = g.hand(target).len() == min_hand;
    let t_hp_min = g.seats[target].hp.max(0) as usize == min_hp;
    let t_equip_min = equip_ids(g, target).len() == min_equip;
    let mut applied = [false; 3];
    if t_hand_min {
        let n = g.draw(target, 1).len();
        g.push_log(format!("安国：{} 摸 {n} 张", g.seats[target].name));
        applied[0] = true;
    }
    if t_hp_min {
        if g.seats[target].hp < g.seats[target].max_hp {
            g.seats[target].hp += 1;
        }
        g.push_log(format!("安国：{} 回复 1 点体力", g.seats[target].name));
        applied[1] = true;
    }
    if t_equip_min {
        // 裁定 Q2=C：从牌堆（无则弃牌堆）随机取装备；都没有也算「满足」
        use_equip_from_piles(g, target);
        applied[2] = true;
    }
    g.presentation.push(
        Presentation::new("anguo", POutcome::Activated {
            skill: "安国".to_string(),
        })
        .actor(actor)
        .target(target)
        .fx("anguo"),
    );
    // 补做：目标**未满足条件**、但朱治满足的项（裁定 Q1=A：按条件判定）
    if !t_hand_min && g.hand(actor).len() == min_hand {
        let n = g.draw(actor, 1).len();
        g.push_log(format!("安国：朱治摸 {n} 张"));
    }
    if !t_hp_min && g.seats[actor].hp.max(0) as usize == min_hp {
        if g.seats[actor].hp < g.seats[actor].max_hp {
            g.seats[actor].hp += 1;
        }
        g.push_log("安国：朱治回复 1 点体力".to_string());
    }
    if !t_equip_min && equip_ids(g, actor).len() == min_equip {
        use_equip_from_piles(g, actor);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 邀名（全琮）：造成 / 受到伤害后各限一次
// ---------------------------------------------------------------------------

pub const YAOMING: SkillDef = SkillDef {
    code: "yaoming",
    name: "邀名",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 25,
    can_active: None,
    handler: yaoming,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yaoming(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        let victim = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
        let source = a
            .data
            .get("cause")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok());
        // 造成 / 受到 两个方向各限一次（裁定 Q2=B）
        let key = if source == Some(a.actor) && victim != a.actor {
            "turn.yaoming_dealt"
        } else if victim == a.actor && source != Some(a.actor) {
            "turn.yaoming_taken"
        } else {
            return Ok(Step::Done);
        };
        if g.counter(key) > 0 {
            return Ok(Step::Done);
        }
        let mine = g.hand(a.actor).len();
        let has = other_alive(g, a.actor)
            .into_iter()
            .any(|i| g.hand(i).len() != mine);
        if !has {
            return Ok(Step::Done);
        }
        g.bump(key);
    }
    let mine = g.hand(a.actor).len();
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = other_alive(g, a.actor).into_iter().collect();
            let mut b = super::builder(a.actor, "choice").skill("yaoming");
            let mut any = false;
            for i in &cands {
                if g.hand(*i).len() > mine {
                    any = true;
                    b = b.option(
                        format!("discard:{i}"),
                        format!("弃置 {} 一张手牌（{} 张）", g.seats[*i].name, g.hand(*i).len()),
                    );
                } else if g.hand(*i).len() < mine {
                    any = true;
                    b = b.option(
                        format!("draw:{i}"),
                        format!("令 {} 摸一张牌（{} 张）", g.seats[*i].name, g.hand(*i).len()),
                    );
                }
            }
            if !any {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不发动邀名").build())),
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    if let Some(rest) = act.strip_prefix("draw:") {
                        let t: usize = rest.parse().map_err(|_| "邀名：目标不合法")?;
                        let n = g.draw(t, 1).len();
                        g.presentation.push(
                            Presentation::new("yaoming", POutcome::Draw { n: n as u32 })
                                .actor(a.actor)
                                .target(t)
                                .fx("yaoming_draw"),
                        );
                        g.push_log(format!("邀名：令 {} 摸 {n} 张", g.seats[t].name));
                        return Ok(Step::Done);
                    }
                    if let Some(rest) = act.strip_prefix("discard:") {
                        let t: usize = rest.parse().map_err(|_| "邀名：目标不合法")?;
                        return Ok(Step::Next(Stage::DiscardTarget, json!({ "target": t })));
                    }
                    Err(format!("邀名：未知选项 {act}"))
                }
                Some(other) => Err(format!("邀名：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(t);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("yaoming");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {} 的 {}", g.seats[t].name, label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不弃置").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("邀名：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("邀名：只能弃置其手牌".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "yaoming")?;
                    g.presentation.push(
                        Presentation::new("yaoming", POutcome::Discard { n: 1 })
                            .actor(a.actor)
                            .target(t)
                            .fx("yaoming_discard"),
                    );
                    g.push_log(format!("邀名：弃置 {} 的【{label}】", g.seats[t].name));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("邀名：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 怀异（公孙渊）：展示手牌弃一种颜色；每弃一张获得一名角色一张牌
// ---------------------------------------------------------------------------

pub const HUAIYI: SkillDef = SkillDef {
    code: "huaiyi",
    name: "怀异",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_huaiyi),
    handler: huaiyi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_huaiyi(g: &Game, seat: usize) -> bool {
    active(g, seat, "huaiyi")
        && play_phase(g, seat)
        && g.counter("turn.huaiyi_used") == 0
        && !g.hand(seat).is_empty()
}

fn huaiyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let remaining = a.data["remaining"].as_u64().unwrap_or(0) as usize;
    let gained = a.data["gained"].as_u64().unwrap_or(0) as usize;
    match a.stage {
        Stage::Choose => {
            let hand = g.hand(a.actor);
            let red: Vec<CardId> = hand.iter().filter(|id| g.is_red(id)).cloned().collect();
            let black: Vec<CardId> = hand.iter().filter(|id| !g.is_red(id)).cloned().collect();
            let mut b = super::builder(a.actor, "choice").skill("huaiyi");
            if !red.is_empty() {
                b = b.option("opt_red", format!("展示手牌，弃置所有红色手牌（{} 张）", red.len()));
            }
            if !black.is_empty() {
                b = b.option("opt_black", format!("展示手牌，弃置所有黑色手牌（{} 张）", black.len()));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不发动怀异").build())),
                Some("pass") => Ok(Step::Done),
                Some(act @ ("opt_red" | "opt_black")) => {
                    let is_red = act == "opt_red";
                    // 展示所有手牌（立即进公共区）
                    for id in &hand {
                        g.reveal(id, Some(a.actor), "huaiyi");
                    }
                    let cards: Vec<CardId> = hand
                        .iter()
                        .filter(|id| g.is_red(id) == is_red)
                        .cloned()
                        .collect();
                    if cards.is_empty() {
                        return Ok(Step::Done);
                    }
                    g.bump("turn.huaiyi_used");
                    for id in &cards {
                        g.to_discard(id, "huaiyi")?;
                    }
                    for id in &hand {
                        g.conceal(id, "huaiyi_done");
                    }
                    g.push_log(format!(
                        "怀异：展示手牌，弃置 {} 张{}色牌",
                        cards.len(),
                        if is_red { "红" } else { "黑" }
                    ));
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "remaining": cards.len(), "gained": 0 }),
                    ))
                }
                Some(other) => Err(format!("怀异：未知选项 {other}")),
            }
        }
        Stage::Target => {
            if remaining == 0 {
                return finish_huaiyi(g, a.actor, gained);
            }
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| g.seats[*i].hp > 0)
                .filter(|i| !hand_and_equip(g, *i).is_empty() || !g.judge_zone(*i).is_empty())
                .collect();
            if cands.is_empty() {
                return finish_huaiyi(g, a.actor, gained);
            }
            let mut b = super::builder(a.actor, "target").skill("huaiyi");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("获得 {} 一张牌（还剩 {remaining} 张）", g.seats[*i].name),
                );
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不再获得（结束怀异）").build())),
                Some("pass") => finish_huaiyi(g, a.actor, gained),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("怀异：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("怀异：目标不合法".into());
                    }
                    // 一张一张选（裁定 Q3）：列出其区域里的牌
                    let mut cards = g.judge_zone(t);
                    cards.extend(equip_ids(g, t));
                    cards.extend(g.hand(t));
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "target": t, "cards": cards, "remaining": remaining, "gained": gained }),
                    ))
                }
                Some(other) => Err(format!("怀异：未知选项 {other}")),
            }
        }
        Stage::PickOwn => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let cards: Vec<CardId> = a.data["cards"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let mut b = super::builder(a.actor, "choice").skill("huaiyi");
            for id in &cards {
                b = b.option(format!("choose:{id}"), format!("获得 {}", label(g, id)));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("跳过该角色").build())),
                Some("pass") => Ok(Step::Next(
                    Stage::Target,
                    json!({ "remaining": remaining, "gained": gained }),
                )),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("怀异：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("怀异：只能获得其区域里的牌".into());
                    }
                    let label = label(g, &id);
                    g.gain(&id, a.actor)?;
                    g.push_log(format!("怀异：获得 {} 的【{label}】", g.seats[t].name));
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "remaining": remaining.saturating_sub(1), "gained": gained + 1 }),
                    ))
                }
                Some(other) => Err(format!("怀异：未知选项 {other}")),
            }
        }
        _ => Err("怀异：未知阶段".into()),
    }
}

fn finish_huaiyi(g: &mut Game, actor: usize, gained: usize) -> Result<Step, String> {
    if gained >= 2 {
        g.presentation.push(
            Presentation::new("huaiyi", POutcome::Activated {
                skill: "怀异".to_string(),
            })
            .actor(actor)
            .fx("huaiyi_cost"),
        );
        g.push_log(format!("怀异：获得 {gained} 张牌，失去 1 点体力"));
        // 走统一 lose_hp（裁定 Q1=A）
        flow::lose_hp(g, actor, 1, "huaiyi");
    } else {
        g.presentation.push(
            Presentation::new("huaiyi", POutcome::Activated {
                skill: "怀异".to_string(),
            })
            .actor(actor)
            .fx("huaiyi"),
        );
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 急攻（郭图逢纪）：出牌阶段开始可摸 2；本阶段手牌上限 = 本阶段伤害值
// ---------------------------------------------------------------------------

pub const JIGONG: SkillDef = SkillDef {
    code: "jigong",
    name: "急攻",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 30,
    can_active: None,
    handler: jigong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jigong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("PhaseStart") {
            return Ok(Step::Done);
        }
        if a.data["cause"].as_str() != Some("Play") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if g.counter("turn.jigong_offered") > 0 {
            return Ok(Step::Done);
        }
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("jigong")
                .option("opt1", "急攻：摸两张牌，本出牌阶段手牌上限等于造成的伤害值")
                .escape("不发动急攻（手牌上限按体力值）")
                .build(),
        )),
        Some("pass") => {
            g.bump("turn.jigong_offered");
            Ok(Step::Done)
        }
        Some("opt1") => {
            g.bump("turn.jigong_offered");
            let n = g.draw(a.actor, 2).len();
            g.seats[a.actor].flags.insert("jigong".to_string());
            g.presentation.push(
                Presentation::new("jigong", POutcome::Draw { n: n as u32 })
                    .actor(a.actor)
                    .fx("jigong"),
            );
            g.push_log(format!(
                "急攻：摸 {n} 张，本出牌阶段手牌上限 = 造成伤害值"
            ));
            Ok(Step::Done)
        }
        Some(other) => Err(format!("急攻：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 饰非（郭图逢纪）：「需要使用或打出【闪】」时的替代入口（使用 + 打出）
// ---------------------------------------------------------------------------

pub const SHIFEI: SkillDef = SkillDef {
    code: "shifei",
    name: "饰非",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: shifei,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 「需要使用/打出【闪】」时可提供的替代选项（使用与打出共用）。
pub fn shifei_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !active(g, seat, "shifei") {
        return Vec::new();
    }
    vec![(
        "skill:shifei".to_string(),
        "饰非：令当前回合角色摸一张，视为使用/打出【闪】".to_string(),
    )]
}

/// 执行饰非（返回 true = 已处理该选项）。
pub fn shifei_use(g: &mut Game, seat: usize, action: &str) -> Result<bool, String> {
    if action != "skill:shifei" {
        return Ok(false);
    }
    if !active(g, seat, "shifei") {
        return Err("饰非：没有该技能".into());
    }
    let cur = g.turn;
    let n = g.draw(cur, 1).len();
    g.push_log(format!(
        "饰非：当前回合角色 {} 摸 {n} 张",
        g.seats[cur].name
    ));
    // 若其不是手牌数全场唯一最多 → 弃置最多者一张牌（由饰非发动者选）
    let mine = g.hand(cur).len();
    let max = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .map(|i| g.hand(i).len())
        .max()
        .unwrap_or(0);
    let max_seats: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .filter(|i| g.hand(*i).len() == max)
        .collect();
    let unique_cur = max_seats.len() == 1 && max_seats[0] == cur && mine == max;
    if unique_cur || max == 0 {
        g.push_log("饰非：当前回合角色是唯一手牌最多者（不弃置），视为使用/打出【闪】".to_string());
        g.bump("turn.shifei_used");
        return Ok(true);
    }
    crate::kernel::resolve::push_ask(
        g,
        "shifei",
        Stage::DiscardTarget,
        seat,
        json!({ "candidates": max_seats, "cur": cur }),
    );
    Ok(true)
}

fn shifei(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::DiscardTarget => {
            let cands: Vec<usize> = a.data["candidates"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let hand_and_eq: Vec<(usize, CardId)> = cands
                .iter()
                .flat_map(|i| hand_and_equip(g, *i).into_iter().map(move |c| (*i, c)))
                .collect();
            if hand_and_eq.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("shifei");
                    for (i, id) in &hand_and_eq {
                        b = b.option(
                            format!("choose:{i}:{id}"),
                            format!("弃置 {} 的 {}", g.seats[*i].name, label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不弃置").build()))
                }
                Some("pass") => {
                    g.bump("turn.shifei_used");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let rest = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("饰非：未知选项 {act}"))?;
                    let (i, id) = rest
                        .split_once(':')
                        .ok_or_else(|| format!("饰非：未知选项 {act}"))?;
                    let i: usize = i.parse().map_err(|_| "饰非：目标不合法")?;
                    let id = id.to_string();
                    if !hand_and_eq.iter().any(|(s, c)| *s == i && *c == id) {
                        return Err("饰非：只能弃置最多者的手牌或装备".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "shifei")?;
                    g.bump("turn.shifei_used");
                    g.presentation.push(
                        Presentation::new("shifei", POutcome::Discard { n: 1 })
                            .actor(a.actor)
                            .fx("shifei"),
                    );
                    g.push_log(format!(
                        "饰非：弃置 {} 的【{label}】，视为使用/打出【闪】",
                        g.seats[i].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("饰非：未知阶段".into()),
    }
}
