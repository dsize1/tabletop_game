//! B5 技能（一将成名·Y4：6 将 / 13 技能）。

use serde_json::{json, Value};

use super::super::cards::{self, Category};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::judge;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{DamageKind, DamageSpec, Game, Phase, SourceKind, Zone};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use super::common;
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "zanhui", name: "谮毁", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "jiaojin", name: "骄矜", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DamageBefore], can_active: false }] },
    SkillDef { code: "fenli", name: "奋励", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "pingkou", name: "平寇", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseEnd, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "shenxing", name: "慎行", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "bingyi", name: "秉壹", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardDiscarded, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "huituo", name: "恢拓", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "mingjian", name: "明鉴", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "xingshuai", name: "兴衰", kind: SkillKind::Lord, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DyingEnter], can_active: false }] },
    SkillDef { code: "qianju", name: "千驹", kind: SkillKind::Locked, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "qingxi", name: "倾袭", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::DamageBefore], can_active: false }] },
    SkillDef { code: "huomo", name: "活墨", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zuoding", name: "佐定", kind: SkillKind::Normal, grant_only: false, transform: false, faces: &[Face { window: Window::OtherPhase(Phase::Play), subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::Other }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zanhui" => zanhui,
        "jiaojin" => jiaojin,
        "fenli" => fenli,
        "pingkou" => pingkou,
        "shenxing" => shenxing,
        "bingyi" => bingyi,
        "huituo" => huituo,
        "mingjian" => mingjian,
        "xingshuai" => xingshuai,
        "qingxi" => qingxi,
        "huomo" => huomo,
        "zuoding" => zuoding,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "shenxing" => can_shenxing,
        "mingjian" => can_mingjian,
        "huomo" => can_huomo,
        _ => return None,
    })
}

/// 千驹：每损失 1 点体力，距离 -1。
pub fn distance_bonus(g: &Game, seat: usize) -> i32 {
    if !skills::enabled("qianju") || !skills::active(g, seat, "qianju") {
        return 0;
    }
    let s = &g.seats[seat];
    -((s.max_hp - s.hp).max(0))
}

/// 明鉴：目标下回合手牌上限 +1 / 【杀】次数 +1。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    if g.counter(&format!("mingjian.{seat}")) > 0 {
        1
    } else {
        0
    }
}
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    if g.counter(&format!("mingjian.{seat}")) > 0 {
        1
    } else {
        0
    }
}

/// 骄矜：男性角色对你造成伤害时可弃装备牌减 1（受害者侧询问）。
pub fn should_ask_damage_before_victim(g: &Game, from: usize, target: usize) -> bool {
    if !skills::enabled("jiaojin") || !skills::active(g, target, "jiaojin") {
        return false;
    }
    if !g.seats.get(from).map(|s| s.male).unwrap_or(false) {
        return false;
    }
    own_equip_cards(g, target).is_empty() == false
}

/// 兴衰：濒死时由其他魏势力角色逐个支援（主公技 + 限定技）。
pub fn should_fire_dying_enter(g: &Game, seat: usize) -> bool {
    if !skills::enabled("xingshuai") || !skills::active(g, seat, "xingshuai") {
        return false;
    }
    if g.seats.get(seat).and_then(|s| s.identity) != Some(super::super::Identity::Lord) {
        return false;
    }
    !skills::limited_used(g, seat, "xingshuai")
}

