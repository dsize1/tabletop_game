//! B8 技能（一将成名·Y7：8 将 / 17 技能 + 4 条授予弦技）。
//!
//! 依据 `docs/games/sanguosha/align/B8.json` 的 `agreed`（权威）。

use serde_json::json;

use super::super::cards::Category;
use super::super::deferred::{Deferred, DeferredKind};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::pindian;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, FreezeUntil, Game, Phase, Suit, Zone};
use super::common::{self, card_label, def_of, own_cards, phase_key, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

const BOOK: &str = "shu";
const XIAN: [&str; 4] = ["hexian", "rouxian", "liexian", "jixian"];

macro_rules! skill {
    ($code:expr, $name:expr, $kind:expr, $faces:expr) => {
        SkillDef {
            code: $code,
            name: $name,
            kind: $kind,
            grant_only: false,
            transform: false,
            faces: $faces,
        }
    };
}

pub static SKILLS: &[SkillDef] = &[
    // —— 辛宪英 ——
    skill!("zhongjian", "忠鉴", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }]),
    skill!("caishi", "才识", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    // —— 吴苋 ——
    skill!("fumian", "福绵", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    skill!("daiyan", "怠宴", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    // —— 徐氏 ——
    skill!("wengua", "问卦", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }]),
    skill!("fuzhu", "伏诛", SkillKind::Normal, &[Face { window: Window::OtherPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    // —— 曹节 ——
    skill!("shouxi", "守玺", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaIntercept], can_active: false }]),
    skill!("huimin", "惠民", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    // —— 嵇康 ——
    skill!("qingxian", "清弦", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[
        Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ },
        Subscription { event: EventKind::Healed, subject: Subject::Self_ },
    ], hooks: &[], can_active: false }]),
    skill!("juexiang", "绝响", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }]),
    // —— 绝响授予的四条弦技（grant_only）——
    SkillDef { code: "hexian", name: "和弦", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::Healed, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "rouxian", name: "柔弦", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "liexian", name: "烈弦", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::Healed, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "jixian", name: "激弦", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    // —— 秦宓 ——
    skill!("jianzheng", "谏征", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaTargeted], can_active: false }]),
    skill!("zhuandui", "专对", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaTargeted], can_active: false }]),
    skill!("tianbian", "天辩", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }]),
    // —— 薛综 ——
    skill!("funan", "复难", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardPlayed, subject: Subject::Other }], hooks: &[], can_active: false }]),
    skill!("jiexun", "诫训", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }]),
    // —— 蔡邕 ——
    skill!("bizhuan", "辟撰", SkillKind::Normal, &[Face { window: Window::AnyTime, subscriptions: &[
        Subscription { event: EventKind::CardUsed, subject: Subject::By },
        Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ },
    ], hooks: &[], can_active: false }]),
    skill!("tongbo", "通博", SkillKind::Normal, &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[Subscription { event: EventKind::PhaseEnd, subject: Subject::Any }], hooks: &[], can_active: false }]),
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zhongjian" => zhongjian,
        "caishi" => caishi,
        "fumian" => fumian,
        "daiyan" => daiyan,
        "wengua" => wengua,
        "fuzhu" => fuzhu,
        "shouxi" => shouxi,
        "huimin" => huimin,
        "qingxian" => qingxian,
        "juexiang" => juexiang,
        "hexian" => xian_heal,
        "liexian" => xian_heal,
        "rouxian" => xian_damage,
        "jixian" => xian_damage,
        "jianzheng" => jianzheng,
        "zhuandui" => zhuandui,
        "funan" => funan,
        "jiexun" => jiexun,
        "bizhuan" => bizhuan,
        "tongbo" => tongbo,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "zhongjian" => can_zhongjian,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 共用工具
// ---------------------------------------------------------------------------

fn no_one_dying(g: &Game) -> bool {
    !g.seats.iter().any(|s| flow::is_dying(s))
}

fn books(g: &Game, seat: usize) -> Vec<CardId> {
    let mut ids: Vec<CardId> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, m) if *s == seat && m == BOOK))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn random_equip_from_pile(g: &mut Game) -> Option<CardId> {
    let is_equip = |g: &Game, id: &str| def_of(g, id).map(|d| d.slot.is_some()).unwrap_or(false);
    let from_deck: Option<String> = g.deck.iter().rev().find(|id| is_equip(g, id)).cloned();
    from_deck.or_else(|| {
        let mut cands: Vec<String> = g
            .cards
            .iter()
            .filter(|(id, z)| **z == Zone::Discard && is_equip(g, id))
            .map(|(id, _)| id.clone())
            .collect();
        cands.sort();
        cands.into_iter().next()
    })
}

/// 韩当式「失去 1 点体力并随机使用一张装备牌」与「回复 1 点体力并弃置一张装备牌」的共享执行。
fn qingxian_execute(g: &mut Game, _owner: usize, target: usize, lose: bool) -> Option<CardId> {
    if lose {
        flow::lose_hp(g, target, 1, "清弦");
        let id = random_equip_from_pile(g)?;
        let _ = super::super::card_play::equip_from_pile(g, target, &id);
        Some(id)
    } else {
        flow::heal(g, target, 1, "清弦");
        None
    }
}

fn is_club(g: &Game, id: &str) -> bool {
    g.card_defs.get(id).map(|c| c.suit == Suit::Club).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 数值出口
// ---------------------------------------------------------------------------

pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    let mut n = 0;
    // 忠鉴：皆不包含 → 本回合手牌上限 -1。
    n -= g.counter(&turn_key(seat, "zhongjian_penalty"));
    // 才识①：本回合手牌上限 +1。
    n += g.counter(&turn_key(seat, "hand_limit_extra"));
    // 辟撰：每张「书」手牌上限 +1。
    n += books(g, seat).len() as i32;
    n
}

pub fn draw_bonus(g: &mut Game, seat: usize) -> usize {
    let key = turn_key(seat, "fumian_draw");
    let n = g.counter(&key).max(0) as usize;
    if n > 0 {
        g.set_counter(&key, 0);
    }
    n
}

pub fn deny_target(g: &Game, user: usize, target: usize) -> bool {
    // 才识②：本回合不能对自己使用牌。
    user == target && g.counter(&turn_key(user, "no_self_target")) > 0
}

/// 绝响保护在**牌实例**层面判定（`deny_target_card` 只拿到牌定义 code，无法定花色）。
pub fn deny_target_card(_g: &Game, _user: usize, _target: usize, _code: &str) -> bool {
    false
}

