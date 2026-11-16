//! B9 技能（界限突破·界标·蜀：7 将 / 14 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B9.json` 的 `agreed`（权威）。

use serde_json::json;

use super::super::cards::{self, Category, ResponseKind};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::judge;
use super::super::pending::PendingBuilder;
use super::super::relay;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, Game, Identity, Phase, Suit, Zone};
use super::CardAs;
use super::common::{self, card_label, def_of, own_cards, phase_key, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界刘备 ——
    SkillDef { code: "rende", name: "仁德", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "jijiang", name: "激将", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[
            Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true },
            Face { window: Window::AnyTime, subscriptions: &[
                Subscription { event: EventKind::CardUsed, subject: Subject::Other },
                Subscription { event: EventKind::CardPlayed, subject: Subject::Other },
            ], hooks: &[], can_active: false },
        ] },
    // —— 界关羽 ——
    SkillDef { code: "yijue", name: "义绝", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "wusheng", name: "武圣", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界张飞 ——
    SkillDef { code: "tishen", name: "替身", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "paoxiao", name: "咆哮", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaLimit], can_active: false }] },
    // —— 界赵云 ——
    SkillDef { code: "yajiao", name: "涯角", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherTurn, subscriptions: &[
            Subscription { event: EventKind::CardUsed, subject: Subject::By },
            Subscription { event: EventKind::CardPlayed, subject: Subject::By },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "longdan", name: "龙胆", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界马超 ——
    SkillDef { code: "mashu", name: "马术", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::Distance(-1)], can_active: false }] },
    SkillDef { code: "tieqi", name: "铁骑", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaTargeted], can_active: false }] },
    // —— 界诸葛亮 ——
    SkillDef { code: "kongcheng", name: "空城", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "guanxing", name: "观星", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[
            Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false },
            Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false },
        ] },
    // —— 界黄月英 ——
    SkillDef { code: "qicai", name: "奇才", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "rende" => rende,
        "jijiang" => jijiang,
        "yijue" => yijue,
        "tishen" => tishen,
        "yajiao" => yajiao,
        "tieqi" => tieqi,
        "guanxing" => guanxing,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "rende" => can_rende,
        "jijiang" => can_jijiang,
        "yijue" => can_yijue,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 牌面替代 / 数值钩子
// ---------------------------------------------------------------------------

/// 武圣（红色牌当【杀】）/ 龙胆（杀↔闪、酒↔桃）的出牌阶段用法。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    let d = def_of(g, id)?;
    let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
    // 龙胆：闪→杀、酒→桃、桃→酒。
    if skills::active(g, seat, "longdan") {
        if d.tags.shan {
            return Some("sha");
        }
        if d.tags.jiu {
            return Some("tao");
        }
        if d.tags.tao {
            return Some("jiu");
        }
    }
    // 武圣：红色牌当【杀】。
    if skills::active(g, seat, "wusheng") && red && !d.tags.sha {
        return Some("sha");
    }
    None
}

/// 响应语义（武圣红牌当杀 / 龙胆杀当闪、闪当杀、酒当桃、桃当酒）。
pub fn response_as(g: &Game, seat: usize, id: &str, kind: ResponseKind) -> CardAs {
    let Some(d) = def_of(g, id) else {
        return CardAs::NoOpinion;
    };
    let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
    if skills::active(g, seat, "longdan") {
        if kind == ResponseKind::Shan && d.tags.sha {
            return CardAs::As("shan");
        }
        if kind == ResponseKind::Sha && d.tags.shan {
            return CardAs::As("sha");
        }
        if kind == ResponseKind::Tao && d.tags.jiu {
            return CardAs::As("tao");
        }
        if kind == ResponseKind::Jiu && d.tags.tao {
            return CardAs::As("jiu");
        }
    }
    if skills::active(g, seat, "wusheng")
        && kind == ResponseKind::Sha
        && red
        && !d.tags.sha
    {
        return CardAs::As("sha");
    }
    CardAs::NoOpinion
}

/// 武圣：使用的方片【杀】无距离限制（读承载牌花色）。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !skills::active(g, actor, "wusheng") {
        return;
    }
    let Some(d) = def_of(g, card) else { return };
    if !d.tags.sha {
        return;
    }
    let diamond = g.card_defs.get(card).map(|c| c.suit == Suit::Diamond).unwrap_or(false);
    if diamond {
        super::super::card_play::declare_no_distance(g, card);
    }
}

