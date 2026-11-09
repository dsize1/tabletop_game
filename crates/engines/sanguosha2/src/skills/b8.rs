//! B8（辛宪英 / 吴苋 / 徐氏 / 曹节 / 嵇康 / 秦宓 / 薛综 / 蔡邕）——按 `align/B8.json` 第 7 段实现。
//!
//! 本批落实的跨批机制：
//! - **拼点**（mechanics `pindian`）：内核 `kernel::pindian`（双方暗选 → 同时亮牌 → 比有效点数 →
//!   双方拼点牌进弃牌堆）。天辩通过**通用钩子**（`pindian_options` / `pindian_take` / `pindian_rank`）
//!   接入，内核不含技能名。
//! - **目标数修正**（`target-extra`）：红色基本牌/红色普通锦囊可多指定 N 个目标（无距离）。
//! - **牌堆承载使用**（`deck-carrier`）：伏诛以牌堆牌为承载牌真实结算【杀】。
//! - **结算前【杀】介入窗口**：守玺 / 谏征 / 专对走 `sha_intercept_skills`（不是 `TargetChosen`
//!   事件——事件在牌结算之后才路由）。
//!
//! 其它裁定：失去体力一律 `flow::lose_hp`，回复一律 `flow::heal`；「随机」走 `Game::rand_below`
//! （确定性、可回放）；复难「不能使用或打出此牌」用**手牌冻结**（按牌 id）。

use serde_json::json;

use crate::cards::{self, CardKind};
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::pindian;
use crate::kernel::resolve::{Ask, FlowFrame, FlowKind, Frame, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};

use super::{active, builder, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 通用工具
// ---------------------------------------------------------------------------

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
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

fn phase_cause(a: &Ask) -> String {
    a.data["cause"].as_str().unwrap_or_default().to_string()
}

/// 是否有角色正处于濒死结算（「若没有角色处于濒死状态」）。
fn someone_dying(g: &Game) -> bool {
    g.stack.iter().any(|f| {
        matches!(
            f,
            Frame::Flow(FlowFrame {
                kind: FlowKind::Dying,
                ..
            })
        )
    }) || g.seats.iter().any(|s| s.hp <= 0)
}

fn seat_counter(g: &Game, seat: usize, key: &str) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get(key).copied())
        .unwrap_or(0)
}

fn set_seat_counter(g: &mut Game, seat: usize, key: &str, value: u32) {
    g.seats[seat].counters.insert(key.to_string(), value);
}

fn bump_seat_counter(g: &mut Game, seat: usize, key: &str) {
    let n = seat_counter(g, seat, key) + 1;
    set_seat_counter(g, seat, key, n);
}

fn mark_count(g: &Game, seat: usize, code: &str) -> usize {
    g.mark_cards(seat, code).len()
}

/// 从牌堆顶向下找第一张满足条件的牌；没有则遍历弃牌堆（用户 2026-11-09 裁定）。
fn find_in_piles(g: &Game, pred: impl Fn(&CardId) -> bool) -> Option<CardId> {
    if let Some(id) = g.ids_in(|z| *z == Zone::Draw).into_iter().find(|id| pred(id)) {
        return Some(id);
    }
    g.discard_pile().into_iter().find(|id| pred(id))
}

/// 「随机使用一张装备牌」（清弦 / 四弦）：牌堆顶向下第一张装备牌，无则遍历弃牌堆。
pub fn take_equip_from_piles(g: &mut Game, cause: &str) -> Option<CardId> {
    let id = find_in_piles(g, |id| g.def_of(id).kind == CardKind::Equip)?;
    g.push_log(format!("{cause}：从{}取【{}】", "牌堆/弃牌堆", label(g, &id)));
    Some(id)
}

/// 失去 1 点体力 + 随机使用一张装备牌（清弦①/激弦/烈弦）。
fn lose_hp_and_use_equip(
    g: &mut Game,
    seat: usize,
    cause: &str,
) -> Result<Option<CardId>, String> {
    flow::lose_hp(g, seat, 1, cause);
    let Some(id) = take_equip_from_piles(g, cause) else {
        return Ok(None);
    };
    let name = g.name_of(&id);
    g.use_as(seat, &id, &name, Vec::new(), false, None)?;
    Ok(Some(id))
}

/// 装备区/手牌区选项的统一标签（通博槽位严格对应）。
fn zone_tagged_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in g.hand(seat) {
        out.push((
            format!("own:hand:{id}"),
            format!("手牌【{}】", label(g, &id)),
        ));
    }
    for slot in [
        Slot::Weapon,
        Slot::Armor,
        Slot::HorseOffense,
        Slot::HorseDefense,
        Slot::Treasure,
    ] {
        if let Some(id) = g.equip_in(seat, slot) {
            out.push((
                format!("own:equip:{slot:?}:{id}"),
                format!("{}【{}】", slot.label(), label(g, &id)),
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 忠鉴（辛宪英）
// ---------------------------------------------------------------------------

pub const ZHONGJIAN: SkillDef = SkillDef {
    code: "zhongjian",
    name: "忠鉴",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhongjian),
    handler: zhongjian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhongjian_limit(g: &Game) -> u32 {
    if g.counter("turn.zhongjian_extra") > 0 {
        2
    } else {
        1
    }
}

fn can_zhongjian(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhongjian")
        && play_phase(g, seat)
        && g.counter("turn.zhongjian_used") < zhongjian_limit(g)
        && !g.hand(seat).is_empty()
        && other_alive(g, seat).into_iter().any(|i| !g.hand(i).is_empty())
}

fn zhongjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("own").to_string();
    match step.as_str() {
        "own" => match g.answer.take().as_deref() {
            None => {
                let hand = g.hand(a.actor);
                let mut b = builder(a.actor, "choice").skill("zhongjian");
                for id in &hand {
                    b = b.option(format!("choose:{id}"), format!("展示【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不发动忠鉴").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("忠鉴：未知选项 {act}"))?
                    .to_string();
                if !g.hand(a.actor).contains(&id) {
                    return Err("忠鉴：只能展示自己的手牌".into());
                }
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "own": id }),
                ))
            }
        },
        "target" => match g.answer.take().as_deref() {
            None => {
                let cands: Vec<usize> = other_alive(g, a.actor)
                    .into_iter()
                    .filter(|i| !g.hand(*i).is_empty())
                    .collect();
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = builder(a.actor, "target").skill("zhongjian");
                for i in &cands {
                    b = b.option(
                        format!("target:{i}"),
                        format!("查看 {} 的手牌", g.seats[*i].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动忠鉴").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("忠鉴：目标不合法 {act}"))?;
                if t == a.actor || g.seats[t].hp <= 0 || g.hand(t).is_empty() {
                    return Err("忠鉴：目标不合法".into());
                }
                let n = g.hand(t).len().min(g.seats[t].hp.max(0) as usize);
                let own = a.data["own"].as_str().unwrap_or_default().to_string();
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "show", "own": own, "target": t, "left": n, "picks": [] }),
                ))
            }
        },
        "show" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let left = a.data["left"].as_u64().unwrap_or(0) as u64;
            let mut picks: Vec<CardId> = a.data["picks"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if left == 0 {
                return resolve_zhongjian(g, a);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(target, "choice").skill("zhongjian");
                    for id in g.hand(target) {
                        if picks.contains(&id) {
                            continue;
                        }
                        b = b.option(format!("pick:{id}"), format!("展示【{}】", label(g, &id)));
                    }
                    Ok(Step::Need(b.escape("停止展示").build()))
                }
                Some("pass") => resolve_zhongjian(g, a),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("忠鉴：未知选项 {act}"))?
                        .to_string();
                    if !g.hand(target).contains(&id) || picks.contains(&id) {
                        return Err("忠鉴：只能展示其手牌".into());
                    }
                    g.reveal(&id, Some(target), "zhongjian");
                    picks.push(id);
                    let data = json!({
                        "step": "show",
                        "own": a.data["own"],
                        "target": target,
                        "left": left - 1,
                        "picks": picks,
                    });
                    Ok(Step::Next(Stage::PickOther, data))
                }
            }
        }
        "reward" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "choice").skill("zhongjian");
                b = b.option("draw", "摸一张牌");
                if other_alive(g, a.actor).into_iter().any(|i| !hand_and_equip(g, i).is_empty()) {
                    b = b.option("discard", "弃置一名其他角色一张牌");
                }
                Ok(Step::Need(b.escape("不获得奖励").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("draw") => {
                let n = g.draw(a.actor, 1).len();
                g.push_log(format!("忠鉴：摸 {n} 张牌"));
                Ok(Step::Done)
            }
            Some("discard") => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "discard_target" }),
            )),
            Some(other) => Err(format!("忠鉴：未知选项 {other}")),
        },
        "discard_target" => match g.answer.take().as_deref() {
            None => {
                let cands: Vec<usize> = other_alive(g, a.actor)
                    .into_iter()
                    .filter(|i| !hand_and_equip(g, *i).is_empty())
                    .collect();
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = builder(a.actor, "target").skill("zhongjian");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("弃置 {} 一张牌", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("放弃弃牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("忠鉴：目标不合法 {act}"))?;
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "discard_pick", "target": t }),
                ))
            }
        },
        "discard_pick" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let ids = hand_and_equip(g, target);
                    if ids.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(a.actor, "choice").skill("zhongjian");
                    for id in &ids {
                        b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("放弃弃牌").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("忠鉴：未知选项 {act}"))?
                        .to_string();
                    if !hand_and_equip(g, target).contains(&id) {
                        return Err("忠鉴：该牌不在目标区域".into());
                    }
                    g.to_discard(&id, "zhongjian")?;
                    g.push_log(format!(
                        "忠鉴：弃置 {} 的【{}】",
                        g.seats[target].name,
                        label(g, &id)
                    ));
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("忠鉴：未知步骤 {other}")),
    }
}

