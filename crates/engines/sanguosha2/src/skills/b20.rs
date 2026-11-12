//! B20（界限突破·J8）——按 `align/B20.json` 第 7 段实现。
//!
//! 覆盖：界夏侯氏（樵拾/燕语）、界张松（强识/献图）、界于禁（镇军/毅重）、界步练师（安恤/追忆）。
//!
//! 用户裁定（2026-11-10，35 条疑点）要点：
//! - 樵拾：仅其他角色结束阶段；发动后手牌数不等 → 本轮（round）失效。
//! - 燕语：重铸【杀】无次数限制；「失去 ≥2 张【杀】」含使用/弃置/被获得等全部失去；阶段结束令一名男性摸 2（可含自己）。
//! - 强识：观看后自选一张公开展示；本阶段同类牌可选摸一张。
//! - 献图：摸至多两张（0/1/2）自选交等量；阶段结束比较「其造成伤害点数 < 交给张数」→ 失去 1 点体力；交 0 张不失去。
//! - 镇军：X=max(1,手牌-体力)；弃目标牌与选项①均由自己选；选项②在于禁本回合结束阶段令其摸 n。
//! - 毅重：①黑色【杀】对体力 ≥ 于禁者无效（Immune，不结算）；②手牌 ≤ 于禁者无法响应其黑色【杀】（含转化【闪】/八卦）。
//! - 安恤：获得两名不同其他角色各一张牌 → 从获得的两张中展示一张交给手牌较少者（获得后比较）→ 非黑桃摸一张。
//! - 追忆：限定技（结束阶段一次）+ 死亡时（亡语，统一 on_death 钩子）可对首次目标再发动一次（独立、不排除杀死者）。

use serde_json::json;

use crate::cards::CardKind;
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};

use super::{active, builder, other_alive, SkillDef, SkillKind};

fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, Stage::Choose, actor, data);
}
fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect()
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}
fn hand(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
}
fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}
fn region_ids(g: &Game, seat: usize) -> Vec<CardId> {
    let mut v = hand(g, seat);
    v.extend(equip_ids(g, seat));
    v
}
fn is_sha(g: &Game, id: &CardId) -> bool {
    g.def_of(id).tags.sha
}
fn card_usable(g: &Game, seat: usize, id: &CardId) -> bool {
    g.can_use_card (seat, id)
}
fn type_bit(g: &Game, id: &CardId) -> u32 {
    match g.def_of(id).kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    }
}
fn event_is(a: &Ask, suffix: &str) -> bool {
    a.data["event"]
        .as_str()
        .map(|e| e.starts_with(suffix))
        .unwrap_or(false)
}
fn cause_is(a: &Ask, phase: Phase) -> bool {
    a.data["cause"].as_str() == Some(&format!("{phase:?}"))
}
fn seat_of(a: &Ask, key: &str) -> usize {
    a.data[key].as_u64().unwrap_or(0) as usize
}

// ---------------------------------------------------------------------------
// 樵拾（界夏侯氏）
// ---------------------------------------------------------------------------

