//! B6 技能（一将成名·Y5：7 将 / 12 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B6.json` 的 `agreed`（权威）。
//! 触发类先询问「发动/放弃」，选牌/选目标用 `escape_mandatory`（不代选）。

use serde_json::json;

use super::super::cards::{self, Category};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::relay;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{
    DamageKind, DamageSpec, Game, Identity, Phase, SourceKind, Suit, Zone,
};
use super::common::{self, card_label, def_of, own_cards, phase_key, turn_key};
use super::{Face, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef {
        code: "zhanjue",
        name: "战绝",
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
        code: "qinwang",
        name: "勤王",
        kind: SkillKind::Lord,
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
        code: "wurong",
        name: "怃戎",
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
        code: "shizhi",
        name: "矢志",
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
        code: "yanzhu",
        name: "宴诛",
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
        code: "xingxue",
        name: "兴学",
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
        code: "zhaofu",
        name: "诏缚",
        kind: SkillKind::Limited,
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
        code: "anguo",
        name: "安国",
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
        code: "yaoming",
        name: "邀名",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            // 造成伤害后（每回合一次）
            Face {
                window: Window::AnyTime,
                subscriptions: &[Subscription {
                    event: EventKind::DamageAfter,
                    subject: Subject::By,
                }],
                hooks: &[],
                can_active: false,
            },
            // 受到伤害后（每回合一次）
            Face {
                window: Window::AnyTime,
                subscriptions: &[Subscription {
                    event: EventKind::DamageAfter,
                    subject: Subject::Self_,
                }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    SkillDef {
        code: "huaiyi",
        name: "怀异",
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
        code: "jigong",
        name: "急攻",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[Subscription {
                event: EventKind::PhaseStart,
                subject: Subject::Any,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "shifei",
        name: "饰非",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[],
            can_active: false,
        }],
    },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zhanjue" => zhanjue,
        "qinwang" => qinwang,
        "wurong" => wurong,
        "yanzhu" => yanzhu,
        "xingxue" => xingxue,
        "zhaofu" => zhaofu,
        "anguo" => anguo,
        "yaoming" => yaoming,
        "huaiyi" => huaiyi,
        "jigong" => jigong,
        "shifei" => shifei,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "zhanjue" => can_zhanjue,
        "qinwang" => can_qinwang,
        "wurong" => can_wurong,
        "yanzhu" => can_yanzhu,
        "zhaofu" => can_zhaofu,
        "anguo" => can_anguo,
        "huaiyi" => can_huaiyi,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 数值/回合钩子的**唯一出口**（`skills::*` 委托到这里）
// ---------------------------------------------------------------------------

/// 急攻：本回合手牌上限 = 本回合造成的伤害值（`Game::hand_limit` 唯一调用点）。
pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if g.counter(&turn_key(seat, "jigong")) > 0 {
        return Some(g.counter(&turn_key(seat, "damage_dealt")).max(0) as usize);
    }
    None
}

/// 矢志：体力为 1 时【闪】**替代**为【杀】（出牌阶段用法）。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    if !skills::active(g, seat, "shizhi") {
        return None;
    }
    if g.seats.get(seat).map(|s| s.hp).unwrap_or(0) != 1 {
        return None;
    }
    let d = def_of(g, id)?;
    if d.tags.shan {
        Some("sha")
    } else {
        None
    }
}

/// 矢志：响应语义（替代：不能再当【闪】用）。
pub fn response_as(g: &Game, seat: usize, id: &str, kind: super::super::cards::ResponseKind) -> super::CardAs {
    if !skills::active(g, seat, "shizhi") {
        return super::CardAs::NoOpinion;
    }
    if g.seats.get(seat).map(|s| s.hp).unwrap_or(0) != 1 {
        return super::CardAs::NoOpinion;
    }
    let Some(d) = def_of(g, id) else {
        return super::CardAs::NoOpinion;
    };
    if !d.tags.shan {
        return super::CardAs::NoOpinion;
    }
    if kind == super::super::cards::ResponseKind::Sha {
        super::CardAs::As("sha")
    } else {
        super::CardAs::Deny
    }
}

/// 饰非：需要【闪】时的替代（覆盖所有需要【闪】的上下文）。
pub fn response_alternatives(
    g: &Game,
    seat: usize,
    kind: super::super::cards::ResponseKind,
    _ctx: &str,
) -> Vec<(&'static str, String)> {
    if kind != super::super::cards::ResponseKind::Shan || !skills::active(g, seat, "shifei") {
        return Vec::new();
    }
    vec![(
        "shifei",
        "饰非：令当前回合角色摸一张，若其不是手牌唯一最多则弃置最多者一张牌，视为使用/打出【闪】"
            .to_string(),
    )]
}

/// 战绝：该【决斗】（虚拟牌）造成伤害时，刘谌与受伤者各摸一张（同一角色只摸一次）。
pub fn on_duel_damage(g: &mut Game, card: &str, from: Option<usize>, victim: usize) {
    if g.mark(&format!("vskill.{card}")) != Some("zhanjue") {
        return;
    }
    let Some(owner) = from else { return };
    if !skills::active(g, owner, "zhanjue") {
        return;
    }
    flow::draw(g, owner, 1);
    if victim != owner {
        flow::draw(g, victim, 1);
    }
}

// ---------------------------------------------------------------------------
// 刘谌 · 战绝（出牌阶段：将所有手牌当【决斗】使用；同伤者各摸 1；两发失效）
// ---------------------------------------------------------------------------

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn can_zhanjue(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "zhanjue")
        && play_phase(g, seat)
        && !g.hand(seat).is_empty()
        && !common::alive_others(g, seat).is_empty()
        && g.counter(&turn_key(seat, "zhanjue_uses")) < 2
}