/// 绝响：保护目标不成为其他角色使用的**梅花**牌的目标（按实例花色判定）。
pub fn card_ineffective_for(g: &mut Game, user: usize, target: usize, card: &str) -> bool {
    if user == target {
        return false;
    }
    if g.counter(&format!("juexiang.protect.{target}")) == 0 {
        return false;
    }
    g.card_defs
        .get(card)
        .map(|c| c.suit == Suit::Club)
        .unwrap_or(false)
}

pub fn on_turn_start(g: &mut Game, seat: usize) {
    // 绝响保护：到其下回合开始即失效。
    let key = format!("juexiang.protect.{seat}");
    if g.counter(&key) > 0 {
        g.set_counter(&key, 0);
    }
}

pub fn on_death(g: &mut Game, dead: usize) {
    // 绝响：死亡时可令一名角色随机获得一条弦技，并获得花色保护。
    if !skills::active(g, dead, "juexiang") {
        return;
    }
    g.deferred.push(Deferred::ask(
        DeferredKind::SkillTrigger,
        "juexiang",
        Stage::Choose,
        dead,
        json!({ "step": "pick" }),
    ));
}

/// 福绵②：本回合限一次，红色基本牌/普通锦囊可多指定 N 个目标（无距离）。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    let n = g.counter(&turn_key(actor, "fumian_extra"));
    if n <= 0 || g.counter(&turn_key(actor, "fumian_extra_used")) > 0 {
        return;
    }
    let Some(def) = def_of(g, card) else { return };
    let red = g.card_defs.get(card).map(|c| c.suit.is_red()).unwrap_or(false);
    let eligible = def.category == Category::Basic
        || (def.category == Category::Trick && !def.tags.delayed);
    if !red || !eligible {
        return;
    }
    super::super::card_play::declare_extra_targets(g, card, n);
    super::super::card_play::declare_no_distance(g, card);
    g.bump(&turn_key(actor, "fumian_extra_used"));
}

trait IntoStep {
    fn into_step(self) -> Result<Step, String>;
}

impl IntoStep for Step {
    fn into_step(self) -> Result<Step, String> {
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// 辛宪英 · 忠鉴
// ---------------------------------------------------------------------------

fn zhongjian_targets(g: &Game, seat: usize) -> Vec<usize> {
    common::alive_others(g, seat)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty())
        .collect()
}

fn can_zhongjian(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "zhongjian")
        || g.turn != seat
        || g.phase != Phase::Play
        || g.hand(seat).is_empty()
        || zhongjian_targets(g, seat).is_empty()
    {
        return false;
    }
    let limit = if g.counter(&turn_key(seat, "zhongjian_twice")) > 0 {
        2
    } else {
        1
    };
    g.counter(&phase_key(seat, "zhongjian")) < limit
}

fn zhongjian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhongjian") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands = zhongjian_targets(g, actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "zhongjian") {
                None => common::pick_seat(g, actor, "zhongjian", "令", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "忠鉴：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("忠鉴：目标不合法".into());
                    }
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "self_card", "target": t })))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        "self_card" => {
            let t = common::seat_of(ask, "target").ok_or("忠鉴：缺目标")?;
            let hand = g.hand(actor);
            match take_answer(g, "zhongjian") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("zhongjian");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("展示【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(b.escape_mandatory("（必选一张）").data(json!({ "step": "self_card", "target": t })).build()))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("忠鉴：只能展示自己的手牌".into());
                    }
                    g.reveal(&[id.clone()], "忠鉴");
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "show", "target": t, "mine": id, "shown": [] }),
                    ))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        "show" => {
            let t = common::seat_of(ask, "target").ok_or("忠鉴：缺目标")?;
            let mine = common::card_of(ask, "mine").ok_or("忠鉴：缺己方展示牌")?;
            let shown = common::cards_of(ask, "shown");
            let need = g.seats.get(t).map(|s| s.hp.max(1) as usize).unwrap_or(1);
            let cands: Vec<String> = g.hand(t).into_iter().filter(|id| !shown.contains(id)).collect();
            if shown.len() >= need || cands.is_empty() {
                return Ok(zhongjian_resolve(g, actor, t, &mine, &shown));
            }
            match take_answer(g, "zhongjian") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("zhongjian");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("展示【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "show", "target": t, "mine": mine, "shown": shown }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("忠鉴：只能展示目标自己的手牌".into());
                    }
                    let mut next = shown.clone();
                    next.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "show", "target": t, "mine": mine, "shown": next }),
                    ))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        "reward" => {
            let t = common::seat_of(ask, "target").ok_or("忠鉴：缺目标")?;
            match take_answer(g, "zhongjian") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("zhongjian");
                    b = b.option("draw", "摸一张牌");
                    b = b.option("discard", "弃置一名其他角色一张牌");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "reward", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("忠鉴：放弃同色奖励".into())),
                Some(a) if a == "draw" => {
                    flow::draw(g, actor, 1);
                    Ok(Step::Finish("忠鉴：摸一张牌".into()))
                }
                Some(a) if a == "discard" => {
                    let cands: Vec<usize> = common::alive_others(g, actor)
                        .into_iter()
                        .filter(|s| !common::target_cards(g, *s).is_empty())
                        .collect();
                    if cands.is_empty() {
                        return Ok(Step::Finish("忠鉴：无牌可弃".into()));
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "discard_seat", "target": t, "cands": cands }),
                    ))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        "discard_seat" => {
            let t = common::seat_of(ask, "target").ok_or("忠鉴：缺目标")?;
            let cands = common::seats_of(ask, "cands");
            match take_answer(g, "zhongjian") {
                None => common::pick_seat(g, actor, "zhongjian", "弃置", &cands, json!({ "step": "discard_seat", "target": t, "cands": cands })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Finish("忠鉴：放弃弃置".into())),
                Some(a) if a.starts_with("t:") => {
                    let s: usize = a[2..].parse().map_err(|_| "忠鉴：座位不合法")?;
                    if !cands.contains(&s) {
                        return Err("忠鉴：座位不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "discard_card", "target": t, "seat": s }),
                    ))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        "discard_card" => {
            let s = common::seat_of(ask, "seat").ok_or("忠鉴：缺座位")?;
            let cands = common::target_cards(g, s);
            match take_answer(g, "zhongjian") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Finish("忠鉴：无牌可弃".into()));
                    }
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("zhongjian");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "discard_card", "seat": s }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("忠鉴：只能弃置目标的牌".into());
                    }
                    flow::discard(g, s, &id, "忠鉴");
                    Ok(Step::Finish("忠鉴：弃置其一张牌".into()))
                }
                Some(o) => Err(format!("忠鉴：未知选项 {o}")),
            }
        }
        other => Err(format!("忠鉴：未知阶段 {other}")),
    }
}