fn turn_key(seat: usize, key: &str) -> String { format!("turn.{seat}.{key}") }
fn phase_key(seat: usize, key: &str) -> String { format!("phase.{seat}.{key}") }
fn seat_of(ask: &Ask, key: &str) -> Option<usize> { ask.data.get(key).and_then(|v| v.as_u64()).map(|v| v as usize) }
fn card_of(ask: &Ask, key: &str) -> Option<String> { ask.data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()) }
fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut v: Vec<String> = g.cards.iter().filter(|(_, z)| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == seat)).map(|(id, _)| id.clone()).collect();
    v.sort(); v
}
fn own_equip_cards(g: &Game, seat: usize) -> Vec<String> {
    own_cards(g, seat).into_iter().filter(|id| g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| d.slot.is_some()).unwrap_or(false)).collect()
}
fn card_label(g: &Game, id: &str) -> String {
    g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| d.name.to_string()).unwrap_or_else(|| id.to_string())
}
fn trigger_ask(actor: usize, skill: &'static str, label: &str, data: Value) -> Step {
    Step::Need(PendingBuilder::new(actor, "choose").skill(skill).option("use", label).escape("不发动").data(data).build())
}
fn must_pick(g: &Game, actor: usize, skill: &'static str, kind: &'static str, verb: &str, cands: &[String], data: Value) -> Step {
    let mut b = PendingBuilder::new(actor, kind).skill(skill);
    for id in cands { b = b.option(format!("card:{id}"), format!("{verb}【{}】", card_label(g, id))); }
    Step::Need(b.escape_mandatory("（必选一张）").data(data).build())
}
fn alive_others(g: &Game, seat: usize) -> Vec<usize> { (0..g.seats.len()).filter(|&i| i != seat && g.seats[i].alive).collect() }

// 谮毁：使用【杀】/黑色普通锦囊指定唯一目标时，令另一名可成为目标的角色二选一
fn zanhui(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(card) = card_of(ask, "card") else { return Ok(Step::Done) };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else { return Ok(Step::Done) };
    let black = g.card_defs.get(&card).map(|c| !c.suit.is_red()).unwrap_or(false);
    if !(def.tags.sha || (def.category == Category::Trick && black)) {
        return Ok(Step::Done);
    }
    let targets: Vec<usize> = g.use_targets.get(&card).cloned().unwrap_or_default();
    if targets.len() != 1 {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = alive_others(g, actor).into_iter().filter(|t| !targets.contains(t)).collect();
    if cands.is_empty() || g.counter(&phase_key(actor, "zanhui")) > 0 {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        return match take_answer(g, "zanhui") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("zanhui");
                for t in &cands { b = b.option(format!("seat:{t}"), format!("令 #{} 选择", t + 1)); }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target", "card": card })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "谮毁：非法目标")?;
                if !cands.contains(&t) { return Err("谮毁：目标非法".into()); }
                Ok(Step::Next(Stage::Choose, json!({ "step": "helper", "card": card, "helper": t })))
            }
            Some(o) => Err(format!("谮毁：未知选项 {o}")),
        };
    }
    let helper = seat_of(ask, "helper").unwrap_or(0);
    if step == "give" {
        let c = own_cards(g, helper);
        if c.is_empty() {
            return Ok(Step::Finish("谮毁：其无牌可交".into()));
        }
        return match take_answer(g, "zanhui") {
            None => Ok(must_pick(g, helper, "zanhui", "pick_own", "交给", &c, json!({ "step": "give", "card": card, "helper": helper }))),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !c.contains(&id) {
                    return Err("谮毁：只能交出自己的牌".into());
                }
                flow::gain(g, actor, &id, helper, "谮毁");
                Ok(Step::Finish("谮毁：获得其一张牌".into()))
            }
            Some(o) => Err(format!("谮毁：未知选项 {o}")),
        };
    }
    match take_answer(g, "zanhui") {
        None => {
            let mut b = PendingBuilder::new(helper, "choice").skill("zanhui");
            b = b.option("give", "交给你一张牌，并代替你成为此牌使用者");
            b = b.option("extra", "成为此牌的额外目标");
            Ok(Step::Need(b.escape_mandatory("（必须选择一项）").data(json!({ "step": "helper", "card": card, "helper": helper })).build()))
        }
        Some(a) if a == "extra" => {
            g.bump(&phase_key(actor, "zanhui"));
            super::super::card_play::declare_extra_targets(g, &card, 1);
            Ok(Step::Finish("谮毁：其成为额外目标".into()))
        }
        Some(a) if a == "give" => {
            g.bump(&phase_key(actor, "zanhui"));
            Ok(Step::Next(Stage::PickOwn, json!({ "step": "give", "card": card, "helper": helper })))
        }
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            flow::gain(g, actor, &id, helper, "谮毁");
            Ok(Step::Finish("谮毁：获得其一张牌".into()))
        }
        Some(o) => Err(format!("谮毁：未知选项 {o}")),
    }
}

