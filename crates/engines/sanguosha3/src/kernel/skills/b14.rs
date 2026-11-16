//! B14 技能（界限突破·J2：8 将 / 20 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B14.json` 的 `agreed`（权威）与
//! `v3-parity-baseline §7.20`（K1'–K4 口径）。
//!
//! 红颜（K1'）：不引入全局花色改写；「使用/打出」时生成**虚拟红桃牌**（`push_virtual_use` 单出口），
//! 判定走 `judge` 结果出口，他人展示手牌判色走火攻单点，技能自身读牌走 `suit_for_*` 助手。

use serde_json::json;

use super::super::cards::{self, ResponseKind};
use super::super::event::{Event, EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::pindian;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, DamageKind, DamageSpec, Game, Phase, Slot, SourceKind, Suit, Zone};
use super::common::{self, card_label, own_cards, phase_key, seat_name, turn_key};
use super::{CardAs, Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界孙策 ——
    SkillDef { code: "zhiba", name: "制霸", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "jiang", name: "激昂", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::TargetChosen, subject: Subject::Any },
            Subscription { event: EventKind::CardDiscarded, subject: Subject::Any },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "hunzi", name: "魂姿", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界夏侯渊 ——
    SkillDef { code: "shensu", name: "神速", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "shebian", name: "设变", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::Flip], can_active: false }] },
    // —— 界卧龙诸葛亮 ——
    SkillDef { code: "bazhen", name: "八阵", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::VirtualEquip(Slot::Armor, "bagua_zhen")], can_active: false }] },
    SkillDef { code: "cangzhuo", name: "藏拙", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "huoji", name: "火计", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "kanpo", name: "看破", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界庞统 ——
    SkillDef { code: "lianhuan", name: "连环", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "niepan", name: "涅槃", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界魏延 ——
    SkillDef { code: "kuanggu", name: "狂骨", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    SkillDef { code: "qimou", name: "奇谋", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界小乔 ——
    SkillDef { code: "hongyan", name: "红颜", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "tianxiang", name: "天香", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DamageBefore], can_active: false }] },
    SkillDef { code: "piaoling", name: "飘零", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界孙坚 ——
    SkillDef { code: "yinghun", name: "英魂", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "wulie", name: "武烈", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界庞德 ——
    SkillDef { code: "jiandu", name: "鞬出", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaTargeted], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zhiba" => zhiba,
        "jiang" => jiang,
        "hunzi" => hunzi,
        "shensu" => shensu,
        "shebian" => shebian,
        "niepan" => niepan,
        "kuanggu" => kuanggu,
        "qimou" => qimou,
        "tianxiang" => tianxiang,
        "piaoling" => piaoling,
        "yinghun" => yinghun,
        "wulie" => wulie,
        "jiandu" => jiandu,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "zhiba" => can_zhiba,
        "qimou" => can_qimou,
        _ => return None,
    })
}

fn step_of(ask: &Ask) -> &str {
    ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask")
}

fn has(g: &Game, seat: usize, code: &str) -> bool {
    skills::active(g, seat, code) && skills::enabled(code)
}

// ---------------------------------------------------------------------------
// 红颜（K1'）——花色助手（唯一出口）
// ---------------------------------------------------------------------------

/// 该座位黑桃视为红桃（红颜）。
fn red_face(g: &Game, seat: usize, physical: Suit) -> Suit {
    if physical == Suit::Spade && has(g, seat, "hongyan") {
        Suit::Heart
    } else {
        physical
    }
}

/// 使用/打出时生成虚拟牌的花色（`push_virtual_use` 唯一调用点）。
pub fn suit_for_use(g: &Game, seat: usize, physical: Suit) -> Suit {
    red_face(g, seat, physical)
}

/// 判定牌花色（`judge::reveal/effective` 唯一调用点）。
pub fn suit_for_judge(g: &Game, seat: usize, physical: Suit) -> Suit {
    red_face(g, seat, physical)
}

/// 该座位自己的牌的有效花色（技能读牌用）。
fn suit_own(g: &Game, seat: usize, id: &str) -> Option<Suit> {
    g.card_defs.get(id).map(|c| red_face(g, seat, c.suit))
}

fn is_red_own(g: &Game, seat: usize, id: &str) -> bool {
    suit_own(g, seat, id).map(|s| s.is_red()).unwrap_or(false)
}

fn is_heart_own(g: &Game, seat: usize, id: &str) -> bool {
    suit_own(g, seat, id) == Some(Suit::Heart)
}

/// 某张牌在其归属者语义下的花色（他人读牌：火攻展示判色）。
pub fn suit_of_card(g: &Game, id: &str) -> Option<Suit> {
    let physical = g.card_defs.get(id)?.suit;
    let owner = g.cards.get(id).and_then(|z| z.seat());
    match owner {
        Some(s) => Some(red_face(g, s, physical)),
        None => Some(physical),
    }
}

/// 红颜②：装备区有红桃牌 → 手牌上限 = 体力上限（赋值）。
pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if !has(g, seat, "hongyan") {
        return None;
    }
    let has_red_equip = common::equip_cards(g, seat)
        .into_iter()
        .any(|id| is_red_own(g, seat, &id));
    if !has_red_equip {
        return None;
    }
    g.seats
        .get(seat)
        .map(|s| s.max_hp.max(0) as usize)
}

// ---------------------------------------------------------------------------
// 界孙策 · 制霸（双向拼点；挑战者没赢 → 孙策获两张拼点牌）
// ---------------------------------------------------------------------------

