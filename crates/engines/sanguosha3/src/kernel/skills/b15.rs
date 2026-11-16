//! B15 技能（界限突破·J3：8 将 / 19 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B15.json` 的 `agreed`（权威）。
//! 本文件按批次推进；`SKILLS` 只声明**已实现并收口**的技能（`check:sgs3-registry` 会报 ⏳ 计数）。

use serde_json::json;

use super::super::cards::{self, ResponseKind};
use super::super::event::{Event, EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, Game, Identity, Phase, Suit, Zone};
use super::common::{self, card_label, own_cards, phase_key, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界董卓 ——
    SkillDef { code: "roulin", name: "肉林", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "bengkuai", name: "崩坏", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "jiuchi", name: "酒池", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "baonue", name: "暴虐", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    // —— 界姜维 ——
    SkillDef { code: "tiaoxin", name: "挑衅", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zhiji", name: "志继", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界太史慈 / 界邓艾 / 界祝融 / 界贾诩 / 界鲁肃 / 界张郃 ——
    SkillDef { code: "tianyi", name: "天义", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "hanzhan", name: "酣战", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "tuotian", name: "屯田", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::CardLost, subject: Subject::Self_ },
            Subscription { event: EventKind::CardDiscarded, subject: Subject::Self_ },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "zaoxian", name: "凿险", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "jixi", name: "急袭", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "juxiang", name: "巨象", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "liereng", name: "烈刃", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    SkillDef { code: "changbiao", name: "长标", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[
            Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
            Subscription { event: EventKind::DamageAfter, subject: Subject::By },
        ], hooks: &[], can_active: true }] },
    SkillDef { code: "weimu", name: "帷幕", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "luanwu", name: "乱武", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "wansha", name: "完杀", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "haoshi", name: "好施", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[
            Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
            Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ },
            Subscription { event: EventKind::TurnStart, subject: Subject::Any },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "dimeng", name: "缔盟", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: true }] },
    SkillDef { code: "qiaobian", name: "巧变", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[Hook::GameStart], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "bengkuai" => bengkuai,
        "baonue" => baonue,
        "tiaoxin" => tiaoxin,
        "zhiji" => zhiji,
        "tianyi" => tianyi,
        "hanzhan" => hanzhan,
        "tuotian" => tuotian,
        "zaoxian" => zaoxian,
        "jixi" => jixi,
        "liereng" => liereng,
        "changbiao" => changbiao,
        "luanwu" => luanwu,
        "haoshi" => haoshi,
        "dimeng" => dimeng,
        "qiaobian" => qiaobian,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "tiaoxin" => can_tiaoxin,
        "tianyi" => can_tianyi,
        "jixi" => can_jixi,
        "changbiao" => can_changbiao,
        "luanwu" => can_luanwu,
        "dimeng" => can_dimeng,
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
// 界董卓 · 肉林（锁定：与女性互用【杀】需两张【闪】；与无双相加）
// ---------------------------------------------------------------------------

/// 肉林对响应张数的加成（`skills::response_need` 单出口）。
pub fn response_bonus(g: &Game, _responder: usize, opponent: usize) -> u32 {
    if !has(g, opponent, "roulin") && !has(g, _responder, "roulin") {
        return 0;
    }
    // 需要知道【杀】的使用者与目标：由调用方在 kind==Shan 时传入（responder=目标，opponent=使用者）。
    let attacker = opponent;
    let defender = _responder;
    let a_roulin = has(g, attacker, "roulin");
    let d_roulin = has(g, defender, "roulin");
    if !a_roulin && !d_roulin {
        return 0;
    }
    // 女性相关：董卓（男）对女性使用 / 女性对董卓（男）使用。
    let attacker_male = g.seats.get(attacker).map(|s| s.male).unwrap_or(true);
    let defender_male = g.seats.get(defender).map(|s| s.male).unwrap_or(true);
    let applies = (a_roulin && !defender_male) || (d_roulin && !attacker_male);
    if applies {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 界董卓 · 崩坏（结束阶段；存在体力不大于你的其他角色 → 失 1 体力或减 1 上限）
// ---------------------------------------------------------------------------

fn bengkuai(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "bengkuai") || g.turn != owner {
        return Ok(Step::Done);
    }
    // 本回合以「酒【杀】」造成过伤害 → 本回合崩坏失效。
    if g.counter(&turn_key(owner, "jiu_sha_hit")) > 0 {
        return Ok(Step::Finish("崩坏：本回合酒【杀】已造成伤害，失效".into()));
    }
    let hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0);
    let trigger = g
        .seats
        .iter()
        .filter(|s| s.alive && s.id != owner)
        .any(|s| s.hp <= hp);
    if !trigger {
        return Ok(Step::Done);
    }
    match take_answer(g, "bengkuai") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("bengkuai");
            b = b.option("lose_hp", "失去 1 点体力");
            if g.seats.get(owner).map(|s| s.max_hp).unwrap_or(1) > 1 {
                b = b.option("lose_max", "减少 1 点体力上限");
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选一项）")
                    .data(json!({ "step": "ask" }))
                    .build(),
            ))
        }
        Some(a) if a == "lose_hp" => {
            flow::lose_hp(g, owner, 1, "崩坏");
            Ok(Step::Finish("崩坏：失去 1 点体力".into()))
        }
        Some(a) if a == "lose_max" => {
            flow::lose_max_hp(g, owner, 1, "崩坏");
            Ok(Step::Finish("崩坏：减少 1 点体力上限".into()))
        }
        Some(o) => Err(format!("崩坏：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界董卓 · 酒池（黑桃手牌当【酒】；【酒】不限次数且叠加；酒【杀】伤害后崩坏失效）
// ---------------------------------------------------------------------------

/// 酒池：黑桃手牌当【酒】。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    if !has(g, seat, "jiuchi") {
        return None;
    }
    if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == seat) {
        return None;
    }
    if cards::suit_is(g, id, Suit::Spade) {
        if common::def_of(g, id).map(|d| d.tags.jiu).unwrap_or(false) {
            return None;
        }
        return Some("jiu");
    }
    None
}

/// 酒池：【酒】无次数限制。
pub fn jiu_unlimited(g: &Game, seat: usize) -> bool {
    has(g, seat, "jiuchi")
}

/// 酒池：酒【杀】造成伤害后本回合崩坏失效（伤害后唯一钩子）。
pub fn on_damage_after(g: &mut Game, by: Option<usize>, is_sha: bool) {
    let Some(by) = by else { return };
    if !has(g, by, "jiuchi") || !is_sha {
        return;
    }
    if g.counter(&turn_key(by, "jiu_used")) > 0 {
        g.bump(&turn_key(by, "jiu_sha_hit"));
    }
}

// ---------------------------------------------------------------------------
// 界董卓 · 暴虐（主公技：其他群势力角色每造成 1 点伤害，可判定；黑桃回复 1 并获牌）
// ---------------------------------------------------------------------------

