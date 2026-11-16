//! B19 技能（界限突破·J7：8 将 / 15 技能；马术复用 B9）。
//!
//! 依据 `docs/games/sanguosha/align/B19.json` 的 `agreed`（权威）。
//! `SKILLS` 只声明**已实现并收口**的技能（`check:sgs3-registry` 报 ⏳）。

use serde_json::json;

use super::super::cards;
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{Game, Phase, Suit, Zone};
use super::common::{card_label, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "mieji", name: "灭计", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "juece", name: "绝策", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "jingce", name: "精策", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "shibei", name: "矢北", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[Hook::GameStart], can_active: false }] },
    SkillDef { code: "qianxi", name: "潜袭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "mieji" => mieji,
        "juece" => juece,
        "jingce" => jingce,
        "shibei" => shibei,
        "qianxi" => qianxi,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "mieji" => Some(can_mieji),
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

// ---------------------------------------------------------------------------
// 灭计（界李儒）
// ---------------------------------------------------------------------------

fn can_mieji(g: &Game, seat: usize) -> bool {
    has(g, seat, "mieji")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "mieji_used")) == 0
        && g.hand(seat).iter().any(|id| {
            g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| {
                matches!(d.category, cards::Category::Trick | cards::Category::DelayedTrick)
            }).unwrap_or(false)
        })
        && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive && !g.hand(t).is_empty())
}

fn mieji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "mieji") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_mieji(g, owner) {
            return Ok(Step::Done);
        }
        let cards: Vec<String> = g.hand(owner).into_iter().filter(|id| {
            g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| {
                matches!(d.category, cards::Category::Trick | cards::Category::DelayedTrick)
            }).unwrap_or(false)
        }).collect();
        if let Some(ans) = take_answer(g, "mieji") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("灭计：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("灭计：该牌不是锦囊".into());
            }
            g.bump(&turn_key(owner, "mieji_used"));
            flow::to_deck_top(g, id);
            g.reveal(&[id.to_string()], "灭计");
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "target" })));
        }
        let mut b = PendingBuilder::new(owner, "mieji_pick").skill("mieji");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("灭计：将【{}】置于牌堆顶", card_label(g, id)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动灭计").build()));
    }
    if stage == "target" {
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive && !g.hand(*t).is_empty()).collect();
        if let Some(ans) = take_answer(g, "mieji") {
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("灭计：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("灭计：目标不合法".into());
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "discard", "target": t })));
        }
        let mut b = PendingBuilder::new(owner, "mieji_target").skill("mieji");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("灭计：目标 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "target" }), b.escape("不选择").build()));
    }
    // discard：目标二选一
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if let Some(ans) = take_answer(g, "mieji") {
        match ans.as_str() {
            "trick" => {
                if let Some(id) = g.hand(t).into_iter().find(|id| g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| matches!(d.category, cards::Category::Trick | cards::Category::DelayedTrick)).unwrap_or(false)) {
                    flow::discard(g, t, &id, "灭计");
                }
                return Ok(Step::Done);
            }
            "two" => {
                let h = g.hand(t);
                for id in h.into_iter().take(2) {
                    flow::discard(g, t, &id, "灭计");
                }
                return Ok(Step::Done);
            }
            _ => return Err("灭计：未知选项".into()),
        }
    }
    let has_trick = g.hand(t).iter().any(|id| g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| matches!(d.category, cards::Category::Trick | cards::Category::DelayedTrick)).unwrap_or(false));
    let mut b = PendingBuilder::new(t, "mieji_choice").skill("mieji");
    if has_trick {
        b = b.option("trick", "灭计：弃置一张锦囊牌");
    }
    if g.hand_size(t) >= 2 {
        b = b.option("two", "灭计：依次弃置两张牌");
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "discard", "target": t }), b.escape_mandatory("（必须选择一项）").build()))
}

// ---------------------------------------------------------------------------
// 绝策（界李儒）
// ---------------------------------------------------------------------------

fn juece(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "juece") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive && g.hand_size(*t) <= g.hand_size(owner)).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "juece") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("绝策：未知选项".into());
        };
        if !cands.contains(&t) {
            return Err("绝策：目标不合法".into());
        }
        let spec = super::super::DamageSpec { from: Some(owner), target: t, amount: 1, kind: super::super::DamageKind::Normal, source: super::super::SourceKind::Skill };
        g.damage_card = None;
        flow::damage(g, spec, super::super::ArmorKind::None, 0);
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "juece_target").skill("juece");
    for t in &cands {
        b = b.option(format!("target:{t}"), format!("绝策：对 {} 造成 1 点伤害", seat_name(g, *t)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "target", "event": "PhaseStart" }), b.escape("不发动绝策").build()))
}