fn can_zhiba(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || g.turn != seat || !has(g, seat, "zhiba") {
        return false;
    }
    if g.counter(&phase_key(seat, "zhiba")) > 0 {
        return false;
    }
    !wu_targets(g, seat).is_empty()
}

fn wu_targets(g: &Game, lord: usize) -> Vec<usize> {
    g.alive_seats()
        .into_iter()
        .filter(|t| *t != lord && g.seats[*t].kingdom == "wu" && pindian::can_start(g, lord, *t))
        .collect()
}

fn zhiba_gain(g: &mut Game, owner: usize, r: &pindian::PindianResult) {
    for id in [r.a.clone(), r.b.clone()] {
        if matches!(g.cards.get(&id), Some(Zone::Discard)) {
            flow::gain(g, owner, &id, owner, "制霸");
        }
    }
}

fn zhiba(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhiba") {
        return Ok(Step::Done);
    }
    match step_of(ask) {
        "wait" => {
            if let Some(r) = g.take_pindian() {
                if !r.initiator_won() {
                    zhiba_gain(g, owner, &r);
                    return Ok(Step::Finish("制霸：挑战者没赢，获得两张拼点牌".into()));
                }
            }
            Ok(Step::Finish("制霸：结算完成".into()))
        }
        "other" => {
            let challenger = common::seat_of(ask, "challenger").unwrap_or(owner);
            match take_answer(g, "zhiba") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("zhiba");
                    b = b.option("accept", format!("接受 {} 的拼点", seat_name(g, challenger)));
                    Ok(Step::Need(
                        b.escape("拒绝")
                            .data(json!({ "step": "other", "challenger": challenger }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("制霸：拒绝拼点".into())),
                Some(_) => {
                    if !pindian::can_start(g, challenger, owner) {
                        return Ok(Step::Finish("制霸：无法拼点".into()));
                    }
                    pindian::start(g, challenger, owner, "制霸");
                    Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
                }
            }
        }
        _ => {
            let cands = wu_targets(g, owner);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "zhiba") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("zhiba");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("与 {} 拼点", seat_name(g, *t)));
                    }
                    Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "制霸：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("制霸：只能与吴势力角色拼点".into());
                    }
                    g.bump(&phase_key(owner, "zhiba"));
                    pindian::start(g, owner, t, "制霸");
                    Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
                }
                Some(o) => Err(format!("制霸：未知选项 {o}")),
            }
        }
    }
}

pub fn other_side_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    let Some(lord) = g.seats.iter().position(|s| {
        s.alive && s.identity == Some(super::super::Identity::Lord) && has(g, s.id, "zhiba")
    }) else {
        return vec![];
    };
    if seat == lord || g.turn != seat || g.phase != Phase::Play {
        return vec![];
    }
    if g.seats.get(seat).map(|s| s.kingdom.as_str()) != Some("wu") {
        return vec![];
    }
    if g.counter(&phase_key(seat, "zhiba_challenge")) > 0 || !pindian::can_start(g, seat, lord) {
        return vec![];
    }
    vec!["zhiba"]
}

pub fn start_other(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if code != "zhiba" {
        return Err("制霸：未知他人侧入口".into());
    }
    if !other_side_actions(g, seat).contains(&"zhiba") {
        return Err("制霸：当前不可挑战".into());
    }
    let lord = g
        .seats
        .iter()
        .position(|s| {
            s.alive && s.identity == Some(super::super::Identity::Lord) && has(g, s.id, "zhiba")
        })
        .ok_or("制霸：无主公")?;
    g.bump(&phase_key(seat, "zhiba_challenge"));
    super::super::resolve::push_ask(
        g,
        "zhiba",
        Stage::Choose,
        lord,
        json!({ "step": "other", "challenger": seat }),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 界孙策 · 激昂
// ---------------------------------------------------------------------------

fn jiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiang") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    if event == "CardDiscarded" {
        let Some(card) = common::card_of(ask, "card") else {
            return Ok(Step::Done);
        };
        if g.counter(&turn_key(owner, "jiang_gain")) > 0
            || g.counter(&turn_key(owner, &format!("jiang.card.{card}"))) <= 0
            || !matches!(g.cards.get(&card), Some(Zone::Discard))
        {
            return Ok(Step::Done);
        }
        return match take_answer(g, "jiang") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("jiang");
                b = b.option("use", format!("失去 1 点体力，获得【{}】", card_label(g, &card)));
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "gain", "card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => {
                g.bump(&turn_key(owner, "jiang_gain"));
                Ok(Step::Done)
            }
            Some(_) => {
                g.bump(&turn_key(owner, "jiang_gain"));
                flow::lose_hp(g, owner, 1, "激昂");
                if matches!(g.cards.get(&card), Some(Zone::Discard)) {
                    flow::gain(g, owner, &card, owner, "激昂");
                }
                Ok(Step::Finish("激昂：失去 1 点体力获得此牌".into()))
            }
        };
    }
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = common::def_of(g, &card) else {
        return Ok(Step::Done);
    };
    let red_sha = def.tags.sha && is_red_own(g, owner, &card);
    if !(def.tags.duel || red_sha) {
        return Ok(Step::Done);
    }
    let by = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
    let involved = by == Some(owner)
        || g.use_targets
            .get(&card)
            .map(|ts| ts.contains(&owner))
            .unwrap_or(false);
    let _ = &by;
    if !involved {
        return Ok(Step::Done);
    }
    let once = turn_key(owner, &format!("jiang.once.{card}"));
    if g.counter(&once) > 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "jiang") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("jiang");
            b = b.option("use", "摸一张牌");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "draw", "card": card })).build(),
            ))
        }
        Some(a) if a == "pass" => {
            g.set_counter(&once, 1);
            Ok(Step::Finish("激昂：不摸牌".into()))
        }
        Some(_) => {
            g.set_counter(&once, 1);
            g.bump(&turn_key(owner, &format!("jiang.card.{card}")));
            flow::draw(g, owner, 1);
            Ok(Step::Finish("激昂：摸一张".into()))
        }
    }
}