fn baonue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "baonue") {
        return Ok(Step::Done);
    }
    // 首次进入（事件帧）才校验触发条件：后续步骤（result/more）不再携带 by/event。
    let step = step_of(ask);
    if step == "ask" {
        let by = common::seat_of(ask, "by").unwrap_or(owner);
        if by == owner || g.seats.get(by).map(|s| s.kingdom.as_str()) != Some("qun") {
            return Ok(Step::Done);
        }
        if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
            return Ok(Step::Done);
        }
    }
    let left = if step == "more" || step == "result" {
        ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize
    } else {
        ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize
    };
    if left == 0 {
        return Ok(Step::Finish("暴虐：结算完成".into()));
    }
    if step == "result" {
        let spade = ask.data.get("suit").and_then(|v| v.as_str()) == Some("Spade");
        let card = common::card_of(ask, "judge_card");
        if spade {
            flow::heal(g, owner, 1, "暴虐");
            if let Some(c) = card {
                if matches!(g.cards.get(&c), Some(Zone::Processing) | Some(Zone::Discard)) {
                    flow::gain(g, owner, &c, owner, "暴虐");
                }
            }
        }
        let rest = left - 1;
        if rest == 0 {
            return Ok(Step::Finish("暴虐：结算完成".into()));
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "more", "left": rest }),
        ));
    }
    match take_answer(g, "baonue") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("baonue");
            b = b.option("use", format!("判定（剩 {left} 次）"));
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "left": left }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("暴虐：不发动".into())),
        Some(_) => {
            super::super::quick_judge::start(
                g,
                owner,
                "暴虐",
                Some(("baonue", owner, json!({ "step": "result", "left": left }))),
            );
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 界姜维 · 挑衅（出牌阶段先至多两次：其不出【杀】/未造成伤害 → 你弃其一张牌）
// ---------------------------------------------------------------------------

fn can_tiaoxin(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || g.turn != seat || !has(g, seat, "tiaoxin") {
        return false;
    }
    if g.counter(&phase_key(seat, "tiaoxin_hit")) > 0 {
        return false; // 首次已令其出【杀】并造成伤害 → 不能再次发动
    }
    if g.counter(&phase_key(seat, "tiaoxin")) >= 2 {
        return false;
    }
    let reach = super::super::gear::attack_range(g, seat) as usize;
    g.alive_seats()
        .into_iter()
        .any(|t| t != seat && super::super::gear::within_attack_range(g, t, seat, reach))
}

fn tiaoxin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "tiaoxin") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "react" {
        let t = common::seat_of(ask, "target").unwrap_or(owner);
        let shas: Vec<CardId> = g
            .hand(t)
            .into_iter()
            .filter(|id| common::def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
            .collect();
        return match take_answer(g, "tiaoxin") {
            None => {
                let mut b = PendingBuilder::new(t, "choose").skill("tiaoxin");
                for id in &shas {
                    b = b.option(format!("s:{id}"), format!("对姜维使用【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape("不出【杀】")
                        .data(json!({ "step": "react", "target": t }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "wait", "target": t }),
            )),
            Some(a) if a.starts_with("s:") => {
                let id = a[2..].to_string();
                if !shas.contains(&id) {
                    return Err("挑衅：只能使用手牌中的【杀】".into());
                }
                // 记录使用前体力：`DamageAfter` 事件在技能帧未让栈空时不会路由，
                // 因此用体力差判定「是否造成伤害」（单一事实：体力值）。
                let hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0);
                g.set_counter(&format!("tiaoxin.hp_before.{owner}"), hp);
                super::super::card_play::use_on_targets(g, t, &id, &[owner])?;
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "wait", "target": t }),
                ))
            }
            Some(o) => Err(format!("挑衅：未知选项 {o}")),
        };
    }
    if step == "wait" {
        let t = common::seat_of(ask, "target").unwrap_or(owner);
        g.set_counter(&format!("tiaoxin.pending.{t}"), 0);
        let before = g.counter(&format!("tiaoxin.hp_before.{owner}"));
        let now = g.seats.get(owner).map(|s| s.hp).unwrap_or(0);
        let hit = now < before;
        if hit {
            g.set_counter(&phase_key(owner, "tiaoxin_hit"), 1);
        }
        if hit {
            return Ok(Step::Finish("挑衅：其【杀】造成伤害，不弃牌".into()));
        }
        return Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "pick", "target": t }),
        ));
    }
    if step == "pick" {
        let t = common::seat_of(ask, "target").unwrap_or(owner);
        let cands = own_cards(g, t);
        if cands.is_empty() {
            return Ok(Step::Finish("挑衅：其无牌可弃".into()));
        }
        return match take_answer(g, "tiaoxin") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_other").skill("tiaoxin");
                for id in &cands {
                    b = b.option(
                        format!("c:{id}"),
                        format!("弃置 {} 的【{}】", seat_name(g, t), card_label(g, id)),
                    );
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一张）")
                        .data(json!({ "step": "pick", "target": t }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !cands.contains(&id) {
                    return Err("挑衅：该牌不在其手牌/装备区".into());
                }
                flow::discard(g, owner, &id, "挑衅");
                Ok(Step::Finish("挑衅：弃置其一张牌".into()))
            }
            Some(o) => Err(format!("挑衅：未知选项 {o}")),
        };
    }
    // 首次进入：选目标 → 询问其是否出【杀】。
    let reach = super::super::gear::attack_range(g, owner) as usize;
    let cands: Vec<usize> = g
        .alive_seats()
        .into_iter()
        .filter(|t| *t != owner && super::super::gear::within_attack_range(g, *t, owner, reach))
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "tiaoxin") {
        None => {
            let mut b = PendingBuilder::new(owner, "target").skill("tiaoxin");
            for t in &cands {
                b = b.option(format!("t:{t}"), format!("挑衅 {}", seat_name(g, *t)));
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "挑衅：非法目标")?;
            if !cands.contains(&t) {
                return Err("挑衅：目标必须在你的攻击范围内".into());
            }
            g.bump(&phase_key(owner, "tiaoxin"));
            g.set_counter(&format!("tiaoxin.pending.{t}"), 1);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "react", "target": t }),
            ))
        }
        Some(o) => Err(format!("挑衅：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界姜维 · 志继（准备/结束阶段：无手牌 → 回复 1 或摸 2，减 1 上限，获得观星）
// ---------------------------------------------------------------------------

fn zhiji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhiji") || g.turn != owner {
        return Ok(Step::Done);
    }
    if g.counter(&format!("awaken.{owner}.zhiji")) > 0 {
        return Ok(Step::Done);
    }
    if !g.hand(owner).is_empty() {
        return Ok(Step::Done);
    }
    let phase = ask.data.get("phase").and_then(|v| v.as_str()).unwrap_or("");
    if phase != "Prepare" && phase != "End" {
        return Ok(Step::Done);
    }
    let full = g
        .seats
        .get(owner)
        .map(|s| s.hp >= s.max_hp)
        .unwrap_or(false);
    match take_answer(g, "zhiji") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("zhiji");
            if !full {
                b = b.option("heal", "回复 1 点体力");
            }
            b = b.option("draw", "摸两张牌");
            Ok(Step::Need(
                b.escape_mandatory("（必选一项）")
                    .data(json!({ "step": "ask", "phase": phase }))
                    .build(),
            ))
        }
        Some(a) if a == "heal" && !full => {
            g.set_counter(&format!("awaken.{owner}.zhiji"), 1);
            flow::heal(g, owner, 1, "志继");
            flow::lose_max_hp(g, owner, 1, "志继");
            skills::grant(g, owner, "guanxing");
            Ok(Step::Finish("志继：觉醒（获得【观星】）".into()))
        }
        Some(a) if a == "draw" => {
            g.set_counter(&format!("awaken.{owner}.zhiji"), 1);
            flow::draw(g, owner, 2);
            flow::lose_max_hp(g, owner, 1, "志继");
            skills::grant(g, owner, "guanxing");
            Ok(Step::Finish("志继：觉醒（获得【观星】）".into()))
        }
        Some(o) => Err(format!("志继：未知选项 {o}")),
    }
}

// ===========================================================================
// B15 内核前置（K5–K12）——机制助手（技能注册前为惰性）
// ===========================================================================

/// K5 巨象：AOE（全体）目标排除（`card_play::legal_targets_with` 的 AllAlive 分支唯一调用点）。
pub fn aoe_excluded(g: &Game, _user: usize, target: usize, def_code: &str) -> bool {
    if !has(g, target, "juxiang") {
        return false;
    }
    def_code == "nanman_ruqin"
}

