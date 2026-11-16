//! B18 技能（界限突破·J6：8 将 / 17 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B18.json` 的 `agreed`（权威）。
//! `SKILLS` 只声明**已实现并收口**的技能（`check:sgs3-registry` 报 ⏳）。

use serde_json::json;

use super::super::cards;
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{Game, Phase, Zone};
use super::common::{card_label, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "jueqing", name: "绝情", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "zishou", name: "自守", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "luoying", name: "落英", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardToDiscard, subject: Subject::Other }], hooks: &[], can_active: false }] },
    SkillDef { code: "shangshi", name: "伤逝", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "zhenlie", name: "贞烈", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "miji", name: "秘计", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "jianmie", name: "翦灭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zongshi", name: "宗室", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DamageBefore], can_active: false }] },
    SkillDef { code: "xianzhou", name: "献州", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "qieting", name: "窃听", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherTurn, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "chengxiang", name: "称象", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "renxin", name: "仁心", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DyingEnter], can_active: false }] },
    SkillDef { code: "jiushi", name: "酒诗", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: true }] },
    SkillDef { code: "ganlu", name: "甘露", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "buyi", name: "补益", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DyingEnter], can_active: false }] },
    SkillDef { code: "dangxian", name: "当先", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "fuli", name: "伏枥", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zishou" => zishou,
        "luoying" => luoying,
        "shangshi" => shangshi,
        "zhenlie" => zhenlie,
        "miji" => miji,
        "jianmie" => jianmie,
        "zongshi" => zongshi,
        "xianzhou" => xianzhou,
        "qieting" => qieting,
        "chengxiang" => chengxiang,
        "renxin" => renxin,
        "buyi" => buyi,
        "jiushi" => jiushi,
        "ganlu" => ganlu,
        "dangxian" => dangxian,
        "fuli" => fuli,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "jianmie" => Some(can_jianmie),
        "xianzhou" => Some(can_xianzhou),
        "jiushi" => Some(can_jiushi),
        "ganlu" => Some(can_ganlu),
        _ => None,
    }
}

fn has(g: &Game, seat: usize, code: &str) -> bool {
    skills::active(g, seat, code) && skills::enabled(code)
}

fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut v: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    v.sort();
    v
}

/// 绝情：伤害视为失去体力。
pub fn damage_to_hp_loss(g: &Game, source: Option<usize>, _victim: usize) -> bool {
    source
        .map(|s| has(g, s, "jueqing"))
        .unwrap_or(false)
}





// ---------------------------------------------------------------------------
// 绝情（界张春华）：你即将造成的伤害视为失去体力
// ---------------------------------------------------------------------------

// 行为在 `damage_to_hp_loss`（flow::damage_apply 单出口）。

// ---------------------------------------------------------------------------
// 自守（界刘表）：摸牌阶段多摸 X（势力数）；结束阶段若使用过伤害牌，弃 X 张
// ---------------------------------------------------------------------------

pub fn draw_bonus(g: &mut Game, seat: usize) -> usize {
    if !has(g, seat, "zishou") || g.turn != seat || g.phase != Phase::Draw {
        return 0;
    }
    g.kingdom_count()
}

pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    // 酒诗：背面朝上时，落英牌无距离且不能被响应。
    if has(g, actor, "jiushi") && g.seats[actor].flipped && g.card_mark(card) == Some("luoying") {
        g.use_opts_mut(card).no_distance = true;
        g.use_opts_mut(card).no_response = true;
    }
    // 当先：本回合获得的【杀】无距离。
    if g.card_mark(card) == Some("dangxian") {
        g.use_opts_mut(card).no_distance = true;
    }
    if !has(g, actor, "zishou") {
        return;
    }
    let is_damage_card = g
        .card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.tags.sha || d.tags.damage_trick)
        .unwrap_or(false);
    if is_damage_card {
        g.bump(&turn_key(actor, "zishou_damage_card"));
    }
}