fn resolve_zhongjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let own = a.data["own"].as_str().unwrap_or_default().to_string();
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let picks: Vec<CardId> = a.data["picks"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let own_suit_red = g.is_red(&own);
    let own_rank = g.cards.get(&own).map(|c| c.rank).unwrap_or(0);
    let mut same_color = false;
    let mut same_rank = false;
    for id in &picks {
        if g.is_red(id) == own_suit_red {
            same_color = true;
        }
        if g.cards.get(id).map(|c| c.rank).unwrap_or(0) == own_rank {
            same_rank = true;
        }
    }
    g.push_log(format!(
        "忠鉴：展示【{}】；{} 展示 {} 张：{}",
        label(g, &own),
        g.seats[target].name,
        picks.len(),
        if picks.is_empty() {
            "（无）".to_string()
        } else {
            picks.iter().map(|id| label(g, id)).collect::<Vec<_>>().join("、")
        }
    ));
    g.bump("turn.zhongjian_used");
    if same_rank {
        g.bump("turn.zhongjian_extra");
        g.push_log("忠鉴：点数相同 → 本回合本技能改为限两次".to_string());
    }
    if !same_color && !same_rank {
        g.bump("turn.zhongjian_penalty");
        g.push_log("忠鉴：皆不包含 → 本回合手牌上限 -1".to_string());
        return Ok(Step::Done);
    }
    if same_color {
        g.push_log("忠鉴：颜色相同".to_string());
        return Ok(Step::Next(Stage::Choose, json!({ "step": "reward" })));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 才识（辛宪英）
// ---------------------------------------------------------------------------

pub const CAISHI: SkillDef = SkillDef {
    code: "caishi",
    name: "才识",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Draw),
    priority: 0,
    can_active: None,
    handler: caishi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn caishi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if g.answer.is_none() {
        if g.counter("phase.caishi_offered") > 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.caishi_offered");
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("caishi");
            b = b.option("opt1", "本回合手牌上限 +1");
            if g.seats[seat].hp < g.seats[seat].max_hp {
                b = b.option("opt2", "回复 1 点体力，然后本回合不能对自己使用牌");
            }
            Ok(Step::Need(b.escape("不发动才识").build()))
        }
        Some("opt1") => {
            g.bump("turn.caishi_limit");
            g.push_log("才识：本回合手牌上限 +1".to_string());
            Ok(Step::Done)
        }
        Some("opt2") => {
            if g.seats[seat].hp >= g.seats[seat].max_hp {
                return Err("才识：体力已满，不能选择第 2 项".into());
            }
            flow::heal(g, seat, 1, "caishi");
            g.seats[seat].flags.insert("caishi_no_self".to_string());
            g.push_log("才识：回复 1 点体力，本回合不能对自己使用牌".to_string());
            Ok(Step::Done)
        }
        Some("pass") => Ok(Step::Done),
        Some(other) => Err(format!("才识：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 福绵（吴苋）
// ---------------------------------------------------------------------------

pub const FUMIAN: SkillDef = SkillDef {
    code: "fumian",
    name: "福绵",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 0,
    can_active: None,
    handler: fumian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 本回合的福绵数值：切换选项时 +1，下一回合重置为 1（用户 2026-11-09 裁定）。
fn fumian_next_value(g: &Game, seat: usize, mode: u32) -> u32 {
    let last = seat_counter(g, seat, "fumian_mode");
    if last != 0 && last != mode {
        2
    } else {
        1
    }
}

fn fumian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if g.answer.is_none() {
        if g.counter("phase.fumian_offered") > 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.fumian_offered");
    }
    let v1 = fumian_next_value(g, seat, 1);
    let v2 = fumian_next_value(g, seat, 2);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("fumian");
            b = b.option("opt1", format!("摸牌阶段多摸 {v1} 张牌"));
            b = b.option(
                "opt2",
                format!("本回合限一次：红色基本牌/普通锦囊可多指定 {v2} 个目标（无距离）"),
            );
            Ok(Step::Need(b.escape("不发动福绵").build()))
        }
        Some("opt1") => {
            set_seat_counter(g, seat, "fumian_mode", 1);
            g.counters.insert("turn.fumian_draw".to_string(), v1);
            g.push_log(format!("福绵：本回合摸牌阶段多摸 {v1} 张"));
            Ok(Step::Done)
        }
        Some("opt2") => {
            set_seat_counter(g, seat, "fumian_mode", 2);
            g.counters.insert("turn.fumian_multi".to_string(), v2);
            g.push_log(format!("福绵：本回合可多指定 {v2} 个目标（无距离）"));
            Ok(Step::Done)
        }
        Some("pass") => Ok(Step::Done),
        Some(other) => Err(format!("福绵：未知选项 {other}")),
    }
}

/// 福绵①：摸牌阶段多摸张数。
pub fn fumian_draw_bonus(g: &Game) -> usize {
    g.counter("turn.fumian_draw") as usize
}

/// 福绵②：可额外指定的目标数（红色基本牌 / 红色普通锦囊，本回合限一次）。
pub fn extra_targets(g: &Game, seat: usize, card: Option<&CardId>) -> usize {
    if !active(g, seat, "fumian") {
        return 0;
    }
    let left = g.counter("turn.fumian_multi") as usize;
    if left == 0 {
        return 0;
    }
    let Some(id) = card else { return 0 };
    let def = g.def_of(id);
    let eligible = matches!(def.kind, CardKind::Basic) || (def.kind == CardKind::Trick && !def.tags.delayed);
    if !eligible || !g.is_red(id) {
        return 0;
    }
    left
}

/// 消费福绵②的机会（用掉一次）。
pub fn consume_extra_targets(g: &mut Game, seat: usize) {
    if active(g, seat, "fumian") && g.counter("turn.fumian_multi") > 0 {
        g.counters.insert("turn.fumian_multi".to_string(), 0);
        g.push_log("福绵：本回合多指定目标的机会已用".to_string());
    }
}

// ---------------------------------------------------------------------------
// 怠宴（吴苋）
// ---------------------------------------------------------------------------

pub const DAIYAN: SkillDef = SkillDef {
    code: "daiyan",
    name: "怠宴",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: daiyan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn daiyan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if g.answer.is_none() {
        if g.counter("phase.daiyan_offered") > 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.daiyan_offered");
        // 自己的回合序号（用于判定「上一个回合」）。
        bump_seat_counter(g, seat, "daiyan_my_turn");
    }
    match g.answer.take().as_deref() {
        None => {
            if other_alive(g, seat).is_empty() {
                return Ok(Step::Done);
            }
            let mut b = builder(seat, "target").skill("daiyan");
            for i in other_alive(g, seat) {
                b = b.option(
                    format!("target:{i}"),
                    format!("令 {} 获得牌堆中一张红桃基本牌", g.seats[i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动怠宴").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("怠宴：目标不合法 {act}"))?;
            if t == seat || g.seats[t].hp <= 0 {
                return Err("怠宴：目标不合法".into());
            }
            let found = find_in_piles(g, |id| {
                g.cards.get(id).is_some_and(|c| {
                    c.suit == crate::kernel::Suit::Heart && g.def_of(id).kind == CardKind::Basic
                })
            });
            match found {
                Some(id) => {
                    let l = label(g, &id);
                    // 从牌堆/弃牌堆进入手牌：走统一「获得」入口（mechanics card-gained）。
                    if g.zone_of(&id) == Some(&Zone::Draw) {
                        g.to_processing(&id)?;
                    }
                    g.gain(&id, t)?;
                    g.push_log(format!("怠宴：{} 获得【{l}】", g.seats[t].name));
                }
                None => g.push_log("怠宴：牌堆与弃牌堆都没有红桃基本牌".to_string()),
            }
            // 「上一个回合」判定：自己的上一次回合（daiyan_my_turn - 1）且目标相同。
            let prev_target = seat_counter(g, seat, "daiyan_target");
            let prev_turn = seat_counter(g, seat, "daiyan_my_turn_at");
            let my_turn = seat_counter(g, seat, "daiyan_my_turn");
            if prev_target == t as u32 + 1 && prev_turn + 1 == my_turn {
                g.push_log(format!(
                    "怠宴：上回合也对其发动过 → {} 失去 1 点体力",
                    g.seats[t].name
                ));
                flow::lose_hp(g, t, 1, "daiyan");
            }
            set_seat_counter(g, seat, "daiyan_target", t as u32 + 1);
            set_seat_counter(g, seat, "daiyan_my_turn_at", my_turn);
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 问卦（徐氏）
// ---------------------------------------------------------------------------

pub const WENGUA: SkillDef = SkillDef {
    code: "wengua",
    name: "问卦",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_wengua),
    handler: wengua,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn wengua_owner(g: &Game, seat: usize) -> Option<usize> {
    other_alive(g, seat)
        .into_iter()
        .find(|i| active(g, *i, "wengua"))
}

fn can_wengua(g: &Game, seat: usize) -> bool {
    play_phase(g, seat)
        && g.counter(&format!("turn.wengua_used:{seat}")) == 0
        && !g.hand(seat).is_empty()
        && wengua_owner(g, seat).is_some()
}

fn wengua(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let Some(owner) = wengua_owner(g, seat) else {
        return Ok(Step::Done);
    };
    let step = a.data["step"].as_str().unwrap_or("card").to_string();
    match step.as_str() {
        "card" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(seat, "choice").skill("wengua");
                for id in g.hand(seat) {
                    b = b.option(
                        format!("choose:{id}"),
                        format!("交给徐氏【{}】", label(g, &id)),
                    );
                }
                Ok(Step::Need(b.escape("不发动问卦").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("问卦：未知选项 {act}"))?
                    .to_string();
                if !g.hand(seat).contains(&id) {
                    return Err("问卦：只能交出自己的手牌".into());
                }
                g.gain(&id, owner)?;
                g.push_log(format!(
                    "问卦：{} 交给徐氏【{}】",
                    g.seats[seat].name,
                    label(g, &id)
                ));
                Ok(Step::Next(
                    Stage::Mode,
                    json!({ "step": "place", "owner": owner, "card": id }),
                ))
            }
        },
        "place" => {
            let id = a.data["card"].as_str().unwrap_or_default().to_string();
            let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("wengua");
                    b = b.option("top", "置于牌堆顶（双方从牌堆底摸一张）");
                    b = b.option("bottom", "置于牌堆底（双方从牌堆顶摸一张）");
                    Ok(Step::Need(b.escape("置底（默认）").build()))
                }
                Some(ans) => {
                    if !g.hand(owner).contains(&id) {
                        return Err("问卦：该牌已不在徐氏手中".into());
                    }
                    let top = ans != "bottom";
                    if top {
                        g.put_on_top(&id)?;
                    } else {
                        g.put_on_bottom(&id)?;
                    }
                    g.push_log(format!(
                        "问卦：将【{}】置于牌堆{}",
                        label(g, &id),
                        if top { "顶" } else { "底" }
                    ));
                    if top {
                        g.draw_bottom(owner, 1);
                        g.draw_bottom(seat, 1);
                    } else {
                        g.draw(owner, 1);
                        g.draw(seat, 1);
                    }
                    g.bump(&format!("turn.wengua_used:{seat}"));
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("问卦：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 伏诛（徐氏）
// ---------------------------------------------------------------------------

pub const FUZHU: SkillDef = SkillDef {
    code: "fuzhu",
    name: "伏诛",
    triggers: &[EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: fuzhu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn fuzhu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    let current = g.turn;
    if step == "ask" {
        // 「每名男性角色的结束阶段」。
        if phase_cause(a) != "End" || current == owner || !g.seats[current].is_male() {
            return Ok(Step::Done);
        }
        if g.answer.is_none() {
            if g.counter("phase.fuzhu_offered") > 0 {
                return Ok(Step::Done);
            }
            if g.ids_in(|z| *z == Zone::Draw).len() > (g.seats[owner].hp.max(0) as usize) * 10 {
                return Ok(Step::Done);
            }
            g.bump("phase.fuzhu_offered");
        }
        let limit = g.seats.len();
        let cards: Vec<CardId> = g
            .ids_in(|z| *z == Zone::Draw)
            .into_iter()
            .filter(|id| g.def_of(id).tags.sha)
            .take(limit)
            .collect();
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("fuzhu");
                b = b.option(
                    "opt1",
                    format!(
                        "依次对 {} 使用牌堆中的 {} 张【杀】",
                        g.seats[current].name,
                        cards.len()
                    ),
                );
                Ok(Step::Need(b.escape("不发动伏诛").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("opt1") => Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "loop", "cards": cards, "cursor": 0, "target": current }),
            )),
            Some(other) => Err(format!("伏诛：未知选项 {other}")),
        }
    } else {
        // loop：逐张「使用牌堆中的【杀】」（真实结算：出闪/防具/属性）。
        let cards: Vec<CardId> = a.data["cards"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        if target >= g.seats.len() || g.seats[target].hp <= 0 {
            return finish_fuzhu(g, owner);
        }
        while cursor < cards.len() {
            let id = cards[cursor].clone();
            cursor += 1;
            if g.zone_of(&id) != Some(&Zone::Draw) {
                continue;
            }
            let def = g.def_of(&id);
            if !def.tags.sha {
                continue;
            }
            let name = g.name_of(&id);
            g.use_as(owner, &id, &name, vec![target], false, None)?;
            g.push_log(format!(
                "伏诛：对 {} 使用牌堆中的【{}】",
                g.seats[target].name,
                label(g, &id)
            ));
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "loop", "cards": cards, "cursor": cursor, "target": target }),
            ));
        }
        finish_fuzhu(g, owner)
    }
}

fn finish_fuzhu(g: &mut Game, owner: usize) -> Result<Step, String> {
    g.shuffle_draw();
    g.push_log(format!("伏诛：{} 洗牌", g.seats[owner].name));
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 守玺（曹节）——【杀】目标介入
// ---------------------------------------------------------------------------

pub const SHOUXI: SkillDef = SkillDef {
    code: "shouxi",
    name: "守玺",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: shouxi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn shouxi_names(g: &Game, seat: usize) -> Vec<&'static str> {
    cards::non_equip_names()
        .into_iter()
        .filter(|n| seat_counter(g, seat, &format!("shouxi_used:{n}")) == 0)
        .collect()
}

fn shouxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let step = a.data["step"].as_str().unwrap_or("declare").to_string();
    match step.as_str() {
        "declare" => {
            let names = shouxi_names(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("shouxi");
                    for n in &names {
                        b = b.option(format!("choose:{n}"), format!("声明【{}】", crate::cards::label_of(n)));
                    }
                    Ok(Step::Need(b.escape("不发动守玺").build()))
                }
                Some("pass") => {
                    g.sha_intercept = Some(ShaIntercept::Continue);
                    Ok(Step::Done)
                }
                Some(act) => {
                    let name = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("守玺：未知选项 {act}"))?
                        .to_string();
                    if !names.contains(&name.as_str()) {
                        return Err("守玺：该牌名不可声明".into());
                    }
                    set_seat_counter(g, owner, &format!("shouxi_used:{name}"), 1);
                    g.push_log(format!(
                        "守玺：声明【{}】",
                        crate::cards::label_of(&name)
                    ));
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "choice", "user": user, "name": name }),
                    ))
                }
            }
        }
        "choice" => {
            let name = a.data["name"].as_str().unwrap_or_default().to_string();
            let has = g
                .hand(user)
                .into_iter()
                .any(|id| g.name_of(&id) == name);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(user, "choice").skill("shouxi");
                    if has {
                        b = b.option(
                            "opt1",
                            format!(
                                "弃置一张【{}】，然后获得曹节一张牌",
                                crate::cards::label_of(&name)
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_with("pass", "令此【杀】对曹节无效").build(),
                    ))
                }
                Some("opt1") => {
                    if !has {
                        return Err("守玺：你没有该牌名的牌".into());
                    }
                    let id = g
                        .hand(user)
                        .into_iter()
                        .find(|id| g.name_of(id) == name)
                        .ok_or_else(|| "守玺：你没有该牌名的牌".to_string())?;
                    g.to_discard(&id, "shouxi")?;
                    g.push_log(format!(
                        "守玺：{} 弃置【{}】",
                        g.seats[user].name,
                        label(g, &id)
                    ));
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "take", "user": user }),
                    ))
                }
                Some(_) => {
                    g.push_log(format!(
                        "守玺：{} 令此【杀】对曹节无效",
                        g.seats[user].name
                    ));
                    g.sha_intercept = Some(ShaIntercept::Immune(owner));
                    Ok(Step::Done)
                }
            }
        }
        "take" => {
            let user = a.data["user"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let ids = hand_and_equip(g, owner);
                    if ids.is_empty() {
                        g.sha_intercept = Some(ShaIntercept::Continue);
                        return Ok(Step::Done);
                    }
                    let mut b = builder(owner, "choice").skill("shouxi");
                    for id in &ids {
                        b = b.option(
                            format!("give:{id}"),
                            format!("交给使用者【{}】", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不交出牌").build()))
                }
                Some("pass") => {
                    g.sha_intercept = Some(ShaIntercept::Continue);
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("give:")
                        .ok_or_else(|| format!("守玺：未知选项 {act}"))?
                        .to_string();
                    if !hand_and_equip(g, owner).contains(&id) {
                        return Err("守玺：该牌不在你的区域".into());
                    }
                    g.gain(&id, user)?;
                    g.push_log(format!(
                        "守玺：{} 获得曹节【{}】",
                        g.seats[user].name,
                        label(g, &id)
                    ));
                    g.sha_intercept = Some(ShaIntercept::Continue);
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("守玺：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 惠民（曹节）
// ---------------------------------------------------------------------------

pub const HUIMIN: SkillDef = SkillDef {
    code: "huimin",
    name: "惠民",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: huimin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn huimin_x(g: &Game) -> usize {
    (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0 && g.hand(*i).len() < g.seats[*i].hp.max(0) as usize)
        .count()
}

fn huimin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        let x = huimin_x(g);
        if g.answer.is_none() {
            if g.counter("phase.huimin_offered") > 0 || x == 0 {
                return Ok(Step::Done);
            }
            g.bump("phase.huimin_offered");
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(seat, "target").skill("huimin");
                for i in (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0) {
                    b = b.option(format!("target:{i}"), format!("从 {} 开始", g.seats[i].name));
                }
                Ok(Step::Need(b.escape("不发动惠民").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let start: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("惠民：未知选项 {act}"))?;
                let ids = g.draw(seat, x);
                for id in &ids {
                    g.to_frame(id)?;
                    g.reveal(id, Some(seat), "huimin");
                }
                g.push_log(format!(
                    "惠民：{} 摸 {x} 张并展示",
                    g.seats[seat].name
                ));
                let mut order = vec![start];
                order.extend(g.seats_from_left_of(start).into_iter().filter(|i| *i != start));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "step": "pick",
                        "pool": ids,
                        "order": order,
                        "cursor": 0,
                    }),
                ))
            }
        }
    } else {
        let mut pool: Vec<CardId> = a.data["pool"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let order: Vec<usize> = a.data["order"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
        while cursor < order.len() && !pool.is_empty() {
            let actor = order[cursor];
            if g.seats[actor].hp <= 0 {
                cursor += 1;
                continue;
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(actor, "choice").skill("huimin");
                    for id in &pool {
                        b = b.option(format!("pick:{id}"), format!("获得【{}】", label(g, id)));
                    }
                    return Ok(Step::Need(b.escape("放弃（不取）").build()));
                }
                Some(ans) => {
                    if ans != "pass" {
                        let id = ans
                            .strip_prefix("pick:")
                            .ok_or_else(|| format!("惠民：未知选项 {ans}"))?
                            .to_string();
                        if !pool.contains(&id) {
                            return Err("惠民：该牌不在展示区".into());
                        }
                        g.gain(&id, actor)?;
                        g.conceal(&id, "huimin");
                        g.push_log(format!(
                            "惠民：{} 获得【{}】",
                            g.seats[actor].name,
                            label(g, &id)
                        ));
                        pool.retain(|c| *c != id);
                    }
                    cursor += 1;
                }
            }
        }
        for id in pool {
            g.conceal(&id, "huimin");
            g.to_discard(&id, "huimin")?;
        }
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 清弦（嵇康）+ 四条子技能（授予）
// ---------------------------------------------------------------------------

pub const QINGXIAN: SkillDef = SkillDef {
    code: "qingxian",
    name: "清弦",
    triggers: &[EventKind::DamageAfter, EventKind::Healed],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qingxian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 触发来源：受伤（伤害来源）/ 回复（一名其他角色，由嵇康选）。
fn qingxian_trigger_kind(a: &Ask) -> Option<usize> {
    let ev = a.data["event"].as_str().unwrap_or_default();
    if ev.starts_with("DamageAfter") {
        // `DamageAfter` 事件：seat = 受害者，cause = 伤害来源座位。
        let victim = a.data["seat"].as_u64().map(|n| n as usize)?;
        if victim != a.actor {
            return None;
        }
        a.data["cause"]
            .as_str()
            .and_then(|c| c.parse::<usize>().ok())
    } else if ev.starts_with("Healed") {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return None; // 只有自己回复体力才触发
        }
        Some(usize::MAX) // 标记：需要另选一名其他角色
    } else {
        None
    }
}

fn qingxian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        let Some(kind) = qingxian_trigger_kind(a) else {
            return Ok(Step::Done);
        };
        if someone_dying(g) {
            return Ok(Step::Done);
        }
        if kind != usize::MAX {
            if kind == owner || g.seats[kind].hp <= 0 {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "choose", "target": kind, "fixed": true }),
            ));
        }
        let cands: Vec<usize> = other_alive(g, owner);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("qingxian");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("令 {} 执行一项", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动清弦").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("清弦：目标不合法 {act}"))?;
                if !cands.contains(&t) {
                    return Err("清弦：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "choose", "target": t, "fixed": false }),
                ))
            }
        }
    } else if step == "choose" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(target, "choice").skill("qingxian");
                b = b.option("opt1", "失去 1 点体力，随机使用一张装备牌");
                b = b.option("opt2", "回复 1 点体力，弃置一张装备牌");
                Ok(Step::Need(b.escape("不执行（清弦不生效）").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("opt1") => {
                let used = lose_hp_and_use_equip(g, target, "qingxian")?;
                if used
                    .as_ref()
                    .and_then(|id| g.cards.get(id))
                    .map(|c| c.suit == crate::kernel::Suit::Club)
                    .unwrap_or(false)
                {
                    let n = g.draw(owner, 1).len();
                    g.push_log(format!("清弦：梅花牌 → 嵇康摸 {n} 张"));
                }
                Ok(Step::Done)
            }
            Some("opt2") => {
                flow::heal(g, target, 1, "qingxian");
                if equip_ids(g, target).is_empty() {
                    g.push_log(format!("{} 没有装备牌可弃置", g.seats[target].name));
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "equip", "owner": target, "club_owner": owner }),
                ))
            }
            Some(other) => Err(format!("清弦：未知选项 {other}")),
        }
    } else if step == "equip" {
        let target = a.data["owner"].as_u64().unwrap_or(0) as usize;
        let club_owner = a.data["club_owner"].as_u64().unwrap_or(0) as usize;
        let equips = equip_ids(g, target);
        match g.answer.take().as_deref() {
            None => {
                if equips.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = builder(target, "choice").skill("qingxian");
                for id in &equips {
                    b = b.option(
                        format!("pick:{id}"),
                        format!("弃置【{}】", label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("不弃置（没有选择）").build()))
            }
            Some(ans) => {
                let mut club = false;
                if let Some(id) = ans.strip_prefix("pick:") {
                    let id = id.to_string();
                    if !equips.contains(&id) {
                        return Err("清弦：只能弃置自己的装备牌".into());
                    }
                    club = g.cards.get(&id).map(|c| c.suit == crate::kernel::Suit::Club).unwrap_or(false);
                    g.to_discard(&id, "qingxian")?;
                    g.push_log(format!(
                        "清弦：{} 弃置装备【{}】",
                        g.seats[target].name,
                        label(g, &id)
                    ));
                }
                if club {
                    let n = g.draw(club_owner, 1).len();
                    g.push_log(format!("清弦：梅花牌 → 嵇康摸 {n} 张"));
                }
                Ok(Step::Done)
            }
        }
    } else if step == "club" {
        // opt1 的梅花判定：使用的那张装备牌已在结算；这里按「使用牌」记录判断。
        Ok(Step::Done)
    } else {
        Err(format!("清弦：未知步骤 {step}"))
    }
}