/// K5b 巨象：其他角色使用的【南蛮入侵】结算结束后获得之（`card_play::finish_card` 唯一调用点）。
/// 取消使用回退（`card_play::cancel_use` 唯一调用点）：长标被取消时退还「本阶段一次」。
pub fn on_use_cancelled(g: &mut Game, actor: usize, card: &str) {
    let vid = g.mark(&turn_key(actor, "changbiao_vid")).map(|s| s.to_string());
    if vid.as_deref() == Some(card) {
        g.marks.remove(&turn_key(actor, "changbiao_vid"));
        let k = phase_key(actor, "changbiao");
        let v = g.counter(&k);
        g.set_counter(&k, (v - 1).max(0));
        g.set_counter(&turn_key(actor, "changbiao_x"), 0);
        g.set_counter(&turn_key(actor, "changbiao_hit"), 0);
    }
}

pub fn on_card_finished(g: &mut Game, card: &str, user: usize) {
    let is_nanman = common::def_of(g, card)
        .map(|d| d.code == "nanman_ruqin")
        .unwrap_or(false);
    if !is_nanman {
        return;
    }
    // 座位序最先的巨象持有者获得（牌仍在处理区/弃牌堆才可获得）。
    let holder = g.seats.iter().position(|s| {
        s.alive && s.id != user && skills::active(g, s.id, "juxiang") && skills::enabled("juxiang")
    });
    let Some(holder) = holder else { return };
    if !matches!(g.cards.get(card), Some(Zone::Processing) | Some(Zone::Discard)) {
        return;
    }
    flow::gain(g, holder, card, holder, "巨象");
}

/// K6a 帷幕：不能成为**黑色锦囊**的目标（按实体牌花色；`legal_targets_with` 唯一调用点）。
pub fn deny_target_instance(
    g: &Game,
    _user: usize,
    target: usize,
    card: Option<&str>,
    def: &cards::CardDef,
) -> bool {
    if !has(g, target, "weimu") {
        return false;
    }
    if def.category != cards::Category::Trick && def.category != cards::Category::DelayedTrick {
        return false;
    }
    let Some(card) = card else { return false };
    g.card_defs
        .get(card)
        .map(|c| c.suit.is_black())
        .unwrap_or(false)
}

/// K6b 帷幕：自己回合内受到的伤害防止，并摸「防止点数 ×2」（`flow::damage_one` 唯一调用点）。
pub fn try_full_prevent(g: &mut Game, target: usize, amount: i32) -> Option<i32> {
    if amount <= 0 || !has(g, target, "weimu") {
        return None;
    }
    if g.turn != target {
        return None;
    }
    flow::draw(g, target, (amount * 2) as usize);
    Some(amount)
}

/// K7a 完杀：濒死求桃限制（`card_play::step_dying` 唯一调用点）。
/// 贾诩回合内：只有贾诩本人与濒死者可以使用【桃】。
pub fn dying_allow_tao(g: &Game, asker: usize, target: usize) -> bool {
    let Some(js) = wansha_owner(g) else { return true };
    asker == js || asker == target
}

/// K7b 完杀：濒死结算中，除贾诩与濒死者外的角色的**非锁定技**失效。
pub fn suppressed_in_dying(g: &Game, seat: usize) -> bool {
    let Some(js) = wansha_owner(g) else { return false };
    // 是否正处于某人的濒死结算（栈中存在 Dying 帧）。
    let dying_target = g.stack.iter().find_map(|f| {
        if let super::super::resolve::FrameInner::Flow(ff) = &f.inner {
            if ff.kind == super::super::resolve::FlowKind::Dying {
                return ff.data.get("target").and_then(|v| v.as_u64()).map(|v| v as usize);
            }
        }
        None
    });
    let Some(victim) = dying_target else { return false };
    seat != js && seat != victim
}

/// **不得**用 `skills::active`（完杀闸门本身在 `active` 内，会无限递归）。
fn owns(g: &Game, seat: usize, code: &str) -> bool {
    g.seats
        .get(seat)
        .map(|s| {
            s.skills.iter().any(|c| c == code) || s.granted.iter().any(|c| c == code)
        })
        .unwrap_or(false)
}

fn wansha_owner(g: &Game) -> Option<usize> {
    g.seats.iter().position(|s| {
        s.alive && g.turn == s.id && owns(g, s.id, "wansha") && skills::enabled("wansha")
    })
}

/// K8 巧变：把场上一张牌移动到另一角色的合理区域（单一出口）。
///
/// 口径（登记在 `v3-parity-baseline §7.22`）：手牌→手牌；装备牌→同槽（空）否则手牌；
/// 判定区牌→判定区（同名去重，已存在则弃置）。
pub fn move_field_card(g: &mut Game, from: usize, card: &str, to: usize) -> Result<(), String> {
    use super::super::Zone;
    match g.cards.get(card).cloned() {
        Some(Zone::Hand(_)) => {
            flow::gain(g, to, card, from, "巧变");
        }
        Some(Zone::Equip(_, slot)) => {
            if super::super::gear::equip_in(g, to, slot).is_none() && !g.slot_abolished(to, slot) {
                super::super::gear::move_equip(g, from, card, to)?;
            } else {
                flow::gain(g, to, card, from, "巧变");
            }
        }
        Some(Zone::Judge(_)) => {
            if let Some(st) = g.judge_stacks.get_mut(&from) {
                st.retain(|c| c != card);
            }
            let code = g.card_defs.get(card).map(|c| c.def.clone()).unwrap_or_default();
            if !super::super::card_play::place_delayed(g, to, card, &code) {
                super::super::card_play::to_discard(g, card);
            }
        }
        _ => return Err("巧变：只能移动场上的手牌/装备/判定区牌".into()),
    }
    Ok(())
}

/// 「田」= 武将牌上的牌（`Zone::Mark(seat, "tian")`），不是计数。
pub fn tian_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let mut v: Vec<CardId> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, n) if *s == seat && n == "tian"))
        .map(|(id, _)| id.clone())
        .collect();
    v.sort();
    v
}

/// K12 屯田：每有一张「田」，距离 -1（`skills::distance_bonus` 链）。
pub fn distance_bonus(g: &Game, seat: usize) -> i32 {
    if !has(g, seat, "tuotian") {
        return 0;
    }
    -(tian_cards(g, seat).len() as i32)
}

/// K12 天义：拼点没赢 → 本回合不能**使用**【杀】（响应仍可打出）。
pub fn sha_use_blocked(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "tianyi") {
        return false;
    }
    g.counter(&turn_key(seat, "tianyi_lost")) > 0
}

/// K12 天义：赢后本回合【杀】次数 +1 / 无距离 / 可多指定一个目标。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    if has(g, seat, "tianyi") && g.counter(&turn_key(seat, "tianyi_won")) > 0 {
        1
    } else {
        0
    }
}

pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "tianyi") || g.counter(&turn_key(actor, "tianyi_won")) <= 0 {
        return;
    }
    let is_sha = common::def_of(g, card).map(|d| d.tags.sha).unwrap_or(false);
    if !is_sha {
        return;
    }
    super::super::card_play::declare_no_distance(g, card);
    super::super::card_play::declare_extra_targets(g, card, 1);
}


// ---------------------------------------------------------------------------
// 界太史慈 · 天义（拼点：赢→次数+1/无距离/多一个目标；没赢→本回合不能用【杀】）
// ---------------------------------------------------------------------------

