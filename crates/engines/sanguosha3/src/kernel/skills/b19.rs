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
    SkillDef { code: "fencheng", name: "焚城", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zhuikong", name: "惴恐", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "qiuyuan", name: "求援", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "gongqi", name: "弓骑", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "jiefan", name: "解烦", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "fuhen", name: "父魂", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    SkillDef { code: "jiazhao", name: "矫诏", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "danxin", name: "殚心", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "jianying", name: "渐营", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "mieji" => mieji,
        "juece" => juece,
        "jingce" => jingce,
        "shibei" => shibei,
        "qianxi" => qianxi,
        "fencheng" => fencheng,
        "zhuikong" => zhuikong,
        "qiuyuan" => qiuyuan,
        "gongqi" => gongqi,
        "jiefan" => jiefan,
        "fuhen" => fuhen_damage,
        "jiazhao" => jiazhao,
        "danxin" => danxin,
        "jianying" => jianying,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "mieji" => Some(can_mieji),
        "fencheng" => Some(can_fencheng),
        "gongqi" => Some(can_gongqi),
        "jiefan" => Some(can_jiefan),
        "jiazhao" => Some(can_jiazhao),
        "jianying" => Some(can_jianying),
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


// ---------------------------------------------------------------------------
// 焚城（界李儒）
// ---------------------------------------------------------------------------

fn can_fencheng(g: &Game, seat: usize) -> bool {
    has(g, seat, "fencheng")
        && g.turn == seat
        && g.phase == Phase::Play
        && !skills::limited_used(g, seat, "fencheng")
        && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive)
}

fn fencheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "fencheng") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_fencheng(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive).collect();
        if let Some(ans) = take_answer(g, "fencheng") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("焚城：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("焚城：目标不合法".into());
            }
            skills::mark_limited_used(g, owner, "fencheng");
            let mut order = vec![];
            let n = g.seats.len();
            for k in 1..=n {
                let s = (t + k) % n;
                if s != owner && g.seats[s].alive {
                    order.push(s);
                }
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "order": order, "idx": 0, "prev": 0 })));
        }
        let mut b = PendingBuilder::new(owner, "fencheng_target").skill("fencheng");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("焚城：从 {} 左手起", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动焚城").build()));
    }
    // ask：当前角色选择弃置 > prev 张或受 2 火伤
    let order: Vec<usize> = ask.data.get("order").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect()).unwrap_or_default();
    let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let prev = ask.data.get("prev").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if idx >= order.len() {
        return Ok(Step::Done);
    }
    let cur = order[idx];
    if !g.seats.get(cur).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "order": order, "idx": idx + 1, "prev": prev })));
    }
    let cards = own_cards(g, cur);
    let can_discard = cards.len() > prev;
    if let Some(ans) = take_answer(g, "fencheng") {
        if ans == "damage" || !can_discard {
            let spec = super::super::DamageSpec { from: Some(owner), target: cur, amount: 2, kind: super::super::DamageKind::Fire, source: super::super::SourceKind::Skill };
            g.damage_card = None;
            flow::damage(g, spec, super::super::ArmorKind::None, 0);
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "order": order, "idx": idx + 1, "prev": prev })));
        }
        // discard all selected? simplified: discard (prev+1) random/any cards from hand+equip
        let need = prev + 1;
        let take: Vec<String> = cards.into_iter().take(need).collect();
        for id in &take {
            flow::discard(g, owner, id, "焚城");
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "order": order, "idx": idx + 1, "prev": need })));
    }
    let mut b = PendingBuilder::new(cur, "fencheng_ask").skill("fencheng");
    if can_discard {
        b = b.option("discard", format!("焚城：弃置至少 {} 张牌", prev + 1));
    }
    b = b.option("damage", "焚城：受到 2 点火焰伤害");
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "ask", "order": order, "idx": idx, "prev": prev }), b.escape_mandatory("（必须选择一项）").build()))
}

// ---------------------------------------------------------------------------
// 惴恐（界伏皇后）
// ---------------------------------------------------------------------------