fn zhongjian_resolve(g: &mut Game, actor: usize, target: usize, mine: &str, shown: &[CardId]) -> Step {
    g.reveal(shown, "忠鉴");
    let my = g.card_defs.get(mine).cloned();
    let same_color = my
        .as_ref()
        .map(|m| {
            shown.iter().any(|id| {
                g.card_defs
                    .get(id)
                    .map(|c| c.suit.is_red() == m.suit.is_red())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let same_rank = my
        .as_ref()
        .map(|m| {
            shown.iter().any(|id| {
                g.card_defs
                    .get(id)
                    .map(|c| c.rank == m.rank)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    g.conceal(shown, "忠鉴结算完成");
    g.bump(&phase_key(actor, "zhongjian"));
    if same_rank {
        g.bump(&turn_key(actor, "zhongjian_twice"));
    }
    if !same_color && !same_rank {
        g.bump(&turn_key(actor, "zhongjian_penalty"));
    }
    if same_color {
        let data = json!({ "step": "reward", "target": target });
        Step::NeedWith(
            Stage::Choose,
            data.clone(),
            PendingBuilder::new(actor, "choose")
                .skill("zhongjian")
                .option("draw", "摸一张牌")
                .option("discard", "弃置一名其他角色一张牌")
                .escape("放弃")
                .data(data)
                .build(),
        )
    } else {
        Step::Finish("忠鉴：展示结算完成".into())
    }
}

// ---------------------------------------------------------------------------
// 辛宪英 · 才识
// ---------------------------------------------------------------------------

fn caishi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "caishi") || g.turn != actor || g.phase != Phase::Draw {
        return Ok(Step::Done);
    }
    let can_heal = g.seats.get(actor).map(|s| s.hp < s.max_hp).unwrap_or(false);
    match take_answer(g, "caishi") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("caishi");
            b = b.option("opt1", "本回合手牌上限 +1");
            if can_heal {
                b = b.option("opt2", "回复 1 点体力，本回合不能对自己使用牌");
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "choose" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "opt1" => {
            g.bump(&turn_key(actor, "hand_limit_extra"));
            Ok(Step::Finish("才识：本回合手牌上限 +1".into()))
        }
        Some(a) if a == "opt2" && can_heal => {
            flow::heal(g, actor, 1, "才识");
            g.bump(&turn_key(actor, "no_self_target"));
            Ok(Step::Finish("才识：回复 1 点体力，本回合不能对自己使用牌".into()))
        }
        Some(o) => Err(format!("才识：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 吴苋 · 福绵
// ---------------------------------------------------------------------------

fn fumian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "fumian") || g.turn != actor || g.phase != Phase::Prepare {
        return Ok(Step::Done);
    }
    // N 的口径：切换后本回合 +1；切换后的下一回合重置为 1。
    if g.counter(&turn_key(actor, "fumian_reset")) > 0 {
        g.set_counter(&turn_key(actor, "fumian_n"), 1);
        g.set_counter(&turn_key(actor, "fumian_reset"), 0);
    }
    if g.counter(&turn_key(actor, "fumian_n")) == 0 {
        g.set_counter(&turn_key(actor, "fumian_n"), 1);
    }
    match take_answer(g, "fumian") {
        None => {
            let n = g.counter(&turn_key(actor, "fumian_n"));
            let mut b = PendingBuilder::new(actor, "choose").skill("fumian");
            b = b.option("opt1", format!("摸牌阶段多摸 {n} 张牌"));
            b = b.option("opt2", format!("本回合限一次：红色牌可多指定 {n} 个目标（无距离）"));
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "choose" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "opt1" || a == "opt2" => {
            let pick = if a == "opt1" { 1 } else { 2 };
            let last = g.counter(&turn_key(actor, "fumian_last"));
            let n = g.counter(&turn_key(actor, "fumian_n"));
            if last != 0 && last != pick {
                g.set_counter(&turn_key(actor, "fumian_n"), n + 1);
                g.set_counter(&turn_key(actor, "fumian_reset"), 1);
            }
            g.set_counter(&turn_key(actor, "fumian_last"), pick);
            let n = g.counter(&turn_key(actor, "fumian_n"));
            if pick == 1 {
                g.set_counter(&turn_key(actor, "fumian_draw"), n);
                g.set_counter(&turn_key(actor, "fumian_extra"), 0);
                Ok(Step::Finish(format!("福绵：摸牌阶段多摸 {n} 张")))
            } else {
                g.set_counter(&turn_key(actor, "fumian_extra"), n);
                g.set_counter(&turn_key(actor, "fumian_extra_used"), 0);
                Ok(Step::Finish(format!("福绵：本回合限一次，红色牌可多指定 {n} 个目标")))
            }
        }
        Some(o) => Err(format!("福绵：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 吴苋 · 怠宴
// ---------------------------------------------------------------------------

fn daiyan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "daiyan") || g.turn != actor || g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let cands = common::alive_others(g, actor);
    match take_answer(g, "daiyan") {
        None => common::pick_seat(g, actor, "daiyan", "令", &cands, json!({ "step": "pick" })).into_step(),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "怠宴：目标不合法")?;
            if !cands.contains(&t) {
                return Err("怠宴：目标不合法".into());
            }
            // 牌堆顶 → 底找第一张红桃基本牌；没有则弃牌堆找一张。
            let is_heart_basic = |g: &Game, id: &str| {
                def_of(g, id)
                    .map(|d| d.category == Category::Basic)
                    .unwrap_or(false)
                    && g.card_defs.get(id).map(|c| c.suit == Suit::Heart).unwrap_or(false)
            };
            let from_deck: Option<CardId> = g.deck.iter().rev().find(|id| is_heart_basic(g, id)).cloned();
            let pick = from_deck.or_else(|| {
                let mut cands: Vec<CardId> = g
                    .cards
                    .iter()
                    .filter(|(id, z)| **z == Zone::Discard && is_heart_basic(g, id))
                    .map(|(id, _)| id.clone())
                    .collect();
                cands.sort();
                cands.into_iter().next()
            });
            let last = g
                .counter(&format!("daiyan.last.{actor}")) as usize;
            match pick {
                Some(id) => {
                    g.deck.retain(|c| *c != id);
                    flow::gain(g, t, &id, actor, "怠宴");
                }
                None => {}
            }
            g.set_counter(&format!("daiyan.last.{actor}"), (t + 1) as i32);
            if last == t + 1 {
                flow::lose_hp(g, t, 1, "怠宴");
                Ok(Step::Finish("怠宴：其获得一张红桃基本牌并失去 1 点体力".into()))
            } else {
                Ok(Step::Finish("怠宴：其获得一张红桃基本牌".into()))
            }
        }
        Some(o) => Err(format!("怠宴：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 徐氏 · 问卦（他人侧）
// ---------------------------------------------------------------------------

pub fn other_side_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    if g.turn != seat || g.phase != Phase::Play {
        return vec![];
    }
    if g.counter(&turn_key(seat, "wengua")) > 0 || g.hand(seat).is_empty() {
        return vec![];
    }
    let has = (0..g.seats.len()).any(|s| s != seat && skills::active(g, s, "wengua") && g.seats[s].alive);
    if has {
        vec!["wengua"]
    } else {
        vec![]
    }
}

pub fn start_other(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if code != "wengua" || !other_side_actions(g, seat).contains(&"wengua") {
        return Err("问卦：当前不可用".into());
    }
    let owner = (0..g.seats.len())
        .find(|s| *s != seat && skills::active(g, *s, "wengua") && g.seats[*s].alive)
        .ok_or("问卦：没有徐氏")?;
    super::super::resolve::push_ask(
        g,
        "wengua",
        Stage::Choose,
        seat,
        json!({ "step": "other", "owner": owner }),
    );
    Ok(())
}

fn wengua(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("other");
    let owner = common::seat_of(ask, "owner").ok_or("问卦：缺拥有者")?;
    match step {
        "other" => {
            let hand = g.hand(actor);
            match take_answer(g, "wengua") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("wengua");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("将【{}】交给徐氏", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "other", "owner": owner }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("问卦：只能交出自己的手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "place", "owner": owner, "card": id }),
                    ))
                }
                Some(o) => Err(format!("问卦：未知选项 {o}")),
            }
        }
        "place" => {
            let card = common::card_of(ask, "card").ok_or("问卦：缺牌")?;
            if !skills::active(g, owner, "wengua") {
                return Ok(Step::Done);
            }
            match take_answer(g, "wengua") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("wengua");
                    b = b.option("opt1", "置于牌堆顶（双方从牌堆底摸一张）");
                    b = b.option("opt2", "置于牌堆底（双方从牌堆顶摸一张）");
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "place", "owner": owner, "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" || a == "opt2" => {
                    g.cards.insert(card.clone(), Zone::Draw);
                    if a == "opt1" {
                        g.deck.push(card.clone());
                        flow::draw_bottom(g, owner, 1);
                        flow::draw_bottom(g, actor, 1);
                    } else {
                        g.deck.insert(0, card.clone());
                        flow::draw(g, owner, 1);
                        flow::draw(g, actor, 1);
                    }
                    g.bump(&turn_key(actor, "wengua"));
                    Ok(Step::Finish("问卦：置牌并双方各摸一张".into()))
                }
                Some(o) => Err(format!("问卦：未知选项 {o}")),
            }
        }
        other => Err(format!("问卦：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 徐氏 · 伏诛
// ---------------------------------------------------------------------------

fn fuzhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "fuzhu") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("fire");
    if step == "fire" {
        let target = common::seat_of(ask, "target")
            .or_else(|| ask.data.get("seat").and_then(|v| v.as_u64()).map(|v| v as usize))
            .unwrap_or(actor);
        if target == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        // 每名男性角色的结束阶段 + 牌堆 ≤ 体力×10。
        if g.seats.get(target).map(|s| s.male).unwrap_or(false) == false {
            return Ok(Step::Done);
        }
        let hp = g.seats.get(actor).map(|s| s.hp.max(0) as usize).unwrap_or(0);
        if g.deck.len() > hp * 10 {
            return Ok(Step::Done);
        }
        let limit = g.seats.len().max(1);
        match take_answer(g, "fuzhu") {
            None => common::pick_seat(
                g,
                actor,
                "fuzhu",
                "对",
                &[target],
                json!({ "step": "fire", "target": target, "used": 0, "limit": limit }),
            )
            .into_step(),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("t:") => Ok(Step::Next(
                Stage::CardAsk,
                json!({ "step": "next", "target": target, "used": 0, "limit": limit }),
            )),
            Some(o) => Err(format!("伏诛：未知选项 {o}")),
        }
    } else {
        // 依次使用牌堆中的【杀】。
        let target = common::seat_of(ask, "target").ok_or("伏诛：缺目标")?;
        let used = ask.data.get("used").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = ask.data.get("limit").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        if !g.seats.get(target).map(|s| s.alive).unwrap_or(false) || used >= limit {
            flow::shuffle_deck(g);
            return Ok(Step::Finish("伏诛：结算完成并洗牌".into()));
        }
        let sha = g
            .deck
            .iter()
            .rev()
            .find(|id| def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
            .cloned();
        match sha {
            Some(id) => {
                super::super::card_play::use_from_pile(g, actor, &id, &[target])?;
                Ok(Step::Next(
                    Stage::CardAsk,
                    json!({ "step": "next", "target": target, "used": used + 1, "limit": limit }),
                ))
            }
            None => {
                flow::shuffle_deck(g);
                Ok(Step::Finish("伏诛：牌堆已无【杀】，洗牌".into()))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 曹节 · 守玺（成为【杀】目标后声明牌名）
// ---------------------------------------------------------------------------

fn shouxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "shouxi") {
        return Ok(Step::Done);
    }
    let Some(by) = common::seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    let card = common::card_of(ask, "card").unwrap_or_default();
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("name");
    if step.starts_with("by_") {
        return shouxi_by_opt(g, ask);
    }
    match step {
        "name" => {
            // 首帧：先消费上一次回答（`Step::Need` 不改帧数据，重入仍是 name）。
            if let Some(ans) = take_answer(g, "shouxi") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let code = ans.strip_prefix("use:").ok_or("守玺：未知选项")?.to_string();
                g.bump(&turn_key(actor, &format!("shouxi.{code}")));
                let has = g
                    .hand(by)
                    .iter()
                    .any(|id| def_of(g, id).map(|d| d.code == code).unwrap_or(false));
                let mut b = PendingBuilder::new(by, "choose").skill("shouxi");
                if has {
                    b = b.option("opt1", "弃置一张该牌名的牌，然后获得曹节一张牌");
                }
                b = b.option("opt2", "令此【杀】对曹节无效");
                let d = json!({ "step": "by_opt", "by": by, "card": card, "decl": code });
                return Ok(Step::NeedWith(
                    Stage::Choose,
                    d.clone(),
                    b.escape_mandatory("选择一项").data(d).build(),
                ));
            }
            let mut b = PendingBuilder::new(actor, "mode").skill("shouxi");
            for d in crate::cards_generated::DEFS.iter() {
                if d.slot.is_some() {
                    continue;
                }
                if g.counter(&turn_key(actor, &format!("shouxi.{}", d.code))) > 0 {
                    continue;
                }
                b = b.option(format!("use:{}", d.code), format!("声明【{}】", d.name));
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "name", "by": by, "card": card }))
                    .build(),
            ))
        }
        _ => Ok(Step::Done),
    }
}

/// 守玺 by_opt / gain 阶段（与主 handler 共用 code，避免多技能帧嵌套）。
fn shouxi_by_opt(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let by = common::seat_of(ask, "by").ok_or("守玺：缺使用者")?;
    let card = common::card_of(ask, "card").unwrap_or_default();
    let decl = ask.data.get("decl").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("by_opt");
    match step {
        "by_opt" => {
            let has = g
                .hand(by)
                .iter()
                .any(|id| def_of(g, id).map(|d| d.code == decl).unwrap_or(false));
            match take_answer(g, "shouxi") {
                None => Ok(Step::Done),
                Some(a) if a == "opt2" => {
                    super::super::card_play::declare_intercept(
                        g,
                        &card,
                        super::super::ShaIntercept::Immune,
                    );
                    Ok(Step::Finish("守玺：此【杀】对曹节无效".into()))
                }
                Some(a) if a == "opt1" && has => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "by_pick", "by": by, "card": card, "decl": decl }),
                )),
                Some(o) => Err(format!("守玺：未知选项 {o}")),
            }
        }
        "by_pick" => {
            let cands: Vec<CardId> = g
                .hand(by)
                .into_iter()
                .filter(|id| def_of(g, id).map(|d| d.code == decl).unwrap_or(false))
                .collect();
            match take_answer(g, "shouxi") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(by, "pick_own").skill("shouxi");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "by_pick", "by": by, "card": card, "decl": decl }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("守玺：只能弃置该牌名的牌".into());
                    }
                    flow::discard(g, by, &id, "守玺");
                    let mine = common::target_cards(g, actor);
                    if mine.is_empty() {
                        return Ok(Step::Finish("守玺：其弃置该牌名，无牌可获得".into()));
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "by_gain", "by": by, "card": card }),
                    ))
                }
                Some(o) => Err(format!("守玺：未知选项 {o}")),
            }
        }
        "by_gain" => {
            let cands = common::target_cards(g, actor);
            match take_answer(g, "shouxi") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("shouxi");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("交出【{}】", card_label(g, id)));
                    }
                    Ok(Step::NeedWith(
                        Stage::PickOther,
                        json!({ "step": "by_gain", "by": by }),
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "by_gain", "by": by }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("守玺：只能交出自己的牌".into());
                    }
                    flow::gain(g, by, &id, actor, "守玺");
                    Ok(Step::Finish("守玺：其弃置该牌名并获得曹节一张牌".into()))
                }
                Some(o) => Err(format!("守玺：未知选项 {o}")),
            }
        }
        other => Err(format!("守玺：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 曹节 · 惠民（结束阶段）
// ---------------------------------------------------------------------------

fn huimin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "huimin") || g.turn != actor || g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let x = common::alive_seats(g)
        .into_iter()
        .filter(|s| g.hand_size(*s) < g.seats[*s].hp.max(0) as usize)
        .count();
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    match step {
        "ask" => {
            if x == 0 {
                return Ok(Step::Done);
            }
            match take_answer(g, "huimin") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("huimin");
                    b = b.option("use", format!("惠民：摸 {x} 张牌并依次分发"));
                    Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    let drawn = flow::draw(g, actor, x);
                    g.reveal(&drawn, "惠民");
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "start", "cards": drawn }),
                    ))
                }
                Some(o) => Err(format!("惠民：未知选项 {o}")),
            }
        }
        "start" => {
            let cards = common::cards_of(ask, "cards");
            let cands = common::alive_seats(g);
            match take_answer(g, "huimin") {
                None => common::pick_seat(
                    g,
                    actor,
                    "huimin",
                    "从",
                    &cands,
                    json!({ "step": "start", "cards": cards }),
                )
                .into_step(),
                Some(a) if a == "pass" => {
                    for id in cards {
                        super::super::card_play::to_discard(g, &id);
                    }
                    Ok(Step::Finish("惠民：放弃分发".into()))
                }
                Some(a) if a.starts_with("t:") => {
                    let start: usize = a[2..].parse().map_err(|_| "惠民：座位不合法")?;
                    if !cands.contains(&start) {
                        return Err("惠民：座位不合法".into());
                    }
                    let order = seat_order(g, start);
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "distribute", "cards": cards, "order": order, "idx": 0 }),
                    ))
                }
                Some(o) => Err(format!("惠民：未知选项 {o}")),
            }
        }
        "distribute" => {
            let mut cards = common::cards_of(ask, "cards");
            let order = common::seats_of(ask, "order");
            let mut idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if cards.is_empty() || idx >= order.len() {
                for id in cards {
                    super::super::card_play::to_discard(g, &id);
                }
                g.conceal(&common::cards_of(ask, "cards"), "惠民结算完成");
                return Ok(Step::Finish("惠民：分发完成".into()));
            }
            let seat = order[idx];
            match take_answer(g, "huimin") {
                None => {
                    let mut b = PendingBuilder::new(seat, "pick_other").skill("huimin");
                    for id in &cards {
                        b = b.option(format!("card:{id}"), format!("获得【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "distribute", "cards": cards, "order": order, "idx": idx }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cards.iter().any(|c| *c == id) {
                        return Err("惠民：牌不在展示区".into());
                    }
                    if !g.seats.get(seat).map(|s| s.alive).unwrap_or(false) {
                        idx += 1;
                        return Ok(Step::Next(
                            Stage::PickOther,
                            json!({ "step": "distribute", "cards": cards, "order": order, "idx": idx }),
                        ));
                    }
                    flow::gain(g, seat, &id, actor, "惠民");
                    cards.retain(|c| *c != id);
                    idx += 1;
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "distribute", "cards": cards, "order": order, "idx": idx }),
                    ))
                }
                Some(o) => Err(format!("惠民：未知选项 {o}")),
            }
        }
        other => Err(format!("惠民：未知阶段 {other}")),
    }
}