fn can_tianyi(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || g.turn != seat || !has(g, seat, "tianyi") {
        return false;
    }
    if g.counter(&phase_key(seat, "tianyi")) > 0 {
        return false;
    }
    g.alive_seats().into_iter().any(|t| {
        t != seat
            && !g.hand(t).is_empty()
            && super::super::pindian::can_start(g, seat, t)
    })
}

fn tianyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "tianyi") {
        return Ok(Step::Done);
    }
    match step_of(ask) {
        "wait" => {
            let Some(r) = g.take_pindian() else {
                return Ok(Step::Finish("天义：结算完成".into()));
            };
            if r.winner == Some(owner) {
                g.set_counter(&turn_key(owner, "tianyi_won"), 1);
            } else {
                g.set_counter(&turn_key(owner, "tianyi_lost"), 1);
            }
            // 酣战已改为独立拼点窗口（`pindian_after`），天义不再内联处理。
            Ok(Step::Finish("天义：结算完成".into()))
        }
        "hanzhan" => {
            let a = common::card_of(ask, "a").unwrap_or_default();
            let card_b = common::card_of(ask, "b").unwrap_or_default();
            let ra = ask.data.get("ra").and_then(|v| v.as_u64()).unwrap_or(0);
            let rb = ask.data.get("rb").and_then(|v| v.as_u64()).unwrap_or(0);
            let max = ra.max(rb);
            let mut cands: Vec<CardId> = Vec::new();
            for (id, rank) in [(&a, ra), (&card_b, rb)] {
                if rank != max {
                    continue;
                }
                let is_sha = common::def_of(g, id).map(|d| d.tags.sha).unwrap_or(false);
                if is_sha
                    && matches!(g.cards.get(id), Some(Zone::Discard) | Some(Zone::Processing))
                {
                    cands.push(id.clone());
                }
            }
            if cands.is_empty() {
                return Ok(Step::Finish("天义：无可获得的【杀】".into()));
            }
            return match take_answer(g, "tianyi") {
                None => {
                    let mut pb = PendingBuilder::new(owner, "choose").skill("tianyi");
                    for id in &cands {
                        pb = pb.option(format!("g:{id}"), format!("获得【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        pb.escape("不获得")
                            .data(json!({ "step": "hanzhan", "a": a, "b": card_b, "ra": ra, "rb": rb }))
                            .build(),
                    ))
                }
                Some(x) if x == "pass" => Ok(Step::Finish("天义：不获得拼点牌".into())),
                Some(x) if x.starts_with("g:") => {
                    let id = x[2..].to_string();
                    if !cands.contains(&id) {
                        return Err("天义：该牌不可获得".into());
                    }
                    flow::gain(g, owner, &id, owner, "酣战");
                    Ok(Step::Finish("天义：获得拼点【杀】".into()))
                }
                Some(o) => Err(format!("天义：未知选项 {o}")),
            };
        }
        "hanzhan_pre" => {
            let t = common::seat_of(ask, "target").unwrap_or(owner);
            return match take_answer(g, "tianyi") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("tianyi");
                    b = b.option("hanzhan", format!("酣战：令 {} 用随机手牌拼点", seat_name(g, t)));
                    Ok(Step::Need(
                        b.escape("正常拼点")
                            .data(json!({ "step": "hanzhan_pre", "target": t }))
                            .build(),
                    ))
                }
                Some(x) if x == "hanzhan" => {
                    super::super::pindian::start_random(g, owner, t, "天义");
                    Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
                }
                Some(_) => {
                    super::super::pindian::start(g, owner, t, "天义");
                    Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
                }
            };
        }
        _ => {
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|t| {
                    *t != owner
                        && !g.hand(*t).is_empty()
                        && super::super::pindian::can_start(g, owner, *t)
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "tianyi") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("tianyi");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("与 {} 拼点", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape("不发动").data(json!({ "step": "target" })).build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "天义：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("天义：目标非法".into());
                    }
                    g.bump(&phase_key(owner, "tianyi"));
                    // 酣战前置由 `pindian::start` 统一压询问（任意拼点来源都生效）。
                    super::super::pindian::start(g, owner, t, "天义");
                    Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
                }
                Some(o) => Err(format!("天义：未知选项 {o}")),
            }
        }
    }
}

/// 酣战·拼点前：参与者可令对手随机手牌拼点（压入询问；由 `pindian` 统一调用）。
pub fn fire_pindian_pre(g: &mut Game, a: usize, b: usize) {
    for (holder, other) in [(a, b), (b, a)] {
        if !has(g, holder, "hanzhan") || g.hand(other).is_empty() {
            continue;
        }
        super::super::resolve::push_ask(
            g,
            "hanzhan",
            Stage::Choose,
            holder,
            json!({ "step": "pre", "other": other }),
        );
    }
}

/// 酣战·拼点后：获得点数最大的**实体**【杀】（询问，可放弃；同最大由持有者选）。
pub fn pindian_after(g: &mut Game, r: &super::super::pindian::PindianResult) {
    let max = r.rank_a.max(r.rank_b);
    let mut cands: Vec<CardId> = Vec::new();
    for (id, rank) in [(&r.a, r.rank_a), (&r.b, r.rank_b)] {
        if rank != max {
            continue;
        }
        let is_sha = common::def_of(g, id).map(|d| d.tags.sha).unwrap_or(false);
        if is_sha && matches!(g.cards.get(id), Some(Zone::Discard)) {
            cands.push(id.clone());
        }
    }
    if cands.is_empty() {
        return;
    }
    for owner in [r.initiator, r.target] {
        if !has(g, owner, "hanzhan") {
            continue;
        }
        super::super::resolve::push_ask(
            g,
            "hanzhan",
            Stage::Choose,
            owner,
            json!({ "step": "after", "cands": cands }),
        );
    }
}

fn hanzhan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "hanzhan") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("");
    if step == "pre" {
        let other = ask.data.get("other").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if !g.seats.get(other).map(|s| s.alive).unwrap_or(false) || g.hand(other).is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "hanzhan") {
            if ans == "use" {
                g.set_counter(&format!("pindian.random.{other}"), 1);
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "hanzhan_pre").skill("hanzhan");
        b = b.option(
            "use",
            format!("酣战：令 {} 用随机手牌拼点", seat_name(g, other)),
        );
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "step": "pre", "other": other }),
            b.escape("正常拼点").build(),
        ));
    }
    if step == "after" {
        let cands: Vec<CardId> = ask
            .data
            .get("cands")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let cands: Vec<CardId> = cands
            .into_iter()
            .filter(|id| matches!(g.cards.get(id), Some(Zone::Discard)))
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "hanzhan") {
            if let Some(id) = ans.strip_prefix("gain:") {
                if !cands.iter().any(|c| c == id) {
                    return Err("酣战：该牌不在拼点牌中".into());
                }
                flow::gain(g, owner, id, owner, "酣战");
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "hanzhan_gain").skill("hanzhan");
        for id in &cands {
            b = b.option(
                format!("gain:{id}"),
                format!("酣战：获得【{}】", card_label(g, id)),
            );
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "step": "after", "cands": cands }),
            b.escape("不获得").build(),
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 界邓艾 · 屯田 / 凿险 / 急袭
// ---------------------------------------------------------------------------