fn zhanjue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhanjue") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    if step == "choose" {
        if !can_zhanjue(g, actor) {
            return Ok(Step::Done);
        }
        let Some(duel) = cards::def("juedou") else {
            return Ok(Step::Done);
        };
        let cands = super::super::card_play::legal_targets(g, actor, duel);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        match take_answer(g, "zhanjue") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("zhanjue");
                for s in &cands {
                    b = b.option(
                        format!("t:{s}"),
                        format!("将所有手牌当【决斗】对{}使用", common::seat_name(g, *s)),
                    );
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "choose" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("t:") => {
                let target: usize = a[2..].parse().map_err(|_| "战绝：目标不合法")?;
                if !cands.contains(&target) {
                    return Err("战绝：目标不合法".into());
                }
                let origins = g.hand(actor);
                if origins.is_empty() {
                    return Ok(Step::Done);
                }
                let vid =
                    super::super::card_play::use_as(g, actor, "juedou", &origins, &[target], false)?;
                // 虚拟牌 → 技能归属（伤害结算时读它，不按牌名/不按「本回合有没有发动过」猜）。
                g.set_mark(&format!("vskill.{vid}"), "zhanjue");
                g.bump(&turn_key(actor, "zhanjue_uses"));
                Ok(Step::Finish(format!(
                    "战绝：将 {} 张手牌当【决斗】使用（本阶段第 {} 次）",
                    origins.len(),
                    g.counter(&turn_key(actor, "zhanjue_uses"))
                )))
            }
            Some(o) => Err(format!("战绝：未知选项 {o}")),
        }
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 刘谌 · 勤王（主公技：弃一张牌发动激将；响应者摸 1；伤害来源视为刘谌）
// ---------------------------------------------------------------------------

fn can_qinwang(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "qinwang") || !play_phase(g, seat) {
        return false;
    }
    if g.seats.get(seat).and_then(|s| s.identity) != Some(Identity::Lord) {
        return false;
    }
    if own_cards(g, seat).is_empty() {
        return false;
    }
    if relay::order(g, seat, &g.seats[seat].kingdom, super::super::cards::ResponseKind::Sha).is_empty() {
        return false;
    }
    let Some(sha) = cards::def("sha") else {
        return false;
    };
    !super::super::card_play::legal_targets(g, seat, sha).is_empty()
}

