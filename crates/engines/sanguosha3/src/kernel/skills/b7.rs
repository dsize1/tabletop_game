//! B7 技能（一将成名·Y6：7 将 / 12 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B7.json` 的 `agreed`（权威）。
//! 触发类先询问「发动/放弃」，选牌/选目标用 `escape_mandatory`（不代选）。

use serde_json::json;

use super::super::cards::{self, Category};
use super::super::event::{Event, EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{
    DamageKind, DamageSpec, FreezeUntil, Game, Phase, SourceKind, Zone,
};
use super::common::{self, card_label, def_of, own_cards, phase_key, turn_key};
use super::{Face, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

const KUANGBI_MARK: &str = "kuangbi";

pub static SKILLS: &[SkillDef] = &[
    SkillDef {
        code: "duliang",
        name: "督粮",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "fulin",
        name: "腹鳞",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Discard),
            subscriptions: &[],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "kuangbi",
        name: "匡弼",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            // 下个回合开始：获得所有「匡弼」牌，然后放置者摸等量张牌。
            Face {
                window: Window::OwnTurn,
                subscriptions: &[Subscription {
                    event: EventKind::TurnStart,
                    subject: Subject::Any,
                }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    SkillDef {
        code: "zhige",
        name: "止戈",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "zongzuo",
        name: "宗祚",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[super::Hook::GameStart],
            can_active: false,
        }],
    },
    SkillDef {
        code: "jishe",
        name: "极奢",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            Face {
                window: Window::OwnPhase(Phase::End),
                subscriptions: &[Subscription {
                    event: EventKind::PhaseStart,
                    subject: Subject::Any,
                }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    SkillDef {
        code: "lianhuo",
        name: "链祸",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "guizao",
        name: "瑰藻",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Discard),
                subscriptions: &[Subscription {
                    event: EventKind::CardDiscarded,
                    subject: Subject::By,
                }],
                hooks: &[],
                can_active: false,
            },
            Face {
                window: Window::OwnPhase(Phase::Discard),
                subscriptions: &[Subscription {
                    event: EventKind::PhaseEnd,
                    subject: Subject::Any,
                }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    SkillDef {
        code: "jiyu",
        name: "讥谀",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "qinqing",
        name: "寝情",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[Subscription {
                event: EventKind::PhaseStart,
                subject: Subject::Any,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "huisheng",
        name: "贿生",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[super::Hook::DamageBefore],
            can_active: false,
        }],
    },
    SkillDef {
        code: "taoluan",
        name: "滔乱",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            // 牌结算完成后收口（令一名其他角色二选一）。
            Face {
                window: Window::AnyTime,
                subscriptions: &[Subscription {
                    event: EventKind::CardUsed,
                    subject: Subject::By,
                }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "duliang" => duliang,
        "kuangbi" => kuangbi,
        "zhige" => zhige,
        "zongzuo" => zongzuo,
        "jishe" => jishe,
        "guizao" => guizao,
        "jiyu" => jiyu,
        "qinqing" => qinqing,
        "huisheng" => huisheng,
        "taoluan" => taoluan,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "duliang" => can_duliang,
        "kuangbi" => can_kuangbi,
        "zhige" => can_zhige,
        "jishe" => can_jishe,
        "jiyu" => can_jiyu,
        "taoluan" => can_taoluan,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 数值/回合/伤害钩子的**唯一出口**
// ---------------------------------------------------------------------------

/// 腹鳞：弃牌阶段，本回合获得的牌不计入手牌上限（+张数）。
/// 极奢：每次发动本回合手牌上限 -1。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    let mut n = 0;
    if skills::active(g, seat, "fulin") && g.phase == Phase::Discard {
        n += g.counter(&turn_key(seat, "gained_cards"));
    }
    if skills::active(g, seat, "jishe") {
        n -= g.counter(&turn_key(seat, "jishe"));
    }
    n
}

/// 督粮②：目标下回合摸牌阶段多摸一张（跳过摸牌阶段则保留到真正摸牌那次）。
pub fn draw_bonus(g: &mut Game, seat: usize) -> usize {
    let key = format!("duliang.extra.{seat}");
    if g.counter(&key) > 0 {
        g.set_counter(&key, 0);
        1
    } else {
        0
    }
}

/// 链祸：火焰伤害 + 自己横置（传导起点）→ +1。
pub fn lianhuo_bonus(g: &Game, spec: &DamageSpec) -> i32 {
    if spec.kind != DamageKind::Fire {
        return 0;
    }
    let chained = g.seats.get(spec.target).map(|s| s.chained).unwrap_or(false);
    if !chained {
        return 0;
    }
    if skills::active(g, spec.target, "lianhuo") {
        1
    } else {
        0
    }
}

/// 贿生：受到其他角色造成的伤害时可发动（本回合未对该来源发动过 opt1）。
pub fn should_ask_damage_before_victim(g: &Game, from: usize, target: usize) -> bool {
    if !skills::active(g, target, "huisheng") || from == target {
        return false;
    }
    if !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return false;
    }
    if own_cards(g, target).is_empty() {
        return false;
    }
    g.counter(&huisheng_used_key(target, from)) == 0
}

fn huisheng_used_key(target: usize, from: usize) -> String {
    format!("huisheng.used.{target}.{from}")
}

/// 宗祚：开局按势力数 +体力上限并回复 1 体力（`Hook::GameStart` 同步结算）。
pub fn on_game_start(g: &mut Game, seat: usize) {
    if !skills::active(g, seat, "zongzuo") {
        return;
    }
    let mut seen = std::collections::BTreeSet::new();
    for s in &g.seats {
        if s.alive && !s.kingdom.is_empty() {
            seen.insert(s.kingdom.clone());
        }
    }
    let n = seen.len() as i32;
    if n <= 0 {
        return;
    }
    if let Some(s) = g.seats.get_mut(seat) {
        s.max_hp += n;
        s.hp = (s.hp + n).min(s.max_hp);
    }
    for _ in 0..n {
        flow::heal(g, seat, 1, "宗祚");
    }
    g.set_counter(&format!("zongzuo.kingdoms.{seat}"), n);
}

/// 宗祚：某势力最后一名角色死亡 → 每个宗祚持有者减 1 点体力上限。
pub fn on_death(g: &mut Game, dead: usize) {
    let kingdom = g
        .seats
        .get(dead)
        .map(|s| s.kingdom.clone())
        .unwrap_or_default();
    if kingdom.is_empty() {
        return;
    }
    let still_alive = g
        .seats
        .iter()
        .any(|s| s.alive && s.kingdom == kingdom);
    if still_alive {
        return;
    }
    let holders: Vec<usize> = (0..g.seats.len())
        .filter(|&s| g.seats[s].alive && skills::active(g, s, "zongzuo"))
        .collect();
    for h in holders {
        flow::lose_max_hp(g, h, 1, "宗祚");
    }
}

// ---------------------------------------------------------------------------
// 李严 · 督粮
// ---------------------------------------------------------------------------

fn duliang_targets(g: &Game, seat: usize) -> Vec<usize> {
    common::alive_others(g, seat)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty())
        .collect()
}

fn can_duliang(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "duliang")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "duliang")) == 0
        && !duliang_targets(g, seat).is_empty()
}

fn duliang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "duliang") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands = duliang_targets(g, actor);
            if cands.is_empty() || g.counter(&phase_key(actor, "duliang")) > 0 {
                return Ok(Step::Done);
            }
            match take_answer(g, "duliang") {
                None => common::pick_seat(
                    g,
                    actor,
                    "duliang",
                    "获得",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "督粮：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("督粮：目标不合法".into());
                    }
                    // 随机获得其一张手牌。
                    let hand = g.hand(t);
                    let idx = g.rand_below(hand.len());
                    if let Some(card) = hand.get(idx).cloned() {
                        flow::gain(g, actor, &card, actor, "督粮");
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "opt", "target": t }),
                    ))
                }
                Some(o) => Err(format!("督粮：未知选项 {o}")),
            }
        }
        "opt" => {
            let t = common::seat_of(ask, "target").ok_or("督粮：缺目标")?;
            match take_answer(g, "duliang") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("duliang");
                    b = b.option("opt1", "令其观看牌堆顶两张牌，获得其中的基本牌");
                    b = b.option("opt2", "令其下回合摸牌阶段多摸一张牌");
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "opt", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" => {
                    duliang_view_top(g, t);
                    g.bump(&phase_key(actor, "duliang"));
                    Ok(Step::Finish("督粮：其观看牌堆顶并获得基本牌".into()))
                }
                Some(a) if a == "opt2" => {
                    g.bump(&format!("duliang.extra.{t}"));
                    g.bump(&phase_key(actor, "duliang"));
                    Ok(Step::Finish("督粮：其下回合摸牌阶段多摸一张".into()))
                }
                Some(o) => Err(format!("督粮：未知选项 {o}")),
            }
        }
        other => Err(format!("督粮：未知阶段 {other}")),
    }
}