fn tuotian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "tuotian") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "result" {
        let heart = ask.data.get("suit").and_then(|v| v.as_str()) == Some("Heart");
        if heart {
            return Ok(Step::Finish("屯田：红桃，不得「田」".into()));
        }
        if let Some(c) = common::card_of(ask, "judge_card") {
            if matches!(g.cards.get(&c), Some(Zone::Processing) | Some(Zone::Discard)) {
                // 田 = 置于武将牌上的实体牌（可被急袭当承载牌使用）。
                flow::store_on_general(g, owner, &c, "tian", "屯田");
            }
        }
        return Ok(Step::Finish("屯田：获得一张「田」".into()));
    }
    if step == "ask" {
        let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let ok = if event == "CardLost" {
            g.turn != owner
        } else if event == "CardDiscarded" {
            let is_sha = common::card_of(ask, "card")
                .and_then(|c| common::def_of(g, &c))
                .map(|d| d.tags.sha)
                .unwrap_or(false);
            g.turn == owner && is_sha
        } else {
            false
        };
        if !ok {
            return Ok(Step::Done);
        }
    }
    match take_answer(g, "tuotian") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("tuotian");
            b = b.option("use", "判定（非红桃得「田」）");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "ask" })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("屯田：不发动".into())),
        Some(_) => {
            super::super::quick_judge::start(
                g,
                owner,
                "屯田",
                Some(("tuotian", owner, json!({ "step": "result" }))),
            );
            Ok(Step::Done)
        }
    }
}

fn zaoxian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zaoxian") || g.turn != owner {
        return Ok(Step::Done);
    }
    if ask.data.get("phase").and_then(|v| v.as_str()) != Some("Prepare") {
        return Ok(Step::Done);
    }
    if g.counter(&format!("awaken.{owner}.zaoxian")) > 0 {
        return Ok(Step::Done);
    }
    if tian_cards(g, owner).len() < 3 {
        return Ok(Step::Done);
    }
    g.set_counter(&format!("awaken.{owner}.zaoxian"), 1);
    flow::lose_max_hp(g, owner, 1, "凿险");
    skills::grant(g, owner, "jixi");
    super::super::turn::extra_turn(g, owner);
    Ok(Step::Finish("凿险：觉醒（获得【急袭】，本回合后额外回合）".into()))
}

fn can_jixi(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || g.turn != seat || !has(g, seat, "jixi") {
        return false;
    }
    if g.counter(&phase_key(seat, "jixi")) > 0 || tian_cards(g, seat).is_empty() {
        return false;
    }
    let Some(def) = cards::def("shunshou_qianyang") else {
        return false;
    };
    !super::super::card_play::legal_targets(g, seat, def).is_empty()
}

fn jixi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !can_jixi(g, owner) {
        return Ok(Step::Done);
    }
    let Some(def) = cards::def("shunshou_qianyang") else {
        return Ok(Step::Done);
    };
    let cands = super::super::card_play::legal_targets(g, owner, def);
    match take_answer(g, "jixi") {
        None => {
            let mut b = PendingBuilder::new(owner, "target").skill("jixi");
            for t in &cands {
                b = b.option(
                    format!("t:{t}"),
                    format!("将「田」当【顺手牵羊】对 {} 使用", seat_name(g, *t)),
                );
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "急袭：非法目标")?;
            if !cands.contains(&t) {
                return Err("急袭：目标非法".into());
            }
            // 消耗一张「田」作为承载牌（实体牌走 `use_as`，结算后整组进弃牌堆）。
            let Some(tian) = tian_cards(g, owner).into_iter().next() else {
                return Ok(Step::Done);
            };
            g.bump(&phase_key(owner, "jixi"));
            super::super::card_play::use_as(g, owner, "shunshou_qianyang", &[tian], &[t], false)?;
            Ok(Step::Finish("急袭：「田」当【顺手牵羊】".into()))
        }
        Some(o) => Err(format!("急袭：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界祝融 · 烈刃
// ---------------------------------------------------------------------------

fn liereng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "liereng") {
        return Ok(Step::Done);
    }
    if step_of(ask) == "wait" {
        if let Some(r) = g.take_pindian() {
            if r.winner == Some(owner) && !own_cards(g, r.target).is_empty() {
                let ids = own_cards(g, r.target);
                let i = g.rand_below(ids.len());
                let id = ids[i].clone();
                flow::gain(g, owner, &id, owner, "烈刃");
                return Ok(Step::Finish("烈刃：拼点赢，获得其一张牌".into()));
            }
        }
        return Ok(Step::Finish("烈刃：未获得牌".into()));
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let victim = common::seat_of(ask, "victim").unwrap_or(owner);
    if victim == owner || !g.seats.get(victim).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    if !super::super::pindian::can_start(g, owner, victim) {
        return Ok(Step::Done);
    }
    match take_answer(g, "liereng") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("liereng");
            b = b.option("use", "与其拼点（赢则获得其一张牌）");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "victim": victim }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => {
            super::super::pindian::start(g, owner, victim, "烈刃");
            Ok(Step::Next(Stage::Choose, json!({ "step": "wait" })))
        }
    }
}

// ---------------------------------------------------------------------------
// 界祝融 · 长标（任意张手牌当无距离【杀】；造成伤害 → 出牌阶段结束摸 X）
// ---------------------------------------------------------------------------

/// 长标的承载牌候选：与 `usable_hand` 同口径（冻结 / 花色封锁 / 颜色封锁的牌不可选）。
fn changbiao_carriers(g: &Game, owner: usize, picked: &[CardId]) -> Vec<CardId> {
    g.hand(owner)
        .into_iter()
        .filter(|id| !picked.contains(id))
        .filter(|id| !g.is_frozen(owner, id))
        .filter(|id| !skills::card_locked(g, owner, id))
        .filter(|id| !skills::hand_color_blocked(g, owner, id))
        .collect()
}

fn can_changbiao(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.turn == seat
        && has(g, seat, "changbiao")
        && g.counter(&phase_key(seat, "changbiao")) == 0
        && !g.hand(seat).is_empty()
}

fn changbiao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "changbiao") {
        return Ok(Step::Done);
    }
    match step_of(ask) {
        "pick" => {
            let picked = common::cards_of(ask, "picked");
            let hand: Vec<CardId> = changbiao_carriers(g, owner, &picked);
            match take_answer(g, "changbiao") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_own").skill("changbiao");
                    for id in &hand {
                        b = b.option(format!("c:{id}"), format!("加入【{}】", card_label(g, id)));
                    }
                    if !picked.is_empty() {
                        b = b.option("confirm", format!("确认（共 {} 张当【杀】）", picked.len()));
                    }
                    Ok(Step::Need(
                        b.escape("取消")
                            .data(json!({ "step": "pick", "picked": picked }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "confirm" => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "picked": picked }),
                )),
                Some(a) if a.starts_with("c:") => {
                    let id = a[2..].to_string();
                    if !hand.contains(&id) {
                        return Err("长标：该牌不在手牌".into());
                    }
                    let mut np = picked;
                    np.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "pick", "picked": np }),
                    ))
                }
                Some(o) => Err(format!("长标：未知选项 {o}")),
            }
        }
        "target" => {
            let picked = common::cards_of(ask, "picked");
            if picked.is_empty() {
                return Ok(Step::Done);
            }
            // 目标合法性/数量/无距离全部走内核单出口：
            //  - `legal_targets_with` 逐目标拦截（陷阵没赢 / 惴恐 / 帷幕…）
            //  - `target_bounds` 目标数（方天画戟「最后的手牌」→ 最多 3）
            //  - `declare_no_distance` 只免距离，不改其它规则
            let vid = match super::super::card_play::use_as(g, owner, "sha", &picked, &[], false) {
                Ok(v) => v,
                Err(e) => return Err(format!("长标：{e}")),
            };
            super::super::card_play::declare_no_distance(g, &vid);
            g.set_mark(&turn_key(owner, "changbiao_vid"), vid);
            g.set_counter(&turn_key(owner, "changbiao_x"), picked.len() as i32);
            g.bump(&phase_key(owner, "changbiao"));
            Ok(Step::Finish("长标：视为使用无距离【杀】".into()))
        }
        _ => {
            let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
            // 命中判定 = **伤害事件**（护甲吸收后 `DamageAfter` 仍成立）；多目标多次伤害只记一次。
            if event == "DamageAfter" {
                let vid = g.mark(&turn_key(owner, "changbiao_vid")).map(|s| s.to_string());
                let is_our_sha = vid.is_some()
                    && ask.data.get("by").and_then(|v| v.as_u64()) == Some(owner as u64)
                    && ask.data.get("damage_source").and_then(|v| v.as_str()) == Some("Slash");
                if is_our_sha {
                    g.set_counter(&turn_key(owner, "changbiao_hit"), 1);
                }
                return Ok(Step::Done);
            }
            // 摸牌时机（用户裁定）：**弃牌阶段之后的结束阶段开始**（不是出牌阶段结束）。
            if event == "PhaseStart" {
                if ask.data.get("phase").and_then(|v| v.as_str()) == Some("End") {
                    if g.counter(&turn_key(owner, "changbiao_hit")) > 0 {
                        let x = g.counter(&turn_key(owner, "changbiao_x")).max(0) as usize;
                        if x > 0 {
                            flow::draw(g, owner, x);
                        }
                    }
                    g.set_counter(&turn_key(owner, "changbiao_hit"), 0);
                    g.set_counter(&turn_key(owner, "changbiao_x"), 0);
                    g.marks.remove(&turn_key(owner, "changbiao_vid"));
                    return Ok(Step::Finish("长标：阶段结束结算".into()));
                }
                return Ok(Step::Done);
            }
            // 首次进入：开始选牌（候选同 `pick` 阶段：过滤冻结/花色封锁/颜色封锁）。
            let first_cands = changbiao_carriers(g, owner, &[]);
            if first_cands.is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "changbiao") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_own").skill("changbiao");
                    for id in &first_cands {
                        b = b.option(format!("c:{id}"), format!("加入【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "pick", "picked": [] }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "picked": [a[2..].to_string()] }),
                )),
                Some(_) => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "picked": [] }),
                )),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 界贾诩 · 乱武（限定技）
