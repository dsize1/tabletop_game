//! B19（界限突破·J7）——按 `align/B19.json` 第 7 段实现。
//!
//! 覆盖：界李儒（灭计/焚城/绝策）、界郭淮（精策）、界伏皇后（惴恐/求援）、
//! 界韩当（弓骑/解烦）、界关兴张苞（父魂）、界马岱（马术/潜袭）、
//! 界郭皇后（矫诏/殚心）、界沮授（矢北/渐营）。
//!
//! 用户裁定（2026-11-10，42 条疑点）要点：
//! - 灭计/焚城：弃牌由目标/本人自选（含装备区），焚城起点由李儒选、数量 ≥ 上家+1。
//! - 精策：摸牌可选；只统计从手牌使用的牌；类型三分（基本/锦囊含延时/装备）。
//! - 惴恐：赢 → 本回合不能对除其以外的角色使用牌（含技能视为使用）；没赢 → 获得其拼点牌并视为对你使用【杀】。
//! - 求援：①同类型（基本/锦囊/装备）不同牌名；②追加目标（群体锦囊不提供②）。
//! - 弓骑：弃牌含自己装备区；攻击范围无限；同花色【杀】不限次数；弃他人牌由韩当选（不含判定区）。
//! - 解烦：投票者各自二选一；武器牌可来自手牌/装备区；第一轮每次回合结束重置（第一轮每回合可发动），之后整局一次。
//! - 父魂：两张手牌当【杀】使用/打出；转化【杀】只能用同色手牌响应（异色不限）；出牌阶段【杀】造成伤害后本回合获得武圣/咆哮。
//! - 潜袭：出牌阶段开始展示（不弃）；距离 1 的所有角色颜色封锁整回合（含新摸牌）；展示牌伤害 +1。
//! - 矫诏：本轮所有角色使用+打出过的牌名不可选（含无懈，不含延时）；修改 1 = 矫诏plus（每轮限一次、回合外可用、任意牌名）；修改 2 = 矫诏plusplus（不要承载牌）。
//! - 殚心：摸 X（=已修改次数）然后修改（最多 2 次，满 2 次后仅摸牌）。
//! - 矢北：开局 3 点护甲；护甲先扣、溢出扣体力；被护甲吸收仍算受伤；首次受伤回 1（满血不回），再次受伤失去 1。
//! - 渐营：①摸牌可选、按当前出牌阶段；②当任意基本牌（区分杀/火杀/雷杀，不含闪），继承上一张牌花色。

use serde_json::json;

use crate::cards::{CardKind, DamageKind, Effect, TargetMode};
use crate::kernel::card_play::ShaIntercept;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};

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
fn suit_code(g: &Game, seat: usize, id: &CardId) -> u32 {
    match crate::skills::effective_suit(g, seat, id) {
        Some(crate::kernel::Suit::Spade) => 1,
        Some(crate::kernel::Suit::Heart) => 2,
        Some(crate::kernel::Suit::Club) => 3,
        Some(crate::kernel::Suit::Diamond) => 4,
        None => 0,
    }
}
fn color_code(g: &Game, _seat: usize, id: &CardId) -> u32 {
    if g.is_red(id) {
        1
    } else {
        2
    }
}
fn weapon_slot_card(g: &Game, seat: usize) -> Option<CardId> {
    g.equip_in(seat, Slot::Weapon)
}
fn hand_weapon_cards(g: &Game, seat: usize) -> Vec<CardId> {
    hand(g, seat)
        .into_iter()
        .filter(|id| g.def_of(id).slot == Some(Slot::Weapon))
        .collect()
}
/// 手牌可用性（冻结/颜色封锁）统一入口。
fn card_usable(g: &Game, seat: usize, id: &CardId) -> bool {
    // 手牌可用性统一入口（冻结 + 颜色封锁）；非该座手牌不由本函数判定。
    !matches!(g.zone_of(id), Some(Zone::Hand(s)) if *s == seat) || g.can_use_card (seat, id)
}
fn others_with_hand(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// 灭计（界李儒）
// ---------------------------------------------------------------------------

pub const MIEJI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "mieji",
    name: "灭计",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_mieji),
    handler: mieji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_mieji(g: &Game, seat: usize) -> bool {
    active(g, seat, "mieji")
        && play_phase(g, seat)
        && g.counter("turn.mieji_used") == 0
        && hand(g, seat)
            .iter()
            .any(|id| g.def_of(id).kind == CardKind::Trick)
        && !others_with_hand(g, seat).is_empty()
}

fn mieji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;

    if stage.is_empty() {
        if !can_mieji(g, owner) {
            return Ok(Step::Done);
        }
        let tricks: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| g.def_of(id).kind == CardKind::Trick)
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("mieji");
                for id in &tricks {
                    b = b.option(
                        format!("choose:{id}"),
                        format!("灭计：将【{}】置于牌堆顶", label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("不发动灭计").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("灭计：未知选项 {act}"))?
                    .to_string();
                if !tricks.contains(&id) {
                    return Err("灭计：需要一张锦囊牌".into());
                }
                g.put_on_top(&id)?;
                g.bump("turn.mieji_used");
                g.push_log(format!(
                    "灭计：将【{}】置于牌堆顶",
                    label(g, &id)
                ));
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "stage": "target", "card": id }),
                ))
            }
        };
    }

    if stage == "target" {
        let cands = others_with_hand(g, owner);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("mieji");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("令 {} 选择", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(
                    b.escape("放弃选择目标（自动选择第一名）").build(),
                ))
            }
            Some("pass") => {
                // 强制选择：自动选第一名候选（保持 pending 出口不变量）。
                let t = cands[0];
                push(
                    g,
                    "mieji",
                    t,
                    json!({ "stage": "choice", "card": card, "user": owner }),
                );
                Ok(Step::Done)
            }
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("灭计：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("灭计：目标不合法".into());
                }
                push(
                    g,
                    "mieji",
                    t,
                    json!({ "stage": "choice", "card": card, "user": owner }),
                );
                Ok(Step::Done)
            }
        };
    }

    if stage == "choice" {
        let tricks_in_hand: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| g.def_of(id).kind == CardKind::Trick)
            .collect();
        let region = region_ids(g, owner);
        let can_two = region.len() >= 2;
        if tricks_in_hand.is_empty() && !can_two {
            g.push_log(format!("灭计：{} 无牌可弃", g.seats[owner].name));
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("mieji");
                if !tricks_in_hand.is_empty() {
                    b = b.option("opt1", "灭计：弃置一张锦囊牌");
                }
                if can_two {
                    b = b.option("opt2", "灭计：依次弃置两张牌");
                }
                Ok(Step::Need(b.escape("放弃（不弃牌）").build()))
            }
            Some("opt1") => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick1", "card": card, "user": user }),
            )),
            Some("opt2") => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick2", "left": 2, "card": card, "user": user }),
            )),
            Some(other) => Err(format!("灭计：未知选项 {other}")),
        };
    }

    if stage == "pick1" {
        let cands: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| g.def_of(id).kind == CardKind::Trick)
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("mieji");
                for id in &cands {
                    b = b.option(format!("choose:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("放弃（不弃牌）").build()))
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("灭计：未知选项 {act}"))?
                    .to_string();
                if !cands.contains(&id) {
                    return Err("灭计：需要弃置一张锦囊牌".into());
                }
                g.to_discard(&id, "mieji")?;
                g.push_log(format!("灭计：弃置锦囊【{}】", label(g, &id)));
                Ok(Step::Done)
            }
        };
    }

    // pick2：依次弃置两张牌（手牌或装备区，自选）
    let left = a.data["left"].as_u64().unwrap_or(0) as usize;
    let cands = region_ids(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("mieji");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("灭计：弃置【{}】（还需 {left} 张）", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("放弃（不弃牌）").build()))
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("灭计：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("灭计：需要弃置自己的牌".into());
            }
            g.to_discard(&id, "mieji")?;
            g.push_log(format!("灭计：弃置【{}】", label(g, &id)));
            if left <= 1 {
                Ok(Step::Done)
            } else {
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick2", "left": left - 1, "card": card, "user": user }),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 焚城（界李儒，限定技）
// ---------------------------------------------------------------------------

pub const FENCHENG: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "fencheng",
    name: "焚城",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_fencheng),
    handler: fencheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_fencheng(g: &Game, seat: usize) -> bool {
    active(g, seat, "fencheng")
        && play_phase(g, seat)
        && !g.seats[seat].flags.contains("fencheng_used")
        && !other_alive(g, seat).is_empty()
}