fn zhuikong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhuikong") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") {
        return Ok(Step::Done);
    }
    if g.turn == owner || g.phase != Phase::Prepare || g.seats[owner].hp >= g.seats[owner].max_hp {
        return Ok(Step::Done);
    }
    let turn_seat = g.turn;
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if g.counter(&turn_key(turn_seat, "zhuikong_offered")) > 0 {
            return Ok(Step::Done);
        }
        g.set_counter(&turn_key(turn_seat, "zhuikong_offered"), 1);
        if !g.hand(owner).is_empty() && !g.hand(turn_seat).is_empty() {
            super::super::pindian::start(g, owner, turn_seat, "惴恐");
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "wait", "turn_seat": turn_seat })));
        }
        return Ok(Step::Done);
    }
    let Some(r) = g.take_pindian() else {
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "wait", "turn_seat": turn_seat })));
    };
    if r.initiator_won() {
        g.set_counter(&turn_key(turn_seat, "zhuikong"), 1);
    } else {
        let card = if r.initiator == owner { r.b } else { r.a };
        if matches!(g.cards.get(&card), Some(Zone::Discard)) {
            flow::gain(g, owner, &card, owner, "惴恐");
        }
        let _ = super::super::card_play::use_virtual(g, turn_seat, "sha", &[owner], false);
    }
    Ok(Step::Done)
}

pub fn others_use_blocked(g: &Game, seat: usize) -> bool {
    // 惴恐赢：**当前回合角色**不能对除自己以外的角色使用牌（标记按回合座位存）。
    g.counter(&turn_key(seat, "zhuikong")) > 0
}

// ---------------------------------------------------------------------------
// 求援（界伏皇后）
// ---------------------------------------------------------------------------

pub fn fire_target_helper(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    if targets.is_empty() {
        return false;
    }
    let Some(def) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) else {
        return false;
    };
    if !(def.tags.sha || def.tags.damage_trick) || def.target_mode == cards::TargetMode::AllAlive {
        return false;
    }
    let mut pushed = false;
    for &t in targets {
        if !has(g, t, "qiuyuan") {
            continue;
        }
        for h in 0..g.seats.len() {
            if h == user || targets.contains(&h) || !g.seats[h].alive {
                continue;
            }
            super::super::resolve::push_ask(
                g,
                "qiuyuan",
                Stage::Choose,
                h,
                json!({ "step": "ask", "user": user, "target": t, "card": card, "kind": format!("{:?}", def.category) }),
            );
            pushed = true;
        }
    }
    pushed
}

fn qiuyuan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let helper = ask.actor;
    let user = ask.data.get("user").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if !has(g, target, "qiuyuan") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "ask" {
        if let Some(ans) = take_answer(g, "qiuyuan") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            if ans == "become" {
                super::super::card_play::declare_mid_target(g, &card, helper);
                return Ok(Step::Done);
            }
            if let Some(id) = ans.strip_prefix("give:") {
                if g.hand(helper).iter().any(|c| c == id) {
                    flow::gain(g, target, id, helper, "求援");
                }
                return Ok(Step::Done);
            }
            return Err("求援：未知选项".into());
        }
        let mut b = PendingBuilder::new(helper, "qiuyuan_ask").skill("qiuyuan");
        b = b.option("become", "求援：成为此牌的额外目标");
        let wanted = match kind {
            "Basic" => cards::Category::Basic,
            "Equip" => cards::Category::Equip,
            _ => cards::Category::Trick,
        };
        for id in g.hand(helper) {
            let d = g.card_defs.get(&id).and_then(|c| cards::def(&c.def));
            if let Some(d) = d {
                if d.category == wanted && d.code != card && d.code != g.card_defs.get(&card).map(|c| c.def.clone()).unwrap_or_default() {
                    b = b.option(format!("give:{id}"), format!("求援：交给其【{}】", card_label(g, &id)));
                }
            }
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "step": "ask", "stage": "ask", "user": user, "target": target, "card": card, "kind": kind }), b.escape("不响应求援").build()));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 弓骑（界韩当）
// ---------------------------------------------------------------------------

fn can_gongqi(g: &Game, seat: usize) -> bool {
    has(g, seat, "gongqi")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "gongqi_used")) == 0
        && !own_cards(g, seat).is_empty()
}

