//! B14（界标·J2）——按 `align/B14.json` 第 7 段实现。
//!
//! 本文件覆盖：界孙策（制霸/激昂/魂姿）、界夏侯渊（神速/设变）、界卧龙诸葛亮（八阵/藏拙）、
//! 界魏延（狂骨/奇谋）、界小乔（红颜/飘零）、界孙坚（英魂/武烈）、界庞统（涅槃）、界庞德（鞬出）。
//! 待补（下一轮）：火计 / 看破 / 连环 / 天香（需要火攻颜色模式、无懈转化窗口、重铸、
//! 「受到伤害时」受害者侧拦截）。
//!
//! 用户裁定（2026-11-09，58 条疑点）：激昂「同一张牌只触发一次」、藏拙「锦囊不计入手牌数（上限不变）」、
//! 涅槃复原清单（正面/无横置/判定区空/装备空/失去涅槃/三选一）、奇谋可扣到 0 濒死（救起后再摸牌）、
//! 鞬出双分支（基本牌→目标获得【杀】但继续结算；非基本牌→不能抵消且次数最多 +1）、设变每次翻面。

use serde_json::json;

use crate::cards::{ArmorKind, CardKind, Effect, ResponseKind};
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, builder, grant_skill, other_alive, SkillDef};

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, crate::kernel::resolve::Stage::Choose, actor, data);
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
    g.cards.get(id).map(|_| g.def_of(id).tags.sha).unwrap_or(false)
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}
/// 本阶段已放弃（防抖：不再重复提示同一主动技）。
fn cancelled(g: &Game, code: &str) -> bool {
    g.counter(&format!("phase.cancel_{code}")) > 0
}
fn mark_cancelled(g: &mut Game, code: &str) {
    g.bump(&format!("phase.cancel_{code}"));
}
fn is_lord(g: &Game, seat: usize) -> bool {
    g.seats.get(seat).is_some_and(|s| s.flags.contains("lord"))
}

// ---------------------------------------------------------------------------
// 勾子：虚拟防具 / 翻面 / 花色 / 手牌数豁免 / 濒死离场 / 限定技濒死选项
// ---------------------------------------------------------------------------

/// 八阵：装备区无防具时视为装备【八卦阵】。
pub fn virtual_armor(g: &Game, seat: usize) -> ArmorKind {
    if active(g, seat, "bazhen") && g.equip_in(seat, crate::kernel::Slot::Armor).is_none() {
        ArmorKind::Bagua
    } else {
        ArmorKind::None
    }
}

/// 统一翻面钩子：设变（任何方向翻面都可发动）。
pub fn on_flip(g: &mut Game, seat: usize, _face_up: bool) {
    if !active(g, seat, "shebian") || !shebian_movable(g) {
        return;
    }
    push(g, "shebian", seat, json!({}));
}

fn shebian_movable(g: &Game) -> bool {
    (0..g.seats.len()).any(|i| {
        equip_ids(g, i).into_iter().any(|id| {
            g.def_of(&id).slot.is_some_and(|slot| {
                (0..g.seats.len()).any(|to| {
                    to != i && g.seats[to].hp > 0 && g.equip_in(to, slot).is_none()
                })
            })
        })
    })
}

/// 花色单入口（红颜：黑桃 → 红桃）。
pub fn effective_suit(g: &Game, seat: usize, card: &CardId) -> Option<crate::kernel::Suit> {
    let suit = g.cards.get(card)?.suit;
    if suit == crate::kernel::Suit::Spade && active(g, seat, "hongyan") {
        Some(crate::kernel::Suit::Heart)
    } else {
        Some(suit)
    }
}

/// 藏拙：未使用锦囊时，手牌中的锦囊牌不计入手牌数。
pub fn hand_limit_exempt(g: &Game, seat: usize) -> usize {
    if !active(g, seat, "cangzhuo") || g.counter("turn.trick_used") > 0 {
        return 0;
    }
    g.hand(seat)
        .into_iter()
        .filter(|id| g.def_of(id).kind == CardKind::Trick)
        .count()
}

/// 奇谋：濒死救起后补摸。
pub fn on_dying_left(g: &mut Game, seat: usize) {
    let pending = g.counter(&format!("qimou_pending:{seat}"));
    if pending == 0 {
        return;
    }
    g.counters
        .insert(format!("qimou_pending:{seat}"), 0);
    let n = g.draw(seat, pending as usize).len();
    g.push_log(format!("奇谋：救起后摸 {n} 张"));
}

/// 涅槃（限定）：濒死窗口选项。
pub fn dying_limited_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !active(g, seat, "niepan") || g.seats[seat].flags.contains("niepan_used") {
        return Vec::new();
    }
    vec![(
        "niepan".to_string(),
        "涅槃：弃置所有牌、复原武将牌、摸 3 张、回复至 3 点体力，并三选一获得技能".to_string(),
    )]
}