/// 观看牌堆顶两张（私密：只给该角色看，通过其本人 pending 文案实现），基本牌获得，其余放回牌堆顶。
fn duliang_view_top(g: &mut Game, target: usize) {
    let mut top: Vec<String> = Vec::new();
    for _ in 0..2 {
        if let Some(id) = g.deck.pop() {
            top.push(id);
        }
    }
    // 非基本牌按原顺序放回牌堆顶（后 push 的在顶）。
    let mut keep: Vec<String> = Vec::new();
    for id in top {
        let basic = def_of(g, &id)
            .map(|d| d.category == Category::Basic)
            .unwrap_or(false);
        if basic {
            g.deck.retain(|c| *c != id);
            flow::gain(g, target, &id, target, "督粮");
        } else {
            keep.push(id);
        }
    }
    for id in keep.into_iter().rev() {
        g.deck.push(id);
    }
}

// ---------------------------------------------------------------------------
// 孙登 · 匡弼
// ---------------------------------------------------------------------------

fn can_kuangbi(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "kuangbi")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "kuangbi")) == 0
        && common::alive_others(g, seat)
            .into_iter()
            .any(|t| !own_cards(g, t).is_empty())
}

fn kuangbi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "kuangbi") {
        return Ok(Step::Done);
    }
    // 回合开始：获得所有「匡弼」牌并令放置者摸等量张。
    if ask.data.get("event").and_then(|v| v.as_str()) == Some("TurnStart") {
        kuangbi_collect(g, actor);
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            if !can_kuangbi(g, actor) {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| !own_cards(g, *t).is_empty())
                .collect();
            match take_answer(g, "kuangbi") {
                None => common::pick_seat(
                    g,
                    actor,
                    "kuangbi",
                    "令",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "匡弼：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("匡弼：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "place", "target": t, "placed": [] }),
                    ))
                }
                Some(o) => Err(format!("匡弼：未知选项 {o}")),
            }
        }
        "place" => {
            let t = common::seat_of(ask, "target").ok_or("匡弼：缺目标")?;
            let placed = common::seats_of(ask, "placed");
            let cands = own_cards(g, t);
            match take_answer(g, "kuangbi") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Finish("匡弼：其无牌可置（视为未发动）".into()));
                    }
                    let mut b = PendingBuilder::new(t, "pick_own").skill("kuangbi");
                    for id in &cands {
                        b = b.option(
                            format!("card:{id}"),
                            format!(
                                "将【{}】置于孙登武将牌上（已置 {} 张）",
                                card_label(g, id),
                                placed.len()
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("不放置（结束）")
                            .data(json!({ "step": "place", "target": t, "placed": placed }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    if placed.is_empty() {
                        Ok(Step::Finish("匡弼：未放置任何牌（视为未发动）".into()))
                    } else {
                        g.bump(&phase_key(actor, "kuangbi"));
                        Ok(Step::Finish(format!("匡弼：{} 张牌置于武将牌上", placed.len())))
                    }
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    let Some(owner) = g.cards.get(&id).and_then(|z| z.seat()) else {
                        return Err("匡弼：牌不存在".into());
                    };
                    if owner != t || !cands.iter().any(|c| *c == id) {
                        return Err("匡弼：只能放置该角色自己的牌".into());
                    }
                    g.cards.insert(id.clone(), Zone::Mark(actor, KUANGBI_MARK.to_string()));
                    g.set_mark(&format!("kuangbi.src.{id}"), t.to_string());
                    let mut ev = Event::card(
                        EventKind::CardOutOfGame,
                        Some(t),
                        id.clone(),
                        Some("匡弼".to_string()),
                    );
                    ev.by = Some(actor);
                    let e = ev.snapshot(g);
                    g.push_event(e);
                    let mut next_placed = placed.clone();
                    next_placed.push(t);
                    if next_placed.len() >= 3 {
                        g.bump(&phase_key(actor, "kuangbi"));
                        Ok(Step::Finish("匡弼：已放置三张牌".into()))
                    } else {
                        Ok(Step::Next(
                            Stage::PickOther,
                            json!({ "step": "place", "target": t, "placed": next_placed }),
                        ))
                    }
                }
                Some(o) => Err(format!("匡弼：未知选项 {o}")),
            }
        }
        other => Err(format!("匡弼：未知阶段 {other}")),
    }
}