/// 四条子技能（绝响授予）：固定单一效果，无梅花摸牌。
macro_rules! xian_skill {
    ($konst:ident, $code:literal, $name:literal, $trigger:expr, $from_source:expr, $mode:literal) => {
        pub const $konst: SkillDef = SkillDef {
            code: $code,
            name: $name,
            triggers: &[$trigger],
            window: Window::AnyTime,
            priority: 0,
            can_active: None,
            handler: xian_handler,
            dying_hook: None,
            flip_hook: None,
            wuxie_opts: None,
            wuxie_use: None,
            target_blocker: None,
            yang_window: None,
            yin_window: None,
        };
        const _: (bool, &str) = ($from_source, $mode);
    };
}

xian_skill!(JIXIAN, "jixian", "激弦", EventKind::DamageAfter, true, "lose");
xian_skill!(ROUXIAN, "rouxian", "柔弦", EventKind::DamageAfter, true, "heal");
xian_skill!(LIEXIAN, "liexian", "烈弦", EventKind::Healed, false, "lose");
xian_skill!(HEXIAN, "hexian", "和弦", EventKind::Healed, false, "heal");

fn xian_handler(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let code = a.skill.clone();
    let code_static: &'static str = match code.as_str() {
        "hexian" => "hexian",
        "rouxian" => "rouxian",
        "liexian" => "liexian",
        _ => "jixian",
    };
    let lose_mode = matches!(code.as_str(), "jixian" | "liexian");
    let from_source = matches!(code.as_str(), "jixian" | "rouxian");
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        // 时机校验：受伤必须是本人；回复必须是本人。
        let ev = a.data["event"].as_str().unwrap_or_default();
        if ev.starts_with("DamageAfter") {
            let victim = a.data["seat"].as_u64().map(|n| n as usize);
            if victim != Some(owner) {
                return Ok(Step::Done);
            }
        } else if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        if someone_dying(g) {
            return Ok(Step::Done);
        }
        if from_source {
            let src = a
                .data["cause"]
                .as_str()
                .and_then(|c| c.parse::<usize>().ok())
                .unwrap_or(owner);
            if src == owner || g.seats.get(src).map(|s| s.hp <= 0).unwrap_or(true) {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "confirm", "target": src }),
            ));
        }
        let cands: Vec<usize> = other_alive(g, owner);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill(code_static);
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("对 {} 发动", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("{code}：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err(format!("{code}：目标不合法"));
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "confirm", "target": t }),
                ))
            }
        }
    } else if step == "equip" {
        xian_equip_step(g, a)
    } else {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill(code_static);
                b = b.option(
                    "opt1",
                    if lose_mode {
                        format!("令 {} 失去 1 点体力，随机使用一张装备牌", g.seats[target].name)
                    } else {
                        format!("令 {} 回复 1 点体力，弃置一张装备牌", g.seats[target].name)
                    },
                );
                Ok(Step::Need(b.escape("不发动").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("opt1") => {
                if lose_mode {
                    lose_hp_and_use_equip(g, target, &code)?;
                    Ok(Step::Done)
                } else {
                    flow::heal(g, target, 1, &code);
                    if equip_ids(g, target).is_empty() {
                        g.push_log(format!("{} 没有装备牌可弃置", g.seats[target].name));
                        return Ok(Step::Done);
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "equip", "owner": target, "club_owner": usize::MAX }),
                    ))
                }
            }
            Some(other) => Err(format!("{code}：未知选项 {other}")),
        }
    }
}