pub const QIAOSHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qiaoshi",
    name: "樵拾",
    triggers: &[EventKind::PhaseStart],
    window: Window::OtherPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: qiaoshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qiaoshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let current = seat_of(a, "seat");
    if !active(g, owner, "qiaoshi") || owner == current || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    if g.counter(&format!("round.qiaoshi_off:{owner}")) > 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let b = builder(owner, "choice").skill("qiaoshi");
            let b = b.option(
                "opt1",
                format!(
                    "樵拾：与你各摸一张（{} 手牌 {}）",
                    g.seats[current].name,
                    g.hand(current).len()
                ),
            );
            Ok(Step::Need(b.escape("不发动樵拾").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            let a1 = g.draw(owner, 1).len();
            let b1 = g.draw(current, 1).len();
            g.push_log(format!(
                "樵拾：{} 摸 {a1} 张，{} 摸 {b1} 张",
                g.seats[owner].name, g.seats[current].name
            ));
            if g.hand(owner).len() != g.hand(current).len() {
                g.bump(&format!("round.qiaoshi_off:{owner}"));
                g.push_log("樵拾：双方手牌数不相等，本技能本轮失效".to_string());
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 燕语（界夏侯氏）
// ---------------------------------------------------------------------------

pub const YANYU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "yanyu",
    name: "燕语",
    triggers: &[EventKind::CardLost, EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 0,
    can_active: Some(can_yanyu),
    handler: yanyu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_yanyu(g: &Game, seat: usize) -> bool {
    active(g, seat, "yanyu")
        && play_phase(g, seat)
        && hand(g, seat)
            .iter()
            .any(|id| is_sha(g, id) && card_usable(g, seat, id))
}

fn yanyu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;

    // ① 本阶段失去【杀】计数（任何离开手牌的方式）——只算**自己**失去的牌。
    if event_is(a, "CardLost") {
        if !active(g, owner, "yanyu") || !play_phase(g, owner) {
            return Ok(Step::Done);
        }
        // 归属守卫（同类缺陷排查）：他人的失去牌不计入「你失去的【杀】」。
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|c| c.to_string()) else {
            return Ok(Step::Done);
        };
        if is_sha(g, &card) {
            g.bump(&format!("turn.yanyu_lost:{owner}"));
        }
        return Ok(Step::Done);
    }

    // ③ 出牌阶段结束：失去 ≥2 张【杀】→ 可令一名男性摸两张。
    if event_is(a, "PhaseEnd") {
        if !active(g, owner, "yanyu") || !cause_is(a, Phase::Play) {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.yanyu_lost:{owner}")) < 2 {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = alive(g)
            .into_iter()
            .filter(|t| g.seats[*t].is_male())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("yanyu");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("燕语：令 {} 摸两张", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动燕语").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("燕语：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("燕语：目标必须是男性角色".into());
                }
                let n = g.draw(t, 2).len();
                g.push_log(format!("燕语：{} 摸 {n} 张", g.seats[t].name));
                Ok(Step::Done)
            }
        };
    }

    // ② 主动：重铸【杀】（弃置并摸一张；无次数限制）。
    if !can_yanyu(g, owner) {
        return Ok(Step::Done);
    }
    let shas: Vec<CardId> = hand(g, owner)
        .into_iter()
        .filter(|id| is_sha(g, id) && card_usable(g, owner, id))
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("yanyu");
            for id in &shas {
                b = b.option(
                    format!("choose:{id}"),
                    format!("燕语：重铸【{}】（摸一张）", label(g, id)),
                );
            }
            // 出口即「结束重铸」（`escape_with` 让出口就是 done 本身）：
            // 之前同时给 `done` 选项和 `pass` 出口、文案还都一样 → 端侧两个一模一样的按钮。
            Ok(Step::Need(b.escape_with("done", "结束重铸").build()))
        }
        Some("done") | Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("燕语：未知选项 {act}"))?
                .to_string();
            if !shas.contains(&id) {
                return Err("燕语：需要一张可重铸的【杀】".into());
            }
            g.to_discard(&id, "yanyu")?;
            let n = g.draw(owner, 1).len();
            g.push_log(format!("燕语：重铸【{}】，摸 {n} 张", label(g, &id)));
            let mut b = builder(owner, "card").skill("yanyu");
            for c in hand(g, owner)
                .into_iter()
                .filter(|c| is_sha(g, c) && card_usable(g, owner, c))
            {
                b = b.option(
                    format!("choose:{c}"),
                    format!("燕语：重铸【{}】（摸一张）", label(g, &c)),
                );
            }
            Ok(Step::NeedAt(
                Stage::Choose,
                b.escape_with("done", "结束重铸").build(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 强识（界张松）
// ---------------------------------------------------------------------------

pub const QIANGSHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qiangshi",
    name: "强识",
    triggers: &[EventKind::PhaseStart, EventKind::CardUsed],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: qiangshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qiangshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "qiangshi") {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();

    // ② 本阶段使用同类别牌 → 可以摸一张。
    if event_is(a, "CardUsed") && stage.is_empty() {
        let want = g.counter("turn.qiangshi_type");
        if want == 0 {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|c| c.to_string()) else {
            return Ok(Step::Done);
        };
        if !g.cards.contains_key(&card) || !is_card_type(g, &card, want) {
            return Ok(Step::Done);
        }
        if g.turn != owner {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let b = builder(owner, "choice").skill("qiangshi");
                let b = b.option("draw", "强识：使用同类别的牌，摸一张");
                Ok(Step::Need(b.escape("不摸牌").build()))
            }
            Some("draw") => {
                let n = g.draw(owner, 1).len();
                g.push_log(format!("强识：{} 摸 {n} 张", g.seats[owner].name));
                Ok(Step::Done)
            }
            Some(_) => Ok(Step::Done),
        };
    }

    // ① 出牌阶段开始：观看并展示一名其他角色的一张手牌。
    if stage.is_empty() {
        if !cause_is(a, Phase::Play) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| !hand(g, *t).is_empty())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("qiangshi");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("强识：观看 {} 的手牌", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动强识").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("强识：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("强识：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "target": t }),
                ))
            }
        };
    }

    // pick：张松从其手牌中自选一张展示。
    let target = seat_of(a, "target");
    let cands = hand(g, target);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("qiangshi");
            for id in &cands {
                b = b.option(format!("choose:{id}"), format!("展示【{}】", label(g, id)));
            }
            Ok(Step::Need(b.escape("不发动强识").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("强识：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("强识：只能展示其手牌中的牌".into());
            }
            g.reveal(&id, Some(target), "qiangshi");
            let bit = type_bit(g, &id);
            g.counters.insert("turn.qiangshi_type".to_string(), bit);
            let kind = match bit {
                1 => "基本牌",
                2 => "锦囊牌",
                _ => "装备牌",
            };
            g.push_log(format!(
                "强识：展示 {} 的【{}】（{kind}），本阶段使用同类牌可摸一张",
                g.seats[target].name,
                label(g, &id)
            ));
            Ok(Step::Done)
        }
    }
}