fn qinwang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if !skills::active(g, actor, "qinwang") {
        return Ok(Step::Done);
    }
    // ① 激将子流程结果（唯一通道：`relay.result.<owner>` 标记）。
    let result_key = format!("relay.result.{actor}");
    if let Some(r) = g.mark(&result_key).map(|s| s.to_string()) {
        g.marks.remove(&result_key);
        if let Some(rest) = r.strip_prefix("played:") {
            let mut it = rest.splitn(2, ':');
            let card = it.next().unwrap_or_default().to_string();
            let by: usize = it.next().and_then(|s| s.parse().ok()).unwrap_or(actor);
            return Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "card": card, "by": by }),
            ));
        }
        return Ok(Step::Finish("勤王：无人代出【杀】".into()));
    }
    match step {
        "ask" | "choose" => match take_answer(g, "qinwang") {
            None => {
                let cands = own_cards(g, actor);
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = PendingBuilder::new(actor, "pick_own").skill("qinwang");
                for id in &cands {
                    b = b.option(
                        format!("card:{id}"),
                        format!("弃置【{}】发动激将", card_label(g, id)),
                    );
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "ask" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own_cards(g, actor).iter().any(|c| *c == id) {
                    return Err("勤王：只能弃置自己的手牌或装备".into());
                }
                flow::discard(g, actor, &id, "勤王");
                let kingdom = g.seats[actor].kingdom.clone();
                let started = relay::start(
                    g,
                    actor,
                    relay::RelayOpts {
                        kingdom,
                        kind: super::super::cards::ResponseKind::Sha,
                        for_use: true,
                        cause: "勤王",
                    },
                );
                if !started {
                    return Ok(Step::Finish("勤王：无人可响应".into()));
                }
                Ok(Step::Next(Stage::Choose, json!({ "step": "relay" })))
            }
            Some(o) => Err(format!("勤王：未知选项 {o}")),
        },
        "relay" => Ok(Step::Done),
        "target" => {
            let card = ask
                .data
                .get("card")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let by = common::seat_of(ask, "by").unwrap_or(actor);
            let Some(sha) = cards::def("sha") else {
                return Ok(Step::Done);
            };
            let cands = super::super::card_play::legal_targets(g, actor, sha);
            match take_answer(g, "qinwang") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Finish("勤王：无合法目标（视为使用取消）".into()));
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("qinwang");
                    for s in &cands {
                        b = b.option(
                            format!("t:{s}"),
                            format!("对{}使用【杀】", common::seat_name(g, *s)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("取消")
                            .data(json!({ "step": "target", "card": card, "by": by }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("勤王：取消使用".into())),
                Some(a) if a.starts_with("t:") => {
                    let target: usize = a[2..].parse().map_err(|_| "勤王：目标不合法")?;
                    if !cands.contains(&target) {
                        return Err("勤王：目标不合法".into());
                    }
                    super::super::card_play::use_relay(
                        g,
                        actor,
                        "sha",
                        &[card],
                        &[target],
                        by,
                    )?;
                    // 响应此激将的角色摸一张。
                    flow::draw(g, by, 1);
                    Ok(Step::Finish(format!(
                        "勤王：{} 代出【杀】，{} 摸一张",
                        common::seat_name(g, by),
                        common::seat_name(g, by)
                    )))
                }
                Some(o) => Err(format!("勤王：未知选项 {o}")),
            }
        }
        other => Err(format!("勤王：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 张嶷 · 怃戎（出牌阶段限一次：同时展示一张手牌，两种组合生效）
// ---------------------------------------------------------------------------

fn can_wurong(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "wurong")
        && play_phase(g, seat)
        && !g.hand(seat).is_empty()
        && g.counter(&phase_key(seat, "wurong")) == 0
        && common::alive_others(g, seat)
            .into_iter()
            .any(|t| !g.hand(t).is_empty())
}

fn wurong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "wurong") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    // 主动技入口统一由 `skills::start_active` 注入 `step="choose"`；本技能入口步名为 `target`。
    let step = if step == "choose" { "target" } else { step };
    match step {
        "target" => {
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| !g.hand(*t).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "wurong") {
                None => common::pick_seat(
                    g,
                    actor,
                    "wurong",
                    "与",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "怃戎：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("怃戎：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "self", "target": t }),
                    ))
                }
                Some(o) => Err(format!("怃戎：未知选项 {o}")),
            }
        }
        "self" => {
            let target = common::seat_of(ask, "target").ok_or("怃戎：缺目标")?;
            let hand = g.hand(actor);
            match take_answer(g, "wurong") {
                None => Ok(common::must_pick(
                    g,
                    actor,
                    "wurong",
                    "pick_own",
                    "展示",
                    &hand,
                    json!({ "step": "self", "target": target }),
                )),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("怃戎：展示牌必须来自手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "other", "target": target, "a": id }),
                    ))
                }
                Some(o) => Err(format!("怃戎：未知选项 {o}")),
            }
        }
        "other" => {
            let target = common::seat_of(ask, "target").ok_or("怃戎：缺目标")?;
            let a = common::card_of(ask, "a").ok_or("怃戎：缺己方展示牌")?;
            let hand = g.hand(target);
            match take_answer(g, "wurong") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Finish("怃戎：目标无手牌".into()));
                    }
                    let mut b = PendingBuilder::new(target, "pick_own").skill("wurong");
                    for id in &hand {
                        b = b.option(format!("card:{id}"), format!("展示【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "other", "target": target, "a": a }))
                            .build(),
                    ))
                }
                Some(ans) if ans.starts_with("card:") => {
                    let b = ans[5..].to_string();
                    if !hand.iter().any(|c| *c == b) {
                        return Err("怃戎：展示牌必须来自手牌".into());
                    }
                    Ok(wurong_resolve(g, actor, target, &a, &b))
                }
                Some(o) => Err(format!("怃戎：未知选项 {o}")),
            }
        }
        "gain" => {
            let target = common::seat_of(ask, "target").ok_or("怃戎：缺目标")?;
            let cands = common::target_cards(g, target);
            match take_answer(g, "wurong") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Finish("怃戎：目标已无牌".into()));
                    }
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("wurong");
                    for id in &cands {
                        b = b.option(
                            format!("card:{id}"),
                            format!("获得{}的【{}】", common::seat_name(g, target), card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "gain", "target": target }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("怃戎：只能获得其手牌或装备".into());
                    }
                    flow::gain(g, actor, &id, actor, "怃戎");
                    Ok(Step::Finish("怃戎：获得其一张牌".into()))
                }
                Some(o) => Err(format!("怃戎：未知选项 {o}")),
            }
        }
        other => Err(format!("怃戎：未知阶段 {other}")),
    }
}

fn wurong_resolve(g: &mut Game, actor: usize, target: usize, a: &str, b: &str) -> Step {
    let a_sha = def_of(g, a).map(|d| d.tags.sha).unwrap_or(false);
    let b_shan = def_of(g, b).map(|d| d.tags.shan).unwrap_or(false);
    // 真正同时展示：选择期不公开，结算时一起亮。
    g.reveal(&[a.to_string(), b.to_string()], "怃戎");
    let mut step = Step::Finish("怃戎：无事发生".to_string());
    if a_sha && !b_shan {
        flow::discard(g, actor, a, "怃戎");
        let armor = gear::armor_of(g, target);
        flow::damage(
            g,
            DamageSpec {
                from: Some(actor),
                target,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Skill,
            },
            armor,
            0,
        );
        step = Step::Finish("怃戎：弃【杀】并造成 1 点伤害".to_string());
    } else if !a_sha && b_shan {
        flow::discard(g, actor, a, "怃戎");
        step = Step::Next(
            Stage::PickOther,
            json!({ "step": "gain", "target": target }),
        );
    }
    g.conceal(&[a.to_string(), b.to_string()], "怃戎结算完成");
    if g.counter(&phase_key(actor, "wurong")) == 0 {
        g.bump(&phase_key(actor, "wurong"));
    }
    step
}

// ---------------------------------------------------------------------------
// 孙休 · 宴诛（出牌阶段限一次：目标二选一；opt1 令孙休失去宴诛并改兴学）
// ---------------------------------------------------------------------------

