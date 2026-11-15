//! B4 技能（一将成名·Y2+Y3：8 将 / 11 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B4.json` 的 `agreed`（权威）。

use serde_json::{json, Value};

use super::super::cards::{self, Category};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{DamageKind, DamageSpec, Game, Phase, SourceKind, Zone};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub const NI: &str = "ni";

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "xiansi", name: "陷嗣", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: true }] },
    SkillDef { code: "duodao", name: "夺刀", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "anjian", name: "暗箭", kind: SkillKind::Locked, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "danshou", name: "胆守", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "sidi", name: "司敌", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OtherPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }, Subscription { event: EventKind::PhaseEnd, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "shenduan", name: "慎断", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardDiscarded, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "yonglue", name: "勇略", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OtherPhase(Phase::Judge), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "pindi", name: "品第", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "faen", name: "法恩", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::Flipped, subject: Subject::Any }, Subscription { event: EventKind::ChainedEnter, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "benxi", name: "奔袭", kind: SkillKind::Locked, grant_only: false, transform: false, faces: &[Face { window: Window::OwnTurn, subscriptions: &[], hooks: &[Hook::Distance(-1), Hook::UsePre], can_active: false }] },
    SkillDef { code: "zhongyong", name: "忠勇", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::By }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "xiansi" => xiansi,
        "duodao" => duodao,
        "danshou" => danshou,
        "sidi" => sidi,
        "shenduan" => shenduan,
        "yonglue" => yonglue,
        "pindi" => pindi,
        "faen" => faen,
        "benxi" => benxi,
        "zhongyong" => zhongyong,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "xiansi" => can_xiansi,
        "danshou" => can_danshou,
        "pindi" => can_pindi,
        _ => return None,
    })
}

/// 暗箭：使用【杀】造成伤害时，若你不在其攻击范围内，伤害 +1。
pub fn damage_bonus(g: &Game, spec: &DamageSpec) -> i32 {
    let Some(from) = spec.from else { return 0 };
    if spec.source != SourceKind::Slash {
        return 0;
    }
    if !skills::enabled("anjian") || !skills::active(g, from, "anjian") {
        return 0;
    }
    if !super::b1::in_attack_range(g, spec.target, from) {
        1
    } else {
        0
    }
}

/// 奔袭：计算与其他角色的距离 -1（由 Hook::Distance 声明，这里提供条件：本回合内）。
pub fn distance_bonus(g: &Game, seat: usize) -> i32 {
    if g.turn == seat && skills::active(g, seat, "benxi") {
        -1
    } else {
        0
    }
}

/// 陷嗣（他人侧）：移去两张「逆」视为对刘封使用一张无距离【杀】。
pub fn other_side_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    let has = all_xiansi_owners(g)
        .into_iter()
        .any(|owner| owner != seat && mark_cards(g, owner, NI).len() >= 2);
    if has {
        vec!["xiansi"]
    } else {
        vec![]
    }
}

fn all_xiansi_owners(g: &Game) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|&s| skills::enabled("xiansi") && skills::active(g, s, "xiansi"))
        .collect()
}

pub fn start_other(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if code != "xiansi" || !other_side_actions(g, seat).contains(&"xiansi") {
        return Err("该他人侧技能当前不可用".into());
    }
    super::super::resolve::push_ask(g, "xiansi", Stage::Choose, seat, json!({ "step": "other" }));
    Ok(())
}

fn turn_key(seat: usize, key: &str) -> String {
    format!("turn.{seat}.{key}")
}

fn phase_key(seat: usize, key: &str) -> String {
    format!("phase.{seat}.{key}")
}

fn seat_of(ask: &Ask, key: &str) -> Option<usize> {
    ask.data.get(key).and_then(|v| v.as_u64()).map(|v| v as usize)
}

fn card_of(ask: &Ask, key: &str) -> Option<String> {
    ask.data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn card_label(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| id.to_string())
}

fn mark_cards(g: &Game, seat: usize, name: &str) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, n) if *s == seat && n == name))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn to_mark(g: &mut Game, seat: usize, name: &str, id: &str) {
    g.cards.insert(id.to_string(), Zone::Mark(seat, name.to_string()));
}