fn is_card_type(g: &Game, id: &CardId, bit: u32) -> bool {
    type_bit(g, id) == bit
}

// ---------------------------------------------------------------------------
// 献图（界张松）
// ---------------------------------------------------------------------------

pub const XIANTU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "xiantu",
    name: "献图",
    triggers: &[EventKind::PhaseStart, EventKind::PhaseEnd],
    window: Window::OtherTurn,
    priority: 0,
    can_active: None,
    handler: xiantu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn xiantu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "xiantu") || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let target = if a.data["target"].is_number() {
        seat_of(a, "target")
    } else {
        seat_of(a, "seat")
    };
    if target == owner {
        return Ok(Step::Done);
    }

    // 阶段结束：其造成伤害点数 < 交给张数 → 失去 1 点体力。
    if event_is(a, "PhaseEnd") && stage.is_empty() {
        if !cause_is(a, Phase::Play) {
            return Ok(Step::Done);
        }
        let marked = g.counter(&format!("turn.xiantu_target:{owner}"));
        if marked == 0 || (marked - 1) as usize != target {
            return Ok(Step::Done);
        }
        let given = g.counter(&format!("turn.xiantu_given:{owner}"));
        let base = g.counter(&format!("turn.xiantu_base:{owner}"));
        let dealt = g
            .counter(&format!("turn.damage_by:{target}"))
            .saturating_sub(base);
        g.counters
            .insert(format!("turn.xiantu_target:{owner}"), 0);
        if dealt < given {
            g.push_log(format!(
                "献图：{} 本阶段造成伤害 {dealt} < 交给的 {given} 张，{} 失去 1 点体力",
                g.seats[target].name, g.seats[owner].name
            ));
            flow::lose_hp(g, owner, 1, "xiantu");
        } else {
            g.push_log(format!(
                "献图：{} 本阶段造成伤害 {dealt} ≥ 交给的 {given} 张，不失去体力",
                g.seats[target].name
            ));
        }
        return Ok(Step::Done);
    }

    // 出牌阶段开始：询问摸至多两张。
    if stage.is_empty() {
        if !cause_is(a, Phase::Play) {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.xiantu_offered:{owner}")) > 0 {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("xiantu");
                for n in [2usize, 1] {
                    b = b.option(
                        format!("opt{n}"),
                        format!("献图：摸 {n} 张并交给 {} {n} 张", g.seats[target].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动献图").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let n = act
                    .strip_prefix("opt")
                    .and_then(|x| x.parse::<usize>().ok())
                    .filter(|n| *n == 1 || *n == 2)
                    .ok_or_else(|| format!("献图：未知选项 {act}"))?;
                g.bump(&format!("turn.xiantu_offered:{owner}"));
                let drawn = g.draw(owner, n).len();
                g.push_log(format!("献图：{} 摸 {drawn} 张", g.seats[owner].name));
                if drawn == 0 {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "stage": "give",
                        "target": target,
                        "left": drawn,
                        "given": 0,
                    }),
                ))
            }
        };
    }

    // give：张松自选等量手牌交给当前回合角色。
    let left = a.data["left"].as_u64().unwrap_or(0) as usize;
    let given = a.data["given"].as_u64().unwrap_or(0) as usize;
    if left == 0 {
        finish_xiantu(g, owner, target, given);
        return Ok(Step::Done);
    }
    let cands: Vec<CardId> = hand(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("xiantu");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("交给【{}】（还需 {left} 张）", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("少交（不再交牌）").build()))
        }
        Some("pass") => {
            finish_xiantu(g, owner, target, given);
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("献图：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("献图：只能交出自己的手牌".into());
            }
            g.gain(&id, target)?;
            g.push_log(format!(
                "献图：将【{}】交给 {}",
                label(g, &id),
                g.seats[target].name
            ));
            let left = left - 1;
            let given = given + 1;
            if left == 0 {
                finish_xiantu(g, owner, target, given);
                Ok(Step::Done)
            } else {
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "give", "target": target, "left": left, "given": given }),
                ))
            }
        }
    }
}