/// 义绝（红桃【杀】+1）/ 咆哮（抵消后下一【杀】+1）。
pub fn sha_damage_bonus(g: &mut Game, actor: usize, target: usize, card: &str) -> i32 {
    let mut n = 0;
    if g.counter(&format!("yijue.bonus.{actor}.{target}")) > 0 {
        let heart = g.card_defs.get(card).map(|c| c.suit == Suit::Heart).unwrap_or(false);
        if heart {
            n += 1;
        }
    }
    if g.counter(&turn_key(actor, "paoxiao_bonus")) > 0 {
        n += 1;
        g.set_counter(&turn_key(actor, "paoxiao_bonus"), 0);
    }
    n
}

pub fn on_sha_dodged(g: &mut Game, actor: usize) {
    if skills::active(g, actor, "paoxiao") {
        g.set_counter(&turn_key(actor, "paoxiao_bonus"), 1);
    }
}

pub fn on_relay_sha(g: &mut Game, actor: usize, source: usize) {
    // 激将：代出【杀】的蜀势力角色于其回合外可令主公摸一张（每回合限一次）。
    if source == actor || g.turn == source || g.seats.get(source).map(|s| s.alive).unwrap_or(false) == false {
        return;
    }
    if g.seats.get(source).map(|s| s.kingdom.as_str()) != Some("shu") {
        return;
    }
    let Some(lord) = g
        .seats
        .iter()
        .position(|s| s.alive && s.identity == Some(Identity::Lord) && skills::active(g, s.id, "jijiang"))
    else {
        return;
    };
    if g.counter(&format!("jijiang.draw.{}", g.turn)) > 0 {
        return;
    }
    if skills::active(g, lord, "jijiang") {
        super::super::resolve::push_ask(
            g,
            "jijiang",
            Stage::Choose,
            source,
            json!({ "step": "draw", "lord": lord }),
        );
    }
}

pub fn trick_no_distance(g: &Game, actor: usize) -> bool {
    skills::active(g, actor, "qicai")
}

pub fn prevent_discard(g: &Game, by: usize, owner: usize, id: &str) -> bool {
    if by == owner || !skills::active(g, owner, "qicai") {
        return false;
    }
    let Some(d) = def_of(g, id) else { return false };
    matches!(d.slot, Some(super::super::Slot::Armor) | Some(super::super::Slot::Treasure))
}

pub fn deny_target_card(g: &Game, _user: usize, target: usize, code: &str) -> bool {
    if !skills::active(g, target, "kongcheng") {
        return false;
    }
    if !g.hand(target).is_empty() {
        return false;
    }
    let Some(d) = cards::def(code) else { return false };
    d.tags.sha || d.effect == cards::Effect::Duel
}

// ---------------------------------------------------------------------------
// 界刘备 · 仁德
// ---------------------------------------------------------------------------

fn can_rende(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "rende") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.hand(seat).is_empty() {
        return false;
    }
    common::alive_others(g, seat)
        .into_iter()
        .any(|t| g.counter(&phase_key(seat, &format!("rende.{t}"))) == 0)
}