// 骄矜：男性角色造成伤害时可弃装备牌令伤害 -1
fn jiaojin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor; // 受害者
    let from = ask.data.get("from").and_then(|v| v.as_u64()).map(|v| v as usize);
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let equips = own_equip_cards(g, actor);
    let apply = |g: &mut Game, n: i32| {
        if n > 0 {
            let spec = DamageSpec { from, target: actor, amount: n, kind: DamageKind::Normal, source: SourceKind::Other };
            let armor = gear::armor_of(g, actor);
            let _ = flow::damage_apply(g, spec, armor, 0);
        }
    };
    if equips.is_empty() {
        apply(g, amount);
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "jiaojin") {
            None => Ok(trigger_ask(actor, "jiaojin", "骄矜：弃一张装备牌，令此伤害 -1", json!({ "step": "ask", "amount": amount, "from": from }))),
            Some(a) if a == "pass" => { apply(g, amount); Ok(Step::Finish("骄矜：未发动".into())) }
            Some(a) if a == "use" => Ok(Step::Next(Stage::PickOwn, json!({ "step": "pick", "amount": amount, "from": from }))),
            Some(o) => Err(format!("骄矜：未知选项 {o}")),
        };
    }
    match take_answer(g, "jiaojin") {
        None => Ok(must_pick(g, actor, "jiaojin", "pick_own", "弃置", &equips, json!({ "step": "pick", "amount": amount, "from": from }))),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !equips.contains(&id) { return Err("骄矜：只能弃置装备牌".into()); }
            flow::discard(g, actor, &id, "骄矜");
            apply(g, amount - 1);
            Ok(Step::Finish("骄矜：伤害 -1".into()))
        }
        Some(o) => Err(format!("骄矜：未知选项 {o}")),
    }
}

// 奋励：三个时机各自判定，可跳过对应阶段
fn fenli(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let phase = g.phase;
    let others: Vec<usize> = alive_others(g, actor);
    let cond = match phase {
        Phase::Draw => others.iter().all(|&i| g.hand_size(actor) > g.hand_size(i)),
        Phase::Play => others.iter().all(|&i| g.seats[actor].hp > g.seats[i].hp),
        Phase::Discard => {
            let eq = gear::equips_of(g, actor).len();
            eq > 0 && others.iter().all(|&i| eq > gear::equips_of(g, i).len())
        }
        _ => return Ok(Step::Done),
    };
    if !cond {
        return Ok(Step::Done);
    }
    let key = phase_key(actor, "fenli");
    if g.counter(&key) > 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "fenli") {
        None => Ok(trigger_ask(actor, "fenli", &format!("奋励：跳过{phase:?}阶段"), json!({ "step": "ask" }))),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            g.bump(&key);
            g.bump(&turn_key(actor, "pingkou"));
            super::super::turn::skip_phase(g, phase);
            Ok(Step::Finish("奋励：跳过该阶段".into()))
        }
        Some(o) => Err(format!("奋励：未知选项 {o}")),
    }
}

