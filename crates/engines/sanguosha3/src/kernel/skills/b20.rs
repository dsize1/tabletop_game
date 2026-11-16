//! B20 技能（界限突破·J8：4 将 / 8 技能；验证批次最后一批）。
//!
//! 依据 `docs/games/sanguosha/align/B20.json` 的 `agreed`（权威）。

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
    SkillDef { code: "qiaoshi", name: "樵拾", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "yanyu", name: "燕语", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::CardLost, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "qiangshi", name: "强识", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "xiantu", name: "献图", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "zhenjun", name: "镇军", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "yizhong", name: "毅重", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "anxu", name: "安恤", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zhuixi", name: "追忆", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[
            Face { window: Window::OwnPhase(Phase::End), subscriptions: &[], hooks: &[], can_active: true },
            Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::Death, subject: Subject::Self_ }], hooks: &[], can_active: false },
        ] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "qiaoshi" => qiaoshi,
        "yanyu" => yanyu,
        "qiangshi" => qiangshi,
        "xiantu" => xiantu,
        "zhenjun" => zhenjun,
        "anxu" => anxu,
        "zhuixi" => zhuixi,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "anxu" => Some(can_anxu),
        "zhuixi" => Some(can_zhuixi),
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
// 樵拾（界夏侯氏）
// ---------------------------------------------------------------------------

fn qiaoshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qiaoshi") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::End || g.turn == owner {
        return Ok(Step::Done);
    }
    if g.counter(&format!("round.{owner}.qiaoshi_off")) > 0 {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "ask" {
        if let Some(ans) = take_answer(g, "qiaoshi") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            flow::draw(g, owner, 1);
            flow::draw(g, g.turn, 1);
            if g.hand_size(owner) != g.hand_size(g.turn) {
                g.set_counter(&format!("round.{owner}.qiaoshi_off"), 1);
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "qiaoshi_ask").skill("qiaoshi");
        b = b.option("use", format!("樵拾：与 {} 各摸一张牌", seat_name(g, g.turn)));
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "ask", "event": "PhaseStart" }), b.escape("不发动樵拾").build()));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 燕语（界夏侯氏）
// ---------------------------------------------------------------------------

fn yanyu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "yanyu") || g.turn != owner {
        return Ok(Step::Done);
    }
    if ask.data.get("stage").and_then(|v| v.as_str()) == Some("end") {
        let cands: Vec<usize> = ask.data.get("cands").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect()).unwrap_or_default();
        if let Some(ans) = take_answer(g, "yanyu") {
            if ans == "pass" { return Ok(Step::Done); }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else { return Err("燕语：未知选项".into()); };
            if !cands.contains(&t) { return Err("燕语：目标不合法".into()); }
            flow::draw(g, t, 2);
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "yanyu_target").skill("yanyu");
        for t in &cands { b = b.option(format!("target:{t}"), format!("燕语：令 {} 摸两张牌", seat_name(g, *t))); }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "end", "cands": cands }), b.escape("不发动燕语").build()));
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("CardLost") || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) else {
        return Ok(Step::Done);
    };
    let is_sha = g
        .card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.tags.sha)
        .unwrap_or(false);
    if is_sha {
        g.bump(&format!("phase.{owner}.yanyu_sha"));
    }
    Ok(Step::Done)
}

pub fn recast_allowed(g: &Game, seat: usize, id: &str) -> bool {
    has(g, seat, "yanyu")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.card_defs
            .get(id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.tags.sha)
            .unwrap_or(false)
}

pub fn on_phase_end(g: &mut Game, seat: usize, phase: Phase) {
    if phase == Phase::Play {
        xiantu_phase_end(g, seat);
    }
    if phase == Phase::End && has(g, seat, "zhenjun") {
        let n = g.counter(&turn_key(seat, "zhenjun_defer"));
        if n > 0 {
            if let Some(t) = g.mark(&turn_key(seat, "zhenjun_target")).and_then(|s| s.parse::<usize>().ok()) {
                if g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                    flow::draw(g, t, n as usize);
                }
            }
            g.set_counter(&turn_key(seat, "zhenjun_defer"), 0);
            g.marks.remove(&turn_key(seat, "zhenjun_target"));
        }
    }
    if phase != Phase::Play || !has(g, seat, "yanyu") {
        return;
    }
    if g.counter(&format!("phase.{seat}.yanyu_sha")) < 2 {
        return;
    }
    let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != seat && g.seats[*t].alive && g.seats[*t].male).collect();
    if cands.is_empty() {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "yanyu",
        Stage::Choose,
        seat,
        json!({ "stage": "end", "cands": cands }),
    );
}

// ---------------------------------------------------------------------------
// 强识（界张松）
// ---------------------------------------------------------------------------

