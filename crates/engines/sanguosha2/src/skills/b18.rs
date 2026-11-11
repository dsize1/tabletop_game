//! B18（界限突破·J6）——按 `align/B18.json` 第 7 段实现。
//!
//! 覆盖：界王异（贞烈/秘计）、界张春华（绝情/伤逝/翦灭）、界刘表（自守/宗室）、
//! 界蔡夫人（献州/窃听）、界曹冲（称象/仁心）、界曹植（落英/酒诗）、
//! 界吴国太（甘露/补益）、界廖化（当先/伏枥）。
//!
//! 用户裁定（2026-11-10，75 条疑点）要点：
//! - 贞烈：失去体力走 lose_hp；获得使用者一张牌（手牌随机/装备区，不含判定区）；秘计额外一次（多次②不叠加）。
//! - 秘计：仅一名目标；**未选贞烈②则不能发动秘计**（用户口径，与官方独立触发不同）。
//! - 绝情：伤害→失去体力（整次、走 lose_hp；自伤同样转化）。
//! - 伤逝：手牌 < 已损失后摸至该值；弃牌阶段内不补、改为弃牌阶段结束判定。
//! - 翦灭：同时选色（顺序询问）；弃多者视为使用【决斗】；目标可无手牌。
//! - 自守：摸牌多摸 X；结束阶段若使用过伤害牌弃 X；界版不防止伤害。
//! - 宗室：每回合每势力首次挡伤害；来源选装备区具体牌/随机手牌（不含判定区）。
//! - 献州：伤害目标由蔡夫人选（接收者攻击范围内至多 X 名）。
//! - 窃听：未造成伤害选一项；未使用过牌选两项；装备可替换。
//! - 称象：每点伤害触发；bonus 可累积；亮牌≤13 任取。
//! - 仁心：界版（濒死时弃装备翻面 → 回复至 1）。
//! - 落英：逐张询问；含判定牌；走统一获得入口；记录回合外累计（供酒诗）。
//! - 酒诗：每回合一次；落英牌带标记（背面朝上时无距离/不可响应）；受伤/累计落英可翻回。
//! - 甘露：X=差额；差额>已损失时弃 X 张手牌（仅手牌、不足弃光仍交换）；可选自己；触发装备钩子。
//! - 补益：询问；手牌/装备区；每次濒死一次。
//! - 当先：准备阶段后、判定阶段前执行额外出牌阶段；获得一张无距离【杀】；未造成伤害自伤 1。
//! - 伏枥：回复至 X 且摸至 X；X > 本局造成伤害数才翻面。

use serde_json::json;

use crate::cards::{CardKind, Effect};
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};

use super::{active, builder, other_alive, SkillDef, SkillKind};

const SLOTS: [Slot; 5] = [
    Slot::Weapon,
    Slot::Armor,
    Slot::HorseOffense,
    Slot::HorseDefense,
    Slot::Treasure,
];

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, Stage::Choose, actor, data);
}
fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}
fn alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect()
}
fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}
fn is_sha(g: &Game, id: &CardId) -> bool {
    g.cards.contains_key(id) && g.def_of(id).tags.sha
}
fn lost_hp(g: &Game, seat: usize) -> usize {
    (g.seats[seat].max_hp - g.seats[seat].hp).max(0) as usize
}

// ---------------------------------------------------------------------------
// 贞烈（界王异）
// ---------------------------------------------------------------------------

pub const ZHENLIE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhenlie",
    name: "贞烈",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhenlie,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhenlie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let key = a.data["key"].as_str().unwrap_or_default().to_string();

    if stage.is_empty() {
        if !active(g, owner, "zhenlie") || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.zhenlie_skip:{key}")) > 0
            || g.counter(&format!("turn.zhenlie_void:{key}")) > 0
        {
            return Ok(Step::Done);
        }
        let is_trick = a.data["trick_intercept"].as_bool().unwrap_or(false);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zhenlie");
                b = b.option("opt1", "贞烈：失去 1 点体力，令此牌对你无效");
                Ok(Step::Need(b.escape("不发动贞烈").build()))
            }
            Some("pass") => {
                g.bump(&format!("turn.zhenlie_skip:{key}"));
                if is_trick {
                    g.trick_intercept = Some((owner, false));
                } else {
                    g.sha_intercept = Some(ShaIntercept::Continue);
                }
                return Ok(Step::Done);
            }
            Some(_) => {
                flow::lose_hp_by(g, owner, 1, "zhenlie", Some(user));
                g.bump(&format!("turn.zhenlie_void:{key}"));
                g.push_log(format!(
                    "贞烈：{} 失去 1 点体力，令此牌对其无效",
                    g.seats[owner].name
                ));
                if is_trick {
                    g.trick_intercept = Some((owner, true));
                } else {
                    g.sha_intercept = Some(ShaIntercept::Immune(owner));
                }
                // 界版：无效后二选一（获得使用者一张牌 / 本回合结束阶段额外秘计）。
                push(
                    g,
                    "zhenlie",
                    owner,
                    json!({ "stage": "jie", "user": user, "key": key }),
                );
                return Ok(Step::Done);
            }
        }
    } else if stage == "jie" {
        let has_card = !g.hand(user).is_empty() || !equip_ids(g, user).is_empty();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zhenlie");
                if has_card {
                    b = b.option("gain", "贞烈：获得使用者一张牌");
                }
                b = b.option("miji", "贞烈：本回合结束阶段发动一次「秘计」");
                Ok(Step::Need(b.escape("不选择").build()))
            }
            Some("gain") => {
                if !has_card {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "gain_pick", "user": user }),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.counters.insert("turn.miji_granted".to_string(), 1);
                g.push_log(format!(
                    "贞烈：{} 本回合结束阶段可发动一次「秘计」",
                    g.seats[owner].name
                ));
                Ok(Step::Done)
            }
        }
    } else {
        // gain_pick：获得使用者一张牌（手牌随机 / 装备区；不含判定区）。
        let mut opts: Vec<(String, String)> = equip_ids(g, user)
            .into_iter()
            .map(|id| (format!("pick:equip:{id}"), format!("装备区：{}", label(g, &id))))
            .collect();
        if !g.hand(user).is_empty() {
            opts.push((
                "pick:hand".to_string(),
                format!("手牌：随机 1 张（共 {} 张）", g.hand(user).len()),
            ));
        }
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zhenlie");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("贞烈：获得{text}"));
                }
                Ok(Step::Need(b.escape("不获得").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let card = if act == "pick:hand" {
                    let n = g.hand(user).len();
                    if n == 0 {
                        return Ok(Step::Done);
                    }
                    let idx = g.rand_below(n);
                    g.hand(user)[idx].clone()
                } else {
                    act.strip_prefix("pick:equip:")
                        .ok_or_else(|| format!("贞烈：未知选项 {act}"))?
                        .to_string()
                };
                if !g.cards.contains_key(&card) {
                    return Ok(Step::Done);
                }
                g.gain(&card, owner)?;
                g.push_log(format!(
                    "贞烈：{} 获得 {} 的【{}】",
                    g.seats[owner].name,
                    g.seats[user].name,
                    label(g, &card)
                ));
                Ok(Step::Done)
            }
        }
    }
}