fn yanzhu_targets(g: &Game, seat: usize) -> Vec<usize> {
    common::alive_others(g, seat)
        .into_iter()
        .filter(|t| !own_cards(g, *t).is_empty())
        .collect()
}

fn can_yanzhu(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "yanzhu")
        && play_phase(g, seat)
        && g.counter(&phase_key(seat, "yanzhu")) == 0
        && !yanzhu_targets(g, seat).is_empty()
}

fn yanzhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "yanzhu") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    // 主动技入口统一由 `skills::start_active` 注入 `step="choose"`；本技能入口步名为 `target`。
    let step = if step == "choose" { "target" } else { step };
    match step {
        "target" => {
            let cands = yanzhu_targets(g, actor);
            match take_answer(g, "yanzhu") {
                None => common::pick_seat(
                    g,
                    actor,
                    "yanzhu",
                    "令",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "宴诛：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("宴诛：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "opt", "target": t }),
                    ))
                }
                Some(o) => Err(format!("宴诛：未知选项 {o}")),
            }
        }
        "opt" => {
            let target = common::seat_of(ask, "target").ok_or("宴诛：缺目标")?;
            let equips = common::equip_cards(g, target);
            let cards = own_cards(g, target);
            match take_answer(g, "yanzhu") {
                None => {
                    let mut b = PendingBuilder::new(target, "choose").skill("yanzhu");
                    if !equips.is_empty() {
                        b = b.option(
                            "opt1",
                            "令孙休获得你装备区的所有牌，其失去【宴诛】并修改【兴学】",
                        );
                    }
                    if !cards.is_empty() {
                        b = b.option("opt2", "弃置一张牌");
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "opt", "target": target }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" => {
                    for id in equips {
                        flow::gain(g, actor, &id, actor, "宴诛");
                    }
                    skills::revoke(g, actor, "yanzhu");
                    g.bump(&format!("xingxue.modified.{actor}"));
                    g.bump(&phase_key(actor, "yanzhu"));
                    Ok(Step::Finish(
                        "宴诛：孙休获得其装备区的所有牌，失去【宴诛】并修改【兴学】".into(),
                    ))
                }
                Some(a) if a == "opt2" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "discard", "target": target }),
                )),
                Some(o) => Err(format!("宴诛：未知选项 {o}")),
            }
        }
        "discard" => {
            let target = common::seat_of(ask, "target").ok_or("宴诛：缺目标")?;
            let cards = own_cards(g, target);
            match take_answer(g, "yanzhu") {
                None => {
                    if cards.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(target, "pick_own").skill("yanzhu");
                    for id in &cards {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "discard", "target": target }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cards.iter().any(|c| *c == id) {
                        return Err("宴诛：只能弃置自己的牌".into());
                    }
                    flow::discard(g, target, &id, "宴诛");
                    g.bump(&phase_key(actor, "yanzhu"));
                    Ok(Step::Finish("宴诛：目标弃置一张牌".into()))
                }
                Some(o) => Err(format!("宴诛：未知选项 {o}")),
            }
        }
        other => Err(format!("宴诛：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 孙休 · 兴学（结束阶段：每点体力一次，摸 1 并由该角色自选一张手牌置顶）
// ---------------------------------------------------------------------------

fn xingxue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "xingxue") {
        return Ok(Step::Done);
    }
    if !(g.turn == actor && g.phase == Phase::End) {
        return Ok(Step::Done);
    }
    let modified = g.counter(&format!("xingxue.modified.{actor}")) > 0;
    let total = if modified {
        1
    } else {
        g.seats.get(actor).map(|s| s.hp.max(0) as usize).unwrap_or(0)
    };
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    let left = ask
        .data
        .get("left")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(total);
    match step {
        "pick" => {
            if left == 0 {
                return Ok(Step::Done);
            }
            let cands = common::alive_seats(g);
            match take_answer(g, "xingxue") {
                None => {
                    let mut b = PendingBuilder::new(actor, "target").skill("xingxue");
                    for s in &cands {
                        b = b.option(
                            format!("t:{s}"),
                            format!(
                                "令{}摸一张牌并将一张手牌置于牌堆顶（剩余 {} 次）",
                                common::seat_name(g, *s),
                                left
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "pick", "left": left }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "兴学：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("兴学：目标不合法".into());
                    }
                    flow::draw(g, t, 1);
                    let next_left = left - 1;
                    if g.hand(t).is_empty() {
                        if next_left == 0 {
                            Ok(Step::Finish("兴学：结算完成".into()))
                        } else {
                            Ok(Step::Next(
                                Stage::Choose,
                                json!({ "step": "pick", "left": next_left }),
                            ))
                        }
                    } else {
                        Ok(Step::Next(
                            Stage::PickOwn,
                            json!({ "step": "top", "target": t, "left": next_left }),
                        ))
                    }
                }
                Some(o) => Err(format!("兴学：未知选项 {o}")),
            }
        }
        "top" => {
            let t = common::seat_of(ask, "target").ok_or("兴学：缺目标")?;
            let left_now = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let hand = g.hand(t);
            match take_answer(g, "xingxue") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(t, "pick_own").skill("xingxue");
                    for id in &hand {
                        b = b.option(
                            format!("card:{id}"),
                            format!("将【{}】置于牌堆顶", card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "top", "target": t, "left": left_now }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("兴学：只能置顶自己的手牌".into());
                    }
                    flow::to_deck_top(g, &id);
                    if left_now == 0 {
                        Ok(Step::Finish("兴学：结算完成".into()))
                    } else {
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "pick", "left": left_now }),
                        ))
                    }
                }
                Some(o) => Err(format!("兴学：未知选项 {o}")),
            }
        }
        other => Err(format!("兴学：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 孙休 · 诏缚（主公技 + 限定技：至多两名角色视为在所有其他吴势力攻击范围内）
// ---------------------------------------------------------------------------

fn can_zhaofu(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "zhaofu")
        && play_phase(g, seat)
        && g.seats.get(seat).and_then(|s| s.identity) == Some(Identity::Lord)
        && !skills::limited_used(g, seat, "zhaofu")
}

fn zhaofu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhaofu") || !can_zhaofu(g, actor) {
        return Ok(Step::Done);
    }
    let mut chosen = common::seats_of(ask, "chosen");
    let cands: Vec<usize> = common::alive_seats(g)
        .into_iter()
        .filter(|s| !chosen.contains(s))
        .collect();
    match take_answer(g, "zhaofu") {
        None => {
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(actor, "target").skill("zhaofu");
            for s in &cands {
                b = b.option(
                    format!("t:{s}"),
                    format!("令{}视为在所有其他吴势力角色攻击范围内", common::seat_name(g, *s)),
                );
            }
            let escape = if chosen.is_empty() {
                "不发动".to_string()
            } else {
                format!("完成（已选 {} 名）", chosen.len())
            };
            Ok(Step::Need(
                b.escape(escape)
                    .data(json!({ "chosen": chosen }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => {
            if chosen.is_empty() {
                Ok(Step::Done)
            } else {
                zhaofu_finalize(g, actor, &chosen);
                Ok(Step::Finish(format!(
                    "诏缚：{} 视为在所有其他吴势力角色攻击范围内",
                    chosen
                        .iter()
                        .map(|s| common::seat_name(g, *s))
                        .collect::<Vec<_>>()
                        .join("、")
                )))
            }
        }
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "诏缚：目标不合法")?;
            if !cands.contains(&t) {
                return Err("诏缚：目标不合法".into());
            }
            chosen.push(t);
            if chosen.len() >= 2 {
                zhaofu_finalize(g, actor, &chosen);
                Ok(Step::Finish(format!(
                    "诏缚：{} 视为在所有其他吴势力角色攻击范围内",
                    chosen
                        .iter()
                        .map(|s| common::seat_name(g, *s))
                        .collect::<Vec<_>>()
                        .join("、")
                )))
            } else {
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "chosen": chosen }),
                ))
            }
        }
        Some(o) => Err(format!("诏缚：未知选项 {o}")),
    }
}