fn zishou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zishou") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let x = g.kingdom_count();
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if g.counter(&turn_key(owner, "zishou_damage_card")) <= 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "discard", "x": x, "left": x, "event": "PhaseStart" }),
        ));
    }
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 || g.hand_size(owner) == 0 {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zishou") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("自守：未知选项".into());
        };
        if !g.hand(owner).iter().any(|c| c == id) {
            return Err("自守：该牌不在手牌".into());
        }
        flow::discard(g, owner, id, "自守");
        let left = left - 1;
        if left == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "discard", "x": x, "left": left, "event": "PhaseStart" }),
        ));
    }
    let mut b = PendingBuilder::new(owner, "zishou_discard").skill("zishou");
    for id in g.hand(owner) {
        b = b.option(format!("pick:{id}"), format!("自守：弃置【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "discard", "x": x, "left": left, "event": "PhaseStart" }),
        b.escape_mandatory("（必须弃置）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 落英（界曹植）：其他角色梅花牌因弃置/判定进弃牌堆后，可获得之
// ---------------------------------------------------------------------------

fn luoying(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "luoying") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("CardToDiscard") {
        return Ok(Step::Done);
    }
    let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) else {
        return Ok(Step::Done);
    };
    let Some(suit) = g.card_defs.get(card).map(|c| c.suit) else {
        return Ok(Step::Done);
    };
    if suit != super::super::Suit::Club || !matches!(g.cards.get(card), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    let cause = ask.data.get("cause").and_then(|v| v.as_str()).unwrap_or("");
    if matches!(cause, "死亡" | "使用落盘" | "转化牌结算完成") {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "luoying") {
        if ans == "gain" {
            flow::gain(g, owner, card, owner, "落英");
            g.set_card_mark(card, "luoying");
            g.bump(&turn_key(owner, "luoying_gained"));
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "luoying_gain").skill("luoying");
    b = b.option("gain", format!("落英：获得梅花【{}】", card_label(g, card)));
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "", "event": "CardToDiscard", "card": card, "cause": cause }),
        b.escape("不获得").build(),
    ))
}

// ---------------------------------------------------------------------------
// 伤逝（界张春华）：手牌数小于已损失体力 → 摸至 X；弃牌阶段内不即时补
// ---------------------------------------------------------------------------

fn shangshi_x(g: &Game, seat: usize) -> usize {
    let s = &g.seats[seat];
    (s.max_hp - s.hp).max(0) as usize
}

pub fn on_hand_changed(g: &mut Game, seat: usize) {
    if !has(g, seat, "shangshi") {
        return;
    }
    if g.phase == Phase::Discard {
        return;
    }
    let x = shangshi_x(g, seat);
    if x == 0 || g.hand_size(seat) >= x {
        return;
    }
    if g.counter(&turn_key(seat, "shangshi_asked")) > 0 {
        return;
    }
    g.set_counter(&turn_key(seat, "shangshi_asked"), 1);
    super::super::resolve::push_ask(
        g,
        "shangshi",
        Stage::Choose,
        seat,
        json!({ "stage": "ask" }),
    );
}

fn shangshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "shangshi") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    let x = shangshi_x(g, owner);
    if x == 0 || g.hand_size(owner) >= x {
        return Ok(Step::Done);
    }
    if stage == "ask" || stage == "end" {
        if let Some(ans) = take_answer(g, "shangshi") {
            if ans == "draw" {
                flow::draw(g, owner, x - g.hand_size(owner));
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "shangshi_ask").skill("shangshi");
        b = b.option(
            "draw",
            format!("伤逝：将手牌摸至 {x} 张（当前 {}）", g.hand_size(owner)),
        );
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": stage, "event": ask.data.get("event") }),
            b.escape("不发动伤逝").build(),
        ));
    }
    Ok(Step::Done)
}

/// 弃牌阶段结束：补一次判定（避免弃 1 摸 1 循环）。
pub fn on_phase_end(g: &mut Game, seat: usize, phase: Phase) {
    if phase != Phase::Discard || !has(g, seat, "shangshi") {
        return;
    }
    let x = shangshi_x(g, seat);
    if x == 0 || g.hand_size(seat) >= x {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "shangshi",
        Stage::Choose,
        seat,
        json!({ "stage": "end", "event": "PhaseEnd" }),
    );
}



// ---------------------------------------------------------------------------
// 贞烈
// ---------------------------------------------------------------------------

pub fn fire_target_ineffective(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    let mut pushed = false;
    for &t in targets {
        if t == user || !has(g, t, "zhenlie") {
            continue;
        }
        super::super::resolve::push_ask(
            g,
            "zhenlie",
            Stage::Choose,
            t,
            json!({ "step": "ask", "user": user, "target": t, "card": card }),
        );
        pushed = true;
    }
    pushed
}