/// 贞烈（杀）：成为【杀】目标后。
pub fn zhenlie_sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let Some(card) = card else { return out };
    if target == user || g.seats[target].hp <= 0 || !is_sha(g, card) {
        return out;
    }
    if !active(g, target, "zhenlie") {
        return out;
    }
    out.push((target, "zhenlie"));
    out
}

/// 贞烈（普通锦囊）：成为目标后。
pub fn trick_intercept_owner(
    g: &Game,
    user: usize,
    name: &str,
    target: usize,
    _carrier: Option<&CardId>,
) -> Option<(usize, &'static str)> {
    let def = crate::cards::def(name);
    if !(def.kind == CardKind::Trick && !def.tags.delayed && !def.tags.wuxie) {
        return None;
    }
    if target == user || g.seats[target].hp <= 0 || !active(g, target, "zhenlie") {
        return None;
    }
    Some((target, "zhenlie"))
}

// ---------------------------------------------------------------------------
// 秘计（界王异）：结束阶段（需贞烈②授权）摸 X，交给其他角色至多 X 张
// ---------------------------------------------------------------------------

pub const MIJI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "miji",
    name: "秘计",
    triggers: &[EventKind::PhaseStart],
    // 贞烈②的「本回合结束阶段」可以是**其他角色**的回合（王异通常在他人回合被指定）。
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: miji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn miji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        let turn_seat = a.data["seat"].as_u64().map(|n| n as usize);
        if !a.data["event"].as_str().unwrap_or("").starts_with("PhaseStart")
            || g.phase != Phase::End
            || !active(g, owner, "miji")
            || g.counter(&format!("turn.miji_offered:{owner}")) > 0
        {
            return Ok(Step::Done);
        }
        // 自己的结束阶段恒可发动一次；其他角色的结束阶段仅在「贞烈②」授权时发动。
        // 同一结束阶段只发动一次（own turn 时即使也有②，也不再额外一次）。
        if turn_seat != Some(owner) && g.counter("turn.miji_granted") == 0 {
            return Ok(Step::Done);
        }
        let x = lost_hp(g, owner);
        if x == 0 {
            return Ok(Step::Done);
        }
        g.bump(&format!("turn.miji_offered:{owner}"));
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "offer", "x": x }),
        ));
    }
    if stage == "offer" {
        let x = a.data["x"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("miji");
                b = b.option("opt1", format!("秘计：摸 {x} 张牌"));
                Ok(Step::Need(b.escape("不发动秘计").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let n = g.draw_with_cause(owner, x, Some("miji")).len();
                g.push_log(format!(
                    "秘计：{} 摸 {n} 张（已损失 {x}）",
                    g.seats[owner].name
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "give", "sel": [], "x": x }),
                ))
            }
        }
    } else if stage == "give" {
        let x = a.data["x"].as_u64().unwrap_or(0) as usize;
        let mut sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| v.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("miji");
                for id in g.hand(owner) {
                    let mark = if sel.contains(&id) { "✔ " } else { "" };
                    b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, &id)));
                }
                if !sel.is_empty() {
                    b = b.option("confirm", format!("确认交给（{} 张）", sel.len()));
                }
                Ok(Step::Need(b.escape("不交给其他角色").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("confirm") => {
                if sel.is_empty() || sel.len() > x {
                    return Err("秘计：交牌数不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "target", "sel": sel }),
                ))
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("秘计：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&id) {
                    return Err("秘计：该牌不在手牌".into());
                }
                if let Some(pos) = sel.iter().position(|c| *c == id) {
                    sel.remove(pos);
                } else if sel.len() < x {
                    sel.push(id);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "give", "sel": sel, "x": x }),
                ))
            }
        }
    } else {
        let sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| v.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let cands = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("miji");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("交给 {}", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消（不交给其他角色）").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("秘计：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("秘计：目标不合法".into());
                }
                let mut n = 0;
                for id in &sel {
                    if g.hand(owner).contains(id) {
                        g.gain(id, t)?;
                        n += 1;
                    }
                }
                g.push_log(format!(
                    "秘计：{} 交给 {} {n} 张牌",
                    g.seats[owner].name,
                    g.seats[t].name
                ));
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 绝情（界张春华，锁定）
// ---------------------------------------------------------------------------

pub const JUEQING: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "jueqing",
    name: "绝情",
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

pub fn damage_to_hp_loss(g: &Game, source: Option<usize>, victim: usize) -> bool {
    let _ = victim;
    source.is_some_and(|s| active(g, s, "jueqing"))
}

pub fn on_damage_to_hp_loss(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    _card: Option<&CardId>,
) {
    let name = source.map(|s| g.seats[s].name.clone()).unwrap_or_default();
    g.push_log(format!(
        "绝情：{name} 对 {} 的 {amount} 点伤害视为失去体力",
        g.seats[victim].name
    ));
}

// ---------------------------------------------------------------------------
// 伤逝（界张春华）
// ---------------------------------------------------------------------------

pub const SHANGSHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "shangshi",
    name: "伤逝",
    triggers: &[EventKind::CardLost, EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: shangshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn shangshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "shangshi") || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    let ev = a.data["event"].as_str().unwrap_or("");
    if ev.starts_with("PhaseStart") {
        // 弃牌阶段结束（进入结束阶段）时判定一次。
        if g.phase != Phase::End || a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
    } else {
        // 失去手牌后（弃牌阶段内不即时补，防循环）。
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner)
            || g.phase == Phase::Discard
        {
            return Ok(Step::Done);
        }
    }
    let lost = lost_hp(g, owner);
    let hand = g.hand(owner).len();
    if lost <= hand {
        return Ok(Step::Done);
    }
    let n = g
        .draw_with_cause(owner, lost - hand, Some("shangshi"))
        .len();
    if n > 0 {
        g.push_log(format!(
            "伤逝：{} 摸至 {lost} 张（摸 {n}）",
            g.seats[owner].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 翦灭（界张春华）
// ---------------------------------------------------------------------------

pub const JIANMIE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jianmie",
    name: "翦灭",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jianmie),
    handler: jianmie,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_jianmie(g: &Game, seat: usize) -> bool {
    active(g, seat, "jianmie")
        && play_phase(g, seat)
        && g.counter("phase.jianmie_used") == 0
        && !other_alive(g, seat).is_empty()
}

fn jianmie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        let cands = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("jianmie");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("翦灭：与 {} 同时选颜色", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动翦灭").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("翦灭：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("翦灭：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "color", "owner": owner, "target": t }),
                ))
            }
        }
    } else if stage == "color" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jianmie");
                b = b.option("choose:red", "翦灭：选择红色");
                b = b.option("choose:black", "翦灭：选择黑色");
                Ok(Step::Need(b.escape("取消翦灭").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let red = match act {
                    "choose:red" => true,
                    "choose:black" => false,
                    _ => return Err("翦灭：需要选择颜色".into()),
                };
                push(
                    g,
                    "jianmie",
                    target,
                    json!({ "stage": "target_color", "owner": owner, "target": target, "owner_red": red }),
                );
                Ok(Step::Done)
            }
        }
    } else {
        // target_color：目标选颜色（选择期互不可见），结算。
        let target = a.actor;
        let owner_red = a.data["owner_red"].as_bool().unwrap_or(true);
        let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(target, "choice").skill("jianmie");
                b = b.option("choose:red", "翦灭：选择红色");
                b = b.option("choose:black", "翦灭：选择黑色");
                Ok(Step::Need(b.escape("取消（视为不弃牌）").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let target_red = match act {
                    "choose:red" => true,
                    "choose:black" => false,
                    _ => return Err("翦灭：需要选择颜色".into()),
                };
                let owner_n = discard_color(g, owner, owner_red)?;
                let target_n = discard_color(g, target, target_red)?;
                g.bump("phase.jianmie_used");
                g.push_log(format!(
                    "翦灭：{} 弃 {owner_n} 张，{} 弃 {target_n} 张",
                    g.seats[owner].name, g.seats[target].name
                ));
                let (user, victim) = if owner_n > target_n {
                    (owner, target)
                } else if target_n > owner_n {
                    (target, owner)
                } else {
                    g.push_log("翦灭：平手，无【决斗】");
                    return Ok(Step::Done);
                };
                g.push_log(format!(
                    "翦灭：{} 视为对 {} 使用【决斗】",
                    g.seats[user].name, g.seats[victim].name
                ));
                g.use_virtual(user, "决斗", vec![victim])?;
                Ok(Step::Done)
            }
        }
    }
}