fn zhaofu_finalize(g: &mut Game, actor: usize, chosen: &[usize]) {
    skills::mark_limited_used(g, actor, "zhaofu");
    for s in chosen {
        g.bump(&format!("zhaofu.{s}"));
    }
}

/// 诏缚：`from` 对 `to` 视为在攻击范围内（统一「能否打到」入口）。
pub fn in_attack_range(g: &Game, from: usize, to: usize) -> bool {
    if from == to {
        return false;
    }
    if g.seats.get(from).map(|s| s.kingdom.as_str()) != Some("wu") {
        return false;
    }
    g.counter(&format!("zhaofu.{to}")) > 0
}

// ---------------------------------------------------------------------------
// 朱治 · 安国（出牌阶段限一次：三项全场最少；补做按条件判定）
// ---------------------------------------------------------------------------

fn can_anguo(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "anguo")
        && play_phase(g, seat)
        && g.counter(&phase_key(seat, "anguo")) == 0
        && !common::alive_others(g, seat).is_empty()
}

fn anguo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_anguo(g, actor) {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    // 主动技入口统一由 `skills::start_active` 注入 `step="choose"`；本技能入口步名为 `target`。
    let step = if step == "choose" { "target" } else { step };
    match step {
        "target" => {
            let cands = common::alive_others(g, actor);
            match take_answer(g, "anguo") {
                None => common::pick_seat(
                    g,
                    actor,
                    "anguo",
                    "令",
                    &cands,
                    json!({ "step": "target" }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "安国：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("安国：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "apply", "target": t }),
                    ))
                }
                Some(o) => Err(format!("安国：未知选项 {o}")),
            }
        }
        "apply" => {
            let target = common::seat_of(ask, "target").ok_or("安国：缺目标")?;
            let (t_hand, t_hp, t_eq, a_hand, a_hp, a_eq) = anguo_conditions(g, actor, target);
            if t_hand {
                flow::draw(g, target, 1);
            }
            if t_hp {
                flow::heal(g, target, 1, "安国");
            }
            if t_eq {
                anguo_equip_from_pile(g, target);
            }
            if !t_hand && a_hand {
                flow::draw(g, actor, 1);
            }
            if !t_hp && a_hp {
                flow::heal(g, actor, 1, "安国");
            }
            if !t_eq && a_eq {
                anguo_equip_from_pile(g, actor);
            }
            g.bump(&phase_key(actor, "anguo"));
            Ok(Step::Finish(format!(
                "安国：结算 {}（手牌{} / 体力{} / 装备{}）",
                common::seat_name(g, target),
                if t_hand { "✓" } else { "✗" },
                if t_hp { "✓" } else { "✗" },
                if t_eq { "✓" } else { "✗" }
            )))
        }
        other => Err(format!("安国：未知阶段 {other}")),
    }
}