fn qiangshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qiangshi") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::Play || g.turn != owner {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if g.counter(&turn_key(owner, "qiangshi_used")) > 0 {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive && !g.hand(*t).is_empty()).collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "qiangshi") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("强识：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("强识：目标不合法".into());
            }
            g.bump(&turn_key(owner, "qiangshi_used"));
            let mut b = PendingBuilder::new(owner, "qiangshi_pick").skill("qiangshi");
            for id in g.hand(t) {
                b = b.option(format!("pick:{id}"), format!("强识：展示【{}】", card_label(g, &id)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "pick", "target": t, "event": "PhaseStart" }),
                b.escape("不展示").build(),
            ));
        }
        let mut b = PendingBuilder::new(owner, "qiangshi_target").skill("qiangshi");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("强识：观看 {} 的手牌", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "", "event": "PhaseStart" }), b.escape("不发动强识").build()));
    }
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if let Some(ans) = take_answer(g, "qiangshi") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("强识：未知选项".into());
        };
        if !g.hand(t).iter().any(|c| c == id) {
            return Err("强识：该牌不在其手牌".into());
        }
        g.reveal(&[id.to_string()], "强识");
        if let Some(d) = g.card_defs.get(id).and_then(|c| cards::def(&c.def)) {
            let cat = match d.category {
                cards::Category::Basic => "basic",
                cards::Category::Trick | cards::Category::DelayedTrick => "trick",
                _ => "equip",
            };
            g.set_mark(&turn_key(owner, "qiangshi_category"), cat);
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "qiangshi_pick").skill("qiangshi");
    for id in g.hand(t) {
        b = b.option(format!("pick:{id}"), format!("强识：展示【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "pick", "target": t, "event": "PhaseStart" }), b.escape("不展示").build()))
}

pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "qiangshi") || g.phase != Phase::Play {
        return;
    }
    let Some(want) = g.mark(&turn_key(actor, "qiangshi_category")).map(|s| s.to_string()) else {
        return;
    };
    let Some(d) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) else { return };
    let cat = match d.category {
        cards::Category::Basic => "basic",
        cards::Category::Trick | cards::Category::DelayedTrick => "trick",
        _ => "equip",
    };
    if cat == want {
        flow::draw(g, actor, 1);
    }
}

// ---------------------------------------------------------------------------
// 献图（界张松）
// ---------------------------------------------------------------------------