// ---------------------------------------------------------------------------
// 精策（界郭淮）
// ---------------------------------------------------------------------------

pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "jingce") || g.turn != actor {
        return;
    }
    if !matches!(g.cards.get(card), Some(Zone::Hand(s)) if *s == actor) {
        return;
    }
    let Some(d) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) else { return };
    if let Some(c) = g.card_defs.get(card) {
        let key = turn_key(actor, &format!("jingce_suit.{:?}", c.suit));
        g.set_counter(&key, 1);
    }
    let kind = match d.category {
        cards::Category::Basic => "basic",
        cards::Category::Trick | cards::Category::DelayedTrick => "trick",
        _ => "equip",
    };
    let key = turn_key(actor, &format!("jingce_type.{kind}"));
    g.set_counter(&key, 1);
}

pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    if !has(g, seat, "jingce") {
        return 0;
    }
    let mut n = 0;
    for suit in [Suit::Spade, Suit::Heart, Suit::Club, Suit::Diamond] {
        if g.counter(&turn_key(seat, &format!("jingce_suit.{suit:?}"))) > 0 {
            n += 1;
        }
    }
    n
}

pub fn on_phase_end(g: &mut Game, seat: usize, phase: Phase) {
    if phase != Phase::Play || !has(g, seat, "jingce") {
        return;
    }
    let x = ["basic", "trick", "equip"].iter().filter(|k| g.counter(&turn_key(seat, &format!("jingce_type.{k}"))) > 0).count();
    if x > 0 {
        flow::draw(g, seat, x);
    }
}

fn jingce(_g: &mut Game, _ask: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 矢北（界沮授）
// ---------------------------------------------------------------------------

fn shibei(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "shibei") {
        return Ok(Step::Done);
    }
    if ask.data.get("hook").and_then(|v| v.as_str()) == Some("game_start") {
        g.add_armor(owner, 3);
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let key = turn_key(owner, "shibei_hit");
    if g.counter(&key) == 0 {
        g.bump(&key);
        flow::heal(g, owner, 1, "矢北");
    } else {
        flow::lose_hp(g, owner, 1, "矢北");
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 潜袭（界马岱）
// ---------------------------------------------------------------------------

fn qianxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qianxi") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::Play || g.turn != owner {
        return Ok(Step::Done);
    }
    if g.counter(&turn_key(owner, "qianxi_used")) > 0 {
        return Ok(Step::Done);
    }
    let hand = g.hand(owner);
    if hand.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "qianxi") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("潜袭：未知选项".into());
        };
        if !hand.iter().any(|c| c == id) {
            return Err("潜袭：该牌不在手牌".into());
        }
        g.bump(&turn_key(owner, "qianxi_used"));
        g.reveal(&[id.to_string()], "潜袭");
        g.set_mark(&turn_key(owner, "qianxi_card"), id);
        let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
        g.set_counter(&turn_key(owner, if red { "qianxi_red" } else { "qianxi_black" }), 1);
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "qianxi_pick").skill("qianxi");
    for id in &hand {
        b = b.option(format!("pick:{id}"), format!("潜袭：展示【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "pick", "event": "PhaseStart" }), b.escape("不发动潜袭").build()))
}

pub fn hand_color_blocked(g: &Game, seat: usize, id: &str) -> bool {
    if seat == g.turn {
        return false;
    }
    let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
    (g.counter(&format!("turn.{}.qianxi_red", g.turn)) > 0
        || g.counter(&format!("turn.{}.qianxi_black", g.turn)) > 0)
        && super::super::gear::distance(g, g.turn, seat) == 1
        && ((red && g.counter(&format!("turn.{}.qianxi_red", g.turn)) > 0)
            || (!red && g.counter(&format!("turn.{}.qianxi_black", g.turn)) > 0))
}

pub fn damage_bonus(g: &Game, spec: &super::super::DamageSpec) -> i32 {
    let Some(from) = spec.from else { return 0 };
    let Some(card) = g.damage_card.as_deref() else { return 0 };
    if g.mark(&turn_key(from, "qianxi_card")) == Some(card) {
        1
    } else {
        0
    }
}