// 平寇：回合结束后，每跳过一个阶段可对一名其他角色造成 1 点伤害
fn pingkou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let mut left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
    if left == 0 {
        left = g.counter(&turn_key(actor, "pingkou"));
    }
    if left <= 0 {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        let others = alive_others(g, actor);
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "pingkou") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("pingkou");
                for t in &others { b = b.option(format!("seat:{t}"), format!("对 #{} 造成 1 点伤害", t + 1)); }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target", "left": left })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Finish("平寇：放弃本次".into())),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "平寇：非法目标")?;
                if !others.contains(&t) { return Err("平寇：目标非法".into()); }
                let spec = DamageSpec { from: Some(actor), target: t, amount: 1, kind: DamageKind::Normal, source: SourceKind::Skill };
                let armor = gear::armor_of(g, t);
                flow::damage(g, spec, armor, 0);
                let left = left - 1;
                if left > 0 {
                    return Ok(Step::Next(Stage::Target, json!({ "step": "target", "left": left })));
                }
                Ok(Step::Finish("平寇：结算完成".into()))
            }
            Some(o) => Err(format!("平寇：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

// 慎行：出牌阶段，弃两张牌 → 摸一张（不限次）
fn can_shenxing(g: &Game, seat: usize) -> bool { g.phase == Phase::Play && own_cards(g, seat).len() >= 2 }
fn shenxing(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let picked: Vec<String> = ask.data.get("picked").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    if !can_shenxing(g, actor) { return Ok(Step::Done); }
    let own = own_cards(g, actor);
    match take_answer(g, "shenxing") {
        None => {
            let mut b = PendingBuilder::new(actor, "multi_select").skill("shenxing");
            for id in &own { if picked.contains(id) { continue; } b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id))); }
            if picked.len() >= 2 { b = b.option("ok", "确定"); }
            Ok(Step::Need(b.escape_with("cancel", "取消").data(json!({ "step": "pick", "picked": picked })).build()))
        }
        Some(a) if a == "cancel" => Ok(Step::Done),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) { return Err("慎行：只能弃自己的牌".into()); }
            let mut next = picked.clone(); next.push(id);
            if next.len() >= 2 {
                for c in &next { flow::discard(g, actor, c, "慎行"); }
                flow::draw(g, actor, 1);
                return Ok(Step::Finish("慎行：弃两张摸一张".into()));
            }
            Ok(Step::Next(Stage::MultiSelect, json!({ "step": "pick", "picked": next })))
        }
        Some(a) if a == "ok" => Ok(Step::Finish("慎行：取消".into())),
        Some(o) => Err(format!("慎行：未知选项 {o}")),
    }
}

// 秉壹：每阶段限一次，你的牌因弃置进弃牌堆后，可展示手牌；同色则至多 X 名其他角色各摸一张
fn bingyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.counter(&phase_key(actor, "bingyi")) > 0 { return Ok(Step::Done); }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "bingyi") {
            None => Ok(trigger_ask(actor, "bingyi", "秉壹：展示手牌（同色则令至多 X 名角色各摸一张）", json!({ "step": "ask" }))),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::Target, json!({ "step": "target" }))),
            Some(o) => Err(format!("秉壹：未知选项 {o}")),
        };
    }
    // 展示与同色判定只做一次（进入 target 阶段前）。
    if ask.data.get("revealed").is_none() {
        let hand = g.hand(actor);
        if hand.is_empty() { return Ok(Step::Done); }
        g.reveal(&hand, "秉壹");
        let reds = hand.iter().filter(|id| g.card_defs.get(*id).map(|c| c.suit.is_red()).unwrap_or(false)).count();
        let same = reds == 0 || reds == hand.len();
        g.conceal(&hand, "秉壹·结算完成");
        if !same {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Target, json!({ "step": "target", "revealed": true, "x": hand.len() })));
    }
    let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let picked: Vec<usize> = ask.data.get("picked").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect()).unwrap_or_default();
    let others = alive_others(g, actor);
    match take_answer(g, "bingyi") {
        None => {
            let mut b = PendingBuilder::new(actor, "multi_select").skill("bingyi");
            for t in &others { if picked.contains(t) || picked.len() >= x { continue; } b = b.option(format!("seat:{t}"), format!("令 #{} 摸一张", t + 1)); }
            if !picked.is_empty() || others.is_empty() { b = b.option("ok", "确定"); }
            Ok(Step::Need(b.escape_with("cancel", "放弃").data(json!({ "step": "target", "revealed": true, "x": x, "picked": picked })).build()))
        }
        Some(a) if a == "cancel" => Ok(Step::Done),
        Some(a) if a == "ok" => {
            g.bump(&phase_key(actor, "bingyi"));
            for t in picked { flow::draw(g, t, 1); }
            Ok(Step::Finish("秉壹：令其各摸一张".into()))
        }
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "秉壹：非法目标")?;
            if !others.contains(&t) { return Err("秉壹：目标非法".into()); }
            let mut next = picked.clone(); next.push(t);
            if next.len() >= x {
                g.bump(&phase_key(actor, "bingyi"));
                for t in next { flow::draw(g, t, 1); }
                return Ok(Step::Finish("秉壹：令其各摸一张".into()));
            }
            Ok(Step::Next(Stage::Target, json!({ "step": "target", "revealed": true, "x": x, "picked": next })))
        }
        Some(o) => Err(format!("秉壹：未知选项 {o}")),
    }
}