// ---------------------------------------------------------------------------
// 界孙策 · 魂姿
// ---------------------------------------------------------------------------

fn hunzi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "hunzi") {
        return Ok(Step::Done);
    }
    let phase = ask.data.get("phase").and_then(|v| v.as_str()).unwrap_or("");
    if g.counter(&format!("awaken.{owner}.hunzi")) == 0 {
        if phase != "Prepare" || g.turn != owner {
            return Ok(Step::Done);
        }
        if g.seats.get(owner).map(|s| s.hp) != Some(1) {
            return Ok(Step::Done);
        }
        g.set_counter(&format!("awaken.{owner}.hunzi"), 1);
        g.set_counter(&turn_key(owner, "hunzi_end"), 1);
        flow::lose_max_hp(g, owner, 1, "魂姿");
        skills::grant(g, owner, "yingzi");
        skills::grant(g, owner, "yinghun");
        return Ok(Step::Finish("魂姿：觉醒（获得【英姿】【英魂】）".into()));
    }
    if phase != "End" || g.counter(&turn_key(owner, "hunzi_end")) <= 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "hunzi") {
        None => {
            g.set_counter(&turn_key(owner, "hunzi_end"), 0);
            let mut b = PendingBuilder::new(owner, "choose").skill("hunzi");
            b = b.option("draw", "摸两张牌");
            b = b.option("heal", "回复 1 点体力");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "end", "phase": phase }))
                    .build(),
            ))
        }
        Some(a) if a == "draw" => {
            flow::draw(g, owner, 2);
            Ok(Step::Finish("魂姿：摸两张".into()))
        }
        Some(a) if a == "heal" => {
            flow::heal(g, owner, 1, "魂姿");
            Ok(Step::Finish("魂姿：回复 1 点".into()))
        }
        Some(_) => Ok(Step::Finish("魂姿：不发动".into())),
    }
}

// ---------------------------------------------------------------------------
// 界夏侯渊 · 神速
// ---------------------------------------------------------------------------