// ---------------------------------------------------------------------------

fn can_luanwu(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && g.turn == seat
        && has(g, seat, "luanwu")
        && !skills::limited_used(g, seat, "luanwu")
}

fn nearest_others(g: &Game, seat: usize) -> Vec<usize> {
    let mut best = usize::MAX;
    let mut out = Vec::new();
    for t in g.alive_seats() {
        if t == seat {
            continue;
        }
        let d = super::super::gear::distance(g, seat, t);
        if d < best {
            best = d;
            out.clear();
            out.push(t);
        } else if d == best {
            out.push(t);
        }
    }
    out
}

fn luanwu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "luanwu") {
        return Ok(Step::Done);
    }
    match step_of(ask) {
        "seat" => {
            let order = common::seats_of(ask, "order");
            let mut idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            while idx < order.len() {
                let s = order[idx];
                if !g.seats.get(s).map(|x| x.alive).unwrap_or(false) {
                    idx += 1;
                    continue;
                }
                let has_sha = g
                    .hand(s)
                    .into_iter()
                    .any(|id| common::def_of(g, &id).map(|d| d.tags.sha).unwrap_or(false));
                let has_target = !nearest_others(g, s).is_empty();
                return match take_answer(g, "luanwu") {
                    None => {
                        let mut b = PendingBuilder::new(s, "choose").skill("luanwu");
                        if has_sha && has_target {
                            b = b.option("sha", "对距离最小的另一名角色使用一张【杀】");
                        }
                        b = b.option("lose_hp", "失去 1 点体力");
                        Ok(Step::Need(
                            b.escape_mandatory("（必选一项）")
                                .data(json!({ "step": "seat", "order": order, "idx": idx }))
                                .build(),
                        ))
                    }
                    Some(x) if x == "lose_hp" => {
                        flow::lose_hp(g, s, 1, "乱武");
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "seat", "order": order, "idx": idx + 1 }),
                        ))
                    }
                    Some(x) if x == "sha" => Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "sha_target", "order": order, "idx": idx, "seat": s }),
                    )),
                    Some(o) => Err(format!("乱武：未知选项 {o}")),
                };
            }
            // 全部结算完毕：可选视为使用无距离【杀】。
            if common::alive_others(g, owner).is_empty() {
                return Ok(Step::Finish("乱武：结算完成".into()));
            }
            match take_answer(g, "luanwu") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("luanwu");
                    b = b.option("use", "视为使用一张无距离限制的【杀】");
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "final" }))
                            .build(),
                    ))
                }
                Some(x) if x == "pass" => Ok(Step::Finish("乱武：结算完成".into())),
                Some(_) => Ok(Step::Next(Stage::Target, json!({ "step": "final_target" }))),
                Some(o) => Err(format!("乱武：未知选项 {o}")),
            }
        }
        "sha_target" => {
            let s = common::seat_of(ask, "seat").unwrap_or(owner);
            let order = common::seats_of(ask, "order");
            let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let cands = nearest_others(g, s);
            match take_answer(g, "luanwu") {
                None => {
                    let mut b = PendingBuilder::new(s, "target").skill("luanwu");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("对 {} 使用【杀】", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一名目标）")
                            .data(json!({ "step": "sha_target", "order": order, "idx": idx, "seat": s }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "乱武：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("乱武：只能对距离最小的角色使用【杀】".into());
                    }
                    let shas: Vec<CardId> = g
                        .hand(s)
                        .into_iter()
                        .filter(|id| common::def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
                        .collect();
                    let Some(card) = shas.first() else {
                        return Err("乱武：该角色没有【杀】".into());
                    };
                    super::super::card_play::use_on_targets(g, s, card, &[t])?;
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "seat", "order": order, "idx": idx + 1 }),
                    ))
                }
                Some(o) => Err(format!("乱武：未知选项 {o}")),
            }
        }
        "final_target" => {
            let cands = common::alive_others(g, owner);
            match take_answer(g, "luanwu") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("luanwu");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("对 {} 使用【杀】", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一名目标）")
                            .data(json!({ "step": "final_target" }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "乱武：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("乱武：目标非法".into());
                    }
                    let vid = super::super::card_play::use_virtual(g, owner, "sha", &[t], false)?;
                    super::super::card_play::declare_no_distance(g, &vid);
                    super::super::card_play::declare_sha_not_counted(g, owner, &vid);
                    Ok(Step::Finish("乱武：视为使用【杀】".into()))
                }
                Some(o) => Err(format!("乱武：未知选项 {o}")),
            }
        }
        _ => {
            if !can_luanwu(g, owner) {
                return Ok(Step::Done);
            }
            match take_answer(g, "luanwu") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("luanwu");
                    b = b.option("use", "发动乱武");
                    Ok(Step::Need(
                        b.escape("不发动").data(json!({ "step": "ask" })).build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(_) => {
                    skills::mark_limited_used(g, owner, "luanwu");
                    let n = g.seats.len();
                    let order: Vec<usize> = (1..n)
                        .map(|k| (owner + k) % n)
                        .filter(|s| *s != owner && g.seats[*s].alive)
                        .collect();
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "seat", "order": order, "idx": 0 }),
                    ))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 界鲁肃 · 好施 / 缔盟
// ---------------------------------------------------------------------------