fn gongqi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "gongqi") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_gongqi(g, owner) {
            return Ok(Step::Done);
        }
        let cards = own_cards(g, owner);
        if let Some(ans) = take_answer(g, "gongqi") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("弓骑：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("弓骑：该牌不在手牌/装备区".into());
            }
            let suit = g.card_defs.get(id).map(|c| c.suit).unwrap_or(Suit::Spade);
            let was_equip = matches!(g.cards.get(id), Some(Zone::Equip(..)));
            g.bump(&turn_key(owner, "gongqi_used"));
            g.set_counter(&turn_key(owner, "gongqi_range"), 1);
            g.set_mark(&turn_key(owner, "gongqi_suit"), format!("{:?}", suit));
            flow::discard(g, owner, id, "弓骑");
            if was_equip {
                return Ok(Step::Next(Stage::Choose, json!({ "stage": "target" })));
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "gongqi_pick").skill("gongqi");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("弓骑：弃置【{}】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动弓骑").build()));
    }
    // target：弃他人一张（随机手牌/装备）
    let cands: Vec<usize> = (0..g.seats.len()).filter(|t| *t != owner && g.seats[*t].alive && !own_cards(g, *t).is_empty()).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "gongqi") {
        let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("弓骑：未知选项".into());
        };
        if !cands.contains(&t) {
            return Err("弓骑：目标不合法".into());
        }
        let id = g.hand(t).first().cloned().or_else(|| super::super::gear::equips_of(g, t).first().cloned());
        if let Some(id) = id {
            flow::discard(g, owner, &id, "弓骑");
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "gongqi_target").skill("gongqi");
    for t in &cands {
        b = b.option(format!("target:{t}"), format!("弓骑：弃置 {} 一张牌", seat_name(g, *t)));
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "target" }), b.escape("不弃置").build()))
}

pub fn infinite_range(g: &Game, seat: usize) -> bool {
    has(g, seat, "gongqi") && g.counter(&turn_key(seat, "gongqi_range")) > 0
}

pub fn sha_unlimited_card(g: &Game, seat: usize, card: &str) -> bool {
    if !has(g, seat, "gongqi") {
        return false;
    }
    let Some(want) = g.mark(&turn_key(seat, "gongqi_suit")).map(|s| s.to_string()) else {
        return false;
    };
    g.card_defs
        .get(card)
        .map(|c| format!("{:?}", c.suit) == want)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 解烦（界韩当）
// ---------------------------------------------------------------------------

fn can_jiefan(g: &Game, seat: usize) -> bool {
    has(g, seat, "jiefan")
        && g.turn == seat
        && g.phase == Phase::Play
        && !skills::limited_used(g, seat, "jiefan")
}

fn jiefan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiefan") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_jiefan(g, owner) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = (0..g.seats.len()).filter(|t| g.seats[*t].alive).collect();
        if let Some(ans) = take_answer(g, "jiefan") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                return Err("解烦：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("解烦：目标不合法".into());
            }
            skills::mark_limited_used(g, owner, "jiefan");
            let reach = super::super::gear::attack_range(g, t) as usize;
            let order: Vec<usize> = g.alive_seats().into_iter().filter(|s| *s != t && super::super::gear::within_attack_range(g, t, *s, reach)).collect();
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "target": t, "order": order, "idx": 0 })));
        }
        let mut b = PendingBuilder::new(owner, "jiefan_target").skill("jiefan");
        for t in &cands {
            b = b.option(format!("target:{t}"), format!("解烦：选择 {}", seat_name(g, *t)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动解烦").build()));
    }
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let order: Vec<usize> = ask.data.get("order").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect()).unwrap_or_default();
    let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if idx >= order.len() {
        return Ok(Step::Done);
    }
    let cur = order[idx];
    if !g.seats.get(cur).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "target": target, "order": order, "idx": idx + 1 })));
    }
    let weapon = super::super::gear::equip_in(g, cur, super::super::Slot::Weapon).cloned();
    if let Some(ans) = take_answer(g, "jiefan") {
        if ans == "draw" || weapon.is_none() {
            flow::draw(g, target, 1);
        } else if let Some(id) = ans.strip_prefix("weapon:") {
            if weapon.as_deref() != Some(id) {
                return Err("解烦：该武器不在其装备区".into());
            }
            flow::discard(g, cur, id, "解烦");
        }
        return Ok(Step::Next(Stage::Choose, json!({ "stage": "ask", "target": target, "order": order, "idx": idx + 1 })));
    }
    let mut b = PendingBuilder::new(cur, "jiefan_ask").skill("jiefan");
    if let Some(id) = &weapon {
        b = b.option(format!("weapon:{id}"), format!("解烦：弃置武器【{}】", card_label(g, id)));
    }
    b = b.option("draw", format!("解烦：令 {} 摸一张牌", seat_name(g, target)));
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "ask", "target": target, "order": order, "idx": idx }), b.escape_mandatory("（必须选择一项）").build()))
}

pub fn on_turn_end(g: &mut Game, seat: usize) {
    let _ = seat;
    if g.round <= 1 {
        for s in 0..g.seats.len() {
            if has(g, s, "jiefan") {
                g.seats[s].counters.remove("skill.limited.jiefan");
            }
        }
    }
    // 父魂授予到期
    for s in 0..g.seats.len() {
        if g.counter(&turn_key(s, "fuhen_granted")) > 0 {
            skills::revoke(g, s, "wusheng");
            skills::revoke(g, s, "paoxiao");
            g.set_counter(&turn_key(s, "fuhen_granted"), 0);
        }
    }
}