fn rende(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "rende") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| g.counter(&phase_key(actor, &format!("rende.{t}"))) == 0)
                .collect();
            if cands.is_empty() || g.hand(actor).is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "rende") {
                None => common::pick_seat(g, actor, "rende", "交给", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "仁德：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("仁德：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "give", "target": t, "cards": [] }),
                    ))
                }
                Some(o) => Err(format!("仁德：未知选项 {o}")),
            }
        }
        "give" => {
            let t = common::seat_of(ask, "target").ok_or("仁德：缺目标")?;
            let picked = common::cards_of(ask, "cards");
            let hand: Vec<CardId> = g
                .hand(actor)
                .into_iter()
                .filter(|id| !picked.contains(id))
                .collect();
            match take_answer(g, "rende") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("rende");
                    for id in &hand {
                        b = b.option(
                            format!("card:{id}"),
                            format!("交给{}【{}】（已选 {} 张）", common::seat_name(g, t), card_label(g, id), picked.len()),
                        );
                    }
                    let escape = if picked.is_empty() { "取消".to_string() } else { "完成交给".to_string() };
                    Ok(Step::Need(
                        b.escape(escape)
                            .data(json!({ "step": "give", "target": t, "cards": picked }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    if picked.is_empty() {
                        return Ok(Step::Done);
                    }
                    for id in &picked {
                        flow::gain(g, t, id, actor, "仁德");
                    }
                    g.bump(&phase_key(actor, &format!("rende.{t}")));
                    let total = g.counter(&phase_key(actor, "rende_given")) + picked.len() as i32;
                    g.set_counter(&phase_key(actor, "rende_given"), total);
                    if total >= 2 && g.counter(&phase_key(actor, "rende_basic")) == 0 {
                        g.bump(&phase_key(actor, "rende_basic"));
                        return Ok(Step::Next(Stage::Mode, json!({ "step": "basic" })));
                    }
                    Ok(Step::Finish(format!("仁德：交给其 {} 张牌", picked.len())))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("仁德：只能交出自己的手牌".into());
                    }
                    let mut next = picked.clone();
                    next.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "give", "target": t, "cards": next }),
                    ))
                }
                Some(o) => Err(format!("仁德：未知选项 {o}")),
            }
        }
        "basic" => {
            let options: Vec<(&str, &str)> = [
                ("sha", "视为使用【杀】"),
                ("huo_sha", "视为使用【火杀】"),
                ("lei_sha", "视为使用【雷杀】"),
                ("tao", "视为使用【桃】"),
                ("jiu", "视为使用【酒】"),
            ]
            .into_iter()
            .filter(|(code, _)| {
                let Some(d) = cards::def(code) else { return false };
                if d.tags.sha && super::super::card_play::sha_limit_reached(g, actor) {
                    return false;
                }
                if d.tags.jiu && super::super::card_play::jiu_limit_reached(g, actor) {
                    return false;
                }
                true
            })
            .collect();
            match take_answer(g, "rende") {
                None => {
                    let mut b = PendingBuilder::new(actor, "mode").skill("rende");
                    for (code, label) in &options {
                        b = b.option(format!("use:{code}"), label.to_string());
                    }
                    Ok(Step::Need(b.escape("不视为使用").data(json!({ "step": "basic" })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("仁德：放弃视为使用基本牌".into())),
                Some(a) if a.starts_with("use:") => {
                    let code = a[4..].to_string();
                    super::super::card_play::use_virtual(g, actor, &code, &[], false)?;
                    Ok(Step::Finish("仁德：视为使用一张基本牌".into()))
                }
                Some(o) => Err(format!("仁德：未知选项 {o}")),
            }
        }
        other => Err(format!("仁德：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界刘备 · 激将
// ---------------------------------------------------------------------------

fn can_jijiang(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "jijiang") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.seats.get(seat).and_then(|s| s.identity) != Some(Identity::Lord) {
        return false;
    }
    if relay::order(g, seat, "shu").is_empty() {
        return false;
    }
    let Some(sha) = cards::def("sha") else { return false };
    !super::super::card_play::legal_targets(g, seat, sha).is_empty()
}

fn jijiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jijiang") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    // ② 摸牌触发（其他蜀势力角色回合外使用/打出/代出【杀】）。
    if step == "draw" || (ask.data.get("event").is_some() && step != "target" && step != "relay") {
        let lord = common::seat_of(ask, "lord").or_else(|| {
            g.seats
                .iter()
                .position(|s| s.alive && s.identity == Some(Identity::Lord) && skills::active(g, s.id, "jijiang"))
        });
        let Some(lord) = lord else { return Ok(Step::Done) };
        let by = common::seat_of(ask, "by").unwrap_or(actor);
        if by == lord || g.turn == by {
            return Ok(Step::Done);
        }
        if g.seats.get(by).map(|s| s.kingdom.as_str()) != Some("shu") {
            return Ok(Step::Done);
        }
        let sha = ask
            .data
            .get("card")
            .and_then(|v| v.as_str())
            .map(|c| def_of(g, c).map(|d| d.tags.sha).unwrap_or(false))
            .unwrap_or(step == "draw");
        if !sha {
            return Ok(Step::Done);
        }
        if g.counter(&format!("jijiang.draw.{}", g.turn)) > 0 {
            return Ok(Step::Done);
        }
        match take_answer(g, "jijiang") {
            None => {
                let mut b = PendingBuilder::new(by, "choose").skill("jijiang");
                b = b.option("use", "令主公摸一张牌");
                Ok(Step::Need(
                    b.escape("不令主公摸牌")
                        .data(json!({ "step": "draw", "lord": lord }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                g.bump(&format!("jijiang.draw.{}", g.turn));
                flow::draw(g, lord, 1);
                Ok(Step::Finish("激将：主公摸一张牌".into()))
            }
            Some(o) => Err(format!("激将：未知选项 {o}")),
        }
    } else {
        // ① 主动形态：弃一张牌？不需要；直接发动接力（视为刘备使用【杀】）。
        if step == "target" {
            let card = common::card_of(ask, "card").ok_or("激将：缺牌")?;
            let by = common::seat_of(ask, "by").unwrap_or(actor);
            let Some(sha) = cards::def("sha") else { return Ok(Step::Done) };
            let cands = super::super::card_play::legal_targets(g, actor, sha);
            match take_answer(g, "jijiang") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Finish("激将：无合法目标".into()));
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("jijiang");
                    for s in &cands {
                        b = b.option(format!("t:{s}"), format!("对{}使用【杀】", common::seat_name(g, *s)));
                    }
                    Ok(Step::Need(
                        b.escape("取消")
                            .data(json!({ "step": "target", "card": card, "by": by }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("激将：取消使用".into())),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "激将：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("激将：目标不合法".into());
                    }
                    super::super::card_play::use_relay(g, actor, "sha", &[card], &[t], by)?;
                    Ok(Step::Finish("激将：同势力角色代出【杀】".into()))
                }
                Some(o) => Err(format!("激将：未知选项 {o}")),
            }
        } else if let Some(r) = g.mark(&format!("relay.result.{actor}")).map(|s| s.to_string()) {
            g.marks.remove(&format!("relay.result.{actor}"));
            if let Some(rest) = r.strip_prefix("played:") {
                let mut it = rest.splitn(2, ':');
                let card = it.next().unwrap_or_default().to_string();
                let by: usize = it.next().and_then(|s| s.parse().ok()).unwrap_or(actor);
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "card": card, "by": by }),
                ))
            } else {
                Ok(Step::Finish("激将：无人代出【杀】".into()))
            }
        } else {
            if !can_jijiang(g, actor) {
                return Ok(Step::Done);
            }
            let started = relay::start(
                g,
                actor,
                relay::RelayOpts {
                    kingdom: "shu".to_string(),
                    for_use: true,
                    cause: "激将",
                },
            );
            if !started {
                return Ok(Step::Finish("激将：无人可代出".into()));
            }
            Ok(Step::Next(Stage::Choose, json!({ "step": "relay" })))
        }
    }
}

// ---------------------------------------------------------------------------
// 界关羽 · 义绝
// ---------------------------------------------------------------------------

fn can_yijue(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "yijue")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "yijue")) == 0
        && !own_cards(g, seat).is_empty()
        && common::alive_others(g, seat).into_iter().any(|t| !g.hand(t).is_empty())
}