pub fn dying_limited_use(
    g: &mut Game,
    seat: usize,
    victim: usize,
    act: &str,
) -> Result<bool, String> {
    if act != "niepan" {
        return Ok(false);
    }
    if seat != victim {
        return Err("涅槃只能自救".into());
    }
    if !active(g, seat, "niepan") || g.seats[seat].flags.contains("niepan_used") {
        return Ok(false);
    }
    g.seats[seat].flags.insert("niepan_used".to_string());
    // 弃置区域内所有牌（手牌 / 装备 / 判定 / 标记牌）。
    let ids: Vec<CardId> = g.ids_in(|z| {
        matches!(
            z,
            Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) | Zone::Mark(s, _) if *s == seat
        )
    });
    for id in ids {
        g.to_discard(&id, "niepan")?;
    }
    // 复原武将牌：正面朝上、无横置、判定区为空（已弃）、装备区为空（已弃）。
    g.seats[seat].flipped = false;
    g.seats[seat].chained = false;
    g.seats[seat].flags.insert("niepan_used".to_string());
    // 失去「涅槃」（限定技用掉即从技能表移除）。
    g.seats[seat].skills.retain(|s| s != "niepan");
    crate::skills::rebuild_subs(g);
    // 回复至 3 点体力。
    let hp = 3.min(g.seats[seat].max_hp);
    g.seats[seat].hp = hp;
    // 摸 3 张。
    let n = g.draw(seat, 3).len();
    g.presentation.push(
        Presentation::new(
            "niepan",
            POutcome::Activated {
                skill: "涅槃".to_string(),
            },
        )
        .actor(seat)
        .fx("niepan"),
    );
    g.push_log(format!(
        "涅槃：{} 弃置所有牌、复原武将牌、摸 {n} 张、回复至 {hp} 点体力",
        g.seats[seat].name
    ));
    // 三选一获得技能。
    push(g, "niepan", seat, json!({ "stage": "pick" }));
    Ok(true)
}

/// 武烈「烈」：抵消该次全部伤害（每枚挡一次，无论伤害点数）。
pub fn lie_prevent(g: &mut Game, victim: usize) -> bool {
    let cur = g.seats.get(victim).and_then(|s| s.counters.get("lie")).copied().unwrap_or(0);
    if cur == 0 {
        return false;
    }
    g.seats[victim].counters.insert("lie".to_string(), cur - 1);
    g.push_log(format!(
        "武烈：{} 移除 1 枚「烈」抵消本次伤害（剩 {}）",
        g.seats[victim].name,
        cur - 1
    ));
    true
}

// ---------------------------------------------------------------------------
// 制霸（界孙策，主公技）
// ---------------------------------------------------------------------------

pub const ZHIBA: SkillDef = SkillDef {
    code: "zhiba",
    name: "制霸",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhiba),
    handler: zhiba,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn wu_targets(g: &Game, owner: usize) -> Vec<usize> {
    other_alive(g, owner)
        .into_iter()
        .filter(|i| g.seats[*i].kingdom == "wu" && !g.hand(*i).is_empty())
        .collect()
}

fn can_zhiba(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhiba")
        && play_phase(g, seat)
        && !cancelled(g, "zhiba")
        && g.counter("turn.zhiba_used") == 0
        && !wu_targets(g, seat).is_empty()
}

