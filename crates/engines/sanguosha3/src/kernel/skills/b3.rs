//! B3 技能（一将成名·批次1/2/Y1：5 将 / 9 技能 + 授予技 排异）。
//!
//! 依据 `docs/games/sanguosha/align/B3.json` 的 `agreed`（权威）。
//! 触发类可选技能先询问「发动/放弃」，选牌/选目标用 `escape_mandatory`（不代选）。

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

pub const QUAN: &str = "quan";

pub static SKILLS: &[SkillDef] = &[
    // —— 马谡 ——
    SkillDef {
        code: "sanyao",
        name: "散谣",
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
        code: "zhiman",
        name: "制蛮",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::DamageBefore],
            can_active: false,
        }],
    },
    // —— 荀攸 ——
    SkillDef {
        code: "qice",
        name: "奇策",
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
        code: "zhiyu",
        name: "智愚",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 钟会 ——
    SkillDef {
        code: "quanji",
        name: "权计",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "zili",
        name: "自立",
        kind: SkillKind::Awaken,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Prepare),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 满宠 ——
    SkillDef {
        code: "junxing",
        name: "峻刑",
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
        code: "yuce",
        name: "御策",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 关平 ——
    SkillDef {
        code: "longyin",
        name: "龙吟",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 授予技 ——
    SkillDef {
        code: "paiyi",
        name: "排异",
        kind: SkillKind::Normal,
        grant_only: true,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "sanyao" => sanyao,
        "zhiman" => zhiman,
        "qice" => qice,
        "zhiyu" => zhiyu,
        "quanji" => quanji,
        "zili" => zili,
        "junxing" => junxing,
        "yuce" => yuce,
        "longyin" => longyin,
        "paiyi" => paiyi,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "sanyao" => can_sanyao,
        "qice" => can_qice,
        "junxing" => can_junxing,
        "paiyi" => can_paiyi,
        _ => return None,
    })
}

/// 手牌上限（权计：每张「权」+1）。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    mark_cards(g, seat, QUAN).len() as i32
}

/// 是否需要先询问「造成伤害时」技能（制蛮）：由 `flow::damage` 在伤害前调用。
pub fn should_ask_damage_before(g: &Game, from: usize, target: usize) -> bool {
    if !skills::enabled("zhiman") || !skills::active(g, from, "zhiman") {
        return false;
    }
    if from == target || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return false;
    }
    // 目标区域（手牌/装备/判定）有牌才能发动。
    (0..g.seats.len()).any(|_| false)
        || g.cards.iter().any(|(_, z)| match z {
            Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) => *s == target,
            _ => false,
        })
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

fn card_category(g: &Game, id: &str) -> Option<Category> {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.category)
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