fn anguo_conditions(g: &Game, actor: usize, target: usize) -> (bool, bool, bool, bool, bool, bool) {
    let alive = common::alive_seats(g);
    let hand_min = alive.iter().map(|s| g.hand_size(*s)).min().unwrap_or(0);
    let hp_min = alive.iter().map(|s| g.seats[*s].hp).min().unwrap_or(0);
    let eq_min = alive.iter().map(|s| common::equip_count(g, *s)).min().unwrap_or(0);
    (
        g.hand_size(target) == hand_min,
        g.seats[target].hp == hp_min,
        common::equip_count(g, target) == eq_min,
        g.hand_size(actor) == hand_min,
        g.seats[actor].hp == hp_min,
        common::equip_count(g, actor) == eq_min,
    )
}

/// 从牌堆（没有则弃牌堆）取第一件装备，直接置入该角色装备区。
fn anguo_equip_from_pile(g: &mut Game, seat: usize) {
    let is_equip = |g: &Game, id: &str| {
        def_of(g, id).map(|d| d.slot.is_some()).unwrap_or(false)
    };
    let from_deck: Option<String> = g
        .deck
        .iter()
        .rev()
        .find(|id| is_equip(g, id))
        .cloned();
    let pick = from_deck.or_else(|| {
        let mut cands: Vec<String> = g
            .cards
            .iter()
            .filter(|(id, z)| **z == Zone::Discard && is_equip(g, id))
            .map(|(id, _)| id.clone())
            .collect();
        cands.sort();
        cands.into_iter().next()
    });
    let Some(id) = pick else { return };
    if let Err(_e) = super::super::card_play::equip_from_pile(g, seat, &id) {
        // 装备栏全部废除等边界：无事发生（牌保持原区）。
    }
}

// ---------------------------------------------------------------------------
// 全琮 · 邀名（造成/受到伤害后各限一次：弃手牌更多者一张 / 令手牌更少者摸 1）
// ---------------------------------------------------------------------------

fn yaoming(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "yaoming") {
        return Ok(Step::Done);
    }
    // 方向：首次入口从伤害事件快照判定；后续阶段从帧数据 `dir` 继承
    // （`Step::Next` 会整份替换 data，事件快照不会自动保留）。
    let dir = match ask.data.get("dir").and_then(|v| v.as_str()) {
        Some(d) => d.to_string(),
        None => {
            let by = common::seat_of(ask, "by");
            let victim = common::seat_of(ask, "victim");
            if by == Some(actor) {
                "dealt".to_string()
            } else if victim == Some(actor) {
                "taken".to_string()
            } else {
                return Ok(Step::Done);
            }
        }
    };
    let used_key = turn_key(actor, &format!("yaoming_{dir}"));
    if g.counter(&used_key) > 0 {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    match step {
        "choose" => {
            let (more, less) = yaoming_cands(g, actor);
            if more.is_empty() && less.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "yaoming") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("yaoming");
                    if !more.is_empty() {
                        b = b.option("discard", "弃置手牌数大于你的一名角色一张手牌");
                    }
                    if !less.is_empty() {
                        b = b.option("draw", "令手牌数小于你的一名角色摸一张牌");
                    }
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "choose", "dir": dir }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "discard" && !more.is_empty() => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "pick_more", "dir": dir }),
                )),
                Some(a) if a == "draw" && !less.is_empty() => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "pick_less", "dir": dir }),
                )),
                Some(o) => Err(format!("邀名：未知选项 {o}")),
            }
        }
        "pick_more" => {
            let dir = ask.data.get("dir").and_then(|v| v.as_str()).unwrap_or("dealt").to_string();
            let (more, _) = yaoming_cands(g, actor);
            match take_answer(g, "yaoming") {
                None => common::pick_seat(
                    g,
                    actor,
                    "yaoming",
                    "弃置",
                    &more,
                    json!({ "step": "pick_more", "dir": dir }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "邀名：目标不合法")?;
                    if !more.contains(&t) {
                        return Err("邀名：目标手牌数必须大于你".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "pick_card", "target": t, "dir": dir }),
                    ))
                }
                Some(o) => Err(format!("邀名：未知选项 {o}")),
            }
        }
        "pick_card" => {
            let t = common::seat_of(ask, "target").ok_or("邀名：缺目标")?;
            let dir = ask.data.get("dir").and_then(|v| v.as_str()).unwrap_or("dealt").to_string();
            let hand = g.hand(t);
            match take_answer(g, "yaoming") {
                None => {
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("yaoming");
                    for id in &hand {
                        b = b.option(
                            format!("card:{id}"),
                            format!("弃置{}的【{}】", common::seat_name(g, t), card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick_card", "target": t, "dir": dir }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("邀名：只能弃置目标手牌".into());
                    }
                    flow::discard(g, t, &id, "邀名");
                    g.bump(&turn_key(actor, &format!("yaoming_{dir}")));
                    Ok(Step::Finish("邀名：弃置其一张手牌".into()))
                }
                Some(o) => Err(format!("邀名：未知选项 {o}")),
            }
        }
        "pick_less" => {
            let dir = ask.data.get("dir").and_then(|v| v.as_str()).unwrap_or("dealt").to_string();
            let (_, less) = yaoming_cands(g, actor);
            match take_answer(g, "yaoming") {
                None => common::pick_seat(
                    g,
                    actor,
                    "yaoming",
                    "令",
                    &less,
                    json!({ "step": "pick_less", "dir": dir }),
                )
                .into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "邀名：目标不合法")?;
                    if !less.contains(&t) {
                        return Err("邀名：目标手牌数必须小于你".into());
                    }
                    flow::draw(g, t, 1);
                    g.bump(&turn_key(actor, &format!("yaoming_{dir}")));
                    Ok(Step::Finish("邀名：令其摸一张牌".into()))
                }
                Some(o) => Err(format!("邀名：未知选项 {o}")),
            }
        }
        other => Err(format!("邀名：未知阶段 {other}")),
    }
}

