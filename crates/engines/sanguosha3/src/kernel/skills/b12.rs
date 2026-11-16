//! B12 技能（界限突破·界标·群：4 将 / 8 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B12.json` 的 `agreed`（权威）。
//!
//! 内核接入：急救（濒死救援转化）、无双（响应张数 `skills::response_need`）、
//! 义从（双向距离：`Hook::Distance` + `distance_to_bonus`）。

use serde_json::json;

use super::super::cards::ResponseKind;
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, Game, Phase, Slot, Zone};
use super::common::{self, card_label, def_of, own_cards, phase_key, seat_name};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界华佗 ——
    SkillDef { code: "jijiu", name: "急救", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherTurn, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "chuli", name: "除疠", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界吕布 ——
    SkillDef { code: "wushuang", name: "无双", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "liyu", name: "利驭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    // —— 界公孙瓒 ——
    SkillDef { code: "yicong", name: "义从", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::Distance(-1)], can_active: false }] },
    SkillDef { code: "qiaomeng", name: "趫猛", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    // —— 界貂蝉 ——
    SkillDef { code: "lijian", name: "离间", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "biyue", name: "闭月", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "chuli" => chuli,
        "liyu" => liyu,
        "qiaomeng" => qiaomeng,
        "lijian" => lijian,
        "biyue" => biyue,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "chuli" => can_chuli,
        "lijian" => can_lijian,
        _ => return None,
    })
}

fn step_of(ask: &Ask) -> &str {
    ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask")
}

fn event_card(ask: &Ask) -> Option<CardId> {
    ask.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn is_owner_in(zone: Option<&Zone>, seat: usize) -> bool {
    matches!(zone, Some(Zone::Hand(s)) | Some(Zone::Equip(s, _)) if *s == seat)
}

/// 「自己使用【杀】类牌造成、受害者存活」的伤害事件（利驭 / 趫猛共用）。
fn own_sha_damage(g: &Game, ask: &Ask, owner: usize) -> Option<(usize, CardId)> {
    let by = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize)?;
    if by != owner {
        return None;
    }
    let victim = ask.data.get("victim").and_then(|v| v.as_u64()).map(|v| v as usize)?;
    if victim == owner || !g.seats.get(victim).map(|s| s.alive).unwrap_or(false) {
        return None;
    }
    let card = event_card(ask)?;
    if !def_of(g, &card).map(|d| d.tags.sha).unwrap_or(false) {
        return None;
    }
    Some((victim, card))
}

/// 目标区域选一张：装备区可选（可读），手牌只给「随机」选项。
fn pick_one_from(
    g: &Game,
    actor: usize,
    owner: usize,
    skill: &'static str,
    verb: &str,
    data: serde_json::Value,
) -> Option<Step> {
    let mut b = PendingBuilder::new(actor, "pick_other").skill(skill);
    let mut any = false;
    for id in common::equip_cards(g, owner) {
        b = b.option(
            format!("equip:{id}"),
            format!("{verb}{}的【{}】", seat_name(g, owner), card_label(g, &id)),
        );
        any = true;
    }
    if !g.hand(owner).is_empty() {
        b = b.option("hand", format!("{verb}{}一张手牌（随机）", seat_name(g, owner)));
        any = true;
    }
    if !any {
        return None;
    }
    Some(Step::Need(b.escape("不选").data(data).build()))
}

fn resolve_pick(g: &mut Game, owner: usize, ans: &str) -> Option<CardId> {
    if ans == "hand" {
        let hand = g.hand(owner);
        if hand.is_empty() {
            return None;
        }
        let i = g.rand_below(hand.len());
        return hand.get(i).cloned();
    }
    ans.strip_prefix("equip:").map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// 界华佗 · 急救（回合外，红色手牌/装备牌当【桃】；判定区不参与）
// ---------------------------------------------------------------------------

/// 濒死救援候选（`card_play::step_dying` 唯一调用点）。
pub fn jijiu_options(g: &Game, asker: usize) -> Vec<CardId> {
    if !skills::active(g, asker, "jijiu") || !skills::enabled("jijiu") {
        return vec![];
    }
    // 回合外才可用。
    if g.turn == asker {
        return vec![];
    }
    own_cards(g, asker)
        .into_iter()
        .filter(|id| g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false))
        .collect()
}