fn xiantu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "xiantu") || g.turn == owner {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "ask" {
        if g.counter(&turn_key(g.turn, "xiantu_offered")) > 0 {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "xiantu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            g.bump(&turn_key(g.turn, "xiantu_offered"));
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "count", "event": "PhaseStart" })));
        }
        let b = PendingBuilder::new(owner, "xiantu_ask").skill("xiantu")
            .option("use", "献图：摸至多两张并交给当前回合角色");
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "ask", "event": "PhaseStart" }), b.escape("不发动献图").build()));
    }
    if stage == "count" {
        if let Some(ans) = take_answer(g, "xiantu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let n: usize = ans.trim_start_matches("n:").parse().unwrap_or(0);
            if n == 0 {
                return Ok(Step::Done);
            }
            let drawn = flow::draw(g, owner, n);
            let _ = drawn;
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "give", "n": n, "left": n, "event": "PhaseStart" })));
        }
        let mut b = PendingBuilder::new(owner, "xiantu_count").skill("xiantu");
        b = b.option("n:0", "献图：不摸不交");
        b = b.option("n:1", "献图：摸 1 张并交给其 1 张");
        b = b.option("n:2", "献图：摸 2 张并交给其 2 张");
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "count", "event": "PhaseStart" }), b.escape("不发动献图").build()));
    }
    // give
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 || g.hand(owner).is_empty() {
        let given = ask.data.get("n").and_then(|v| v.as_u64()).unwrap_or(0) as usize - left;
        g.set_counter(&turn_key(g.turn, "xiantu_given"), given as i32);
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "xiantu") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("献图：未知选项".into());
        };
        if !g.hand(owner).iter().any(|c| c == id) {
            return Err("献图：该牌不在手牌".into());
        }
        flow::gain(g, g.turn, id, owner, "献图");
        let left = left - 1;
        if left == 0 {
            let n = ask.data.get("n").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            g.set_counter(&turn_key(g.turn, "xiantu_given"), n as i32);
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "give", "n": ask.data.get("n"), "left": left, "event": "PhaseStart" })));
    }
    let mut b = PendingBuilder::new(owner, "xiantu_give").skill("xiantu");
    for id in g.hand(owner) {
        b = b.option(format!("pick:{id}"), format!("献图：交给【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "give", "n": ask.data.get("n"), "left": left, "event": "PhaseStart" }), b.escape_mandatory("（必须交牌）").build()))
}


pub fn xiantu_phase_end(g: &mut Game, turn_seat: usize) {
    if g.counter(&turn_key(turn_seat, "xiantu_given")) <= 0 {
        return;
    }
    let given = g.counter(&turn_key(turn_seat, "xiantu_given")) as usize;
    let dmg = g.counter(&format!("phase.{turn_seat}.damage_dealt")) as usize;
    g.set_counter(&turn_key(turn_seat, "xiantu_given"), 0);
    if dmg < given {
        // 张松失去 1 点体力：找到持有献图的角色（唯一来源）。
        if let Some(owner) = (0..g.seats.len()).find(|s| has(g, *s, "xiantu")) {
            flow::lose_hp(g, owner, 1, "献图");
        }
    }
}

// ---------------------------------------------------------------------------
// 镇军（界于禁）
// ---------------------------------------------------------------------------

fn zhenjun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhenjun") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.phase != Phase::Prepare || g.turn != owner {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| g.seats[*t].alive && !own_cards(g, *t).is_empty()).collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "zhenjun") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("镇军：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("镇军：目标不合法".into());
            }
            let x = (g.hand_size(t) as i32 - g.seats[t].hp).max(1) as usize;
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "discard", "target": t, "left": x, "non_equip": 0, "event": "PhaseStart" })));
        }
        let mut b = PendingBuilder::new(owner, "zhenjun_target").skill("zhenjun");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("镇军：弃置 {} 的牌", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "", "event": "PhaseStart" }), b.escape("不发动镇军").build()));
    }
    if stage == "choice" {
        let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let non_equip = ask.data.get("non_equip").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if let Some(ans) = take_answer(g, "zhenjun") {
            if ans == "self" {
                let ids: Vec<String> = g.hand(owner).into_iter().take(non_equip).collect();
                for id in ids { flow::discard(g, owner, &id, "镇军"); }
            } else if ans == "defer" {
                g.set_counter(&turn_key(owner, "zhenjun_defer"), non_equip as i32);
                g.set_mark(&turn_key(owner, "zhenjun_target"), t.to_string());
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "zhenjun_choice").skill("zhenjun");
        b = b.option("self", format!("镇军：自己弃置 {non_equip} 张牌"));
        b = b.option("defer", format!("镇军：本回合结束阶段令其摸 {non_equip} 张牌"));
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "choice", "target": t, "non_equip": non_equip }), b.escape_mandatory("（必须选择）").build()));
    }
    // discard
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut non_equip = ask.data.get("non_equip").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 {
        if non_equip == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "choice", "target": t, "non_equip": non_equip, "event": "PhaseStart" })));
    }
    let equips = super::super::gear::equips_of(g, t);
    if let Some(ans) = take_answer(g, "zhenjun") {
        let id = if ans == "hand" {
            let idx = g.rand_below(g.hand(t).len().max(1));
            g.hand(t).get(idx).cloned()
        } else {
            ans.strip_prefix("equip:").map(|s| s.to_string())
        };
        let Some(id) = id else {
            return Err("镇军：未知选项".into());
        };
        let is_equip = matches!(g.cards.get(&id), Some(Zone::Equip(..)));
        flow::discard(g, t, &id, "镇军");
        if !is_equip {
            non_equip += 1;
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "discard", "target": t, "left": left - 1, "non_equip": non_equip, "event": "PhaseStart" })));
    }
    let mut b = PendingBuilder::new(owner, "zhenjun_pick").skill("zhenjun");
    if !g.hand(t).is_empty() {
        b = b.option("hand", "镇军：随机弃置其一张手牌");
    }
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("镇军：弃置其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "discard", "target": t, "left": left, "non_equip": non_equip, "event": "PhaseStart" }), b.escape("停止弃置").build()))
}

// ---------------------------------------------------------------------------
// 毅重（界于禁）
// ---------------------------------------------------------------------------

pub fn card_ineffective_for(g: &mut Game, user: usize, target: usize, card: &str) -> bool {
    if !has(g, target, "yizhong") {
        return false;
    }
    if g.seats[user].hp < g.seats[target].hp {
        return false;
    }
    g.card_defs
        .get(card)
        .map(|c| c.suit == Suit::Spade || c.suit == Suit::Club)
        .unwrap_or(false)
        && g.card_defs
            .get(card)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.tags.sha)
            .unwrap_or(false)
}

pub fn shan_blocked_for(g: &Game, user: usize, target: usize, card: &str) -> bool {
    if !has(g, user, "yizhong") || g.hand_size(target) > g.hand_size(user) {
        return false;
    }
    g.card_defs
        .get(card)
        .map(|c| c.suit == Suit::Spade || c.suit == Suit::Club)
        .unwrap_or(false)
        && g.card_defs
            .get(card)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.tags.sha)
            .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 安恤（界步练师）
// ---------------------------------------------------------------------------