fn finish_xiantu(g: &mut Game, owner: usize, target: usize, given: usize) {
    if given == 0 {
        return;
    }
    g.counters
        .insert(format!("turn.xiantu_given:{owner}"), given as u32);
    g.counters
        .insert(format!("turn.xiantu_target:{owner}"), target as u32 + 1);
    g.counters.insert(
        format!("turn.xiantu_base:{owner}"),
        g.counter(&format!("turn.damage_by:{target}")),
    );
    g.push_log(format!(
        "献图：共交给 {} {given} 张（阶段结束时比较其造成的伤害）",
        g.seats[target].name
    ));
}

// ---------------------------------------------------------------------------
// 镇军（界于禁）
// ---------------------------------------------------------------------------

pub const ZHENJUN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhenjun",
    name: "镇军",
    triggers: &[EventKind::PhaseStart, EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: zhenjun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhenjun_x(g: &Game, target: usize) -> usize {
    let h = g.hand(target).len() as i32;
    let hp = g.seats[target].hp.max(0);
    ((h - hp).max(1)) as usize
}

fn zhenjun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "zhenjun") {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();

    // 结束阶段（阶段开始时）：结算选项②的延迟摸牌。
    if event_is(a, "PhaseStart") && cause_is(a, Phase::End) {
        zhenjun_end_draw(g);
        return Ok(Step::Done);
    }

    // 结束阶段：结算选项②的延迟摸牌。
    if event_is(a, "PhaseEnd") {
        if cause_is(a, Phase::End) {
            zhenjun_end_draw(g);
        }
        return Ok(Step::Done);
    }

    if stage.is_empty() {
        if !cause_is(a, Phase::Prepare) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = alive(g)
            .into_iter()
            .filter(|t| !region_ids(g, *t).is_empty())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("zhenjun");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("镇军：{}（X={}）", g.seats[*t].name, zhenjun_x(g, *t)),
                    );
                }
                Ok(Step::Need(b.escape("不发动镇军").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("镇军：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("镇军：目标不合法".into());
                }
                let x = zhenjun_x(g, t).min(region_ids(g, t).len());
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "discard", "target": t, "left": x, "non_equip": 0 }),
                ))
            }
        };
    }

    // discard：由于禁逐张选择弃置目标的牌（手牌随机 / 装备区具体牌）。
    if stage == "discard" {
        let target = seat_of(a, "target");
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        let non_equip = a.data["non_equip"].as_u64().unwrap_or(0) as usize;
        if left == 0 || region_ids(g, target).is_empty() {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "after", "target": target, "non_equip": non_equip }),
            ));
        }
        let equips = equip_ids(g, target);
        let has_hand = !hand(g, target).is_empty();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("zhenjun");
                if has_hand {
                    b = b.option(
                        "hand",
                        format!(
                            "镇军：弃置 {} 一张随机手牌（还需 {left} 张）",
                            g.seats[target].name
                        ),
                    );
                }
                for id in &equips {
                    b = b.option(
                        format!("equip:{id}"),
                        format!("镇军：弃置其装备【{}】", label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("停止弃置").build()))
            }
            Some("pass") => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "after", "target": target, "non_equip": non_equip }),
            )),
            Some("hand") => {
                if !has_hand {
                    return Err("镇军：其没有手牌".into());
                }
                let idx = g.rand_below(g.hand(target).len());
                let id = g.hand(target)[idx].clone();
                g.to_discard(&id, "zhenjun")?;
                g.push_log(format!(
                    "镇军：弃置 {} 的【{}】",
                    g.seats[target].name,
                    label(g, &id)
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "stage": "discard",
                        "target": target,
                        "left": left - 1,
                        "non_equip": non_equip + 1,
                    }),
                ))
            }
            Some(act) => {
                let id = act
                    .strip_prefix("equip:")
                    .ok_or_else(|| format!("镇军：未知选项 {act}"))?
                    .to_string();
                if !equips.contains(&id) {
                    return Err("镇军：需要弃置其装备区的牌".into());
                }
                g.to_discard(&id, "zhenjun")?;
                g.push_log(format!(
                    "镇军：弃置 {} 的装备【{}】",
                    g.seats[target].name,
                    label(g, &id)
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "stage": "discard",
                        "target": target,
                        "left": left - 1,
                        "non_equip": non_equip,
                    }),
                ))
            }
        };
    }

    // after：二选一（非装备牌数为 0 则结束）。
    if stage == "after" {
        let target = seat_of(a, "target");
        let n = a.data["non_equip"].as_u64().unwrap_or(0) as usize;
        if n == 0 {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zhenjun");
                b = b.option("opt1", format!("镇军：你弃置 {n} 张牌"));
                b = b.option(
                    "opt2",
                    format!(
                        "镇军：本回合结束阶段，{} 摸 {n} 张",
                        g.seats[target].name
                    ),
                );
                Ok(Step::Need(b.escape("放弃后续（不弃不摸）").build()))
            }
            Some("opt1") => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "self", "left": n }),
            )),
            Some("opt2") => {
                g.counters
                    .insert(format!("turn.zhenjun_draw:{target}"), n as u32);
                g.push_log(format!(
                    "镇军：{} 将于本回合结束阶段摸 {n} 张",
                    g.seats[target].name
                ));
                Ok(Step::Done)
            }
            Some(_) => Ok(Step::Done),
        };
    }

    // self：选项①自己弃 n 张（自选）。
    let left = a.data["left"].as_u64().unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    let cands = region_ids(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("zhenjun");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("镇军：你弃置【{}】（还需 {left} 张）", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("停止弃置").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("镇军：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("镇军：只能弃置自己的牌".into());
            }
            g.to_discard(&id, "zhenjun")?;
            g.push_log(format!(
                "镇军：{} 弃置【{}】",
                g.seats[owner].name,
                label(g, &id)
            ));
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "self", "left": left - 1 }),
            ))
        }
    }
}