fn discard_color(g: &mut Game, seat: usize, red: bool) -> Result<u32, String> {
    let hand = g.hand(seat);
    let mut n = 0;
    for id in hand {
        let is_red = g.cards.get(&id).map(|c| c.suit.is_red()).unwrap_or(false);
        if is_red == red {
            g.to_discard(&id, "jianmie")?;
            n += 1;
        }
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// 自守（界刘表）
// ---------------------------------------------------------------------------

pub const ZISHOU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zishou",
    name: "自守",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: zishou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 摸牌阶段开始前询问（`skills::before_phase`）。
pub fn before_phase(g: &mut Game, seat: usize, phase: Phase) -> bool {
    if phase == Phase::Draw
        && active(g, seat, "zishou")
        && g.counter(&format!("turn.zishou_offered:{seat}")) == 0
        && g.counter("turn.skip_draw") == 0
    {
        g.bump(&format!("turn.zishou_offered:{seat}"));
        push(g, "zishou", seat, json!({ "stage": "offer" }));
        return true;
    }
    false
}

pub fn draw_bonus(g: &Game, seat: usize) -> usize {
    if active(g, seat, "zishou") && g.counter(&format!("turn.zishou_used:{seat}")) > 0 {
        super::kingdom_count(g)
    } else {
        0
    }
}

fn zishou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "offer" {
        let x = super::kingdom_count(g);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zishou");
                b = b.option("opt1", format!("自守：摸牌阶段多摸 {x} 张"));
                Ok(Step::Need(b.escape("不发动自守").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.bump(&format!("turn.zishou_used:{owner}"));
                g.push_log(format!(
                    "自守：{} 摸牌阶段多摸 {x} 张",
                    g.seats[owner].name
                ));
                Ok(Step::Done)
            }
        }
    } else if a.data["event"].as_str().unwrap_or("").starts_with("PhaseStart") {
        // 结束阶段：若本回合使用过伤害牌 → 弃 X 张。
        if g.phase != Phase::End
            || a.data["seat"].as_u64().map(|n| n as usize) != Some(owner)
            || !active(g, owner, "zishou")
            || g.counter(&format!("turn.zishou_used:{owner}")) == 0
            || g.counter(&format!("turn.damage_card_used:{owner}")) == 0
            || g.counter(&format!("turn.zishou_paid:{owner}")) > 0
        {
            return Ok(Step::Done);
        }
        g.bump(&format!("turn.zishou_paid:{owner}"));
        let want = super::kingdom_count(g);
        if want == 0 || g.hand(owner).is_empty() {
            return Ok(Step::Done);
        }
        // 不足则弃至手牌为空（自动）。
        if want >= g.hand(owner).len() {
            let ids = g.hand(owner);
            for id in ids {
                g.to_discard(&id, "zishou")?;
            }
            g.push_log(format!(
                "自守：{} 本回合使用过伤害牌，弃置 {} 张牌（不足 X={want}）",
                g.seats[owner].name,
                g.hand(owner).len()
            ));
            return Ok(Step::Done);
        }
        g.push_log(format!(
            "自守：{} 本回合使用过伤害牌，弃置 {want} 张牌",
            g.seats[owner].name
        ));
        Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pay", "left": want, "sel": [] }),
        ))
    } else {
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        let mut sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| v.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        if sel.len() >= left {
            for id in &sel {
                if g.hand(owner).contains(id) {
                    g.to_discard(id, "zishou")?;
                }
            }
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            Some("confirm") => {
                if sel.len() != left {
                    return Err(format!("自守：还需弃置 {} 张", left - sel.len()));
                }
                for id in &sel {
                    if g.hand(owner).contains(id) {
                        g.to_discard(id, "zishou")?;
                    }
                }
                Ok(Step::Done)
            }
            None => {
                let mut b = builder(owner, "choice").skill("zishou");
                for id in g.hand(owner) {
                    let mark = if sel.contains(&id) { "✔ " } else { "" };
                    b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, &id)));
                }
                if !sel.is_empty() {
                    b = b.option("confirm", format!("确认弃置（{}）", sel.len()));
                }
                Ok(Step::Need(b.escape("（自动弃置剩余）").build()))
            }
            Some("pass") => {
                for id in &sel {
                    if g.hand(owner).contains(id) {
                        g.to_discard(id, "zishou")?;
                    }
                }
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("自守：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&id) {
                    return Err("自守：该牌不在手牌".into());
                }
                if let Some(pos) = sel.iter().position(|c| *c == id) {
                    sel.remove(pos);
                } else if sel.len() < left {
                    sel.push(id);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pay", "left": left, "sel": sel }),
                ))
            }
        }
    }
}

/// 自守：统计本回合使用过的伤害牌（绝情转化也算使用过）。
pub fn on_card_use_for_zishou(g: &mut Game, seat: usize, name: &str) {
    if !active(g, seat, "zishou") {
        return;
    }
    let def = crate::cards::def(name);
    if def.tags.sha || def.damage_trick {
        g.bump(&format!("turn.damage_card_used:{seat}"));
    }
}

// ---------------------------------------------------------------------------
// 宗室（界刘表，锁定）
// ---------------------------------------------------------------------------

pub const ZONGSHI: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "zongshi",
    name: "宗室",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zongshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 手牌上限：全场每有一个势力 +1。
pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "zongshi") {
        super::kingdom_count(g) as i32
    } else {
        0
    }
}