fn fencheng_order(g: &Game, owner: usize, start: usize) -> Vec<usize> {
    let n = g.seats.len();
    (0..n)
        .map(|k| (start + k) % n)
        .filter(|s| *s != owner && g.seats[*s].hp > 0)
        .collect()
}

fn fencheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let order: Vec<usize> = a.data["order"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let last = a.data["last"].as_u64().unwrap_or(0) as usize;
    let user = a.data["user"].as_u64().unwrap_or(owner as u64) as usize;

    if stage.is_empty() {
        if !can_fencheng(g, owner) {
            return Ok(Step::Done);
        }
        let cands = other_alive(g, owner);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("fencheng");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("焚城：从 {} 开始", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动焚城").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let start = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("焚城：未知选项 {act}"))?;
                if !cands.contains(&start) {
                    return Err("焚城：目标不合法".into());
                }
                let order = fencheng_order(g, owner, start);
                g.push_log(format!(
                    "焚城：从 {} 开始依次结算",
                    g.seats[start].name
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "run", "order": order, "cursor": 0, "last": 0 }),
                ))
            }
        };
    }

    if stage == "run" {
        let mut cursor = cursor;
        while cursor < order.len() && g.seats[order[cursor]].hp <= 0 {
            cursor += 1;
        }
        if cursor >= order.len() {
            g.seats[owner].flags.insert("fencheng_used".to_string());
            g.push_log("焚城：结算完成（限定技已使用）".to_string());
            return Ok(Step::Done);
        }
        push(
            g,
            "fencheng",
            order[cursor],
            json!({ "stage": "pick", "order": order, "cursor": cursor, "last": last, "user": owner }),
        );
        return Ok(Step::Done);
    }

    if stage == "pick" {
        let need = last + 1;
        let region = region_ids(g, owner);
        let can_discard = region.len() >= need;
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("fencheng");
                if can_discard {
                    b = b.option("opt1", format!("焚城：弃置 {need} 张牌（须比上家多）"));
                }
                b = b.option("opt2", "焚城：受到 2 点火焰伤害");
                Ok(Step::Need(b.escape("视为受到伤害").build()))
            }
            Some("opt1") => {
                if !can_discard {
                    return Err("焚城：牌不足，不能弃置".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "stage": "discard",
                        "picked": 0,
                        "need": need,
                        "order": order,
                        "cursor": cursor,
                        "last": last,
                        "user": user,
                    }),
                ))
            }
            Some("opt2") | Some("pass") => {
                g.push_log(format!(
                    "焚城：{} 受到 2 点火焰伤害",
                    g.seats[owner].name
                ));
                flow::damage_typed(g, Some(user), owner, 2, DamageKind::Fire, None, false);
                push(
                    g,
                    "fencheng",
                    user,
                    json!({ "stage": "run", "order": order, "cursor": cursor + 1, "last": last }),
                );
                Ok(Step::Done)
            }
            Some(other) => Err(format!("焚城：未知选项 {other}")),
        };
    }

    // discard：逐张自选，直到 ≥ need（可选「完成」）
    let need = a.data["need"].as_u64().unwrap_or(0) as usize;
    let picked = a.data["picked"].as_u64().unwrap_or(0) as usize;
    let cands = region_ids(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("fencheng");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("焚城：弃置【{}】（已弃 {picked}/{need}）", label(g, id)),
                );
            }
            if picked >= need {
                b = b.option("done", format!("焚城：完成弃置（共 {picked} 张）"));
            }
            Ok(Step::Need(b.escape("完成弃置").build()))
        }
        Some("done") | Some("pass") => {
            if picked >= need {
                push(
                    g,
                    "fencheng",
                    user,
                    json!({
                        "stage": "run",
                        "order": order,
                        "cursor": cursor + 1,
                        "last": picked,
                    }),
                );
            } else {
                // 未弃够又放弃 → 视为选择受到伤害。
                flow::damage_typed(g, Some(user), owner, 2, DamageKind::Fire, None, false);
                push(
                    g,
                    "fencheng",
                    user,
                    json!({
                        "stage": "run",
                        "order": order,
                        "cursor": cursor + 1,
                        "last": last,
                    }),
                );
            }
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("焚城：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("焚城：需要弃置自己的牌".into());
            }
            g.to_discard(&id, "fencheng")?;
            g.push_log(format!(
                "焚城：{} 弃置【{}】",
                g.seats[owner].name,
                label(g, &id)
            ));
            Ok(Step::Next(
                Stage::Choose,
                json!({
                    "stage": "discard",
                    "picked": picked + 1,
                    "need": need,
                    "order": order,
                    "cursor": cursor,
                    "last": last,
                    "user": user,
                }),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 绝策（界李儒）
// ---------------------------------------------------------------------------

pub const JUECE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "juece",
    name: "绝策",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: juece,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn juece_candidates(g: &Game, owner: usize) -> Vec<usize> {
    let n = g.hand(owner).len();
    other_alive(g, owner)
        .into_iter()
        .filter(|t| g.hand(*t).len() <= n)
        .collect()
}

fn juece(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "juece") || g.counter("turn.juece_used") > 0 {
        return Ok(Step::Done);
    }
    let cands = juece_candidates(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("juece");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("绝策：对 {} 造成 1 点伤害", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动绝策").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("绝策：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("绝策：目标不合法".into());
            }
            g.bump("turn.juece_used");
            g.push_log(format!(
                "绝策：对 {} 造成 1 点伤害",
                g.seats[t].name
            ));
            flow::damage(g, Some(owner), t, 1);
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 精策（界郭淮）
// ---------------------------------------------------------------------------

pub const JINGCE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jingce",
    name: "精策",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: jingce,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jingce_types(g: &Game) -> u32 {
    g.counter("turn.jingce_types").count_ones()
}

fn jingce(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let x = jingce_types(g) as usize;
    if !active(g, owner, "jingce") || x == 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let b = builder(owner, "choice").skill("jingce");
            let b = b.option("draw", format!("精策：摸 {x} 张牌"));
            Ok(Step::Need(b.escape("不发动精策").build()))
        }
        Some("draw") => {
            let n = g.draw(owner, x).len();
            g.push_log(format!("精策：{} 摸 {n} 张", g.seats[owner].name));
            Ok(Step::Done)
        }
        Some("pass") => Ok(Step::Done),
        Some(other) => Err(format!("精策：未知选项 {other}")),
    }
}

/// 精策：使用**手牌**时按花色/类型记账（回合内累计，回合切换清空）。
pub fn on_card_use_jingce(g: &mut Game, seat: usize, carrier: Option<&CardId>) {
    if !active(g, seat, "jingce") || g.turn != seat {
        return;
    }
    let Some(id) = carrier else { return };
    if !matches!(g.zone_of(id), Some(Zone::Hand(s)) if *s == seat) {
        return;
    }
    let suit_bit = match crate::skills::effective_suit(g, seat, id) {
        Some(crate::kernel::Suit::Spade) => 1u32,
        Some(crate::kernel::Suit::Heart) => 2,
        Some(crate::kernel::Suit::Club) => 4,
        Some(crate::kernel::Suit::Diamond) => 8,
        None => 0,
    };
    let type_bit = match g.def_of(id).kind {
        CardKind::Basic => 1u32,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    };
    let suits = g.counter("turn.jingce_suits") | suit_bit;
    g.counters.insert("turn.jingce_suits".to_string(), suits);
    let types = g.counter("turn.jingce_types") | type_bit;
    g.counters.insert("turn.jingce_types".to_string(), types);
}

/// 精策：手牌上限 + 本回合用过的花色数。
pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    if !active(g, seat, "jingce") {
        return 0;
    }
    g.counter("turn.jingce_suits").count_ones() as i32
}

// ---------------------------------------------------------------------------
// 惴恐（界伏皇后）
// ---------------------------------------------------------------------------

pub const ZHUIKONG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhuikong",
    name: "惴恐",
    triggers: &[EventKind::TurnStart],
    window: Window::OtherTurn,
    priority: 0,
    can_active: None,
    handler: zhuikong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhuikong_ready(g: &Game, owner: usize, current: usize) -> bool {
    active(g, owner, "zhuikong")
        && g.seats[owner].hp < g.seats[owner].max_hp
        && !g.hand(owner).is_empty()
        && !g.hand(current).is_empty()
        && !super::blocks_pindian(g, current)
}

fn zhuikong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let current = a.data["current"]
        .as_u64()
        .or_else(|| a.data["seat"].as_u64())
        .unwrap_or(0) as usize;

    if stage == "after" {
        let Some(res) = g.pindian_result.take() else {
            return Ok(Step::Done);
        };
        if res.a.is_empty() || res.b.is_empty() {
            return Ok(Step::Done);
        }
        if res.winner == Some(owner) {
            let n = g.seats.len();
            for i in 0..n {
                if i != current && g.seats[i].hp > 0 {
                    g.counters
                        .insert(format!("turn.zhuikong_deny:{current}"), 1);
                }
            }
            g.push_log(format!(
                "惴恐：拼点赢，{} 本回合不能对除其以外的角色使用牌",
                g.seats[current].name
            ));
        } else {
            // 没赢：获得其拼点牌，然后其视为对你使用一张【杀】。
            if matches!(g.zone_of(&res.b), Some(Zone::Discard) | Some(Zone::Processing)) {
                g.gain(&res.b, owner)?;
                g.push_log(format!(
                    "惴恐：获得 {} 的拼点牌【{}】",
                    g.seats[current].name,
                    label(g, &res.b)
                ));
            }
            g.push_log(format!(
                "惴恐：{} 视为对 {} 使用一张【杀】",
                g.seats[current].name, g.seats[owner].name
            ));
            g.use_virtual(current, "杀", vec![owner])?;
        }
        return Ok(Step::Done);
    }

    if !zhuikong_ready(g, owner, current) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let b = builder(owner, "choice").skill("zhuikong");
            let b = b.option(
                "opt1",
                format!("惴恐：与 {} 拼点", g.seats[current].name),
            );
            Ok(Step::Need(b.escape("不发动惴恐").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("opt1") => {
            g.push_log(format!(
                "惴恐：{} 与 {} 拼点",
                g.seats[owner].name, g.seats[current].name
            ));
            push(
                g,
                "zhuikong",
                owner,
                json!({ "stage": "after", "current": current }),
            );
            crate::kernel::pindian::start(g, owner, current, "zhuikong");
            Ok(Step::Done)
        }
        Some(other) => Err(format!("惴恐：未知选项 {other}")),
    }
}

/// 惴恐（赢）：该角色本回合不能对除其以外的角色使用牌。
pub fn others_use_blocked(g: &Game, seat: usize) -> bool {
    g.counter(&format!("turn.zhuikong_deny:{seat}")) > 0
}

// ---------------------------------------------------------------------------
// 求援（界伏皇后）
// ---------------------------------------------------------------------------

pub const QIUYUAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qiuyuan",
    name: "求援",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qiuyuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qiuyuan_helpers(g: &Game, owner: usize, user: usize) -> Vec<usize> {
    other_alive(g, owner)
        .into_iter()
        .filter(|t| *t != user && *t != owner)
        .collect()
}

fn qiuyuan_free(g: &Game, owner: usize) -> bool {
    active(g, owner, "qiuyuan")
        && g.counter(&format!("turn.qiuyuan:{owner}")) == 0
}

/// 〔杀〕被指定目标后：求援（洗牌前询问；结果经 `ShaIntercept::Continue` + 追加目标通道）。
pub fn qiuyuan_sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let Some(card) = card else { return Vec::new() };
    if !g.def_of(card).tags.sha || target == user {
        return Vec::new();
    }
    if !qiuyuan_free(g, target) || qiuyuan_helpers(g, target, user).is_empty() {
        return Vec::new();
    }
    vec![(target, "qiuyuan")]
}