// 恢拓：受伤后 → 一名角色判定；红 → 回复 1；黑 → 摸伤害值张
fn huituo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as usize;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "resolve" {
        // 判定已生效（quick_judge 窗口结束）：红 → 回复；黑 → 摸伤害值张。
        let t = common::seat_of(ask, "target").unwrap_or(actor);
        match g.judge_result.take() {
            Some(r) if r.is_red() => {
                flow::heal(g, t, 1, "恢拓");
            }
            Some(_) => {
                flow::draw(g, t, amount);
            }
            None => {}
        }
        return Ok(Step::Finish("恢拓：判定结算".into()));
    }
    if step == "target" {
        let alive: Vec<usize> = (0..g.seats.len()).filter(|&t| g.seats[t].alive).collect();
        return match take_answer(g, "huituo") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("huituo");
                for t in &alive { b = b.option(format!("seat:{t}"), format!("令 #{} 判定", t + 1)); }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target", "amount": amount })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "恢拓：非法目标")?;
                if !alive.contains(&t) { return Err("恢拓：目标非法".into()); }
                // 判定走 `quick_judge`（含鬼才改判 / 天妒窗口）；结果在下一次进入 "resolve" 时读。
                super::super::quick_judge::start(g, t, "恢拓", None);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "resolve", "target": t, "amount": amount }),
                ))
            }
            Some(o) => Err(format!("恢拓：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

// 明鉴：出牌阶段限一次，将所有手牌交给一名其他角色；其下回合手牌上限+1、杀次数+1
fn can_mingjian(g: &Game, seat: usize) -> bool { g.phase == Phase::Play && !g.hand(seat).is_empty() && !alive_others(g, seat).is_empty() }
fn mingjian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_mingjian(g, actor) { return Ok(Step::Done); }
    let others = alive_others(g, actor);
    match take_answer(g, "mingjian") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("mingjian");
            for t in &others { b = b.option(format!("seat:{t}"), format!("交给 #{}", t + 1)); }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "明鉴：非法目标")?;
            if !others.contains(&t) { return Err("明鉴：目标非法".into()); }
            for id in g.hand(actor) { flow::gain(g, t, &id, actor, "明鉴"); }
            g.set_counter(&format!("mingjian.{t}"), 1);
            Ok(Step::Finish("明鉴：所有手牌交给其".into()))
        }
        Some(o) => Err(format!("明鉴：未知选项 {o}")),
    }
}