fn kuangbi_collect(g: &mut Game, actor: usize) {
    let ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, m) if *s == actor && m == KUANGBI_MARK))
        .map(|(id, _)| id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    let mut per_src: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for id in ids {
        let src = g
            .mark(&format!("kuangbi.src.{id}"))
            .and_then(|s| s.parse::<usize>().ok());
        flow::gain(g, actor, &id, actor, "匡弼");
        let mut ev = Event::card(
            EventKind::CardIntoGame,
            Some(actor),
            id.clone(),
            Some("匡弼".to_string()),
        );
        ev.by = Some(actor);
        let e = ev.snapshot(g);
        g.push_event(e);
        g.marks.remove(&format!("kuangbi.src.{id}"));
        if let Some(src) = src {
            *per_src.entry(src).or_default() += 1;
        }
    }
    for (src, n) in per_src {
        if g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
            flow::draw(g, src, n);
        }
    }
}

// ---------------------------------------------------------------------------
// 刘虞 · 止戈
// ---------------------------------------------------------------------------

fn zhige_targets(g: &Game, seat: usize) -> Vec<usize> {
    common::alive_others(g, seat)
        .into_iter()
        .filter(|t| {
            gear::within_attack_range(g, *t, seat, gear::attack_range(g, *t) as usize)
                && (g.hand(*t)
                    .iter()
                    .any(|id| def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
                    || !common::equip_cards(g, *t).is_empty())
        })
        .collect()
}

fn can_zhige(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "zhige")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "zhige")) == 0
        && g.hand_size(seat) > g.seats[seat].hp.max(0) as usize
        && !zhige_targets(g, seat).is_empty()
}