fn zhenlie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhenlie") {
        return Ok(Step::Done);
    }
    let user = ask.data.get("user").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let stage = ask
        .data
        .get("stage")
        .or_else(|| ask.data.get("step"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if stage == "ask" {
        if let Some(ans) = take_answer(g, "zhenlie") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            flow::lose_hp(g, owner, 1, "贞烈");
            super::super::card_play::declare_target_ineffective(g, &card, owner);
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "choice", "user": user, "card": card }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "zhenlie_ask").skill("zhenlie");
        b = b.option("use", "贞烈：失去 1 点体力，令此牌对你无效");
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "ask", "user": user, "card": card }),
            b.escape("不发动贞烈").build(),
        ));
    }
    if stage == "choice" {
        if let Some(ans) = take_answer(g, "zhenlie") {
            match ans.as_str() {
                "miji" => {
                    g.set_counter(&turn_key(owner, "miji_extra"), 1);
                    return Ok(Step::Done);
                }
                "gain" => {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "pick", "user": user, "card": card }),
                    ));
                }
                _ => return Err("贞烈：未知选项".into()),
            }
        }
        let mut b = PendingBuilder::new(owner, "zhenlie_choice").skill("zhenlie");
        b = b.option("gain", format!("贞烈：获得 {} 一张牌", seat_name(g, user)));
        b = b.option("miji", "贞烈：本回合结束阶段额外发动一次秘计");
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "choice", "user": user, "card": card }),
            b.escape("不选择（仅令此牌无效）").build(),
        ));
    }
    // pick
    let equips = super::super::gear::equips_of(g, user);
    let has_hand = !g.hand(user).is_empty();
    if equips.is_empty() && !has_hand {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zhenlie") {
        let id = if ans == "hand" {
            let idx = g.rand_below(g.hand(user).len().max(1));
            g.hand(user).get(idx).cloned()
        } else {
            ans.strip_prefix("equip:").map(|s| s.to_string())
        };
        let Some(id) = id else {
            return Err("贞烈：未知选项".into());
        };
        flow::gain(g, owner, &id, owner, "贞烈");
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "zhenlie_pick").skill("zhenlie");
    if has_hand {
        b = b.option("hand", "贞烈：获得其一张随机手牌");
    }
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("贞烈：获得其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "pick", "user": user, "card": card }),
        b.escape("不获得").build(),
    ))
}

// ---------------------------------------------------------------------------
// 秘计
// ---------------------------------------------------------------------------

fn can_miji(g: &Game, owner: usize) -> bool {
    if !has(g, owner, "miji") || g.phase != Phase::End {
        return false;
    }
    if g.turn != owner && g.counter(&turn_key(owner, "miji_extra")) <= 0 {
        return false;
    }
    g.counter(&format!("phase.{owner}.miji_done")) == 0
}

fn miji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "miji") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    let x = (g.seats[owner].max_hp - g.seats[owner].hp).max(0) as usize;
    if stage.is_empty() {
        if !can_miji(g, owner) {
            return Ok(Step::Done);
        }
        g.bump(&format!("phase.{owner}.miji_done"));
        if g.turn != owner {
            g.set_counter(&turn_key(owner, "miji_extra"), 0);
        }
        if x == 0 {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "miji") {
            if ans == "use" {
                flow::draw(g, owner, x);
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "target", "x": x, "event": "PhaseStart" }),
                ));
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "miji_ask").skill("miji");
        b = b.option("use", format!("秘计：摸 {x} 张牌（已损失体力）"));
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "ask", "event": "PhaseStart" }),
            b.escape("不发动秘计").build(),
        ));
    }
    if stage == "ask" {
        if let Some(ans) = take_answer(g, "miji") {
            if ans == "use" {
                flow::draw(g, owner, x);
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "target", "x": x, "event": "PhaseStart" }),
                ));
            }
        }
        return Ok(Step::Done);
    }
    if stage == "target" {
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive).collect();
        if cands.is_empty() || g.hand(owner).is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "miji") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("秘计：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("秘计：目标不合法".into());
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "give", "x": x, "target": t, "sel": [], "event": "PhaseStart" }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "miji_target").skill("miji");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("秘计：交给 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "target", "x": x, "event": "PhaseStart" }),
            b.escape("不交给").build(),
        ));
    }
    // give
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut sel: Vec<String> = ask.data.get("sel").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    if sel.len() >= x || g.hand(owner).is_empty() {
        for id in &sel {
            flow::gain(g, t, id, owner, "秘计");
        }
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "miji") {
        if ans == "pass" || ans == "ok" {
            for id in &sel {
                flow::gain(g, t, id, owner, "秘计");
            }
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("秘计：未知选项".into());
        };
        if !g.hand(owner).iter().any(|c| c == id) || sel.iter().any(|c| *c == id) {
            return Err("秘计：该牌不是可选手牌".into());
        }
        sel.push(id.to_string());
        if sel.len() >= x {
            for id in &sel {
                flow::gain(g, t, id, owner, "秘计");
            }
            return Ok(Step::Done);
        }
    }
    let mut b = PendingBuilder::new(owner, "miji_give").skill("miji");
    if !sel.is_empty() {
        b = b.option("ok", format!("秘计：确定交给（{} 张）", sel.len()));
    }
    for id in g.hand(owner) {
        if sel.iter().any(|c| *c == id) {
            continue;
        }
        b = b.option(format!("pick:{id}"), format!("秘计：交给【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "give", "x": x, "target": t, "sel": sel, "event": "PhaseStart" }),
        b.escape("停止交给").build(),
    ))
}