/// 〔成为普通锦囊目标后〕：伤害锦囊也能触发求援。
pub fn qiuyuan_trick_owner(
    g: &Game,
    user: usize,
    name: &str,
    target: usize,
    _carrier: Option<&CardId>,
) -> Option<(usize, &'static str)> {
    if !crate::cards::is_damage_trick(name) || target == user {
        return None;
    }
    if !qiuyuan_free(g, target) || qiuyuan_helpers(g, target, user).is_empty() {
        return None;
    }
    Some((target, "qiuyuan"))
}

fn qiuyuan_continue(g: &mut Game, owner: usize, trick: bool) {
    if trick {
        g.trick_intercept = Some((owner, false));
    } else {
        g.sha_intercept = Some(ShaIntercept::Continue);
    }
}

/// 「与此牌同类型不同牌名」：按牌类别（基本/锦囊/装备）判定。
fn same_kind_other_name(g: &Game, id: &CardId, base_name: &str) -> bool {
    let mine = g.def_of(id);
    let base = crate::cards::def(base_name);
    mine.kind == base.kind && g.name_of(id) != base_name
}

fn qiuyuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let trick = a.data["trick_intercept"].as_bool().unwrap_or(false);
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let name = a.data["name"].as_str().unwrap_or("杀").to_string();

    if stage.is_empty() {
        if !qiuyuan_free(g, owner) || qiuyuan_helpers(g, owner, user).is_empty() {
            qiuyuan_continue(g, owner, trick);
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let b = builder(owner, "choice").skill("qiuyuan");
                let b = b.option("opt1", "求援：令另一名角色交牌或成为额外目标");
                Ok(Step::Need(b.escape("不发动求援").build()))
            }
            Some("opt1") => Ok(Step::Next(
                Stage::Target,
                json!({ "stage": "helper", "user": user, "trick": trick, "trick_intercept": trick, "card": card, "name": name }),
            )),
            Some(_) => {
                g.bump(&format!("turn.qiuyuan:{owner}"));
                qiuyuan_continue(g, owner, trick);
                Ok(Step::Done)
            }
        };
    }

    if stage == "helper" {
        let cands = qiuyuan_helpers(g, owner, user);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("qiuyuan");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("令 {} 求援", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动求援").build()))
            }
            Some("pass") => {
                qiuyuan_continue(g, owner, trick);
                Ok(Step::Done)
            }
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("求援：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("求援：协助者不合法".into());
                }
                g.bump(&format!("turn.qiuyuan:{owner}"));
                g.push_log(format!(
                    "求援：{} 令 {} 选择交牌或成为额外目标",
                    g.seats[owner].name, g.seats[t].name
                ));
                push(
                    g,
                    "qiuyuan",
                    t,
                    json!({
                        "stage": "choice",
                        "user": user,
                        "owner": owner,
                        "trick": trick,
                        "trick_intercept": trick,
                        "card": card,
                        "name": name,
                    }),
                );
                Ok(Step::Done)
            }
        };
    }

    if stage == "choice" {
        let recipient = a.data["owner"].as_u64().unwrap_or(0) as usize;
        let cands: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| same_kind_other_name(g, id, &name))
            .collect();
        let aoe = crate::cards::target_mode(&name) == TargetMode::AllAlive;
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qiuyuan");
                if !cands.is_empty() {
                    b = b.option(
                        "opt1",
                        format!("求援：交给 {} 一张同类型不同牌名的牌", g.seats[recipient].name),
                    );
                }
                if !aoe {
                    b = b.option("opt2", "求援：成为此牌的额外目标");
                }
                Ok(Step::Need(b.escape("不作选择").build()))
            }
            Some("opt1") => Ok(Step::Next(
                Stage::Choose,
                json!({
                    "stage": "give",
                    "user": user,
                    "owner": recipient,
                    "trick": trick,
                    "trick_intercept": trick,
                    "card": card,
                    "name": name,
                }),
            )),
            Some("opt2") => {
                g.extra_targets_mid.push(recipient);
                g.push_log(format!(
                    "求援：{} 成为此牌的额外目标",
                    g.seats[recipient].name
                ));
                qiuyuan_continue(g, recipient, trick);
                Ok(Step::Done)
            }
            Some(_) => {
                qiuyuan_continue(g, recipient, trick);
                Ok(Step::Done)
            }
        };
    }

    // give：协助者从手牌自选一张符合条件（同类型不同牌名）的牌
    let recipient = a.data["owner"].as_u64().unwrap_or(0) as usize;
    let cands: Vec<CardId> = hand(g, owner)
        .into_iter()
        .filter(|id| same_kind_other_name(g, id, &name))
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("qiuyuan");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("交给{}【{}】", g.seats[recipient].name, label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不交牌").build()))
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("求援：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("求援：需要同类型不同牌名的牌".into());
            }
            g.gain(&id, recipient)?;
            g.push_log(format!(
                "求援：{} 将【{}】交给 {}",
                g.seats[owner].name,
                label(g, &id),
                g.seats[recipient].name
            ));
            qiuyuan_continue(g, recipient, trick);
            Ok(Step::Done)
        }
    }
}
// ---------------------------------------------------------------------------
// 弓骑（界韩当）
// ---------------------------------------------------------------------------