/// 造成伤害时（受害者侧）：防止伤害，令来源获得刘表一张区域牌（每回合每势力一次）。
pub fn zongshi_guard(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
) -> bool {
    let Some(src) = source else { return false };
    if amount <= 0 || src == victim || g.seats[victim].hp <= 0 {
        return false;
    }
    if !active(g, victim, "zongshi") {
        return false;
    }
    let kingdom = g.seats[src].kingdom.clone();
    if kingdom.is_empty() || g.counter(&format!("turn.zongshi_guard:{kingdom}")) > 0 {
        return false;
    }
    g.bump(&format!("turn.zongshi_guard:{kingdom}"));
    g.push_log(format!(
        "宗室：防止 {} 对 {} 的伤害，其获得一张区域牌",
        g.seats[src].name, g.seats[victim].name
    ));
    push(
        g,
        "zongshi",
        src,
        json!({ "stage": "pick", "victim": victim }),
    );
    true
}

fn zongshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let src = a.actor;
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    if g.seats[victim].hp <= 0 {
        return Ok(Step::Done);
    }
    let mut opts: Vec<(String, String)> = equip_ids(g, victim)
        .into_iter()
        .map(|id| (format!("pick:equip:{id}"), format!("装备区：{}", label(g, &id))))
        .collect();
    if !g.hand(victim).is_empty() {
        opts.push((
            "pick:hand".to_string(),
            format!("手牌：随机 1 张（共 {} 张）", g.hand(victim).len()),
        ));
    }
    if opts.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(src, "choice").skill("zongshi");
            for (id, text) in &opts {
                b = b.option(id.clone(), format!("宗室：获得{}", text));
            }
            Ok(Step::Need(b.escape("（无牌可获得）").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let card = if act == "pick:hand" {
                let n = g.hand(victim).len();
                if n == 0 {
                    return Ok(Step::Done);
                }
                let idx = g.rand_below(n);
                g.hand(victim)[idx].clone()
            } else {
                act.strip_prefix("pick:equip:")
                    .ok_or_else(|| format!("宗室：未知选项 {act}"))?
                    .to_string()
            };
            if !g.cards.contains_key(&card) {
                return Ok(Step::Done);
            }
            g.gain(&card, src)?;
            g.push_log(format!(
                "宗室：{} 获得 {} 的【{}】",
                g.seats[src].name,
                g.seats[victim].name,
                label(g, &card)
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 献州（界蔡夫人，限定）
// ---------------------------------------------------------------------------

pub const XIANZHOU: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "xianzhou",
    name: "献州",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_xianzhou),
    handler: xianzhou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_xianzhou(g: &Game, seat: usize) -> bool {
    active(g, seat, "xianzhou")
        && play_phase(g, seat)
        && !g.seats[seat].flags.contains("xianzhou_used")
        && !equip_ids(g, seat).is_empty()
        && !other_alive(g, seat).is_empty()
}

fn xianzhou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        let cands = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("xianzhou");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("献州：将所有装备牌交给 {}", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动献州").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("献州：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("献州：目标不合法".into());
                }
                g.seats[owner].flags.insert("xianzhou_used".into());
                let cards = equip_ids(g, owner);
                if cards.is_empty() {
                    return Err("献州：装备区没有牌".into());
                }
                let x = cards.len();
                for id in cards {
                    g.gain(&id, t)?;
                }
                g.push_log(format!(
                    "献州：{} 将 {x} 张装备牌交给 {}",
                    g.seats[owner].name,
                    g.seats[t].name
                ));
                push(
                    g,
                    "xianzhou",
                    t,
                    json!({ "stage": "choice", "owner": owner, "receiver": t, "x": x }),
                );
                Ok(Step::Done)
            }
        }
    } else if stage == "choice" {
        let receiver = a.actor;
        let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
        let x = a.data["x"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(receiver, "choice").skill("xianzhou");
                b = b.option("opt1", format!("令 {} 回复 {x} 点体力", g.seats[owner].name));
                b = b.option(
                    "opt2",
                    format!("对其攻击范围内至多 {x} 名角色各造成 1 点伤害"),
                );
                Ok(Step::Need(b.escape_mandatory("（不能放弃）不选择").build()))
            }
            Some("opt1") => {
                flow::heal(g, owner, x as i32, "xianzhou");
                g.push_log(format!(
                    "献州：{} 回复 {x} 点体力",
                    g.seats[owner].name
                ));
                Ok(Step::Done)
            }
            Some(_) => {
                // 由蔡夫人选择接收者攻击范围内至多 X 名角色（不含接收者）。
                push(
                    g,
                    "xianzhou",
                    owner,
                    json!({ "stage": "targets", "owner": owner, "receiver": receiver, "left": x, "sel": [] }),
                );
                Ok(Step::Done)
            }
        }
    } else {
        let receiver = a.data["receiver"].as_u64().unwrap_or(0) as usize;
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        let mut sel: Vec<usize> = a.data["sel"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        if sel.len() >= left {
            for t in &sel {
                flow::damage(g, Some(receiver), *t, 1);
            }
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = alive(g)
            .into_iter()
            .filter(|t| {
                *t != receiver
                    && !sel.contains(t)
                    && g.in_attack_range(receiver, *t)
            })
            .collect();
        if cands.is_empty() || left == 0 {
            for t in &sel {
                flow::damage(g, Some(receiver), *t, 1);
            }
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("xianzhou");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("对 {} 造成 1 点伤害", g.seats[*t].name),
                    );
                }
                if !sel.is_empty() {
                    b = b.option("confirm", "确认（开始结算）");
                }
                Ok(Step::Need(b.escape("停止选择并结算").build()))
            }
            Some("pass") | Some("confirm") => {
                for t in &sel {
                    flow::damage(g, Some(receiver), *t, 1);
                }
                Ok(Step::Done)
            }
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("献州：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("献州：目标不合法".into());
                }
                sel.push(t);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "targets", "owner": owner, "receiver": receiver, "left": left, "sel": sel }),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 窃听（界蔡夫人）
// ---------------------------------------------------------------------------

pub const QIETING: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qieting",
    name: "窃听",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qieting,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 回合结束（在清空 `turn.` 计数之前调用）。
pub fn on_turn_end(g: &mut Game, ended: usize) {
    if g.seats.get(ended).is_none() {
        return;
    }
    let owners: Vec<usize> = (0..g.seats.len())
        .filter(|i| *i != ended && g.seats[*i].hp > 0 && active(g, *i, "qieting"))
        .collect();
    if owners.is_empty() {
        return;
    }
    let no_damage = g.counter(&format!("turn.damage_by:{ended}")) == 0;
    let no_cards = g.counter(&format!("turn.targeted_others:{ended}")) == 0;
    if !no_damage && !no_cards {
        return;
    }
    for owner in owners {
        push(
            g,
            "qieting",
            owner,
            json!({ "stage": "ask", "current": ended, "no_damage": no_damage, "no_cards": no_cards }),
        );
    }
}