// ---------------------------------------------------------------------------
// 翦灭
// ---------------------------------------------------------------------------

fn can_jianmie(g: &Game, seat: usize) -> bool {
    has(g, seat, "jianmie")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "jianmie_used")) == 0
        && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive)
}

fn discard_color(g: &mut Game, seat: usize, red: bool) -> usize {
    let ids: Vec<String> = g
        .hand(seat)
        .into_iter()
        .filter(|id| g.card_defs.get(id).map(|c| c.suit.is_red() == red).unwrap_or(false))
        .collect();
    let n = ids.len();
    for id in ids {
        flow::discard(g, seat, &id, "翦灭");
    }
    n
}

fn jianmie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jianmie") {
        return Ok(Step::Done);
    }
    let mut stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "choose" {
        stage = "";
    }
    if stage.is_empty() {
        if !can_jianmie(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive).collect();
        if let Some(ans) = take_answer(g, "jianmie") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("翦灭：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("翦灭：目标不合法".into());
            }
            g.bump(&turn_key(owner, "jianmie_used"));
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "self", "target": t })));
        }
        let mut b = PendingBuilder::new(owner, "jianmie_target").skill("jianmie");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("翦灭：与 {} 同时选色", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动翦灭").build()));
    }
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let actor = if stage == "self" { owner } else { t };
    if let Some(ans) = take_answer(g, "jianmie") {
        let red = match ans.as_str() {
            "red" => true,
            "black" => false,
            "pass" => return Ok(Step::Done),
            _ => return Err("翦灭：未知颜色".into()),
        };
        if stage == "self" {
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "other", "target": t, "self_red": red })));
        }
        let self_red = ask.data.get("self_red").and_then(|v| v.as_bool()).unwrap_or(true);
        let a = discard_color(g, owner, self_red);
        let b = discard_color(g, t, red);
        if a > b {
            let _ = super::super::card_play::use_virtual(g, owner, "juedou", &[t], false);
        } else if b > a {
            let _ = super::super::card_play::use_virtual(g, t, "juedou", &[owner], false);
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(actor, "jianmie_color").skill("jianmie");
    b = b.option("red", "翦灭：红色");
    b = b.option("black", "翦灭：黑色");
    let mut data = json!({ "stage": stage, "target": t });
    if let Some(c) = ask.data.get("self_red") {
        data["self_red"] = c.clone();
    }
    Ok(Step::NeedWith(Stage::Choose, data, b.escape_mandatory("（必须选择颜色）").build()))
}

// ---------------------------------------------------------------------------
// 宗室
// ---------------------------------------------------------------------------

pub fn should_ask_damage_before_victim(g: &Game, from: usize, target: usize) -> bool {
    if from == target || !has(g, target, "zongshi") || g.seats[target].hp <= 0 {
        return false;
    }
    let kingdom = g.seats[from].kingdom.clone();
    if kingdom.is_empty() || g.counter(&turn_key(target, &format!("zongshi.{kingdom}"))) > 0 {
        return false;
    }
    !g.hand(target).is_empty() || !super::super::gear::equips_of(g, target).is_empty()
}

pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    if has(g, seat, "zongshi") {
        g.kingdom_count() as i32
    } else {
        0
    }
}