pub const GONGQI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "gongqi",
    name: "弓骑",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_gongqi),
    handler: gongqi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_gongqi(g: &Game, seat: usize) -> bool {
    active(g, seat, "gongqi")
        && play_phase(g, seat)
        && g.counter("turn.gongqi_used") == 0
        && !region_ids(g, seat).is_empty()
}

fn gongqi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !can_gongqi(g, owner) {
            return Ok(Step::Done);
        }
        let cands = region_ids(g, owner);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("gongqi");
                for id in &cands {
                    b = b.option(
                        format!("choose:{id}"),
                        format!("弓骑：弃置【{}】", label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("不发动弓骑").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("弓骑：未知选项 {act}"))?
                    .to_string();
                if !cands.contains(&id) {
                    return Err("弓骑：需要弃置自己的牌".into());
                }
                let was_equip = g.def_of(&id).slot.is_some();
                let suit = suit_code(g, owner, &id);
                g.to_discard(&id, "gongqi")?;
                g.bump("turn.gongqi_used");
                g.counters
                    .insert(format!("turn.gongqi_range:{owner}"), 1);
                if suit > 0 {
                    g.counters
                        .insert(format!("turn.gongqi_suit:{owner}"), suit);
                } else {
                    g.counters.remove(&format!("turn.gongqi_suit:{owner}"));
                }
                g.push_log(format!(
                    "弓骑：弃置【{}】，本回合攻击范围无限{}",
                    label(g, &id),
                    if was_equip {
                        "，可弃置一名其他角色一张牌"
                    } else {
                        ""
                    }
                ));
                if was_equip {
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "stage": "target" }),
                    ))
                } else {
                    Ok(Step::Done)
                }
            }
        };
    }

    // target：选一名有牌的其他角色，再由韩当选其手牌（随机）或装备区具体牌。
    if stage == "target" {
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| !region_ids(g, *t).is_empty())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("gongqi");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("弓骑：弃置 {} 一张牌", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("放弃弃置其牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("弓骑：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("弓骑：目标不合法".into());
                }
                if g.seats[t].hp <= 0 {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "target": t }),
                ))
            }
        };
    }

    // pick：手牌（随机）或装备区具体牌（不含判定区）。
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let equips = equip_ids(g, target);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("gongqi");
            if !g.hand(target).is_empty() {
                b = b.option("hand", format!("弓骑：弃置 {} 一张随机手牌", g.seats[target].name));
            }
            for id in &equips {
                b = b.option(
                    format!("equip:{id}"),
                    format!("弓骑：弃置其装备【{}】", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("放弃弃置其牌").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("hand") => {
            if g.hand(target).is_empty() {
                return Err("弓骑：其没有手牌".into());
            }
            let idx = g.rand_below(g.hand(target).len());
            let id = g.hand(target)[idx].clone();
            g.to_discard(&id, "gongqi")?;
            g.push_log(format!(
                "弓骑：弃置 {} 的【{}】",
                g.seats[target].name,
                label(g, &id)
            ));
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("equip:")
                .ok_or_else(|| format!("弓骑：未知选项 {act}"))?
                .to_string();
            if !equips.contains(&id) {
                return Err("弓骑：需要弃置其装备区的牌".into());
            }
            g.to_discard(&id, "gongqi")?;
            g.push_log(format!(
                "弓骑：弃置 {} 的装备【{}】",
                g.seats[target].name,
                label(g, &id)
            ));
            Ok(Step::Done)
        }
    }
}

/// 弓骑：本回合攻击范围无限。
pub fn infinite_range(g: &Game, seat: usize) -> bool {
    active(g, seat, "gongqi") && g.counter(&format!("turn.gongqi_range:{seat}")) > 0
}

/// 弓骑：与弃置的牌花色相同的【杀】无次数限制。
pub fn sha_unlimited_card(g: &Game, seat: usize, card: Option<&CardId>) -> bool {
    if !active(g, seat, "gongqi") {
        return false;
    }
    let Some(id) = card else { return false };
    let want = g.counter(&format!("turn.gongqi_suit:{seat}"));
    if want == 0 || !g.def_of(id).tags.sha {
        return false;
    }
    suit_code(g, seat, id) == want
}

// ---------------------------------------------------------------------------
// 解烦（界韩当，限定技）
// ---------------------------------------------------------------------------

pub const JIEFAN: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "jiefan",
    name: "解烦",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jiefan),
    handler: jiefan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_jiefan(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "jiefan") || !play_phase(g, seat) {
        return false;
    }
    if g.round <= 1 {
        // 第一轮：回合结束时视为未发动 → 每回合可发动一次。
        g.counter(&format!("turn.jiefan_used:{seat}")) == 0
    } else {
        !g.seats[seat].flags.contains("jiefan_used")
    }
}