fn seat_order(g: &Game, start: usize) -> Vec<usize> {
    let n = g.seats.len();
    (0..n)
        .map(|k| (start + k) % n)
        .filter(|s| g.seats.get(*s).map(|x| x.alive).unwrap_or(false))
        .collect()
}

// ---------------------------------------------------------------------------
// 嵇康 · 清弦 / 绝响 / 四弦
// ---------------------------------------------------------------------------

fn qingxian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    if !no_one_dying(g) {
        return Ok(Step::Done);
    }
    qingxian_common(g, ask, "qingxian", false)
}

fn xian_damage(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    if !no_one_dying(g) {
        return Ok(Step::Done);
    }
    qingxian_common(g, ask, &ask.skill, false)
}

fn xian_heal(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    if !no_one_dying(g) {
        return Ok(Step::Done);
    }
    qingxian_common(g, ask, &ask.skill, true)
}

/// 清弦/四弦共用：`from_heal` = 回复体力分支（对象由自己选一名其他角色），否则对象是伤害来源。
fn qingxian_common(g: &mut Game, ask: &Ask, code: &str, from_heal: bool) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, code) {
        return Ok(Step::Done);
    }
    let skill = code;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    let other = common::seat_of(ask, "target")
        .or_else(|| common::seat_of(ask, "by"))
        .or_else(|| ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize));
    match step {
        "ask" => {
            if from_heal {
                let cands = common::alive_others(g, actor);
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                match take_answer(g, skill) {
                    None => {
                        let mut b = PendingBuilder::new(actor, "target").skill(skill);
                        for s in &cands {
                            b = b.option(format!("t:{s}"), format!("令{}执行一项", common::seat_name(g, *s)));
                        }
                        Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
                    }
                    Some(a) if a == "pass" => Ok(Step::Done),
                    Some(a) if a.starts_with("t:") => {
                        let t: usize = a[2..].parse().map_err(|_| "清弦：目标不合法")?;
                        if !cands.contains(&t) {
                            return Err("清弦：目标不合法".into());
                        }
                        Ok(Step::Next(Stage::Choose, json!({ "step": "opt", "target": t })))
                    }
                    Some(o) => Err(format!("清弦：未知选项 {o}")),
                }
            } else {
                let Some(by) = other else { return Ok(Step::Done) };
                if by == actor {
                    return Ok(Step::Done);
                }
                match take_answer(g, skill) {
                    None => {
                        let mut b = PendingBuilder::new(actor, "choose").skill(skill);
                        b = b.option("use", format!("令{}执行一项", common::seat_name(g, by)));
                        Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask", "target": by })).build()))
                    }
                    Some(a) if a == "pass" => Ok(Step::Done),
                    Some(a) if a == "use" => {
                        Ok(Step::Next(Stage::Choose, json!({ "step": "opt", "target": by })))
                    }
                    Some(o) => Err(format!("清弦：未知选项 {o}")),
                }
            }
        }
        "opt" => {
            let t = common::seat_of(ask, "target").ok_or("清弦：缺目标")?;
            let has_equip = !common::equip_cards(g, t).is_empty();
            match take_answer(g, skill) {
                None => {
                    let mut b = PendingBuilder::new(t, "choose").skill(skill);
                    b = b.option("opt1", "失去 1 点体力，随机使用一张装备牌");
                    if has_equip {
                        b = b.option("opt2", "回复 1 点体力，弃置一张装备牌");
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "opt", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" => {
                    let used = qingxian_execute(g, actor, t, true);
                    if let Some(id) = used {
                        if is_club(g, &id) {
                            flow::draw(g, actor, 1);
                        }
                    }
                    Ok(Step::Finish("清弦：其失去 1 点体力并使用一张装备牌".into()))
                }
                Some(a) if a == "opt2" && has_equip => Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "equip", "target": t }),
                )),
                Some(o) => Err(format!("清弦：未知选项 {o}")),
            }
        }
        "equip" => {
            let t = common::seat_of(ask, "target").ok_or("清弦：缺目标")?;
            let equips = common::equip_cards(g, t);
            match take_answer(g, skill) {
                None => {
                    if equips.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(t, "pick_other").skill(skill);
                    for id in &equips {
                        b = b.option(format!("card:{id}"), format!("弃置装备【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "equip", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !equips.iter().any(|c| *c == id) {
                        return Err("清弦：只能弃置其装备区的牌".into());
                    }
                    flow::heal(g, t, 1, "清弦");
                    let club = is_club(g, &id);
                    flow::discard(g, t, &id, "清弦");
                    if club {
                        flow::draw(g, actor, 1);
                    }
                    Ok(Step::Finish("清弦：其回复 1 点体力并弃置一张装备牌".into()))
                }
                Some(o) => Err(format!("清弦：未知选项 {o}")),
            }
        }
        other => Err(format!("清弦：未知阶段 {other}")),
    }
}

