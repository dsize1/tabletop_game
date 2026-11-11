//! B13（界标·J1）——按 `align/B13.json` 第 7 段实现。
//!
//! 本文件当前覆盖：界黄忠（烈弓）、界徐庶（诛害/潜心 + 授予技「荐言」）、
//! 界华雄（耀武/势斩）、界张角（雷击/鬼道/黄天）。
//! 尚未实现（下一轮）：界左慈（化身/新生，需武将牌池生成物）、界于吉（蛊惑，需扣置+质疑链）、
//! 界袁绍（乱击/血裔）、界刘禅（享乐/放权/若愚 + 授予技「思蜀」）。
//!
//! 用户裁定（2026-11-09，54 条疑点）：
//! - 烈弓：距离 ≤ 此【杀】点数即达；询问一次；+1 与其他加成累加。
//! - 诛害：**必须真实使用手牌【杀】且无距离限制**；每个结束阶段一次。
//! - 潜心：「荐言」**没有存活男性角色时不能发动**。
//! - 耀武：红色牌但来源为自己/无来源 → 不摸牌。
//! - 雷击/鬼道：鬼道改判作用于张角自己的判定时，判定生效后可发动雷击的判定后效果。
//! - 黄天：仅黑桃手牌；每名给牌者各自每阶段一次；走统一「获得」。

use serde_json::json;

use crate::cards::{Effect, ResponseKind};
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, builder, grant_skill, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}
/// 主动技「本阶段已放弃」标记（防抖：同一阶段不再重复提示）。
fn cancelled(g: &Game, code: &str) -> bool {
    g.counter(&format!("phase.cancel_{code}")) > 0
}
fn mark_cancelled(g: &mut Game, code: &str) {
    g.bump(&format!("phase.cancel_{code}"));
}
fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}
fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}
fn is_sha(g: &Game, id: &CardId) -> bool {
    g.cards.get(id).map(|c| g.def_of(id).tags.sha).unwrap_or(false)
}
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, crate::kernel::resolve::Stage::Choose, actor, data);
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

// ---------------------------------------------------------------------------
// 烈弓（界黄忠）
// ---------------------------------------------------------------------------

pub const LIEGONG: SkillDef = SkillDef {
    code: "liegong",
    name: "烈弓",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: liegong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 【杀】的可达性覆盖：距离 ≤ 此【杀】点数（逐目标）。
pub fn range_override_reach(g: &Game, from: usize, to: usize, card: &CardId) -> bool {
    if !active(g, from, "liegong") || !is_sha(g, card) {
        return false;
    }
    let rank = g.cards.get(card).map(|c| c.rank as usize).unwrap_or(0);
    rank > 0 && g.distance(from, to) <= rank
}

/// 介入【杀】结算（目标锁定技的同窗口，见 `sha_intercept_skills`）。
pub fn sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let Some(card) = card else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if user != target && is_sha(g, card) {
        if active(g, user, "liegong") {
            out.push((user, "liegong"));
        }
        // 享乐（界刘禅，锁定）：目标视角介入。
        if active(g, target, "xiangle") {
            out.push((target, "xiangle"));
        }
    }
    out
}

fn liegong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("liegong");
            b = b.option("opt1", format!("发动烈弓（目标 {}）", g.seats[target].name));
            Ok(Step::Need(b.escape("不发动烈弓").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            let mut parts = Vec::new();
            if g.hand(owner).len() >= g.hand(target).len() {
                // 其不能使用【闪】抵消此【杀】
                g.sha_intercept = Some(ShaIntercept::Unresponsive(owner));
                parts.push("其不能使用【闪】");
            }
            if g.seats[owner].hp <= g.seats[target].hp {
                g.bump(&format!("turn.liegong_bonus:{owner}:{target}"));
                parts.push("此【杀】伤害 +1");
            }
            g.presentation.push(
                Presentation::new(
                    "liegong",
                    POutcome::Activated {
                        skill: "烈弓".to_string(),
                    },
                )
                .actor(owner)
                .target(target)
                .fx("liegong"),
            );
            g.push_log(format!(
                "烈弓：{}（{}）",
                g.seats[owner].name,
                if parts.is_empty() {
                    "条件均不满足".to_string()
                } else {
                    parts.join("；")
                }
            ));
            Ok(Step::Done)
        }
    }
}

/// 烈弓的伤害 +1（一次性标记，`on_sha_finished` 清理）。
pub fn liegong_bonus(g: &Game, source: usize, victim: usize, card: Option<&CardId>) -> i32 {
    let Some(card) = card else { return 0 };
    if !is_sha(g, card) {
        return 0;
    }
    if g.counter(&format!("turn.liegong_bonus:{source}:{victim}")) > 0 {
        1
    } else {
        0
    }
}

pub fn on_sha_finished(g: &mut Game, user: usize, target: Option<usize>) {
    if let Some(t) = target {
        g.counters.remove(&format!("turn.liegong_bonus:{user}:{t}"));
    }
}

// ---------------------------------------------------------------------------
// 诛害（界徐庶）：其他角色结束阶段且其本回合造成过伤害 → 真实使用手牌【杀】（无距离）
// ---------------------------------------------------------------------------