fn jiefan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "weapon" {
        return jiefan_weapon(g, a);
    }
    if stage.is_empty() {
        if !can_jiefan(g, owner) {
            return Ok(Step::Done);
        }
        let cands = alive(g);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("jiefan");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("解烦：选择 {}", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动解烦").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("解烦：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("解烦：目标不合法".into());
                }
                g.bump(&format!("turn.jiefan_used:{owner}"));
                if g.round >= 2 {
                    g.seats[owner].flags.insert("jiefan_used".to_string());
                }
                let voters: Vec<usize> = alive(g)
                    .into_iter()
                    .filter(|v| *v != t && g.distance(*v, t) <= g.attack_range(*v))
                    .collect();
                g.push_log(format!(
                    "解烦：选择 {}，攻击范围含其的角色依次选择",
                    g.seats[t].name
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "vote", "target": t, "voters": voters, "cursor": 0 }),
                ))
            }
        };
    }

    if stage == "vote" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let voters: Vec<usize> = a.data["voters"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
        while cursor < voters.len() && g.seats[voters[cursor]].hp <= 0 {
            cursor += 1;
        }
        if target >= g.seats.len() || g.seats[target].hp <= 0 || cursor >= voters.len() {
            return Ok(Step::Done);
        }
        push(
            g,
            "jiefan",
            voters[cursor],
            json!({ "stage": "pick", "target": target, "voters": voters, "cursor": cursor }),
        );
        return Ok(Step::Done);
    }

    // pick：投票者二选一（弃一张武器牌 / 令目标摸一张）。
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let voters: Vec<usize> = a.data["voters"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let weapons: Vec<CardId> = {
        let mut v = hand_weapon_cards(g, owner);
        if let Some(id) = weapon_slot_card(g, owner) {
            v.push(id);
        }
        v
    };
    let next = |g: &mut Game, last: Option<CardId>, voters: &[usize], cursor: usize, target: usize| {
        let mut c = cursor + 1;
        while c < voters.len() && g.seats[voters[c]].hp <= 0 {
            c += 1;
        }
        let _ = last;
        if c >= voters.len() {
            g.push_log("解烦：结算完成".to_string());
        } else {
            push(
                g,
                "jiefan",
                voters[c],
                json!({ "stage": "pick", "target": target, "voters": voters, "cursor": c }),
            );
        }
    };
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("jiefan");
            if !weapons.is_empty() {
                b = b.option("opt1", "解烦：弃置一张武器牌");
            }
            b = b.option("opt2", format!("解烦：令 {} 摸一张牌", g.seats[target].name));
            Ok(Step::Need(b.escape("不选择（视为令其摸牌）").build()))
        }
        Some("opt1") => {
            if weapons.is_empty() {
                return Err("解烦：没有武器牌".into());
            }
            if weapons.len() == 1 {
                let id = weapons[0].clone();
                g.to_discard(&id, "jiefan")?;
                g.push_log(format!("解烦：弃置武器【{}】", label(g, &id)));
                next(g, None, &voters, cursor, target);
                return Ok(Step::Done);
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "weapon", "target": target, "voters": voters, "cursor": cursor }),
            ))
        }
        Some("opt2") | Some("pass") => {
            let n = g.draw(target, 1).len();
            g.push_log(format!(
                "解烦：{} 摸 {n} 张",
                g.seats[target].name
            ));
            next(g, None, &voters, cursor, target);
            Ok(Step::Done)
        }
        Some(other) => Err(format!("解烦：未知选项 {other}")),
    }
}

fn jiefan_weapon(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let voters: Vec<usize> = a.data["voters"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let mut weapons = hand_weapon_cards(g, owner);
    if let Some(id) = weapon_slot_card(g, owner) {
        weapons.push(id);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("jiefan");
            for id in &weapons {
                b = b.option(
                    format!("choose:{id}"),
                    format!("解烦：弃置武器【{}】", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("改为令其摸一张牌").build()))
        }
        Some("pass") => Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "vote", "target": target, "voters": voters, "cursor": cursor }),
        )),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("解烦：未知选项 {act}"))?
                .to_string();
            if !weapons.contains(&id) {
                return Err("解烦：需要弃置一张武器牌".into());
            }
            g.to_discard(&id, "jiefan")?;
            g.push_log(format!("解烦：弃置武器【{}】", label(g, &id)));
            let mut c = cursor + 1;
            while c < voters.len() && g.seats[voters[c]].hp <= 0 {
                c += 1;
            }
            if c >= voters.len() {
                g.push_log("解烦：结算完成".to_string());
                Ok(Step::Done)
            } else {
                push(
                    g,
                    "jiefan",
                    voters[c],
                    json!({ "stage": "pick", "target": target, "voters": voters, "cursor": c }),
                );
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 父魂（界关兴张苞）
// ---------------------------------------------------------------------------

pub const FUHEN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "fuhen",
    name: "父魂",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_fuhen),
    handler: fuhen,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_fuhen(g: &Game, seat: usize) -> bool {
    active(g, seat, "fuhen")
        && play_phase(g, seat)
        && hand(g, seat).len() >= 2
        && other_alive(g, seat).iter().any(|t| {
            // 攻击范围内至少有一个目标（父魂无距离修正）
            g.distance(seat, *t) <= g.attack_range(seat)
        })
}

fn fuhen_pairs(g: &Game, seat: usize) -> Vec<(CardId, CardId)> {
    let h = hand(g, seat);
    let mut out = Vec::new();
    for i in 0..h.len() {
        for j in (i + 1)..h.len() {
            out.push((h[i].clone(), h[j].clone()));
        }
    }
    out
}

fn fuhen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;

    // 伤害后：出牌阶段使用【杀】造成伤害 → 本回合获得武圣/咆哮。
    if a.data["amount"].as_i64().is_some() {
        if !active(g, owner, "fuhen") {
            return Ok(Step::Done);
        }
        let src = a.data["cause"]
            .as_str()
            .and_then(|c| c.parse::<usize>().ok());
        if src != Some(owner) {
            return Ok(Step::Done);
        }
        let sha = a.data["card"]
            .as_str()
            .map(|c| g.def_of(&c.to_string()).tags.sha)
            .unwrap_or(false);
        if !sha || !play_phase(g, owner) || g.counter("turn.fuhen_granted") > 0 {
            return Ok(Step::Done);
        }
        g.bump("turn.fuhen_granted");
        crate::skills::grant_skill(g, owner, "wusheng");
        crate::skills::grant_skill(g, owner, "paoxiao");
        g.seats[owner].flags.insert("fuhen_granted".to_string());
        g.push_log(format!(
            "父魂：{} 本回合获得「武圣」「咆哮」",
            g.seats[owner].name
        ));
        return Ok(Step::Done);
    }

    // target：为转化【杀】选择目标（攻击范围内）。
    if a.data["stage"].as_str() == Some("target") {
        let c1 = a.data["c1"].as_str().unwrap_or_default().to_string();
        let c2 = a.data["c2"].as_str().unwrap_or_default().to_string();
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| g.distance(owner, *t) <= g.attack_range(owner))
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("fuhen");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("父魂：对 {} 使用【杀】", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消父魂").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("父魂：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("父魂：目标不合法".into());
                }
                let red1 = g.is_red(&c1);
                let red2 = g.is_red(&c2);
                g.set_virtual(&c1, "杀", vec![c1.clone(), c2.clone()]);
                g.response_color_lock = if red1 == red2 { Some(red1) } else { None };
                g.push_log(format!(
                    "父魂：将【{}】【{}】当【杀】使用",
                    label(g, &c1),
                    label(g, &c2)
                ));
                g.use_as(owner, &c1, "杀", vec![t], true, None)?;
                Ok(Step::Done)
            }
        };
    }

    if !can_fuhen(g, owner) {
        return Ok(Step::Done);
    }
    let pairs = fuhen_pairs(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("fuhen");
            for (c1, c2) in &pairs {
                b = b.option(
                    format!("pair:{c1},{c2}"),
                    format!("父魂：【{}】【{}】当【杀】使用", label(g, c1), label(g, c2)),
                );
            }
            Ok(Step::Need(b.escape("不发动父魂").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let pair = act
                .strip_prefix("pair:")
                .ok_or_else(|| format!("父魂：未知选项 {act}"))?;
            let (c1, c2) = pair
                .split_once(',')
                .ok_or_else(|| "父魂：需要两张手牌".to_string())?;
            let (c1, c2) = (c1.to_string(), c2.to_string());
            if !pairs.contains(&(c1.clone(), c2.clone())) {
                return Err("父魂：需要两张不同的手牌".into());
            }
            Ok(Step::Next(
                Stage::Target,
                json!({ "stage": "target", "c1": c1, "c2": c2 }),
            ))
        }
    }
}