fn juexiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let cands = common::alive_seats(g);
    match take_answer(g, "juexiang") {
        None => {
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(actor, "target").skill("juexiang");
            for s in &cands {
                b = b.option(
                    format!("t:{s}"),
                    format!("令{}随机获得一条弦技", common::seat_name(g, *s)),
                );
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "pick" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "绝响：目标不合法")?;
            if !cands.contains(&t) {
                return Err("绝响：目标不合法".into());
            }
            let idx = g.rand_below(XIAN.len());
            let code = XIAN[idx];
            skills::grant(g, t, code);
            g.bump(&format!("juexiang.protect.{t}"));
            Ok(Step::Finish(format!(
                "绝响：{} 获得「{}」",
                common::seat_name(g, t),
                skills::def(code).map(|d| d.name).unwrap_or(code)
            )))
        }
        Some(o) => Err(format!("绝响：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 秦宓 · 谏征 / 专对 / 天辩
// ---------------------------------------------------------------------------

pub fn fire_sha_targeted(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    let mut pushed = false;
    // 谏征：第三方（在使用者攻击范围内）。
    let jianzheng: Vec<usize> = (0..g.seats.len())
        .filter(|&s| {
            s != user
                && !targets.contains(&s)
                && g.seats[s].alive
                && skills::active(g, s, "jianzheng")
                && !g.hand(s).is_empty()
                && gear::within_attack_range(g, user, s, gear::attack_range(g, user) as usize)
        })
        .collect();
    for s in jianzheng {
        super::super::resolve::push_ask(
            g,
            "jianzheng",
            Stage::Choose,
            s,
            json!({ "step": "sha_targeted", "user": user, "targets": targets, "card": card }),
        );
        pushed = true;
    }
    // 专对：使用者与目标各自可拼点（每张【杀】每个时机一次）。
    let mut owners: Vec<(usize, &'static str)> = Vec::new();
    if skills::active(g, user, "zhuandui") && g.counter(&format!("zhuandui.once.{card}.user")) == 0 {
        owners.push((user, "user"));
    }
    for t in targets {
        if skills::active(g, *t, "zhuandui")
            && g.counter(&format!("zhuandui.once.{card}.target{t}")) == 0
        {
            owners.push((*t, "target"));
        }
    }
    // 铁骑（界马超）：使用者侧（指定目标后）询问。
    if g.seats.get(user).map(|s| s.alive).unwrap_or(false) && skills::active(g, user, "tieqi") {
        super::super::resolve::push_ask(
            g,
            "tieqi",
            Stage::Choose,
            user,
            json!({ "step": "sha_targeted", "role": "user", "user": user, "targets": targets, "card": card }),
        );
        pushed = true;
    }
    for (owner, role) in owners {
        super::super::resolve::push_ask(
            g,
            "zhuandui",
            Stage::Choose,
            owner,
            json!({ "step": "sha_targeted", "role": role, "user": user, "targets": targets, "card": card }),
        );
        pushed = true;
    }
    pushed
}

pub fn take_sha_targets(g: &mut Game, card: &str) -> Option<Vec<usize>> {
    let key = format!("sha.targets.{card}");
    let raw = g.mark(&key).map(|s| s.to_string())?;
    g.marks.remove(&key);
    serde_json::from_str::<Vec<usize>>(&raw).ok()
}

fn jianzheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jianzheng") {
        return Ok(Step::Done);
    }
    let user = common::seat_of(ask, "user").ok_or("谏征：缺使用者")?;
    let card = common::card_of(ask, "card").ok_or("谏征：缺牌")?;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("sha_targeted");
    match step {
        "sha_targeted" => match take_answer(g, "jianzheng") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("jianzheng");
                b = b.option("use", "谏征：将一张手牌置于牌堆顶，取消所有目标");
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "sha_targeted", "user": user, "card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "user": user, "card": card }),
            )),
            Some(o) => Err(format!("谏征：未知选项 {o}")),
        },
        "pick" => {
            let hand = g.hand(actor);
            match take_answer(g, "jianzheng") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("jianzheng");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("将【{}】置于牌堆顶", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick", "user": user, "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("谏征：只能用手牌".into());
                    }
                    let black = g.card_defs.get(&card).map(|c| !c.suit.is_red()).unwrap_or(false);
                    flow::to_deck_top(g, &id);
                    let new_targets: Vec<usize> = if black { vec![] } else { vec![actor] };
                    g.set_mark(&format!("sha.targets.{card}"), serde_json::to_string(&new_targets).unwrap_or_default());
                    Ok(Step::Finish(if black {
                        "谏征：取消所有目标".into()
                    } else {
                        "谏征：取消所有目标，秦宓成为目标".to_string()
                    }))
                }
                Some(o) => Err(format!("谏征：未知选项 {o}")),
            }
        }
        other => Err(format!("谏征：未知阶段 {other}")),
    }
}