fn qieting(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "qieting") {
        return Ok(Step::Done);
    }
    let current = a.data["current"].as_u64().unwrap_or(0) as usize;
    let no_damage = a.data["no_damage"].as_bool().unwrap_or(false);
    let no_cards = a.data["no_cards"].as_bool().unwrap_or(false);
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "ask" {
        let mut opts: Vec<(String, String)> = Vec::new();
        for slot in SLOTS {
            if let Some(id) = g.equip_in(current, slot) {
                opts.push((
                    format!("steal:{id}"),
                    format!("获得其{}区的【{}】", slot.label(), label(g, &id)),
                ));
            }
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qieting");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("窃听：{text}"));
                }
                b = b.option("draw", "窃听：摸一张牌");
                if no_cards {
                    b = b.option("both", "窃听：获得装备并摸一张（两项）");
                }
                Ok(Step::Need(b.escape("不发动窃听").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("draw") => {
                let n = g.draw_with_cause(owner, 1, Some("qieting")).len();
                g.push_log(format!("窃听：{} 摸 {n} 张", g.seats[owner].name));
                Ok(Step::Done)
            }
            Some("both") => {
                if !no_cards {
                    return Err("窃听：本回合不满足选择两项的条件".into());
                }
                let n = g.draw_with_cause(owner, 1, Some("qieting")).len();
                g.push_log(format!("窃听：{} 摸 {n} 张", g.seats[owner].name));
                if opts.is_empty() {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "steal", "current": current, "opts": opts.iter().map(|(i,_)| i).collect::<Vec<_>>() }),
                ))
            }
            Some(act) => {
                if !act.starts_with("steal:") {
                    return Err(format!("窃听：未知选项 {act}"));
                }
                steal_equip(g, owner, act)?;
                if no_cards {
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "draw_extra", "current": current }),
                    ))
                } else {
                    Ok(Step::Done)
                }
            }
        }
    } else if stage == "steal" {
        let opts: Vec<String> = a.data["opts"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qieting");
                for id in &opts {
                    b = b.option(id.clone(), format!("窃听：获得【{}】", id));
                }
                Ok(Step::Need(b.escape("放弃获得装备").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                if !opts.contains(&act.to_string()) {
                    return Err("窃听：该装备不在候选".into());
                }
                steal_equip(g, owner, act)?;
                Ok(Step::Done)
            }
        }
    } else {
        let n = g.draw_with_cause(owner, 1, Some("qieting")).len();
        g.push_log(format!("窃听：{} 摸 {n} 张", g.seats[owner].name));
        Ok(Step::Done)
    }
}

fn steal_equip(g: &mut Game, owner: usize, act: &str) -> Result<(), String> {
    let card = act
        .strip_prefix("steal:")
        .ok_or_else(|| format!("窃听：未知选项 {act}"))?
        .to_string();
    let Some(zone) = g.zone_of(&card).cloned() else {
        return Err("窃听：该牌不在场上".into());
    };
    let Zone::Equip(from, slot) = zone else {
        return Err("窃听：该牌不在装备区".into());
    };
    if from == owner {
        return Err("窃听：不能获得自己的装备".into());
    }
    if g.is_slot_abolished(owner, slot) {
        return Err("窃听：你的该装备栏已废除".into());
    }
    if let Some(old) = g.equip_in(owner, slot) {
        g.to_discard(&old, "qieting")?;
        super::on_equip_lost(g, owner);
    }
    // 移动语义：离开原持有者装备区（触发失去装备钩子）。
    g.to_processing(&card)?;
    g.to_equip(&card, owner, slot)?;
    super::on_equip_lost(g, from);
    g.push_log(format!(
        "窃听：{} 获得 {} 的【{}】",
        g.seats[owner].name,
        g.seats[from].name,
        label(g, &card)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 称象（界曹冲）
// ---------------------------------------------------------------------------

pub const CHENGXIANG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "chengxiang",
    name: "称象",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: chengxiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn chengxiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("DamageAfter")
            || !active(g, owner, "chengxiang")
            || g.seats[owner].hp <= 0
        {
            return Ok(Step::Done);
        }
        let amount = a.data["amount"].as_i64().unwrap_or(1).max(1) as u32;
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "left": amount }),
        ));
    }
    if stage == "ask" {
        let left = a.data["left"].as_u64().unwrap_or(0) as u32;
        if left == 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("chengxiang");
                b = b.option("opt1", "称象：亮出牌堆顶牌并获得点数和 ≤13 的牌");
                Ok(Step::Need(b.escape("不发动称象").build()))
            }
            Some("pass") => {
                if left > 1 {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "ask", "left": left - 1 }),
                    ));
                }
                Ok(Step::Done)
            }
            Some(_) => {
                let bonus = g
                    .seats[owner]
                    .counters
                    .get("chengxiang_bonus")
                    .copied()
                    .unwrap_or(0) as usize;
                g.seats[owner].counters.remove("chengxiang_bonus");
                let n = 4 + bonus;
                let mut cards: Vec<CardId> = Vec::new();
                for _ in 0..n {
                    let Some(id) = g.draw_top() else { break };
                    if g.to_processing(&id).is_err() {
                        break;
                    }
                    g.reveal(&id, Some(owner), "chengxiang");
                    cards.push(id);
                }
                if cards.is_empty() {
                    return Ok(Step::Done);
                }
                g.push_log(format!(
                    "称象：亮出牌堆顶 {} 张（{}）",
                    cards.len(),
                    cards
                        .iter()
                        .map(|id| format!("{}({})", label(g, id), g.cards.get(id).map(|c| c.rank).unwrap_or(0)))
                        .collect::<Vec<_>>()
                        .join("、")
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "left": left, "cards": cards, "sel": [] }),
                ))
            }
        }
    } else {
        let left = a.data["left"].as_u64().unwrap_or(1) as u32;
        let cards: Vec<String> = a.data["cards"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let mut sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let sum = |s: &[String], g: &Game| -> u32 {
            s.iter()
                .filter_map(|id| g.cards.get(id).map(|c| c.rank as u32))
                .sum()
        };
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("chengxiang");
                let cur = sum(&sel, g);
                for id in &cards {
                    let rank = g.cards.get(id).map(|c| c.rank as u32).unwrap_or(0);
                    let picked = sel.contains(id);
                    if picked {
                        b = b.option(format!("choose:{id}"), format!("✔ {}({rank}) 和 {cur}", label(g, id)));
                    } else if cur + rank <= 13 {
                        b = b.option(format!("choose:{id}"), format!("{}({rank})", label(g, id)));
                    }
                }
                if !sel.is_empty() {
                    b = b.option("confirm", format!("确认获得（和 {cur}）"));
                }
                Ok(Step::Need(b.escape("不获得任何牌").build()))
            }
            Some("pass") => {
                finish_chengxiang(g, owner, &cards, &sel, left, false)
            }
            Some("confirm") => {
                let sum13 = sum(&sel, g) == 13;
                finish_chengxiang(g, owner, &cards, &sel, left, sum13)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("称象：未知选项 {act}"))?
                    .to_string();
                if !cards.contains(&id) {
                    return Err("称象：该牌不在亮出的牌中".into());
                }
                if let Some(pos) = sel.iter().position(|c| *c == id) {
                    sel.remove(pos);
                } else {
                    let rank = g.cards.get(&id).map(|c| c.rank as u32).unwrap_or(0);
                    if sum(&sel, g) + rank > 13 {
                        return Err("称象：点数和不能超过 13".into());
                    }
                    sel.push(id);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "left": left, "cards": cards, "sel": sel }),
                ))
            }
        }
    }
}