/// 父魂：两张手牌当【杀】打出（响应窗口）。
pub fn two_card_sha(g: &Game, seat: usize) -> bool {
    active(g, seat, "fuhen") && hand(g, seat).len() >= 2
}

pub fn two_card_sha_pairs(g: &Game, seat: usize) -> Vec<(CardId, CardId)> {
    fuhen_pairs(g, seat)
}

/// 回合结束撤销父魂授予的「武圣」「咆哮」。
pub fn on_turn_end_fuhen(g: &mut Game, ended: usize) {
    if g.seats.get(ended).is_none() {
        return;
    }
    if g.seats[ended].flags.remove("fuhen_granted") {
        g.seats[ended].granted.retain(|x| x != "wusheng" && x != "paoxiao");
        crate::skills::rebuild_subs(g);
        g.push_log(format!(
            "父魂：{} 回合结束，失去「武圣」「咆哮」",
            g.seats[ended].name
        ));
    }
}

// ---------------------------------------------------------------------------
// 潜袭（界马岱）
// ---------------------------------------------------------------------------

pub const QIANXI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qianxi",
    name: "潜袭",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: None,
    handler: qianxi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn qianxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "qianxi") || hand(g, owner).is_empty() {
        return Ok(Step::Done);
    }
    let cands = hand(g, owner);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "card").skill("qianxi");
            for id in &cands {
                b = b.option(
                    format!("choose:{id}"),
                    format!("潜袭：展示【{}】", label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不发动潜袭").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("潜袭：未知选项 {act}"))?
                .to_string();
            if !cands.contains(&id) {
                return Err("潜袭：需要展示一张手牌".into());
            }
            let color = color_code(g, owner, &id);
            g.reveal(&id, Some(owner), "qianxi");
            // 展示牌本回合造成伤害 +1（离开手牌即失效，伤害时再校验）。
            g.counters
                .insert(format!("turn.qianxi_bonus:{id}"), 1);
            let targets: Vec<usize> = other_alive(g, owner)
                .into_iter()
                .filter(|t| g.distance(owner, *t) == 1)
                .collect();
            for t in &targets {
                g.counters
                    .insert(format!("turn.color_lock:{t}"), color);
            }
            g.push_log(format!(
                "潜袭：展示【{}】（{}色），距离 1 的 {} 名角色本回合不能用该色手牌",
                label(g, &id),
                if color == 1 { "红" } else { "黑" },
                targets.len()
            ));
            Ok(Step::Done)
        }
    }
}

/// 潜袭：颜色封锁（本回合不能用/打出该颜色手牌）。
pub fn hand_color_blocked(g: &Game, seat: usize, id: &CardId) -> bool {
    let lock = g.counter(&format!("turn.color_lock:{seat}"));
    if lock == 0 {
        return false;
    }
    if !matches!(g.zone_of(id), Some(Zone::Hand(s)) if *s == seat) {
        return false;
    }
    color_code(g, seat, id) == lock
}

/// 潜袭：展示牌本回合造成伤害 +1（该牌离开手牌后失效）。
pub fn qianxi_bonus(g: &Game, _source: usize, card: Option<&CardId>) -> i32 {
    let Some(id) = card else { return 0 };
    if g.counter(&format!("turn.qianxi_bonus:{id}")) == 0 {
        return 0;
    }
    1
}

// ---------------------------------------------------------------------------
// 矫诏（界郭皇后）+ 殚心
// ---------------------------------------------------------------------------

pub const JIAZHAO: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jiazhao",
    name: "矫诏",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_jiazhao),
    handler: jiazhao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 可「回合外发动」的主动技（矫诏 修改 1 起）。
pub const OUT_OF_TURN_SKILLS: &[&str] = &["jiazhao"];

pub fn jiazhao_mod(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get("jiazhao_mod").copied())
        .unwrap_or(0)
}

pub fn jiazhao_display_name(g: &Game, seat: usize) -> &'static str {
    match jiazhao_mod(g, seat) {
        0 => "矫诏",
        1 => "矫诏plus",
        _ => "矫诏plusplus",
    }
}

fn jiazhao_names(g: &Game, seat: usize) -> Vec<&'static str> {
    let mods = jiazhao_mod(g, seat);
    let mut out = Vec::new();
    for n in crate::cards::non_equip_names() {
        let d = crate::cards::def(n);
        if d.tags.shan {
            continue;
        }
        if !(d.kind == CardKind::Basic || (d.kind == CardKind::Trick && !d.tags.delayed)) {
            continue;
        }
        if mods == 0 && g.counter(&format!("round.used:{n}")) > 0 {
            continue;
        }
        out.push(n);
    }
    out
}

fn can_jiazhao(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "jiazhao") {
        return false;
    }
    let mods = jiazhao_mod(g, seat);
    if jiazhao_names(g, seat).is_empty() {
        return false;
    }
    if mods == 0 {
        play_phase(g, seat)
            && g.counter("turn.jiazhao_used") == 0
            && !hand(g, seat).is_empty()
    } else {
        g.counter(&format!("round.jiazhao_used:{seat}")) == 0
            && (mods >= 2 || !hand(g, seat).is_empty())
    }
}