// 兴衰：主公技+限定技：濒死时其他魏势力逐个支援（各回复 1），之后支援者各受 1 点伤害
fn xingshuai(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    // 主公技：仅主公身份可发动。
    if g.seats.get(actor).and_then(|s| s.identity) != Some(super::super::Identity::Lord) {
        return Ok(Step::Done);
    }
    if g.counter(&format!("xingshuai.{actor}.done")) > 0 { return Ok(Step::Done); }
    let supporters: Vec<usize> = (0..g.seats.len())
        .filter(|&t| t != actor && g.seats[t].alive && g.seats[t].kingdom == "wei")
        .collect();
    if supporters.is_empty() {
        return Ok(Step::Done);
    }
    let cursor = ask.data.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let helped: i32 = ask.data.get("helped").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    if cursor >= supporters.len() || !g.seats[actor].alive || g.seats[actor].hp > 0 {
        g.bump(&format!("xingshuai.{actor}.done"));
        skills::mark_limited_used(g, actor, "xingshuai");
        for s in supporters {
            if helped > 0 && g.seats[s].alive {
                g.defer_after_dying(super::super::AfterDying { victim: actor, seat: s, amount: 1, cause: "兴衰".into() });
            }
        }
        return Ok(Step::Finish("兴衰：支援结算完成".into()));
    }
    let cur = supporters[cursor];
    match take_answer(g, "xingshuai") {
        None => {
            let mut b = PendingBuilder::new(cur, "choose").skill("xingshuai");
            b = b.option("use", "令主公回复 1 点体力（此后你受到 1 点伤害）");
            Ok(Step::Need(b.escape("不支援").data(json!({ "step": "ask", "cursor": cursor, "helped": helped })).build()))
        }
        Some(a) if a == "use" => {
            flow::heal(g, actor, 1, "兴衰");
            Ok(Step::Next(Stage::Choose, json!({ "step": "ask", "cursor": cursor + 1, "helped": helped + 1 })))
        }
        Some(_) => Ok(Step::Next(Stage::Choose, json!({ "step": "ask", "cursor": cursor + 1, "helped": helped }))),
    }
}

// 倾袭：使用【杀】造成伤害时，若有武器，令目标二选一（弃等量手牌并弃武器 / 伤害+1）
fn qingxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else { return Ok(Step::Done) };
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let weapon = gear::equip_in(g, actor, super::super::Slot::Weapon).cloned();
    let Some(weapon) = weapon else { return Ok(Step::Done) };
    let range = g.card_defs.get(&weapon).and_then(|c| cards::def(&c.def)).and_then(|d| d.weapon_range).unwrap_or(1) as usize;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "qingxi") {
            None => Ok(trigger_ask(actor, "qingxi", "倾袭：令目标二选一（弃牌并弃武器 / 伤害+1）", json!({ "step": "ask", "target": target, "amount": amount, "range": range, "weapon": weapon }))),
            Some(a) if a == "pass" => {
                let spec = DamageSpec { from: Some(actor), target, amount, kind: DamageKind::Normal, source: SourceKind::Slash };
                let armor = gear::armor_of(g, target);
                let _ = flow::damage_apply(g, spec, armor, 0);
                Ok(Step::Finish("倾袭：未发动".into()))
            }
            Some(a) if a == "use" => Ok(Step::Next(Stage::Choose, json!({ "step": "choice", "target": target, "amount": amount, "range": range, "weapon": weapon }))),
            Some(o) => Err(format!("倾袭：未知选项 {o}")),
        };
    }
    match take_answer(g, "qingxi") {
        None => {
            let mut b = PendingBuilder::new(target, "choice").skill("qingxi");
            if g.hand_size(target) >= range { b = b.option("discard", format!("弃置 {range} 张手牌，然后其弃置武器")); }
            b = b.option("plus", "令此【杀】伤害 +1");
            Ok(Step::Need(b.escape_mandatory("（必须选择一项）").data(json!({ "step": "choice", "target": target, "amount": amount, "range": range, "weapon": weapon })).build()))
        }
        Some(a) if a == "plus" => {
            let spec = DamageSpec { from: Some(actor), target, amount: amount + 1, kind: DamageKind::Normal, source: SourceKind::Slash };
            let armor = gear::armor_of(g, target);
            let _ = flow::damage_apply(g, spec, armor, 0);
            Ok(Step::Finish("倾袭：伤害 +1".into()))
        }
        Some(a) if a == "discard" => {
            let h = g.hand(target);
            for id in h.into_iter().take(range) { flow::discard(g, target, &id, "倾袭"); }
            if let Some(w) = gear::equip_in(g, actor, super::super::Slot::Weapon).cloned() {
                flow::discard(g, actor, &w, "倾袭");
            }
            let spec = DamageSpec { from: Some(actor), target, amount, kind: DamageKind::Normal, source: SourceKind::Slash };
            let armor = gear::armor_of(g, target);
            let _ = flow::damage_apply(g, spec, armor, 0);
            Ok(Step::Finish("倾袭：弃牌并弃武器".into()))
        }
        Some(o) => Err(format!("倾袭：未知选项 {o}")),
    }
}

