//! B2 技能（风林火山·SP-2：8 将 / 15 技能 + 授予技 清侧/怀柔 + 临时技 集智）。
//!
//! 依据 `docs/games/sanguosha/align/B2.json` 的 `agreed`（权威）与 19 条裁定。
//! 口径要点：
//! - **破势**按原文补齐：准备阶段（装备栏均被废除 **或** 体力=1）→ 减 1 上限 → 废除全部装备栏
//!   （栏内牌进弃牌堆）→ 摸至体力上限 → 失去决堰 → 授予怀柔。
//! - **废除类只共享状态原语**（`Seat.abolished_slots` / `judge_abolished`）：
//!   决堰 = 限一次/单槽/本回合效果 + 永久废除该槽；破势 = 觉醒全槽；雄乱 = 装备区 + 判定区。
//! - 触发类可选技能一律先询问「发动/放弃」；选牌/选目标用 `escape_mandatory`（不得代选）。

use serde_json::{json, Value};

use super::super::cards;
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{DamageKind, DamageSpec, Game, Phase, SourceKind, Zone};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

/// 武将牌上的持牌标记名（明任/箜声/征荣）。
pub const REN: &str = "ren";
pub const KONG: &str = "kongsheng";
pub const RONG: &str = "rong";

pub static SKILLS: &[SkillDef] = &[
    // —— 卢植 ——
    SkillDef {
        code: "mingren",
        name: "明任",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[
                Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
                Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
            ],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "zhenliang",
        name: "贞良",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: true,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            Face {
                window: Window::OtherTurn,
                // 「使用或打出的牌进入弃牌堆时」：使用/打出路由后判定（牌将在本次结算后落弃牌堆）。
                subscriptions: &[
                    Subscription { event: EventKind::CardUsed, subject: Subject::By },
                    Subscription { event: EventKind::CardPlayed, subject: Subject::By },
                ],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    // —— 郝昭 ——
    SkillDef {
        code: "zhengu",
        name: "镇骨",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 陈到 ——
    SkillDef {
        code: "wanglie",
        name: "往烈",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[Hook::UsePre],
            can_active: false,
        }],
    },
    // —— 诸葛瞻 ——
    SkillDef {
        code: "zuilun",
        name: "罪论",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "fuyin",
        name: "父荫",
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
    // —— 周妃 ——
    SkillDef {
        code: "liangyin",
        name: "良姻",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[
                Subscription { event: EventKind::CardOutOfGame, subject: Subject::Any },
                Subscription { event: EventKind::CardIntoGame, subject: Subject::Any },
            ],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "kongsheng",
        name: "箜声",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[
                Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
                Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
            ],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 毌丘俭 ——
    SkillDef {
        code: "zhengrong",
        name: "征荣",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::By }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "hongju",
        name: "鸿举",
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
    // —— 陆抗 ——
    SkillDef {
        code: "qianjie",
        name: "谦节",
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
        code: "jueyan",
        name: "决堰",
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
        code: "poshi",
        name: "破势",
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
    // —— 张绣 ——
    SkillDef {
        code: "xiongluan",
        name: "雄乱",
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
        code: "congjian",
        name: "从谏",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 授予技（一等公民；破势/鸿举授予）——
    SkillDef {
        code: "huairou",
        name: "怀柔",
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
    SkillDef {
        code: "qingce",
        name: "清侧",
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
    // —— 临时技（决堰·宝物：本回合获得「集智」）——
    SkillDef {
        code: "jizhi",
        name: "集智",
        kind: SkillKind::Normal,
        grant_only: true,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::By }],
            hooks: &[],
            can_active: false,
        }],
    },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "mingren" => mingren,
        "zhenliang" => zhenliang,
        "zhengu" => zhengu,
        "wanglie" => wanglie,
        "zuilun" => zuilun,
        "fuyin" => noop,
        "liangyin" => liangyin,
        "kongsheng" => kongsheng,
        "zhengrong" => zhengrong,
        "hongju" => hongju,
        "qianjie" => noop,
        "jueyan" => jueyan,
        "poshi" => poshi,
        "xiongluan" => xiongluan,
        "congjian" => congjian,
        "huairou" => huairou,
        "qingce" => qingce,
        "jizhi" => jizhi,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "zhenliang" => can_zhenliang,
        "jueyan" => can_jueyan,
        "xiongluan" => can_xiongluan,
        "huairou" => can_huairou,
        "qingce" => can_qingce,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// 数值/查询单出口的落点（`skills::*` 委托到这里）
// ---------------------------------------------------------------------------

/// 【杀】次数加成（决堰·武器：本回合 +3）。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    g.counter(&format!("turn.{seat}.jueyan_sha"))
}

/// 【杀】是否无限（雄乱：本回合无次数限制）。
pub fn sha_unlimited(g: &Game, seat: usize) -> bool {
    g.counter(&format!("turn.{seat}.xiongluan")) > 0
}

/// 手牌上限加成（决堰·防具：本回合 +3）。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    g.counter(&format!("turn.{seat}.jueyan_hand_limit"))
}

/// 本回合是否无视距离（决堰·坐骑 / 雄乱）。
pub fn ignore_distance(g: &Game, seat: usize) -> bool {
    g.counter(&format!("turn.{seat}.jueyan_no_distance")) > 0
        || g.counter(&format!("turn.{seat}.xiongluan")) > 0
}

/// 雄乱：此使用者对本回合的目标「对方不能响应」。
pub fn no_response_for(g: &Game, user: usize, target: usize) -> bool {
    g.counter(&format!("turn.{user}.xiongluan_target.{target}")) > 0
}

/// 谦节：不能成为延时锦囊 / 铁索连环的目标。
pub fn deny_target_card(g: &Game, _user: usize, target: usize, code: &str) -> bool {
    if !skills::enabled("qianjie") || !skills::active(g, target, "qianjie") {
        return false;
    }
    let Some(def) = cards::def(code) else { return false };
    def.tags.delayed || def.tags.lianhuan
}

/// 谦节：不能成为其他角色拼点的目标。
pub fn deny_pindian(g: &Game, target: usize) -> bool {
    skills::enabled("qianjie") && skills::active(g, target, "qianjie")
}

/// 父荫：每回合首次成为其他角色【杀】/【决斗】目标后，若其手牌数 ≥ 你，此牌对你无效。
pub fn card_ineffective_for(g: &mut Game, user: usize, target: usize, card: &str) -> bool {
    if !skills::enabled("fuyin") || !skills::active(g, target, "fuyin") {
        return false;
    }
    let Some(def) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) else {
        return false;
    };
    if !(def.tags.sha || def.tags.duel) {
        return false;
    }
    let key = format!("turn.{target}.fuyin");
    if g.counter(&key) > 0 {
        return false;
    }
    if g.hand_size(user) < g.hand_size(target) {
        return false;
    }
    g.bump(&key);
    true
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
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

fn alive_others(g: &Game, seat: usize) -> Vec<usize> {
    (0..g.seats.len()).filter(|&i| i != seat && g.seats[i].alive).collect()
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
    verb: &str,
    cands: &[String],
    data: Value,
) -> Step {
    let mut b = PendingBuilder::new(actor, "pick_own").skill(skill);
    for id in cands {
        b = b.option(format!("card:{id}"), format!("{verb}【{}】", card_label(g, id)));
    }
    Step::Need(b.escape_mandatory("（必选一张）").data(data).build())
}

/// 武将牌上某标记的牌（按 id 排序）。
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

fn to_mark(g: &mut Game, seat: usize, name: &str, id: &str, by: usize, cause: &str) {
    if let Some(zone) = g.cards.get(id).cloned() {
        if let Some(owner) = zone.seat() {
            let mut lost = super::super::event::Event::card(
                EventKind::CardLost,
                Some(owner),
                id.to_string(),
                Some(cause.to_string()),
            );
            lost.by = Some(by);
            lost.from = Some(owner);
            let ev = lost.snapshot(g);
            g.push_event(ev);
        }
    }
    g.cards.insert(id.to_string(), Zone::Mark(seat, name.to_string()));
}

fn mark_to_hand(g: &mut Game, seat: usize, id: &str, cause: &str) {
    g.cards.insert(id.to_string(), Zone::Hand(seat));
    let mut ev = super::super::event::Event::card(
        EventKind::CardGained,
        Some(seat),
        id.to_string(),
        Some(cause.to_string()),
    );
    ev.by = Some(seat);
    ev.from = Some(seat);
    let e = ev.snapshot(g);
    g.push_event(e);
}

fn card_color(g: &Game, id: &str) -> Option<bool> {
    g.card_defs.get(id).map(|c| c.suit.is_red())
}

fn awakened(g: &Game, seat: usize, key: &str) -> bool {
    g.counter(&format!("awaken.{seat}.{key}")) > 0
}

fn mark_awakened(g: &mut Game, seat: usize, key: &str) {
    g.bump(&format!("awaken.{seat}.{key}"));
}

// ---------------------------------------------------------------------------
// 明任（卢植）：首个准备阶段摸 1 并必选 1 张手牌为「任」；结束阶段可一进一出替换
// ---------------------------------------------------------------------------

fn mingren(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let phase = g.phase;
    if phase == Phase::Prepare {
        if g.counter(&format!("mingren.{actor}.init")) > 0 {
            return Ok(Step::Done);
        }
        let own = own_cards(g, actor);
        if own.is_empty() {
            // 仍需摸 1（原文：游戏开始时摸 1 张，然后置 1 张为任）。
            flow::draw(g, actor, 1);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("init_draw");
        if let Some(ans) = take_answer(g, "mingren") {
            let Some(id) = ans.strip_prefix("card:") else {
                return Err("明任：未知选项".into());
            };
            let id = id.to_string();
            if !own_cards(g, actor).contains(&id) {
                return Err("明任：只能选自己的手牌".into());
            }
            to_mark(g, actor, REN, &id, actor, "明任");
            g.bump(&format!("mingren.{actor}.init"));
            return Ok(Step::Finish("明任：置「任」".into()));
        }
        if step == "init_draw" {
            if g.counter(&format!("mingren.{actor}.drawn")) == 0 {
                g.bump(&format!("mingren.{actor}.drawn"));
                flow::draw(g, actor, 1);
            }
            let own = own_cards(g, actor);
            if own.is_empty() {
                g.bump(&format!("mingren.{actor}.init"));
                return Ok(Step::Done);
            }
            return Ok(must_pick(
                g,
                actor,
                "mingren",
                "置于武将牌上作为「任」",
                &own,
                json!({ "step": "init_pick" }),
            ));
        }
        match take_answer(g, "mingren") {
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own_cards(g, actor).contains(&id) {
                    return Err("明任：只能选自己的手牌".into());
                }
                to_mark(g, actor, REN, &id, actor, "明任");
                g.bump(&format!("mingren.{actor}.init"));
                Ok(Step::Finish("明任：置「任」".into()))
            }
            _ => Err("明任：未知选项".into()),
        }
    } else {
        // 结束阶段：可用手牌替换「任」（必须一进一出）。
        let ren = mark_cards(g, actor, REN);
        let own = own_cards(g, actor);
        if ren.is_empty() || own.is_empty() {
            return Ok(Step::Done);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("swap_ask");
        if step == "swap_ask" {
            return match take_answer(g, "mingren") {
                None => Ok(trigger_ask(
                    actor,
                    "mingren",
                    "明任：用手牌替换「任」",
                    json!({ "step": "swap_ask" }),
                )),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "swap_pick" }),
                )),
                Some(o) => Err(format!("明任：未知选项 {o}")),
            };
        }
        match take_answer(g, "mingren") {
            None => Ok(must_pick(
                g,
                actor,
                "mingren",
                "置为新的「任」",
                &own,
                json!({ "step": "swap_pick" }),
            )),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own_cards(g, actor).contains(&id) {
                    return Err("明任：只能用自己的手牌替换".into());
                }
                for old in ren {
                    mark_to_hand(g, actor, &old, "明任·取回");
                }
                to_mark(g, actor, REN, &id, actor, "明任");
                Ok(Step::Finish("明任：替换「任」".into()))
            }
            Some(o) => Err(format!("明任：未知选项 {o}")),
        }
    }
}