fn shensu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "shensu") {
        return Ok(Step::Done);
    }
    if g.turn != owner || ask.data.get("phase").and_then(|v| v.as_str()) != Some("Prepare") {
        return Ok(Step::Done);
    }
    let done = common::cards_of(ask, "done");
    if step_of(ask) == "fire" {
        let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if idx >= done.len() {
            return Ok(Step::Finish("神速：结算完成".into()));
        }
        let cands: Vec<usize> = common::alive_others(g, owner);
        if cands.is_empty() {
            return Ok(Step::Finish("神速：无可选目标".into()));
        }
        return match take_answer(g, "shensu") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("shensu");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("对 {} 使用【杀】", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名目标）")
                        .data(json!({ "step": "fire", "done": done, "idx": idx, "phase": "Prepare" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "神速：非法目标")?;
                if !cands.contains(&t) {
                    return Err("神速：目标非法".into());
                }
                let vid = super::super::card_play::use_virtual(g, owner, "sha", &[t], false)?;
                super::super::card_play::declare_no_distance(g, &vid);
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "fire", "done": done, "idx": idx + 1, "phase": "Prepare" }),
                ))
            }
            Some(o) => Err(format!("神速：未知选项 {o}")),
        };
    }
    if step_of(ask) == "discard" {
        let equips: Vec<CardId> = own_cards(g, owner)
            .into_iter()
            .filter(|id| common::def_of(g, id).map(|d| d.slot.is_some()).unwrap_or(false))
            .collect();
        if equips.is_empty() {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "ask", "done": done, "phase": "Prepare" }),
            ));
        }
        return match take_answer(g, "shensu") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("shensu");
                for id in &equips {
                    b = b.option(format!("c:{id}"), format!("弃置装备牌【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必弃一张装备牌）")
                        .data(json!({ "step": "discard", "done": done, "phase": "Prepare" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !equips.contains(&id) {
                    return Err("神速：只能弃置装备牌".into());
                }
                flow::discard(g, owner, &id, "神速");
                super::super::turn::skip_phase(g, Phase::Play);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "ask", "done": done, "phase": "Prepare" }),
                ))
            }
            Some(o) => Err(format!("神速：未知选项 {o}")),
        };
    }
    let has_equip = own_cards(g, owner)
        .into_iter()
        .any(|id| common::def_of(g, &id).map(|d| d.slot.is_some()).unwrap_or(false));
    let mut b = PendingBuilder::new(owner, "choose").skill("shensu");
    if !done.contains(&"o1".to_string()) {
        b = b.option("o1", "①跳过判定与摸牌阶段");
    }
    if !done.contains(&"o2".to_string()) && has_equip {
        b = b.option("o2", "②弃一张装备牌并跳过出牌阶段");
    }
    if !done.contains(&"o3".to_string()) {
        b = b.option("o3", "③翻面并跳过弃牌阶段");
    }
    if !done.is_empty() {
        b = b.option("confirm", "结束选择（已选各项视为使用【杀】）");
    }
    match take_answer(g, "shensu") {
        None => Ok(Step::Need(
            b.escape("不发动")
                .data(json!({ "step": "ask", "done": done, "phase": "Prepare" }))
                .build(),
        )),
        Some(a) if a == "pass" => Ok(Step::Finish("神速：不发动".into())),
        Some(a) if a == "confirm" => Ok(Step::Next(
            Stage::Target,
            json!({ "step": "fire", "done": done, "idx": 0, "phase": "Prepare" }),
        )),
        Some(a) if a.starts_with('o') => {
            let mut all = done.clone();
            all.push(a.clone());
            match a.as_str() {
                "o1" => {
                    super::super::turn::skip_phase(g, Phase::Judge);
                    super::super::turn::skip_phase(g, Phase::Draw);
                }
                "o2" => {
                    return Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "discard", "done": all, "phase": "Prepare" }),
                    ))
                }
                _ => {
                    skills::flip_seat(g, owner);
                    super::super::turn::skip_phase(g, Phase::Discard);
                }
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "ask", "done": all, "phase": "Prepare" }),
            ))
        }
        Some(o) => Err(format!("神速：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界夏侯渊 · 设变
// ---------------------------------------------------------------------------

fn shebian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "shebian") {
        return Ok(Step::Done);
    }
    if step_of(ask) == "pick" {
        let from = common::seat_of(ask, "from").unwrap_or(owner);
        let card = common::card_of(ask, "card").unwrap_or_default();
        let Some(slot) = common::def_of(g, &card).and_then(|d| d.slot) else {
            return Ok(Step::Done);
        };
        let cands: Vec<usize> = common::alive_others(g, from)
            .into_iter()
            .filter(|t| gear::equip_in(g, *t, slot).is_none() && !g.slot_abolished(*t, slot))
            .collect();
        if cands.is_empty() {
            return Ok(Step::Finish("设变：无可移动目标".into()));
        }
        return match take_answer(g, "shebian") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("shebian");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("移动到 {}", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名其他角色）")
                        .data(json!({ "step": "pick", "from": from, "card": card }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "设变：非法目标")?;
                if !cands.contains(&t) {
                    return Err("设变：目标非法（同槽位需为空）".into());
                }
                gear::move_equip(g, from, &card, t)?;
                Ok(Step::Finish("设变：移动装备".into()))
            }
            Some(o) => Err(format!("设变：未知选项 {o}")),
        };
    }
    let mut equips: Vec<(usize, CardId)> = Vec::new();
    for s in g.alive_seats() {
        for id in common::equip_cards(g, s) {
            equips.push((s, id));
        }
    }
    if equips.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "shebian") {
        None => {
            let mut b = PendingBuilder::new(owner, "pick_other").skill("shebian");
            for (s, id) in &equips {
                b = b.option(
                    format!("e:{s}:{id}"),
                    format!("移动 {} 的【{}】", seat_name(g, *s), card_label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("e:") => {
            let mut it = a[2..].splitn(2, ':');
            let from: usize = it.next().and_then(|x| x.parse().ok()).ok_or("设变：非法选择")?;
            let card = it.next().ok_or("设变：缺牌")?.to_string();
            if !equips.iter().any(|(s, id)| *s == from && *id == card) {
                return Err("设变：该装备不在场上".into());
            }
            Ok(Step::Next(
                Stage::Target,
                json!({ "step": "pick", "from": from, "card": card }),
            ))
        }
        Some(o) => Err(format!("设变：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界卧龙诸葛亮 · 藏拙 / 火计 / 看破（转化与计数）
// ---------------------------------------------------------------------------

/// 弃牌阶段手牌计数（藏拙：本回合未使用锦囊时，锦囊牌不计入）。
pub fn hand_size_for_discard(g: &Game, seat: usize) -> usize {
    let total = g.hand_size(seat);
    if !has(g, seat, "cangzhuo") || g.counter(&turn_key(seat, "trick_used")) > 0 {
        return total;
    }
    let tricks = g
        .hand(seat)
        .into_iter()
        .filter(|id| {
            common::def_of(g, id)
                .map(|d| {
                    matches!(
                        d.category,
                        super::super::cards::Category::Trick
                            | super::super::cards::Category::DelayedTrick
                    )
                })
                .unwrap_or(false)
        })
        .count();
    total.saturating_sub(tricks)
}

/// 火计（红色→【火攻】）/ 连环（梅花→【铁索连环】）。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    let d = common::def_of(g, id)?;
    if has(g, seat, "huoji") && is_red_own(g, seat, id) && d.code != "huogong" {
        return Some("huogong");
    }
    if has(g, seat, "lianhuan") && suit_own(g, seat, id) == Some(Suit::Club) && d.code != "tie_suo_lian_huan" {
        return Some("tie_suo_lian_huan");
    }
    None
}

/// 看破：黑色牌当【无懈可击】（判定区外的黑牌）。
pub fn response_as(g: &Game, seat: usize, id: &str, kind: ResponseKind) -> CardAs {
    if kind != ResponseKind::Wuxie || !has(g, seat, "kanpo") {
        return CardAs::NoOpinion;
    }
    if !matches!(g.cards.get(id), Some(Zone::Hand(_)) | Some(Zone::Equip(..))) {
        return CardAs::NoOpinion;
    }
    if is_red_own(g, seat, id) {
        return CardAs::NoOpinion;
    }
    if common::def_of(g, id).map(|d| d.tags.wuxie).unwrap_or(false) {
        return CardAs::NoOpinion;
    }
    CardAs::As("wuxie_keji")
}

/// 看破：转化的【无懈可击】不能被响应（跳过无懈链）。
pub fn wuxie_no_chain(g: &Game, seat: usize, card: &str) -> bool {
    if !has(g, seat, "kanpo") {
        return false;
    }
    let real = common::def_of(g, card).map(|d| d.tags.wuxie).unwrap_or(false);
    !real && !is_red_own(g, seat, card)
}

/// 连环：梅花重铸视为「使用牌」。
pub fn on_recast(g: &mut Game, seat: usize, card: &str) {
    if !has(g, seat, "lianhuan") || suit_own(g, seat, card) != Some(Suit::Club) {
        return;
    }
    g.bump(&turn_key(seat, "trick_used"));
    let mut ev = Event::simple(EventKind::CardUsed, Some(seat), Some("连环·重铸"));
    ev.card = Some(card.to_string());
    ev.by = Some(seat);
    let e = ev.snapshot(g);
    g.push_event(e);
}

// ---------------------------------------------------------------------------
// 界庞统 · 涅槃
// ---------------------------------------------------------------------------

fn niepan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    let step = step_of(ask);
    // 限定门禁只在**首次进入**校验（用后仍要完成后续步骤）。
    if !has(g, owner, "niepan") || (step != "grant" && skills::limited_used(g, owner, "niepan")) {
        return Ok(Step::Done);
    }
    if step == "grant" {
        let cands = ["bazhen", "huoji", "kanpo"];
        return match take_answer(g, "niepan") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("niepan");
                for c in cands {
                    let name = skills::def(c).map(|d| d.name).unwrap_or(c);
                    b = b.option(format!("g:{c}"), format!("获得「{name}」"));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一个技能）")
                        .data(json!({ "step": "grant" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("g:") => {
                let c = a[2..].to_string();
                if !cands.contains(&c.as_str()) {
                    return Err("涅槃：非法技能".into());
                }
                skills::grant(g, owner, &c);
                Ok(Step::Finish("涅槃：获得技能".into()))
            }
            Some(o) => Err(format!("涅槃：未知选项 {o}")),
        };
    }
    match take_answer(g, "niepan") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("niepan");
            b = b.option("use", "涅槃：弃所有牌、复原、摸三张、回复至 3");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "ask" })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => {
            skills::mark_limited_used(g, owner, "niepan");
            // 弃置区域内所有牌（手牌/装备/判定区）。
            let mut ids = g.hand(owner);
            ids.extend(common::equip_cards(g, owner));
            ids.extend(
                g.cards
                    .iter()
                    .filter(|(_, z)| matches!(z, Zone::Judge(s) if *s == owner))
                    .map(|(id, _)| id.clone()),
            );
            for id in ids {
                g.cards.insert(id.clone(), Zone::Discard);
                let mut ev = Event::card(
                    EventKind::CardDiscarded,
                    Some(owner),
                    id.clone(),
                    Some("涅槃".to_string()),
                );
                ev.by = Some(owner);
                let e = ev.snapshot(g);
                g.push_event(e);
            }
            if let Some(s) = g.seats.get_mut(owner) {
                s.chained = false;
                s.flipped = false;
            }
            g.judge_stacks.insert(owner, vec![]);
            let max_hp = g.seats.get(owner).map(|s| s.max_hp).unwrap_or(3);
            flow::draw(g, owner, 3);
            let target_hp = 3.min(max_hp);
            if let Some(s) = g.seats.get_mut(owner) {
                s.hp = target_hp;
            }
            // 濒死帧回传：hp 已复原（≥1），濒死帧据此收口。
            g.set_mark(&format!("dying.niepan.{owner}"), "ok");
            Ok(Step::Next(Stage::Choose, json!({ "step": "grant" })))
        }
    }
}

// ---------------------------------------------------------------------------
// 界魏延 · 狂骨 / 奇谋
// ---------------------------------------------------------------------------

fn kuanggu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "kuanggu") {
        return Ok(Step::Done);
    }
    let victim = ask.data.get("victim").and_then(|v| v.as_u64()).map(|v| v as usize);
    let Some(victim) = victim else { return Ok(Step::Done) };
    if victim == owner || gear::distance(g, owner, victim) > 1 {
        return Ok(Step::Done);
    }
    match take_answer(g, "kuanggu") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("kuanggu");
            b = b.option("draw", "摸一张牌");
            b = b.option("heal", "回复 1 点体力");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "ask", "victim": victim })).build(),
            ))
        }
        Some(a) if a == "draw" => {
            flow::draw(g, owner, 1);
            Ok(Step::Finish("狂骨：摸一张".into()))
        }
        Some(a) if a == "heal" => {
            flow::heal(g, owner, 1, "狂骨");
            Ok(Step::Finish("狂骨：回复 1 点".into()))
        }
        Some(_) => Ok(Step::Finish("狂骨：不发动".into())),
    }
}