fn zongshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zongshi") {
        return Ok(Step::Done);
    }
    let from = ask.data.get("from").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let kingdom = g.seats.get(from).map(|s| s.kingdom.clone()).unwrap_or_default();
    if g.counter(&turn_key(owner, &format!("zongshi.{kingdom}"))) > 0 {
        return Ok(Step::Done);
    }
    let equips = super::super::gear::equips_of(g, owner);
    let has_hand = !g.hand(owner).is_empty();
    if equips.is_empty() && !has_hand {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zongshi") {
        let id = if ans == "hand" {
            let idx = g.rand_below(g.hand(owner).len().max(1));
            g.hand(owner).get(idx).cloned()
        } else {
            ans.strip_prefix("equip:").map(|s| s.to_string())
        };
        let Some(id) = id else {
            return Err("宗室：未知选项".into());
        };
        g.set_counter(&turn_key(owner, &format!("zongshi.{kingdom}")), 1);
        flow::gain(g, from, &id, owner, "宗室");
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(from, "zongshi_pick").skill("zongshi");
    if has_hand {
        b = b.option("hand", "宗室：随机获得其一张手牌");
    }
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("宗室：获得其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "step": "ask", "target": owner, "from": from }),
        b.escape_mandatory("（必须获得一张牌）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 献州
// ---------------------------------------------------------------------------

fn can_xianzhou(g: &Game, seat: usize) -> bool {
    has(g, seat, "xianzhou")
        && g.turn == seat
        && g.phase == Phase::Play
        && !skills::limited_used(g, seat, "xianzhou")
        && !super::super::gear::equips_of(g, seat).is_empty()
}

fn xianzhou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "xianzhou") {
        return Ok(Step::Done);
    }
    let mut stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "choose" {
        stage = "";
    }
    if stage.is_empty() {
        if !can_xianzhou(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive).collect();
        if let Some(ans) = take_answer(g, "xianzhou") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("献州：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("献州：目标不合法".into());
            }
            let equips = super::super::gear::equips_of(g, owner);
            let x = equips.len();
            for id in &equips {
                flow::gain(g, t, id, owner, "献州");
            }
            skills::mark_limited_used(g, owner, "xianzhou");
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "choice", "target": t, "x": x })));
        }
        let mut b = PendingBuilder::new(owner, "xianzhou_target").skill("xianzhou");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("献州：将装备区所有牌交给 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动献州").build()));
    }
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if stage == "choice" {
        if let Some(ans) = take_answer(g, "xianzhou") {
            match ans.as_str() {
                "heal" => {
                    flow::heal(g, owner, x as i32, "献州");
                    return Ok(Step::Done);
                }
                "damage" => {
                    return Ok(Step::Next(Stage::Choose, json!({ "stage": "damage", "target": t, "left": x })));
                }
                _ => return Err("献州：未知选项".into()),
            }
        }
        let mut b = PendingBuilder::new(t, "xianzhou_choice").skill("xianzhou");
        b = b.option("heal", format!("献州：令蔡夫人回复 {x} 点体力"));
        b = b.option("damage", "献州：由蔡夫人选其攻击范围内角色各造成 1 点伤害");
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "choice", "target": t, "x": x }), b.escape_mandatory("（必须选择）").build()));
    }
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    let reach = super::super::gear::attack_range(g, t) as usize;
    let cands: Vec<usize> = g.alive_seats().into_iter().filter(|v| *v != t && super::super::gear::within_attack_range(g, t, *v, reach)).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "xianzhou") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(v) = ans.strip_prefix("damage:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("献州：未知选项".into());
        };
        if !cands.contains(&v) {
            return Err("献州：目标不在其攻击范围".into());
        }
        let spec = super::super::DamageSpec { from: Some(owner), target: v, amount: 1, kind: super::super::DamageKind::Normal, source: super::super::SourceKind::Skill };
        g.damage_card = None;
        flow::damage(g, spec, super::super::ArmorKind::None, 0);
        let left = left - 1;
        if left == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "damage", "target": t, "left": left })));
    }
    let mut b = PendingBuilder::new(owner, "xianzhou_damage").skill("xianzhou");
    for v in &cands {
        b = b.option(format!("damage:{v}"), format!("献州：对 {} 造成 1 点伤害", seat_name(g, *v)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "damage", "target": t, "left": left }), b.escape("停止选择").build()))
}

// ---------------------------------------------------------------------------
// 窃听
// ---------------------------------------------------------------------------