/// 急救结算：承载牌进弃牌堆（装备区走统一离开钩子），由调用方回复 1 点体力。
pub fn use_jijiu(g: &mut Game, asker: usize, id: &str) -> Result<(), String> {
    if !jijiu_options(g, asker).iter().any(|c| c == id) {
        return Err("急救：该牌不可作为【桃】".into());
    }
    let was_equip = matches!(g.cards.get(id), Some(Zone::Equip(..)));
    if was_equip {
        flow::leave_equip(g, asker, id, "急救");
    }
    g.cards.insert(id.to_string(), Zone::Discard);
    let mut ev = super::super::event::Event::card(
        EventKind::CardLost,
        Some(asker),
        id.to_string(),
        Some("急救".to_string()),
    );
    ev.by = Some(asker);
    ev.from = Some(asker);
    let e = ev.snapshot(g);
    g.push_event(e);
    Ok(())
}

// ---------------------------------------------------------------------------
// 界华佗 · 除疠（出牌阶段限一次：势力各不相同、有牌的其他角色；各弃一张；黑桃者摸 1）
// ---------------------------------------------------------------------------

fn can_chuli(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "chuli") {
        return false;
    }
    if g.phase != Phase::Play || g.turn != seat {
        return false;
    }
    if g.counter(&phase_key(seat, "chuli")) > 0 {
        return false;
    }
    if own_cards(g, seat).is_empty() {
        return false;
    }
    candidates(g, seat, &[]).iter().any(|_| true)
}

fn candidates(g: &Game, owner: usize, picked: &[usize]) -> Vec<usize> {
    let used_kingdoms: Vec<String> = picked
        .iter()
        .filter_map(|s| g.seats.get(*s).map(|x| x.kingdom.clone()))
        .collect();
    common::alive_others(g, owner)
        .into_iter()
        .filter(|s| own_cards(g, *s).len() > 0)
        .filter(|s| !picked.contains(s))
        .filter(|s| {
            g.seats
                .get(*s)
                .map(|x| !used_kingdoms.iter().any(|k| *k == x.kingdom))
                .unwrap_or(false)
        })
        .collect()
}