fn zhuandui(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhuandui") {
        return Ok(Step::Done);
    }
    let card = common::card_of(ask, "card").ok_or("专对：缺牌")?;
    let user = common::seat_of(ask, "user").ok_or("专对：缺使用者")?;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("sha_targeted");
    let role = ask.data.get("role").and_then(|v| v.as_str()).unwrap_or("target").to_string();
    let other = if role == "user" {
        common::seats_of(ask, "targets").first().copied().unwrap_or(user)
    } else {
        user
    };
    match step {
        "sha_targeted" => {
            if other == actor || g.hand(actor).is_empty() || g.hand(other).is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "zhuandui") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("zhuandui");
                    b = b.option("use", format!("专对：与{}拼点", common::seat_name(g, other)));
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "sha_targeted", "role": role, "user": user, "other": other, "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => {
                    g.bump(&format!("zhuandui.once.{card}.{}", if role == "user" { "user".to_string() } else { format!("target{actor}") }));
                    pindian::start(g, actor, other, "专对");
                    Ok(Step::Next(
                        Stage::PindianInit,
                        json!({ "step": "wait", "role": role, "user": user, "other": other, "card": card }),
                    ))
                }
                Some(o) => Err(format!("专对：未知选项 {o}")),
            }
        }
        "wait" => {
            let Some(res) = g.take_pindian() else {
                return Ok(Step::Done);
            };
            if !res.initiator_won() {
                return Ok(Step::Done);
            }
            if role == "user" {
                super::super::card_play::declare_no_response(g, &card);
            } else {
                super::super::card_play::declare_intercept(g, &card, super::super::ShaIntercept::Immune);
            }
            Ok(Step::Finish("专对：拼点获胜".into()))
        }
        other => Err(format!("专对：未知阶段 {other}")),
    }
}