fn zhige(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhige") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            if !can_zhige(g, actor) {
                return Ok(Step::Done);
            }
            let cands = zhige_targets(g, actor);
            match take_answer(g, "zhige") {
                None => common::pick_seat(
                    g,
                    actor,
                    "zhige",
                    "令",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "止戈：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("止戈：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "opt", "target": t }),
                    ))
                }
                Some(o) => Err(format!("止戈：未知选项 {o}")),
            }
        }
        "opt" => {
            let t = common::seat_of(ask, "target").ok_or("止戈：缺目标")?;
            let has_sha = g
                .hand(t)
                .iter()
                .any(|id| def_of(g, id).map(|d| d.tags.sha).unwrap_or(false));
            let equips = common::equip_cards(g, t);
            match take_answer(g, "zhige") {
                None => {
                    let mut b = PendingBuilder::new(t, "choose").skill("zhige");
                    if has_sha {
                        b = b.option("opt1", "使用一张【杀】对刘虞");
                    }
                    if !equips.is_empty() {
                        b = b.option("opt2", "交给刘虞一张装备区的牌");
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "opt", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" && has_sha => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick_sha", "target": t }),
                )),
                Some(a) if a == "opt2" && !equips.is_empty() => Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "pick_equip", "target": t }),
                )),
                Some(o) => Err(format!("止戈：未知选项 {o}")),
            }
        }
        "pick_sha" => {
            let t = common::seat_of(ask, "target").ok_or("止戈：缺目标")?;
            let shas: Vec<String> = g
                .hand(t)
                .into_iter()
                .filter(|id| def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
                .collect();
            match take_answer(g, "zhige") {
                None => {
                    if shas.is_empty() {
                        return Ok(Step::Finish("止戈：其已无【杀】".into()));
                    }
                    let mut b = PendingBuilder::new(t, "pick_own").skill("zhige");
                    for id in &shas {
                        b = b.option(
                            format!("card:{id}"),
                            format!("对刘虞使用【{}】", card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick_sha", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !shas.iter().any(|c| *c == id) {
                        return Err("止戈：只能使用其手牌中的【杀】".into());
                    }
                    g.bump(&phase_key(actor, "zhige"));
                    super::super::card_play::use_on_targets(g, t, &id, &[actor])?;
                    Ok(Step::Finish("止戈：其对你使用【杀】".into()))
                }
                Some(o) => Err(format!("止戈：未知选项 {o}")),
            }
        }
        "pick_equip" => {
            let t = common::seat_of(ask, "target").ok_or("止戈：缺目标")?;
            let equips = common::equip_cards(g, t);
            match take_answer(g, "zhige") {
                None => {
                    if equips.is_empty() {
                        return Ok(Step::Finish("止戈：其已无装备".into()));
                    }
                    let mut b = PendingBuilder::new(t, "pick_other").skill("zhige");
                    for id in &equips {
                        b = b.option(
                            format!("card:{id}"),
                            format!("将装备【{}】交给刘虞", card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick_equip", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !equips.iter().any(|c| *c == id) {
                        return Err("止戈：只能交出其装备区的牌".into());
                    }
                    flow::gain(g, actor, &id, t, "止戈");
                    g.bump(&phase_key(actor, "zhige"));
                    Ok(Step::Finish("止戈：其交给刘虞一张装备牌".into()))
                }
                Some(o) => Err(format!("止戈：未知选项 {o}")),
            }
        }
        other => Err(format!("止戈：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 刘虞 · 宗祚（锁定，开局 + 势力消亡）
// ---------------------------------------------------------------------------

fn zongzuo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let hook = ask.data.get("hook").and_then(|v| v.as_str()).unwrap_or("");
    if hook == "game_start" {
        on_game_start(g, actor);
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 岑昏 · 极奢（出牌阶段可反复 / 结束阶段无手牌则横置）
// ---------------------------------------------------------------------------

fn can_jishe(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "jishe")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.hand_limit(seat) > 0
}

fn jishe(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jishe") {
        return Ok(Step::Done);
    }
    let end_phase = g.turn == actor && g.phase == Phase::End;
    if !end_phase {
        // 出牌阶段主动形态。
        if !can_jishe(g, actor) {
            return Ok(Step::Done);
        }
        flow::draw(g, actor, 1);
        g.bump(&turn_key(actor, "jishe"));
        return Ok(Step::Finish("极奢：摸一张，本回合手牌上限 -1".into()));
    }
    // 结束阶段：若没有手牌，每点体力令一名角色进入连环状态。
    if !g.hand(actor).is_empty() {
        return Ok(Step::Done);
    }
    let left = ask
        .data
        .get("left")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or_else(|| g.seats.get(actor).map(|s| s.hp.max(0) as usize).unwrap_or(0));
    if left == 0 {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = common::alive_seats(g)
        .into_iter()
        .filter(|s| !g.seats[*s].chained)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "jishe") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("jishe");
            for s in &cands {
                b = b.option(
                    format!("t:{s}"),
                    format!("令{}进入连环状态（剩余 {} 次）", common::seat_name(g, *s), left),
                );
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "left": left }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "极奢：目标不合法")?;
            if !cands.contains(&t) {
                return Err("极奢：目标不合法".into());
            }
            flow::chain_enter(g, t, "极奢");
            let next = left - 1;
            if next == 0 {
                Ok(Step::Finish("极奢：结算完成".into()))
            } else {
                Ok(Step::Next(Stage::Target, json!({ "left": next })))
            }
        }
        Some(o) => Err(format!("极奢：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 孙资刘放 · 瑰藻
// ---------------------------------------------------------------------------

fn guizao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "guizao") {
        return Ok(Step::Done);
    }
    let key = format!("guizao.suits.{actor}");
    if let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) {
        // 记花色（弃牌阶段的每次弃置；含该阶段内被技能弃置的牌）。
        if let Some(suit) = g.card_defs.get(card).map(|c| c.suit.label()) {
            let mut suits: Vec<String> = g
                .mark(&key)
                .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
                .unwrap_or_default();
            suits.push(suit.to_string());
            g.set_mark(&key, suits.join(","));
        }
        return Ok(Step::Done);
    }
    // PhaseEnd：结算。
    let suits: Vec<String> = g
        .mark(&key)
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
        .unwrap_or_default();
    if suits.len() < 2 {
        return Ok(Step::Done);
    }
    let distinct: std::collections::BTreeSet<&String> = suits.iter().collect();
    if distinct.len() != suits.len() {
        return Ok(Step::Done);
    }
    match take_answer(g, "guizao") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("guizao");
            b = b.option("heal", "回复 1 点体力");
            b = b.option("draw", "摸一张牌");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "choose" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "heal" => {
            g.marks.remove(&key);
            flow::heal(g, actor, 1, "瑰藻");
            Ok(Step::Finish("瑰藻：回复 1 点体力".into()))
        }
        Some(a) if a == "draw" => {
            g.marks.remove(&key);
            flow::draw(g, actor, 1);
            Ok(Step::Finish("瑰藻：摸一张牌".into()))
        }
        Some(o) => Err(format!("瑰藻：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 孙资刘放 · 讥谀
// ---------------------------------------------------------------------------

fn can_jiyu(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "jiyu")
        && g.turn == seat
        && g.phase == Phase::Play
        && !super::super::card_play::usable_hand(g, seat).is_empty()
        && common::alive_others(g, seat).into_iter().any(|t| {
            !g.hand(t).is_empty() && g.counter(&turn_key(seat, &format!("jiyu.{t}"))) == 0
        })
}

fn jiyu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jiyu") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            if !can_jiyu(g, actor) {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| {
                    !g.hand(*t).is_empty()
                        && g.counter(&turn_key(actor, &format!("jiyu.{t}"))) == 0
                })
                .collect();
            match take_answer(g, "jiyu") {
                None => common::pick_seat(
                    g,
                    actor,
                    "jiyu",
                    "令",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "讥谀：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("讥谀：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "pick", "target": t }),
                    ))
                }
                Some(o) => Err(format!("讥谀：未知选项 {o}")),
            }
        }
        "pick" => {
            let t = common::seat_of(ask, "target").ok_or("讥谀：缺目标")?;
            let hand = g.hand(t);
            match take_answer(g, "jiyu") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(t, "pick_own").skill("jiyu");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick", "target": t }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("讥谀：只能弃置目标自己的手牌".into());
                    }
                    let spade = g
                        .card_defs
                        .get(&id)
                        .map(|c| c.suit == super::super::Suit::Spade)
                        .unwrap_or(false);
                    flow::discard(g, t, &id, "讥谀");
                    // 本回合不能使用与之相同花色的牌（手牌冻结）。
                    if let Some(suit) = g.card_defs.get(&id).map(|c| c.suit) {
                        let frozen: Vec<String> = g
                            .hand(actor)
                            .into_iter()
                            .filter(|c| g.card_defs.get(c).map(|x| x.suit == suit).unwrap_or(false))
                            .collect();
                        g.freeze_cards(actor, &frozen, FreezeUntil::Turn, "讥谀");
                    }
                    g.bump(&turn_key(actor, &format!("jiyu.{t}")));
                    if spade {
                        flow::lose_hp(g, t, 1, "讥谀");
                        skills::flip_seat(g, actor);
                        Ok(Step::Finish("讥谀：黑桃，其失去 1 点体力，你翻面".into()))
                    } else {
                        Ok(Step::Finish("讥谀：其弃置一张手牌".into()))
                    }
                }
                Some(o) => Err(format!("讥谀：未知选项 {o}")),
            }
        }
        other => Err(format!("讥谀：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 黄皓 · 寝情
// ---------------------------------------------------------------------------

fn qinqing(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "qinqing") {
        return Ok(Step::Done);
    }
    let Some(lord) = g.current_lord() else {
        return Ok(Step::Done);
    };
    let chosen = common::seats_of(ask, "chosen");
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    match step {
        "pick" => {
            let cands: Vec<usize> = common::alive_seats(g)
                .into_iter()
                .filter(|s| {
                    *s != actor
                        && !chosen.contains(s)
                        && gear::within_attack_range(g, *s, lord, gear::attack_range(g, *s) as usize)
                        && !own_cards(g, *s).is_empty()
                })
                .collect();
            match take_answer(g, "qinqing") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("qinqing");
                    for s in &cands {
                        b = b.option(
                            format!("t:{s}"),
                            format!("弃置{}一张牌并令其摸一张", common::seat_name(g, *s)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("不发动/结束")
                            .data(json!({ "step": "pick", "chosen": chosen }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "寝情：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("寝情：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "card", "chosen": chosen, "target": t }),
                    ))
                }
                Some(o) => Err(format!("寝情：未知选项 {o}")),
            }
        }
        "card" => {
            let t = common::seat_of(ask, "target").ok_or("寝情：缺目标")?;
            let equips = common::equip_cards(g, t);
            let has_hand = !g.hand(t).is_empty();
            match take_answer(g, "qinqing") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("qinqing");
                    if has_hand {
                        b = b.option("random_hand", "弃置其一张手牌（随机）");
                    }
                    for id in &equips {
                        b = b.option(
                            format!("card:{id}"),
                            format!("弃置其装备【{}】", card_label(g, id)),
                        );
                    }
                    if !has_hand && equips.is_empty() {
                        return Ok(Step::Done);
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选）")
                            .data(json!({ "step": "card", "chosen": chosen, "target": t }))
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
                    flow::discard(g, t, &id, "寝情");
                    Ok(qinqing_after(g, actor, lord, t, chosen))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !equips.iter().any(|c| *c == id) {
                        return Err("寝情：只能指定其装备区的牌".into());
                    }
                    flow::discard(g, t, &id, "寝情");
                    Ok(qinqing_after(g, actor, lord, t, chosen))
                }
                Some(o) => Err(format!("寝情：未知选项 {o}")),
            }
        }
        other => Err(format!("寝情：未知阶段 {other}")),
    }
}