fn can_qimou(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.turn == seat
        && has(g, seat, "qimou")
        && !skills::limited_used(g, seat, "qimou")
}

fn qimou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if step_of(ask) == "after" {
        if !g.seats.get(owner).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        let x = ask.data.get("x").and_then(|v| v.as_i64()).unwrap_or(0).max(0);
        flow::draw(g, owner, x as usize);
        g.set_counter(&turn_key(owner, "qimou_x"), x as i32);
        return Ok(Step::Finish(format!("奇谋：摸 {x} 张")));
    }
    if !can_qimou(g, owner) {
        return Ok(Step::Done);
    }
    let hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0).max(0);
    match take_answer(g, "qimou") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("qimou");
            for x in 0..=hp {
                b = b.option(format!("x:{x}"), format!("失去 {x} 点体力，摸 {x} 张"));
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("x:") => {
            let x: i32 = a[2..].parse().map_err(|_| "奇谋：非法体力值")?;
            if x > hp {
                return Err("奇谋：不能超过当前体力".into());
            }
            skills::mark_limited_used(g, owner, "qimou");
            if x > 0 {
                flow::lose_hp(g, owner, x, "奇谋");
            }
            // `lose_hp` 可能压入濒死帧：保持本帧，等濒死结算后（"after"）再摸牌。
            Ok(Step::Next(Stage::Choose, json!({ "step": "after", "x": x })))
        }
        Some(o) => Err(format!("奇谋：未知选项 {o}")),
    }
}