fn trigger_ask(actor: usize, skill: &'static str, label: &str, data: Value) -> Step {
    Step::Need(
        PendingBuilder::new(actor, "choose")
            .skill(skill)
            .option("use", label)
            .escape("不发动")
            .data(data)
            .build(),
    )
}

fn must_pick(g: &Game, actor: usize, skill: &'static str, kind: &'static str, verb: &str, cands: &[String], data: Value) -> Step {
    let mut b = PendingBuilder::new(actor, kind).skill(skill);
    for id in cands {
        b = b.option(format!("card:{id}"), format!("{verb}【{}】", card_label(g, id)));
    }
    Step::Need(b.escape_mandatory("（必选一张）").data(data).build())
}

fn alive_others(g: &Game, seat: usize) -> Vec<usize> {
    (0..g.seats.len()).filter(|&i| i != seat && g.seats[i].alive).collect()
}

// ---------------------------------------------------------------------------
// 陷嗣（刘封）：准备阶段可取至多两名其他角色各一张牌为「逆」；他人可移两张「逆」出杀
// ---------------------------------------------------------------------------

fn can_xiansi(g: &Game, seat: usize) -> bool {
    !alive_others(g, seat).is_empty()
}

fn xiansi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "other" {
        // 他人侧：移去两张「逆」→ 视为对刘封使用无距离【杀】。
        let Some(owner) = all_xiansi_owners(g).into_iter().find(|o| *o != actor && mark_cards(g, *o, NI).len() >= 2) else {
            return Ok(Step::Done);
        };
        let ni = mark_cards(g, owner, NI);
        for id in ni.into_iter().take(2) {
            super::super::card_play::remove_card(g, owner, &id, "陷嗣");
        }
        super::super::card_play::use_virtual(g, actor, "sha", &[owner], false)?;
        return Ok(Step::Done);
    }
    if g.phase != Phase::Prepare || !can_xiansi(g, actor) {
        return Ok(Step::Done);
    }
    let taken = ask.data.get("taken").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if taken >= 2 {
        return Ok(Step::Done);
    }
    if step == "ask" {
        return match take_answer(g, "xiansi") {
            None => Ok(trigger_ask(actor, "xiansi", "陷嗣：取至多两名其他角色各一张牌为「逆」", json!({ "step": "ask", "taken": taken }))),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::Target, json!({ "step": "target", "taken": taken }))),
            Some(o) => Err(format!("陷嗣：未知选项 {o}")),
        };
    }
    if step == "target" {
        let others: Vec<usize> = alive_others(g, actor)
            .into_iter()
            .filter(|&t| !own_cards(g, t).is_empty())
            .collect();
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "xiansi") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("xiansi");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(b.escape_mandatory("（必选目标）").data(json!({ "step": "target", "taken": taken })).build()))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "陷嗣：非法目标")?;
                if !others.contains(&t) {
                    return Err("陷嗣：目标须为有牌的其他角色".into());
                }
                Ok(Step::Next(Stage::PickOther, json!({ "step": "pick", "taken": taken, "target": t })))
            }
            Some(o) => Err(format!("陷嗣：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let cands = own_cards(g, target);
    match take_answer(g, "xiansi") {
        None => Ok(must_pick(g, actor, "xiansi", "pick_other", "置于武将牌上", &cands, json!({ "step": "pick", "taken": taken, "target": target }))),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("陷嗣：只能取其区域里的一张牌".into());
            }
            to_mark(g, actor, NI, &id);
            let taken = taken + 1;
            if taken >= 2 {
                return Ok(Step::Finish("陷嗣：置「逆」".into()));
            }
            Ok(Step::Next(Stage::Choose, json!({ "step": "ask", "taken": taken })))
        }
        Some(o) => Err(format!("陷嗣：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 夺刀（潘璋马忠）：其他角色使用【杀】对你造成伤害后，可弃一张牌获得其武器
// ---------------------------------------------------------------------------

fn duodao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(src) = seat_of(ask, "by").or_else(|| seat_of(ask, "target")) else {
        return Ok(Step::Done);
    };
    if src == actor || !g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let Some(weapon) = gear::equip_in(g, src, super::super::Slot::Weapon).cloned() else {
        return Ok(Step::Done);
    };
    let own = own_cards(g, actor);
    if own.is_empty() {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "duodao") {
            None => Ok(trigger_ask(actor, "duodao", "夺刀：弃一张牌，获得其武器", json!({ "step": "ask", "by": src }))),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::PickOwn, json!({ "step": "pick", "by": src }))),
            Some(o) => Err(format!("夺刀：未知选项 {o}")),
        };
    }
    match take_answer(g, "duodao") {
        None => Ok(must_pick(g, actor, "duodao", "pick_own", "弃置", &own, json!({ "step": "pick", "by": src }))),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) {
                return Err("夺刀：只能弃置自己的牌".into());
            }
            flow::discard(g, actor, &id, "夺刀");
            if gear::equip_in(g, src, super::super::Slot::Weapon).is_some() {
                flow::gain(g, actor, &weapon, actor, "夺刀");
            }
            Ok(Step::Finish("夺刀：获得其武器".into()))
        }
        Some(o) => Err(format!("夺刀：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 胆守（朱然）：出牌阶段可多次发动；第 X 次弃 X 张牌，按 X 结算
// ---------------------------------------------------------------------------

fn can_danshou(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && !alive_others(g, seat).is_empty()
        && own_cards(g, seat).len() >= (g.counter(&phase_key(seat, "danshou")) as usize + 1)
}

fn danshou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    let x = g.counter(&phase_key(actor, "danshou")) + 1;
    if step == "pick" {
        if !can_danshou(g, actor) {
            return Ok(Step::Done);
        }
        let picked: Vec<String> = ask.data.get("picked").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        let own = own_cards(g, actor);
        return match take_answer(g, "danshou") {
            None => {
                let mut b = PendingBuilder::new(actor, "multi_select").skill("danshou");
                for id in &own {
                    if picked.contains(id) {
                        continue;
                    }
                    b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                if picked.len() >= x as usize {
                    b = b.option("ok", "确定");
                }
                Ok(Step::Need(b.escape_with("cancel", "取消").data(json!({ "step": "pick", "picked": picked, "x": x })).build()))
            }
            Some(a) if a == "cancel" => Ok(Step::Done),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own.contains(&id) {
                    return Err("胆守：只能弃置自己的牌".into());
                }
                let mut next = picked.clone();
                next.push(id);
                Ok(Step::Next(Stage::MultiSelect, json!({ "step": "pick", "picked": next, "x": x })))
            }
            Some(a) if a == "ok" => {
                for id in &picked {
                    flow::discard(g, actor, id, "胆守");
                }
                Ok(Step::Next(Stage::Target, json!({ "step": "target", "x": x })))
            }
            Some(o) => Err(format!("胆守：未知选项 {o}")),
        };
    }
    if step == "target" {
        let reach = gear::attack_range(g, actor) as usize;
        let cands: Vec<usize> = alive_others(g, actor).into_iter().filter(|&t| gear::distance(g, actor, t) <= reach).collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "danshou") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("danshou");
                for t in &cands {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(b.escape_mandatory("（必选目标）").data(json!({ "step": "target", "x": x })).build()))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "胆守：非法目标")?;
                if !cands.contains(&t) {
                    return Err("胆守：目标须在攻击范围内".into());
                }
                g.bump(&phase_key(actor, "danshou"));
                match x {
                    1 => {
                        let c = own_cards(g, t);
                        if c.is_empty() {
                            return Ok(Step::Finish("胆守：其无牌可弃".into()));
                        }
                        Ok(Step::Next(Stage::PickOther, json!({ "step": "pick1", "target": t })))
                    }
                    2 => Ok(Step::Next(Stage::PickOther, json!({ "step": "pick2", "target": t }))),
                    3 => {
                        let spec = DamageSpec { from: Some(actor), target: t, amount: 1, kind: DamageKind::Normal, source: SourceKind::Skill };
                        let armor = gear::armor_of(g, t);
                        flow::damage(g, spec, armor, 0);
                        Ok(Step::Finish("胆守：造成 1 点伤害".into()))
                    }
                    _ => {
                        flow::draw(g, actor, 2);
                        flow::draw(g, t, 2);
                        Ok(Step::Finish("胆守：各摸两张".into()))
                    }
                }
            }
            Some(o) => Err(format!("胆守：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    if step == "pick1" {
        let c = own_cards(g, target);
        return match take_answer(g, "danshou") {
            None => Ok(must_pick(g, actor, "danshou", "pick_other", "弃置", &c, json!({ "step": "pick1", "target": target }))),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                flow::discard(g, actor, &id, "胆守");
                Ok(Step::Finish("胆守：弃其一张牌".into()))
            }
            Some(o) => Err(format!("胆守：未知选项 {o}")),
        };
    }
    // pick2：其交给你一张牌（自动由你选）。
    let c = own_cards(g, target);
    match take_answer(g, "danshou") {
        None => Ok(must_pick(g, actor, "danshou", "pick_other", "获得", &c, json!({ "step": "pick2", "target": target }))),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            flow::gain(g, actor, &id, target, "胆守");
            Ok(Step::Finish("胆守：获得其一张牌".into()))
        }
        Some(o) => Err(format!("胆守：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 司敌（曹真）：其他角色出牌阶段开始 → 弃同色非基本牌/装备，封锁其该颜色本阶段
// ---------------------------------------------------------------------------

fn sidi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else { return Ok(Step::Done) };
    let phase = g.phase;
    let started = g.counter(&format!("sidi.{actor}.{target}")) > 0;
    if phase == Phase::Play && !started {
        // 出牌阶段开始。
        if g.turn == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        let equips = gear::equips_of(g, actor);
        if equips.is_empty() {
            return Ok(Step::Done);
        }
        let colors: Vec<bool> = equips.iter().filter_map(|id| g.card_defs.get(id).map(|c| c.suit.is_red())).collect();
        let cands: Vec<String> = own_cards(g, actor)
            .into_iter()
            .filter(|id| {
                let Some(c) = g.card_defs.get(id) else { return false };
                let Some(def) = cards::def(&c.def) else { return false };
                (def.category == Category::Trick || def.slot.is_some()) && colors.contains(&c.suit.is_red())
            })
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
        if step == "ask" {
            return match take_answer(g, "sidi") {
                None => Ok(trigger_ask(actor, "sidi", "司敌：弃一张同色非基本牌/装备，封锁其该颜色本阶段", json!({ "step": "ask", "target": target }))),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(Stage::PickOwn, json!({ "step": "pick", "target": target }))),
                Some(o) => Err(format!("司敌：未知选项 {o}")),
            };
        }
        return match take_answer(g, "sidi") {
            None => Ok(must_pick(g, actor, "sidi", "pick_own", "弃置", &cands, json!({ "step": "pick", "target": target }))),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !cands.contains(&id) {
                    return Err("司敌：须弃同色非基本牌/装备".into());
                }
                let red = g.card_defs.get(&id).map(|c| c.suit.is_red()).unwrap_or(false);
                flow::discard(g, actor, &id, "司敌");
                let frozen: Vec<String> = g.hand(target).into_iter().filter(|c| g.card_defs.get(c).map(|x| x.suit.is_red() == red).unwrap_or(false)).collect();
                g.freeze_cards(target, &frozen, super::super::FreezeUntil::Phase, "司敌");
                g.set_counter(&format!("sidi.{actor}.{target}"), 1);
                Ok(Step::Finish("司敌：封锁其该颜色手牌".into()))
            }
            Some(o) => Err(format!("司敌：未知选项 {o}")),
        };
    }
    // 出牌阶段结束：若其本阶段未使用【杀】，你视为对其使用一张【杀】。
    if !started {
        return Ok(Step::Done);
    }
    let used_sha = g.log.iter().any(|e| e.kind == EventKind::CardUsed && e.by == Some(target) && e.turn == g.turn && e.card.as_deref().map(|c| g.card_defs.get(c).and_then(|x| cards::def(&x.def)).map(|d| d.tags.sha).unwrap_or(false)).unwrap_or(false));
    g.set_counter(&format!("sidi.{actor}.{target}"), 0);
    if used_sha {
        return Ok(Step::Done);
    }
    super::super::card_play::use_virtual(g, actor, "sha", &[target], false)?;
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 慎断（韩浩史涣）：黑色基本牌被弃置后，可当无距离【兵粮寸断】使用
// ---------------------------------------------------------------------------