fn qinqing_after(g: &mut Game, actor: usize, lord: usize, t: usize, mut chosen: Vec<usize>) -> Step {
    flow::draw(g, t, 1);
    if g.hand_size(t) > g.hand_size(lord) {
        flow::draw(g, actor, 1);
    }
    chosen.push(t);
    Step::Next(
        Stage::Target,
        json!({ "step": "pick", "chosen": chosen }),
    )
}

// ---------------------------------------------------------------------------
// 黄皓 · 贿生（受到其他角色伤害时：私密展示任意张 → 来源二选一）
// ---------------------------------------------------------------------------

fn huisheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let victim = ask.actor;
    let Some(from) = common::seat_of(ask, "from") else {
        return Ok(Step::Done);
    };
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("normal");
    let source = ask.data.get("source").and_then(|v| v.as_str()).unwrap_or("skill");
    let ctx = json!({ "from": from, "amount": amount, "kind": kind, "source": source });
    let spec_of = |kind: &str, source: &str| DamageSpec {
        from: Some(from),
        target: victim,
        amount,
        kind: match kind {
            "fire" => DamageKind::Fire,
            "thunder" => DamageKind::Thunder,
            _ => DamageKind::Normal,
        },
        source: match source {
            "slash" => SourceKind::Slash,
            "nanman" => SourceKind::Nanman,
            "wanjian" => SourceKind::Wanjian,
            "duel" => SourceKind::Duel,
            "skill" => SourceKind::Skill,
            _ => SourceKind::Other,
        },
    };
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    match step {
        "ask" => {
            if !should_ask_damage_before_victim(g, from, victim) {
                let spec = spec_of(kind, source);
                let armor = gear::armor_of(g, victim);
                let _ = flow::damage_apply(g, spec, armor, 0);
                return Ok(Step::Done);
            }
            match take_answer(g, "huisheng") {
                None => {
                    let mut b = PendingBuilder::new(victim, "choose").skill("huisheng");
                    b = b.option("use", "贿生：展示任意张牌，令伤害来源二选一");
                    Ok(Step::Need(
                        b.escape("不发动（承受伤害）")
                            .data(merge(&ctx, &json!({ "step": "ask" })))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    let spec = spec_of(kind, source);
                    let armor = gear::armor_of(g, victim);
                    let _ = flow::damage_apply(g, spec, armor, 0);
                    Ok(Step::Finish("贿生：未发动".into()))
                }
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::PickOwn,
                    merge(
                        &ctx,
                        &json!({ "step": "show", "shown": Vec::<String>::new() }),
                    ),
                )),
                Some(o) => Err(format!("贿生：未知选项 {o}")),
            }
        }
        "show" => {
            let shown = common::cards_of(ask, "shown");
            let cands: Vec<String> = own_cards(g, victim)
                .into_iter()
                .filter(|id| !shown.contains(id))
                .collect();
            match take_answer(g, "huisheng") {
                None => {
                    let mut b = PendingBuilder::new(victim, "pick_own").skill("huisheng");
                    for id in &cands {
                        b = b.option(
                            format!("card:{id}"),
                            format!("展示【{}】（已选 {} 张）", card_label(g, id), shown.len()),
                        );
                    }
                    let escape = if shown.is_empty() {
                        "取消".to_string()
                    } else {
                        "完成展示".to_string()
                    };
                    Ok(Step::Need(
                        b.escape(escape)
                            .data(merge(
                                &ctx,
                                &json!({ "step": "show", "shown": shown }),
                            ))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    if shown.is_empty() {
                        let spec = spec_of(kind, source);
                        let armor = gear::armor_of(g, victim);
                        let _ = flow::damage_apply(g, spec, armor, 0);
                        return Ok(Step::Finish("贿生：未展示牌".into()));
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        merge(
                            &ctx,
                            &json!({ "step": "source", "shown": shown }),
                        ),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("贿生：只能展示自己的牌".into());
                    }
                    let mut next = shown.clone();
                    next.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        merge(
                            &ctx,
                            &json!({ "step": "show", "shown": next }),
                        ),
                    ))
                }
                Some(o) => Err(format!("贿生：未知选项 {o}")),
            }
        }
        "source" => {
            let shown = common::cards_of(ask, "shown");
            let can_discard = g.hand_size(from) >= shown.len();
            match take_answer(g, "huisheng") {
                None => {
                    let mut b = PendingBuilder::new(from, "choose").skill("huisheng");
                    if !shown.is_empty() {
                        b = b.option("opt1", "获得其中一张，防止此伤害");
                    }
                    if can_discard {
                        b = b.option(
                            "opt2",
                            format!("弃置等量（{}）张牌，伤害照常结算", shown.len()),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(merge(
                                &ctx,
                                &json!({ "step": "source", "shown": shown }),
                            ))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" => Ok(Step::Next(
                    Stage::PickOther,
                    merge(
                        &ctx,
                        &json!({ "step": "gain", "shown": shown }),
                    ),
                )),
                Some(a) if a == "opt2" => Ok(Step::Next(
                    Stage::PickOwn,
                    merge(
                        &ctx,
                        &json!({ "step": "discard", "shown": shown, "picked": Vec::<String>::new() }),
                    ),
                )),
                Some(o) => Err(format!("贿生：未知选项 {o}")),
            }
        }
        "gain" => {
            let shown = common::cards_of(ask, "shown");
            match take_answer(g, "huisheng") {
                None => {
                    if shown.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(from, "pick_other").skill("huisheng");
                    for id in &shown {
                        b = b.option(
                            format!("card:{id}"),
                            format!("获得展示的【{}】", card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(merge(&ctx, &json!({ "step": "gain", "shown": shown })))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !shown.iter().any(|c| *c == id) {
                        return Err("贿生：只能获得展示的牌".into());
                    }
                    flow::gain(g, from, &id, victim, "贿生");
                    g.bump(&huisheng_used_key(victim, from));
                    Ok(Step::Finish("贿生：其获得一张牌，伤害被防止".into()))
                }
                Some(o) => Err(format!("贿生：未知选项 {o}")),
            }
        }
        "discard" => {
            let shown = common::cards_of(ask, "shown");
            let picked = common::cards_of(ask, "picked");
            if picked.len() >= shown.len() {
                let spec = spec_of(kind, source);
                let armor = gear::armor_of(g, victim);
                let _ = flow::damage_apply(g, spec, armor, 0);
                return Ok(Step::Finish("贿生：其弃置等量张牌，伤害照常结算".into()));
            }
            let cands: Vec<String> = g
                .hand(from)
                .into_iter()
                .filter(|id| !picked.contains(id))
                .collect();
            match take_answer(g, "huisheng") {
                None => {
                    if cands.is_empty() {
                        let spec = spec_of(kind, source);
                        let armor = gear::armor_of(g, victim);
                        let _ = flow::damage_apply(g, spec, armor, 0);
                        return Ok(Step::Finish("贿生：其无牌可弃，伤害照常结算".into()));
                    }
                    let mut b = PendingBuilder::new(from, "pick_own").skill("huisheng");
                    for id in &cands {
                        b = b.option(
                            format!("card:{id}"),
                            format!(
                                "弃置【{}】（已弃 {}/{}）",
                                card_label(g, id),
                                picked.len(),
                                shown.len()
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选）")
                            .data(merge(
                                &ctx,
                                &json!({ "step": "discard", "shown": shown, "picked": picked }),
                            ))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !g.hand(from).iter().any(|c| *c == id) {
                        return Err("贿生：只能弃置自己的手牌".into());
                    }
                    flow::discard(g, from, &id, "贿生");
                    let mut next = picked.clone();
                    next.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        merge(
                            &ctx,
                            &json!({ "step": "discard", "shown": shown, "picked": next }),
                        ),
                    ))
                }
                Some(o) => Err(format!("贿生：未知选项 {o}")),
            }
        }
        other => Err(format!("贿生：未知阶段 {other}")),
    }
}

fn merge(base: &serde_json::Value, extra: &serde_json::Value) -> serde_json::Value {
    let mut m = base.as_object().cloned().unwrap_or_default();
    if let Some(e) = extra.as_object() {
        for (k, v) in e {
            m.insert(k.clone(), v.clone());
        }
    }
    serde_json::Value::Object(m)
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
// 张让 · 滔乱
// ---------------------------------------------------------------------------

fn can_taoluan(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "taoluan") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.counter(&turn_key(seat, "taoluan_off")) > 0 {
        return false;
    }
    if g.seats.iter().any(|s| flow::is_dying(s)) {
        return false;
    }
    if g.hand(seat).is_empty() {
        return false;
    }
    !taoluan_defs(g).is_empty() && !common::alive_others(g, seat).is_empty()
}

/// 滔乱可声明的牌名：基本牌 + 普通锦囊（按属性过滤），且本局未用过。
fn taoluan_defs(g: &Game) -> Vec<&'static cards::CardDef> {
    crate::cards_generated::DEFS
        .iter()
        .filter(|d| {
            let ok = d.category == Category::Basic
                || (d.category == Category::Trick && !d.tags.delayed);
            ok && g.counter(&format!("taoluan.used.{}", d.code)) == 0
        })
        .collect()
}

fn taoluan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "taoluan") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("name");
    // 主动入口首帧（`active_actions` 压的 `step=choose`）与牌名选择同义。
    let step = if step == "choose" { "name" } else { step };
    // 牌结算后的收口（CardUsed 订阅）：令一名其他角色二选一。
    if step == "name" {
        if let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) {
            if g.mark(&format!("vskill.{card}")) == Some("taoluan") {
                let def = g.card_defs.get(card).and_then(|c| cards::def(&c.def));
                let category = def.map(|d| d.category).unwrap_or(Category::Basic);
                return Ok(taoluan_pick_target(g, actor, category));
            }
            return Ok(Step::Done);
        }
    }
    match step {
        "name" => {
            if !can_taoluan(g, actor) {
                return Ok(Step::Done);
            }
            match take_answer(g, "taoluan") {
                None => {
                    let mut b = PendingBuilder::new(actor, "mode").skill("taoluan");
                    for d in taoluan_defs(g) {
                        b = b.option(
                            format!("use:{}", d.code),
                            format!("将一张牌当【{}】使用", d.name),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "name" }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("use:") => {
                    let code = a[4..].to_string();
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "card", "use": code })))
                }
                Some(o) => Err(format!("滔乱：未知选项 {o}")),
            }
        }
        "card" => {
            let code = ask
                .data
                .get("use")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let hand = g.hand(actor);
            match take_answer(g, "taoluan") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("taoluan");
                    for id in &hand {
                        b = b.option(
                            format!("card:{id}"),
                            format!(
                                "将【{}】当【{}】使用",
                                card_label(g, id),
                                cards::def(&code).map(|d| d.name).unwrap_or(&code)
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "card", "use": code }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("滔乱：只能使用自己的手牌".into());
                    }
                    let vid = super::super::card_play::use_as(g, actor, &code, &[id], &[], false)?;
                    g.set_mark(&format!("vskill.{vid}"), "taoluan");
                    g.bump(&format!("taoluan.used.{code}"));
                    Ok(Step::Finish(format!(
                        "滔乱：将一张牌当【{}】使用",
                        cards::def(&code).map(|d| d.name).unwrap_or(&code)
                    )))
                }
                Some(o) => Err(format!("滔乱：未知选项 {o}")),
            }
        }
        "target" => {
            let cat = ask
                .data
                .get("cat")
                .and_then(|v| v.as_str())
                .unwrap_or("Basic")
                .to_string();
            let cands = common::alive_others(g, actor);
            match take_answer(g, "taoluan") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("taoluan");
                    for s in &cands {
                        b = b.option(
                            format!("t:{s}"),
                            format!(
                                "令{}选择：交给张让一张牌 / 张让失去 1 点体力",
                                common::seat_name(g, *s)
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("放弃（不询问）")
                            .data(json!({ "step": "target", "cat": cat }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "滔乱：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("滔乱：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "choice", "target": t, "cat": cat }),
                    ))
                }
                Some(o) => Err(format!("滔乱：未知选项 {o}")),
            }
        }
        "choice" => {
            let t = common::seat_of(ask, "target").ok_or("滔乱：缺目标")?;
            let cat = ask
                .data
                .get("cat")
                .and_then(|v| v.as_str())
                .unwrap_or("Basic")
                .to_string();
            let want = match cat.as_str() {
                "Basic" => Category::Trick,
                _ => Category::Basic,
            };
            let give: Vec<String> = own_cards(g, t)
                .into_iter()
                .filter(|id| def_of(g, id).map(|d| d.category == want).unwrap_or(false))
                .collect();
            match take_answer(g, "taoluan") {
                None => {
                    let mut b = PendingBuilder::new(t, "choose").skill("taoluan");
                    if !give.is_empty() {
                        b = b.option("opt1", "交给张让一张类型不同的牌");
                    }
                    b = b.option("opt2", "令张让失去 1 点体力，本回合本技能失效");
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "choice", "target": t, "cat": cat }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" && !give.is_empty() => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "give", "target": t, "cat": cat }),
                )),
                Some(a) if a == "opt2" => {
                    flow::lose_hp(g, actor, 1, "滔乱");
                    g.bump(&turn_key(actor, "taoluan_off"));
                    Ok(Step::Finish("滔乱：张让失去 1 点体力，本回合本技能失效".into()))
                }
                Some(o) => Err(format!("滔乱：未知选项 {o}")),
            }
        }
        "give" => {
            let t = common::seat_of(ask, "target").ok_or("滔乱：缺目标")?;
            let cat = ask
                .data
                .get("cat")
                .and_then(|v| v.as_str())
                .unwrap_or("Basic")
                .to_string();
            let want = match cat.as_str() {
                "Basic" => Category::Trick,
                _ => Category::Basic,
            };
            let give: Vec<String> = own_cards(g, t)
                .into_iter()
                .filter(|id| def_of(g, id).map(|d| d.category == want).unwrap_or(false))
                .collect();
            match take_answer(g, "taoluan") {
                None => {
                    if give.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(t, "pick_own").skill("taoluan");
                    for id in &give {
                        b = b.option(
                            format!("card:{id}"),
                            format!("交给张让【{}】", card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "give", "target": t, "cat": cat }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !give.iter().any(|c| *c == id) {
                        return Err("滔乱：只能交给类型不同的牌".into());
                    }
                    flow::gain(g, actor, &id, t, "滔乱");
                    Ok(Step::Finish("滔乱：其交给张让一张牌".into()))
                }
                Some(o) => Err(format!("滔乱：未知选项 {o}")),
            }
        }
        other => Err(format!("滔乱：未知阶段 {other}")),
    }
}

fn taoluan_pick_target(g: &mut Game, actor: usize, category: Category) -> Step {
    let cands = common::alive_others(g, actor);
    if cands.is_empty() {
        return Step::Done;
    }
    let mut b = PendingBuilder::new(actor, "target").skill("taoluan");
    for s in &cands {
        b = b.option(
            format!("t:{s}"),
            format!(
                "令{}选择：交给张让一张牌 / 张让失去 1 点体力",
                common::seat_name(g, *s)
            ),
        );
    }
    // 用 `NeedWith` 写回帧数据：下一次进入必须走 target 阶段（不能再被 CardUsed 分支接管）。
    let data = json!({ "step": "target", "cat": format!("{category:?}") });
    Step::NeedWith(
        Stage::Target,
        data.clone(),
        b.escape("放弃（不询问）").data(data).build(),
    )
}