/// 涅槃：濒死窗口的可用技能（`card_play::step_dying` 唯一调用点）。
pub fn dying_options(g: &Game, asker: usize, target: usize) -> Vec<(&'static str, String)> {
    if asker != target || !has(g, asker, "niepan") || skills::limited_used(g, asker, "niepan") {
        return vec![];
    }
    if g.deck.is_empty() && g.cards.values().all(|z| *z != Zone::Discard) {
        return vec![];
    }
    vec![("niepan", "涅槃".to_string())]
}

/// 奇谋：本回合计算与其他角色的距离 -X。
pub fn distance_bonus(g: &Game, seat: usize) -> i32 {
    -g.counter(&turn_key(seat, "qimou_x")).max(0)
}

/// 奇谋：本回合【杀】次数上限 +X。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    g.counter(&turn_key(seat, "qimou_x")).max(0)
}

// ---------------------------------------------------------------------------
// 界小乔 · 天香 / 飘零
// ---------------------------------------------------------------------------

/// 天香是否可能被触发（`ask_damage_before_victim` 的门控）。
pub fn should_ask_damage_before_victim(g: &Game, from: usize, target: usize) -> bool {
    if from == target || !has(g, target, "tianxiang") {
        return false;
    }
    common::own_cards(g, target)
        .into_iter()
        .any(|id| is_heart_own(g, target, &id))
}

fn tianxiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "tianxiang") {
        return Ok(Step::Done);
    }
    let from = common::seat_of(ask, "from").unwrap_or(owner);
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let step = step_of(ask);
    if step == "mode" {
        let card = common::card_of(ask, "card").unwrap_or_default();
        let cands = common::alive_others(g, owner);
        if cands.is_empty() {
            return Ok(Step::Finish("天香：无可转移目标".into()));
        }
        return match take_answer(g, "tianxiang") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("tianxiang");
                b = b.option("m1", "①令一名其他角色受到伤害来源的 1 点伤害，然后其摸 X 张");
                b = b.option("m2", "②令一名其他角色失去 1 点体力并获得你弃置的牌");
                Ok(Step::Need(
                    b.escape_mandatory("（必选一项）")
                        .data(json!({ "step": "mode", "card": card, "from": from }))
                        .build(),
                ))
            }
            Some(a) if a == "m1" || a == "m2" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "card": card, "from": from, "mode": a }),
            )),
            Some(o) => Err(format!("天香：未知选项 {o}")),
        };
    }
    if step == "target" {
        let card = common::card_of(ask, "card").unwrap_or_default();
        let mode = ask.data.get("mode").and_then(|v| v.as_str()).unwrap_or("m1").to_string();
        let cands = common::alive_others(g, owner);
        return match take_answer(g, "tianxiang") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("tianxiang");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("转移给 {}", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名其他角色）")
                        .data(json!({ "step": "target", "card": card, "from": from, "mode": mode }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "天香：非法目标")?;
                if !cands.contains(&t) {
                    return Err("天香：不能转移给自己".into());
                }
                flow::discard(g, owner, &card, "天香");
                if mode == "m1" {
                    let spec = DamageSpec {
                        from: Some(from),
                        target: t,
                        amount: 1,
                        kind: DamageKind::Normal,
                        source: SourceKind::Other,
                    };
                    let armor = gear::armor_of(g, t);
                    let _ = flow::damage(g, spec, armor, 0);
                    let lost = g
                        .seats
                        .get(t)
                        .map(|s| (s.max_hp - s.hp).max(0) as usize)
                        .unwrap_or(0)
                        .min(5);
                    flow::draw(g, t, lost);
                } else {
                    flow::lose_hp(g, t, 1, "天香");
                    flow::gain(g, t, &card, owner, "天香");
                }
                Ok(Step::Finish("天香：转移伤害（防止原伤害）".into()))
            }
            Some(o) => Err(format!("天香：未知选项 {o}")),
        };
    }
    if step == "pick" {
        let hearts: Vec<CardId> = own_cards(g, owner)
            .into_iter()
            .filter(|id| is_heart_own(g, owner, id))
            .collect();
        if hearts.is_empty() {
            return Ok(Step::Finish("天香：无红桃牌".into()));
        }
        return match take_answer(g, "tianxiang") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("tianxiang");
                for id in &hearts {
                    b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必弃一张红桃牌）")
                        .data(json!({ "step": "pick", "from": from }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !hearts.contains(&id) {
                    return Err("天香：只能弃置红桃牌".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "mode", "card": id, "from": from }),
                ))
            }
            Some(o) => Err(format!("天香：未知选项 {o}")),
        };
    }
    if amount <= 0 {
        return Ok(Step::Done);
    }
    let hearts: Vec<CardId> = own_cards(g, owner)
        .into_iter()
        .filter(|id| is_heart_own(g, owner, id))
        .collect();
    if hearts.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "tianxiang") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("tianxiang");
            b = b.option("use", "天香：弃一张红桃牌防止此伤害");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "from": from, "amount": amount }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("天香：不发动".into())),
        Some(_) => Ok(Step::Next(
            Stage::PickOwn,
            json!({ "step": "pick", "from": from }),
        )),
    }
}