fn jiazhao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let mods = jiazhao_mod(g, owner);

    if stage.is_empty() {
        if !can_jiazhao(g, owner) {
            return Ok(Step::Done);
        }
        let names = jiazhao_names(g, owner);
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiazhao");
                for n in &names {
                    b = b.option(
                        format!("name:{n}"),
                        format!("{}：当【{n}】使用", jiazhao_display_name(g, owner)),
                    );
                }
                Ok(Step::Need(b.escape("取消矫诏").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let name = act
                    .strip_prefix("name:")
                    .ok_or_else(|| format!("矫诏：未知选项 {act}"))?
                    .to_string();
                if !names.contains(&name.as_str()) {
                    return Err("矫诏：该牌名不可用（本轮已被使用或不是基本/普通锦囊）".into());
                }
                // 承载牌：修改 2 不再需要承载牌（虚拟牌）。
                if mods >= 2 {
                    jiazhao_resolve(g, owner, &name, None, Vec::new())?;
                    return Ok(Step::Done);
                }
                let cards = hand(g, owner);
                if cards.is_empty() {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({
                        "stage": "card",
                        "name": name,
                        "user": owner,
                        "jiazhao_target": a.data["jiazhao_target"].clone(),
                    }),
                ))
            }
        };
    }

    if stage == "card" {
        let name = a.data["name"].as_str().unwrap_or_default().to_string();
        let cards: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| card_usable(g, owner, id))
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("jiazhao");
                for id in &cards {
                    b = b.option(
                        format!("choose:{id}"),
                        format!(
                            "{}：将【{}】当【{name}】使用",
                            jiazhao_display_name(g, owner),
                            label(g, id)
                        ),
                    );
                }
                Ok(Step::Need(b.escape("取消矫诏").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("矫诏：未知选项 {act}"))?
                    .to_string();
                if !cards.contains(&id) {
                    return Err("矫诏：需要一张可用的手牌".into());
                }
                let mode = crate::cards::target_mode(&name);
                if mode != TargetMode::Chosen {
                    jiazhao_resolve(g, owner, &name, Some(id), Vec::new())?;
                    Ok(Step::Done)
                } else {
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "stage": "target", "name": name, "card": id }),
                    ))
                }
            }
        };
    }

    // target2：借刀杀人的第二目标（持武器者）。
    if stage == "target2" {
        let name = a.data["name"].as_str().unwrap_or_default().to_string();
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        let first = a.data["first"].as_u64().unwrap_or(0) as usize;
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| *t != first && g.equip_in(*t, Slot::Weapon).is_some())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("jiazhao");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("{}：令 {} 交武器", jiazhao_display_name(g, owner), g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消矫诏").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("矫诏：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("矫诏：目标不合法".into());
                }
                jiazhao_resolve(g, owner, &name, Some(card), vec![first, t])?;
                Ok(Step::Done)
            }
        };
    }

    // target：为需要选目标的牌名选目标（严格校验由 `use_as` 负责）。
    let name = a.data["name"].as_str().unwrap_or_default().to_string();
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let def = crate::cards::def(&name);
    let cands: Vec<usize> = match def.effect {
        Effect::Dismantle | Effect::Snatch => other_alive(g, owner)
            .into_iter()
            .filter(|t| !region_ids(g, *t).is_empty())
            .collect(),
        _ => other_alive(g, owner),
    };
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("jiazhao");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("{}：对 {} 使用【{name}】", jiazhao_display_name(g, owner), g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("取消矫诏").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("矫诏：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("矫诏：目标不合法".into());
            }
            if def.effect == Effect::Collateral {
                return Ok(Step::Next(
                    Stage::Target,
                    json!({ "stage": "target2", "name": name, "card": card, "first": t }),
                ));
            }
            jiazhao_resolve(g, owner, &name, Some(card), vec![t])?;
            Ok(Step::Done)
        }
    }
}

fn jiazhao_resolve(
    g: &mut Game,
    owner: usize,
    name: &str,
    card: Option<CardId>,
    chosen: Vec<usize>,
) -> Result<(), String> {
    let mods = jiazhao_mod(g, owner);
    let def = crate::cards::def(name);
    let targets: Vec<usize> = match crate::cards::target_mode(name) {
        TargetMode::AllAlive => alive(g),
        TargetMode::SelfOnly => vec![owner],
        TargetMode::Chosen => chosen,
    };
    let display = jiazhao_display_name(g, owner).to_string();
    if mods >= 2 {
        g.use_virtual(owner, name, targets.clone())?;
    } else {
        let c = card.ok_or_else(|| "矫诏：需要承载牌".to_string())?;
        g.set_virtual(&c, name, vec![c.clone()]);
        // 修改 1 起允许回合外使用、【无懈可击】只能在技能声明下使用 → 不走严格校验。
        let strict = mods == 0 && !def.tags.wuxie;
        g.use_as(owner, &c, name, targets.clone(), strict, None)?;
    }
    if mods == 0 {
        g.bump("turn.jiazhao_used");
    } else {
        g.bump(&format!("round.jiazhao_used:{owner}"));
    }
    g.push_log(format!(
        "{display}：当【{name}】使用{}",
        if targets.is_empty() {
            String::new()
        } else {
            format!(
                "（目标：{}）",
                targets
                    .iter()
                    .map(|t| g.seats[*t].name.clone())
                    .collect::<Vec<_>>()
                    .join("、")
            )
        }
    ));
    Ok(())
}

pub const DANXIN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "danxin",
    name: "殚心",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: danxin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn danxin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "danxin") {
        return Ok(Step::Done);
    }
    if a.data["amount"].as_i64().unwrap_or(0) <= 0 || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    let x = jiazhao_mod(g, owner) as usize;
    match g.answer.take().as_deref() {
        None => {
            let b = builder(owner, "choice").skill("danxin");
            let b = b.option(
                "opt1",
                format!("殚心：摸 {x} 张并修改「矫诏」"),
            );
            Ok(Step::Need(b.escape("不发动殚心").build()))
        }
        Some("opt1") => {
            let drawn = g.draw(owner, x).len();
            let before = jiazhao_mod(g, owner);
            let after = (before + 1).min(2);
            g.seats[owner]
                .counters
                .insert("jiazhao_mod".to_string(), after);
            g.push_log(format!(
                "殚心：摸 {drawn} 张，修改「矫诏」为「{}」",
                jiazhao_display_name(g, owner)
            ));
            crate::skills::rebuild_subs(g);
            Ok(Step::Done)
        }
        Some("pass") => Ok(Step::Done),
        Some(other) => Err(format!("殚心：未知选项 {other}")),
    }
}
// ---------------------------------------------------------------------------
// 矢北（界沮授，锁定技）
// ---------------------------------------------------------------------------

pub const SHIBEI: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "shibei",
    name: "矢北",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: shibei,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn shibei(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "shibei") || a.data["amount"].as_i64().unwrap_or(0) <= 0 {
        return Ok(Step::Done);
    }
    let key = format!("turn.shibei_hits:{owner}");
    let hits = g.counter(&key);
    g.counters.insert(key, hits + 1);
    if hits == 0 {
        let before = g.seats[owner].hp;
        flow::heal(g, owner, 1, "shibei");
        let healed = g.seats[owner].hp - before;
        g.push_log(format!(
            "矢北：本回合首次受伤，回复 {healed} 点体力（{}）",
            g.seats[owner].hp
        ));
    } else {
        g.push_log(format!(
            "矢北：本回合再次受伤，失去 1 点体力（{}）",
            g.seats[owner].hp
        ));
        flow::lose_hp(g, owner, 1, "shibei");
    }
    Ok(Step::Done)
}