// ---------------------------------------------------------------------------
// 父魂（界关兴张苞）
// ---------------------------------------------------------------------------

pub fn pair_sha_available(g: &Game, seat: usize) -> bool {
    has(g, seat, "fuhen") && g.hand_size(seat) >= 2
}

pub fn on_use_start_fuhen(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "fuhen") {
        return;
    }
    let Some(origins) = g.virtual_origins.get(card).cloned() else { return };
    if origins.len() != 2 {
        return;
    }
    let colors: Vec<bool> = origins.iter().filter_map(|id| g.card_defs.get(id).map(|c| c.suit.is_red())).collect();
    if colors.len() == 2 && colors[0] == colors[1] {
        super::super::card_play::declare_response_color_lock(g, card, colors[0]);
    }
    g.bump(&turn_key(actor, "fuhen_pair"));
}

fn fuhen_damage(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "fuhen") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    if g.phase == Phase::Play && g.counter(&turn_key(owner, "fuhen_pair")) > 0 {
        skills::grant(g, owner, "wusheng");
        skills::grant(g, owner, "paoxiao");
        g.set_counter(&turn_key(owner, "fuhen_granted"), 1);
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 矫诏 / 殚心（界郭皇后）
// ---------------------------------------------------------------------------

fn jiazhao_mod(g: &Game, seat: usize) -> i32 {
    g.seats.get(seat).map(|s| s.counter("jiazhao_mod")).unwrap_or(0)
}

fn can_jiazhao(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "jiazhao") {
        return false;
    }
    if jiazhao_mod(g, seat) == 0 {
        return g.turn == seat
            && g.phase == Phase::Play
            && g.counter(&format!("phase.{seat}.jiazhao")) == 0;
    }
    g.counter(&format!("round.{seat}.jiazhao")) == 0
}

fn jiazhao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiazhao") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_jiazhao(g, owner) {
            return Ok(Step::Done);
        }
        let cards = g.hand(owner);
        if let Some(ans) = take_answer(g, "jiazhao") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("矫诏：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("矫诏：该牌不在手牌".into());
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "code", "card": id })));
        }
        let mut b = PendingBuilder::new(owner, "jiazhao_pick").skill("jiazhao");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("矫诏：以【{}】声明", card_label(g, id)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动矫诏").build()));
    }
    // code：本轮未使用过的基本/普通锦囊
    let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mods = jiazhao_mod(g, owner);
    let mut b = PendingBuilder::new(owner, "jiazhao_code").skill("jiazhao");
    let mut any = false;
    for code in ALL_DECLARABLE {
        let used = g.counter(&format!("round.used.{code}")) > 0;
        if used && mods == 0 {
            continue;
        }
        if mods == 0 && g.card_defs.get(&card).and_then(|c| cards::def(&c.def)).map(|d| d.code == *code).unwrap_or(false) {
            // 仍允许声明同名？按「本轮未使用过」允许；承载牌可以是任意。
        }
        let Some(d) = cards::def(code) else { continue };
        if matches!(d.category, cards::Category::DelayedTrick) {
            continue;
        }
        b = b.option(format!("code:{code}"), format!("矫诏：声明【{}】", d.name));
        any = true;
    }
    if !any {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "jiazhao") {
        let Some(code) = ans.strip_prefix("code:") else {
            return Err("矫诏：未知选项".into());
        };
        if mods == 0 {
            g.bump(&format!("phase.{owner}.jiazhao"));
        } else {
            g.bump(&format!("round.{owner}.jiazhao"));
        }
        if mods >= 2 {
            // plusplus：无承载虚拟牌
            let _ = super::super::card_play::use_virtual(g, owner, code, &[], false);
        } else {
            let _ = super::super::card_play::use_as(g, owner, code, &[card], &[], false);
        }
        return Ok(Step::Done);
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "code", "card": card }), b.escape("不声明").build()))
}

const ALL_DECLARABLE: &[&str] = &[
    "sha", "huo_sha", "lei_sha", "tao", "jiu", "wuxie_keji", "juedou", "shunshou_qianyang",
    "guohe_chaiqiao", "wuzhong_shengyou", "nanman_ruqin", "wanjian_qifa", "taoyuan_jieyi",
    "wugu_fengdeng", "huogong", "jiedao_sharen", "tiesuo_lianhuan",
];

