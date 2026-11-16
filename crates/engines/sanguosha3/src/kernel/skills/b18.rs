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
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zishou" => zishou,
        "luoying" => luoying,
        "shangshi" => shangshi,
        _ => return None,
    })
}

pub fn availability(_code: &str) -> Option<fn(&Game, usize) -> bool> {
    None
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

/// 第三方濒死钩子（补益/仁心）：返回 (技能持有者, 技能 code)。
pub fn dying_helper_seats(_g: &Game, _dying: usize) -> Vec<(usize, &'static str)> {
    vec![]
}

/// 当先：回合准备阶段后插入额外出牌阶段。
pub fn take_extra_play(_g: &mut Game, _seat: usize) -> bool {
    false
}

/// 当先：额外出牌阶段结束（若本阶段未造成伤害 → 自伤 1）。
pub fn on_extra_play_end(_g: &mut Game, _seat: usize) {}

/// 濒死技能选项（伏枥）。
pub fn dying_options(_g: &Game, _asker: usize, _target: usize) -> Vec<(&'static str, String, &'static str)> {
    vec![]
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