fn chuli(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "chuli") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "ask" || step == "choose" {
        return Ok(Step::Next(Stage::Target, json!({ "step": "select", "picked": [] })));
    }
    if step == "select" {
        let picked = common::seats_of(ask, "picked");
        let cands = candidates(g, owner, &picked);
        return match take_answer(g, "chuli") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("chuli");
                for t in &cands {
                    b = b.option(
                        format!("t:{t}"),
                        format!("{}（{}）", seat_name(g, *t), g.seats[*t].kingdom),
                    );
                }
                if !picked.is_empty() {
                    b = b.option("confirm", "确认（开始各自弃牌）");
                }
                let data = json!({ "step": "select", "picked": picked });
                Ok(Step::Need(b.escape("取消").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "confirm" => {
                if picked.is_empty() {
                    return Err("除疠：至少选择一名其他角色".into());
                }
                let mut part = vec![owner];
                part.extend(picked);
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "discard", "part": part, "idx": 0, "drawers": [] }),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "除疠：非法目标")?;
                if !cands.contains(&t) {
                    return Err("除疠：目标非法（势力需各不相同且需有牌）".into());
                }
                let mut np = picked;
                np.push(t);
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "select", "picked": np }),
                ))
            }
            Some(o) => Err(format!("除疠：未知选项 {o}")),
        };
    }
    // discard：逐个参与者自选一张弃置（黑桃记录）。
    let part = common::seats_of(ask, "part");
    let mut idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut drawers = common::seats_of(ask, "drawers");
    while idx < part.len() {
        let who = part[idx];
        if !g.seats.get(who).map(|s| s.alive).unwrap_or(false) || own_cards(g, who).is_empty() {
            idx += 1;
            continue;
        }
        let data = json!({ "step": "discard", "part": part, "idx": idx, "drawers": drawers });
        return match take_answer(g, "chuli") {
            None => {
                let mut b = PendingBuilder::new(who, "pick_own").skill("chuli");
                for id in own_cards(g, who) {
                    b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, &id)));
                }
                Ok(Step::Need(b.escape_mandatory("弃置一张牌").data(data).build()))
            }
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !is_owner_in(g.cards.get(&id), who) {
                    return Err("除疠：该牌不在该角色的手牌/装备区".into());
                }
                let spade = g.card_defs.get(&id).map(|c| c.suit == super::super::Suit::Spade).unwrap_or(false);
                flow::discard(g, who, &id, "除疠");
                if spade && !drawers.contains(&who) {
                    drawers.push(who);
                }
                idx += 1;
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "discard", "part": part, "idx": idx, "drawers": drawers }),
                ))
            }
            Some(o) => Err(format!("除疠：未知选项 {o}")),
        };
    }
    // 「然后」：被弃置黑桃的参与者（含自己）各摸一张。
    for who in drawers.iter() {
        if g.seats.get(*who).map(|s| s.alive).unwrap_or(false) {
            flow::draw(g, *who, 1);
        }
    }
    g.bump(&phase_key(owner, "chuli"));
    Ok(Step::Finish("除疠：结算完成".into()))
}

// ---------------------------------------------------------------------------
// 界吕布 · 无双（锁定：响应需连续两张）
// ---------------------------------------------------------------------------

/// 响应张数（`card_play` 唯一调用点）：`opponent` 有【无双】→ 2 张。
pub fn response_need(g: &Game, _responder: usize, opponent: usize, _kind: ResponseKind) -> u32 {
    if skills::active(g, opponent, "wushuang") && skills::enabled("wushuang") {
        2
    } else {
        1
    }
}

// ---------------------------------------------------------------------------
// 界吕布 · 利驭（自己【杀】造成伤害后：获得其一张牌；非装备 → 其摸 1；装备 → 视为决斗）
// ---------------------------------------------------------------------------