fn danxin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "danxin") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let mods = jiazhao_mod(g, owner);
    if mods > 0 {
        flow::draw(g, owner, mods as usize);
    }
    if let Some(s) = g.seats.get_mut(owner) {
        if s.counter("jiazhao_mod") < 2 {
            s.add_counter("jiazhao_mod", 1);
        }
    }
    Ok(Step::Done)
}

pub fn display_name(g: &Game, seat: usize, code: &str) -> Option<&'static str> {
    if code != "jiazhao" {
        return None;
    }
    match jiazhao_mod(g, seat) {
        0 => None,
        1 => Some("矫诏plus"),
        _ => Some("矫诏plusplus"),
    }
}

pub fn out_of_turn_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    if has(g, seat, "jiazhao")
        && jiazhao_mod(g, seat) >= 1
        && g.counter(&format!("round.{seat}.jiazhao")) == 0
        && !g.hand(seat).is_empty()
    {
        vec!["jiazhao"]
    } else {
        vec![]
    }
}

pub fn start_out_of_turn(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if !out_of_turn_actions(g, seat).contains(&code) {
        return Err(format!("技能 {code} 当前不可回合外发动"));
    }
    super::super::resolve::push_ask(
        g,
        code,
        Stage::Choose,
        seat,
        json!({ "active": true, "step": "choose", "out_of_turn": true }),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 渐营
// ---------------------------------------------------------------------------

pub fn on_use_start_jianying(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "jianying") || g.turn != actor {
        return;
    }
    let Some(d) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) else { return };
    let cur_suit = g.card_defs.get(card).map(|c| format!("{:?}", c.suit)).unwrap_or_default();
    let cur_rank = g.card_defs.get(card).map(|c| c.rank).unwrap_or(0);
    let prev_suit = g.mark(&turn_key(actor, "jianying_suit")).map(|s| s.to_string());
    let prev_rank = g.counter(&turn_key(actor, "jianying_rank"));
    if prev_suit.as_deref() == Some(cur_suit.as_str()) || (prev_rank > 0 && prev_rank as u8 == cur_rank) {
        flow::draw(g, actor, 1);
    }
    g.set_mark(&turn_key(actor, "jianying_suit"), cur_suit);
    g.set_counter(&turn_key(actor, "jianying_rank"), cur_rank as i32);
    let _ = d;
}

fn can_jianying(g: &Game, seat: usize) -> bool {
    has(g, seat, "jianying")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&turn_key(seat, "jianying_used")) == 0
        && !g.hand(seat).is_empty()
}

fn jianying(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jianying") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() || stage == "choose" {
        if !can_jianying(g, owner) {
            return Ok(Step::Done);
        }
        let cards = g.hand(owner);
        if let Some(ans) = take_answer(g, "jianying") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("渐营：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("渐营：该牌不在手牌".into());
            }
            return Ok(Step::Next(Stage::Choose, json!({ "stage": "code", "card": id })));
        }
        let mut b = PendingBuilder::new(owner, "jianying_pick").skill("jianying");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("渐营：将【{}】当基本牌使用", card_label(g, id)));
        }
        return Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "" }), b.escape("不发动渐营").build()));
    }
    let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mut b = PendingBuilder::new(owner, "jianying_code").skill("jianying");
    for code in ["sha", "huo_sha", "lei_sha", "tao", "jiu"] {
        if let Some(d) = cards::def(code) {
            b = b.option(format!("code:{code}"), format!("渐营：当【{}】使用", d.name));
        }
    }
    if let Some(ans) = take_answer(g, "jianying") {
        let Some(code) = ans.strip_prefix("code:") else {
            return Err("渐营：未知选项".into());
        };
        g.bump(&turn_key(owner, "jianying_used"));
        g.set_counter(&turn_key(owner, "jianying_inherit"), 1);
        let _ = super::super::card_play::use_as(g, owner, code, &[card], &[], false);
        return Ok(Step::Done);
    }
    Ok(Step::NeedWith(Stage::Choose, json!({ "stage": "code", "card": card }), b.escape("取消").build()))
}

pub fn suit_for_use(g: &Game, seat: usize, physical: Suit) -> Suit {
    if !has(g, seat, "jianying") || g.counter(&turn_key(seat, "jianying_inherit")) <= 0 {
        return physical;
    }
    g.mark(&turn_key(seat, "jianying_suit"))
        .and_then(|s| match s {
            "Spade" => Some(Suit::Spade),
            "Heart" => Some(Suit::Heart),
            "Club" => Some(Suit::Club),
            "Diamond" => Some(Suit::Diamond),
            _ => None,
        })
        .unwrap_or(physical)
}

