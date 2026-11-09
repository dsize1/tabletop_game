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
    if user == target || !active(g, user, "liegong") || !is_sha(g, card) {
        return Vec::new();
    }
    vec![(user, "liegong")]
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
    if g.counter(&format!("turn.zhuhai_asked:{owner}")) > 0 {
        return Ok(Step::Done);
    }
    if !g.hand(owner).into_iter().any(|c| is_sha(g, &c)) {
        return Ok(Step::Done);
    }
    g.bump(&format!("turn.zhuhai_asked:{owner}"));
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
    play_phase(g, seat)
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
                g.bump("turn.jianyan_used");
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
    play_phase(g, seat) && g.counter("turn.shizhan_used") < 2 && !other_alive(g, seat).is_empty()
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
        Some("pass") => Ok(Step::Done),
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
        Some("pass") => Ok(Step::Done),
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
// 注册表（本批已完成部分；其余技能在后续提交补齐）
// ---------------------------------------------------------------------------

pub const B13_SKILLS: &[SkillDef] = &[
    LIEGONG, ZHUHAI, QIANXIN, JIANYAN, YAOWU, SHIZHAN, LEIJI, GUIDAO, HUANGTIAN,
];

/// 占位：让 `mod.rs` 的注册区在本批补齐前保持可编译（未注册）。
#[allow(dead_code)]
const _UNUSED: () = ();