// ---------------------------------------------------------------------------
// 贞良（卢植，转换技）：阳=弃同色牌造成 1 点伤害；阴=回合外同色牌进弃牌堆→令一名角色摸 1
// ---------------------------------------------------------------------------

fn can_zhenliang(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.counter(&phase_key(seat, "zhenliang")) == 0
        && !mark_cards(g, seat, REN).is_empty()
        && own_cards(g, seat)
            .iter()
            .any(|id| same_color_as_ren(g, seat, id))
        && !alive_others(g, seat).is_empty()
}

fn same_color_as_ren(g: &Game, seat: usize, id: &str) -> bool {
    let ren = mark_cards(g, seat, REN);
    let Some(ren_color) = ren.first().and_then(|r| card_color(g, r)) else {
        return false;
    };
    card_color(g, id).map(|c| c == ren_color).unwrap_or(false)
}

fn zhenliang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let face = ask.data.get("face").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if face == 1 {
        // 阴：回合外，使用/打出的同色牌进入弃牌堆时。
        let Some(card) = card_of(ask, "card") else {
            return Ok(Step::Done);
        };
        if g.turn == actor || !same_color_as_ren(g, actor, &card) {
            return Ok(Step::Done);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
        return match (step, take_answer(g, "zhenliang")) {
            ("ask", None) => Ok(trigger_ask(
                actor,
                "zhenliang",
                "贞良（阴）：令一名角色摸一张牌",
                json!({ "step": "ask", "face": 1, "card": card }),
            )),
            ("ask", Some(a)) if a == "pass" => Ok(Step::Done),
            ("ask", Some(a)) if a == "use" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "face": 1, "card": card }),
            )),
            ("target", None) => {
                let alive: Vec<usize> = (0..g.seats.len()).filter(|&i| g.seats[i].alive).collect();
                let mut b = PendingBuilder::new(actor, "target").skill("zhenliang");
                for t in &alive {
                    b = b.option(format!("seat:{t}"), format!("令 #{} 摸一张", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target", "face": 1, "card": card }))
                        .build(),
                ))
            }
            ("target", Some(a)) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "贞良：非法目标")?;
                if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                    return Err("贞良：目标须存活".into());
                }
                flow::draw(g, t, 1);
                skills::flip_face(g, actor, "zhenliang");
                Ok(Step::Finish("贞良（阴）：令其摸一张".into()))
            }
            _ => Err("贞良：未知选项".into()),
        };
    }
    // 阳：出牌阶段限一次，弃同色牌 → 攻击范围内一名其他角色受 1 点伤害。
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        let reach = gear::attack_range(g, actor) as usize;
        let cands: Vec<usize> = alive_others(g, actor)
            .into_iter()
            .filter(|&t| gear::distance(g, actor, t) <= reach)
            .collect();
        return match take_answer(g, "zhenliang") {
            None => {
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                let mut b = PendingBuilder::new(actor, "target").skill("zhenliang");
                for t in &cands {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "贞良：非法目标")?;
                if !cands.contains(&t) {
                    return Err("贞良：目标须在攻击范围内".into());
                }
                let own: Vec<String> = own_cards(g, actor)
                    .into_iter()
                    .filter(|id| same_color_as_ren(g, actor, id))
                    .collect();
                if own.is_empty() {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "target": t }),
                ))
            }
            Some(o) => Err(format!("贞良：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let own: Vec<String> = own_cards(g, actor)
        .into_iter()
        .filter(|id| same_color_as_ren(g, actor, id))
        .collect();
    if own.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "zhenliang") {
        None => Ok(must_pick(
            g,
            actor,
            "zhenliang",
            "弃置（与「任」同色）",
            &own,
            json!({ "step": "pick", "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) {
                return Err("贞良：须弃与「任」同色的牌".into());
            }
            flow::discard(g, actor, &id, "贞良");
            let spec = DamageSpec {
                from: Some(actor),
                target,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Skill,
            };
            let armor = gear::armor_of(g, target);
            flow::damage(g, spec, armor, 0);
            g.bump(&phase_key(actor, "zhenliang"));
            skills::flip_face(g, actor, "zhenliang");
            Ok(Step::Finish("贞良（阳）：造成 1 点伤害".into()))
        }
        Some(o) => Err(format!("贞良：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 决堰（陆抗）：限一次；废除一个未废除装备栏（永久）+ 本回合效果
// ---------------------------------------------------------------------------

const SLOTS: [super::super::Slot; 5] = [
    super::super::Slot::Weapon,
    super::super::Slot::Armor,
    super::super::Slot::HorseOffense,
    super::super::Slot::HorseDefense,
    super::super::Slot::Treasure,
];

fn open_slots(g: &Game, seat: usize) -> Vec<super::super::Slot> {
    SLOTS
        .into_iter()
        .filter(|s| !g.slot_abolished(seat, *s))
        .collect()
}

fn can_jueyan(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.counter(&phase_key(seat, "jueyan")) == 0
        && !open_slots(g, seat).is_empty()
}

fn slot_label(slot: super::super::Slot) -> &'static str {
    match slot {
        super::super::Slot::Weapon => "武器",
        super::super::Slot::Armor => "防具",
        super::super::Slot::HorseOffense => "坐骑（进攻）",
        super::super::Slot::HorseDefense => "坐骑（防御）",
        super::super::Slot::Treasure => "宝物",
    }
}

fn slot_from_id(id: &str) -> Option<super::super::Slot> {
    match id {
        "weapon" => Some(super::super::Slot::Weapon),
        "armor" => Some(super::super::Slot::Armor),
        "offense" => Some(super::super::Slot::HorseOffense),
        "defense" => Some(super::super::Slot::HorseDefense),
        "treasure" => Some(super::super::Slot::Treasure),
        _ => None,
    }
}

fn slot_id(slot: super::super::Slot) -> &'static str {
    match slot {
        super::super::Slot::Weapon => "weapon",
        super::super::Slot::Armor => "armor",
        super::super::Slot::HorseOffense => "offense",
        super::super::Slot::HorseDefense => "defense",
        super::super::Slot::Treasure => "treasure",
    }
}

fn jueyan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_jueyan(g, actor) {
        return Ok(Step::Done);
    }
    let slots = open_slots(g, actor);
    match take_answer(g, "jueyan") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("jueyan");
            for s in &slots {
                b = b.option(format!("slot:{}", slot_id(*s)), format!("废除{}栏", slot_label(*s)));
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "choose" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("slot:") => {
            let slot = slot_from_id(&a[5..]).ok_or("决堰：非法栏位")?;
            if !slots.contains(&slot) {
                return Err("决堰：该栏位已废除".into());
            }
            // 永久废除 + 栏内牌进弃牌堆。
            if let Some(old) = gear::equip_in(g, actor, slot).cloned() {
                flow::discard(g, actor, &old, "决堰");
            }
            g.abolish_slot(actor, slot);
            match slot {
                super::super::Slot::Weapon => {
                    let k = turn_key(actor, "jueyan_sha");
                    let v = g.counter(&k) + 3;
                    g.set_counter(&k, v);
                }
                super::super::Slot::Armor => {
                    let k = turn_key(actor, "jueyan_hand_limit");
                    let v = g.counter(&k) + 3;
                    g.set_counter(&k, v);
                }
                super::super::Slot::HorseOffense | super::super::Slot::HorseDefense => {
                    g.set_counter(&turn_key(actor, "jueyan_no_distance"), 1);
                }
                super::super::Slot::Treasure => {
                    g.set_counter(&turn_key(actor, "jizhi"), 1);
                }
            }
            g.bump(&phase_key(actor, "jueyan"));
            Ok(Step::Finish(format!("决堰：废除{}栏", slot_label(slot))))
        }
        Some(o) => Err(format!("决堰：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 破势（陆抗，觉醒）：准备阶段（全栏废除 或 体力=1）→ 减上限/全废除/摸至上限/失决堰/授怀柔
// ---------------------------------------------------------------------------

fn poshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.phase != Phase::Prepare || awakened(g, actor, "poshi") {
        return Ok(Step::Done);
    }
    let all_abolished = open_slots(g, actor).is_empty();
    let hp_one = g.seats.get(actor).map(|s| s.hp == 1).unwrap_or(false);
    if !(all_abolished || hp_one) {
        return Ok(Step::Done);
    }
    mark_awakened(g, actor, "poshi");
    let _ = flow::lose_max_hp(g, actor, 1, "破势");
    for slot in SLOTS {
        if let Some(old) = gear::equip_in(g, actor, slot).cloned() {
            flow::discard(g, actor, &old, "破势");
        }
        g.abolish_slot(actor, slot);
    }
    let max_hp = g.seats[actor].max_hp as usize;
    flow::fill_hand(g, actor, max_hp);
    skills::revoke(g, actor, "jueyan");
    skills::grant(g, actor, "huairou");
    Ok(Step::Finish(
        "破势：觉醒（减上限/全栏废除/摸至上限/失去决堰/获得怀柔）".into(),
    ))
}

// ---------------------------------------------------------------------------
// 雄乱（张绣，限定技）：废除装备区+判定区 → 本回合对目标不可响应且无距离/次数限制
// ---------------------------------------------------------------------------

fn can_xiongluan(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && !skills::limited_used(g, seat, "xiongluan")
        && !alive_others(g, seat).is_empty()
}

fn xiongluan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_xiongluan(g, actor) {
        return Ok(Step::Done);
    }
    match take_answer(g, "xiongluan") {
        None => {
            let others = alive_others(g, actor);
            let mut b = PendingBuilder::new(actor, "target").skill("xiongluan");
            for t in &others {
                b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "target" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "雄乱：非法目标")?;
            if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) || t == actor {
                return Err("雄乱：目标非法".into());
            }
            skills::mark_limited_used(g, actor, "xiongluan");
            // 废除装备区（全部栏）与判定区。
            for slot in SLOTS {
                if let Some(old) = gear::equip_in(g, actor, slot).cloned() {
                    flow::discard(g, actor, &old, "雄乱");
                }
                g.abolish_slot(actor, slot);
            }
            super::super::card_play::abolish_judge_zone(g, actor);
            g.set_counter(&turn_key(actor, "xiongluan"), 1);
            g.set_counter(&format!("turn.{actor}.xiongluan_target.{t}"), 1);
            Ok(Step::Finish("雄乱：废除装备区与判定区".into()))
        }
        Some(o) => Err(format!("雄乱：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 授予技：怀柔 / 清侧 / 集智（临时）
// ---------------------------------------------------------------------------

fn can_huairou(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && own_cards(g, seat)
            .iter()
            .any(|id| g.card_defs.get(id).and_then(|c| cards::def(&c.def)).map(|d| d.slot.is_some()).unwrap_or(false))
}

fn huairou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_huairou(g, actor) {
        return Ok(Step::Done);
    }
    let equips: Vec<String> = own_cards(g, actor)
        .into_iter()
        .filter(|id| {
            g.card_defs
                .get(id)
                .and_then(|c| cards::def(&c.def))
                .map(|d| d.slot.is_some())
                .unwrap_or(false)
        })
        .collect();
    match take_answer(g, "huairou") {
        None => Ok(must_pick(
            g,
            actor,
            "huairou",
            "重铸（弃置并摸一张）",
            &equips,
            json!({ "step": "pick" }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !equips.contains(&id) {
                return Err("怀柔：只能重铸装备牌".into());
            }
            flow::discard(g, actor, &id, "怀柔");
            flow::draw(g, actor, 1);
            Ok(Step::Finish("怀柔：重铸一张装备牌".into()))
        }
        Some(o) => Err(format!("怀柔：未知选项 {o}")),
    }
}

fn can_qingce(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.counter(&phase_key(seat, "qingce")) == 0
        && !alive_others(g, seat).is_empty()
        && (0..g.seats.len()).any(|t| g.seats[t].alive && !own_cards(g, t).is_empty())
}

fn qingce(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_qingce(g, actor) {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    if step == "target" {
        return match take_answer(g, "qingce") {
            None => {
                let cands: Vec<usize> = (0..g.seats.len())
                    .filter(|&t| g.seats[t].alive && !own_cards(g, t).is_empty())
                    .collect();
                let mut b = PendingBuilder::new(actor, "target").skill("qingce");
                for t in &cands {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "target" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "清侧：非法目标")?;
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "pick", "target": t }),
                ))
            }
            Some(o) => Err(format!("清侧：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let cands = own_cards(g, target);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "qingce") {
        None => Ok(must_pick(
            g,
            actor,
            "qingce",
            "弃置",
            &cands,
            json!({ "step": "pick", "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("清侧：只能弃置目标的一张牌".into());
            }
            flow::discard(g, actor, &id, "清侧");
            g.bump(&phase_key(actor, "qingce"));
            if gear::distance(g, target, actor) > 1 {
                flow::draw(g, actor, 1);
            }
            Ok(Step::Finish("清侧：弃其一张牌".into()))
        }
        Some(o) => Err(format!("清侧：未知选项 {o}")),
    }
}

fn jizhi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.counter(&turn_key(actor, "jizhi")) == 0 {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    if def.category != cards::Category::Trick {
        return Ok(Step::Done);
    }
    match take_answer(g, "jizhi") {
        None => Ok(trigger_ask(
            actor,
            "jizhi",
            "集智：摸一张牌",
            json!({ "step": "ask", "card": card }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, actor, 1);
            Ok(Step::Finish("集智：摸一张".into()))
        }
        Some(o) => Err(format!("集智：未知选项 {o}")),
    }
}


// ---------------------------------------------------------------------------
// 镇骨（郝昭）：结束阶段指定一名其他角色；其手牌数向郝昭看齐（两个时刻）
// ---------------------------------------------------------------------------

fn zhengu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    // 延迟项：目标回合结束时再结算一次。
    if ask.data.get("hook").and_then(|v| v.as_str()) == Some("end") {
        let Some(target) = seat_of(ask, "target") else {
            return Ok(Step::Done);
        };
        let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if g.phase != Phase::End || g.turn != target {
            return Ok(Step::Done);
        }
        return zhengu_settle(g, actor, target, x, false);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        if g.phase != Phase::End {
            return Ok(Step::Done);
        }
        return match take_answer(g, "zhengu") {
            None => Ok(trigger_ask(
                actor,
                "zhengu",
                "镇骨：指定一名其他角色（其手牌数向你手牌数看齐）",
                json!({ "step": "ask" }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::Target, json!({ "step": "target" }))),
            Some(o) => Err(format!("镇骨：未知选项 {o}")),
        };
    }
    if step == "target" {
        let others = alive_others(g, actor);
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "zhengu") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("zhengu");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "镇骨：非法目标")?;
                if !others.contains(&t) {
                    return Err("镇骨：目标须为其他存活角色".into());
                }
                let x = g.hand_size(actor);
                g.bump(&format!("zhengu.{actor}.last"));
                g.marks
                    .insert(format!("zhengu.{actor}.target"), t.to_string());
                zhengu_settle(g, actor, t, x, true)
            }
            Some(o) => Err(format!("镇骨：未知选项 {o}")),
        };
    }
    // discard 阶段：由目标逐张弃到手牌数 = X。
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let hand = g.hand(target);
    match take_answer(g, "zhengu") {
        None => {
            if hand.is_empty() {
                return zhengu_settle(g, actor, target, x, true);
            }
            let mut b = PendingBuilder::new(target, "pick_own").skill("zhengu");
            for id in &hand {
                b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必须弃至手牌数相符）")
                    .data(json!({ "step": "discard", "target": target, "x": x }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !hand.contains(&id) {
                return Err("镇骨：只能弃置其自己的手牌".into());
            }
            flow::discard(g, target, &id, "镇骨");
            zhengu_settle(g, actor, target, x, true)
        }
        Some(o) => Err(format!("镇骨：未知选项 {o}")),
    }
}

fn zhengu_settle(
    g: &mut Game,
    actor: usize,
    target: usize,
    x: usize,
    defer: bool,
) -> Result<Step, String> {
    let theirs = g.hand_size(target);
    if theirs < x {
        flow::fill_hand(g, target, x);
    } else if theirs > x {
        // 继续逐张弃（由目标选择）。
        return Ok(Step::Next(
            Stage::PickOwn,
            json!({ "step": "discard", "target": target, "x": x }),
        ));
    }
    if defer {
        g.deferred.push(super::super::deferred::Deferred::ask(
            super::super::deferred::DeferredKind::TurnEndCheck,
            "zhengu",
            Stage::Choose,
            actor,
            json!({ "hook": "end", "target": target, "x": x }),
        ));
    }
    Ok(Step::Finish(format!("镇骨：目标手牌数调整为 {x}")))
}

// ---------------------------------------------------------------------------
// 往烈（陈到）：首牌无距离（锁定）+ 主动：此牌不能被响应，然后本回合不能用牌
// ---------------------------------------------------------------------------

fn wanglie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    if g.turn != actor || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    let basic_or_trick = def.category == cards::Category::Basic
        || def.category == cards::Category::Trick;
    if !basic_or_trick {
        return Ok(Step::Done);
    }
    // ① 锁定：本阶段使用的首张牌无距离限制。
    let first_key = phase_key(actor, "wanglie_first");
    if g.counter(&first_key) == 0 {
        g.bump(&first_key);
        super::super::card_play::declare_no_distance(g, &card);
    }
    // ② 主动：令此牌不能被响应，然后本回合不能再使用牌。
    match take_answer(g, "wanglie") {
        None => Ok(trigger_ask(
            actor,
            "wanglie",
            "往烈：此牌不能被响应，然后你本回合不能再使用牌",
            json!({ "step": "use_pre", "card": card }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            super::super::card_play::declare_no_response(g, &card);
            g.freeze_hand(actor, super::super::FreezeUntil::Turn, "往烈");
            Ok(Step::Finish("往烈：此牌不能被响应，本回合不能再使用牌".into()))
        }
        Some(o) => Err(format!("往烈：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 罪论（诸葛瞻）：结束阶段观星 3 张；按条件取牌；剩余按序置顶；均不满足则双方失 1 体力
// ---------------------------------------------------------------------------

fn zuilun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("start");
    if step == "start" {
        let top = top_cards(g, 3);
        let conds = zuilun_conditions(g, actor);
        let k = conds.iter().filter(|&&c| c).count();
        if k == 0 {
            if alive_others(g, actor).is_empty() {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Target,
                json!({ "step": "penalty", "top": top, "conds": conds }),
            ));
        }
        return Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "pick", "top": top, "conds": conds, "k": k, "taken": 0 }),
        ));
    }
    if step == "penalty" {
        let others = alive_others(g, actor);
        return match take_answer(g, "zuilun") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("zuilun");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("你与其各失去 1 点体力：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "penalty" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "罪论：非法目标")?;
                if !others.contains(&t) {
                    return Err("罪论：目标须为其他存活角色".into());
                }
                flow::lose_hp(g, actor, 1, "罪论");
                flow::lose_hp(g, t, 1, "罪论");
                Ok(Step::Finish("罪论：你与其各失去 1 点体力".into()))
            }
            Some(o) => Err(format!("罪论：未知选项 {o}")),
        };
    }
    if step == "pick" {
        let top: Vec<String> = ask
            .data
            .get("top")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let k = ask.data.get("k").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let mut taken = ask.data.get("taken").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let conds: Vec<bool> = ask
            .data
            .get("conds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_bool()).collect())
            .unwrap_or_default();
        if taken >= k || top.is_empty() {
            return zuilun_order(g, actor, top, &conds);
        }
        return match take_answer(g, "zuilun") {
            None => {
                if top.len() == 1 {
                    // 合法集只有一个：直接取（无选择，不算代选）。
                    let id = top[0].clone();
                    remove_from_deck(g, &id);
                    mark_to_hand(g, actor, &id, "罪论");
                    taken += 1;
                    let rest: Vec<String> = top.into_iter().filter(|x| *x != id).collect();
                    return zuilun_order(g, actor, rest, &conds);
                }
                let mut b = PendingBuilder::new(actor, "pick_other").skill("zuilun");
                for id in &top {
                    b = b.option(format!("card:{id}"), format!("获得【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一张）")
                        .data(json!({ "step": "pick", "top": top, "conds": conds, "k": k, "taken": taken }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !top.contains(&id) {
                    return Err("罪论：只能从牌堆顶 3 张中取得".into());
                }
                remove_from_deck(g, &id);
                mark_to_hand(g, actor, &id, "罪论");
                taken += 1;
                let rest: Vec<String> = top.into_iter().filter(|x| *x != id).collect();
                if taken >= k {
                    return zuilun_order(g, actor, rest, &conds);
                }
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "pick", "top": rest, "conds": conds, "k": k, "taken": taken }),
                ))
            }
            Some(o) => Err(format!("罪论：未知选项 {o}")),
        };
    }
    if step == "order" {
        let rest: Vec<String> = ask
            .data
            .get("top")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let conds: Vec<bool> = ask
            .data
            .get("conds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_bool()).collect())
            .unwrap_or_default();
        if rest.len() != 2 {
            return Ok(Step::Done);
        }
        return match take_answer(g, "zuilun") {
            None => {
                let a = card_label(g, &rest[0]);
                let b = card_label(g, &rest[1]);
                let mut pending = PendingBuilder::new(actor, "choose").skill("zuilun");
                pending = pending.option("order:ab", format!("{a} → {b}（新牌堆顶）"));
                pending = pending.option("order:ba", format!("{b} → {a}（新牌堆顶）"));
                Ok(Step::Need(
                    pending
                        .escape_mandatory("（必须选择顺序）")
                        .data(json!({ "step": "order", "top": rest, "conds": conds }))
                        .build(),
                ))
            }
            Some(a) if a == "order:ab" || a == "order:ba" => {
                let order = if a == "order:ab" {
                    rest.clone()
                } else {
                    vec![rest[1].clone(), rest[0].clone()]
                };
                put_back_top(g, &order);
                Ok(Step::Finish("罪论：剩余牌按所选顺序置于牌堆顶".into()))
            }
            Some(o) => Err(format!("罪论：未知选项 {o}")),
        };
    }
    Ok(Step::Done)
}

fn zuilun_order(
    g: &mut Game,
    actor: usize,
    rest: Vec<String>,
    conds: &[bool],
) -> Result<Step, String> {
    if rest.len() == 2 {
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "order", "top": rest, "conds": conds }),
        ));
    }
    if rest.len() == 1 {
        put_back_top(g, &rest);
    }
    Ok(Step::Finish("罪论：结算完成".into()))
}

/// 当前牌堆顶 n 张（末尾 = 顶；返回顺序 = 从顶到底）。
fn top_cards(g: &mut Game, n: usize) -> Vec<String> {
    if g.deck.len() < n {
        flow::reshuffle(g);
    }
    let mut out: Vec<String> = g.deck.iter().rev().take(n).cloned().collect();
    if out.len() < n {
        out.truncate(out.len());
    }
    out
}

fn remove_from_deck(g: &mut Game, id: &str) {
    g.deck.retain(|c| c != id);
}

fn put_back_top(g: &mut Game, order_top_to_bottom: &[String]) {
    for id in order_top_to_bottom.iter().rev() {
        g.deck.retain(|c| c != id);
        g.deck.push(id.clone());
    }
}

fn zuilun_conditions(g: &Game, actor: usize) -> Vec<bool> {
    let damaged = g.log.iter().any(|e| {
        e.kind == EventKind::DamageAfter && e.by == Some(actor) && e.turn == g.turn
    });
    let no_discard = !g.log.iter().any(|e| {
        e.kind == EventKind::CardDiscarded && e.by == Some(actor) && e.turn == g.turn
    });
    let min_hand = {
        let mine = g.hand_size(actor);
        (0..g.seats.len())
            .filter(|&i| g.seats[i].alive)
            .all(|i| g.hand_size(i) >= mine)
    };
    vec![damaged, no_discard, min_hand]
}

// ---------------------------------------------------------------------------
// 良姻（周妃）：回合内每回合首次有牌移出/移入游戏 → 与一名其他角色各摸或各弃
// ---------------------------------------------------------------------------

fn liangyin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.turn != actor {
        return Ok(Step::Done);
    }
    let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("out");
    let first_key = turn_key(actor, &format!("liangyin_{kind}"));
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        if g.counter(&first_key) > 0 {
            return Ok(Step::Done);
        }
        let others = alive_others(g, actor);
        if others.is_empty() {
            return Ok(Step::Done);
        }
        return match take_answer(g, "liangyin") {
            None => Ok(trigger_ask(
                actor,
                "liangyin",
                "良姻：与一名其他角色各摸一张或各弃一张",
                json!({ "step": "ask", "kind": kind }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "kind": kind }),
            )),
            Some(o) => Err(format!("良姻：未知选项 {o}")),
        };
    }
    if step == "target" {
        let others = alive_others(g, actor);
        return match take_answer(g, "liangyin") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("liangyin");
                for t in &others {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target", "kind": kind }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "良姻：非法目标")?;
                if !others.contains(&t) {
                    return Err("良姻：目标须为其他存活角色".into());
                }
                g.bump(&first_key);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "mode", "kind": kind, "target": t }),
                ))
            }
            Some(o) => Err(format!("良姻：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    match take_answer(g, "liangyin") {
        None => {
            let can_discard = !g.hand(actor).is_empty() && !g.hand(target).is_empty();
            let mut b = PendingBuilder::new(actor, "choose").skill("liangyin");
            b = b.option("use:draw", "各摸一张");
            if can_discard {
                b = b.option("use:discard", "各弃一张");
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "mode", "kind": kind, "target": target }))
                    .build(),
            ))
        }
        Some(a) if a == "use:draw" => {
            flow::draw(g, actor, 1);
            flow::draw(g, target, 1);
            Ok(Step::Finish("良姻：各摸一张".into()))
        }
        Some(a) if a == "use:discard" => Ok(Step::Next(
            Stage::PickOwn,
            json!({ "step": "discard_actor", "kind": kind, "target": target }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(o) => Err(format!("良姻：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 箜声（周妃）：准备阶段置牌为「箜声」；结束阶段 ①取回非装备 ②良姻回血 ③剩余装备令其使用并失 1 体力
// ---------------------------------------------------------------------------

fn kongsheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let phase = g.phase;
    if phase == Phase::Prepare {
        let own = own_cards(g, actor);
        if own.is_empty() {
            return Ok(Step::Done);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
        let picked: Vec<String> = ask
            .data
            .get("picked")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        if step == "ask" {
            return match take_answer(g, "kongsheng") {
                None => Ok(trigger_ask(
                    actor,
                    "kongsheng",
                    "箜声：将任意张手牌/装备置于武将牌上",
                    json!({ "step": "ask" }),
                )),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::MultiSelect,
                    json!({ "step": "pick", "picked": [] }),
                )),
                Some(o) => Err(format!("箜声：未知选项 {o}")),
            };
        }
        return match take_answer(g, "kongsheng") {
            None => {
                let mut b = PendingBuilder::new(actor, "multi_select").skill("kongsheng");
                for id in own_cards(g, actor) {
                    if picked.contains(&id) {
                        continue;
                    }
                    b = b.option(format!("card:{id}"), format!("选择【{}】", card_label(g, &id)));
                }
                if !picked.is_empty() {
                    b = b.option("ok", "确定");
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
                if !own_cards(g, actor).contains(&id) {
                    return Err("箜声：只能选择自己的手牌/装备".into());
                }
                let mut next = picked.clone();
                next.push(id);
                Ok(Step::Next(
                    Stage::MultiSelect,
                    json!({ "step": "pick", "picked": next }),
                ))
            }
            Some(a) if a == "ok" => {
                for id in &picked {
                    to_mark(g, actor, KONG, id, actor, "箜声");
                }
                Ok(Step::Finish(format!("箜声：置 {} 张牌为「箜声」", picked.len())))
            }
            Some(o) => Err(format!("箜声：未知选项 {o}")),
        };
    }
    // 结束阶段固定顺序。
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("return");
    if step == "return" {
        let marks = mark_cards(g, actor, KONG);
        let mut equips = Vec::new();
        for id in marks {
            let is_equip = g
                .card_defs
                .get(&id)
                .and_then(|c| cards::def(&c.def))
                .map(|d| d.slot.is_some())
                .unwrap_or(false);
            if is_equip {
                equips.push(id);
            } else {
                mark_to_hand(g, actor, &id, "箜声·取回");
            }
        }
        if equips.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "heal_check", "equips": equips }),
        ));
    }
    if step == "heal_check" {
        // ② 良姻回血判定（X = 剩余「箜声」牌数）。
        let equips: Vec<String> = ask
            .data
            .get("equips")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let x = mark_cards(g, actor, KONG).len();
        let cands: Vec<usize> = (0..g.seats.len())
            .filter(|&t| g.seats[t].alive && g.hand_size(t) == x && x > 0)
            .collect();
        if !skills::active(g, actor, "liangyin") || cands.is_empty() {
            return Ok(Step::Next(
                Stage::Target,
                json!({ "step": "use_equip", "equips": equips }),
            ));
        }
        return match take_answer(g, "kongsheng") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("kongsheng");
                for t in &cands {
                    b = b.option(format!("heal:{t}"), format!("令 #{} 回复 1 点体力", t + 1));
                }
                Ok(Step::Need(
                    b.escape("不回血")
                        .data(json!({ "step": "heal_check", "equips": equips }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "use_equip", "equips": equips }),
            )),
            Some(a) if a.starts_with("heal:") => {
                let t: usize = a[5..].parse().map_err(|_| "箜声：非法目标")?;
                flow::heal(g, t, 1, "箜声·良姻");
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "use_equip", "equips": equips }),
                ))
            }
            Some(o) => Err(format!("箜声：未知选项 {o}")),
        };
    }
    // ③ 剩余装备令一名角色使用，然后其失去 1 点体力。
    let equips: Vec<String> = ask
        .data
        .get("equips")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if equips.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "kongsheng") {
        None => {
            let alive: Vec<usize> = (0..g.seats.len()).filter(|&t| g.seats[t].alive).collect();
            let mut b = PendingBuilder::new(actor, "target").skill("kongsheng");
            for t in &alive {
                b = b.option(format!("seat:{t}"), format!("令 #{} 使用这些装备", t + 1));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选目标）")
                    .data(json!({ "step": "use_equip", "equips": equips }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "箜声：非法目标")?;
            if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                return Err("箜声：目标须存活".into());
            }
            for id in equips {
                g.cards.insert(id.clone(), Zone::Hand(t));
                let _ = super::super::card_play::equip_from_hand(g, t, &id);
            }
            flow::lose_hp(g, t, 1, "箜声");
            Ok(Step::Finish("箜声：令其使用剩余装备，然后失去 1 点体力".into()))
        }
        Some(o) => Err(format!("箜声：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 征荣（毌丘俭）：使用【杀】/伤害锦囊指定目标后，若其手牌 ≥ 你，可取其一张为「荣」
// ---------------------------------------------------------------------------

fn zhengrong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    if !(def.tags.sha || def.tags.damage_trick) {
        return Ok(Step::Done);
    }
    if target == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    if g.hand_size(target) < g.hand_size(actor) {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "zhengrong") {
            None => Ok(trigger_ask(
                actor,
                "zhengrong",
                "征荣：取其一张牌置于武将牌上为「荣」",
                json!({ "step": "ask", "target": target }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "pick", "target": target, "card": card }),
            )),
            Some(o) => Err(format!("征荣：未知选项 {o}")),
        };
    }
    // 取荣：随机手牌 或 指定装备（先取荣，再判定原牌响应——v3 事件在结算后路由，见文档边界）。
    let hand = g.hand(target);
    let equips: Vec<String> = own_cards(g, target)
        .into_iter()
        .filter(|id| !hand.contains(id))
        .collect();
    match take_answer(g, "zhengrong") {
        None => {
            let mut b = PendingBuilder::new(actor, "pick_other").skill("zhengrong");
            if !hand.is_empty() {
                b = b.option("rand", "随机获得其一张手牌");
            }
            for id in &equips {
                b = b.option(format!("card:{id}"), format!("获得其装备【{}】", card_label(g, id)));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选一张）")
                    .data(json!({ "step": "pick", "target": target }))
                    .build(),
            ))
        }
        Some(a) if a == "rand" => {
            if hand.is_empty() {
                return Err("征荣：目标没有手牌".into());
            }
            let idx = g.rand_below(hand.len());
            let id = hand[idx].clone();
            to_mark(g, actor, RONG, &id, actor, "征荣");
            Ok(Step::Finish("征荣：随机取一张作为「荣」".into()))
        }
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !equips.contains(&id) {
                return Err("征荣：只能取目标的装备牌".into());
            }
            to_mark(g, actor, RONG, &id, actor, "征荣");
            Ok(Step::Finish("征荣：取其装备作为「荣」".into()))
        }
        Some(o) => Err(format!("征荣：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 鸿举（毌丘俭，觉醒）：准备阶段荣 ≥3 → 减 1 上限；等量手牌与「荣」交换；获得清侧
// ---------------------------------------------------------------------------

fn hongju(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.phase != Phase::Prepare {
        return Ok(Step::Done);
    }
    let rong = mark_cards(g, actor, RONG);
    let mut data = ask.data.clone();
    let step = data.get("step").and_then(|v| v.as_str()).unwrap_or("start");
    if step == "start" {
        if awakened(g, actor, "hongju") || rong.len() < 3 {
            return Ok(Step::Done);
        }
        mark_awakened(g, actor, "hongju");
        let _ = flow::lose_max_hp(g, actor, 1, "鸿举");
        data["step"] = json!("pick");
        data["picked"] = json!([]);
        return Ok(Step::Next(Stage::MultiSelect, data));
    }
    let n = rong.len();
    let picked: Vec<String> = data
        .get("picked")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    match take_answer(g, "hongju") {
        None => {
            let own = own_cards(g, actor);
            let mut b = PendingBuilder::new(actor, "multi_select").skill("hongju");
            for id in &own {
                if picked.contains(id) {
                    continue;
                }
                b = b.option(format!("card:{id}"), format!("选择【{}】", card_label(g, id)));
            }
            if picked.len() == n {
                b = b.option("ok", "确定交换");
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
            if !own_cards(g, actor).contains(&id) {
                return Err("鸿举：只能选择自己的手牌".into());
            }
            let mut next = picked.clone();
            next.push(id);
            if next.len() > n {
                return Err("鸿举：须等量交换".into());
            }
            Ok(Step::Next(
                Stage::MultiSelect,
                json!({ "step": "pick", "picked": next }),
            ))
        }
        Some(a) if a == "ok" => {
            for old in mark_cards(g, actor, RONG) {
                mark_to_hand(g, actor, &old, "鸿举·取回");
            }
            for id in &picked {
                to_mark(g, actor, RONG, id, actor, "鸿举");
            }
            skills::grant(g, actor, "qingce");
            Ok(Step::Finish("鸿举：觉醒（等量交换并授予清侧）".into()))
        }
        Some(o) => Err(format!("鸿举：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 从谏（张绣）：成为其他角色使用牌的目标后 → 交给其中一个目标一张自己的牌 → 摸 1
// ---------------------------------------------------------------------------

fn congjian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(user) = seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    if user == actor {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let targets: Vec<usize> = g
        .use_targets
        .get(&card)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|&t| t != actor && g.seats.get(t).map(|s| s.alive).unwrap_or(false))
        .collect();
    if targets.is_empty() {
        return Ok(Step::Done);
    }
    let own = own_cards(g, actor);
    if own.is_empty() {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if step == "ask" {
        return match take_answer(g, "congjian") {
            None => Ok(trigger_ask(
                actor,
                "congjian",
                "从谏：交给其中一个目标一张牌，然后摸一张",
                json!({ "step": "ask", "card": card, "by": user }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                if targets.len() == 1 {
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "give", "card": card, "by": user, "target": targets[0] }),
                    ))
                } else {
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "target", "card": card, "by": user }),
                    ))
                }
            }
            Some(o) => Err(format!("从谏：未知选项 {o}")),
        };
    }
    if step == "target" {
        return match take_answer(g, "congjian") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("congjian");
                for t in &targets {
                    b = b.option(format!("seat:{t}"), format!("交给 #{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target", "card": card, "by": user }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "从谏：非法目标")?;
                if !targets.contains(&t) {
                    return Err("从谏：只能交给此牌的目标".into());
                }
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "give", "card": card, "by": user, "target": t }),
                ))
            }
            Some(o) => Err(format!("从谏：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    match take_answer(g, "congjian") {
        None => Ok(must_pick(
            g,
            actor,
            "congjian",
            "交给其",
            &own,
            json!({ "step": "give", "card": card, "by": user, "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !own.contains(&id) {
                return Err("从谏：只能交出自己的手牌或装备牌".into());
            }
            flow::gain(g, target, &id, actor, "从谏");
            flow::draw(g, actor, 1);
            Ok(Step::Finish("从谏：交给其一张牌，然后摸一张".into()))
        }
        Some(o) => Err(format!("从谏：未知选项 {o}")),
    }
}