pub fn on_turn_end(g: &mut Game, turn_seat: usize) {
    for owner in 0..g.seats.len() {
        if owner == turn_seat || !has(g, owner, "qieting") {
            continue;
        }
        let no_damage = g.counter(&format!("turn.{turn_seat}.damage_dealt")) == 0;
        let no_cards = g.counter(&format!("turn.{turn_seat}.target_other")) == 0;
        if !no_damage && !no_cards {
            continue;
        }
        super::super::resolve::push_ask(
            g,
            "qieting",
            Stage::Choose,
            owner,
            json!({ "stage": "ask", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": false, "used_draw": false }),
        );
    }
}

fn qieting(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qieting") {
        return Ok(Step::Done);
    }
    let turn_seat = ask.data.get("turn_seat").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let no_damage = ask.data.get("no_damage").and_then(|v| v.as_bool()).unwrap_or(false);
    let no_cards = ask.data.get("no_cards").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut used_take = ask.data.get("used_take").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut used_draw = ask.data.get("used_draw").and_then(|v| v.as_bool()).unwrap_or(false);
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "ask" {
        let equips = super::super::gear::equips_of(g, turn_seat);
        if let Some(ans) = take_answer(g, "qieting") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            if ans == "draw" {
                flow::draw(g, owner, 1);
                used_draw = true;
            } else if ans == "take" {
                return Ok(Step::Next(Stage::Choose, json!({ "stage": "pick", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": used_take, "used_draw": used_draw })));
            } else {
                return Err("窃听：未知选项".into());
            }
            let again = (no_damage && !used_take && !equips.is_empty()) || (no_cards && !used_draw);
            if !again {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": used_take, "used_draw": used_draw })));
        }
        let mut b = PendingBuilder::new(owner, "qieting_ask").skill("qieting");
        if no_damage && !used_take && !equips.is_empty() {
            b = b.option("take", "窃听：获得当前回合角色装备区一张牌");
        }
        if no_cards && !used_draw {
            b = b.option("draw", "窃听：摸一张牌");
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "ask", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": used_take, "used_draw": used_draw }), b.escape("不发动窃听").build()));
    }
    let equips = super::super::gear::equips_of(g, turn_seat);
    if equips.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "qieting") {
        let Some(id) = ans.strip_prefix("equip:") else {
            return Err("窃听：未知选项".into());
        };
        if !equips.iter().any(|c| c == id) {
            return Err("窃听：该装备不在其装备区".into());
        }
        if let Some(slot) = g.card_defs.get(id).and_then(|c| cards::def(&c.def)).and_then(|d| d.slot) {
            if let Some(old) = g.cards.iter().find(|(_, z)| matches!(z, Zone::Equip(s, sl) if *s == owner && *sl == slot)).map(|(id, _)| id.clone()) {
                flow::discard(g, owner, &old, "窃听");
            }
            g.cards.insert(id.to_string(), Zone::Equip(owner, slot));
            flow::note_lost(g, turn_seat, id, true);
            let _ = flow::leave_equip(g, turn_seat, id, "窃听");
        }
        used_take = true;
        if no_cards && !used_draw {
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": used_take, "used_draw": used_draw })));
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "qieting_pick").skill("qieting");
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("窃听：获得其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "pick", "turn_seat": turn_seat, "no_damage": no_damage, "no_cards": no_cards, "used_take": used_take, "used_draw": used_draw }), b.escape("不获得").build()))
}


// ---------------------------------------------------------------------------
// 称象
// ---------------------------------------------------------------------------

fn chengxiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "chengxiang") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        let n = 4 + g.counter(&turn_key(owner, "chengxiang_bonus")).max(0) as usize;
        let mut pool = vec![];
        for _ in 0..n {
            if let Some(id) = g.deck.pop() {
                g.cards.insert(id.clone(), Zone::Processing);
                g.reveal(&[id.clone()], "称象");
                pool.push(id);
            }
        }
        if pool.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "event": "DamageAfter", "pool": pool, "picked": [], "sum": 0 }),
        ));
    }
    let pool: Vec<String> = ask.data.get("pool").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    let mut picked: Vec<String> = ask.data.get("picked").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    let mut sum = ask.data.get("sum").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if let Some(ans) = take_answer(g, "chengxiang") {
        if ans == "ok" || ans == "pass" {
            for id in &pool {
                if picked.contains(id) {
                    flow::gain(g, owner, id, owner, "称象");
                } else if matches!(g.cards.get(id), Some(Zone::Processing)) {
                    super::super::card_play::to_discard_with(g, id, "称象", Some(owner));
                }
            }
            if sum == 13 {
                g.bump(&turn_key(owner, "chengxiang_bonus"));
            }
            for id in &pool {
                g.conceal(&[id.clone()], "称象结算完成");
            }
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("take:") else {
            return Err("称象：未知选项".into());
        };
        if !pool.iter().any(|c| c == id) || picked.iter().any(|c| c == id) {
            return Err("称象：该牌不在亮出池".into());
        }
        let rank = g.card_defs.get(id).map(|c| c.rank as u32).unwrap_or(0);
        if sum + rank > 13 {
            return Err("称象：点数和超过 13".into());
        }
        picked.push(id.to_string());
        sum += rank;
    }
    let mut b = PendingBuilder::new(owner, "chengxiang_pick").skill("chengxiang");
    for id in &pool {
        if picked.iter().any(|c| c == id) {
            continue;
        }
        b = b.option(format!("take:{id}"), format!("称象：获得【{}】", card_label(g, id)));
    }
    b = b.option("ok", format!("确定（当前点数和 {sum}）"));
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "pick", "event": "DamageAfter", "pool": pool, "picked": picked, "sum": sum }),
        b.escape("放弃/结算").build(),
    ))
}

// ---------------------------------------------------------------------------
// 仁心 / 补益（濒死第三方）
// ---------------------------------------------------------------------------

pub fn dying_helper_seats(g: &Game, dying: usize) -> Vec<(usize, &'static str)> {
    let mut out = vec![];
    for s in 0..g.seats.len() {
        if s == dying || !g.seats[s].alive {
            continue;
        }
        if has(g, s, "renxin") {
            let has_equip = own_cards(g, s).into_iter().any(|id| {
                g.card_defs.get(&id).and_then(|c| cards::def(&c.def)).map(|d| d.slot.is_some()).unwrap_or(false)
            });
            if has_equip {
                out.push((s, "renxin"));
            }
        }
        if has(g, s, "buyi") && !g.hand(dying).is_empty() {
            out.push((s, "buyi"));
        }
    }
    out
}