fn zhenjun_end_draw(g: &mut Game) {
    let keys: Vec<String> = g
        .counters
        .keys()
        .filter(|k| k.starts_with("turn.zhenjun_draw:"))
        .cloned()
        .collect();
    for k in keys {
        let n = g.counters.remove(&k).unwrap_or(0) as usize;
        let Ok(t) = k.trim_start_matches("turn.zhenjun_draw:").parse::<usize>() else {
            continue;
        };
        if t < g.seats.len() && g.seats[t].hp > 0 {
            let drawn = g.draw(t, n).len();
            g.push_log(format!(
                "镇军：{} 在结束阶段摸 {drawn} 张",
                g.seats[t].name
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// 毅重（界于禁，锁定技）
// ---------------------------------------------------------------------------

pub const YIZHONG: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "yizhong",
    name: "毅重",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: yizhong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yizhong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["intercept"].as_bool().unwrap_or(false) {
        let target = seat_of(a, "target");
        g.sha_intercept = Some(ShaIntercept::Immune(target));
    }
    Ok(Step::Done)
}

fn is_black_sha(g: &Game, seat: usize, card: Option<&CardId>) -> bool {
    let Some(c) = card else { return false };
    if !g.def_of(c).tags.sha {
        return false;
    }
    // 颜色读实体牌花色（转化牌按承载牌；红颜等走 `effective_suit` 入口）。
    !crate::skills::effective_suit(g, seat, c)
        .map(|s| s.is_red())
        .unwrap_or(false)
}

/// 〔杀〕指定目标后：①体力 ≥ 于禁者的黑色【杀】无效（该【杀】对其不结算）。
pub fn sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    if user == target || !active(g, target, "yizhong") {
        return Vec::new();
    }
    if !is_black_sha(g, user, card) {
        return Vec::new();
    }
    if g.seats[user].hp < g.seats[target].hp {
        return Vec::new();
    }
    vec![(target, "yizhong")]
}

/// ②手牌数 ≤ 于禁的角色无法响应其黑色【杀】。
pub fn shan_blocked_for(
    g: &Game,
    responder: usize,
    source: Option<usize>,
    card: Option<&CardId>,
) -> bool {
    let Some(src) = source else { return false };
    if src == responder || !active(g, src, "yizhong") {
        return false;
    }
    if !is_black_sha(g, src, card) {
        return false;
    }
    g.hand(responder).len() <= g.hand(src).len()
}

// ---------------------------------------------------------------------------
// 安恤（界步练师）
// ---------------------------------------------------------------------------

pub const ANXU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "anxu",
    name: "安恤",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_anxu),
    handler: anxu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_anxu(g: &Game, seat: usize) -> bool {
    active(g, seat, "anxu")
        && play_phase(g, seat)
        && g.counter("turn.anxu_used") == 0
        && other_alive(g, seat)
            .into_iter()
            .filter(|t| !region_ids(g, *t).is_empty())
            .count()
            >= 2
}

fn anxu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();

    // pick：获得某人一张牌（手牌随机 / 装备区具体牌）。
    if stage == "pick" {
        let who = seat_of(a, "who");
        let first = a.data["first"].as_bool().unwrap_or(false);
        let a_seat = a.data["a"].as_u64().map(|n| n as usize);
        let c1 = a.data["c1"].as_str().map(|s| s.to_string());
        let equips = equip_ids(g, who);
        let has_hand = !hand(g, who).is_empty();
        let picked: Option<CardId> = match g.answer.take().as_deref() {
            None => None,
            Some("pass") => return Ok(Step::Done),
            Some("hand") => {
                if !has_hand {
                    return Err("安恤：其没有手牌".into());
                }
                let idx = g.rand_below(g.hand(who).len());
                Some(g.hand(who)[idx].clone())
            }
            Some(act) => {
                let id = act
                    .strip_prefix("equip:")
                    .ok_or_else(|| format!("安恤：未知选项 {act}"))?
                    .to_string();
                if !equips.contains(&id) {
                    return Err("安恤：需要获得其装备区的牌".into());
                }
                Some(id)
            }
        };
        let Some(id) = picked else {
            let mut b = builder(owner, "card").skill("anxu");
            if has_hand {
                b = b.option(
                    "hand",
                    format!("安恤：获得 {} 一张随机手牌", g.seats[who].name),
                );
            }
            for e in &equips {
                b = b.option(
                    format!("equip:{e}"),
                    format!("安恤：获得其装备【{}】", label(g, e)),
                );
            }
            return Ok(Step::Need(b.escape("取消安恤").build()));
        };
        g.gain(&id, owner)?;
        g.push_log(format!(
            "安恤：获得 {} 的【{}】",
            g.seats[who].name,
            label(g, &id)
        ));

        if first {
            return Ok(Step::Next(
                Stage::Target,
                json!({ "stage": "second", "a": who, "c1": id }),
            ));
        }
        let a_seat = a_seat.ok_or_else(|| "安恤：缺少第一名角色".to_string())?;
        let c1 = c1.ok_or_else(|| "安恤：缺少第一张牌".to_string())?;
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "give", "a": a_seat, "b": who, "c1": c1, "c2": id }),
        ));
    }

    // second：选第二名角色（不同、仍有牌）→ 回到 pick。
    if stage == "second" {
        let a_seat = seat_of(a, "a");
        let c1 = a.data["c1"].as_str().unwrap_or_default().to_string();
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| *t != a_seat && !region_ids(g, *t).is_empty())
            .collect();
        if cands.is_empty() {
            g.push_log("安恤：没有第二名角色，结算结束".to_string());
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("anxu");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("安恤：再获得 {} 一张牌", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消安恤").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("安恤：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("安恤：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "who": t, "first": false, "a": a_seat, "c1": c1 }),
                ))
            }
        };
    }

    // give：从获得的两张中选一张展示 → 交给手牌较少者。
    if stage == "give" {
        let a_seat = seat_of(a, "a");
        let b_seat = seat_of(a, "b");
        let c1 = a.data["c1"].as_str().unwrap_or_default().to_string();
        let c2 = a.data["c2"].as_str().unwrap_or_default().to_string();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("anxu");
                for id in [&c1, &c2] {
                    if g.cards.contains_key(id) {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("展示【{}】并交给手牌较少者", label(g, id)),
                        );
                    }
                }
                Ok(Step::Need(b.escape("取消安恤").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("安恤：未知选项 {act}"))?
                    .to_string();
                if id != c1 && id != c2 {
                    return Err("安恤：只能展示刚获得的两张之一".into());
                }
                g.reveal(&id, Some(owner), "anxu");
                let ha = g.hand(a_seat).len();
                let hb = g.hand(b_seat).len();
                if ha == hb {
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "stage": "lesser", "id": id, "a": a_seat, "b": b_seat }),
                    ))
                } else {
                    let recipient = if ha < hb { a_seat } else { b_seat };
                    finish_anxu(g, owner, &id, recipient)
                }
            }
        };
    }

    // lesser：手牌数相同 → 由安恤者选交给谁。
    if stage == "lesser" {
        let id = a.data["id"].as_str().unwrap_or_default().to_string();
        let a_seat = seat_of(a, "a");
        let b_seat = seat_of(a, "b");
        let cands = [a_seat, b_seat];
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("anxu");
                for t in cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("安恤：交给 {}", g.seats[t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消安恤").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("安恤：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("安恤：只能交给两名角色之一".into());
                }
                finish_anxu(g, owner, &id, t)
            }
        };
    }

    // 起始：选第一名角色。
    if !can_anxu(g, owner) {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = other_alive(g, owner)
        .into_iter()
        .filter(|t| !region_ids(g, *t).is_empty())
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("anxu");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("安恤：获得 {} 一张牌", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动安恤").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("安恤：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("安恤：目标不合法".into());
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick", "who": t, "first": true }),
            ))
        }
    }
}