/// 矢北：游戏开始时获得 3 点护甲。
pub fn init_shibei(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if active(g, seat, "shibei") {
            g.add_armor(seat, 3);
            g.push_log(format!(
                "矢北：{} 游戏开始获得 3 点护甲",
                g.seats[seat].name
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// 渐营（界沮授）
// ---------------------------------------------------------------------------

pub const JIANYING: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jianying",
    name: "渐营",
    triggers: &[EventKind::CardUsed],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jianying),
    handler: jianying,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jianying_basic_names() -> Vec<&'static str> {
    crate::cards::non_equip_names()
        .into_iter()
        .filter(|n| {
            let d = crate::cards::def(n);
            d.kind == CardKind::Basic && !d.tags.shan
        })
        .collect()
}

fn can_jianying(g: &Game, seat: usize) -> bool {
    active(g, seat, "jianying")
        && play_phase(g, seat)
        && g.counter(&format!("turn.jianying_used:{seat}")) == 0
        && !hand(g, seat).is_empty()
        && !jianying_basic_names().is_empty()
}

fn jianying(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();

    // ① 使用牌时（CardUsed 触发）：与上一张牌同花色/点数 → 可摸一张。
    if stage.is_empty() && a.data["card"].is_string() {
        if !active(g, owner, "jianying") || g.turn != owner || g.phase != Phase::Play {
            return Ok(Step::Done);
        }
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        if g.counter(&format!("turn.jy_match:{card}")) == 0 {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let b = builder(owner, "choice").skill("jianying");
                let b = b.option("draw", "渐营：与上一张牌同花色/点数，摸一张牌");
                Ok(Step::Need(b.escape("不摸牌").build()))
            }
            Some("draw") => {
                g.counters
                    .remove(&format!("turn.jy_match:{card}"));
                let n = g.draw(owner, 1).len();
                g.push_log(format!("渐营：{} 摸 {n} 张", g.seats[owner].name));
                Ok(Step::Done)
            }
            Some("pass") => {
                g.counters
                    .remove(&format!("turn.jy_match:{card}"));
                Ok(Step::Done)
            }
            Some(other) => Err(format!("渐营：未知选项 {other}")),
        };
    }

    // ② 出牌阶段限一次：将一张牌当任意基本牌使用（花色继承本阶段上一张牌）。
    if stage.is_empty() {
        if !can_jianying(g, owner) {
            return Ok(Step::Done);
        }
        let names = jianying_basic_names();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jianying");
                for n in &names {
                    b = b.option(format!("name:{n}"), format!("渐营：当【{n}】使用"));
                }
                Ok(Step::Need(b.escape("不发动渐营").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let name = act
                    .strip_prefix("name:")
                    .ok_or_else(|| format!("渐营：未知选项 {act}"))?
                    .to_string();
                if !names.contains(&name.as_str()) {
                    return Err("渐营：只能当基本牌使用（不含【闪】）".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "card", "name": name }),
                ))
            }
        };
    }

    if stage == "card" {
        let name = a.data["name"].as_str().unwrap_or_default().to_string();
        let cards: Vec<CardId> = hand(g, owner)
            .into_iter()
            .filter(|id| card_usable(g, owner, id))
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "card").skill("jianying");
                for id in &cards {
                    b = b.option(
                        format!("choose:{id}"),
                        format!("渐营：将【{}】当【{name}】使用", label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("不发动渐营").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("渐营：未知选项 {act}"))?
                    .to_string();
                if !cards.contains(&id) {
                    return Err("渐营：需要一张可用的手牌".into());
                }
                if crate::cards::def(&name).tags.sha {
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "stage": "target", "name": name, "card": id }),
                    ))
                } else {
                    jianying_use(g, owner, &name, &id, None)?;
                    Ok(Step::Done)
                }
            }
        };
    }

    // target：当【杀】使用时的目标。
    let name = a.data["name"].as_str().unwrap_or_default().to_string();
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    let cands = other_alive(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("jianying");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("渐营：对 {} 使用【{name}】", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动渐营").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("渐营：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("渐营：目标不合法".into());
            }
            jianying_use(g, owner, &name, &card, Some(t))?;
            Ok(Step::Done)
        }
    }
}

fn jianying_use(
    g: &mut Game,
    owner: usize,
    name: &str,
    card: &CardId,
    target: Option<usize>,
) -> Result<(), String> {
    let targets: Vec<usize> = match crate::cards::target_mode(name) {
        TargetMode::AllAlive => alive(g),
        TargetMode::SelfOnly => vec![owner],
        TargetMode::Chosen => target.map(|t| vec![t]).unwrap_or_default(),
    };
    // 花色继承：本阶段上一张牌有花色时，此牌花色视为与其相同。
    let prev_suit = g.counter("phase.jy_suit");
    if prev_suit > 0 {
        g.counters
            .insert(format!("turn.jy_suit_override:{card}"), prev_suit);
    }
    g.set_virtual(card, name, vec![card.clone()]);
    g.bump(&format!("turn.jianying_used:{owner}"));
    g.push_log(format!(
        "渐营：将【{}】当【{name}】使用{}",
        label(g, card),
        if prev_suit == 0 {
            String::new()
        } else {
            format!("（花色继承：{}）", suit_label(prev_suit))
        }
    ));
    g.use_as(owner, card, name, targets, true, None)?;
    Ok(())
}

fn suit_label(code: u32) -> &'static str {
    match code {
        1 => "黑桃",
        2 => "红桃",
        3 => "梅花",
        _ => "方块",
    }
}

/// 渐营②：花色继承（判定/属性读取的统一入口）。
pub fn jy_suit_override(g: &Game, card: &CardId) -> Option<crate::kernel::Suit> {
    match g.counter(&format!("turn.jy_suit_override:{card}")) {
        1 => Some(crate::kernel::Suit::Spade),
        2 => Some(crate::kernel::Suit::Heart),
        3 => Some(crate::kernel::Suit::Club),
        4 => Some(crate::kernel::Suit::Diamond),
        _ => None,
    }
}

/// 渐营①：使用牌时记录「与上一张牌是否同花色/点数」（结算后由触发询问读取）。
pub fn on_card_use_jianying(g: &mut Game, seat: usize, carrier: Option<&CardId>) {
    if !active(g, seat, "jianying") || g.turn != seat || g.phase != Phase::Play {
        return;
    }
    let Some(id) = carrier else { return };
    let cur_suit = suit_code(g, seat, id);
    let Some(inst) = g.cards.get(id) else { return };
    let cur_rank = u32::from(
        crate::skills::effective_rank(g, seat, id).unwrap_or(inst.rank),
    );
    let prev_suit = g.counter("phase.jy_suit");
    let prev_rank = g.counter("phase.jy_rank");
    if (prev_suit > 0 && prev_suit == cur_suit) || (prev_rank > 0 && prev_rank == cur_rank) {
        g.counters.insert(format!("turn.jy_match:{id}"), 1);
    }
    g.counters.insert("phase.jy_suit".to_string(), cur_suit);
    g.counters.insert("phase.jy_rank".to_string(), cur_rank as u32);
}

// ---------------------------------------------------------------------------
// 跨批钩子汇总（由 `skills::mod` 调用）
// ---------------------------------------------------------------------------

/// 矢北：开局护甲。
pub fn on_game_start(g: &mut Game) {
    init_shibei(g);
}

/// 父魂：回合结束撤销授予技。
pub fn on_turn_end(g: &mut Game, ended: usize) {
    on_turn_end_fuhen(g, ended);
}

/// 一张牌被使用：精策（手牌花色/类型）+ 渐营（上一张牌）。
pub fn on_card_use(g: &mut Game, seat: usize, carrier: Option<&CardId>, _name: &str) {
    on_card_use_jingce(g, seat, carrier);
    on_card_use_jianying(g, seat, carrier);
}

/// 「造成伤害时」的数值加成：潜袭（展示牌 +1）。
pub fn damage_bonus(g: &Game, source: usize, card: Option<&CardId>) -> i32 {
    qianxi_bonus(g, source, card)
}

/// 「成为普通锦囊目标后」拦截：求援（伤害锦囊）。
pub fn trick_intercept_owner(
    g: &Game,
    user: usize,
    name: &str,
    target: usize,
    carrier: Option<&CardId>,
) -> Option<(usize, &'static str)> {
    qiuyuan_trick_owner(g, user, name, target, carrier)
}

/// 【杀】结算前介入：求援。
pub fn sha_intercept_skills(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    qiuyuan_sha_intercept(g, user, target, card)
}

// ---------------------------------------------------------------------------
// 测试辅助（集成测试用；不属于技能语义）
// ---------------------------------------------------------------------------

/// 矫诏当前显示名（Web/群侧文案与 legal 标签共用）。
pub fn display_name(g: &Game, seat: usize) -> &'static str {
    jiazhao_display_name(g, seat)
}