fn renxin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "renxin") {
        return Ok(Step::Done);
    }
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let cards: Vec<String> = own_cards(g, owner)
        .into_iter()
        .filter(|id| g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| d.slot.is_some()).unwrap_or(false))
        .collect();
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "renxin") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("仁心：未知选项".into());
        };
        if !cards.iter().any(|c| c == id) {
            return Err("仁心：该牌不是装备牌".into());
        }
        flow::discard(g, owner, id, "仁心");
        skills::flip_seat(g, owner);
        flow::restore_hp(g, target, 1, "仁心");
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "renxin_pick").skill("renxin");
    for id in &cards {
        b = b.option(format!("pick:{id}"), format!("仁心：弃置装备【{}】并翻面", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "step": "ask", "target": target }),
        b.escape("不发动仁心").build(),
    ))
}

fn buyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "buyi") {
        return Ok(Step::Done);
    }
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let cards = own_cards(g, target);
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "buyi") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("补益：未知选项".into());
        };
        if !own_cards(g, target).iter().any(|c| c == id) {
            return Err("补益：该牌不在其区域".into());
        }
        let basic = g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| d.category == cards::Category::Basic).unwrap_or(false);
        if !basic {
            flow::discard(g, target, id, "补益");
            flow::restore_hp(g, target, 1, "补益");
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "buyi_pick").skill("buyi");
    for id in &cards {
        b = b.option(format!("pick:{id}"), format!("补益：查看/弃置【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "step": "ask", "target": target }),
        b.escape("不发动补益").build(),
    ))
}

// ---------------------------------------------------------------------------
// 酒诗
// ---------------------------------------------------------------------------

fn can_jiushi(g: &Game, seat: usize) -> bool {
    has(g, seat, "jiushi")
        && g.turn == seat
        && g.phase == Phase::Play
        && !g.seats[seat].flipped
        && g.counter(&turn_key(seat, "jiushi_used")) == 0
        && !super::super::card_play::jiu_limit_reached(g, seat)
}

fn jiushi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    if ask.data.get("event").and_then(|v| v.as_str()) == Some("DamageAfter") {
        return on_damage_after(g, ask);
    }
    let owner = ask.actor;
    if !has(g, owner, "jiushi") {
        return Ok(Step::Done);
    }
    let st = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if st != "flip" && st != "choose" && st != "" {
        return Ok(Step::Done);
    }
    if !can_jiushi(g, owner) {
        return Ok(Step::Done);
    }
    skills::flip_seat(g, owner);
    g.bump(&turn_key(owner, "jiushi_used"));
    let _ = super::super::card_play::use_virtual(g, owner, "jiu", &[], false);
    Ok(Step::Done)
}

// 酒诗：落英牌无距离且不能被响应；受伤后/累计落英后翻回。
pub fn on_damage_after(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiushi") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") || !g.seats[owner].flipped {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "jiushi") {
        if ans == "flip" {
            skills::flip_seat(g, owner);
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "jiushi_flip").skill("jiushi");
    b = b.option("flip", "酒诗：翻至正面");
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "step": "damage", "event": "DamageAfter" }),
        b.escape("不翻回").build(),
    ))
}

// ---------------------------------------------------------------------------
// 甘露
// ---------------------------------------------------------------------------

fn can_ganlu(g: &Game, seat: usize) -> bool {
    has(g, seat, "ganlu")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "ganlu_used")) == 0
        && (0..g.seats.len()).filter(|t| g.seats[*t].alive).count() >= 2
}