fn yaoming_cands(g: &Game, actor: usize) -> (Vec<usize>, Vec<usize>) {
    let mine = g.hand_size(actor);
    let more: Vec<usize> = common::alive_others(g, actor)
        .into_iter()
        .filter(|t| g.hand_size(*t) > mine && !g.hand(*t).is_empty())
        .collect();
    let less: Vec<usize> = common::alive_others(g, actor)
        .into_iter()
        .filter(|t| g.hand_size(*t) < mine)
        .collect();
    (more, less)
}

// ---------------------------------------------------------------------------
// 公孙渊 · 怀异（出牌阶段限一次：展示手牌 → 弃一种颜色 → 每张可获一名角色一张牌）
// ---------------------------------------------------------------------------

fn can_huaiyi(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "huaiyi")
        && play_phase(g, seat)
        && g.counter(&phase_key(seat, "huaiyi")) == 0
        && !g.hand(seat).is_empty()
        && common::alive_others(g, seat)
            .into_iter()
            .any(|t| !common::target_cards(g, t).is_empty())
}

fn huaiyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "huaiyi") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("color");
    // 主动技入口统一由 `skills::start_active` 注入 `step="choose"`；本技能入口步名为 `color`。
    let step = if step == "choose" { "color" } else { step };
    // 前置只在校验首次入口（后续阶段手牌可能被弃空/获得，不能再跑一次前置）。
    if step == "color" && !can_huaiyi(g, actor) {
        return Ok(Step::Done);
    }
    match step {
        "color" => {
            let hand = g.hand(actor);
            let has_red = hand
                .iter()
                .any(|id| g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false));
            let has_black = hand
                .iter()
                .any(|id| g.card_defs.get(id).map(|c| c.suit.is_black()).unwrap_or(false));
            match take_answer(g, "huaiyi") {
                None => {
                    // 展示所有手牌（公共可见）。
                    g.reveal(&hand, "怀异");
                    let mut b = PendingBuilder::new(actor, "choose").skill("huaiyi");
                    if has_red {
                        b = b.option("red", "弃置所有红色手牌");
                    }
                    if has_black {
                        b = b.option("black", "弃置所有黑色手牌");
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("选择一种颜色")
                            .data(json!({ "step": "color" }))
                            .build(),
                    ))
                }
                Some(a) if a == "red" || a == "black" => {
                    let red = a == "red";
                    let targets: Vec<String> = hand
                        .into_iter()
                        .filter(|id| {
                            g.card_defs
                                .get(id)
                                .map(|c| c.suit.is_red() == red)
                                .unwrap_or(false)
                        })
                        .collect();
                    for id in &targets {
                        flow::discard(g, actor, id, "怀异");
                    }
                    g.conceal(&targets, "怀异");
                    let quota = targets.len();
                    if quota == 0 {
                        g.bump(&phase_key(actor, "huaiyi"));
                        return Ok(Step::Finish("怀异：没有可弃置的该颜色手牌".into()));
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "pick", "quota": quota, "gained": 0 }),
                    ))
                }
                Some(o) => Err(format!("怀异：未知选项 {o}")),
            }
        }
        "pick" => {
            let quota = ask.data.get("quota").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let gained = ask.data.get("gained").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if gained >= quota {
                return huaiyi_finish(g, actor, gained);
            }
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| !common::target_cards(g, *t).is_empty())
                .collect();
            match take_answer(g, "huaiyi") {
                None => {
                    if cands.is_empty() {
                        return huaiyi_finish(g, actor, gained);
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("huaiyi");
                    for s in &cands {
                        b = b.option(
                            format!("t:{s}"),
                            format!(
                                "获得{}一张牌（已获得 {}/{}）",
                                common::seat_name(g, *s),
                                gained,
                                quota
                            ),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("停止获得")
                            .data(json!({ "step": "pick", "quota": quota, "gained": gained }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => huaiyi_finish(g, actor, gained),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "怀异：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("怀异：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "gain", "quota": quota, "gained": gained, "target": t }),
                    ))
                }
                Some(o) => Err(format!("怀异：未知选项 {o}")),
            }
        }
        "gain" => {
            let t = common::seat_of(ask, "target").ok_or("怀异：缺目标")?;
            let quota = ask.data.get("quota").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let gained = ask.data.get("gained").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let cands = common::target_cards(g, t);
            match take_answer(g, "huaiyi") {
                None => {
                    if cands.is_empty() {
                        return huaiyi_finish(g, actor, gained);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("huaiyi");
                    for id in &cands {
                        b = b.option(
                            format!("card:{id}"),
                            format!("获得{}的【{}】", common::seat_name(g, t), card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({
                                "step": "gain",
                                "quota": quota,
                                "gained": gained,
                                "target": t
                            }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("怀异：只能获得其手牌或装备".into());
                    }
                    flow::gain(g, actor, &id, actor, "怀异");
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "pick", "quota": quota, "gained": gained + 1 }),
                    ))
                }
                Some(o) => Err(format!("怀异：未知选项 {o}")),
            }
        }
        other => Err(format!("怀异：未知阶段 {other}")),
    }
}