// 活墨：将一张黑色非基本牌置于牌堆顶，视为使用一张本回合未用过的基本牌
fn can_huomo(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.cards.iter().any(|(id, z)| matches!(z, Zone::Hand(s) if *s == seat) && {
            let Some(c) = g.card_defs.get(id) else { return false };
            let Some(d) = cards::def(&c.def) else { return false };
            !c.suit.is_red() && d.category != Category::Basic
        })
}
fn huomo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    if step == "pick" {
        if !can_huomo(g, actor) { return Ok(Step::Done); }
        let cands: Vec<String> = g.hand(actor).into_iter().filter(|id| {
            let Some(c) = g.card_defs.get(id) else { return false };
            let Some(d) = cards::def(&c.def) else { return false };
            !c.suit.is_red() && d.category != Category::Basic
        }).collect();
        return match take_answer(g, "huomo") {
            None => Ok(must_pick(g, actor, "huomo", "pick_own", "置于牌堆顶", &cands, json!({ "step": "pick" }))),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !cands.contains(&id) { return Err("活墨：须为黑色非基本牌".into()); }
                flow::to_deck_top(g, &id);
                Ok(Step::Next(Stage::Choose, json!({ "step": "name" })))
            }
            Some(o) => Err(format!("活墨：未知选项 {o}")),
        };
    }
    // 选择本回合未使用过的基本牌牌名。
    let used: Vec<String> = ask.data.get("used").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    let names = ["sha", "tao", "jiu"];
    let avail: Vec<&str> = names.into_iter().filter(|n| !used.contains(&n.to_string())).collect();
    match take_answer(g, "huomo") {
        None => {
            let mut b = PendingBuilder::new(actor, "mode").skill("huomo");
            for n in &avail {
                let name = cards::def(n).map(|d| d.name).unwrap_or(n);
                b = b.option(format!("use:{n}"), format!("视为使用【{name}】"));
            }
            Ok(Step::Need(b.escape("取消").data(json!({ "step": "name", "used": used })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("use:") => {
            let def = a[4..].to_string();
            let mut used = used.clone(); used.push(def.clone());
            g.set_counter(&turn_key(actor, &format!("huomo.{def}")), 1);
            super::super::card_play::use_virtual(g, actor, &def, &[], false)?;
            let _ = used;
            Ok(Step::Done)
        }
        Some(o) => Err(format!("活墨：未知选项 {o}")),
    }
}

// 佐定：其他角色于其出牌阶段使用黑桃牌指定目标后，若本阶段无伤害，可令其中一个目标摸一张
fn zuoding(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(card) = card_of(ask, "card") else { return Ok(Step::Done) };
    let Some(defcard) = g.card_defs.get(&card) else { return Ok(Step::Done) };
    if defcard.suit != super::super::Suit::Spade || g.phase != Phase::Play || g.turn == actor {
        return Ok(Step::Done);
    }
    let no_damage = !g.log.iter().any(|e| e.kind == EventKind::DamageAfter && e.turn == g.turn && e.phase == Phase::Play);
    if !no_damage {
        return Ok(Step::Done);
    }
    let targets: Vec<usize> = g.use_targets.get(&card).cloned().unwrap_or_default().into_iter().filter(|&t| g.seats.get(t).map(|s| s.alive).unwrap_or(false)).collect();
    if targets.is_empty() || g.counter(&format!("turn.{}.zuoding", g.turn)) > 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "zuoding") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("zuoding");
            for t in &targets { b = b.option(format!("seat:{t}"), format!("令 #{} 摸一张", t + 1)); }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target", "card": card })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "佐定：非法目标")?;
            if !targets.contains(&t) { return Err("佐定：只能令此牌的目标摸牌".into()); }
            g.bump(&format!("turn.{}.zuoding", g.turn));
            flow::draw(g, t, 1);
            Ok(Step::Finish("佐定：令其摸一张".into()))
        }
        Some(o) => Err(format!("佐定：未知选项 {o}")),
    }
}