fn piaoling(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "piaoling") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "result" {
        let heart = ask.data.get("red").and_then(|v| v.as_bool()).unwrap_or(false);
        let card = common::card_of(ask, "judge_card");
        if !heart {
            return Ok(Step::Finish("飘零：非红桃，不结算".into()));
        }
        let Some(card) = card else { return Ok(Step::Done) };
        return match take_answer(g, "piaoling") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("piaoling");
                b = b.option("top", format!("【{}】置于牌堆顶", card_label(g, &card)));
                b = b.option("give", "交给一名角色");
                Ok(Step::Need(
                    b.escape("不处理")
                        .data(json!({ "step": "result", "judge_card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "top" => {
                flow::to_deck_top(g, &card);
                Ok(Step::Finish("飘零：置于牌堆顶".into()))
            }
            Some(a) if a == "give" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "give", "judge_card": card }),
            )),
            Some(_) => Ok(Step::Finish("飘零：不处理".into())),
        };
    }
    if step == "give" {
        let card = common::card_of(ask, "judge_card").unwrap_or_default();
        let cands = g.alive_seats();
        return match take_answer(g, "piaoling") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("piaoling");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("交给 {}", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名角色）")
                        .data(json!({ "step": "give", "judge_card": card }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "飘零：非法目标")?;
                if !cands.contains(&t) {
                    return Err("飘零：目标非法".into());
                }
                flow::gain(g, t, &card, owner, "飘零");
                if t == owner {
                    // 给自己 → 弃置一张手牌。
                    let hand = g.hand(owner);
                    if hand.is_empty() {
                        return Ok(Step::Finish("飘零：无手牌可弃".into()));
                    }
                    return Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "discard" }),
                    ));
                }
                Ok(Step::Finish("飘零：交给角色".into()))
            }
            Some(o) => Err(format!("飘零：未知选项 {o}")),
        };
    }
    if step == "discard" {
        let hand = g.hand(owner);
        return match take_answer(g, "piaoling") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("piaoling");
                for id in &hand {
                    b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必弃一张）")
                        .data(json!({ "step": "discard" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !hand.contains(&id) {
                    return Err("飘零：该牌不在手牌".into());
                }
                flow::discard(g, owner, &id, "飘零");
                Ok(Step::Finish("飘零：弃置一张手牌".into()))
            }
            Some(o) => Err(format!("飘零：未知选项 {o}")),
        };
    }
    // 结束阶段开始：判定（可被改判）。
    super::super::quick_judge::start(
        g,
        owner,
        "飘零",
        Some(("piaoling", owner, json!({ "step": "result" }))),
    );
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 界孙坚 · 英魂 / 武烈
// ---------------------------------------------------------------------------

fn yinghun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "yinghun") || g.turn != owner {
        return Ok(Step::Done);
    }
    let wounded = g
        .seats
        .get(owner)
        .map(|s| (s.max_hp - s.hp).max(0) as usize)
        .unwrap_or(0);
    if wounded == 0 {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "target" {
        let mode = ask.data.get("mode").and_then(|v| v.as_str()).unwrap_or("m1").to_string();
        let cands = common::alive_others(g, owner);
        return match take_answer(g, "yinghun") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("yinghun");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("目标 {}", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "target", "mode": mode, "x": wounded }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "英魂：非法目标")?;
                if !cands.contains(&t) {
                    return Err("英魂：目标非法（不含自己）".into());
                }
                if mode == "m1" {
                    flow::draw(g, t, wounded);
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "discard1", "target": t }),
                    ));
                }
                flow::draw(g, t, 1);
                for _ in 0..wounded.min(g.hand_size(t)) {
                    if g.hand(t).is_empty() {
                        break;
                    }
                    let first = g.hand(t).remove(0);
                    flow::discard(g, t, &first, "英魂");
                }
                Ok(Step::Finish("英魂：摸 1 弃 X".into()))
            }
            Some(o) => Err(format!("英魂：未知选项 {o}")),
        };
    }
    if step == "discard1" {
        let t = common::seat_of(ask, "target").unwrap_or(owner);
        let hand = g.hand(t);
        if hand.is_empty() {
            return Ok(Step::Finish("英魂：无牌可弃".into()));
        }
        return match take_answer(g, "yinghun") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_other").skill("yinghun");
                for id in &hand {
                    b = b.option(format!("c:{id}"), format!("弃置其【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必弃一张）")
                        .data(json!({ "step": "discard1", "target": t }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !hand.contains(&id) {
                    return Err("英魂：该牌不在其手牌".into());
                }
                flow::discard(g, owner, &id, "英魂");
                Ok(Step::Finish("英魂：摸 X 弃 1".into()))
            }
            Some(o) => Err(format!("英魂：未知选项 {o}")),
        };
    }
    match take_answer(g, "yinghun") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("yinghun");
            b = b.option("m1", format!("①令其摸 {wounded} 张，然后弃一张牌"));
            b = b.option("m2", format!("②令其摸一张牌，然后弃 {wounded} 张牌"));
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "mode" })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "m1" || a == "m2" => Ok(Step::Next(
            Stage::Target,
            json!({ "step": "target", "mode": a, "x": wounded }),
        )),
        Some(o) => Err(format!("英魂：未知选项 {o}")),
    }
}