fn huaiyi_finish(g: &mut Game, actor: usize, gained: usize) -> Result<Step, String> {
    g.bump(&phase_key(actor, "huaiyi"));
    if gained >= 2 {
        flow::lose_hp(g, actor, 1, "怀异");
        Ok(Step::Finish(format!("怀异：获得 {gained} 张牌，失去 1 点体力")))
    } else {
        Ok(Step::Finish(format!("怀异：获得 {gained} 张牌")))
    }
}

// ---------------------------------------------------------------------------
// 郭图逢纪 · 急攻（出牌阶段开始时：可摸 2；本回合手牌上限 = 本阶段造成伤害值）
// ---------------------------------------------------------------------------

fn jigong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jigong") {
        return Ok(Step::Done);
    }
    if g.turn != actor || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    match take_answer(g, "jigong") {
        None => {
            if g.counter(&turn_key(actor, "jigong_offered")) > 0 {
                return Ok(Step::Done);
            }
            g.bump(&turn_key(actor, "jigong_offered"));
            Ok(Step::Need(
                PendingBuilder::new(actor, "choose")
                    .skill("jigong")
                    .option("opt1", "急攻：摸两张牌，本出牌阶段手牌上限等于造成的伤害值")
                    .escape("不发动急攻（手牌上限按体力值）")
                    .data(json!({ "step": "choose" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "opt1" => {
            flow::draw(g, actor, 2);
            g.set_counter(&turn_key(actor, "jigong"), 1);
            // 「本出牌阶段」口径：从发动时刻开始计。
            g.set_counter(&turn_key(actor, "damage_dealt"), 0);
            Ok(Step::Finish("急攻：摸两张，本出牌阶段手牌上限 = 造成的伤害值".into()))
        }
        Some(o) => Err(format!("急攻：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 郭图逢纪 · 饰非（需要【闪】时：当前回合角色摸 1；非唯一最多则弃最多者一张）
// ---------------------------------------------------------------------------

fn shifei(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "shifei") {
        return Ok(Step::Done);
    }
    let cur = g.turn;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("draw");
    // 统一响应替代窗口用 `step=alt`；保留旧的 `draw` 入口（历史用例/直接驱动）。
    let step = if step == "alt" { "draw" } else { step };
    match step {
        "draw" => {
            flow::draw(g, cur, 1);
            let mine = g.hand_size(cur);
            let alive = common::alive_seats(g);
            let max = alive.iter().map(|s| g.hand_size(*s)).max().unwrap_or(0);
            let max_seats: Vec<usize> = alive
                .into_iter()
                .filter(|s| g.hand_size(*s) == max)
                .collect();
            let unique_cur = max > 0 && max_seats.len() == 1 && max_seats[0] == cur && mine == max;
            if unique_cur || max == 0 {
                g.set_mark(&format!("response.sub.{actor}"), "1");
                return Ok(Step::Finish(
                    "饰非：当前回合角色手牌唯一最多（不弃置），视为使用/打出【闪】".into(),
                ));
            }
            Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "pick", "seats": max_seats }),
            ))
        }
        "pick" => {
            let seats = common::seats_of(ask, "seats");
            let mut cands: Vec<String> = Vec::new();
            for s in &seats {
                cands.extend(common::target_cards(g, *s));
            }
            cands.sort();
            if cands.is_empty() {
                g.set_mark(&format!("response.sub.{actor}"), "1");
                return Ok(Step::Finish("饰非：无牌可弃，视为使用/打出【闪】".into()));
            }
            match take_answer(g, "shifei") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_other").skill("shifei");
                    for id in &cands {
                        let owner = g.cards.get(id).and_then(|z| z.seat()).unwrap_or(actor);
                        b = b.option(
                            format!("card:{id}"),
                            format!("弃置{}的【{}】", common::seat_name(g, owner), card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick", "seats": seats }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    let Some(owner) = g.cards.get(&id).and_then(|z| z.seat()) else {
                        return Err("饰非：牌不存在".into());
                    };
                    if !cands.iter().any(|c| *c == id) {
                        return Err("饰非：只能弃置手牌最多者的牌".into());
                    }
                    flow::discard(g, owner, &id, "饰非");
                    g.set_mark(&format!("response.sub.{actor}"), "1");
                    Ok(Step::Finish("饰非：弃置最多者一张牌，视为使用/打出【闪】".into()))
                }
                Some(o) => Err(format!("饰非：未知选项 {o}")),
            }
        }
        other => Err(format!("饰非：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

trait IntoStep {
    fn into_step(self) -> Result<Step, String>;
}

impl IntoStep for Step {
    fn into_step(self) -> Result<Step, String> {
        Ok(self)
    }
}

/// 花色短名（怀异文案/调试用；避免在 handler 里拼中文花色字符串）。
#[allow(dead_code)]
fn suit_label(s: Suit) -> &'static str {
    s.label()
}

/// 牌类别（保留给后续批次；避免 unused import 警告演变成门禁噪音）。
#[allow(dead_code)]
fn is_trick(g: &Game, id: &str) -> bool {
    def_of(g, id)
        .map(|d| d.category == Category::Trick)
        .unwrap_or(false)
}