fn liyu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "liyu") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "pick" {
        let victim = common::seat_of(ask, "victim").unwrap_or(owner);
        let data = json!({ "step": "pick", "victim": victim });
        if let Some(ans) = take_answer(g, "liyu") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = resolve_pick(g, victim, &ans) else {
                return Err(format!("利驭：非法选项 {ans}"));
            };
            if !is_owner_in(g.cards.get(&id), victim) {
                return Err("利驭：该牌不在目标区域".into());
            }
            let is_equip = def_of(g, &id).map(|d| d.slot.is_some()).unwrap_or(false);
            flow::gain(g, owner, &id, owner, "利驭");
            if !is_equip {
                flow::draw(g, victim, 1);
                return Ok(Step::Finish("利驭：获得非装备牌，其摸一张".into()));
            }
            // 装备牌 → 受害者指定另一名角色，吕布视为对其使用【决斗】。
            return Ok(Step::Next(
                Stage::Target,
                json!({ "step": "designate", "victim": victim, "card": id }),
            ));
        }
        let Some(st) = pick_one_from(g, owner, victim, "liyu", "获得", data) else {
            return Ok(Step::Finish("利驭：目标无牌可拿".into()));
        };
        return Ok(st);
    }
    if step == "designate" {
        let victim = common::seat_of(ask, "victim").unwrap_or(owner);
        let cands: Vec<usize> = common::alive_others(g, victim)
            .into_iter()
            .filter(|s| *s != owner && *s != victim)
            .collect();
        if cands.is_empty() {
            return Ok(Step::Finish("利驭：无可指定目标".into()));
        }
        return match take_answer(g, "liyu") {
            None => {
                let mut b = PendingBuilder::new(victim, "target").skill("liyu");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("令吕布对 {} 使用【决斗】", seat_name(g, *t)));
                }
                let data = json!({ "step": "designate", "victim": victim });
                Ok(Step::Need(b.escape_mandatory("指定一名其他角色").data(data).build()))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "利驭：非法目标")?;
                if !cands.contains(&t) {
                    return Err("利驭：目标非法".into());
                }
                super::super::card_play::use_virtual(g, owner, "juedou", &[t], false)?;
                Ok(Step::Finish("利驭：视为对指定角色使用【决斗】".into()))
            }
            Some(o) => Err(format!("利驭：未知选项 {o}")),
        };
    }
    let Some((victim, _card)) = own_sha_damage(g, ask, owner) else {
        return Ok(Step::Done);
    };
    if own_cards(g, victim).is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "liyu") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("liyu");
            b = b.option("use", format!("获得 {} 一张牌", seat_name(g, victim)));
            let data = json!({ "step": "ask", "victim": victim });
            Ok(Step::Need(b.escape("不发动").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "pick", "victim": victim }),
        )),
        Some(o) => Err(format!("利驭：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界公孙瓒 · 义从（距离：我 -1；体力 ≤2 时他人对我 +1）
// ---------------------------------------------------------------------------

/// 「他人 → 我」距离修正（`gear::distance` 唯一调用点）。
pub fn distance_to_bonus(g: &Game, seat: usize) -> i32 {
    if !skills::active(g, seat, "yicong") || !skills::enabled("yicong") {
        return 0;
    }
    if g.seats.get(seat).map(|s| s.hp <= 2).unwrap_or(false) {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 界公孙瓒 · 趫猛（自己【杀】造成伤害后：弃置其一张牌；坐骑牌改为获得）
// ---------------------------------------------------------------------------

fn qiaomeng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "qiaomeng") {
        return Ok(Step::Done);
    }
    if step_of(ask) == "pick" {
        let victim = common::seat_of(ask, "victim").unwrap_or(owner);
        let data = json!({ "step": "pick", "victim": victim });
        if let Some(ans) = take_answer(g, "qiaomeng") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = resolve_pick(g, victim, &ans) else {
                return Err(format!("趫猛：非法选项 {ans}"));
            };
            if !is_owner_in(g.cards.get(&id), victim) {
                return Err("趫猛：该牌不在目标区域".into());
            }
            let is_horse = def_of(g, &id)
                .map(|d| matches!(d.slot, Some(Slot::HorseOffense) | Some(Slot::HorseDefense)))
                .unwrap_or(false);
            if is_horse {
                flow::gain(g, owner, &id, owner, "趫猛");
                return Ok(Step::Finish("趫猛：坐骑牌改为获得".into()));
            }
            flow::discard(g, owner, &id, "趫猛");
            return Ok(Step::Finish("趫猛：弃置其一张牌".into()));
        }
        let Some(st) = pick_one_from(g, owner, victim, "qiaomeng", "弃置", data) else {
            return Ok(Step::Finish("趫猛：目标无牌可弃".into()));
        };
        return Ok(st);
    }
    let Some((victim, _card)) = own_sha_damage(g, ask, owner) else {
        return Ok(Step::Done);
    };
    if own_cards(g, victim).is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "qiaomeng") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("qiaomeng");
            b = b.option("use", format!("弃置 {} 一张牌", seat_name(g, victim)));
            let data = json!({ "step": "ask", "victim": victim });
            Ok(Step::Need(b.escape("不发动").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "pick", "victim": victim }),
        )),
        Some(o) => Err(format!("趫猛：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界貂蝉 · 离间（出牌阶段限一次：弃一张牌 → 男性 A 视为对男性 B 使用【决斗】，不可无懈）
// ---------------------------------------------------------------------------

fn can_lijian(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "lijian") {
        return false;
    }
    if g.phase != Phase::Play || g.turn != seat {
        return false;
    }
    if g.counter(&phase_key(seat, "lijian")) > 0 {
        return false;
    }
    if own_cards(g, seat).is_empty() {
        return false;
    }
    male_seats(g).len() >= 2
}

fn male_seats(g: &Game) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|&s| g.seats[s].alive && g.seats[s].male)
        .collect()
}