/// 武烈：「烈」抵消该次全部伤害（`skills::prevent_damage` 单出口）。
pub fn prevent_damage(g: &mut Game, victim: usize) -> bool {
    let key = format!("lie.{victim}");
    if g.counter(&key) <= 0 {
        return false;
    }
    g.set_counter(&key, g.counter(&key) - 1);
    true
}

fn wulie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    let step = step_of(ask);
    if !has(g, owner, "wulie")
        || g.turn != owner
        || (step == "ask" && skills::limited_used(g, owner, "wulie"))
    {
        return Ok(Step::Done);
    }
    if step == "seat" {
        let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        if left <= 0 {
            return Ok(Step::Finish("武烈：结算完成".into()));
        }
        let cands = common::alive_others(g, owner);
        if cands.is_empty() {
            return Ok(Step::Finish("武烈：无其他角色".into()));
        }
        return match take_answer(g, "wulie") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("wulie");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("令 {} 获得「烈」", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名其他角色）")
                        .data(json!({ "step": "seat", "left": left }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "武烈：非法目标")?;
                if !cands.contains(&t) {
                    return Err("武烈：目标非法".into());
                }
                g.bump(&format!("lie.{t}"));
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "seat", "left": left - 1 }),
                ))
            }
            Some(o) => Err(format!("武烈：未知选项 {o}")),
        };
    }
    if step == "amount" {
        let hp = ask.data.get("hp").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as i32;
        let mut b = PendingBuilder::new(owner, "choose").skill("wulie");
        for x in 0..=hp {
            b = b.option(format!("x:{x}"), format!("失去 {x} 点体力"));
        }
        return match take_answer(g, "wulie") {
            None => Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "amount", "hp": hp }))
                    .build(),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("x:") => {
                let x: i32 = a[2..].parse().map_err(|_| "武烈：非法体力值")?;
                if x > hp {
                    return Err("武烈：不能超过当前体力".into());
                }
                skills::mark_limited_used(g, owner, "wulie");
                if x > 0 {
                    flow::lose_hp(g, owner, x, "武烈");
                }
                Ok(Step::Next(Stage::Target, json!({ "step": "seat", "left": x })))
            }
            Some(o) => Err(format!("武烈：未知选项 {o}")),
        };
    }
    let hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0).max(0);
    match take_answer(g, "wulie") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("wulie");
            b = b.option("use", "武烈：失去任意点体力，为其他角色授予「烈」");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "ask" })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "amount", "hp": hp }),
        )),
    }
}

// ---------------------------------------------------------------------------
// 界庞德 · 鞬出
// ---------------------------------------------------------------------------

fn jiandu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiandu") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "pick" {
        let victim = common::seat_of(ask, "victim").unwrap_or(owner);
        let card = common::card_of(ask, "card").unwrap_or_default();
        let cands = own_cards(g, victim);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "jiandu") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_other").skill("jiandu");
                for id in &cands {
                    b = b.option(format!("c:{id}"), format!("弃置其【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一张）")
                        .data(json!({ "step": "pick", "victim": victim, "card": card }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !cands.contains(&id) {
                    return Err("鞬出：该牌不在其区域".into());
                }
                let basic = common::def_of(g, &id)
                    .map(|d| d.category == super::super::cards::Category::Basic)
                    .unwrap_or(false);
                if basic {
                    // 基本牌：其获得此【杀】，本次【杀】仍继续结算。
                    flow::gain(g, victim, &id, owner, "鞬出");
                    if matches!(g.cards.get(&card), Some(Zone::Processing)) {
                        flow::gain(g, victim, &card, owner, "鞬出");
                    }
                    Ok(Step::Finish("鞬出：其获得此【杀】，继续结算".into()))
                } else {
                    flow::discard(g, owner, &id, "鞬出");
                    // 非基本牌：其不能抵消此【杀】，且本回合【杀】次数 +1（最多 +1）。
                    g.use_opts_mut(&card).unresponsive.push(victim);
                    if g.counter(&turn_key(owner, "jiandu_sha")) == 0 {
                        g.bump(&turn_key(owner, "jiandu_sha"));
                    }
                    Ok(Step::Finish("鞬出：其不能抵消，次数 +1".into()))
                }
            }
            Some(o) => Err(format!("鞬出：未知选项 {o}")),
        };
    }
    let card = common::card_of(ask, "card").unwrap_or_default();
    let targets = common::seats_of(ask, "targets");
    let victim = targets.first().copied().unwrap_or(owner);
    if victim == owner || own_cards(g, victim).is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "jiandu") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("jiandu");
            b = b.option("use", format!("弃置 {} 一张牌", seat_name(g, victim)));
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "victim": victim, "card": card }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "pick", "victim": victim, "card": card }),
        )),
    }
}

#[allow(dead_code)]
fn _unused(_: &Game, _: &mut Game) {
    let _ = (cards::def, seat_name);
}