fn to_mark(g: &mut Game, seat: usize, name: &str, id: &str, cause: &str) {
    if let Some(owner) = g.cards.get(id).cloned().and_then(|z| z.seat()) {
        let mut lost = super::super::event::Event::card(
            EventKind::CardLost,
            Some(owner),
            id.to_string(),
            Some(cause.to_string()),
        );
        lost.by = Some(seat);
        lost.from = Some(owner);
        let ev = lost.snapshot(g);
        g.push_event(ev);
    }
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

fn must_pick(
    g: &Game,
    actor: usize,
    skill: &'static str,
    kind: &'static str,
    verb: &str,
    cands: &[String],
    data: Value,
) -> Step {
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
// 散谣（马谡）：出牌阶段各限一次（体力最大 / 手牌最多）
// ---------------------------------------------------------------------------

fn can_sanyao(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && (g.counter(&phase_key(seat, "sanyao_hp")) == 0
            || g.counter(&phase_key(seat, "sanyao_hand")) == 0)
        && !own_cards(g, seat).is_empty()
}

fn sanyao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("mode");
    if step == "mode" {
        if !can_sanyao(g, actor) {
            return Ok(Step::Done);
        }
        let hp_ok = g.counter(&phase_key(actor, "sanyao_hp")) == 0;
        let hand_ok = g.counter(&phase_key(actor, "sanyao_hand")) == 0;
        return match take_answer(g, "sanyao") {
            None => {
                let mut b = PendingBuilder::new(actor, "mode").skill("sanyao");
                if hp_ok {
                    b = b.option("mode:hp", "散谣①：对体力值最大的一名角色造成 1 点伤害");
                }
                if hand_ok {
                    b = b.option("mode:hand", "散谣②：对手牌数最多的一名角色造成 1 点伤害");
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "mode" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "mode:hp" => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "cost", "mode": "hp" }),
            )),
            Some(a) if a == "mode:hand" => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "cost", "mode": "hand" }),
            )),
            Some(o) => Err(format!("散谣：未知选项 {o}")),
        };
    }
    if step == "cost" {
        let mode = ask.data.get("mode").and_then(|v| v.as_str()).unwrap_or("hp").to_string();
        let own = own_cards(g, actor);
        return match take_answer(g, "sanyao") {
            None => Ok(must_pick(
                g,
                actor,
                "sanyao",
                "pick_own",
                "弃置",
                &own,
                json!({ "step": "cost", "mode": mode }),
            )),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own.contains(&id) {
                    return Err("散谣：只能弃置自己的手牌/装备".into());
                }
                flow::discard(g, actor, &id, "散谣");
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "mode": mode }),
                ))
            }
            Some(o) => Err(format!("散谣：未知选项 {o}")),
        };
    }
    // target：体力值最大 / 手牌数最多（含自己；并列任选）。
    let mode = ask.data.get("mode").and_then(|v| v.as_str()).unwrap_or("hp").to_string();
    let cands: Vec<usize> = {
        let alive: Vec<usize> = (0..g.seats.len()).filter(|&i| g.seats[i].alive).collect();
        if mode == "hp" {
            let max = alive.iter().map(|&i| g.seats[i].hp).max().unwrap_or(0);
            alive.into_iter().filter(|&i| g.seats[i].hp == max).collect()
        } else {
            let max = alive.iter().map(|&i| g.hand_size(i)).max().unwrap_or(0);
            alive.into_iter().filter(|&i| g.hand_size(i) == max).collect()
        }
    };
    match take_answer(g, "sanyao") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("sanyao");
            for t in &cands {
                b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选目标）")
                    .data(json!({ "step": "target", "mode": mode }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "散谣：非法目标")?;
            if !cands.contains(&t) {
                return Err("散谣：目标须为该项目的最大者".into());
            }
            g.bump(&phase_key(actor, if mode == "hp" { "sanyao_hp" } else { "sanyao_hand" }));
            let spec = DamageSpec {
                from: Some(actor),
                target: t,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Skill,
            };
            let armor = gear::armor_of(g, t);
            flow::damage(g, spec, armor, 0);
            Ok(Step::Finish("散谣：造成 1 点伤害".into()))
        }
        Some(o) => Err(format!("散谣：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 制蛮（马谡）：造成伤害时可防止，然后获得其区域一张牌（伤害结算前）
// ---------------------------------------------------------------------------

fn zhiman(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("normal");
    let source = ask.data.get("source").and_then(|v| v.as_str()).unwrap_or("skill");
    let data = json!({ "step": "ask", "target": target, "amount": amount, "kind": kind, "source": source });
    let damage_kind = match kind {
        "fire" => DamageKind::Fire,
        "thunder" => DamageKind::Thunder,
        _ => DamageKind::Normal,
    };
    let source_kind = match source {
        "slash" => SourceKind::Slash,
        "nanman" => SourceKind::Nanman,
        "wanjian" => SourceKind::Wanjian,
        "duel" => SourceKind::Duel,
        "skill" => SourceKind::Skill,
        _ => SourceKind::Other,
    };
    let apply = |g: &mut Game| {
        let spec = DamageSpec {
            from: Some(actor),
            target,
            amount,
            kind: damage_kind,
            source: source_kind,
        };
        let armor = gear::armor_of(g, target);
        let _ = flow::damage_apply(g, spec, armor, 0);
    };
    let cands: Vec<String> = {
        let mut ids = own_cards(g, target);
        for id in g
            .cards
            .iter()
            .filter(|(_, z)| matches!(z, Zone::Judge(s) if *s == target))
        {
            ids.push(id.0.clone());
        }
        ids.sort();
        ids
    };
    if cands.is_empty() {
        apply(g);
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "zhiman") {
            None => Ok(trigger_ask(
                actor,
                "zhiman",
                "制蛮：防止此伤害，然后获得其一张牌",
                data,
            )),
            Some(a) if a == "pass" => {
                apply(g);
                Ok(Step::Finish("制蛮：未发动".into()))
            }
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "pick", "target": target }),
            )),
            Some(o) => Err(format!("制蛮：未知选项 {o}")),
        };
    }
    match take_answer(g, "zhiman") {
        None => Ok(must_pick(
            g,
            actor,
            "zhiman",
            "pick_other",
            "获得",
            &cands,
            json!({ "step": "pick", "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("制蛮：只能获得其区域里的一张牌".into());
            }
            if matches!(g.cards.get(&id), Some(Zone::Judge(_))) {
                super::super::card_play::remove_card(g, target, &id, "制蛮");
            } else {
                flow::gain(g, actor, &id, actor, "制蛮");
            }
            Ok(Step::Finish("制蛮：防止伤害并获得其一张牌".into()))
        }
        Some(o) => Err(format!("制蛮：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 奇策（荀攸）：出牌阶段限一次，将所有手牌当任意非延时锦囊使用
// ---------------------------------------------------------------------------

fn can_qice(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play && g.counter(&phase_key(seat, "qice")) == 0 && !g.hand(seat).is_empty()
}

fn qice(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_qice(g, actor) {
        return Ok(Step::Done);
    }
    match take_answer(g, "qice") {
        None => {
            let mut b = PendingBuilder::new(actor, "mode").skill("qice");
            for d in crate::cards_generated::DEFS {
                if d.category == Category::Trick
                    && !d.tags.delayed
                    && !d.tags.wuxie
                    && d.slot.is_none()
                {
                    b = b.option(format!("use:{}", d.code), format!("将所有手牌当【{}】使用", d.name));
                }
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "mode" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("use:") => {
            let code = a[4..].to_string();
            let origin = g.hand(actor);
            if origin.is_empty() {
                return Ok(Step::Done);
            }
            super::super::card_play::use_as(g, actor, &code, &origin, &[], false)?;
            g.bump(&phase_key(actor, "qice"));
            Ok(Step::Done)
        }
        Some(o) => Err(format!("奇策：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 智愚（荀攸）：受伤后摸 1 → 展示手牌；颜色均相同 → 来源弃一张手牌
// ---------------------------------------------------------------------------

fn zhiyu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let src = seat_of(ask, "by").or_else(|| seat_of(ask, "target"));
    let data = json!({ "step": "ask", "by": src });
    match take_answer(g, "zhiyu") {
        None => Ok(trigger_ask(actor, "zhiyu", "智愚：摸一张并展示手牌", data)),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, actor, 1);
            let hand = g.hand(actor);
            g.reveal(&hand, "智愚");
            let reds = hand
                .iter()
                .filter(|id| g.card_defs.get(*id).map(|c| c.suit.is_red()).unwrap_or(false))
                .count();
            let same = reds == 0 || reds == hand.len();
            g.conceal(&hand, "智愚·结算完成");
            if same {
                if let Some(src) = src {
                    if g.seats.get(src).map(|s| s.alive).unwrap_or(false) && !g.hand(src).is_empty() {
                        let cands = g.hand(src);
                        return Ok(Step::Next(
                            Stage::PickOwn,
                            json!({ "step": "pick", "by": src, "cands": cands }),
                        ));
                    }
                }
            }
            Ok(Step::Finish("智愚：摸一张并展示手牌".into()))
        }
        Some(o) => Err(format!("智愚：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 权计（钟会）：受到每点伤害后，可摸 1 并将 1 张手牌置于武将牌上为「权」；权 +1 手牌上限
// ---------------------------------------------------------------------------

fn quanji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let points = ask
        .data
        .get("amount")
        .or_else(|| ask.data.get("points"))
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as i32;
    let done = ask.data.get("done").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    let data = json!({ "step": "ask", "points": points, "done": done });
    if done >= points {
        return Ok(Step::Done);
    }
    if step == "ask" {
        return match take_answer(g, "quanji") {
            None => Ok(trigger_ask(
                actor,
                "quanji",
                "权计：摸一张并将一张手牌置于武将牌上为「权」",
                data,
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                flow::draw(g, actor, 1);
                let own = g.hand(actor);
                if own.is_empty() {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "points": points, "done": done }),
                ))
            }
            Some(o) => Err(format!("权计：未知选项 {o}")),
        };
    }
    let own = g.hand(actor);
    match take_answer(g, "quanji") {
        None => Ok(must_pick(
            g,
            actor,
            "quanji",
            "pick_own",
            "置于武将牌上",
            &own,
            json!({ "step": "pick", "points": points, "done": done }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) {
                return Err("权计：只能选择自己的手牌".into());
            }
            to_mark(g, actor, QUAN, &id, "权计");
            let done = done + 1;
            if done >= points {
                return Ok(Step::Finish("权计：获得「权」".into()));
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "ask", "points": points, "done": done }),
            ))
        }
        Some(o) => Err(format!("权计：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 自立（钟会，觉醒）：准备阶段权 ≥3 → 回复 1 体力或摸 2（满血隐藏回复）→ 减 1 上限 → 授予排异
// ---------------------------------------------------------------------------

fn zili(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.phase != Phase::Prepare || g.counter(&format!("awaken.{actor}.zili")) > 0 {
        return Ok(Step::Done);
    }
    if mark_cards(g, actor, QUAN).len() < 3 {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    let data = json!({ "step": "choose" });
    if step == "choose" {
        return match take_answer(g, "zili") {
            None => {
                let full = g.seats[actor].hp >= g.seats[actor].max_hp;
                let mut b = PendingBuilder::new(actor, "choose").skill("zili");
                if !full {
                    b = b.option("use:hp", "回复 1 点体力");
                }
                b = b.option("use:draw", "摸两张牌");
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(data)
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use:hp" => {
                g.bump(&format!("awaken.{actor}.zili"));
                flow::heal(g, actor, 1, "自立");
                let _ = flow::lose_max_hp(g, actor, 1, "自立");
                skills::grant(g, actor, "paiyi");
                Ok(Step::Finish("自立：觉醒（回复并授予排异）".into()))
            }
            Some(a) if a == "use:draw" => {
                g.bump(&format!("awaken.{actor}.zili"));
                flow::draw(g, actor, 2);
                let _ = flow::lose_max_hp(g, actor, 1, "自立");
                skills::grant(g, actor, "paiyi");
                Ok(Step::Finish("自立：觉醒（摸两张并授予排异）".into()))
            }
            Some(o) => Err(format!("自立：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 峻刑（满宠）：出牌阶段限一次，弃任意张手牌 → 目标弃异类型手牌或翻面摸等量
// ---------------------------------------------------------------------------

fn can_junxing(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.counter(&phase_key(seat, "junxing")) == 0
        && !g.hand(seat).is_empty()
}

fn junxing(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    if step == "pick" {
        if !can_junxing(g, actor) {
            return Ok(Step::Done);
        }
        let picked: Vec<String> = ask
            .data
            .get("picked")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let own = g.hand(actor);
        return match take_answer(g, "junxing") {
            None => {
                let mut b = PendingBuilder::new(actor, "multi_select").skill("junxing");
                for id in &own {
                    if picked.contains(id) {
                        continue;
                    }
                    b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                if !picked.is_empty() {
                    b = b.option("ok", "确定并选择目标");
                }
                Ok(Step::Need(
                    b.escape_with("cancel", "取消")
                        .data(json!({ "step": "pick", "picked": picked }))
                        .build(),
                ))
            }
            Some(a) if a == "cancel" => Ok(Step::Done),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own.contains(&id) {
                    return Err("峻刑：只能弃置自己的手牌".into());
                }
                let mut next = picked.clone();
                next.push(id);
                Ok(Step::Next(
                    Stage::MultiSelect,
                    json!({ "step": "pick", "picked": next }),
                ))
            }
            Some(a) if a == "ok" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "picked": picked }),
            )),
            Some(o) => Err(format!("峻刑：未知选项 {o}")),
        };
    }
    if step == "target" {
        let picked: Vec<String> = ask
            .data
            .get("picked")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let others = alive_others(g, actor);
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "junxing") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("junxing");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target", "picked": picked }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "峻刑：非法目标")?;
                if !others.contains(&t) {
                    return Err("峻刑：目标须为其他存活角色".into());
                }
                for id in &picked {
                    if own_cards(g, actor).contains(id) {
                        flow::discard(g, actor, id, "峻刑");
                    }
                }
                g.bump(&phase_key(actor, "junxing"));
                let types: Vec<Category> =
                    picked.iter().filter_map(|id| card_category(g, id)).collect();
                Ok(Step::Next(
                    Stage::CardOpt,
                    json!({ "step": "choice", "target": t, "n": picked.len(), "types": types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>() }),
                ))
            }
            Some(o) => Err(format!("峻刑：未知选项 {o}")),
        };
    }
    if step == "discard_card" {
        let Some(target) = seat_of(ask, "target") else {
            return Ok(Step::Done);
        };
        let types: Vec<String> = ask
            .data
            .get("types")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let cands: Vec<String> = g
            .hand(target)
            .into_iter()
            .filter(|id| {
                card_category(g, id)
                    .map(|c| !types.contains(&format!("{c:?}")))
                    .unwrap_or(false)
            })
            .collect();
        return match take_answer(g, "junxing") {
            None => Ok(must_pick(
                g,
                target,
                "junxing",
                "pick_own",
                "弃置",
                &cands,
                json!({ "step": "discard_card", "target": target, "types": types }),
            )),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !cands.contains(&id) {
                    return Err("峻刑：只能弃置与所弃牌类型不同的手牌".into());
                }
                flow::discard(g, target, &id, "峻刑");
                Ok(Step::Finish("峻刑：目标弃置一张手牌".into()))
            }
            Some(o) => Err(format!("峻刑：未知选项 {o}")),
        };
    }
    // 目标二选一：弃异类型手牌 / 翻面并摸等量。
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let n = ask.data.get("n").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let types: Vec<String> = ask
        .data
        .get("types")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    match take_answer(g, "junxing") {
        None => {
            let cands: Vec<String> = g
                .hand(target)
                .into_iter()
                .filter(|id| {
                    card_category(g, id)
                        .map(|c| !types.contains(&format!("{c:?}")))
                        .unwrap_or(false)
                })
                .collect();
            let mut b = PendingBuilder::new(target, "choice").skill("junxing");
            if !cands.is_empty() {
                b = b.option("discard", "弃置一张与你弃牌类型不同的手牌");
            }
            b = b.option("flip", format!("翻面，然后摸 {n} 张"));
            Ok(Step::Need(
                b.escape_mandatory("（必须选择一项）")
                    .data(json!({ "step": "choice", "target": target, "n": n, "types": types }))
                    .build(),
            ))
        }
        Some(a) if a == "discard" => Ok(Step::Next(
            Stage::CardOpt,
            json!({ "step": "discard_card", "target": target, "types": types }),
        )),
        Some(a) if a == "flip" => {
            skills::flip_seat(g, target);
            flow::draw(g, target, n);
            Ok(Step::Finish("峻刑：翻面并摸等量张".into()))
        }
        Some(o) => Err(format!("峻刑：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 御策（满宠）：受伤后展示一张手牌 → 来源弃异类型手牌，或你回复 1 点体力
// ---------------------------------------------------------------------------

fn yuce(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let src = seat_of(ask, "by").or_else(|| seat_of(ask, "target"));
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("pick");
    if step == "pick" {
        let own = g.hand(actor);
        if own.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "yuce") {
            None => Ok(must_pick(
                g,
                actor,
                "yuce",
                "pick_own",
                "展示",
                &own,
                json!({ "step": "pick", "by": src }),
            )),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own.contains(&id) {
                    return Err("御策：只能展示自己的手牌".into());
                }
                g.reveal(&[id.clone()], "御策");
                offer_yuce_choice(g, actor, src, &id)
            }
            Some(o) => Err(format!("御策：未知选项 {o}")),
        };
    }
    if step == "discard_card" {
        let Some(src) = src else {
            return Ok(Step::Done);
        };
        let shown = card_of(ask, "shown").unwrap_or_default();
        let t = card_category(g, &shown);
        let cands: Vec<String> = g
            .hand(src)
            .into_iter()
            .filter(|id| card_category(g, id).map(|c| Some(c) != t).unwrap_or(false))
            .collect();
        return match take_answer(g, "yuce") {
            None => Ok(must_pick(
                g,
                src,
                "yuce",
                "pick_own",
                "弃置",
                &cands,
                json!({ "step": "discard_card", "by": src, "shown": shown }),
            )),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !cands.contains(&id) {
                    return Err("御策：只能弃置与展示牌类型不同的手牌".into());
                }
                flow::discard(g, src, &id, "御策");
                g.conceal(&[shown], "御策·结算完成");
                Ok(Step::Finish("御策：来源弃置一张手牌".into()))
            }
            Some(o) => Err(format!("御策：未知选项 {o}")),
        };
    }
    // choice：来源（若有手牌）选择弃异类型手牌，否则/放弃则你回复 1。
    let shown = card_of(ask, "shown").unwrap_or_default();
    let Some(src) = src else {
        return Ok(heal_and_finish(g, actor, &shown));
    };
    if g.hand(src).is_empty() {
        return Ok(heal_and_finish(g, actor, &shown));
    }
    match take_answer(g, "yuce") {
        None => {
            let t = card_category(g, &shown);
            let mut b = PendingBuilder::new(src, "choice").skill("yuce");
            let cands: Vec<String> = g
                .hand(src)
                .into_iter()
                .filter(|id| card_category(g, id).map(|c| Some(c) != t).unwrap_or(false))
                .collect();
            if !cands.is_empty() {
                b = b.option("discard", "弃置一张不同类型的手牌");
            }
            b = b.option("heal", "令其回复 1 点体力");
            Ok(Step::Need(
                b.escape_mandatory("（必须选择一项）")
                    .data(json!({ "step": "choice", "by": src, "shown": shown }))
                    .build(),
            ))
        }
        Some(a) if a == "heal" => Ok(heal_and_finish(g, actor, &shown)),
        Some(a) if a == "discard" => Ok(Step::Next(
            Stage::CardOpt,
            json!({ "step": "discard_card", "by": src, "shown": shown }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            flow::discard(g, src, &id, "御策");
            g.conceal(&[shown], "御策·结算完成");
            Ok(Step::Finish("御策：来源弃置一张手牌".into()))
        }
        Some(o) => Err(format!("御策：未知选项 {o}")),
    }
}

fn offer_yuce_choice(g: &mut Game, actor: usize, src: Option<usize>, shown: &str) -> Result<Step, String> {
    let Some(src) = src else {
        return Ok(heal_and_finish(g, actor, shown));
    };
    let t = card_category(g, shown);
    let has_diff = g
        .hand(src)
        .iter()
        .any(|id| card_category(g, id).map(|c| Some(c) != t).unwrap_or(false));
    if !has_diff {
        return Ok(heal_and_finish(g, actor, shown));
    }
    Ok(Step::Next(
        Stage::Choose,
        json!({ "step": "choice", "by": src, "shown": shown }),
    ))
}

fn heal_and_finish(g: &mut Game, actor: usize, shown: &str) -> Step {
    if !shown.is_empty() {
        g.conceal(&[shown.to_string()], "御策·结算完成");
    }
    flow::heal(g, actor, 1, "御策");
    Step::Finish("御策：回复 1 点体力".into())
}

// ---------------------------------------------------------------------------
// 龙吟（关平）：一名角色于其出牌阶段使用【杀】时，可弃一张牌令此杀不计次数；红色则摸 1
// ---------------------------------------------------------------------------

fn longyin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(user) = seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    if g.turn != user || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    // 同场多个龙吟：只询问从使用者左手侧开始的第一个拥有者。
    let n = g.seats.len();
    let first = (1..=n)
        .map(|k| (user + k) % n)
        .find(|&s| skills::active(g, s, "longyin") && skills::enabled("longyin"));
    if first != Some(actor) {
        return Ok(Step::Done);
    }
    let own = own_cards(g, actor);
    if own.is_empty() {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "longyin") {
            None => Ok(trigger_ask(
                actor,
                "longyin",
                "龙吟：弃一张牌，令此【杀】不计入本阶段次数",
                json!({ "step": "ask", "by": user, "card": card }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "by": user, "card": card }),
            )),
            Some(o) => Err(format!("龙吟：未知选项 {o}")),
        };
    }
    match take_answer(g, "longyin") {
        None => Ok(must_pick(
            g,
            actor,
            "longyin",
            "pick_own",
            "弃置",
            &own,
            json!({ "step": "pick", "by": user, "card": card }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) {
                return Err("龙吟：只能弃置自己的手牌/装备".into());
            }
            flow::discard(g, actor, &id, "龙吟");
            super::super::card_play::declare_sha_not_counted(g, user, &card);
            let red = g
                .card_defs
                .get(&card)
                .map(|c| c.suit.is_red())
                .unwrap_or(false);
            if red {
                flow::draw(g, actor, 1);
            }
            Ok(Step::Finish("龙吟：此【杀】不计次数".into()))
        }
        Some(o) => Err(format!("龙吟：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 排异（授予技）：出牌阶段限一次，移去一张「权」→ 一名角色摸 2；其手牌数 > 你则造成 1 点伤害
// ---------------------------------------------------------------------------

fn can_paiyi(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.counter(&phase_key(seat, "paiyi")) == 0
        && !mark_cards(g, seat, QUAN).is_empty()
        && !(0..g.seats.len()).all(|t| !g.seats[t].alive)
}

fn paiyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_paiyi(g, actor) {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        let alive: Vec<usize> = (0..g.seats.len()).filter(|&t| g.seats[t].alive).collect();
        return match take_answer(g, "paiyi") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("paiyi");
                for t in &alive {
                    b = b.option(format!("seat:{t}"), format!("令 #{} 摸两张", t + 1));
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "target" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "排异：非法目标")?;
                if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                    return Err("排异：目标须存活".into());
                }
                // 移去一张「权」（进弃牌堆）。
                if let Some(quan) = mark_cards(g, actor, QUAN).first().cloned() {
                    super::super::card_play::to_discard_with(g, &quan, "排异", Some(actor));
                    let ev = super::super::event::Event::card(
                        EventKind::CardToDiscard,
                        Some(actor),
                        quan,
                        Some("排异".to_string()),
                    );
                    let e = ev.snapshot(g);
                    g.push_event(e);
                }
                g.bump(&phase_key(actor, "paiyi"));
                flow::draw(g, t, 2);
                if g.hand_size(t) > g.hand_size(actor) {
                    let spec = DamageSpec {
                        from: Some(actor),
                        target: t,
                        amount: 1,
                        kind: DamageKind::Normal,
                        source: SourceKind::Skill,
                    };
                    let armor = gear::armor_of(g, t);
                    flow::damage(g, spec, armor, 0);
                }
                Ok(Step::Finish("排异：令其摸两张".into()))
            }
            Some(o) => Err(format!("排异：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}