fn lijian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "lijian") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    match step {
        "discard" => {
            let data = json!({ "step": "discard" });
            return match take_answer(g, "lijian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_own").skill("lijian");
                    for id in own_cards(g, owner) {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, &id)));
                    }
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !is_owner_in(g.cards.get(&id), owner) {
                        return Err("离间：该牌不在手牌/装备区".into());
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "attacker", "card": id }),
                    ))
                }
                Some(o) => Err(format!("离间：未知选项 {o}")),
            };
        }
        "attacker" => {
            let card = event_card(ask).ok_or("离间：缺承载牌")?;
            let cands = male_seats(g);
            return match take_answer(g, "lijian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("lijian");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("令 {} 视为使用【决斗】", seat_name(g, *t)));
                    }
                    let data = json!({ "step": "attacker", "card": card });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let atk: usize = a[2..].parse().map_err(|_| "离间：非法发起者")?;
                    if !cands.contains(&atk) {
                        return Err("离间：发起者必须是男性角色".into());
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "defender", "card": card, "atk": atk }),
                    ))
                }
                Some(o) => Err(format!("离间：未知选项 {o}")),
            };
        }
        "defender" => {
            let card = event_card(ask).ok_or("离间：缺承载牌")?;
            let atk = common::seat_of(ask, "atk").unwrap_or(owner);
            // 目标：男性、非发起者、非空城（有手牌）。
            let cands: Vec<usize> = male_seats(g)
                .into_iter()
                .filter(|s| *s != atk)
                .filter(|s| g.hand_size(*s) > 0)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Finish("离间：无可选目标".into()));
            }
            return match take_answer(g, "lijian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("lijian");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("目标 {}（先出【杀】）", seat_name(g, *t)));
                    }
                    let data = json!({ "step": "defender", "card": card, "atk": atk });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let def: usize = a[2..].parse().map_err(|_| "离间：非法目标")?;
                    if !cands.contains(&def) {
                        return Err("离间：目标非法（男性且不能是空城者）".into());
                    }
                    // 一次性弃置承载牌（装备区触发失去装备钩子）。
                    flow::discard(g, owner, &card, "离间");
                    g.bump(&phase_key(owner, "lijian"));
                    let vid = super::super::card_play::use_virtual(g, atk, "juedou", &[def], false)?;
                    g.no_wuxie(&vid);
                    Ok(Step::Finish("离间：视为使用【决斗】（不可无懈）".into()))
                }
                Some(o) => Err(format!("离间：未知选项 {o}")),
            };
        }
        _ => {
            // 首次进入：先选承载牌。
            Ok(Step::Next(Stage::PickOwn, json!({ "step": "discard" })))
        }
    }
}

// ---------------------------------------------------------------------------
// 界貂蝉 · 闭月（结束阶段开始：无手牌摸 2，否则摸 1）
// ---------------------------------------------------------------------------

fn biyue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "biyue") {
        return Ok(Step::Done);
    }
    match take_answer(g, "biyue") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("biyue");
            b = b.option("use", "发动闭月（摸牌）");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let n = if g.hand(owner).is_empty() { 2 } else { 1 };
            flow::draw(g, owner, n);
            Ok(Step::Finish(format!("闭月：摸 {n} 张")))
        }
        Some(o) => Err(format!("闭月：未知选项 {o}")),
    }
}