fn ganlu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "ganlu") {
        return Ok(Step::Done);
    }
    let mut stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "choose" {
        stage = "";
    }
    if stage.is_empty() {
        if !can_ganlu(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| g.seats[*t].alive).collect();
        if let Some(ans) = take_answer(g, "ganlu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("a:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("甘露：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("甘露：目标不合法".into());
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "second", "a": t })));
        }
        let mut b = PendingBuilder::new(owner, "ganlu_a").skill("ganlu");
        for t in &cands {
            if super::super::gear::equips_of(g, *t).is_empty() {
                continue;
            }
            b = b.option(format!("a:{t}"), format!("甘露：选择 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动甘露").build()));
    }
    let a = ask.data.get("a").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if stage == "second" {
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| g.seats[*t].alive && *t != a && !super::super::gear::equips_of(g, *t).is_empty()).collect();
        if let Some(ans) = take_answer(g, "ganlu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(b2) = ans.strip_prefix("b:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("甘露：未知选项".into());
            };
            if !cands.contains(&b2) {
                return Err("甘露：目标不合法".into());
            }
            let na = super::super::gear::equips_of(g, a).len();
            let nb = super::super::gear::equips_of(g, b2).len();
            let diff = na.abs_diff(nb);
            let lost = (g.seats[owner].max_hp - g.seats[owner].hp).max(0) as usize;
            g.bump(&turn_key(owner, "ganlu_used"));
            if diff > lost {
                let need = diff;
                return Ok(Step::Next(Stage::Choose, json!({ "stage": "pay", "a": a, "b": b2, "left": need })));
            }
            super::super::gear::swap_zones(g, a, b2)?;
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "ganlu_b").skill("ganlu");
        for t in &cands {
            b = b.option(format!("b:{t}"), format!("甘露：交换 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "second", "a": a }), b.escape("不选择（甘露不结算）").build()));
    }
    // pay
    let b2 = ask.data.get("b").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 || g.hand_size(owner) == 0 {
        super::super::gear::swap_zones(g, a, b2)?;
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "ganlu") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("甘露：未知选项".into());
        };
        if !g.hand(owner).iter().any(|c| c == id) {
            return Err("甘露：该牌不在手牌".into());
        }
        flow::discard(g, owner, id, "甘露");
        let left = left - 1;
        if left == 0 || g.hand_size(owner) == 0 {
            super::super::gear::swap_zones(g, a, b2)?;
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "pay", "a": a, "b": b2, "left": left })));
    }
    let mut b = PendingBuilder::new(owner, "ganlu_pay").skill("ganlu");
    for id in g.hand(owner) {
        b = b.option(format!("pick:{id}"), format!("甘露：弃置【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "pay", "a": a, "b": b2, "left": left }), b.escape_mandatory("（必须弃置）").build()))
}

// ---------------------------------------------------------------------------
// 当先 / 伏枥
// ---------------------------------------------------------------------------

pub fn take_extra_play(g: &mut Game, seat: usize) -> bool {
    if !has(g, seat, "dangxian") || g.turn != seat {
        return false;
    }
    if g.counter(&turn_key(seat, "dangxian_used")) > 0 {
        return false;
    }
    g.set_counter(&turn_key(seat, "dangxian_used"), 1);
    true
}

pub fn on_extra_play_end(g: &mut Game, seat: usize) {
    if !has(g, seat, "dangxian") || !g.seats.get(seat).map(|s| s.alive).unwrap_or(false) {
        return;
    }
    if g.counter(&format!("phase.{seat}.damage_dealt")) > 0 {
        return;
    }
    let spec = super::super::DamageSpec { from: None, target: seat, amount: 1, kind: super::super::DamageKind::Normal, source: super::super::SourceKind::Skill };
    g.damage_card = None;
    flow::damage(g, spec, super::super::ArmorKind::None, 0);
}

fn dangxian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "dangxian") {
        return Ok(Step::Done);
    }
    if g.extra_play != Some(owner) || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") {
        return Ok(Step::Done);
    }
    if g.counter(&turn_key(owner, "dangxian_sha")) > 0 {
        return Ok(Step::Done);
    }
    let sha: Vec<String> = g.deck.iter().rev().chain(g.cards.iter().filter(|(_, z)| **z == Zone::Discard).map(|(id, _)| id)).filter(|id| g.card_defs.get(*id).and_then(|c| cards::def(&c.def)).map(|d| d.tags.sha).unwrap_or(false)).cloned().take(1).collect();
    if let Some(id) = sha.first() {
        if matches!(g.cards.get(id), Some(Zone::Draw)) {
            g.deck.retain(|x| x != id);
        }
        flow::gain(g, owner, id, owner, "当先");
        g.set_card_mark(id, "dangxian");
        g.bump(&turn_key(owner, "dangxian_sha"));
    }
    Ok(Step::Done)
}

pub fn dying_options(g: &Game, asker: usize, target: usize) -> Vec<(&'static str, String, &'static str)> {
    if asker != target || !has(g, asker, "fuli") || skills::limited_used(g, asker, "fuli") {
        return vec![];
    }
    vec![("fuli", "伏枥：回复至 X 并摸至 X".to_string(), "dying")]
}

fn fuli(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "fuli") || skills::limited_used(g, owner, "fuli") {
        return Ok(Step::Done);
    }
    if ask.data.get("stage").and_then(|v| v.as_str()) != Some("dying") {
        return Ok(Step::Done);
    }
    let x = g.kingdom_count() as i32;
    skills::mark_limited_used(g, owner, "fuli");
    flow::restore_hp(g, owner, x, "伏枥");
    let hand = g.hand_size(owner);
    if (hand as i32) < x {
        flow::draw(g, owner, (x - hand as i32) as usize);
    }
    if x > g.counter(&format!("total.damage_dealt.{owner}")) {
        skills::flip_seat(g, owner);
    }
    g.set_mark(&format!("dying.fuli.{owner}"), "ok");
    Ok(Step::Done)
}