fn finish_chengxiang(
    g: &mut Game,
    owner: usize,
    cards: &[String],
    sel: &[String],
    left: u32,
    sum13: bool,
) -> Result<Step, String> {
    let mut got = 0;
    for id in cards {
        if sel.contains(id) {
            if matches!(g.zone_of(id), Some(Zone::Processing)) {
                g.gain(id, owner)?;
                got += 1;
            }
            g.conceal(id, "chengxiang");
        } else if matches!(g.zone_of(id), Some(Zone::Processing)) {
            g.conceal(id, "chengxiang");
            g.to_discard(id, "chengxiang")?;
        }
    }
    if got > 0 {
        g.push_log(format!(
            "称象：{} 获得 {got} 张牌",
            g.seats[owner].name
        ));
    }
    if sum13 {
        let cur = g
            .seats[owner]
            .counters
            .get("chengxiang_bonus")
            .copied()
            .unwrap_or(0);
        g.seats[owner]
            .counters
            .insert("chengxiang_bonus".to_string(), cur + 1);
        g.push_log(format!(
            "称象：点数和恰为 13，下次多亮一张（累计 {}）",
            cur + 1
        ));
    }
    if left > 1 {
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "left": left - 1 }),
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 仁心（界曹冲）：其他角色濒死时，弃装备翻面 → 回复至 1
// ---------------------------------------------------------------------------