fn can_anxu(g: &Game, seat: usize) -> bool {
    has(g, seat, "anxu")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "anxu_used")) == 0
        && (0..g.seats.len()).filter(|t| *t != seat && g.seats[*t].alive && !own_cards(g, *t).is_empty()).count() >= 2
}

fn anxu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "anxu") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_anxu(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive && !own_cards(g, *t).is_empty()).collect();
        if let Some(ans) = take_answer(g, "anxu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(a) = ans.strip_prefix("a:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("安恤：未知选项".into());
            };
            if !cands.contains(&a) {
                return Err("安恤：目标不合法".into());
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "second", "a": a })));
        }
        let mut b = PendingBuilder::new(owner, "anxu_a").skill("anxu");
        for t in &cands {
            b = b.option(format!("a:{t}"), format!("安恤：选择 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动安恤").build()));
    }
    if stage == "second" {
        let a = ask.data.get("a").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && *t != a && g.seats[*t].alive && !own_cards(g, *t).is_empty()).collect();
        if let Some(ans) = take_answer(g, "anxu") {
            let Some(b2) = ans.strip_prefix("b:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("安恤：未知选项".into());
            };
            if !cands.contains(&b2) {
                return Err("安恤：目标不合法".into());
            }
            g.bump(&turn_key(owner, "anxu_used"));
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "grab", "a": a, "b": b2, "picked": [] })));
        }
        let mut b = PendingBuilder::new(owner, "anxu_b").skill("anxu");
        for t in &cands {
            b = b.option(format!("b:{t}"), format!("安恤：再选择 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "second", "a": a }), b.escape("不选择").build()));
    }
    // grab：各获得一张（随机手牌/装备）
    let a = ask.data.get("a").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let b2 = ask.data.get("b").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut picked: Vec<String> = ask.data.get("picked").and_then(|v| v.as_array()).map(|x| x.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    if picked.len() < 2 {
        let src = if picked.is_empty() { a } else { b2 };
        let id = g.hand(src).first().cloned().or_else(|| super::super::gear::equips_of(g, src).first().cloned());
        if let Some(id) = id {
            flow::gain(g, owner, &id, owner, "安恤");
            picked.push(id);
        }
        if picked.len() < 2 {
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "grab", "a": a, "b": b2, "picked": picked })));
        }
    }
    // 选一张展示并交给手牌较少者
    if let Some(ans) = take_answer(g, "anxu") {
        let Some(id) = ans.strip_prefix("give:") else {
            return Err("安恤：未知选项".into());
        };
        if !picked.iter().any(|c| c == id) {
            return Err("安恤：该牌不在获得牌中".into());
        }
        let (target, other) = if g.hand_size(a) <= g.hand_size(b2) { (a, b2) } else { (b2, a) };
        g.reveal(&[id.to_string()], "安恤");
        flow::gain(g, target, id, owner, "安恤");
        let _ = other;
        let spade = g.card_defs.get(id).map(|c| c.suit == Suit::Spade).unwrap_or(false);
        if !spade {
            flow::draw(g, owner, 1);
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "anxu_give").skill("anxu");
    for id in &picked {
        b = b.option(format!("give:{id}"), format!("安恤：将【{}】交给手牌较少者", card_label(g, id)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "grab", "a": a, "b": b2, "picked": picked }), b.escape_mandatory("（必须选择一张）").build()))
}

// ---------------------------------------------------------------------------
// 追忆（界步练师）
// ---------------------------------------------------------------------------

fn can_zhuixi(g: &Game, seat: usize) -> bool {
    has(g, seat, "zhuixi")
        && g.turn == seat
        && g.phase == Phase::End
        && !skills::limited_used(g, seat, "zhuixi")
}

fn zhuixi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhuixi") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    if event == "Death" {
        let Some(t) = g.mark(&turn_key(owner, "zhuixi_target")).and_then(|s| s.parse::<usize>().ok()) else {
            return Ok(Step::Done);
        };
        if g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
            flow::draw(g, t, 3);
            flow::heal(g, t, 1, "追忆");
        }
        return Ok(Step::Done);
    }
    if !can_zhuixi(g, owner) {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zhuixi") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("追忆：未知选项".into());
        };
        if !cands.contains(&t) {
            return Err("追忆：目标不合法".into());
        }
        skills::mark_limited_used(g, owner, "zhuixi");
        g.set_mark(&turn_key(owner, "zhuixi_target"), t.to_string());
        flow::draw(g, t, 3);
        flow::heal(g, t, 1, "追忆");
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "zhuixi_target").skill("zhuixi");
    for t in &cands {
        b = b.option(format!("target:{t}"), format!("追忆：令 {} 摸三张并回复 1", seat_name(g, *t)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "target", "event": "PhaseStart" }), b.escape("不发动追忆").build()))
}