fn finish_anxu(
    g: &mut Game,
    owner: usize,
    id: &CardId,
    recipient: usize,
) -> Result<Step, String> {
    g.gain(id, recipient)?;
    g.bump("turn.anxu_used");
    let spade = crate::skills::effective_suit(g, owner, id)
        .map(|s| matches!(s, crate::kernel::Suit::Spade))
        .unwrap_or(false);
    g.push_log(format!(
        "安恤：展示【{}】并交给 {}",
        label(g, id),
        g.seats[recipient].name
    ));
    if !spade {
        let n = g.draw(owner, 1).len();
        g.push_log(format!(
            "安恤：展示牌不为黑桃，{} 摸 {n} 张",
            g.seats[owner].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 追忆（界步练师，限定技 + 亡语）
// ---------------------------------------------------------------------------

pub const ZHUIXI: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "zhuixi",
    name: "追忆",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: zhuixi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhuixi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "zhuixi") {
        return Ok(Step::Done);
    }
    let via = a.data["via"].as_str().unwrap_or("end").to_string();

    // 死亡时再次发动：只对首次选定的角色。
    if via == "death" {
        let target = g
            .seats[owner]
            .counters
            .get("zhuixi_target")
            .copied()
            .unwrap_or(0) as usize;
        if target == 0 || g.seats[target - 1].hp <= 0 {
            return Ok(Step::Done);
        }
        let t = target - 1;
        return match g.answer.take().as_deref() {
            None => {
                let b = builder(owner, "choice").skill("zhuixi");
                let b = b.option(
                    "opt1",
                    format!("追忆（亡语）：对 {} 再次发动", g.seats[t].name),
                );
                Ok(Step::Need(b.escape("不再发动").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let n = g.draw(t, 3).len();
                flow::heal(g, t, 1, "zhuixi");
                g.push_log(format!(
                    "追忆（亡语）：{} 摸 {n} 张并回复 1 点体力",
                    g.seats[t].name
                ));
                Ok(Step::Done)
            }
        };
    }

    if g.seats[owner].flags.contains("zhuixi_used") {
        return Ok(Step::Done);
    }
    let cands = other_alive(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("zhuixi");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("追忆：令 {} 摸三张并回复 1", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动追忆").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("追忆：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("追忆：目标不合法".into());
            }
            g.seats[owner].flags.insert("zhuixi_used".to_string());
            g.seats[owner]
                .counters
                .insert("zhuixi_target".to_string(), t as u32 + 1);
            let n = g.draw(t, 3).len();
            flow::heal(g, t, 1, "zhuixi");
            g.push_log(format!(
                "追忆：{} 摸 {n} 张并回复 1 点体力",
                g.seats[t].name
            ));
            Ok(Step::Done)
        }
    }
}

/// 亡语（统一死亡钩子）：步练师死亡时，可对首次目标再次发动追忆。
pub fn on_death(g: &mut Game, victim: usize, _source: Option<usize>) {
    if !active(g, victim, "zhuixi") || !g.seats[victim].flags.contains("zhuixi_used") {
        return;
    }
    let target = g
        .seats[victim]
        .counters
        .get("zhuixi_target")
        .copied()
        .unwrap_or(0);
    if target == 0 || g.seats[target as usize - 1].hp <= 0 {
        return;
    }
    push(g, "zhuixi", victim, json!({ "via": "death" }));
}