pub const ZHUHAI: SkillDef = SkillDef {
    code: "zhuhai",
    name: "诛害",
    triggers: &[EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhuhai,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhuhai(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if let Some(stage) = a.data["stage"].as_str() {
        if stage == "pick" {
            let ending = a.data["target"].as_u64().unwrap_or(0) as usize;
            return match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("zhuhai");
                    for id in g.hand(owner).into_iter().filter(|c| is_sha(g, c)) {
                        b = b.option(format!("pick:{id}"), format!("使用【{}】", label(g, &id)));
                    }
                    Ok(Step::Need(b.escape("不发动诛害").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("诛害：未知选项 {act}"))?
                        .to_string();
                    if !is_sha(g, &id) || !g.hand(owner).contains(&id) {
                        return Err("诛害：只能使用手牌【杀】".into());
                    }
                    g.presentation.push(
                        Presentation::new(
                            "zhuhai",
                            POutcome::Activated {
                                skill: "诛害".to_string(),
                            },
                        )
                        .actor(owner)
                        .target(ending)
                        .cards(vec![id.clone()])
                        .fx("zhuhai"),
                    );
                    g.push_log(format!(
                        "诛害：对 {} 使用【{}】（无距离限制）",
                        g.seats[ending].name,
                        label(g, &id)
                    ));
                    // 技能驱动的真实使用：`strict=false` → 不检查回合/阶段/距离。
                    g.use_as(owner, &id, "杀", vec![ending], false, None)?;
                    Ok(Step::Done)
                }
            };
        }
        return Ok(Step::Done);
    }
    // 触发：其他角色结束阶段
    let Some(ending) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    if ending == owner || g.turn != ending {
        return Ok(Step::Done);
    }
    if g.counter(&format!("turn.damage_by:{ending}")) == 0 {
        return Ok(Step::Done);
    }
    if g.answer.is_none() {
        if g.counter(&format!("turn.zhuhai_asked:{owner}")) > 0 {
            return Ok(Step::Done);
        }
        if !g.hand(owner).into_iter().any(|c| is_sha(g, &c)) {
            return Ok(Step::Done);
        }
        g.bump(&format!("turn.zhuhai_asked:{owner}"));
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("zhuhai");
            b = b.option(
                "opt1",
                format!("诛害：对 {} 使用一张【杀】", g.seats[ending].name),
            );
            Ok(Step::Need(b.escape("不发动诛害").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            push(
                g,
                "zhuhai",
                owner,
                json!({ "stage": "pick", "target": ending }),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 潜心（界徐庶，觉醒）+ 授予技「荐言」
// ---------------------------------------------------------------------------

pub const QIANXIN: SkillDef = SkillDef {
    code: "qianxin",
    name: "潜心",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qianxin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn qianxin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let Some(src) = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok()) else {
        return Ok(Step::Done);
    };
    if src != owner || a.data["amount"].as_i64().unwrap_or(0) <= 0 {
        return Ok(Step::Done);
    }
    if g.seats[owner].flags.contains("awaken_qianxin") || g.seats[owner].hp >= g.seats[owner].max_hp {
        return Ok(Step::Done);
    }
    g.seats[owner].flags.insert("awaken_qianxin".to_string());
    g.seats[owner].max_hp = (g.seats[owner].max_hp - 1).max(1);
    g.seats[owner].hp = g.seats[owner].hp.min(g.seats[owner].max_hp);
    grant_skill(g, owner, "jianyan");
    g.presentation.push(
        Presentation::new(
            "qianxin",
            POutcome::Activated {
                skill: "潜心".to_string(),
            },
        )
        .actor(owner)
        .fx("qianxin"),
    );
    g.push_log(format!(
        "潜心：{} 觉醒，体力上限 -1（{}），获得「荐言」",
        g.seats[owner].name, g.seats[owner].max_hp
    ));
    Ok(Step::Done)
}

pub const JIANYAN: SkillDef = SkillDef {
    code: "jianyan",
    name: "荐言",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jianyan),
    handler: jianyan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn male_alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0 && g.seats[*i].male)
        .collect()
}

fn drawing(g: &Game) -> Vec<CardId> {
    g.ids_in(|z| *z == Zone::Draw)
}

fn jianyan_matches(g: &Game, id: &CardId, mode: &str) -> bool {
    let def = g.def_of(id);
    match mode {
        "basic" => def.kind == crate::cards::CardKind::Basic,
        "trick" => def.kind == crate::cards::CardKind::Trick,
        "equip" => def.kind == crate::cards::CardKind::Equip,
        "red" => g.is_red(id),
        "black" => !g.is_red(id),
        _ => false,
    }
}

fn jianyan_modes(g: &Game) -> Vec<&'static str> {
    ["basic", "trick", "equip", "red", "black"]
        .into_iter()
        .filter(|m| drawing(g).into_iter().any(|id| jianyan_matches(g, &id, m)))
        .collect()
}

fn can_jianyan(g: &Game, seat: usize) -> bool {
    active(g, seat, "jianyan")
        && play_phase(g, seat)
        && g.counter("turn.jianyan_used") == 0
        && !male_alive(g).is_empty()
        && !jianyan_modes(g).is_empty()
}

fn jianyan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("target") {
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("jianyan");
                for i in male_alive(g) {
                    b = b.option(
                        format!("target:{i}"),
                        format!("令 {} 获得此牌", g.seats[i].name),
                    );
                }
                Ok(Step::Need(b.escape("放弃（牌置入弃牌堆）").build()))
            }
            Some("pass") => {
                if g.cards.contains_key(&card) {
                    g.to_discard(&card, "jianyan")?;
                }
                g.push_log("荐言：没有角色获得，亮出的牌置入弃牌堆".to_string());
                Ok(Step::Done)
            }
            Some(act) => {
                let target = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or_else(|| format!("荐言：未知选项 {act}"))?;
                if !male_alive(g).contains(&target) {
                    return Err("荐言：只能交给存活的男性角色".into());
                }
                if g.cards.contains_key(&card) {
                    g.gain(&card, target)?;
                }
                g.push_log(format!(
                    "荐言：{} 获得【{}】",
                    g.seats[target].name,
                    label(g, &card)
                ));
                Ok(Step::Done)
            }
        }
    } else {
        let modes = jianyan_modes(g);
        match g.answer.take().as_deref() {
            None => {
                g.bump("turn.jianyan_used");
                let mut b = builder(owner, "choice").skill("jianyan");
                for m in &modes {
                    let name = match *m {
                        "basic" => "基本牌",
                        "trick" => "锦囊牌",
                        "equip" => "装备牌",
                        "red" => "红色牌",
                        _ => "黑色牌",
                    };
                    b = b.option(format!("mode:{m}"), format!("声明{name}"));
                }
                Ok(Step::Need(b.escape("取消荐言").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let mode = act
                    .strip_prefix("mode:")
                    .ok_or_else(|| format!("荐言：未知选项 {act}"))?
                    .to_string();
                if !modes.contains(&mode.as_str()) {
                    return Err("荐言：牌堆中没有该类别/颜色的牌".into());
                }
                let card = drawing(g)
                    .into_iter()
                    .find(|id| jianyan_matches(g, id, &mode))
                    .ok_or_else(|| "荐言：牌堆中没有符合条件的牌".to_string())?;
                g.to_processing(&card)?;
                g.reveal(&card, Some(owner), "jianyan");
                g.push_log(format!(
                    "荐言：声明后亮出牌堆中的【{}】",
                    label(g, &card)
                ));
                push(
                    g,
                    "jianyan",
                    owner,
                    json!({ "stage": "target", "card": card }),
                );
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 耀武（界华雄，锁定）
// ---------------------------------------------------------------------------

pub const YAOWU: SkillDef = SkillDef {
    code: "yaowu",
    name: "耀武",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: yaowu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn yaowu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
        return Ok(Step::Done);
    }
    if a.data["amount"].as_i64().unwrap_or(0) <= 0 {
        return Ok(Step::Done);
    }
    let src = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok());
    let red = a
        .data["card"]
        .as_str()
        .map(|c| g.is_red(&c.to_string()))
        .unwrap_or(false);
    if red {
        if let Some(src) = src {
            if src != owner && g.seats.get(src).is_some_and(|s| s.hp > 0) {
                let n = g.draw(src, 1).len();
                g.push_log(format!(
                    "耀武：伤害牌为红色，{} 摸 {n} 张",
                    g.seats[src].name
                ));
                return Ok(Step::Done);
            }
        }
        g.push_log("耀武：伤害牌为红色但无有效来源，不摸牌".to_string());
        return Ok(Step::Done);
    }
    let n = g.draw(owner, 1).len();
    g.push_log(format!("耀武：伤害牌不为红色，{} 摸 {n} 张", g.seats[owner].name));
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 势斩（界华雄）：出牌阶段限两次，令一名其他角色视为对华雄使用【决斗】
// ---------------------------------------------------------------------------

pub const SHIZHAN: SkillDef = SkillDef {
    code: "shizhan",
    name: "势斩",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_shizhan),
    handler: shizhan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_shizhan(g: &Game, seat: usize) -> bool {
    active(g, seat, "shizhan")
        && play_phase(g, seat)
        && g.counter("turn.shizhan_used") < 2
        && !cancelled(g, "shizhan")
        && !other_alive(g, seat).is_empty()
}

fn shizhan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let cands = other_alive(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("shizhan");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("令 {} 视为对你使用【决斗】", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("取消势斩").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "shizhan");
            Ok(Step::Done)
        }
        Some(act) => {
            let target = act
                .strip_prefix("target:")
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("势斩：未知选项 {act}"))?;
            if !cands.contains(&target) {
                return Err("势斩：目标不合法".into());
            }
            if g.counter("turn.shizhan_used") >= 2 {
                return Err("势斩：本阶段已发动两次".into());
            }
            g.bump("turn.shizhan_used");
            g.presentation.push(
                Presentation::new(
                    "shizhan",
                    POutcome::Activated {
                        skill: "势斩".to_string(),
                    },
                )
                .actor(owner)
                .target(target)
                .fx("shizhan"),
            );
            g.push_log(format!(
                "势斩：{} 视为对 {} 使用【决斗】",
                g.seats[target].name, g.seats[owner].name
            ));
            // 视为使用普通锦囊：走真实结算（可被【无懈可击】响应）。
            g.use_virtual(target, "决斗", vec![owner])?;
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 雷击（界张角）
// ---------------------------------------------------------------------------

pub const LEIJI: SkillDef = SkillDef {
    code: "leiji",
    name: "雷击",
    triggers: &[EventKind::CardUsed, EventKind::CardPlayed, EventKind::JudgeEffective],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: leiji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn leiji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default().to_string();
    // 判定后效果（黑桃 2 点 / 梅花回复 + 1 点）
    if let Some(stage) = a.data["stage"].as_str() {
        let dmg = a.data["damage"].as_i64().unwrap_or(1) as i32;
        match stage {
            "ask" => {
                let card = a.data["card"].as_str().unwrap_or_default().to_string();
                match g.answer.take().as_deref() {
                    None => {
                        let mut b = builder(owner, "choice").skill("leiji");
                        b = b.option("opt1", "雷击：判定");
                        Ok(Step::Need(b.escape("不发动雷击").build()))
                    }
                    Some("pass") => Ok(Step::Done),
                    Some(_) => {
                        // 先压「判定后」帧，再压判定子流程（LIFO：判定先跑）。
                        push(
                            g,
                            "leiji",
                            owner,
                            json!({ "stage": "after", "from": "leiji" }),
                        );
                        crate::kernel::card_play::start_quick_judge(g, owner, "leiji");
                        let _ = card;
                        Ok(Step::Done)
                    }
                }
            }
            "after" => {
                let Some(card) = g.judge_result.take() else {
                    return Ok(Step::Done);
                };
                // 清掉鬼道联动标记，避免 JudgeEffective 分支重复触发。
                g.counters.remove(&format!("turn.guidao_leiji:{owner}"));
                leiji_effect(g, owner, &card)
            }
            "target" => match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("leiji");
                    for i in 0..g.seats.len() {
                        if g.seats[i].hp > 0 {
                            b = b.option(
                                format!("target:{i}"),
                                format!(
                                    "对 {} 造成 {dmg} 点雷电伤害",
                                    g.seats[i].name
                                ),
                            );
                        }
                    }
                    Ok(Step::Need(b.escape("不造成伤害").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| format!("雷击：未知选项 {act}"))?;
                    if g.seats.get(target).map(|s| s.hp > 0).unwrap_or(true) == false {
                        return Err("雷击：目标不合法".into());
                    }
                    g.push_log(format!(
                        "雷击：对 {} 造成 {dmg} 点雷电伤害",
                        g.seats[target].name
                    ));
                    flow::damage_typed(
                        g,
                        Some(owner),
                        target,
                        dmg,
                        crate::cards::DamageKind::Thunder,
                        None,
                        false,
                    );
                    Ok(Step::Done)
                }
            },
            _ => Ok(Step::Done),
        }
    } else if ev.starts_with("JudgeEffective") {
        // 鬼道改判张角自己的判定 → 判定后效果
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.guidao_leiji:{owner}")) == 0 {
            return Ok(Step::Done);
        }
        g.counters.remove(&format!("turn.guidao_leiji:{owner}"));
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        leiji_effect(g, owner, &card)
    } else {
        // 触发：使用/打出【闪】或使用【闪电】
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        let def = g.def_of(&card);
        let is_shan = def.tags.shan;
        let is_lightning = def.effect == Effect::Lightning;
        if !(is_shan || is_lightning) {
            return Ok(Step::Done);
        }
        if g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        push(g, "leiji", owner, json!({ "stage": "ask", "card": card }));
        Ok(Step::Done)
    }
}

/// 判定后效果：黑桃 → 可选 2 点雷伤；梅花 → 回复 1 并可选 1 点雷伤。
fn leiji_effect(g: &mut Game, owner: usize, card: &CardId) -> Result<Step, String> {
    let suit = g.cards.get(card).map(|c| c.suit);
    if !g.cards.contains_key(card) {
        return Ok(Step::Done);
    }
    match suit {
        Some(crate::kernel::Suit::Spade) => {
            g.push_log("雷击：判定为黑桃，可造成 2 点雷电伤害".to_string());
            push(g, "leiji", owner, json!({ "stage": "target", "damage": 2 }));
            Ok(Step::Done)
        }
        Some(crate::kernel::Suit::Club) => {
            let before = g.seats[owner].hp;
            g.seats[owner].hp = (before + 1).min(g.seats[owner].max_hp);
            let healed = g.seats[owner].hp - before;
            g.push_log(format!(
                "雷击：判定为梅花，{} 回复 {healed} 点体力",
                g.seats[owner].name
            ));
            push(g, "leiji", owner, json!({ "stage": "target", "damage": 1 }));
            Ok(Step::Done)
        }
        _ => {
            g.push_log("雷击：判定非黑桃/梅花，未生效".to_string());
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 鬼道（界张角）：黑色牌改判（手牌/装备区）；黑桃 2~9 摸 1
// ---------------------------------------------------------------------------

pub const GUIDAO: SkillDef = SkillDef {
    code: "guidao",
    name: "鬼道",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: guidao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

pub fn active_guidao(g: &Game, seat: usize) -> bool {
    active(g, seat, "guidao")
}

pub fn start_guidao(g: &mut Game, seat: usize, judge_owner: usize) {
    push(g, "guidao", seat, json!({ "judge_owner": judge_owner }));
}

/// 某座位可用于改判的牌（鬼才：手牌 + 装备区；鬼道：仅黑色）。
pub fn rewrite_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let cards = hand_and_equip(g, seat);
    if super::b11::active_guicai(g, seat) {
        return cards;
    }
    if super::b13::active_guidao(g, seat) {
        return cards.into_iter().filter(|id| !g.is_red(id)).collect();
    }
    Vec::new()
}

fn guidao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("guidao");
            for id in rewrite_cards(g, owner) {
                b = b.option(format!("pick:{id}"), format!("用【{}】改判", label(g, &id)));
            }
            Ok(Step::Need(b.escape("不改判").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("鬼道：未知选项 {act}"))?
                .to_string();
            if !rewrite_cards(g, owner).contains(&id) {
                return Err("鬼道：该牌不可用于改判".into());
            }
            let spade_2_9 = g
                .cards
                .get(&id)
                .is_some_and(|c| c.suit == crate::kernel::Suit::Spade && (2..=9).contains(&c.rank));
            if g.zone_of(&id) == Some(&Zone::Hand(owner)) {
                g.to_processing(&id)?;
            } else if let Some(slot) = g.def_of(&id).slot {
                g.discard_equip_slot(owner, slot, "guidao")?;
                g.to_processing(&id)?;
            }
            g.judge_rewrite = Some(id.clone());
            g.push_log(format!("鬼道：用【{}】改判", label(g, &id)));
            if spade_2_9 {
                let n = g.draw(owner, 1).len();
                g.push_log(format!("鬼道：改判牌为黑桃 2~9，摸 {n} 张"));
            }
            // 张角自己的判定 → 判定生效后可发动雷击的判定后效果。
            if a.data["judge_owner"].as_u64().map(|n| n as usize) == Some(owner) {
                g.bump(&format!("turn.guidao_leiji:{owner}"));
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 黄天（界张角，主公技）：其他群势力角色出牌阶段限一次交【闪】或黑桃手牌
// ---------------------------------------------------------------------------

pub const HUANGTIAN: SkillDef = SkillDef {
    code: "huangtian",
    name: "黄天",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_huangtian),
    handler: huangtian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn huangtian_lord(g: &Game, giver: usize) -> Option<usize> {
    if g.seats.get(giver).map(|s| s.kingdom.as_str()) != Some("qun") {
        return None;
    }
    (0..g.seats.len()).find(|i| {
        *i != giver
            && g.seats[*i].hp > 0
            && active(g, *i, "huangtian")
            && g.seats[*i].flags.contains("lord")
    })
}

fn huangtian_cards(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| {
            let def = g.def_of(id);
            def.tags.shan
                || g.cards
                    .get(id)
                    .is_some_and(|c| c.suit == crate::kernel::Suit::Spade)
        })
        .collect()
}

fn can_huangtian(g: &Game, seat: usize) -> bool {
    play_phase(g, seat)
        && !cancelled(g, "huangtian")
        && g.counter("turn.huangtian_used") == 0
        && huangtian_lord(g, seat).is_some()
        && !huangtian_cards(g, seat).is_empty()
}

fn huangtian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("huangtian");
            for id in huangtian_cards(g, owner) {
                b = b.option(format!("pick:{id}"), format!("交给张角【{}】", label(g, &id)));
            }
            Ok(Step::Need(b.escape("取消黄天").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "huangtian");
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("黄天：未知选项 {act}"))?
                .to_string();
            if !huangtian_cards(g, owner).contains(&id) {
                return Err("黄天：只能交【闪】或黑桃手牌".into());
            }
            let Some(lord) = huangtian_lord(g, owner) else {
                return Err("黄天：没有可用此技的主公".into());
            };
            g.bump("turn.huangtian_used");
            g.gain(&id, lord)?;
            g.push_log(format!(
                "黄天：{} 将【{}】交给 {}",
                g.seats[owner].name,
                label(g, &id),
                g.seats[lord].name
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 享乐（界刘禅，锁定）：使用者需弃基本牌，否则此【杀】对其无效
// ---------------------------------------------------------------------------

pub const XIANGLE: SkillDef = SkillDef {
    code: "xiangle",
    name: "享乐",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: xiangle,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn basic_hand(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| g.def_of(id).kind == crate::cards::CardKind::Basic)
        .collect()
}

fn xiangle(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let holder = a.data["target"].as_u64().unwrap_or(0) as usize;
    let user = a
        .data["user"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    if a.data["stage"].is_null() {
        // 首个询问发给【杀】的使用者（享乐是目标锁定技）。
        push(
            g,
            "xiangle",
            user,
            json!({ "stage": "ask", "target": holder, "user": user }),
        );
        return Ok(Step::Done);
    }
    if a.data["stage"].as_str() == Some("pick") {
        let cards = basic_hand(g, user);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(user, "choice").skill("xiangle");
                for id in &cards {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不弃牌（此【杀】无效）").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(ShaIntercept::Immune(holder));
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("享乐：未知选项 {act}"))?
                    .to_string();
                if !cards.contains(&id) {
                    return Err("享乐：只能弃置基本牌".into());
                }
                g.to_discard(&id, "xiangle")?;
                g.push_log(format!(
                    "享乐：{} 弃置【{}】，此【杀】正常结算",
                    g.seats[user].name,
                    label(g, &id)
                ));
                Ok(Step::Done)
            }
        };
    }
    if basic_hand(g, user).is_empty() {
        // 没有基本牌 → 此【杀】对享乐持有者无效（不产生 pending）。
        g.sha_intercept = Some(ShaIntercept::Immune(holder));
        g.push_log(format!(
            "享乐：{} 没有基本牌，此【杀】对 {} 无效",
            g.seats[user].name, g.seats[holder].name
        ));
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(user, "choice").skill("xiangle");
            b = b.option(
                "opt1",
                format!(
                    "享乐：弃置一张基本牌（否则此【杀】对 {} 无效）",
                    g.seats[holder].name
                ),
            );
            Ok(Step::Need(b.escape("不弃牌（此【杀】无效）").build()))
        }
        Some("pass") => {
            g.sha_intercept = Some(ShaIntercept::Immune(holder));
            g.push_log(format!(
                "享乐：{} 不弃牌，此【杀】对 {} 无效",
                g.seats[user].name, g.seats[holder].name
            ));
            Ok(Step::Done)
        }
        Some(_) => {
            push(
                g,
                "xiangle",
                user,
                json!({ "stage": "pick", "target": holder }),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 乱击（界袁绍）：两张同花色手牌当【万箭齐发】；可少选一个目标
// ---------------------------------------------------------------------------

pub const LUANJI: SkillDef = SkillDef {
    code: "luanji",
    name: "乱击",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_luanji),
    handler: luanji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn luanji_pairs(g: &Game, seat: usize) -> Vec<(CardId, CardId)> {
    let hand = g.hand(seat);
    let mut out = Vec::new();
    for i in 0..hand.len() {
        for j in (i + 1)..hand.len() {
            let (a, b) = (&hand[i], &hand[j]);
            if g.cards.get(a).map(|c| c.suit) == g.cards.get(b).map(|c| c.suit) {
                out.push((a.clone(), b.clone()));
            }
        }
    }
    out
}

fn can_luanji(g: &Game, seat: usize) -> bool {
    active(g, seat, "luanji")
        && play_phase(g, seat)
        && !cancelled(g, "luanji")
        && !other_alive(g, seat).is_empty()
        && !luanji_pairs(g, seat).is_empty()
}

fn luanji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("trim") {
        let c1 = a.data["card"].as_str().unwrap_or_default().to_string();
        let cands = other_alive(g, owner);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("luanji");
                for t in &cands {
                    b = b.option(format!("drop:{t}"), format!("少选 {}", g.seats[*t].name));
                }
                Ok(Step::Need(b.escape("不减少目标").build()))
            }
            Some("pass") => {
                luanji_use(g, owner, &c1)?;
                Ok(Step::Done)
            }
            Some(act) => {
                let t = act
                    .strip_prefix("drop:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("乱击：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("乱击：目标不合法".into());
                }
                g.counters
                    .insert("turn.aoe_trim".to_string(), t as u32 + 1);
                luanji_use(g, owner, &c1)?;
                Ok(Step::Done)
            }
        };
    }
    let pairs = luanji_pairs(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("luanji");
            for (a1, a2) in &pairs {
                b = b.option(
                    format!("pair:{a1},{a2}"),
                    format!("【{}】【{}】当【万箭齐发】", label(g, a1), label(g, a2)),
                );
            }
            Ok(Step::Need(b.escape("取消乱击").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "luanji");
            Ok(Step::Done)
        }
        Some(act) => {
            let pair = act
                .strip_prefix("pair:")
                .ok_or_else(|| format!("乱击：未知选项 {act}"))?;
            let (c1, c2) = pair
                .split_once(',')
                .ok_or_else(|| "乱击：需要两张手牌".to_string())?;
            let (c1, c2) = (c1.to_string(), c2.to_string());
            if !pairs.contains(&(c1.clone(), c2.clone())) {
                return Err("乱击：需要两张花色相同的手牌".into());
            }
            g.set_virtual(&c1, "万箭齐发", vec![c1.clone(), c2.clone()]);
            g.push_log(format!(
                "乱击：将【{}】【{}】当【万箭齐发】",
                label(g, &c1),
                label(g, &c2)
            ));
            if other_alive(g, owner).len() >= 2 {
                push(
                    g,
                    "luanji",
                    owner,
                    json!({ "stage": "trim", "card": c1 }),
                );
                Ok(Step::Done)
            } else {
                luanji_use(g, owner, &c1)?;
                Ok(Step::Done)
            }
        }
    }
}

fn luanji_use(g: &mut Game, owner: usize, card: &CardId) -> Result<(), String> {
    g.use_as(owner, card, "万箭齐发", Vec::new(), true, None)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 血裔（界袁绍，主公技）：「裔」标记 / 出牌阶段开始摸 1 / 手牌上限 +1
// ---------------------------------------------------------------------------

pub const XUEYI: SkillDef = SkillDef {
    code: "xueyi",
    name: "血裔",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: xueyi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

pub fn xueyi_marks(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get("yi"))
        .copied()
        .unwrap_or(0)
}

pub fn init_xueyi(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if !active(g, seat, "xueyi") || !g.seats[seat].flags.contains("lord") {
            continue;
        }
        let qun = (0..g.seats.len())
            .filter(|i| g.seats[*i].hp > 0 && g.seats[*i].kingdom == "qun")
            .count() as u32;
        if qun == 0 {
            continue;
        }
        g.seats[seat].counters.insert("yi".to_string(), qun * 2);
        g.push_log(format!(
            "血裔：全场 {qun} 名群势力角色，{} 获得 {} 枚「裔」",
            g.seats[seat].name,
            qun * 2
        ));
    }
}

pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "xueyi") && g.seats[seat].flags.contains("lord") {
        xueyi_marks(g, seat) as i32
    } else {
        0
    }
}

fn xueyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::Play || !g.seats[owner].flags.contains("lord") {
        return Ok(Step::Done);
    }
    if g.answer.is_none() {
        if g.counter("phase.xueyi_asked") > 0 || xueyi_marks(g, owner) == 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.xueyi_asked");
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("xueyi");
            b = b.option(
                "opt1",
                format!(
                    "移去 1 枚「裔」（剩 {}）并摸一张牌",
                    xueyi_marks(g, owner) - 1
                ),
            );
            Ok(Step::Need(b.escape("不发动血裔").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            let cur = xueyi_marks(g, owner);
            g.seats[owner].counters.insert("yi".to_string(), cur - 1);
            let n = g.draw(owner, 1).len();
            g.push_log(format!("血裔：移去 1 枚「裔」，摸 {n} 张"));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 放权（界刘禅）：跳过出牌阶段；弃牌阶段开始时弃一张手牌令他人额外回合
// ---------------------------------------------------------------------------

pub const FANGQUAN: SkillDef = SkillDef {
    code: "fangquan",
    name: "放权",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: Some(can_fangquan),
    handler: fangquan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_fangquan(g: &Game, seat: usize) -> bool {
    active(g, seat, "fangquan")
        && play_phase(g, seat)
        && g.counter("turn.fangquan_used") == 0
        && !other_alive(g, seat).is_empty()
}

fn fangquan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    // 触发路径（PhaseStart）：只在弃牌阶段开始时询问弃牌 → 指定额外回合目标。
    if a.data["event"].is_string() {
        if g.phase != Phase::Discard
            || !g.seats[owner].flags.contains("fangquan_pending")
            || g.hand(owner).is_empty()
        {
            return Ok(Step::Done);
        }
        push(g, "fangquan", owner, json!({ "stage": "discard" }));
        return Ok(Step::Done);
    }
    match a.data["stage"].as_str() {
        None => {
            if !play_phase(g, owner) {
                return Ok(Step::Done);
            }
            // 主动发动：跳过出牌阶段
            g.bump("turn.fangquan_used");
            g.counters.insert("turn.skip_play".to_string(), 1);
            g.seats[owner]
                .flags
                .insert("fangquan_pending".to_string());
            g.presentation.push(
                Presentation::new(
                    "fangquan",
                    POutcome::Activated {
                        skill: "放权".to_string(),
                    },
                )
                .actor(owner)
                .fx("fangquan"),
            );
            g.push_log(format!("放权：{} 跳过出牌阶段", g.seats[owner].name));
            Ok(Step::Done)
        }
        Some("discard") => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("fangquan");
                for id in g.hand(owner) {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("放弃放权（不授予额外回合）").build()))
            }
            Some("pass") => {
                g.seats[owner].flags.remove("fangquan_pending");
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("放权：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&id) {
                    return Err("放权：该牌不在手牌".into());
                }
                g.to_discard(&id, "fangquan")?;
                push(g, "fangquan", owner, json!({ "stage": "target" }));
                Ok(Step::Done)
            }
        },
        Some("target") => {
            let cands = other_alive(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("fangquan");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("令 {} 执行额外回合", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃放权").build()))
                }
                Some("pass") => {
                    g.seats[owner].flags.remove("fangquan_pending");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("放权：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("放权：目标不合法".into());
                    }
                    g.seats[owner].flags.remove("fangquan_pending");
                    g.extra_turn = Some(t);
                    g.push_log(format!(
                        "放权：{} 令 {} 执行额外回合",
                        g.seats[owner].name, g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 若愚（界刘禅，主公技+觉醒）+ 授予技「思蜀」
// ---------------------------------------------------------------------------

pub const RUOYU: SkillDef = SkillDef {
    code: "ruoyu",
    name: "若愚",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 0,
    can_active: None,
    handler: ruoyu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn ruoyu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::Prepare || !g.seats[owner].flags.contains("lord") {
        return Ok(Step::Done);
    }
    if g.seats[owner].flags.contains("awaken_ruoyu") {
        return Ok(Step::Done);
    }
    // 必须**严格小于**全场其他角色（并列最小不算）。
    let strictly_min = (0..g.seats.len())
        .filter(|i| *i != owner && g.seats[*i].hp > 0)
        .all(|i| g.seats[i].hp > g.seats[owner].hp);
    if !strictly_min {
        return Ok(Step::Done);
    }
    g.seats[owner].flags.insert("awaken_ruoyu".to_string());
    g.seats[owner].max_hp += 1;
    if g.seats[owner].hp < 3 {
        g.seats[owner].hp = 3.min(g.seats[owner].max_hp);
    }
    grant_skill(g, owner, "jijiang");
    grant_skill(g, owner, "sishu");
    g.presentation.push(
        Presentation::new(
            "ruoyu",
            POutcome::Activated {
                skill: "若愚".to_string(),
            },
        )
        .actor(owner)
        .fx("ruoyu"),
    );
    g.push_log(format!(
        "若愚：{} 觉醒，体力上限 +1（{}），获得「激将」「思蜀」",
        g.seats[owner].name, g.seats[owner].max_hp
    ));
    Ok(Step::Done)
}

pub const SISHU: SkillDef = SkillDef {
    code: "sishu",
    name: "思蜀",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: sishu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 当前反转目标（`g.counters["sishu_target"]` 存 target+1，0 = 无）。
pub fn sishu_target(g: &Game) -> Option<usize> {
    g.counter("sishu_target")
        .checked_sub(1)
        .map(|n| n as usize)
}

pub fn sishu_reversed(g: &Game, seat: usize) -> bool {
    g.seats
        .get(seat)
        .is_some_and(|s| s.flags.contains("sishu_reverse"))
}

fn sishu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    if g.answer.is_none() {
        if g.counter("phase.sishu_asked") > 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.sishu_asked");
    }
    let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("sishu");
            for t in &cands {
                let act = if sishu_reversed(g, *t) {
                    "恢复"
                } else {
                    "反转"
                };
                b = b.option(
                    format!("target:{t}"),
                    format!("{act} {} 的【乐不思蜀】判定", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动思蜀").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("思蜀：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("思蜀：目标不合法".into());
            }
            let already = sishu_reversed(g, t);
            if let Some(prev) = sishu_target(g) {
                g.seats[prev].flags.remove("sishu_reverse");
            }
            if already {
                // toggle off：恢复常规效果
                g.seats[t].flags.remove("sishu_reverse");
                g.counters.remove("sishu_target");
                g.push_log(format!(
                    "思蜀：{} 的【乐不思蜀】判定恢复常规",
                    g.seats[t].name
                ));
            } else {
                g.seats[t].flags.insert("sishu_reverse".to_string());
                g.counters.insert("sishu_target".to_string(), t as u32 + 1);
                g.push_log(format!(
                    "思蜀：{} 的【乐不思蜀】判定效果反转（红桃→跳过）",
                    g.seats[t].name
                ));
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 化身（界左慈）+ 新生
// ---------------------------------------------------------------------------

pub const HUASHEN: SkillDef = SkillDef {
    code: "huashen",
    name: "化身",
    triggers: &[EventKind::TurnStart, EventKind::PhaseEnd],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: huashen,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn huashen_state(g: &Game, seat: usize) -> crate::kernel::HuashenState {
    g.huashen.get(&seat).cloned().unwrap_or_default()
}

/// 某武将可授予的技能 code（按 heroes.json 的技能名映射注册表；只保留已注册的）。
pub fn huashen_skill_codes(general_code: &str) -> Vec<&'static str> {
    let Some(def) = crate::generals::by_code(general_code) else {
        return Vec::new();
    };
    let mut out: Vec<&'static str> = Vec::new();
    for name in def.skills {
        if let Some(code) = crate::skills::code_by_name(name) {
            if !out.contains(&code) {
                out.push(code);
            }
        }
    }
    out
}

/// 从上线池里抽一张不在 `owned` 中的化身牌。
fn huashen_draw_card(g: &mut Game, own_general: &str, owned: &[String]) -> Option<String> {
    let pool: Vec<String> = crate::generals::live()
        .filter(|d| d.code != own_general && !owned.iter().any(|c| c == d.code))
        .map(|d| d.code.to_string())
        .collect();
    if pool.is_empty() {
        return None;
    }
    let idx = g.rand_below(pool.len());
    Some(pool[idx].clone())
}

/// 展示某张化身牌：撤旧技能 → 授予新技能（多技能时询问）→ 覆盖势力/性别。
fn huashen_activate(g: &mut Game, seat: usize, code: &str) -> Result<Step, String> {
    let mut st = huashen_state(g, seat);
    if let Some(old) = st.active_skill.clone() {
        g.seats[seat].granted.retain(|x| x != &old);
    }
    if let (Some(def), true) = (crate::generals::by_code(code), true) {
        g.seats[seat].kingdom = def.kingdom.to_string();
        g.seats[seat].male = def.male;
    }
    st.active = Some(code.to_string());
    st.active_skill = None;
    g.huashen.insert(seat, st.clone());
    let codes = huashen_skill_codes(code);
    let name = crate::generals::by_code(code).map(|d| d.name).unwrap_or(code);
    match codes.len() {
        0 => {
            g.push_log(format!("化身：展示「{name}」（该武将无已注册技能）"));
            crate::skills::rebuild_subs(g);
            Ok(Step::Done)
        }
        1 => {
            crate::skills::grant_skill(g, seat, codes[0]);
            let mut st = huashen_state(g, seat);
            st.active_skill = Some(codes[0].to_string());
            g.huashen.insert(seat, st);
            g.push_log(format!(
                "化身：展示「{name}」，获得技能「{}」",
                crate::skills::def(codes[0]).map(|d| d.name).unwrap_or(codes[0])
            ));
            Ok(Step::Done)
        }
        _ => {
            push(
                g,
                "huashen",
                seat,
                json!({ "stage": "skill", "card": code }),
            );
            Ok(Step::Done)
        }
    }
}

pub fn init_huashen(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if !active(g, seat, "huashen") {
            continue;
        }
        if g.huashen.get(&seat).is_some_and(|s| !s.cards.is_empty()) {
            continue;
        }
        let own = g.seats[seat].general.clone();
        let mut cards: Vec<String> = Vec::new();
        for _ in 0..3 {
            let Some(c) = huashen_draw_card(g, &own, &cards) else {
                break;
            };
            cards.push(c);
        }
        if cards.is_empty() {
            continue;
        }
        g.huashen.insert(
            seat,
            crate::kernel::HuashenState {
                cards: cards.clone(),
                active: None,
                active_skill: None,
                orig_kingdom: g.seats[seat].kingdom.clone(),
                orig_male: g.seats[seat].male,
            },
        );
        g.push_log(format!(
            "化身：{} 获得 {} 张化身牌",
            g.seats[seat].name,
            cards.len()
        ));
        let first = cards[0].clone();
        let _ = huashen_activate(g, seat, &first);
    }
}

fn huashen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("ask").to_string();
    let ev = a.data["event"].as_str().unwrap_or_default().to_string();
    let mut st = huashen_state(g, owner);
    match stage.as_str() {
        "skill" => {
            let code = a.data["card"].as_str().unwrap_or_default().to_string();
            let codes = huashen_skill_codes(&code);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("huashen");
                    for c in &codes {
                        b = b.option(
                            format!("skill:{c}"),
                            format!(
                                "获得技能「{}」",
                                crate::skills::def(c).map(|d| d.name).unwrap_or(c)
                            ),
                        );
                    }
                    Ok(Step::Need(b.escape("不获得技能").build()))
                }
                Some("pass") => {
                    crate::skills::rebuild_subs(g);
                    Ok(Step::Done)
                }
                Some(act) => {
                    let pick = act
                        .strip_prefix("skill:")
                        .ok_or_else(|| format!("化身：未知选项 {act}"))?;
                    if !codes.contains(&pick) {
                        return Err("化身：技能不合法".into());
                    }
                    crate::skills::grant_skill(g, owner, pick);
                    let mut st = huashen_state(g, owner);
                    st.active_skill = Some(pick.to_string());
                    g.huashen.insert(owner, st);
                    g.push_log(format!(
                        "化身：获得技能「{}」",
                        crate::skills::def(pick).map(|d| d.name).unwrap_or(pick)
                    ));
                    Ok(Step::Done)
                }
            }
        }
        "pick" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("huashen");
                for c in st.cards.iter().filter(|c| Some(*c) != st.active.as_ref()) {
                    let name = crate::generals::by_code(c).map(|d| d.name).unwrap_or(c);
                    b = b.option(format!("pick:{c}"), format!("替换为「{name}」"));
                }
                Ok(Step::Need(b.escape("保持当前化身").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let code = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("化身：未知选项 {act}"))?
                    .to_string();
                if !st.cards.iter().any(|c| c == &code) || Some(&code) == st.active.as_ref() {
                    return Err("化身：只能替换为其他化身牌".into());
                }
                huashen_activate(g, owner, &code)
            }
        },
        _ => {
            // 触发：回合开始（TurnStart）/ 回合结束（PhaseEnd, cause=End）
            let is_start = ev.starts_with("TurnStart");
            let is_end = ev.starts_with("PhaseEnd") && a.data["cause"].as_str() == Some("End");
            if !is_start && !is_end {
                return Ok(Step::Done);
            }
            if st.cards.is_empty() {
                return Ok(Step::Done);
            }
            let key = if is_start { "turn.huashen_start" } else { "turn.huashen_end" };
            if g.answer.is_none() {
                if g.counter(key) > 0 {
                    return Ok(Step::Done);
                }
                g.bump(key);
            }
            st = huashen_state(g, owner);
            let others = st
                .cards
                .iter()
                .filter(|c| Some(*c) != st.active.as_ref())
                .count();
            if others == 0 {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("huashen");
                    b = b.option("opt1", "替换展示的化身牌及技能");
                    if others > 0 {
                        b = b.option("opt2", "移去至多两张化身牌，换一张新的化身牌");
                    }
                    Ok(Step::Need(b.escape("保持当前化身").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("opt1") => {
                    push(g, "huashen", owner, json!({ "stage": "pick" }));
                    Ok(Step::Done)
                }
                Some(_) => {
                    // 2 换 1：移去至多两张非展示化身牌，获得一张新的化身牌。
                    let active = st.active.clone();
                    let mut removed = 0;
                    st.cards.retain(|c| {
                        if Some(c) == active.as_ref() || removed >= 2 {
                            true
                        } else {
                            removed += 1;
                            false
                        }
                    });
                    if removed == 0 {
                        return Ok(Step::Done);
                    }
                    let own = g.seats[owner].general.clone();
                    let owned = st.cards.clone();
                    let gained = huashen_draw_card(g, &own, &owned);
                    if let Some(card) = gained {
                        st.cards.push(card.clone());
                        let name = crate::generals::by_code(&card)
                            .map(|d| d.name)
                            .unwrap_or(card.as_str());
                        g.push_log(format!(
                            "化身：移去 {removed} 张化身牌，获得「{name}」"
                        ));
                    } else {
                        g.push_log(format!("化身：移去 {removed} 张化身牌（池空，未补牌）"));
                    }
                    g.huashen.insert(owner, st);
                    Ok(Step::Done)
                }
            }
        }
    }
}

pub const XINSHENG: SkillDef = SkillDef {
    code: "xinsheng",
    name: "新生",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: xinsheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn xinsheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let remaining = a
        .data["remaining"]
        .as_u64()
        .unwrap_or_else(|| a.data["amount"].as_u64().unwrap_or(0));
    if a.data["stage"].as_str() != Some("ask") {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        if remaining == 0 {
            return Ok(Step::Done);
        }
        push(
            g,
            "xinsheng",
            owner,
            json!({ "stage": "ask", "remaining": remaining }),
        );
        return Ok(Step::Done);
    }
    if remaining == 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("xinsheng");
            b = b.option("opt1", "新生：获得一张新的化身牌");
            Ok(Step::Need(b.escape("不获得").build()))
        }
        Some("pass") => {
            if remaining > 1 {
                push(
                    g,
                    "xinsheng",
                    owner,
                    json!({ "stage": "ask", "remaining": remaining - 1 }),
                );
            }
            Ok(Step::Done)
        }
        Some(_) => {
            let st = huashen_state(g, owner);
            let own = g.seats[owner].general.clone();
            let owned = st.cards.clone();
            if let Some(card) = huashen_draw_card(g, &own, &owned) {
                let mut st = st;
                st.cards.push(card.clone());
                g.huashen.insert(owner, st);
                let name = crate::generals::by_code(&card)
                    .map(|d| d.name)
                    .unwrap_or(card.as_str());
                g.push_log(format!("新生：获得化身牌「{name}」"));
            }
            if remaining > 1 {
                push(
                    g,
                    "xinsheng",
                    owner,
                    json!({ "stage": "ask", "remaining": remaining - 1 }),
                );
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 蛊惑（界于吉）：扣置声明 + 同时质疑链 + 缠怨
// ---------------------------------------------------------------------------

pub const GUHUO: SkillDef = SkillDef {
    code: "guhuo",
    name: "蛊惑",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_guhuo),
    handler: guhuo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: Some(guhuo_wuxie_opts),
    wuxie_use: Some(guhuo_wuxie_use),
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

pub fn guhuo_available(g: &Game, seat: usize) -> bool {
    active(g, seat, "guhuo")
        && g.counter(&format!("turn.guhuo_used:{seat}")) == 0
        && !g.hand(seat).is_empty()
        && g.seats.get(seat).is_some_and(|s| s.hp > 0)
}

fn can_guhuo(g: &Game, seat: usize) -> bool {
    active(g, seat, "guhuo") && play_phase(g, seat) && guhuo_available(g, seat)
}

/// 响应窗口进入蛊惑：声明牌名由需要牌固定（闪/杀/无懈）。
pub fn start_guhuo_response(
    g: &mut Game,
    seat: usize,
    kind: ResponseKind,
) -> Result<(), String> {
    if !guhuo_available(g, seat) {
        return Err("蛊惑：本回合已使用过或没有手牌".into());
    }
    let declared = match kind {
        ResponseKind::Shan => "闪",
        ResponseKind::Sha => "杀",
        ResponseKind::Wuxie => "无懈可击",
    };
    push(
        g,
        "guhuo",
        seat,
        json!({
            "stage": "card",
            "owner": seat,
            "purpose": "respond",
            "declared": declared,
        }),
    );
    Ok(())
}

/// 可声明的基本牌/普通锦囊（按牌定义属性筛选；延时锦囊与【无懈可击】不可声明）。
fn declare_options(g: &Game, seat: usize) -> Vec<&'static str> {
    let mut out = Vec::new();
    for row in crate::cards_generated::DEFS {
        let usable = match row.kind {
            crate::cards::CardKind::Basic => !row.shan && !row.wuxie,
            crate::cards::CardKind::Trick => !row.delayed && !row.wuxie,
            _ => false,
        };
        if !usable {
            continue;
        }
        if row.effect == Effect::None {
            continue;
        }
        if row.tao && g.seats[seat].hp >= g.seats[seat].max_hp {
            continue;
        }
        out.push(row.name);
    }
    out
}

fn guhuo_questions(g: &Game, owner: usize) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|i| {
            *i != owner
                && g.seats[*i].hp > 0
                && !g.seats[*i].flags.contains("chanyuan")
        })
        .collect()
}

/// 濒死窗口的额外选项（蛊惑）。
pub fn guhuo_dying_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !guhuo_available(g, seat) {
        return Vec::new();
    }
    vec![("guhuo".to_string(), "蛊惑：扣置一张手牌当【桃】".to_string())]
}

/// 濒死窗口的蛊惑入口（启动质疑链，结果经 `Game.guhuo_result` 回传）。
pub fn guhuo_dying_use(
    g: &mut Game,
    seat: usize,
    victim: usize,
    act: &str,
) -> Result<bool, String> {
    if act != "guhuo" {
        return Ok(false);
    }
    if !guhuo_available(g, seat) {
        return Err("蛊惑：本回合已使用过或没有手牌".into());
    }
    g.bump(&format!("turn.guhuo_used:{seat}"));
    g.counters
        .insert("guhuo_dying_victim".to_string(), victim as u32 + 1);
    push(
        g,
        "guhuo",
        seat,
        json!({
            "stage": "card",
            "owner": seat,
            "purpose": "dying",
            "declared": "桃",
        }),
    );
    g.skill_deferred = true;
    Ok(true)
}

/// 无懈窗口的额外选项（蛊惑）。
fn guhuo_wuxie_opts(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !guhuo_available(g, seat) {
        return Vec::new();
    }
    vec![(
        "guhuo".to_string(),
        "蛊惑：扣置一张手牌当【无懈可击】".to_string(),
    )]
}

fn guhuo_wuxie_use(g: &mut Game, seat: usize, act: &str) -> Result<bool, String> {
    if act != "guhuo" {
        return Ok(false);
    }
    start_guhuo_response(g, seat, ResponseKind::Wuxie)?;
    g.skill_deferred = true;
    Ok(true)
}

fn guhuo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a
        .data["owner"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    let stage = a.data["stage"].as_str().unwrap_or("card").to_string();
    let purpose = a.data["purpose"].as_str().unwrap_or("play").to_string();
    match stage.as_str() {
        "card" => match g.answer.take().as_deref() {
            None => {
                if purpose == "play" {
                    g.bump(&format!("turn.guhuo_used:{owner}"));
                }
                let mut b = builder(owner, "choice").skill("guhuo");
                for id in g.hand(owner) {
                    b = b.option(format!("pick:{id}"), format!("扣置【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("取消蛊惑").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let carrier = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("蛊惑：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&carrier) {
                    return Err("蛊惑：只能扣置手牌".into());
                }
                if purpose == "respond" || purpose == "dying" {
                    g.bump(&format!("turn.guhuo_used:{owner}"));
                    let fallback = if purpose == "dying" { "桃" } else { "闪" };
                    let declared = a.data["declared"].as_str().unwrap_or(fallback).to_string();
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "question",
                            "owner": owner,
                            "purpose": purpose,
                            "declared": declared,
                            "carrier": carrier,
                            "questions": [],
                            "index": 0,
                        }),
                    );
                } else {
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "declare",
                            "owner": owner,
                            "purpose": purpose,
                            "carrier": carrier,
                        }),
                    );
                }
                Ok(Step::Done)
            }
        },
        "declare" => {
            let carrier = a.data["carrier"].as_str().unwrap_or_default().to_string();
            let names = declare_options(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("guhuo");
                    for n in &names {
                        b = b.option(format!("declare:{n}"), format!("蛊惑为【{n}】"));
                    }
                    Ok(Step::Need(b.escape("取消蛊惑").build()))
                }
                Some("pass") => {
                    if g.cards.contains_key(&carrier) {
                        g.guhuo_result = None;
                    }
                    Ok(Step::Done)
                }
                Some(act) => {
                    let declared = act
                        .strip_prefix("declare:")
                        .ok_or_else(|| format!("蛊惑：未知选项 {act}"))?
                        .to_string();
                    if !names.contains(&declared.as_str()) {
                        return Err("蛊惑：该牌名不可声明".into());
                    }
                    let def = crate::cards::def(&declared);
                    let (stage, targets) = match def.tags.target_mode {
                        Some(crate::cards::TargetMode::AllAlive) => ("question", Vec::new()),
                        Some(crate::cards::TargetMode::SelfOnly) => ("question", vec![owner]),
                        _ => ("target", Vec::new()),
                    };
                    if stage == "question" {
                        push(
                            g,
                            "guhuo",
                            owner,
                            json!({
                                "stage": "question",
                                "owner": owner,
                                "purpose": purpose,
                                "declared": declared,
                                "carrier": carrier,
                                "targets": targets,
                                "questions": [],
                                "index": 0,
                            }),
                        );
                    } else {
                        push(
                            g,
                            "guhuo",
                            owner,
                            json!({
                                "stage": "target",
                                "owner": owner,
                                "purpose": purpose,
                                "declared": declared,
                                "carrier": carrier,
                            }),
                        );
                    }
                    Ok(Step::Done)
                }
            }
        }
        "target" => {
            let declared = a.data["declared"].as_str().unwrap_or_default().to_string();
            let carrier = a.data["carrier"].as_str().unwrap_or_default().to_string();
            let def = crate::cards::def(&declared);
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != owner && g.seats[*i].hp > 0)
                // 借刀杀人的首位目标必须装备武器。
                .filter(|i| {
                    def.effect != Effect::Collateral
                        || g.equip_in(*i, crate::kernel::Slot::Weapon).is_some()
                })
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("guhuo");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("目标：{}", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消蛊惑").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("蛊惑：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("蛊惑：目标不合法".into());
                    }
                    let def = crate::cards::def(&declared);
                    if matches!(def.effect, Effect::Collateral | Effect::IronChain) {
                        push(
                            g,
                            "guhuo",
                            owner,
                            json!({
                                "stage": "target2",
                                "owner": owner,
                                "purpose": purpose,
                                "declared": declared,
                                "carrier": carrier,
                                "targets": vec![t],
                            }),
                        );
                        return Ok(Step::Done);
                    }
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "question",
                            "owner": owner,
                            "purpose": purpose,
                            "declared": declared,
                            "carrier": carrier,
                            "targets": vec![t],
                            "questions": [],
                            "index": 0,
                        }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "target2" => {
            let declared = a.data["declared"].as_str().unwrap_or_default().to_string();
            let carrier = a.data["carrier"].as_str().unwrap_or_default().to_string();
            let first: Vec<usize> = a.data["targets"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let def = crate::cards::def(&declared);
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != owner && g.seats[*i].hp > 0 && !first.contains(i))
                .collect();
            let mut targets = first.clone();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("guhuo");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("第二目标：{}", g.seats[*t].name),
                        );
                    }
                    // 铁索连环允许只指定 1 个目标；借刀杀人必须 2 个。
                    if def.effect == Effect::IronChain {
                        Ok(Step::Need(b.escape("只指定一个目标").build()))
                    } else {
                        Ok(Step::Need(
                            b.escape_with("cancel", "取消蛊惑").build(),
                        ))
                    }
                }
                Some("pass") => {
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "question",
                            "owner": owner,
                            "purpose": purpose,
                            "declared": declared,
                            "carrier": carrier,
                            "targets": targets,
                            "questions": [],
                            "index": 0,
                        }),
                    );
                    Ok(Step::Done)
                }
                Some("cancel") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("蛊惑：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("蛊惑：第二目标不合法".into());
                    }
                    targets.push(t);
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "question",
                            "owner": owner,
                            "purpose": purpose,
                            "declared": declared,
                            "carrier": carrier,
                            "targets": targets,
                            "questions": [],
                            "index": 0,
                        }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "question" => {
            let carrier = a.data["carrier"].as_str().unwrap_or_default().to_string();
            let declared = a.data["declared"].as_str().unwrap_or_default().to_string();
            let targets: Vec<usize> = a.data["targets"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let mut questions: Vec<usize> = a.data["questions"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let order = guhuo_questions(g, owner);
            let mut idx = a.data["index"].as_u64().unwrap_or(0) as usize;
            // 跳过已不在场的顺序项
            while idx < order.len() && !order.contains(&order[idx]) {
                idx += 1;
            }
            if idx >= order.len() {
                return guhuo_resolve(g, owner, &carrier, &declared, &targets, &purpose, &questions);
            }
            let asker = order[idx];
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(asker, "choice").skill("guhuo");
                    b = b.option("choose:question", "质疑：翻开此牌");
                    Ok(Step::Need(b.escape("不质疑").build()))
                }
                Some(act) => {
                    if act == "choose:question" {
                        questions.push(asker);
                    }
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "question",
                            "owner": owner,
                            "purpose": purpose,
                            "declared": declared,
                            "carrier": carrier,
                            "targets": targets,
                            "questions": questions,
                            "index": idx + 1,
                        }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "penalty" => {
            let carrier = a.data["carrier"].as_str().unwrap_or_default().to_string();
            let declared = a.data["declared"].as_str().unwrap_or_default().to_string();
            let targets: Vec<usize> = a.data["targets"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let questions: Vec<usize> = a.data["questions"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let qidx = a.data["qidx"].as_u64().unwrap_or(0) as usize;
            if qidx >= questions.len() {
                return guhuo_effect(g, owner, &carrier, &declared, &targets, &purpose);
            }
            let q = questions[qidx];
            if g.seats[q].hp <= 0 {
                push(
                    g,
                    "guhuo",
                    owner,
                    json!({
                        "stage": "penalty", "owner": owner, "purpose": purpose,
                        "declared": declared, "carrier": carrier, "targets": targets,
                        "questions": questions, "qidx": qidx + 1,
                    }),
                );
                return Ok(Step::Done);
            }
            let hand = g.hand(q);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(q, "choice").skill("guhuo");
                    for id in &hand {
                        b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                    }
                    b = b.option("hurt", "失去 1 点体力");
                    Ok(Step::Need(b.escape("不选择（失去 1 点体力）").build()))
                }
                Some("hurt") | Some("pass") => {
                    flow::lose_hp(g, q, 1, "guhuo");
                    g.push_log(format!(
                        "蛊惑：{} 质疑为真，获得「缠怨」并失去 1 点体力",
                        g.seats[q].name
                    ));
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "penalty", "owner": owner, "purpose": purpose,
                            "declared": declared, "carrier": carrier, "targets": targets,
                            "questions": questions, "qidx": qidx + 1,
                        }),
                    );
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("蛊惑：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("蛊惑：该牌不在其手牌".into());
                    }
                    g.to_discard(&id, "guhuo")?;
                    g.push_log(format!(
                        "蛊惑：{} 质疑为真，获得「缠怨」并弃置【{}】",
                        g.seats[q].name,
                        label(g, &id)
                    ));
                    push(
                        g,
                        "guhuo",
                        owner,
                        json!({
                            "stage": "penalty", "owner": owner, "purpose": purpose,
                            "declared": declared, "carrier": carrier, "targets": targets,
                            "questions": questions, "qidx": qidx + 1,
                        }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

fn guhuo_resolve(
    g: &mut Game,
    owner: usize,
    carrier: &CardId,
    declared: &str,
    targets: &[usize],
    purpose: &str,
    questions: &[usize],
) -> Result<Step, String> {
    g.reveal(carrier, Some(owner), "guhuo");
    let real = g
        .cards
        .get(carrier)
        .map(|c| c.name == declared)
        .unwrap_or(false);
    if !real {
        // 假：此牌作废，质疑者各摸一张。
        if g.cards.contains_key(carrier) {
            g.to_discard(carrier, "guhuo")?;
        }
        for q in questions {
            let n = g.draw(*q, 1).len();
            g.push_log(format!(
                "蛊惑：展示为假（声明【{declared}】），{} 摸 {n} 张",
                g.seats[*q].name
            ));
        }
        g.push_log(format!(
            "蛊惑：{} 的扣置牌为假，此牌作废",
            g.seats[owner].name
        ));
        if purpose == "respond" || purpose == "dying" {
            g.guhuo_result = Some(None);
        }
        return Ok(Step::Done);
    }
    for q in questions {
        g.seats[*q].flags.insert("chanyuan".to_string());
    }
    g.push_log(format!(
        "蛊惑：{} 的扣置牌为真（声明【{declared}】），质疑者获得「缠怨」",
        g.seats[owner].name
    ));
    push(
        g,
        "guhuo",
        owner,
        json!({
            "stage": "penalty", "owner": owner, "purpose": purpose,
            "declared": declared, "carrier": carrier, "targets": targets,
            "questions": questions, "qidx": 0,
        }),
    );
    Ok(Step::Done)
}

fn guhuo_effect(
    g: &mut Game,
    owner: usize,
    carrier: &CardId,
    declared: &str,
    targets: &[usize],
    purpose: &str,
) -> Result<Step, String> {
    if purpose == "dying" {
        if !g.cards.contains_key(carrier) {
            g.guhuo_result = Some(None);
            return Ok(Step::Done);
        }
        g.to_discard(carrier, "guhuo")?;
        g.push_log(format!(
            "蛊惑：将【{}】当【桃】自救/救援",
            label(g, carrier)
        ));
        g.guhuo_result = Some(Some(carrier.clone()));
        return Ok(Step::Done);
    }
    if purpose == "respond" {
        if !g.cards.contains_key(carrier) {
            g.guhuo_result = Some(None);
            return Ok(Step::Done);
        }
        g.to_discard(carrier, "respond")?;
        g.events.push(crate::kernel::event::Event::card(
            crate::kernel::event::EventKind::CardPlayed,
            Some(owner),
            carrier.clone(),
            Some("response".to_string()),
        ));
        crate::skills::on_response_played(g, None, owner, carrier);
        g.push_log(format!(
            "蛊惑：将【{}】当【{declared}】打出",
            label(g, carrier)
        ));
        g.guhuo_result = Some(Some(carrier.clone()));
        return Ok(Step::Done);
    }
    if !g.cards.contains_key(carrier) {
        return Ok(Step::Done);
    }
    g.push_log(format!(
        "蛊惑：将【{}】当【{declared}】使用",
        label(g, carrier)
    ));
    g.set_virtual(carrier, declared, vec![carrier.clone()]);
    g.use_as(owner, carrier, declared, targets.to_vec(), true, None)?;
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 注册表（本批已完成部分；其余技能在后续提交补齐）
// ---------------------------------------------------------------------------

pub const B13_SKILLS: &[SkillDef] = &[
    LIEGONG, ZHUHAI, QIANXIN, JIANYAN, YAOWU, SHIZHAN, LEIJI, GUIDAO, HUANGTIAN, HUASHEN,
    XINSHENG, XIANGLE, LUANJI, XUEYI, FANGQUAN, RUOYU, SISHU,
];

/// 占位：让 `mod.rs` 的注册区在本批补齐前保持可编译（未注册）。
#[allow(dead_code)]
const _UNUSED: () = ();