fn haoshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "haoshi") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    if event == "TurnStart" {
        if g.turn == owner {
            g.marks.remove("haoshi.giver");
        }
        return Ok(Step::Done);
    }
    if event == "TargetChosen" {
        let Some(giver) = g.mark("haoshi.giver").and_then(|s| s.parse::<usize>().ok()) else {
            return Ok(Step::Done);
        };
        if giver == owner || !g.seats.get(giver).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        if g.hand(giver).is_empty() {
            return Ok(Step::Done);
        }
        let ok = common::card_of(ask, "card")
            .and_then(|c| common::def_of(g, &c))
            .map(|d| {
                d.tags.sha
                    || matches!(
                        d.category,
                        cards::Category::Trick | cards::Category::DelayedTrick
                    )
            })
            .unwrap_or(false);
        if !ok {
            return Ok(Step::Done);
        }
        return match take_answer(g, "haoshi") {
            None => {
                let mut b = PendingBuilder::new(giver, "pick_own").skill("haoshi");
                for id in g.hand(giver) {
                    b = b.option(
                        format!("c:{id}"),
                        format!("交给鲁肃【{}】", card_label(g, &id)),
                    );
                }
                Ok(Step::Need(b.escape("不交").data(json!({ "step": "giver" })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !g.hand(giver).contains(&id) {
                    return Err("好施：该牌不在其手牌".into());
                }
                flow::gain(g, owner, &id, giver, "好施");
                Ok(Step::Finish("好施：回礼者交给鲁肃一张牌".into()))
            }
            Some(o) => Err(format!("好施：未知选项 {o}")),
        };
    }
    // 摸牌阶段：可多摸 2；手牌 >5 → 交一半给手牌最少者。
    if event == "PhaseStart" {
        if ask.data.get("phase").and_then(|v| v.as_str()) != Some("Draw") {
            return Ok(Step::Done);
        }
        if step_of(ask) == "target" {
            let half = g.hand_size(owner) / 2;
            let min = g
                .alive_seats()
                .into_iter()
                .filter(|t| *t != owner)
                .map(|t| g.hand_size(t))
                .min()
                .unwrap_or(0);
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|t| *t != owner && g.hand_size(*t) == min)
                .collect();
            if cands.is_empty() || half == 0 {
                return Ok(Step::Finish("好施：无可交牌对象".into()));
            }
            return match take_answer(g, "haoshi") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("haoshi");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("交给 {}", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一名）")
                            .data(json!({ "step": "target" }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "好施：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("好施：只能交给手牌最少的角色".into());
                    }
                    let mut ids = g.hand(owner);
                    ids.truncate(half);
                    for id in ids {
                        flow::gain(g, t, &id, owner, "好施");
                    }
                    g.set_mark("haoshi.giver", t.to_string());
                    Ok(Step::Finish("好施：交出一半手牌".into()))
                }
                Some(o) => Err(format!("好施：未知选项 {o}")),
            };
        }
        return match take_answer(g, "haoshi") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("haoshi");
                b = b.option("use", "好施：多摸两张牌");
                Ok(Step::Need(
                    b.escape("不发动").data(json!({ "step": "ask" })).build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(_) => {
                flow::draw(g, owner, 2);
                if g.hand_size(owner) <= 5 {
                    return Ok(Step::Finish("好施：多摸两张（手牌 ≤5，不交牌）".into()));
                }
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "event": "PhaseStart", "phase": "Draw" }),
                ))
            }
        };
    }
    Ok(Step::Done)
}

fn can_dimeng(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || g.turn != seat || !has(g, seat, "dimeng") {
        return false;
    }
    if g.counter(&phase_key(seat, "dimeng")) > 0 {
        return false;
    }
    let own = g.hand_size(seat);
    let others: Vec<usize> = g.alive_seats().into_iter().filter(|t| *t != seat).collect();
    for i in 0..others.len() {
        for j in (i + 1)..others.len() {
            if g.hand_size(others[i]).abs_diff(g.hand_size(others[j])) <= own {
                return true;
            }
        }
    }
    false
}

fn dimeng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "dimeng") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    match step_of(ask) {
        "second" => {
            let first = common::seat_of(ask, "first").unwrap_or(owner);
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|t| *t != owner && *t != first)
                .collect();
            match take_answer(g, "dimeng") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("dimeng");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("第二名：{}", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape("取消")
                            .data(json!({ "step": "second", "first": first }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let second: usize = a[2..].parse().map_err(|_| "缔盟：非法目标")?;
                    if !cands.contains(&second) {
                        return Err("缔盟：目标非法".into());
                    }
                    let diff = g.hand_size(first).abs_diff(g.hand_size(second));
                    if diff > g.hand_size(owner) {
                        return Err("缔盟：手牌差超过你的手牌数".into());
                    }
                    flow::swap_hands(g, first, second);
                    g.bump(&phase_key(owner, "dimeng"));
                    g.set_counter(&turn_key(owner, "dimeng_x"), diff as i32);
                    Ok(Step::Finish("缔盟：两名角色交换手牌".into()))
                }
                Some(o) => Err(format!("缔盟：未知选项 {o}")),
            }
        }
        "cost" => {
            let need = ask.data.get("need").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let picked = common::cards_of(ask, "picked");
            if picked.len() >= need {
                g.set_counter(&turn_key(owner, "dimeng_x"), 0);
                return Ok(Step::Finish("缔盟：费用已弃置".into()));
            }
            let hand: Vec<CardId> = g
                .hand(owner)
                .into_iter()
                .filter(|c| !picked.contains(c))
                .collect();
            if hand.is_empty() {
                g.set_counter(&turn_key(owner, "dimeng_x"), 0);
                return Ok(Step::Finish("缔盟：牌不足，弃光".into()));
            }
            match take_answer(g, "dimeng") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_own").skill("dimeng");
                    for id in &hand {
                        b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必弃）")
                            .data(json!({ "step": "cost", "need": need, "picked": picked }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("c:") => {
                    let id = a[2..].to_string();
                    if !hand.contains(&id) {
                        return Err("缔盟：该牌不在手牌".into());
                    }
                    flow::discard(g, owner, &id, "缔盟");
                    let mut np = picked;
                    np.push(id);
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "cost", "need": need, "picked": np }),
                    ))
                }
                Some(o) => Err(format!("缔盟：未知选项 {o}")),
            }
        }
        _ => {
            // 出牌阶段结束：弃 X（X = 手牌差）。
            if event == "PhaseStart" {
                if ask.data.get("phase").and_then(|v| v.as_str()) != Some("End") {
                    return Ok(Step::Done);
                }
                let x = g.counter(&turn_key(owner, "dimeng_x")).max(0) as usize;
                if x == 0 {
                    return Ok(Step::Done);
                }
                let need = x.min(g.hand_size(owner));
                if need == 0 {
                    g.set_counter(&turn_key(owner, "dimeng_x"), 0);
                    return Ok(Step::Finish("缔盟：无牌可弃".into()));
                }
                return Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "cost", "need": need, "picked": [] }),
                ));
            }
            let cands = common::alive_others(g, owner);
            if cands.len() < 2 {
                return Ok(Step::Done);
            }
            match take_answer(g, "dimeng") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("dimeng");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("第一名：{}", seat_name(g, *t)));
                    }
                    Ok(Step::Need(
                        b.escape("不发动").data(json!({ "step": "first" })).build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let first: usize = a[2..].parse().map_err(|_| "缔盟：非法目标")?;
                    if !cands.contains(&first) {
                        return Err("缔盟：目标非法".into());
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "second", "first": first }),
                    ))
                }
                Some(o) => Err(format!("缔盟：未知选项 {o}")),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 界张郃 · 巧变
// ---------------------------------------------------------------------------