fn yijue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "yijue") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            if g.counter(&phase_key(actor, "yijue")) > 0 {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| !g.hand(*t).is_empty())
                .collect();
            match take_answer(g, "yijue") {
                None => common::pick_seat(g, actor, "yijue", "令", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "义绝：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("义绝：目标不合法".into());
                    }
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "cost", "target": t })))
                }
                Some(o) => Err(format!("义绝：未知选项 {o}")),
            }
        }
        "cost" => {
            let t = common::seat_of(ask, "target").ok_or("义绝：缺目标")?;
            let cands = own_cards(g, actor);
            match take_answer(g, "yijue") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("yijue");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "cost", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("义绝：只能弃置自己的牌".into());
                    }
                    flow::discard(g, actor, &id, "义绝");
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "show", "target": t })))
                }
                Some(o) => Err(format!("义绝：未知选项 {o}")),
            }
        }
        "show" => {
            let t = common::seat_of(ask, "target").ok_or("义绝：缺目标")?;
            let hand = g.hand(t);
            match take_answer(g, "yijue") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("yijue");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("展示【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "show", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("义绝：只能展示目标自己的手牌".into());
                    }
                    g.reveal(&[id.clone()], "义绝");
                    let black = g.card_defs.get(&id).map(|c| !c.suit.is_red()).unwrap_or(false);
                    g.bump(&phase_key(actor, "yijue"));
                    if black {
                        // 本回合非锁定技失效 + 不能使用/打出手牌。
                        g.bump(&format!("suppress.{t}.{}", g.turn));
                        g.freeze_hand(t, super::super::FreezeUntil::Turn, "义绝");
                        g.bump(&format!("yijue.bonus.{actor}.{t}"));
                        Ok(Step::Finish("义绝：其本回合非锁定技失效且不能使用/打出手牌".into()))
                    } else {
                        // 红色：关羽获得，然后可令其回复 1 点体力。
                        flow::gain(g, actor, &id, t, "义绝");
                        Ok(Step::Next(Stage::Choose, json!({ "step": "heal", "target": t })))
                    }
                }
                Some(o) => Err(format!("义绝：未知选项 {o}")),
            }
        }
        "heal" => {
            let t = common::seat_of(ask, "target").ok_or("义绝：缺目标")?;
            match take_answer(g, "yijue") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("yijue");
                    b = b.option("use", "令其回复 1 点体力");
                    Ok(Step::Need(b.escape("不回复").data(json!({ "step": "heal", "target": t })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("义绝：获得其一张红牌".into())),
                Some(a) if a == "use" => {
                    flow::heal(g, t, 1, "义绝");
                    Ok(Step::Finish("义绝：获得其一张红牌并令其回复 1 点体力".into()))
                }
                Some(o) => Err(format!("义绝：未知选项 {o}")),
            }
        }
        other => Err(format!("义绝：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界张飞 · 替身 / 咆哮
// ---------------------------------------------------------------------------

fn tishen(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "tishen") || g.turn != actor || g.phase != Phase::Prepare {
        return Ok(Step::Done);
    }
    let missing = g
        .seats
        .get(actor)
        .map(|s| (s.max_hp - s.hp).max(0))
        .unwrap_or(0);
    if missing == 0 || skills::limited_used(g, actor, "tishen") {
        return Ok(Step::Done);
    }
    match take_answer(g, "tishen") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("tishen");
            b = b.option("use", format!("替身：回复 {missing} 点体力并摸 {missing} 张牌"));
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "choose" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            skills::mark_limited_used(g, actor, "tishen");
            flow::heal(g, actor, missing, "替身");
            flow::draw(g, actor, missing as usize);
            Ok(Step::Finish(format!("替身：回复 {missing} 点体力并摸 {missing} 张")))
        }
        Some(o) => Err(format!("替身：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界赵云 · 涯角
// ---------------------------------------------------------------------------

fn yajiao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "yajiao") {
        return Ok(Step::Done);
    }
    if g.turn == actor {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    // 只有首帧需要「触发牌」；后续阶段的 data 由 `Step::Next` 整份替换。
    let card = match common::card_of(ask, "card") {
        Some(c) => c,
        None if step == "ask" => return Ok(Step::Done),
        None => String::new(),
    };
    if step == "ask" && card.starts_with("v:") {
        return Ok(Step::Done);
    }
    match step {
        "ask" => {
            match take_answer(g, "yajiao") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("yajiao");
                    b = b.option("use", "涯角：展示牌堆顶一张牌");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "ask", "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    let Some(top) = g.deck.pop() else {
                        return Ok(Step::Finish("涯角：牌堆为空".into()));
                    };
                    g.reveal(&[top.clone()], "涯角");
                    let same = {
                        let a = def_of(g, &card).map(|d| d.category);
                        let b = def_of(g, &top).map(|d| d.category);
                        a.is_some() && a == b
                    };
                    // 后续阶段仍需 `card`（血缘判定/文案），`Step::Next` 整份替换 data → 必须携带。
                    if same {
                        Ok(Step::Next(
                            Stage::Target,
                            json!({ "step": "same", "top": top, "card": card }),
                        ))
                    } else {
                        Ok(Step::Next(
                            Stage::Target,
                            json!({ "step": "diff", "top": top, "card": card }),
                        ))
                    }
                }
                Some(o) => Err(format!("涯角：未知选项 {o}")),
            }
        }
        "same" => {
            let top = common::card_of(ask, "top").ok_or("涯角：缺展示牌")?;
            let cands = common::alive_seats(g);
            match take_answer(g, "yajiao") {
                None => common::pick_seat(g, actor, "yajiao", "令", &cands, json!({ "step": "same", "top": top })).into_step(),
                Some(a) if a == "pass" => {
                    // 放回牌堆顶。
                    flow::to_deck_top(g, &top);
                    g.conceal(&[top], "涯角结算完成");
                    Ok(Step::Finish("涯角：不令任何人获得".into()))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "涯角：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("涯角：目标不合法".into());
                    }
                    flow::gain(g, t, &top, actor, "涯角");
                    g.conceal(&[top], "涯角结算完成");
                    Ok(Step::Finish("涯角：令其获得展示牌".into()))
                }
                Some(o) => Err(format!("涯角：未知选项 {o}")),
            }
        }
        "diff" => {
            let top = common::card_of(ask, "top").ok_or("涯角：缺展示牌")?;
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|s| gear::within_attack_range(g, *s, actor, gear::attack_range(g, *s) as usize))
                .filter(|s| !common::target_cards(g, *s).is_empty())
                .collect();
            if cands.is_empty() {
                flow::to_deck_top(g, &top);
                g.conceal(&[top], "涯角结算完成");
                return Ok(Step::Finish("涯角：无可弃置目标".into()));
            }
            match take_answer(g, "yajiao") {
                None => common::pick_seat(g, actor, "yajiao", "弃置", &cands, json!({ "step": "diff", "top": top })).into_step(),
                Some(a) if a == "pass" => {
                    flow::to_deck_top(g, &top);
                    g.conceal(&[top], "涯角结算完成");
                    Ok(Step::Finish("涯角：不弃置".into()))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "涯角：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("涯角：目标不合法".into());
                    }
                    flow::to_deck_top(g, &top);
                    g.conceal(&[top], "涯角结算完成");
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "pick", "seat": t }),
                    ))
                }
                Some(o) => Err(format!("涯角：未知选项 {o}")),
            }
        }
        "pick" => {
            let t = common::seat_of(ask, "seat").ok_or("涯角：缺座位")?;
            let equips = common::equip_cards(g, t);
            let has_hand = !g.hand(t).is_empty();
            match take_answer(g, "yajiao") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("yajiao");
                    if has_hand {
                        b = b.option("random_hand", "弃置其一张手牌（随机）");
                    }
                    for id in &equips {
                        b = b.option(format!("card:{id}"), format!("弃置其装备【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选）")
                            .data(json!({ "step": "pick", "seat": t }))
                            .build(),
                    ))
                }
                Some(a) if a == "random_hand" => {
                    let hand = g.hand(t);
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let idx = g.rand_below(hand.len());
                    let id = hand[idx].clone();
                    flow::discard(g, t, &id, "涯角");
                    Ok(Step::Finish("涯角：弃置其一张手牌".into()))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !equips.iter().any(|c| *c == id) {
                        return Err("涯角：只能指定其装备区的牌".into());
                    }
                    flow::discard(g, t, &id, "涯角");
                    Ok(Step::Finish("涯角：弃置其一张装备牌".into()))
                }
                Some(o) => Err(format!("涯角：未知选项 {o}")),
            }
        }
        other => Err(format!("涯角：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界马超 · 铁骑（指定目标后：非锁定技失效 + 判定，不弃同花色则不能【闪】）
// ---------------------------------------------------------------------------

fn tieqi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "tieqi") {
        return Ok(Step::Done);
    }
    let card = common::card_of(ask, "card").ok_or("铁骑：缺牌")?;
    let targets = common::seats_of(ask, "targets");
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("sha_targeted");
    match step {
        "sha_targeted" => {
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("tieqi");
                    b = b.option("use", "铁骑：令目标本回合非锁定技失效并判定");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "sha_targeted", "card": card, "targets": targets }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    for t in &targets {
                        g.bump(&format!("suppress.{t}.{}", g.turn));
                    }
                    let Some(t) = targets.first().copied() else {
                        return Ok(Step::Done);
                    };
                    let r = judge::judge(g, actor, "铁骑").map(|r| r.suit);
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "judged", "card": card, "target": t, "suit": format!("{:?}", r) }),
                    ))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }
        "judged" => {
            let t = common::seat_of(ask, "target").ok_or("铁骑：缺目标")?;
            let suit = ask.data.get("suit").and_then(|v| v.as_str()).unwrap_or("Spade").to_string();
            let want = match suit.as_str() {
                "Spade" => Suit::Spade,
                "Heart" => Suit::Heart,
                "Club" => Suit::Club,
                _ => Suit::Diamond,
            };
            let cands: Vec<CardId> = own_cards(g, t)
                .into_iter()
                .filter(|id| g.card_defs.get(id).map(|c| c.suit == want).unwrap_or(false))
                .collect();
            match take_answer(g, "tieqi") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("tieqi");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】抵消铁骑", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape("不弃置（不能使用【闪】）")
                            .data(json!({ "step": "judged", "card": card, "target": t, "suit": suit }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    super::super::card_play::declare_intercept(
                        g,
                        &card,
                        super::super::ShaIntercept::Unresponsive,
                    );
                    Ok(Step::Finish("铁骑：其不弃置同花色牌，不能使用【闪】".into()))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("铁骑：只能弃置同花色的手牌/装备".into());
                    }
                    flow::discard(g, t, &id, "铁骑");
                    Ok(Step::Finish("铁骑：其弃置同花色牌".into()))
                }
                Some(o) => Err(format!("铁骑：未知选项 {o}")),
            }
        }
        other => Err(format!("铁骑：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界诸葛亮 · 观星
// ---------------------------------------------------------------------------

fn guanxing(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "guanxing") || g.turn != actor {
        return Ok(Step::Done);
    }
    let end_phase = g.phase == Phase::End;
    if end_phase && g.counter(&turn_key(actor, "guanxing_again")) == 0 {
        return Ok(Step::Done);
    }
    if g.phase != Phase::Prepare && !end_phase {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("start");
    if step == "start" {
        let n = if common::alive_seats(g).len() >= 4 { 5 } else { 3 };
        let mut cards: Vec<CardId> = Vec::new();
        for _ in 0..n {
            if let Some(id) = g.deck.pop() {
                cards.push(id);
            }
        }
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "guanxing") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("guanxing");
                b = b.option("use", format!("观星：观看牌堆顶 {n} 张"));
                // 先把牌放回牌堆顶，等待确认（避免牌悬空）。
                for id in cards.iter().rev() {
                    g.deck.push(id.clone());
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "start" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                // 取回刚才放回的牌。
                let mut picked: Vec<CardId> = Vec::new();
                for _ in 0..cards.len() {
                    if let Some(id) = g.deck.pop() {
                        picked.push(id);
                    }
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "place", "cards": picked, "idx": 0, "all_bottom": true }),
                ))
            }
            Some(o) => Err(format!("观星：未知选项 {o}")),
        };
    }
    // place：逐张置顶 / 置底。
    let mut cards = common::cards_of(ask, "cards");
    let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let mut all_bottom = ask.data.get("all_bottom").and_then(|v| v.as_bool()).unwrap_or(true);
    if idx >= cards.len() {
        if end_phase {
            g.set_counter(&turn_key(actor, "guanxing_again"), 0);
        } else if all_bottom {
            g.set_counter(&turn_key(actor, "guanxing_again"), 1);
        }
        return Ok(Step::Finish("观星：结算完成".into()));
    }
    let card = cards[idx].clone();
    match take_answer(g, "guanxing") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("guanxing");
            b = b.option("top", format!("将【{}】置于牌堆顶", card_label(g, &card)));
            b = b.option("bottom", format!("将【{}】置于牌堆底", card_label(g, &card)));
            Ok(Step::Need(
                b.escape_mandatory("（必选）")
                    .data(json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }))
                    .build(),
            ))
        }
        Some(a) if a == "top" => {
            g.deck.push(card.clone());
            all_bottom = false;
            cards.retain(|c| *c != card);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }),
            ))
        }
        Some(a) if a == "bottom" => {
            g.deck.insert(0, card.clone());
            cards.retain(|c| *c != card);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cards": cards, "idx": idx, "all_bottom": all_bottom }),
            ))
        }
        Some(o) => Err(format!("观星：未知选项 {o}")),
    }
}

trait IntoStep {
    fn into_step(self) -> Result<Step, String>;
}

impl IntoStep for Step {
    fn into_step(self) -> Result<Step, String> {
        Ok(self)
    }
}

// 保留常用工具的可读别名（避免 unused import 警告扩散）。
#[allow(dead_code)]
fn is_basic(g: &Game, id: &str) -> bool {
    def_of(g, id)
        .map(|d| d.category == Category::Basic)
        .unwrap_or(false)
}

#[allow(dead_code)]
fn zone_of(g: &Game, id: &str) -> Option<Zone> {
    g.cards.get(id).cloned()
}