pub fn pindian_deck_option(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "tianbian") && !g.deck.is_empty()
}

pub fn pindian_rank(g: &Game, seat: usize, card: &str) -> u8 {
    let base = g.card_defs.get(card).map(|c| c.rank).unwrap_or(0);
    let heart = g.card_defs.get(card).map(|c| c.suit == Suit::Heart).unwrap_or(false);
    if skills::active(g, seat, "tianbian") && heart {
        13
    } else {
        base
    }
}

// ---------------------------------------------------------------------------
// 薛综 · 复难 / 诫训
// ---------------------------------------------------------------------------

fn funan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "funan") {
        return Ok(Step::Done);
    }
    let Some(response) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    // 这张响应牌对应的是哪张使用牌。
    let Some(used) = g.mark(&format!("responds.{response}")).map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if g.mark(&format!("used_by.{used}")).and_then(|s| s.parse::<usize>().ok()) != Some(actor) {
        return Ok(Step::Done);
    }
    if used.starts_with("v:") || response.starts_with("v:") {
        return Ok(Step::Done);
    }
    let up = g.counter(&format!("funan.plus.{actor}")) > 0;
    match take_answer(g, "funan") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("funan");
            b = b.option(
                "use",
                if up {
                    "复难plus：获得其响应时使用/打出的牌".to_string()
                } else {
                    "复难：令其获得你使用的牌（本回合不能使用/打出），你获得其响应牌".to_string()
                },
            );
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "used": used, "response": response }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let responder = common::seat_of(ask, "victim").unwrap_or(actor);
            flow::gain(g, actor, &response, actor, "复难");
            if !up {
                flow::gain(g, responder, &used, actor, "复难");
                // 响应者本回合不能使用/打出该牌（手牌冻结）。
                g.freeze_cards(responder, &[used.clone()], FreezeUntil::Turn, "复难");
            }
            Ok(Step::Finish("复难：交换/获得响应牌".into()))
        }
        Some(o) => Err(format!("复难：未知选项 {o}")),
    }
}