fn qiaobian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qiaobian") {
        return Ok(Step::Done);
    }
    // 开局：2 枚「变」。
    if ask.data.get("event").is_none() {
        g.set_counter(&format!("bian.{owner}"), 2);
        return Ok(Step::Finish("巧变：获得 2 枚「变」".into()));
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") || g.turn != owner {
        return Ok(Step::Done);
    }
    let phase = ask.data.get("phase").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if phase == "End" {
        let key = turn_key(owner, "qiaobian_last");
        let now = g.hand_size(owner) as i32;
        let prev = g.marks.get(&key).map(|s| s.to_string());
        let fresh = prev
            .as_deref()
            .map(|p| !p.split(',').any(|x| x.parse::<i32>().ok() == Some(now)))
            .unwrap_or(true);
        if fresh {
            g.bump(&format!("bian.{owner}"));
        }
        let mut all: Vec<String> = prev
            .map(|p| p.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_default();
        all.push(now.to_string());
        g.set_mark(&key, all.join(","));
        return Ok(Step::Finish("巧变：结束阶段结算".into()));
    }
    if !matches!(phase.as_str(), "Judge" | "Draw" | "Play" | "Discard") {
        return Ok(Step::Done);
    }
    match step_of(ask) {
        "pick" => {
            let cands = own_cards(g, owner);
            return match take_answer(g, "qiaobian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_own").skill("qiaobian");
                    for id in &cands {
                        b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape("取消")
                            .data(json!({ "step": "pick", "phase": phase }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => {
                    let id = a[2..].to_string();
                    if !cands.contains(&id) {
                        return Err("巧变：该牌不在手牌/装备区".into());
                    }
                    flow::discard(g, owner, &id, "巧变");
                    qiaobian_skip(g, owner, &phase)
                }
                Some(o) => Err(format!("巧变：未知选项 {o}")),
            };
        }
        "draw_pick" => {
            let picked = common::seats_of(ask, "picked");
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|t| *t != owner && !g.hand(*t).is_empty() && !picked.contains(t))
                .collect();
            if picked.len() >= 2 || cands.is_empty() {
                for t in &picked {
                    let ids = g.hand(*t);
                    if !ids.is_empty() {
                        let i = g.rand_below(ids.len());
                        let id = ids[i].clone();
                        flow::gain(g, owner, &id, owner, "巧变");
                    }
                }
                return Ok(Step::Finish("巧变：跳过摸牌阶段并获得手牌".into()));
            }
            return match take_answer(g, "qiaobian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("qiaobian");
                    for t in &cands {
                        b = b.option(format!("t:{t}"), format!("获得 {} 一张随机手牌", seat_name(g, *t)));
                    }
                    if !picked.is_empty() {
                        b = b.option("confirm", "确认（结算已选）");
                    }
                    Ok(Step::Need(
                        b.escape("结束选择")
                            .data(json!({ "step": "draw_pick", "picked": picked }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" || a == "confirm" => {
                    for t in &picked {
                        let ids = g.hand(*t);
                        if !ids.is_empty() {
                            let i = g.rand_below(ids.len());
                            let id = ids[i].clone();
                            flow::gain(g, owner, &id, owner, "巧变");
                        }
                    }
                    Ok(Step::Finish("巧变：跳过摸牌阶段并获得手牌".into()))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "巧变：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("巧变：目标非法".into());
                    }
                    let mut np = picked;
                    np.push(t);
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "draw_pick", "picked": np }),
                    ))
                }
                Some(o) => Err(format!("巧变：未知选项 {o}")),
            };
        }
        "move_pick" => {
            let mut field: Vec<(usize, CardId)> = Vec::new();
            for s in g.alive_seats() {
                for id in common::equip_cards(g, s) {
                    field.push((s, id));
                }
                for (id, z) in g.cards.iter() {
                    if matches!(z, Zone::Judge(s2) if *s2 == s) {
                        field.push((s, id.clone()));
                    }
                }
            }
            if field.is_empty() {
                return Ok(Step::Finish("巧变：场上无牌可移动".into()));
            }
            if let Some(card) = common::card_of(ask, "card") {
                let from = common::seat_of(ask, "from").unwrap_or(owner);
                let cands: Vec<usize> = g.alive_seats().into_iter().filter(|t| *t != from).collect();
                return match take_answer(g, "qiaobian") {
                    None => {
                        let mut b = PendingBuilder::new(owner, "target").skill("qiaobian");
                        for t in &cands {
                            b = b.option(format!("t:{t}"), format!("移动到 {}", seat_name(g, *t)));
                        }
                        Ok(Step::Need(
                            b.escape_mandatory("（必选一名角色）")
                                .data(json!({ "step": "move_pick", "from": from, "card": card }))
                                .build(),
                        ))
                    }
                    Some(a) if a.starts_with("t:") => {
                        let t: usize = a[2..].parse().map_err(|_| "巧变：非法目标")?;
                        if !cands.contains(&t) {
                            return Err("巧变：目标非法".into());
                        }
                        super::super::skills::b15::move_field_card(g, from, &card, t)?;
                        Ok(Step::Finish("巧变：移动场上一张牌".into()))
                    }
                    Some(o) => Err(format!("巧变：未知选项 {o}")),
                };
            }
            return match take_answer(g, "qiaobian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "pick_other").skill("qiaobian");
                    for (s, id) in &field {
                        b = b.option(
                            format!("e:{s}:{id}"),
                            format!("移动 {} 的【{}】", seat_name(g, *s), card_label(g, id)),
                        );
                    }
                    Ok(Step::Need(
                        b.escape("结束选择")
                            .data(json!({ "step": "move_pick" }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Finish("巧变：不移动".into())),
                Some(a) if a.starts_with("e:") => {
                    let mut it = a[2..].splitn(2, ':');
                    let from: usize = it.next().and_then(|x| x.parse().ok()).ok_or("巧变：非法选择")?;
                    let card = it.next().ok_or("巧变：缺牌")?.to_string();
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "move_pick", "from": from, "card": card }),
                    ))
                }
                Some(o) => Err(format!("巧变：未知选项 {o}")),
            };
        }
        _ => {
            let bian = g.counter(&format!("bian.{owner}"));
            let has_card = !own_cards(g, owner).is_empty();
            if bian <= 0 && !has_card {
                return Ok(Step::Done);
            }
            match take_answer(g, "qiaobian") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("qiaobian");
                    if bian > 0 {
                        b = b.option("bian", "移除 1 枚「变」跳过该阶段");
                    }
                    if has_card {
                        b = b.option("card", "弃置一张牌跳过该阶段");
                    }
                    Ok(Step::Need(
                        b.escape("不发动")
                            .data(json!({ "step": "ask", "phase": phase }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "bian" => {
                    g.set_counter(&format!("bian.{owner}"), bian - 1);
                    qiaobian_skip(g, owner, &phase)
                }
                Some(a) if a == "card" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "phase": phase }),
                )),
                Some(o) => Err(format!("巧变：未知选项 {o}")),
            }
        }
    }
}

/// 跳过某阶段（判定区保留；摸牌/出牌有后续收益）。
fn qiaobian_skip(g: &mut Game, owner: usize, phase: &str) -> Result<Step, String> {
    match phase {
        "Judge" => {
            super::super::turn::skip_phase(g, Phase::Judge);
            Ok(Step::Finish("巧变：跳过判定阶段（判定区保留）".into()))
        }
        "Draw" => {
            super::super::turn::skip_phase(g, Phase::Draw);
            Ok(Step::Next(
                Stage::Target,
                json!({ "step": "draw_pick", "picked": [] }),
            ))
        }
        "Play" => {
            super::super::turn::skip_current_play(g, owner);
            Ok(Step::Next(Stage::PickOther, json!({ "step": "move_pick" })))
        }
        _ => {
            super::super::turn::skip_phase(g, Phase::Discard);
            Ok(Step::Finish("巧变：跳过弃牌阶段".into()))
        }
    }
}