pub const RENXIN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "renxin",
    name: "仁心",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: renxin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn renxin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "pick" {
        let pool: Vec<CardId> = {
            let mut v: Vec<CardId> = g
                .hand(owner)
                .into_iter()
                .filter(|id| g.def_of(id).slot.is_some())
                .collect();
            v.extend(equip_ids(g, owner));
            v
        };
        if pool.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("renxin");
                for id in &pool {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不发动仁心").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("仁心：未知选项 {act}"))?
                    .to_string();
                if !pool.contains(&id) {
                    return Err("仁心：该牌不是装备牌".into());
                }
                g.to_discard(&id, "renxin")?;
                g.flip(owner);
                flow::revive_to(g, victim, 1, "renxin");
                g.push_log(format!(
                    "仁心：{} 弃置【{}】并翻面，令 {} 回复至 1 点体力",
                    g.seats[owner].name,
                    label(g, &id),
                    g.seats[victim].name
                ));
                Ok(Step::Done)
            }
        }
    } else {
        // ask：是否发动（可有多个仁心持有者，逐个询问）。
        let pool: Vec<CardId> = {
            let mut v: Vec<CardId> = g
                .hand(owner)
                .into_iter()
                .filter(|id| g.def_of(id).slot.is_some())
                .collect();
            v.extend(equip_ids(g, owner));
            v
        };
        if pool.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("renxin");
                b = b.option("opt1", format!("仁心：弃一张装备牌并翻面，令 {} 回复至 1 点体力", g.seats[victim].name));
                Ok(Step::Need(b.escape("不发动仁心").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick", "victim": victim }),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// 落英（界曹植）
// ---------------------------------------------------------------------------

pub const LUOYING: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "luoying",
    name: "落英",
    triggers: &[EventKind::CardDiscarded],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: luoying,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn luoying(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "luoying") || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let from = a.data["seat"].as_u64().map(|n| n as usize);
    if from == Some(owner) {
        return Ok(Step::Done);
    }
    if !matches!(g.zone_of(&card), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    if !g
        .cards
        .get(&card)
        .map(|c| c.suit == crate::kernel::Suit::Club)
        .unwrap_or(false)
    {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("luoying");
            b = b.option(
                format!("gain:{card}"),
                format!("落英：获得【{}】", label(g, &card)),
            );
            Ok(Step::Need(b.escape("不获得").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            if act != format!("gain:{card}") {
                return Err(format!("落英：未知选项 {act}"));
            }
            if matches!(g.zone_of(&card), Some(Zone::Discard)) {
                g.gain(&card, owner)?;
                g.luoying_cards.insert(card.clone());
                if g.turn != owner {
                    g.bump(&format!("turn.luoying_gained:{owner}"));
                }
                g.push_log(format!(
                    "落英：{} 获得【{}】",
                    g.seats[owner].name,
                    label(g, &card)
                ));
                jiushi_maybe_flip(g, owner);
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 酒诗（界曹植）
// ---------------------------------------------------------------------------

pub const JIUSHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jiushi",
    name: "酒诗",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_jiushi),
    handler: jiushi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_jiushi(g: &Game, seat: usize) -> bool {
    active(g, seat, "jiushi")
        && play_phase(g, seat)
        && !g.seats[seat].flipped
        && g.counter(&format!("turn.jiu_used:{seat}")) == 0
        && !super::jiu_blocked(g, seat)
}

fn jiushi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty()
        && a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("DamageAfter")
    {
        // 受伤后询问翻回。
        if !active(g, owner, "jiushi") || !g.seats[owner].flipped || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        push(g, "jiushi", owner, json!({ "stage": "flipback" }));
        return Ok(Step::Done);
    }
    if stage.is_empty() {
        if !can_jiushi(g, owner) {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiushi");
                b = b.option("opt1", "酒诗：翻至背面，视为使用【酒】");
                Ok(Step::Need(b.escape("不发动酒诗").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.flip(owner);
                crate::kernel::card_play::apply_jiu(g, owner);
                g.push_log(format!(
                    "酒诗：{} 翻至背面，视为使用【酒】",
                    g.seats[owner].name
                ));
                Ok(Step::Done)
            }
        }
    } else if stage == "flipback" {
        if !active(g, owner, "jiushi") || !g.seats[owner].flipped || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiushi");
                b = b.option("opt1", "酒诗：翻至正面");
                Ok(Step::Need(b.escape("保持背面").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.flip(owner);
                g.counters
                    .insert(format!("turn.luoying_gained:{owner}"), 0);
                g.push_log(format!("酒诗：{} 翻至正面", g.seats[owner].name));
                Ok(Step::Done)
            }
        }
    } else {
        Ok(Step::Done)
    }
}

/// 酒诗：回合外累计获得 ≥X 张落英牌后（X=体力上限）询问翻回。
fn jiushi_maybe_flip(g: &mut Game, owner: usize) {
    if !active(g, owner, "jiushi") || !g.seats[owner].flipped || g.seats[owner].hp <= 0 {
        return;
    }
    let x = g.seats[owner].max_hp.max(1) as u32;
    if g.counter(&format!("turn.luoying_gained:{owner}")) >= x
        && g.counter(&format!("turn.jiushi_flip_asked:{owner}")) == 0
    {
        g.bump(&format!("turn.jiushi_flip_asked:{owner}"));
        push(g, "jiushi", owner, json!({ "stage": "flipback" }));
    }
}

// ---------------------------------------------------------------------------
// 甘露（界吴国太）
// ---------------------------------------------------------------------------

pub const GANLU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "ganlu",
    name: "甘露",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_ganlu),
    handler: ganlu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_ganlu(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "ganlu") || !play_phase(g, seat) || g.counter("phase.ganlu_used") > 0 {
        return false;
    }
    let with_equip = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0 && !equip_ids(g, *i).is_empty())
        .count();
    // 两名角色（可含自己）都需有装备牌。
    with_equip >= 2
}

fn ganlu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let first: Option<usize> = a.data["first"].as_u64().map(|n| n as usize);
    let cands: Vec<usize> = alive(g)
        .into_iter()
        .filter(|i| Some(*i) != first && !equip_ids(g, *i).is_empty())
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("ganlu");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!(
                        "甘露：{}（装备 {} 张）",
                        g.seats[*t].name,
                        equip_ids(g, *t).len()
                    ),
                );
            }
            Ok(Step::Need(b.escape("不发动甘露").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("甘露：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("甘露：目标不合法".into());
            }
            match first {
                None => Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "second", "first": t }),
                )),
                Some(f) => {
                    let na = equip_ids(g, f).len();
                    let nb = equip_ids(g, t).len();
                    let diff = na.abs_diff(nb);
                    let lost = lost_hp(g, owner);
                    let need = if diff > lost { diff } else { 0 };
                    if need > 0 {
                        // 弃置 X 张手牌（仅手牌；不足则弃光仍交换）。
                        let pay = need.min(g.hand(owner).len());
                        for _ in 0..pay {
                            let n = g.hand(owner).len();
                            if n == 0 {
                                break;
                            }
                            let idx = g.rand_below(n);
                            let id = g.hand(owner)[idx].clone();
                            g.to_discard(&id, "ganlu")?;
                        }
                        g.push_log(format!(
                            "甘露：差额 {diff} 超过已损失 {lost}，弃置 {pay} 张手牌后仍交换",
                        ));
                    }
                    g.bump("phase.ganlu_used");
                    swap_equips(g, f, t)?;
                    g.push_log(format!(
                        "甘露：交换 {} 与 {} 的装备区",
                        g.seats[f].name, g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
    }
}

fn swap_equips(g: &mut Game, a: usize, b: usize) -> Result<(), String> {
    let ea: Vec<(Slot, CardId)> = SLOTS
        .iter()
        .filter_map(|slot| g.equip_in(a, *slot).map(|id| (*slot, id)))
        .collect();
    let eb: Vec<(Slot, CardId)> = SLOTS
        .iter()
        .filter_map(|slot| g.equip_in(b, *slot).map(|id| (*slot, id)))
        .collect();
    // 全部先移到处理区（触发失去装备钩子），再放到对方同槽位。
    for (_, id) in ea.iter().chain(eb.iter()) {
        g.to_processing(id)?;
    }
    let mut move_in = |g: &mut Game, from: usize, to: usize, cards: &[(Slot, CardId)]| -> Result<(), String> {
        for (slot, id) in cards {
            if g.is_slot_abolished(to, *slot) {
                g.gain(id, to)?;
            } else {
                g.to_equip(id, to, *slot)?;
            }
        }
        let _ = from;
        Ok(())
    };
    move_in(g, a, b, &ea)?;
    move_in(g, b, a, &eb)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 补益（界吴国太）
// ---------------------------------------------------------------------------

pub const BUYI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "buyi",
    name: "补益",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: buyi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn buyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if g.seats.get(victim).is_none() || g.seats[victim].hp > 0 {
        return Ok(Step::Done);
    }
    if stage == "pick" {
        let mut opts: Vec<(String, String)> = g
            .hand(victim)
            .into_iter()
            .map(|id| (format!("pick:hand:{id}"), format!("手牌：{}", label(g, &id))))
            .collect();
        for id in equip_ids(g, victim) {
            opts.push((format!("pick:equip:{id}"), format!("装备区：{}", label(g, &id))));
        }
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("buyi");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("补益：{text}"));
                }
                Ok(Step::Need(b.escape("不发动补益").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let card = act
                    .strip_prefix("pick:hand:")
                    .or_else(|| act.strip_prefix("pick:equip:"))
                    .ok_or_else(|| format!("补益：未知选项 {act}"))?
                    .to_string();
                if !g.cards.contains_key(&card) {
                    return Ok(Step::Done);
                }
                g.reveal(&card, Some(victim), "buyi");
                g.push_log(format!(
                    "补益：{} 亮出【{}】",
                    g.seats[victim].name,
                    label(g, &card)
                ));
                let basic = g.def_of(&card).kind == CardKind::Basic;
                if basic {
                    g.conceal(&card, "buyi");
                    return Ok(Step::Done);
                }
                // 非基本牌：弃置并回复 1 点体力。
                g.conceal(&card, "buyi");
                g.to_discard(&card, "buyi")?;
                flow::revive_add(g, victim, 1, "buyi");
                g.push_log(format!(
                    "补益：{} 弃置【{}】并回复 1 点体力",
                    g.seats[victim].name,
                    label(g, &card)
                ));
                Ok(Step::Done)
            }
        }
    } else {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("buyi");
                b = b.option("opt1", "补益：选择其一张牌（非基本牌则弃置并回复 1）");
                Ok(Step::Need(b.escape("不发动补益").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick", "victim": victim }),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// 当先（界廖化）
// ---------------------------------------------------------------------------

pub const DANGXIAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "dangxian",
    name: "当先",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: dangxian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 准备阶段结束：执行额外出牌阶段（返回 true 表示已切换）。
pub fn start_extra_play_phase(g: &mut Game, seat: usize) -> bool {
    if !active(g, seat, "dangxian")
        || g.counter(&format!("turn.dangxian_done:{seat}")) > 0
        || g.seats[seat].flipped
    {
        return false;
    }
    g.bump(&format!("turn.dangxian_done:{seat}"));
    g.bump(&format!("turn.dangxian_extra:{seat}"));
    g.bump(&format!("turn.dangxian_no_dmg:{seat}"));
    g.push_log(format!(
        "当先：{} 执行一个额外的出牌阶段",
        g.seats[seat].name
    ));
    push(g, "dangxian", seat, json!({ "stage": "start" }));
    true
}

/// 出牌阶段结束（可能改写为「判定阶段」）。
pub fn on_end_play(g: &mut Game, seat: usize) -> Phase {
    if g.counter(&format!("turn.dangxian_extra:{seat}")) == 0 {
        return Phase::Discard;
    }
    g.counters
        .insert(format!("turn.dangxian_extra:{seat}"), 0);
    let no_dmg = g.counter(&format!("turn.dangxian_no_dmg:{seat}")) > 0;
    g.counters
        .insert(format!("turn.dangxian_no_dmg:{seat}"), 0);
    if no_dmg && g.seats[seat].hp > 0 {
        g.push_log(format!(
            "当先：{} 额外出牌阶段未造成伤害，自伤 1 点",
            g.seats[seat].name
        ));
        flow::damage(g, None, seat, 1);
    }
    g.push_log(format!(
        "当先：{} 额外出牌阶段结束",
        g.seats[seat].name
    ));
    Phase::Judge
}

fn dangxian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "start" {
        let has_sha = g
            .ids_in(|z| *z == Zone::Draw || *z == Zone::Discard)
            .into_iter()
            .any(|id| is_sha(g, &id));
        if !active(g, owner, "dangxian") || !has_sha {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("dangxian");
                b = b.option("opt1", "当先：从牌堆/弃牌堆获得一张【杀】（无距离）");
                Ok(Step::Need(b.escape("不获得").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let id = g
                    .ids_in(|z| *z == Zone::Draw)
                    .into_iter()
                    .find(|id| is_sha(g, id))
                    .or_else(|| {
                        g.ids_in(|z| *z == Zone::Discard)
                            .into_iter()
                            .find(|id| is_sha(g, id))
                    });
                if let Some(id) = id {
                    g.gain(&id, owner)?;
                    g.no_distance_cards.insert(id.clone());
                    g.push_log(format!(
                        "当先：{} 获得【{}】（本回合无距离限制）",
                        g.seats[owner].name,
                        label(g, &id)
                    ));
                }
                Ok(Step::Done)
            }
        }
    } else {
        // DamageAfter：本阶段造成过伤害 → 清除自伤标记。
        if a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter")
            && a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok()) == Some(owner)
        {
            g.counters
                .insert(format!("turn.dangxian_no_dmg:{owner}"), 0);
        }
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 伏枥（界廖化，限定）
// ---------------------------------------------------------------------------

pub const FULI: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "fuli",
    name: "伏枥",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: fuli,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn fuli(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let victim = a.data["victim"].as_u64().unwrap_or(owner as u64) as usize;
    if victim != owner || !active(g, owner, "fuli") {
        return Ok(Step::Done);
    }
    if g.seats[owner].flags.contains("fuli_used") {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() || stage == "ask" {
        match g.answer.take().as_deref() {
            None => {
                let x = super::kingdom_count(g);
                let mut b = builder(owner, "choice").skill("fuli");
                b = b.option(
                    "opt1",
                    format!("伏枥：回复至 {x} 点体力并摸至 {x} 张（限定技）"),
                );
                Ok(Step::Need(b.escape("不发动伏枥").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.seats[owner].flags.insert("fuli_used".into());
                // X=全场势力数（濒死者自己的势力也算）。
                let mut set = std::collections::BTreeSet::new();
                for (i, s) in g.seats.iter().enumerate() {
                    if s.hp > 0 || i == owner {
                        set.insert(s.kingdom.clone());
                    }
                }
                let x = set.len().max(1) as i32;
                flow::revive_to(g, owner, x, "fuli");
                let hand = g.hand(owner).len() as i32;
                if hand < x {
                    let n = g.draw_with_cause(owner, (x - hand) as usize, Some("fuli")).len();
                    g.push_log(format!("伏枥：{} 摸 {n} 张至 {x} 张", g.seats[owner].name));
                }
                let dealt = g.counter(&format!("total.damage_dealt:{owner}")) as i32;
                if x > dealt {
                    g.flip(owner);
                    g.push_log(format!(
                        "伏枥：X={x} > 本局造成伤害数 {dealt}，翻面"
                    ));
                }
                g.push_log(format!(
                    "伏枥：{} 回复至 {x} 点体力（体力 {}）",
                    g.seats[owner].name, g.seats[owner].hp
                ));
                Ok(Step::Done)
            }
        }
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 跨批钩子：进入濒死（补益 / 仁心 / 伏枥）
// ---------------------------------------------------------------------------

pub fn on_dying_enter(g: &mut Game, victim: usize, _source: Option<usize>) {
    // 伏枥（自救）：优先询问濒死者本人。
    if active(g, victim, "fuli") && !g.seats[victim].flags.contains("fuli_used") {
        push(
            g,
            "fuli",
            victim,
            json!({ "stage": "ask", "victim": victim }),
        );
    }
    // 补益（每名濒死者每次濒死一次）：找第一个其他角色。
    if !g.hand(victim).is_empty() || !equip_ids(g, victim).is_empty() {
        if let Some(owner) = alive(g)
            .into_iter()
            .find(|i| *i != victim && active(g, *i, "buyi"))
        {
            push(
                g,
                "buyi",
                owner,
                json!({ "stage": "ask", "victim": victim }),
            );
        }
    }
    // 仁心：逐个询问其他有仁心且有装备牌的角色。
    let owners: Vec<usize> = alive(g)
        .into_iter()
        .filter(|i| *i != victim && active(g, *i, "renxin"))
        .collect();
    for owner in owners {
        let has_equip = g.hand(owner).iter().any(|id| g.def_of(id).slot.is_some())
            || !equip_ids(g, owner).is_empty();
        if has_equip && g.seats[victim].hp <= 0 {
            push(
                g,
                "renxin",
                owner,
                json!({ "stage": "ask", "victim": victim }),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 跨批钩子：使用牌 / 目标 / 回合结束
// ---------------------------------------------------------------------------

pub fn on_card_use(g: &mut Game, seat: usize, carrier: Option<&CardId>, name: &str) {
    on_card_use_for_zishou(g, seat, name);
    // 酒诗（背面朝上时，落英牌无距离限制且不能被响应）。
    if let Some(card) = carrier {
        if active(g, seat, "jiushi")
            && g.seats[seat].flipped
            && g.luoying_cards.contains(card)
        {
            g.mark_unresponsive(card);
            g.push_log(format!(
                "酒诗：背面朝上，【{}】不能被响应",
                label(g, card)
            ));
        }
    }
}

pub fn on_targets_chosen(g: &mut Game, seat: usize, targets: &[usize]) {
    if targets.iter().any(|t| *t != seat) {
        g.bump(&format!("turn.targeted_others:{seat}"));
    }
}

/// 「无距离限制」的单张牌标记（酒诗的落英牌 / 当先获得的【杀】）。
pub fn card_no_distance(g: &Game, _seat: usize, card: Option<&CardId>) -> bool {
    card.is_some_and(|c| g.no_distance_cards.contains(c))
        || card.is_some_and(|c| g.luoying_cards.contains(c) && card_flipped_owner(g, c))
}

fn card_flipped_owner(g: &Game, card: &CardId) -> bool {
    // 落英牌当前在某名背面朝上的曹植手牌中 → 无距离。
    matches!(g.zone_of(card), Some(Zone::Hand(s)) if g.seats[*s].flipped && active(g, *s, "jiushi"))
}

pub fn sha_no_distance(g: &Game, seat: usize, card: &CardId) -> bool {
    card_no_distance(g, seat, Some(card))
}

pub const B18_SKILLS: &[SkillDef] = &[
    ZHENLIE,
    MIJI,
    JUEQING,
    SHANGSHI,
    JIANMIE,
    ZISHOU,
    ZONGSHI,
    XIANZHOU,
    QIETING,
    CHENGXIANG,
    RENXIN,
    LUOYING,
    JIUSHI,
    GANLU,
    BUYI,
    DANGXIAN,
    FULI,
];