fn shenduan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(card) = card_of(ask, "card") else { return Ok(Step::Done) };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else { return Ok(Step::Done) };
    let black = g.card_defs.get(&card).map(|c| !c.suit.is_red()).unwrap_or(false);
    if def.category != Category::Basic || !black {
        return Ok(Step::Done);
    }
    if !matches!(g.cards.get(&card), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "shenduan") {
            None => Ok(trigger_ask(actor, "shenduan", "慎断：将此牌当无距离【兵粮寸断】使用", json!({ "step": "ask", "card": card }))),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::Target, json!({ "step": "target", "card": card }))),
            Some(o) => Err(format!("慎断：未知选项 {o}")),
        };
    }
    // 兵粮寸断无距离限制：可直接指定任意其他角色（判定区为空者）。
    let cands: Vec<usize> = alive_others(g, actor)
        .into_iter()
        .filter(|&t| g.judge_stacks.get(&t).map(|s| s.is_empty()).unwrap_or(true))
        .filter(|&t| !g.seats.get(t).map(|s| s.judge_abolished).unwrap_or(false))
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "shenduan") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("shenduan");
            for t in &cands {
                b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
            }
            Ok(Step::Need(b.escape_mandatory("（必选目标）").data(json!({ "step": "target", "card": card })).build()))
        }
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "慎断：非法目标")?;
            if !cands.contains(&t) {
                return Err("慎断：目标非法".into());
            }
            super::super::card_play::use_from_pile_as(g, actor, &card, "bingliang_cunduan", &[t])?;
            Ok(Step::Done)
        }
        Some(o) => Err(format!("慎断：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 勇略（韩浩史涣）：其他角色判定阶段开始，若其在你的攻击范围内 → 弃其判定区一张牌，视为对其使用【杀】
// ---------------------------------------------------------------------------

fn yonglue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else { return Ok(Step::Done) };
    let reach = gear::attack_range(g, actor) as usize;
    if g.turn == actor || gear::distance(g, actor, target) > reach {
        return Ok(Step::Done);
    }
    let judge: Vec<String> = g.judge_stacks.get(&target).cloned().unwrap_or_default();
    if judge.is_empty() {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "yonglue") {
            None => Ok(trigger_ask(actor, "yonglue", "勇略：弃其判定区一张牌，视为对其使用【杀】", json!({ "step": "ask", "target": target }))),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                let id = judge.last().cloned().unwrap_or_default();
                super::super::card_play::remove_card(g, target, &id, "勇略");
                super::super::card_play::use_virtual(g, actor, "sha", &[target], false)?;
                // 若此【杀】未产生伤害 → 摸一张（在本帧结束后由日志判定，简化：立即检查是否已有伤害事件）。
                if !g.log.iter().any(|e| e.kind == EventKind::DamageAfter && e.by == Some(actor)) {
                    flow::draw(g, actor, 1);
                }
                Ok(Step::Finish("勇略：弃判定牌并视为使用【杀】".into()))
            }
            Some(o) => Err(format!("勇略：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 品第（陈群）：出牌阶段每名角色限一次，弃一张本阶段未用过的类型 → 其摸/弃 X
// ---------------------------------------------------------------------------

fn can_pindi(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play && !own_cards(g, seat).is_empty() && !alive_others(g, seat).is_empty()
}

fn pindi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    if step == "pick" {
        if !can_pindi(g, actor) {
            return Ok(Step::Done);
        }
        let used: Vec<String> = ask.data.get("types").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        let cands: Vec<String> = own_cards(g, actor)
            .into_iter()
            .filter(|id| {
                g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| !used.contains(&format!("{:?}", d.category))).unwrap_or(false)
            })
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "pindi") {
            None => Ok(must_pick(g, actor, "pindi", "pick_own", "弃置", &cands, json!({ "step": "pick", "types": used }))),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !cands.contains(&id) {
                    return Err("品第：须弃本阶段未以此法弃置过的类型".into());
                }
                let cat = g.card_defs.get(&id).and_then(|c| cards::def(&c.def)).map(|d| format!("{:?}", d.category)).unwrap_or_default();
                flow::discard(g, actor, &id, "品第");
                let mut types = used.clone();
                types.push(cat);
                Ok(Step::Next(Stage::Target, json!({ "step": "target", "types": types })))
            }
            Some(o) => Err(format!("品第：未知选项 {o}")),
        };
    }
    if step == "target" {
        let types = ask.data.get("types").cloned().unwrap_or(json!([]));
        let others: Vec<usize> = alive_others(g, actor)
            .into_iter()
            .filter(|&t| !g.hand(t).is_empty())
            .collect();
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "pindi") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("pindi");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(b.escape_mandatory("（必选目标）").data(json!({ "step": "target", "types": types })).build()))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "品第：非法目标")?;
                if !others.contains(&t) {
                    return Err("品第：目标须为其他有手牌角色".into());
                }
                let n = g.counter(&phase_key(actor, "pindi")) + 1;
                // 已受伤 → 你进入连环状态。
                if g.seats[t].hp < g.seats[t].max_hp {
                    let _ = flow::chain_enter(g, actor, "品第");
                }
                Ok(Step::Next(Stage::Choose, json!({ "step": "mode", "target": t, "n": n })))
            }
            Some(o) => Err(format!("品第：未知选项 {o}")),
        };
    }
    if step == "mode" {
        let target = seat_of(ask, "target").unwrap_or(0);
        let n = ask.data.get("n").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        return match take_answer(g, "pindi") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("pindi");
                b = b.option("use:draw", format!("令其摸 {n} 张"));
                if g.hand_size(target) >= n {
                    b = b.option("use:discard", format!("令其弃 {n} 张"));
                }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "mode", "target": target, "n": n })).build()))
            }
            Some(a) if a == "use:draw" => {
                g.bump(&phase_key(actor, "pindi"));
                flow::draw(g, target, n);
                Ok(Step::Finish("品第：令其摸牌".into()))
            }
            Some(a) if a == "use:discard" => {
                g.bump(&phase_key(actor, "pindi"));
                let h = g.hand(target);
                for id in h.into_iter().take(n) {
                    flow::discard(g, target, &id, "品第");
                }
                Ok(Step::Finish("品第：令其弃牌".into()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(o) => Err(format!("品第：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 法恩（陈群）：一名角色翻至正面/进入连环后，可令其摸一张
// ---------------------------------------------------------------------------

fn faen(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else { return Ok(Step::Done) };
    if g.turn == target && ask.data.get("face").is_none() {
        // 自身回合内的翻面事件仍允许；这里不额外过滤。
    }
    if !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let data = json!({ "step": "ask", "target": target });
    match take_answer(g, "faen") {
        None => Ok(trigger_ask(actor, "faen", "法恩：令其摸一张", data)),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, target, 1);
            Ok(Step::Finish("法恩：令其摸一张".into()))
        }
        Some(o) => Err(format!("法恩：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 奔袭（吴懿）：距离 -1（Hook）；使用单目标【杀】/普通锦囊且与其他角色距离均为 1 → 至多两项
// ---------------------------------------------------------------------------

fn benxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.turn != actor || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else { return Ok(Step::Done) };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else { return Ok(Step::Done) };
    if !(def.tags.sha || def.category == Category::Trick) {
        return Ok(Step::Done);
    }
    // 「距离与所有其他角色均为 1」
    let all_one = alive_others(g, actor).into_iter().all(|t| gear::distance(g, actor, t) == 1);
    if !all_one {
        return Ok(Step::Done);
    }
    let picked: Vec<String> = ask.data.get("picked").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    let opts = ["extra", "armor", "no_offset", "draw"];
    if let Some(a) = take_answer(g, "benxi") {
        if a == "ok" {
            for p in &picked {
                match p.as_str() {
                    "extra" => super::super::card_play::declare_extra_targets(g, &card, 1),
                    "armor" => g.use_opts_mut(&card).ignore_armor = true,
                    "no_offset" => {
                        super::super::card_play::declare_no_wuxie(g, &card);
                        super::super::card_play::declare_no_response(g, &card);
                    }
                    "draw" => g.use_opts_mut(&card).draw_on_damage = true,
                    _ => {}
                }
            }
            return Ok(Step::Done);
        }
        if a.starts_with("opt:") {
            let id = a[4..].to_string();
            let mut next = picked.clone();
            next.push(id);
            if next.len() >= 2 {
                return apply_benxi(g, &card, &next);
            }
            return Ok(Step::Next(Stage::MultiSelect, json!({ "step": "pick", "card": card, "picked": next })));
        }
        if a == "pass" {
            return Ok(Step::Done);
        }
    }
    let mut b = PendingBuilder::new(actor, "multi_select").skill("benxi");
    for o in opts {
        if picked.iter().any(|p| p == o) {
            continue;
        }
        let label = match o {
            "extra" => "目标数 +1",
            "armor" => "无视防具",
            "no_offset" => "不能被抵消",
            _ => "造成伤害后你摸一张",
        };
        b = b.option(format!("opt:{o}"), label);
    }
    if !picked.is_empty() {
        b = b.option("ok", "确定");
    }
    Ok(Step::Need(b.escape_with("cancel", "取消").data(json!({ "step": "pick", "card": card, "picked": picked })).build()))
}

fn apply_benxi(g: &mut Game, card: &str, picked: &[String]) -> Result<Step, String> {
    for p in picked {
        match p.as_str() {
            "extra" => super::super::card_play::declare_extra_targets(g, card, 1),
            "armor" => g.use_opts_mut(card).ignore_armor = true,
            "no_offset" => {
                super::super::card_play::declare_no_wuxie(g, card);
                super::super::card_play::declare_no_response(g, card);
            }
            "draw" => g.use_opts_mut(card).draw_on_damage = true,
            _ => {}
        }
    }
    Ok(Step::Finish("奔袭：已选择增益".into()))
}

// ---------------------------------------------------------------------------
// 忠勇（周仓）：使用【杀】后，可将此【杀】交给一名其他角色；若为红色，其可使用一张【杀】
// ---------------------------------------------------------------------------

fn zhongyong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(card) = card_of(ask, "card") else { return Ok(Step::Done) };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else { return Ok(Step::Done) };
    if !def.tags.sha || g.turn != actor {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        let others = alive_others(g, actor);
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "zhongyong") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("zhongyong");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("交给 #{}", t + 1));
                }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target", "card": card })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "忠勇：非法目标")?;
                if !others.contains(&t) {
                    return Err("忠勇：目标须为其他存活角色".into());
                }
                if g.cards.get(&card).is_some() {
                    flow::gain(g, t, &card, actor, "忠勇");
                }
                let red = g.card_defs.get(&card).map(|c| c.suit.is_red()).unwrap_or(false);
                if red {
                    // 其可以对你攻击范围内的角色使用一张【杀】（简化：可选目标）。
                    let reach = gear::attack_range(g, actor) as usize;
                    if let Some(target) = alive_others(g, actor).into_iter().find(|&x| gear::distance(g, actor, x) <= reach) {
                        super::super::card_play::use_virtual(g, t, "sha", &[target], false)?;
                    }
                }
                Ok(Step::Finish("忠勇：交给其【杀】".into()))
            }
            Some(o) => Err(format!("忠勇：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}