fn jiexun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jiexun") || g.turn != actor || g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands = common::alive_others(g, actor);
            match take_answer(g, "jiexun") {
                None => common::pick_seat(g, actor, "jiexun", "令", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "诫训：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("诫训：目标不合法".into());
                    }
                    // 场上方片牌（装备区 + 判定区）。
                    let x = g
                        .cards
                        .iter()
                        .filter(|(_, z)| matches!(z, Zone::Equip(..) | Zone::Judge(_)))
                        .filter(|(id, _)| g.card_defs.get(*id).map(|c| c.suit == Suit::Diamond).unwrap_or(false))
                        .count();
                    if x > 0 {
                        flow::draw(g, t, x);
                    }
                    let y = g.counter(&format!("jiexun.used.{actor}")) + 1;
                    g.bump(&format!("jiexun.used.{actor}"));
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "discard", "target": t, "left": y }),
                    ))
                }
                Some(o) => Err(format!("诫训：未知选项 {o}")),
            }
        }
        "discard" => {
            let t = common::seat_of(ask, "target").ok_or("诫训：缺目标")?;
            let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let cands = own_cards(g, t);
            if left == 0 || cands.is_empty() {
                let empty = own_cards(g, t).is_empty();
                if empty {
                    skills::revoke(g, actor, "jiexun");
                    g.bump(&format!("funan.plus.{actor}"));
                    return Ok(Step::Finish("诫训：其弃光所有牌，升级复难plus".into()));
                }
                return Ok(Step::Finish("诫训：结算完成".into()));
            }
            match take_answer(g, "jiexun") {
                None => {
                    let mut b = PendingBuilder::new(t, "pick_own").skill("jiexun");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】（还需 {} 张）", card_label(g, id), left));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "discard", "target": t, "left": left }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("诫训：只能弃置其自己的牌".into());
                    }
                    flow::discard(g, t, &id, "诫训");
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "discard", "target": t, "left": left - 1 }),
                    ))
                }
                Some(o) => Err(format!("诫训：未知选项 {o}")),
            }
        }
        other => Err(format!("诫训：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 蔡邕 · 辟撰 / 通博
// ---------------------------------------------------------------------------

fn bizhuan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "bizhuan") {
        return Ok(Step::Done);
    }
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let spade = g.card_defs.get(&card).map(|c| c.suit == Suit::Spade).unwrap_or(false);
    if !spade {
        return Ok(Step::Done);
    }
    // 只处理「自己使用」与「成为其他角色黑桃牌目标」。
    if ask.data.get("event").and_then(|v| v.as_str()) == Some("TargetChosen") {
        if ask.data.get("by").and_then(|v| v.as_u64()) == Some(actor as u64) {
            return Ok(Step::Done);
        }
    }
    if books(g, actor).len() >= 4 {
        return Ok(Step::Done);
    }
    match take_answer(g, "bizhuan") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("bizhuan");
            b = b.option("use", "辟撰：将牌堆顶一张牌置于武将牌上为「书」");
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let Some(top) = g.deck.pop() else {
                return Ok(Step::Done);
            };
            g.cards.insert(top.clone(), Zone::Mark(actor, BOOK.to_string()));
            g.reveal(&[top.clone()], "辟撰");
            Ok(Step::Finish("辟撰：获得一张「书」".into()))
        }
        Some(o) => Err(format!("辟撰：未知选项 {o}")),
    }
}

fn tongbo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "tongbo") || g.turn != actor || g.phase != Phase::Draw {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    match step {
        "ask" => {
            if books(g, actor).is_empty() || own_cards(g, actor).is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "tongbo") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("tongbo");
                    b = b.option("use", "通博：用牌替换「书」");
                    Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(Stage::PickOwn, json!({ "step": "mine" }))),
                Some(o) => Err(format!("通博：未知选项 {o}")),
            }
        }
        "mine" => {
            let cands = own_cards(g, actor);
            if cands.is_empty() || books(g, actor).is_empty() {
                return Ok(tongbo_finish(g, actor));
            }
            match take_answer(g, "tongbo") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("tongbo");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("用【{}】替换一张「书」", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape("结束替换")
                            .data(json!({ "step": "mine" }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(tongbo_finish(g, actor)),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("通博：只能用自己的牌替换".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "book", "mine": id }),
                    ))
                }
                Some(o) => Err(format!("通博：未知选项 {o}")),
            }
        }
        "book" => {
            let mine = common::card_of(ask, "mine").ok_or("通博：缺己方牌")?;
            let bs = books(g, actor);
            if bs.is_empty() {
                return Ok(tongbo_finish(g, actor));
            }
            match take_answer(g, "tongbo") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("tongbo");
                    for id in &bs {
                        b = b.option(format!("card:{id}"), format!("替换「书」【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "book", "mine": mine }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let book = a[5..].to_string();
                    if !bs.iter().any(|c| *c == book) {
                        return Err("通博：只能替换「书」".into());
                    }
                    let was_equip = matches!(g.cards.get(&mine), Some(Zone::Equip(..)));
                    // 书 → 手牌（获得语义）
                    flow::gain(g, actor, &book, actor, "通博");
                    // 己方牌 → 书（移出游戏：离开原区域）
                    let mut lost = super::super::event::Event::card(
                        EventKind::CardLost,
                        Some(actor),
                        mine.clone(),
                        Some("通博".to_string()),
                    );
                    lost.by = Some(actor);
                    lost.from = Some(actor);
                    let e = lost.snapshot(g);
                    g.push_event(e);
                    if was_equip {
                        flow::leave_equip(g, actor, &mine, "通博");
                    }
                    g.cards.insert(mine.clone(), Zone::Mark(actor, BOOK.to_string()));
                    g.reveal(&[mine.clone()], "通博");
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "mine" })))
                }
                Some(o) => Err(format!("通博：未知选项 {o}")),
            }
        }
        "give" => _tongbo_give(g, ask),
        other => Err(format!("通博：未知阶段 {other}")),
    }
}

fn tongbo_finish(g: &mut Game, actor: usize) -> Step {
    let bs = books(g, actor);
    let suits: std::collections::BTreeSet<String> = bs
        .iter()
        .filter_map(|id| g.card_defs.get(id).map(|c| format!("{:?}", c.suit)))
        .collect();
    if suits.len() < 4 || bs.is_empty() {
        return Step::Finish("通博：替换结束".into());
    }
    let cands = common::alive_others(g, actor);
    if cands.is_empty() {
        return Step::Finish("通博：无其他角色".into());
    }
    let mut b = PendingBuilder::new(actor, "target").skill("tongbo");
    for s in &cands {
        b = b.option(format!("t:{s}"), format!("将所有「书」交给{}", common::seat_name(g, *s)));
    }
    let data = json!({ "step": "give", "books": bs });
    Step::NeedWith(
        Stage::Target,
        data.clone(),
        b.escape("不交出").data(data).build(),
    )
}

// `tongbo` 的 give 阶段（放在同一 handler 末尾以免再拆文件）。
fn _tongbo_give(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let bs = common::cards_of(ask, "books");
    match take_answer(g, "tongbo") {
        None => Ok(Step::Done),
        Some(a) if a == "pass" => Ok(Step::Finish("通博：不交出".into())),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "通博：目标不合法")?;
            for id in bs {
                flow::gain(g, t, &id, actor, "通博");
            }
            Ok(Step::Finish("通博：所有「书」交给其".into()))
        }
        Some(o) => Err(format!("通博：未知选项 {o}")),
    }
}