fn a_skill_static(code: &str) -> &'static str {
    match code {
        "hexian" => "hexian",
        "rouxian" => "rouxian",
        "liexian" => "liexian",
        _ => "jixian",
    }
}

/// 四弦的 equip 处置（与清弦共用 `equip` 步骤，但无梅花摸牌）。
fn xian_equip_step(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let target = a.data["owner"].as_u64().unwrap_or(0) as usize;
    let equips = equip_ids(g, target);
    match g.answer.take().as_deref() {
        None => {
            if equips.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = builder(target, "choice").skill(a_skill_static(&a.skill));
            for id in &equips {
                b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
            }
            Ok(Step::Need(b.escape("不弃置").build()))
        }
        Some(ans) => {
            if let Some(id) = ans.strip_prefix("pick:") {
                let id = id.to_string();
                if !equips.contains(&id) {
                    return Err("只能弃置自己的装备牌".into());
                }
                g.to_discard(&id, "xian")?;
                g.push_log(format!(
                    "{}：{} 弃置装备【{}】",
                    a.skill,
                    g.seats[target].name,
                    label(g, &id)
                ));
            }
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 绝响（嵇康）
// ---------------------------------------------------------------------------

pub const JUEXIANG: SkillDef = SkillDef {
    code: "juexiang",
    name: "绝响",
    triggers: &[EventKind::Death, EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: juexiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(juexiang_block),
    yang_window: None,
    yin_window: None,
};

fn juexiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default();
    if ev.starts_with("PhaseStart") {
        // 自己回合开始时解除「梅花保护」。
        if g.turn == owner {
            g.seats[owner].flags.remove("juexiang_guard");
        }
        return Ok(Step::Done);
    }
    if g.answer.is_none() {
        if g.counter("phase.juexiang_offered") > 0 {
            return Ok(Step::Done);
        }
        g.bump("phase.juexiang_offered");
    }
    let cands: Vec<usize> = other_alive(g, owner);
    match g.answer.take().as_deref() {
        None => {
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = builder(owner, "target").skill("juexiang");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("令 {} 随机获得一条弦技", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动绝响").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("绝响：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("绝响：目标不合法".into());
            }
            const XIAN: [&str; 4] = ["hexian", "rouxian", "liexian", "jixian"];
            let pick = g.rand_below(4);
            let code = XIAN[pick];
            crate::skills::grant_skill(g, t, code);
            g.seats[t].flags.insert("juexiang_guard".to_string());
            g.push_log(format!(
                "绝响：{} 随机获得「{}」；直到其下回合开始不能成为其他角色使用的梅花牌目标",
                g.seats[t].name,
                crate::skills::def(code).map(|d| d.name).unwrap_or(code)
            ));
            Ok(Step::Done)
        }
    }
}

/// 梅花牌目标拦截（绝响保护）。
fn juexiang_block(g: &Game, target: usize, _source: usize, card: &str) -> bool {
    if !g.seats[target].flags.contains("juexiang_guard") {
        return false;
    }
    card_suit_is_club(g, card)
}

fn card_suit_is_club(g: &Game, name: &str) -> bool {
    g.cards
        .values()
        .find(|c| c.name == name)
        .map(|c| c.suit == crate::kernel::Suit::Club)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 谏征（秦宓）——使用【杀】指定目标时的第三方介入
// ---------------------------------------------------------------------------

pub const JIANZHENG: SkillDef = SkillDef {
    code: "jianzheng",
    name: "谏征",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jianzheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn jianzheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let card = a.data["card"].as_str().map(|s| s.to_string());
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    if step == "ask" {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jianzheng");
                b = b.option("opt1", "将一张手牌置于牌堆顶，取消此【杀】的目标");
                Ok(Step::Need(b.escape("不发动谏征").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some("opt1") => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "card", "user": user, "card": card }),
            )),
            Some(other) => Err(format!("谏征：未知选项 {other}")),
        }
    } else {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jianzheng");
                for id in g.hand(owner) {
                    b = b.option(
                        format!("choose:{id}"),
                        format!("置于牌堆顶【{}】", label(g, &id)),
                    );
                }
                Ok(Step::Need(b.escape("不发动谏征").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("谏征：未知选项 {act}"))?
                    .to_string();
                if !g.hand(owner).contains(&id) {
                    return Err("谏征：只能置自己的手牌".into());
                }
                g.put_on_top(&id)?;
                g.push_log(format!(
                    "谏征：{} 将【{}】置于牌堆顶，取消此【杀】的目标",
                    g.seats[owner].name,
                    label(g, &id)
                ));
                let black = card
                    .as_deref()
                    .and_then(|c| g.cards.get(c))
                    .map(|c| !c.suit.is_red())
                    .unwrap_or(false);
                if black {
                    g.sha_intercept = Some(ShaIntercept::Replace(Vec::new()));
                } else {
                    g.push_log(format!("谏征：此【杀】不为黑色 → {} 成为目标", g.seats[owner].name));
                    g.sha_intercept = Some(ShaIntercept::Replace(vec![owner]));
                }
                let _ = user;
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 专对（秦宓）——拼点
// ---------------------------------------------------------------------------

pub const ZHUANDUI: SkillDef = SkillDef {
    code: "zhuandui",
    name: "专对",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhuandui,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhuandui(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    if step == "ask" {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("zhuandui");
                b = b.option("opt1", "拼点");
                Ok(Step::Need(b.escape("不发动专对").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some("opt1") => {
                let other = if owner == user { target } else { user };
                pindian::start(g, owner, other, "zhuandui");
                Ok(Step::Next(Stage::PindianResp, json!({ "step": "wait" })))
            }
            Some(other) => Err(format!("专对：未知选项 {other}")),
        }
    } else {
        // 读取拼点结果。
        let Some(result) = g.pindian_result.take() else {
            return Err("专对：缺少拼点结果".into());
        };
        if !result.initiator_won() {
            g.sha_intercept = Some(ShaIntercept::Continue);
            return Ok(Step::Done);
        }
        if owner == user {
            g.push_log(format!("专对：拼点赢 → {} 不能响应此【杀】", g.seats[target].name));
            g.sha_intercept = Some(ShaIntercept::Unresponsive(target));
        } else {
            g.push_log("专对：拼点赢 → 此【杀】对秦宓无效".to_string());
            g.sha_intercept = Some(ShaIntercept::Immune(owner));
        }
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 天辩（秦宓）——拼点钩子（被动，无独立窗口）
// ---------------------------------------------------------------------------

pub const TIANBIAN: SkillDef = SkillDef {
    code: "tianbian",
    name: "天辩",
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

pub fn pindian_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !active(g, seat, "tianbian") {
        return Vec::new();
    }
    if g.ids_in(|z| *z == Zone::Draw).is_empty() {
        return Vec::new();
    }
    vec![(
        "deck_top".to_string(),
        "天辩：用牌堆顶的牌拼点".to_string(),
    )]
}

pub fn pindian_take(g: &mut Game, seat: usize, option: &str) -> Result<Option<CardId>, String> {
    if option != "deck_top" {
        return Ok(None);
    }
    if !active(g, seat, "tianbian") {
        return Err("天辩：没有该技能".into());
    }
    let Some(id) = g.draw_top() else {
        return Err("天辩：牌堆为空".into());
    };
    g.to_processing(&id)?;
    g.push_log(format!(
        "天辩：以牌堆顶【{}】拼点",
        label(g, &id)
    ));
    Ok(Some(id))
}

pub fn pindian_rank(g: &Game, seat: usize, card: &CardId) -> u8 {
    let rank = g.cards.get(card).map(|c| c.rank).unwrap_or(0);
    if active(g, seat, "tianbian")
        && g.cards
            .get(card)
            .is_some_and(|c| c.suit == crate::kernel::Suit::Heart)
    {
        return 13;
    }
    rank
}

// ---------------------------------------------------------------------------
// 复难（薛综）
// ---------------------------------------------------------------------------

pub const FUNAN: SkillDef = SkillDef {
    code: "funan",
    name: "复难",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: funan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn funan_upgraded(g: &Game, seat: usize) -> bool {
    g.seats[seat].flags.contains("funan_upgraded")
}

fn funan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let used = a.data["used"].as_str().map(|s| s.to_string());
    let response = a.data["response"].as_str().map(|s| s.to_string());
    let responder = a.data["responder"].as_u64().unwrap_or(0) as usize;
    let upgraded = funan_upgraded(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("funan");
            if upgraded {
                b = b.option("opt1", "复难plus：获得其响应时使用或打出的牌");
            } else {
                b = b.option(
                    "opt1",
                    "令其获得你使用的牌（本回合不能用或打出），你获得其响应牌",
                );
            }
            Ok(Step::Need(b.escape("不发动复难").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("opt1") => {
            if let (Some(resp), false) = (response.clone(), upgraded) {
                if let (Some(used), true) = (used.clone(), !upgraded) {
                    if g.zone_of(&used) == Some(&Zone::Discard) {
                        g.gain(&used, responder)?;
                        // 本回合不能使用或打出此牌（手牌冻结，按牌 id）。
                        g.seats[responder].frozen.insert(used.clone());
                        g.push_log(format!(
                            "复难：{} 获得【{}】（本回合不能使用或打出）",
                            g.seats[responder].name,
                            label(g, &used)
                        ));
                    }
                }
                if g.zone_of(&resp) == Some(&Zone::Discard) {
                    g.gain(&resp, owner)?;
                    g.push_log(format!(
                        "复难：{} 获得【{}】",
                        g.seats[owner].name,
                        label(g, &resp)
                    ));
                }
            } else if let Some(resp) = response {
                if g.zone_of(&resp) == Some(&Zone::Discard) {
                    g.gain(&resp, owner)?;
                    g.push_log(format!(
                        "复难plus：{} 获得【{}】",
                        g.seats[owner].name,
                        label(g, &resp)
                    ));
                }
            }
            Ok(Step::Done)
        }
        Some(other) => Err(format!("复难：未知选项 {other}")),
    }
}

/// 内核响应出口钩子：其他角色响应「你使用的牌」后询问复难。
pub fn on_response_played(
    g: &mut Game,
    used: Option<&CardId>,
    responder: usize,
    response: &CardId,
) {
    let Some(used) = used else { return };
    // 使用者 = 该牌的来源（响应发生时响应牌在处理区/弃牌堆，使用者信息从当前结算帧取）。
    let user = g
        .stack
        .iter()
        .rev()
        .find_map(|f| match f {
            Frame::Flow(ff) if ff.kind == FlowKind::Card => Some(ff.actor),
            _ => None,
        })
        .unwrap_or(responder);
    if user == responder {
        return;
    }
    if !active(g, user, "funan") {
        return;
    }
    if g.counter("phase.funan_offered") > 0 {
        return;
    }
    g.bump("phase.funan_offered");
    crate::kernel::resolve::push_ask(
        g,
        "funan",
        Stage::Choose,
        user,
        json!({
            "used": used.clone(),
            "response": response.clone(),
            "responder": responder,
        }),
    );
}

// ---------------------------------------------------------------------------
// 诫训（薛综）
// ---------------------------------------------------------------------------

pub const JIEXUN: SkillDef = SkillDef {
    code: "jiexun",
    name: "诫训",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: jiexun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 场上**公开**的方片牌数（装备区 + 判定区；不含手牌与武将牌上的牌）。
fn public_diamonds(g: &Game) -> usize {
    let mut n = 0;
    for seat in 0..g.seats.len() {
        for id in equip_ids(g, seat).into_iter().chain(g.judge_zone(seat)) {
            if g.cards.get(&id).map(|c| c.suit == crate::kernel::Suit::Diamond).unwrap_or(false) {
                n += 1;
            }
        }
    }
    n
}

fn jiexun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    if step == "ask" {
        if g.answer.is_none() {
            if g.counter("phase.jiexun_offered") > 0 {
                return Ok(Step::Done);
            }
            g.bump("phase.jiexun_offered");
        }
        let cands: Vec<usize> = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = builder(owner, "target").skill("jiexun");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("对 {} 发动诫训", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动诫训").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("诫训：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("诫训：目标不合法".into());
                }
                let x = public_diamonds(g);
                if x > 0 {
                    let n = g.draw(t, x).len();
                    g.push_log(format!(
                        "诫训：{} 摸 {n} 张（场上方片牌 {x} 张）",
                        g.seats[t].name
                    ));
                } else {
                    g.push_log("诫训：场上没有方片牌，不摸牌".to_string());
                }
                // Y = 本技能自游戏开始至今的发动次数（从 1 开始）。
                let y = g.counter("jiexun_count") + 1;
                g.bump("jiexun_count");
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "discard", "target": t, "left": y }),
                ))
            }
        }
    } else {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let left = a.data["left"].as_u64().unwrap_or(0) as u64;
        if left == 0 {
            return finish_jiexun(g, owner, target);
        }
        match g.answer.take().as_deref() {
            None => {
                let ids = hand_and_equip(g, target);
                if ids.is_empty() {
                    return finish_jiexun(g, owner, target);
                }
                let mut b = builder(target, "choice").skill("jiexun");
                for id in &ids {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("停止弃置（没有更多牌）").build()))
            }
            Some("pass") => finish_jiexun(g, owner, target),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("诫训：未知选项 {act}"))?
                    .to_string();
                if !hand_and_equip(g, target).contains(&id) {
                    return Err("诫训：该牌不在其区域".into());
                }
                g.to_discard(&id, "jiexun")?;
                g.push_log(format!(
                    "诫训：{} 弃置【{}】",
                    g.seats[target].name,
                    label(g, &id)
                ));
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "discard", "target": target, "left": left - 1 }),
                ))
            }
        }
    }
}