fn zhiba(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("after") {
        let Some(res) = g.pindian_result.take() else {
            return Ok(Step::Done);
        };
        // 挑战者没赢（含平局）→ 孙策获得两张拼点牌。
        if res.winner != Some(res.target) {
            let mut got = 0;
            for id in [&res.a, &res.b] {
                if matches!(g.zone_of(id), Some(Zone::Discard)) {
                    g.gain(id, owner)?;
                    got += 1;
                }
            }
            g.push_log(format!("制霸：挑战者没赢，{} 获得 {got} 张拼点牌", g.seats[owner].name));
        }
        return Ok(Step::Done);
    }
    let cands = wu_targets(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("zhiba");
            for i in &cands {
                b = b.option(format!("target:{i}"), format!("与 {} 拼点", g.seats[*i].name));
            }
            Ok(Step::Need(b.escape("取消制霸").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "zhiba");
            Ok(Step::Done)
        }
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("制霸：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("制霸：目标必须是手牌非空的吴势力角色".into());
            }
            g.bump("turn.zhiba_used");
            g.push_log(format!(
                "制霸：{} 与 {} 拼点",
                g.seats[owner].name, g.seats[t].name
            ));
            push(g, "zhiba", owner, json!({ "stage": "after" }));
            crate::kernel::pindian::start(g, owner, t, "zhiba");
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 激昂（界孙策）
// ---------------------------------------------------------------------------

pub const JIANG: SkillDef = SkillDef {
    code: "jiang",
    name: "激昂",
    triggers: &[
        EventKind::CardUsed,
        EventKind::TargetChosen,
        EventKind::CardDiscarded,
    ],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn is_jiang_card(g: &Game, id: &CardId) -> bool {
    let def = g.def_of(id);
    def.tags.duel || (def.tags.sha && g.is_red(id))
}

fn jiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default().to_string();
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if !g.cards.contains_key(&card) || !is_jiang_card(g, &card) {
        return Ok(Step::Done);
    }
    if ev.starts_with("CardDiscarded") {
        // ② 每回合首次因弃置进入弃牌堆 → 可失去 1 点体力获得之。
        if g.counter("turn.jiang_jie_used") > 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiang");
                b = b.option(
                    "opt1",
                    format!("失去 1 点体力获得【{}】", label(g, &card)),
                );
                Ok(Step::Need(b.escape("放弃").build()))
            }
            Some("pass") => {
                g.bump("turn.jiang_jie_used");
                Ok(Step::Done)
            }
            Some(_) => {
                g.bump("turn.jiang_jie_used");
                flow::lose_hp(g, owner, 1, "jiang");
                if matches!(g.zone_of(&card), Some(Zone::Discard)) {
                    g.gain(&card, owner)?;
                }
                g.push_log(format!(
                    "激昂：失去 1 点体力获得【{}】",
                    label(g, &card)
                ));
                Ok(Step::Done)
            }
        }
    } else {
        // ① 使用/成为【决斗】或红色【杀】目标后摸 1（同一张牌只触发一次）。
        let key = format!("turn.jiang_draw:{card}");
        if g.counter(&key) > 0 {
            return Ok(Step::Done);
        }
        // 只认「自己使用」或「自己成为目标」。
        let involved = a.data["seat"].as_u64().map(|n| n as usize) == Some(owner)
            || a.data["cause"]
                .as_str()
                .is_some_and(|c| c.split(',').any(|x| x == owner.to_string()));
        if !involved {
            return Ok(Step::Done);
        }
        g.bump(&key);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiang");
                b = b.option("opt1", "激昂：摸一张牌");
                Ok(Step::Need(b.escape("不摸牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let n = g.draw(owner, 1).len();
                g.push_log(format!("激昂：摸 {n} 张"));
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 魂姿（界孙策，觉醒）
// ---------------------------------------------------------------------------

pub const HUNZI: SkillDef = SkillDef {
    code: "hunzi",
    name: "魂姿",
    triggers: &[EventKind::PhaseStart, EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: hunzi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn hunzi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default().to_string();
    if ev.starts_with("PhaseStart") && g.phase == Phase::Prepare {
        if g.seats[owner].flags.contains("awaken_hunzi") || g.seats[owner].hp != 1 {
            return Ok(Step::Done);
        }
        g.seats[owner].flags.insert("awaken_hunzi".to_string());
        g.seats[owner].flags.insert("hunzi_end".to_string());
        g.seats[owner].max_hp = (g.seats[owner].max_hp - 1).max(1);
        g.seats[owner].hp = g.seats[owner].hp.min(g.seats[owner].max_hp);
        grant_skill(g, owner, "yingzi");
        grant_skill(g, owner, "yinghun");
        g.presentation.push(
            Presentation::new(
                "hunzi",
                POutcome::Activated {
                    skill: "魂姿".to_string(),
                },
            )
            .actor(owner)
            .fx("hunzi"),
        );
        g.push_log(format!(
            "魂姿：{} 觉醒，体力上限 -1（{}），获得「英姿」「英魂」",
            g.seats[owner].name, g.seats[owner].max_hp
        ));
        return Ok(Step::Done);
    }
    // 本回合结束阶段：摸 2 或回复 1。
    if !g.seats[owner].flags.contains("hunzi_end") {
        return Ok(Step::Done);
    }
    if g.phase != Phase::End {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("hunzi");
            b = b.option("opt1", "魂姿：摸两张牌");
            b = b.option("opt2", "魂姿：回复 1 点体力");
            Ok(Step::Need(b.escape("放弃").build()))
        }
        Some("pass") => {
            g.seats[owner].flags.remove("hunzi_end");
            Ok(Step::Done)
        }
        Some(act) => {
            g.seats[owner].flags.remove("hunzi_end");
            if act == "opt2" {
                flow::heal(g, owner, 1, "hunzi");
                g.push_log("魂姿：回复 1 点体力".to_string());
            } else {
                let n = g.draw(owner, 2).len();
                g.push_log(format!("魂姿：摸 {n} 张"));
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 神速（界夏侯渊）：准备阶段至多三项
// ---------------------------------------------------------------------------

pub const SHENSU: SkillDef = SkillDef {
    code: "shensu",
    name: "神速",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 0,
    can_active: None,
    handler: shensu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn shensu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("pick_equip") {
        let opts: Vec<CardId> = hand_and_equip(g, owner)
            .into_iter()
            .filter(|id| g.def_of(id).slot.is_some())
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("shensu");
                for id in &opts {
                    b = b.option(format!("pick:{id}"), format!("弃置装备【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不发动").build()))
            }
            Some("pass") => {
                push(g, "shensu", owner, json!({}));
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("神速：未知选项 {act}"))?
                    .to_string();
                if !opts.contains(&id) {
                    return Err("神速：只能弃置装备牌".into());
                }
                g.to_discard(&id, "shensu")?;
                g.counters.insert("turn.skip_play".to_string(), 1);
                shensu_sha(g, owner)?;
                push(g, "shensu", owner, json!({}));
                Ok(Step::Done)
            }
        };
    }
    if a.data["stage"].as_str() == Some("sha_target") {
        let cands = other_alive(g, owner);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("shensu");
                for t in &cands {
                    b = b.option(format!("target:{t}"), format!("对 {} 使用【杀】", g.seats[*t].name));
                }
                Ok(Step::Need(b.escape("不指定目标").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("神速：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("神速：目标不合法".into());
                }
                g.use_virtual(owner, "杀", vec![t])?;
                Ok(Step::Done)
            }
        };
    }
    if g.phase != Phase::Prepare {
        return Ok(Step::Done);
    }
    let mut opts: Vec<(String, String)> = Vec::new();
    if g.counter("turn.shensu_skip_judge") == 0 {
        opts.push(("opt1".into(), "神速①：跳过判定与摸牌阶段".into()));
    }
    if g.counter("turn.shensu_skip_play") == 0
        && hand_and_equip(g, owner)
            .into_iter()
            .any(|id| g.def_of(&id).slot.is_some())
    {
        opts.push(("opt2".into(), "神速②：弃置一张装备牌并跳过出牌阶段".into()));
    }
    if g.counter("turn.shensu_skip_discard") == 0 {
        opts.push(("opt3".into(), "神速③：翻面并跳过弃牌阶段".into()));
    }
    if opts.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("shensu");
            for (id, label) in &opts {
                b = b.option(id.clone(), label.clone());
            }
            Ok(Step::Need(b.escape("不发动神速").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            match act {
                "opt1" => {
                    g.bump("turn.shensu_skip_judge");
                    g.counters.insert("turn.skip_judge".to_string(), 1);
                    g.counters.insert("turn.skip_draw".to_string(), 1);
                    // 先压「继续选择」帧，再压【杀】目标帧（LIFO：先结算【杀】）。
                    push(g, "shensu", owner, json!({}));
                    shensu_sha(g, owner)?;
                    return Ok(Step::Done);
                }
                "opt2" => {
                    g.bump("turn.shensu_skip_play");
                    push(g, "shensu", owner, json!({ "stage": "pick_equip" }));
                    return Ok(Step::Done);
                }
                _ => {
                    g.bump("turn.shensu_skip_discard");
                    g.counters.insert("turn.skip_discard".to_string(), 1);
                    g.flip(owner);
                    push(g, "shensu", owner, json!({}));
                    shensu_sha(g, owner)?;
                    return Ok(Step::Done);
                }
            }
            // 继续询问（至多三项）。
            push(g, "shensu", owner, json!({}));
            Ok(Step::Done)
        }
    }
}

fn shensu_sha(g: &mut Game, owner: usize) -> Result<(), String> {
    g.push_log(format!("神速：{} 视为使用一张无距离限制的【杀】", g.seats[owner].name));
    // 无距离限制：虚拟【杀】不再选目标限制；由技能的 use_virtual 直接结算全部敌人？
    // 这里逐个询问目标（单一目标），用 skill 的 pending 选择。
    push(g, "shensu", owner, json!({ "stage": "sha_target" }));
    Ok(())
}

// ---------------------------------------------------------------------------
// 设变（界夏侯渊）
// ---------------------------------------------------------------------------

pub const SHEBIAN: SkillDef = SkillDef {
    code: "shebian",
    name: "设变",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: shebian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn shebian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default().to_string();
    if let Some(rest) = a.data["stage"].as_str() {
        let _ = ev;
        let id = a.data["card"].as_str().unwrap_or_default().to_string();
        match g.answer.take().as_deref() {
            None => {
                let Some(slot) = g.def_of(&id).slot else {
                    return Ok(Step::Done);
                };
                let mut b = builder(owner, "target").skill("shebian");
                for t in 0..g.seats.len() {
                    if g.seats[t].hp > 0
                        && g.equip_in(t, slot).is_none()
                        && g.zone_of(&id) != Some(&Zone::Equip(t, slot))
                    {
                        b = b.option(
                            format!("target:{t}"),
                            format!("移到 {} 的{}区", g.seats[t].name, slot.label()),
                        );
                    }
                }
                Ok(Step::Need(b.escape("放弃设变").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("设变：未知选项 {act}"))?;
                let Some(slot) = g.def_of(&id).slot else {
                    return Ok(Step::Done);
                };
                g.move_equip(&id, t, slot)?;
                g.push_log(format!(
                    "设变：将【{}】移到 {}",
                    label(g, &id),
                    g.seats[t].name
                ));
                Ok(Step::Done)
            }
        }
    } else {
        // 翻面钩子入口：先选装备。
        let mut opts: Vec<CardId> = Vec::new();
        for i in 0..g.seats.len() {
            for id in equip_ids(g, i) {
                let Some(slot) = g.def_of(&id).slot else {
                    continue;
                };
                if (0..g.seats.len()).any(|t| {
                    t != i && g.seats[t].hp > 0 && g.equip_in(t, slot).is_none()
                }) {
                    opts.push(id);
                }
            }
        }
        match g.answer.take().as_deref() {
            None => {
                if opts.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = builder(owner, "choice").skill("shebian");
                for id in &opts {
                    b = b.option(format!("pick:{id}"), format!("移动【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不发动设变").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("设变：未知选项 {act}"))?
                    .to_string();
                if !opts.contains(&id) {
                    return Err("设变：该装备不可移动".into());
                }
                push(
                    g,
                    "shebian",
                    owner,
                    json!({ "stage": "target", "card": id }),
                );
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 八阵 / 藏拙（界卧龙诸葛亮）
// ---------------------------------------------------------------------------

pub const BAZHEN: SkillDef = SkillDef {
    code: "bazhen",
    name: "八阵",
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

pub const CANGZHUO: SkillDef = SkillDef {
    code: "cangzhuo",
    name: "藏拙",
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

// ---------------------------------------------------------------------------
// 红颜（界小乔）
// ---------------------------------------------------------------------------

pub const HONGYAN: SkillDef = SkillDef {
    code: "hongyan",
    name: "红颜",
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

pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if !active(g, seat, "hongyan") {
        return None;
    }
    let has_heart_equip = equip_ids(g, seat)
        .into_iter()
        .any(|id| effective_suit(g, seat, &id) == Some(crate::kernel::Suit::Heart));
    if has_heart_equip {
        Some(g.seats[seat].max_hp.max(0) as usize)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 狂骨（界魏延）
// ---------------------------------------------------------------------------

pub const KUANGGU: SkillDef = SkillDef {
    code: "kuanggu",
    name: "狂骨",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: kuanggu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn kuanggu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let Some(src) = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok()) else {
        return Ok(Step::Done);
    };
    if src != owner {
        return Ok(Step::Done);
    }
    let Some(victim) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    if victim == owner || g.seats[victim].hp <= 0 {
        return Ok(Step::Done);
    }
    if g.distance(owner, victim) > 1 {
        return Ok(Step::Done);
    }
    let amount = a.data["amount"].as_i64().unwrap_or(1).max(1);
    let left = a
        .data["left"]
        .as_u64()
        .unwrap_or(amount as u64) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("kuanggu");
            b = b.option("opt1", "狂骨：回复 1 点体力");
            b = b.option("opt2", "狂骨：摸一张牌");
            Ok(Step::Need(b.escape("放弃").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            if act == "opt1" {
                flow::heal(g, owner, 1, "kuanggu");
                g.push_log("狂骨：回复 1 点体力".to_string());
            } else {
                let n = g.draw(owner, 1).len();
                g.push_log(format!("狂骨：摸 {n} 张"));
            }
            if left > 1 {
                push(
                    g,
                    "kuanggu",
                    owner,
                    json!({ "cause": owner.to_string(), "seat": victim, "left": left - 1 }),
                );
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 奇谋（界魏延，限定）
// ---------------------------------------------------------------------------

pub const QIMOU: SkillDef = SkillDef {
    code: "qimou",
    name: "奇谋",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_qimou),
    handler: qimou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_qimou(g: &Game, seat: usize) -> bool {
    active(g, seat, "qimou")
        && play_phase(g, seat)
        && !cancelled(g, "qimou")
        && !g.seats[seat].flags.contains("qimou_used")
        && g.seats[seat].hp > 0
}

fn qimou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let hp = g.seats[owner].hp.max(0) as usize;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("qimou");
            for x in 1..=hp {
                b = b.option(
                    format!("x:{x}"),
                    format!("失去 {x} 点体力，摸 {x} 张并本回合距离-{x}、杀次数+{x}"),
                );
            }
            Ok(Step::Need(b.escape("取消奇谋").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "qimou");
            Ok(Step::Done)
        }
        Some(act) => {
            let x = act
                .strip_prefix("x:")
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("奇谋：未知选项 {act}"))?;
            if x == 0 || x > hp {
                return Err("奇谋：失去体力数不合法".into());
            }
            g.seats[owner].flags.insert("qimou_used".to_string());
            g.bump("turn.qimou");
            g.bump("turn.qimou_x");
            g.counters.insert("turn.qimou".to_string(), x as u32);
            // 本回合距离/次数加成（在读取处按 turn.qimou 计算）。
            g.push_log(format!(
                "奇谋：{} 失去 {x} 点体力，本回合距离 -{x}、【杀】次数 +{x}",
                g.seats[owner].name
            ));
            flow::lose_hp(g, owner, x as i32, "qimou");
            if g.seats[owner].hp > 0 {
                let n = g.draw(owner, x).len();
                g.push_log(format!("奇谋：摸 {n} 张"));
            } else {
                // 濒死：救起后再摸（`on_dying_left` 消费）。
                g.counters
                    .insert(format!("qimou_pending:{owner}"), x as u32);
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 飘零（界小乔）
// ---------------------------------------------------------------------------

pub const PIAOLING: SkillDef = SkillDef {
    code: "piaoling",
    name: "飘零",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: piaoling,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn piaoling(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("ask").to_string();
    match stage.as_str() {
        "ask" => {
            if g.phase != Phase::End {
                return Ok(Step::Done);
            }
            if g.answer.is_none() {
                if g.counter("phase.piaoling_asked") > 0 {
                    return Ok(Step::Done);
                }
                g.bump("phase.piaoling_asked");
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("piaoling");
                    b = b.option("opt1", "飘零：判定");
                    Ok(Step::Need(b.escape("不发动飘零").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(_) => {
                    push(g, "piaoling", owner, json!({ "stage": "after" }));
                    crate::kernel::card_play::start_quick_judge(g, owner, "piaoling");
                    Ok(Step::Done)
                }
            }
        }
        "after" => {
            let Some(card) = g.judge_result.take() else {
                return Ok(Step::Done);
            };
            let heart = effective_suit(g, owner, &card) == Some(crate::kernel::Suit::Heart);
            if !heart {
                if g.cards.contains_key(&card) {
                    g.to_discard(&card, "piaoling")?;
                }
                g.push_log("飘零：判定非红桃，未生效".to_string());
                return Ok(Step::Done);
            }
            push(
                g,
                "piaoling",
                owner,
                json!({ "stage": "place", "card": card }),
            );
            Ok(Step::Done)
        }
        "discard" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("piaoling");
                for id in g.hand(owner) {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("不弃置").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("飘零：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&id) {
                    return Err("飘零：该牌不在手牌".into());
                }
                g.to_discard(&id, "piaoling")?;
                Ok(Step::Done)
            }
        },
        _ => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("piaoling");
                    b = b.option("top", "置于牌堆顶");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("交给 {}", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不放置").build()))
                }
                Some("pass") => {
                    if g.cards.contains_key(&card) {
                        g.to_discard(&card, "piaoling")?;
                    }
                    Ok(Step::Done)
                }
                Some("top") => {
                    if matches!(g.zone_of(&card), Some(Zone::Processing) | Some(Zone::Frame(_))) {
                        g.put_on_top(&card)?;
                    }
                    g.push_log("飘零：判定牌置于牌堆顶".to_string());
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("飘零：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("飘零：目标不合法".into());
                    }
                    g.gain(&card, t)?;
                    g.push_log(format!(
                        "飘零：将判定牌【{}】交给 {}",
                        label(g, &card),
                        g.seats[t].name
                    ));
                    if t == owner {
                        let hand = g.hand(owner);
                        if !hand.is_empty() {
                            push(
                                g,
                                "piaoling",
                                owner,
                                json!({ "stage": "discard" }),
                            );
                        }
                    }
                    Ok(Step::Done)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 英魂（界孙坚）
// ---------------------------------------------------------------------------

pub const YINGHUN: SkillDef = SkillDef {
    code: "yinghun",
    name: "英魂",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 0,
    can_active: None,
    handler: yinghun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yinghun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::Prepare {
        return Ok(Step::Done);
    }
    let x = (g.seats[owner].max_hp - g.seats[owner].hp).max(0) as usize;
    if x == 0 {
        return Ok(Step::Done);
    }
    let cands = other_alive(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if a.data["stage"].as_str() == Some("target") {
        let mode = a.data["mode"].as_u64().unwrap_or(1);
        let x = a.data["x"].as_u64().unwrap_or(0) as usize;
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("yinghun");
                for t in &cands {
                    b = b.option(format!("target:{t}"), format!("令 {} 受到英魂", g.seats[*t].name));
                }
                Ok(Step::Need(b.escape("不发动英魂").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or_else(|| format!("英魂：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("英魂：目标不合法".into());
                }
                let (draw, discard) = if mode == 2 {
                    (1, x)
                } else {
                    (x, 1)
                };
                let n = g.draw(t, draw).len();
                g.push_log(format!(
                    "英魂：{} 摸 {n} 张，随后弃 {discard} 张",
                    g.seats[t].name
                ));
                let need = discard.min(g.hand(t).len());
                for _ in 0..need {
                    crate::turn::start_discard(g, t);
                }
                Ok(Step::Done)
            }
        };
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("yinghun");
            b = b.option("opt1", format!("令一名其他角色摸 {x} 张并弃 1 张"));
            b = b.option("opt2", "令一名其他角色摸 1 张并弃 X 张".to_string());
            Ok(Step::Need(b.escape("不发动英魂").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let mode = if act == "opt2" { 2 } else { 1 };
            push(
                g,
                "yinghun",
                owner,
                json!({ "stage": "target", "mode": mode, "x": x }),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 武烈（界孙坚，限定）
// ---------------------------------------------------------------------------

pub const WULIE: SkillDef = SkillDef {
    code: "wulie",
    name: "武烈",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: wulie,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn wulie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("target") {
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        if left == 0 {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len())
            .filter(|i| *i != owner && g.seats[*i].hp > 0)
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("wulie");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("令 {} 获得「烈」", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("放弃剩余").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or_else(|| format!("武烈：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("武烈：目标不合法".into());
                }
                let cur = g.seats[t].counters.get("lie").copied().unwrap_or(0);
                g.seats[t].counters.insert("lie".to_string(), cur + 1);
                g.push_log(format!("武烈：{} 获得 1 枚「烈」", g.seats[t].name));
                if left > 1 {
                    push(g, "wulie", owner, json!({ "stage": "target", "left": left - 1 }));
                }
                Ok(Step::Done)
            }
        };
    }
    if g.phase != Phase::End || g.seats[owner].flags.contains("wulie_used") {
        return Ok(Step::Done);
    }
    let hp = g.seats[owner].hp.max(0) as usize;
    let others = other_alive(g, owner).len();
    if hp == 0 || others == 0 {
        return Ok(Step::Done);
    }
    let cap = hp.min(others);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("wulie");
            for x in 1..=cap {
                b = b.option(
                    format!("x:{x}"),
                    format!("失去 {x} 点体力，令 {x} 名其他角色获得「烈」"),
                );
            }
            Ok(Step::Need(b.escape("不发动武烈").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let x = act
                .strip_prefix("x:")
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("武烈：未知选项 {act}"))?;
            if x == 0 || x > cap {
                return Err("武烈：数值不合法".into());
            }
            g.seats[owner].flags.insert("wulie_used".to_string());
            flow::lose_hp(g, owner, x as i32, "wulie");
            push(
                g,
                "wulie",
                owner,
                json!({ "stage": "target", "left": x }),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 鞬出（界庞德）
// ---------------------------------------------------------------------------

pub const JIANDU: SkillDef = SkillDef {
    code: "jiandu",
    name: "鞬出",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiandu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub fn sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let Some(card) = card else {
        return Vec::new();
    };
    if user == target || !active(g, user, "jiandu") || !is_sha(g, card) {
        return Vec::new();
    }
    if hand_and_equip(g, target).is_empty() {
        return Vec::new();
    }
    vec![(user, "jiandu")]
}

fn jiandu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let sha = a.data["card"].as_str().unwrap_or_default().to_string();
    let stage = a.data["stage"].as_str().unwrap_or("ask").to_string();
    if stage == "pick" {
        let opts = hand_and_equip(g, target);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiandu");
                if !g.hand(target).is_empty() {
                    b = b.option("pick:hand", "手牌：随机一张");
                }
                for id in &opts {
                    if g.def_of(id).slot.is_some() {
                        b = b.option(
                            format!("pick:{id}"),
                            format!("装备区：【{}】", label(g, id)),
                        );
                    }
                }
                Ok(Step::Need(b.escape("不发动鞬出").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let picked = if act == "pick:hand" {
                    if g.hand(target).is_empty() {
                        return Err("鞬出：目标没有手牌".into());
                    }
                    let n = g.hand(target).len();
                    let idx = g.rand_below(n);
                    g.hand(target)[idx].clone()
                } else {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("鞬出：未知选项 {act}"))?
                        .to_string();
                    if !g
                        .zone_of(&id)
                        .is_some_and(|z| matches!(z, Zone::Equip(s, _) if *s == target))
                    {
                        return Err("鞬出：该装备不在其装备区".into());
                    }
                    id
                };
                let basic = g.def_of(&picked).kind == CardKind::Basic;
                let lab = label(g, &picked);
                if basic {
                    // 基本牌：目标获得此【杀】，且本次【杀】继续结算。
                    g.to_discard(&picked, "jiandu")?;
                    g.push_log(format!(
                        "鞬出：弃置 {} 的【{lab}】（基本牌），其获得此【杀】",
                        g.seats[target].name
                    ));
                    if g.cards.contains_key(&sha) {
                        g.gain(&sha, target)?;
                    }
                } else {
                    g.to_discard(&picked, "jiandu")?;
                    g.sha_intercept = Some(ShaIntercept::Unresponsive(owner));
                    if g.counter("turn.jiandu_bonus") == 0 {
                        g.bump("turn.jiandu_bonus");
                    }
                    g.push_log(format!(
                        "鞬出：弃置 {} 的【{lab}】（非基本牌），其不能抵消此【杀】",
                        g.seats[target].name
                    ));
                }
                Ok(Step::Done)
            }
        };
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("jiandu");
            b = b.option(
                "opt1",
                format!("鞬出：弃置 {} 一张牌", g.seats[target].name),
            );
            Ok(Step::Need(b.escape("不发动鞬出").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            push(
                g,
                "jiandu",
                owner,
                json!({ "stage": "pick", "target": target, "card": sha }),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 注册表（本批已完成部分）
// ---------------------------------------------------------------------------

pub const B14_SKILLS: &[SkillDef] = &[
    ZHIBA, JIANG, HUNZI, SHENSU, SHEBIAN, BAZHEN, CANGZHUO, HONGYAN, KUANGGU, QIMOU,
    PIAOLING, YINGHUN, WULIE, JIANDU, HUOJI, KANPO, LIANHUAN, TIANXIANG, NIEPAN,
];

/// 涅槃（限定）：濒死窗口选项（实现在 `dying_limited_options/use`）。
pub const NIEPAN: SkillDef = SkillDef {
    code: "niepan",
    name: "涅槃",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: niepan_pick,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn niepan_pick(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("niepan");
            b = b.option("opt1", "获得「八阵」");
            b = b.option("opt2", "获得「火计」");
            b = b.option("opt3", "获得「看破」");
            Ok(Step::Need(b.escape("暂不获得技能").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let code = match act {
                "opt1" => "bazhen",
                "opt2" => "huoji",
                _ => "kanpo",
            };
            grant_skill(g, owner, code);
            g.push_log(format!(
                "涅槃：{} 获得「{}」",
                g.seats[owner].name,
                crate::skills::def(code).map(|d| d.name).unwrap_or(code)
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 火计 / 看破 / 连环（界卧龙诸葛亮 / 界庞统）：转化技
// ---------------------------------------------------------------------------

pub const HUOJI: SkillDef = SkillDef {
    code: "huoji",
    name: "火计",
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

pub const KANPO: SkillDef = SkillDef {
    code: "kanpo",
    name: "看破",
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

pub const LIANHUAN: SkillDef = SkillDef {
    code: "lianhuan",
    name: "连环",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_lianhuan),
    handler: lianhuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 被动转化：火计（红色牌当【火攻】）、看破（黑色牌当【无懈可击】）、
/// 连环（梅花牌当【铁索连环】）。承载牌 = 手牌 + 装备区。
pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    let mut out = Vec::new();
    let pool = hand_and_equip(g, seat);
    if active(g, seat, "huoji") {
        for id in &pool {
            let red = effective_suit(g, seat, id)
                .map(|s| s.is_red())
                .unwrap_or(false);
            if red {
                out.push(super::Conversion {
                    skill: "huoji",
                    from: id.clone(),
                    to: "火攻",
                    usable: true,
                    playable: false,
                });
            }
        }
    }
    if active(g, seat, "kanpo") {
        for id in &pool {
            let black = !effective_suit(g, seat, id)
                .map(|s| s.is_red())
                .unwrap_or(true);
            if black {
                out.push(super::Conversion {
                    skill: "kanpo",
                    from: id.clone(),
                    to: "无懈可击",
                    usable: false,
                    playable: true,
                });
            }
        }
    }
    if active(g, seat, "lianhuan") {
        for id in &pool {
            if effective_suit(g, seat, id) == Some(crate::kernel::Suit::Club) {
                out.push(super::Conversion {
                    skill: "lianhuan",
                    from: id.clone(),
                    to: "铁索连环",
                    usable: true,
                    playable: false,
                });
            }
        }
    }
    out
}

/// 界火计：内核【火攻】改为「随机展示 + 同颜色弃牌（手牌/装备区）」。
pub fn fire_attack_color_mode(g: &Game, source: usize) -> bool {
    active(g, source, "huoji")
}

/// 界看破：你的【无懈可击】不能被响应。
pub fn wuxie_uncounterable(g: &Game, seat: usize) -> bool {
    active(g, seat, "kanpo")
}

/// 连环：使用【铁索连环】（含转化）可多指定一个目标。
pub fn extra_targets(g: &Game, seat: usize, card: Option<&CardId>) -> usize {
    let Some(card) = card else { return 0 };
    if !active(g, seat, "lianhuan") {
        return 0;
    }
    if g.def_of(card).effect == Effect::IronChain {
        1
    } else {
        0
    }
}

fn can_lianhuan(g: &Game, seat: usize) -> bool {
    active(g, seat, "lianhuan")
        && play_phase(g, seat)
        && !cancelled(g, "lianhuan")
        && hand_and_equip(g, seat)
            .into_iter()
            .any(|id| effective_suit(g, seat, &id) == Some(crate::kernel::Suit::Club))
}

/// 连环的「重铸」入口（梅花牌：弃置并摸 1，算「使用牌」）。
fn lianhuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let pool: Vec<CardId> = hand_and_equip(g, owner)
        .into_iter()
        .filter(|id| effective_suit(g, owner, id) == Some(crate::kernel::Suit::Club))
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("lianhuan");
            for id in &pool {
                b = b.option(format!("pick:{id}"), format!("重铸【{}】", label(g, id)));
            }
            Ok(Step::Need(b.escape("取消连环").build()))
        }
        Some("pass") => {
            mark_cancelled(g, "lianhuan");
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("连环：未知选项 {act}"))?
                .to_string();
            if !pool.contains(&id) {
                return Err("连环：只能重铸梅花牌".into());
            }
            g.to_discard(&id, "lianhuan")?;
            // 重铸算「使用牌」（用户 2026-11-09 裁定）。
            g.emit(crate::kernel::event::Event::card(
                EventKind::CardUsed,
                Some(owner),
                id.clone(),
                Some("recast".to_string()),
            ));
            let n = g.draw(owner, 1).len();
            g.push_log(format!("连环：重铸【{}】，摸 {n} 张", label(g, &id)));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 天香（界小乔）：受到伤害时弃红桃牌防止并转移
// ---------------------------------------------------------------------------

pub const TIANXIANG: SkillDef = SkillDef {
    code: "tianxiang",
    name: "天香",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: tianxiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn hearts(g: &Game, seat: usize) -> Vec<CardId> {
    hand_and_equip(g, seat)
        .into_iter()
        .filter(|id| effective_suit(g, seat, id) == Some(crate::kernel::Suit::Heart))
        .collect()
}

/// 「受到伤害时」受害者侧拦截（返回 true = 已压帧，本次伤害由天香接管）。
pub fn damage_before_victim(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) -> bool {
    if !active(g, victim, "tianxiang") || hearts(g, victim).is_empty() {
        return false;
    }
    push(
        g,
        "tianxiang",
        victim,
        json!({
            "stage": "ask",
            "victim": victim,
            "src": source,
            "amount": amount,
            "card": card,
        }),
    );
    true
}

fn take_damage(
    g: &mut Game,
    src: Option<usize>,
    victim: usize,
    amount: i32,
    card: &Option<CardId>,
) {
    flow::damage_now_typed(
        g,
        src,
        victim,
        amount,
        card.as_ref(),
        crate::cards::DamageKind::Normal,
        false,
    );
}

fn tianxiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("ask").to_string();
    let src: Option<usize> = a.data["src"].as_u64().map(|n| n as usize);
    let amount = a.data["amount"].as_i64().unwrap_or(1) as i32;
    let card = a.data["card"].as_str().map(|s| s.to_string());
    match stage.as_str() {
        "ask" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(victim, "choice").skill("tianxiang");
                b = b.option("opt1", "天香：弃置一张红桃牌，防止此伤害并转移");
                Ok(Step::Need(b.escape("承受伤害").build()))
            }
            Some("pass") => {
                take_damage(g, src, victim, amount, &card);
                Ok(Step::Done)
            }
            Some(_) => {
                push(
                    g,
                    "tianxiang",
                    victim,
                    json!({ "stage": "pick", "victim": victim, "src": src, "amount": amount, "card": card }),
                );
                Ok(Step::Done)
            }
        },
        "pick" => {
            let opts = hearts(g, victim);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(victim, "choice").skill("tianxiang");
                    for id in &opts {
                        b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("承受伤害").build()))
                }
                Some("pass") => {
                    take_damage(g, src, victim, amount, &card);
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("天香：未知选项 {act}"))?
                        .to_string();
                    if !opts.contains(&id) {
                        return Err("天香：只能弃置红桃牌".into());
                    }
                    push(
                        g,
                        "tianxiang",
                        victim,
                        json!({ "stage": "target", "victim": victim, "src": src, "discarded": id }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "target" => {
            let discarded = a.data["discarded"].as_str().unwrap_or_default().to_string();
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != victim && g.seats[*i].hp > 0)
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(victim, "target").skill("tianxiang");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("转移给 {}", g.seats[*t].name));
                    }
                    Ok(Step::Need(b.escape("承受伤害").build()))
                }
                Some("pass") => {
                    take_damage(g, src, victim, amount, &card);
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("天香：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("天香：目标不合法".into());
                    }
                    push(
                        g,
                        "tianxiang",
                        victim,
                        json!({ "stage": "mode", "victim": victim, "src": src, "discarded": discarded, "target": t }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        _ => {
            let discarded = a.data["discarded"].as_str().unwrap_or_default().to_string();
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(victim, "choice").skill("tianxiang");
                    b = b.option("mode:damage", "令其受到伤害来源的 1 点伤害并摸牌");
                    b = b.option("mode:lose", "令其失去 1 点体力并获得你弃置的牌");
                    Ok(Step::Need(b.escape("承受伤害").build()))
                }
                Some("pass") => {
                    take_damage(g, src, victim, amount, &card);
                    Ok(Step::Done)
                }
                Some(act) => {
                    if g.cards.contains_key(&discarded) {
                        g.to_discard(&discarded, "tianxiang")?;
                    }
                    if act == "mode:damage" {
                        flow::damage_now_typed(
                            g,
                            src,
                            target,
                            1,
                            None,
                            crate::cards::DamageKind::Normal,
                            false,
                        );
                        let lost = (g.seats[target].max_hp - g.seats[target].hp)
                            .max(0)
                            .min(5) as usize;
                        let n = if g.seats[target].hp > 0 {
                            g.draw(target, lost).len()
                        } else {
                            0
                        };
                        g.push_log(format!(
                            "天香：{} 受到 1 点伤害并摸 {n} 张",
                            g.seats[target].name
                        ));
                    } else {
                        flow::lose_hp(g, target, 1, "tianxiang");
                        if matches!(g.zone_of(&discarded), Some(Zone::Discard)) {
                            g.gain(&discarded, target)?;
                        }
                        g.push_log(format!(
                            "天香：{} 失去 1 点体力并获得【{}】",
                            g.seats[target].name,
                            label(g, &discarded)
                        ));
                    }
                    Ok(Step::Done)
                }
            }
        }
    }
}