fn finish_jiexun(g: &mut Game, owner: usize, target: usize) -> Result<Step, String> {
    let empty = g.hand(target).is_empty() && equip_ids(g, target).is_empty();
    if empty {
        g.seats[owner].skills.retain(|s| s != "jiexun");
        g.seats[owner].flags.insert("funan_upgraded".to_string());
        crate::skills::rebuild_subs(g);
        g.push_log(format!(
            "诫训：{} 的牌已全部失去 → 薛综失去诫训，「复难」升级为「复难plus」",
            g.seats[target].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 辟撰（蔡邕）
// ---------------------------------------------------------------------------

pub const BIZHUAN: SkillDef = SkillDef {
    code: "bizhuan",
    name: "辟撰",
    triggers: &[EventKind::CardUsed, EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: bizhuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn bizhuan_trigger_ok(g: &Game, a: &Ask) -> bool {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default();
    if mark_count(g, owner, "shu") >= 4 {
        return false;
    }
    if ev.starts_with("CardUsed") {
        // 自己使用黑桃牌。
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return false;
        }
        return a
            .data["card"]
            .as_str()
            .and_then(|c| g.cards.get(c))
            .map(|c| !c.suit.is_red() && c.suit == crate::kernel::Suit::Spade)
            .unwrap_or(false);
    }
    if ev.starts_with("TargetChosen") {
        // 成为其他角色黑桃牌的目标。
        let targets = a.data["cause"].as_str().unwrap_or_default();
        if !targets.split(',').any(|t| t.trim() == owner.to_string()) {
            return false;
        }
        if a.data["seat"].as_u64().map(|n| n as usize) == Some(owner) {
            return false;
        }
        return a
            .data["card"]
            .as_str()
            .and_then(|c| g.cards.get(c))
            .map(|c| c.suit == crate::kernel::Suit::Spade)
            .unwrap_or(false);
    }
    false
}

fn bizhuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !bizhuan_trigger_ok(g, a) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("bizhuan");
            b = b.option("opt1", "将牌堆顶一张牌置于武将牌上为「书」");
            Ok(Step::Need(b.escape("不发动辟撰").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("opt1") => {
            let Some(id) = g.draw_top() else {
                return Ok(Step::Done);
            };
            g.to_mark(&id, owner, "shu")?;
            g.reveal(&id, Some(owner), "bizhuan");
            g.push_log(format!(
                "辟撰：将【{}】置于武将牌上为「书」（{} 张）",
                label(g, &id),
                mark_count(g, owner, "shu")
            ));
            Ok(Step::Done)
        }
        Some(other) => Err(format!("辟撰：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 通博（蔡邕）
// ---------------------------------------------------------------------------

pub const TONGBO: SkillDef = SkillDef {
    code: "tongbo",
    name: "通博",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: tongbo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn tongbo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        if g.answer.is_none() {
            if g.counter("phase.tongbo_offered") > 0
                || mark_count(g, owner, "shu") == 0
                || tongbo_own_cards(g, owner).is_empty()
            {
                return Ok(Step::Done);
            }
            g.bump("phase.tongbo_offered");
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("tongbo");
                for (id, label) in tongbo_own_cards(g, owner) {
                    b = b.option(id, label);
                }
                Ok(Step::Need(b.escape("不替换（结束通博）").build()))
            }
            Some("pass") => tongbo_give(g, owner),
            Some(act) => {
                // 槽位校验：选项必须严格对应手牌/装备区（用户 2026-11-09 裁定）。
                let Some((id, _)) = parse_own_option(g, owner, &act) else {
                    return Err(format!("通博：非法牌选项 {act}"));
                };
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "shu", "own": id }),
                ))
            }
        }
    } else if step == "shu" {
        let own = a.data["own"].as_str().unwrap_or_default().to_string();
        match g.answer.take().as_deref() {
            None => {
                let shu = g.mark_cards(owner, "shu");
                let mut b = builder(owner, "choice").skill("tongbo");
                for id in &shu {
                    b = b.option(format!("pick:{id}"), format!("替换「书」【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不替换").build()))
            }
            Some("pass") => tongbo_give(g, owner),
            Some(act) => {
                let shu_id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("通博：未知选项 {act}"))?
                    .to_string();
                if !g.mark_cards(owner, "shu").contains(&shu_id) {
                    return Err("通博：该牌不是你的「书」".into());
                }
                if !tongbo_own_ids(g, owner).contains(&own) {
                    return Err("通博：要替换的牌已不在手牌/装备区".into());
                }
                // 牌 → 「书」；「书」→ **手牌**（原装备区的牌不能进手牌，手牌不能进装备区）。
                let slot_of_own = g.zone_of(&own).cloned();
                g.to_mark(&own, owner, "shu")?;
                g.to_hand(&shu_id, owner)?;
                g.push_log(format!(
                    "通博：{} 用【{}】替换「书」【{}】",
                    g.seats[owner].name,
                    label(g, &own),
                    label(g, &shu_id)
                ));
                let _ = slot_of_own;
                // 继续替换（任意张）。
                if tongbo_own_cards(g, owner).is_empty() {
                    return tongbo_give(g, owner);
                }
                let mut b = builder(owner, "choice").skill("tongbo");
                for (id, label) in tongbo_own_cards(g, owner) {
                    b = b.option(id, label);
                }
                Ok(Step::Need(b.escape("结束替换").build()))
            }
        }
    } else if step == "give" {
        let cands: Vec<usize> = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("tongbo");
                for i in &cands {
                    b = b.option(
                        format!("target:{i}"),
                        format!("将所有「书」交给 {}", g.seats[*i].name),
                    );
                }
                Ok(Step::Need(b.escape("不交出「书」").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("通博：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("通博：目标不合法".into());
                }
                let shu = g.mark_cards(owner, "shu");
                let n = shu.len();
                for id in &shu {
                    g.gain(id, t)?;
                }
                g.push_log(format!(
                    "通博：将 {n} 张「书」交给 {}",
                    g.seats[t].name
                ));
                Ok(Step::Done)
            }
        }
    } else {
        Err(format!("通博：未知步骤 {step}"))
    }
}

/// 通博可替换的牌（手牌 + 装备区，槽位标记严格对应）。
fn tongbo_own_cards(g: &Game, seat: usize) -> Vec<(String, String)> {
    zone_tagged_options(g, seat)
}

fn tongbo_own_ids(g: &Game, seat: usize) -> Vec<CardId> {
    hand_and_equip(g, seat)
}

fn parse_own_option(g: &Game, seat: usize, opt: &str) -> Option<(CardId, Slot)> {
    let rest = opt.strip_prefix("own:")?;
    let mut parts = rest.splitn(3, ':');
    let zone = parts.next()?;
    match zone {
        "hand" => {
            let id = parts.next()?.to_string();
            if g.zone_of(&id) == Some(&Zone::Hand(seat)) {
                Some((id, Slot::Weapon))
            } else {
                None
            }
        }
        "equip" => {
            let slot_name = parts.next()?;
            let id = parts.next()?.to_string();
            let slot = match slot_name {
                "Weapon" => Slot::Weapon,
                "Armor" => Slot::Armor,
                "HorseOffense" => Slot::HorseOffense,
                "HorseDefense" => Slot::HorseDefense,
                "Treasure" => Slot::Treasure,
                _ => return None,
            };
            if g.zone_of(&id) == Some(&Zone::Equip(seat, slot)) {
                Some((id, slot))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 四花色检查通过则进入分配。
fn tongbo_give(g: &mut Game, owner: usize) -> Result<Step, String> {
    let shu = g.mark_cards(owner, "shu");
    let mut suits = std::collections::BTreeSet::new();
    for id in &shu {
        if let Some(c) = g.cards.get(id) {
            suits.insert(match c.suit {
                crate::kernel::Suit::Spade => 1,
                crate::kernel::Suit::Heart => 2,
                crate::kernel::Suit::Club => 3,
                crate::kernel::Suit::Diamond => 4,
            });
        }
    }
    if suits.len() == 4 && !shu.is_empty() && !other_alive(g, owner).is_empty() {
        return Ok(Step::Next(Stage::Target, json!({ "step": "give" })));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 注册表
// ---------------------------------------------------------------------------

pub const B8_SKILLS: &[SkillDef] = &[
    ZHONGJIAN,
    CAISHI,
    FUMIAN,
    DAIYAN,
    WENGUA,
    FUZHU,
    SHOUXI,
    HUIMIN,
    QINGXIAN,
    JUEXIANG,
    JIANZHENG,
    ZHUANDUI,
    TIANBIAN,
    FUNAN,
    JIEXUN,
    BIZHUAN,
    TONGBO,
    JIXIAN,
    ROUXIAN,
    LIEXIAN,
    HEXIAN,
];

// ---------------------------------------------------------------------------
// 内核钩子（内核不认识技能名；这里集中回答「谁在什么时候影响什么」）
// ---------------------------------------------------------------------------

/// 手牌上限修正（才识 +1 / 忠鉴 -1 / 辟撰「书」数）。
pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    let mut d = 0i32;
    if active(g, seat, "caishi") {
        d += g.counter("turn.caishi_limit") as i32;
    }
    if active(g, seat, "zhongjian") {
        d -= g.counter("turn.zhongjian_penalty") as i32;
    }
    if active(g, seat, "bizhuan") {
        d += mark_count(g, seat, "shu") as i32;
    }
    d
}

/// 才识②：本回合不能对自己使用牌（含装备牌）。
pub fn self_use_blocked(g: &Game, seat: usize) -> bool {
    active(g, seat, "caishi") && g.seats[seat].flags.contains("caishi_no_self")
}

/// 【杀】介入候选（守玺 / 谏征 / 专对）。
pub fn sha_intercept_skills(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let is_sha = card.map(|c| g.def_of(c).tags.sha).unwrap_or(false);
    if !is_sha {
        return out;
    }
    // 守玺（曹节）：成为【杀】的目标后。
    if target != user
        && active(g, target, "shouxi")
        && !shouxi_names(g, target).is_empty()
    {
        out.push((target, "shouxi"));
    }
    // 谏征（秦宓）：其他角色使用【杀】指定**其他角色**为目标时，秦宓在使用者攻击范围内。
    for seat in 0..g.seats.len() {
        if seat == user || seat == target || g.seats[seat].hp <= 0 {
            continue;
        }
        if active(g, seat, "jianzheng")
            && g.hand(seat).is_empty() == false
            && g.in_attack_range(user, seat)
        {
            out.push((seat, "jianzheng"));
        }
    }
    // 流离（界大乔）：成为【杀】目标时可弃一张牌转移（目标视角，需有可转移对象）。
    if target != user
        && crate::skills::active(g, target, "liuli")
        && (!g.hand(target).is_empty()
            || g.ids_in(|z| matches!(z, crate::kernel::Zone::Equip(s, _) if *s == target))
                .len()
                > 0)
    {
        let has_dest = (0..g.seats.len()).any(|i| {
            i != target && i != user && g.seats[i].hp > 0 && g.in_attack_range(target, i)
        });
        if has_dest {
            out.push((target, "liuli"));
        }
    }
    // 铁骑（界马超）：使用者视角，令目标本回合非锁定技失效并判定。
    if target != user && crate::skills::active(g, user, "tieqi") {
        out.push((user, "tieqi"));
    }
    // 专对（秦宓）：使用者视角 / 目标视角。
    if target != user && active(g, target, "zhuandui") && !g.hand(target).is_empty() && !g.hand(user).is_empty() {
        out.push((target, "zhuandui"));
    }
    if target != user && active(g, user, "zhuandui") && !g.hand(user).is_empty() && !g.hand(target).is_empty() {
        out.push((user, "zhuandui"));
    }
    // B13：烈弓（不能【闪】/ +1）、享乐（弃基本牌否则无效）。
    out.extend(crate::skills::b13::sha_intercept(g, user, target, card));
    // B14：鞬出（弃目标一张牌）。
    out.extend(crate::skills::b14::sha_intercept(g, user, target, card));
    out
}

/// 每张牌只问一次的介入技能（谏征是「指定目标时」的全局介入）。
pub fn sha_intercept_per_card(code: &str) -> bool {
    code == "jianzheng"
}
