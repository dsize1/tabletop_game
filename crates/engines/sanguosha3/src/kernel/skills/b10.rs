//! B10 技能（界限突破·界标·吴：8 将 / 17 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B10.json` 的 `agreed`（权威）。

use serde_json::json;

use super::super::cards::{self, Category};
use super::super::event::{Event, EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, Game, Identity, Phase, Slot, Suit, Zone};
use super::common::{self, card_label, def_of, own_cards, phase_key, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界甘宁 ——
    SkillDef { code: "qixi", name: "奇袭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "fenwei", name: "奋威", kind: SkillKind::Limited, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界吕蒙 ——
    SkillDef { code: "keji", name: "克己", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "qinxue", name: "勤学", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "botu", name: "博图", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界黄盖 ——
    SkillDef { code: "kurou", name: "苦肉", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zhaxiang", name: "诈降", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::HpLost, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    // —— 界周瑜 ——
    SkillDef { code: "yingzi", name: "英姿", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "fanjian", name: "反间", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界大乔 ——
    SkillDef { code: "liuli", name: "流离", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaIntercept], can_active: false }] },
    SkillDef { code: "guose", name: "国色", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界陆逊 ——
    SkillDef { code: "qianxun", name: "谦逊", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "lianying", name: "连营", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界孙权 ——
    SkillDef { code: "zhiheng", name: "制衡", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "jiuyuan", name: "救援", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界孙尚香 ——
    SkillDef { code: "xiaoji", name: "枭姬", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "jieyin", name: "结姻", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 勤学授予：攻心（grant_only）——
    SkillDef { code: "gongxin", name: "攻心", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "fenwei" => fenwei,
        "qinxue" => qinxue,
        "kurou" => kurou,
        "zhaxiang" => zhaxiang,
        "fanjian" => fanjian,
        "liuli" => liuli,
        "guose" => guose,
        "qianxun" => qianxun,
        "lianying" => lianying,
        "zhiheng" => zhiheng,
        "xiaoji" => xiaoji,
        "jieyin" => jieyin,
        "jiuyuan" => jiuyuan,
        "gongxin" => gongxin,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "kurou" => can_kurou,
        "fanjian" => can_fanjian,
        "guose" => can_guose,
        "zhiheng" => can_zhiheng,
        "jieyin" => can_jieyin,
        "gongxin" => can_gongxin,
        _ => return None,
    })
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
// 数值 / 牌面替代钩子
// ---------------------------------------------------------------------------

/// 奇袭：黑色牌（手牌或装备区）当【过河拆桥】。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    if !skills::active(g, seat, "qixi") {
        return None;
    }
    let black = g.card_defs.get(id).map(|c| !c.suit.is_red()).unwrap_or(false);
    if !black {
        return None;
    }
    let d = def_of(g, id)?;
    if d.code == "guohe_chaiqiao" {
        return None;
    }
    Some("guohe_chaiqiao")
}

/// 英姿：手牌上限 = 体力上限（赋值）。
pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if skills::active(g, seat, "yingzi") {
        return Some(g.seats.get(seat).map(|s| s.max_hp.max(0) as usize).unwrap_or(0));
    }
    None
}

/// 英姿：摸牌阶段多摸一张。
pub fn draw_bonus(g: &mut Game, seat: usize) -> usize {
    if skills::active(g, seat, "yingzi") {
        1
    } else {
        0
    }
}

/// 诈降：【杀】次数 +1（叠加）。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    g.counter(&turn_key(seat, "zhaxiang_sha"))
}

/// 诈降：红色【杀】无距离。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if g.counter(&turn_key(actor, "zhaxiang_red")) <= 0 {
        return;
    }
    let Some(d) = def_of(g, card) else { return };
    let red = g.card_defs.get(card).map(|c| c.suit.is_red()).unwrap_or(false);
    if d.tags.sha && red {
        super::super::card_play::declare_no_distance(g, card);
    }
}

/// 诈降：红色【杀】不能被**真实打出**的【闪】抵消。
pub fn no_real_shan(g: &Game, user: usize, card: &str) -> bool {
    if g.counter(&turn_key(user, "zhaxiang_red")) <= 0 {
        return false;
    }
    let red = g.card_defs.get(card).map(|c| c.suit.is_red()).unwrap_or(false);
    let sha = def_of(g, card).map(|d| d.tags.sha).unwrap_or(false);
    sha && red
}

/// 克己：本回合出牌阶段未使用【杀】→ 跳过弃牌阶段。
pub fn skip_discard(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "keji") && g.counter(&turn_key(seat, "sha_played")) == 0
}

/// 救援：吴势力角色于其回合内回复体力时可**选择**改为令主公回复。
///
/// 返回 true = 已压询问帧，本次回复由 `jiuyuan` handler 收口（不再走 `flow::heal`）。
pub fn heal_ask(g: &mut Game, target: usize, amount: i32, cause: &str) -> bool {
    if g.seats.get(target).map(|s| s.kingdom.as_str()) != Some("wu") {
        return false;
    }
    if g.turn != target || g.counter(&format!("jiuyuan.busy.{target}")) > 0 {
        return false;
    }
    let Some(lord) = g.current_lord() else { return false };
    if lord == target || !skills::active(g, lord, "jiuyuan") {
        return false;
    }
    if g.seats.get(lord).map(|s| s.hp >= s.max_hp).unwrap_or(true) {
        return false;
    }
    if g.seats[target].hp < g.seats[lord].hp {
        return false;
    }
    g.set_counter(&format!("jiuyuan.busy.{target}"), 1);
    super::super::resolve::push_ask(
        g,
        "jiuyuan",
        Stage::Choose,
        target,
        json!({ "step": "ask", "target": target, "lord": lord, "amount": amount, "cause": cause }),
    );
    true
}

fn jiuyuan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let target = common::seat_of(ask, "target").unwrap_or(ask.actor);
    let lord = common::seat_of(ask, "lord").unwrap_or(target);
    let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let cause = ask
        .data
        .get("cause")
        .and_then(|v| v.as_str())
        .unwrap_or("恢复")
        .to_string();
    match take_answer(g, "jiuyuan") {
        None => {
            let mut b = PendingBuilder::new(target, "choose").skill("jiuyuan");
            b = b.option("use", "改为令主公回复 1 点体力，然后你摸一张牌");
            Ok(Step::Need(
                b.escape("照常自己回复")
                    .data(json!({ "step": "ask", "target": target, "lord": lord, "amount": amount, "cause": cause }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => {
            // 收口本次回复：先清 busy 再补上「正在结算」标记，避免再次触发 救援 询问。
            g.set_counter(&format!("jiuyuan.busy.{target}"), 1);
            flow::heal(g, target, amount, &cause);
            g.set_counter(&format!("jiuyuan.busy.{target}"), 0);
            Ok(Step::Finish("救援：照常自己回复".into()))
        }
        Some(a) if a == "use" => {
            g.set_counter(&format!("jiuyuan.busy.{target}"), 0);
            flow::heal(g, lord, 1, &cause);
            flow::draw(g, target, 1);
            Ok(Step::Finish("救援：改为令主公回复，其摸一张".into()))
        }
        Some(o) => Err(format!("救援：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界甘宁 · 奋威（限定技：多目标牌可令任意个目标无效）
// ---------------------------------------------------------------------------

pub fn fire_multi_target(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    let owners: Vec<usize> = (0..g.seats.len())
        .filter(|&s| {
            g.seats[s].alive
                && skills::active(g, s, "fenwei")
                && !skills::limited_used(g, s, "fenwei")
        })
        .collect();
    let mut pushed = false;
    for s in owners {
        super::super::resolve::push_ask(
            g,
            "fenwei",
            Stage::Choose,
            s,
            json!({ "step": "fenwei", "user": user, "targets": targets, "card": card }),
        );
        pushed = true;
    }
    pushed
}

pub fn take_multi_invalid(g: &mut Game, card: &str) -> Option<Vec<usize>> {
    let key = format!("fenwei.invalid.{card}");
    let raw = g.mark(&key).map(|s| s.to_string())?;
    g.marks.remove(&key);
    serde_json::from_str::<Vec<usize>>(&raw).ok()
}

fn fenwei(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "fenwei") || skills::limited_used(g, actor, "fenwei") {
        return Ok(Step::Done);
    }
    let card = common::card_of(ask, "card").ok_or("奋威：缺牌")?;
    let targets = common::seats_of(ask, "targets");
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    match step {
        "ask" => match take_answer(g, "fenwei") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("fenwei");
                b = b.option("use", "奋威：令此牌对任意个目标无效");
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "ask", "card": card, "targets": targets }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "pick", "card": card, "targets": targets, "invalid": [] }),
            )),
            Some(o) => Err(format!("奋威：未知选项 {o}")),
        },
        "pick" => {
            let invalid = common::seats_of(ask, "invalid");
            let cands: Vec<usize> = targets.iter().copied().filter(|t| !invalid.contains(t)).collect();
            match take_answer(g, "fenwei") {
                None => {
                    let mut b = PendingBuilder::new(actor, "target").skill("fenwei");
                    for t in &cands {
                        b = b.option(
                            format!("t:{t}"),
                            format!("令此牌对{}无效", common::seat_name(g, *t)),
                        );
                    }
                    let escape = if invalid.is_empty() { "取消" } else { "完成" };
                    Ok(Step::Need(
                        b.escape(escape)
                            .data(json!({ "step": "pick", "card": card, "targets": targets, "invalid": invalid }))
                            .build(),
                    ))
                }
                Some(a) if a == "pass" => {
                    if invalid.is_empty() {
                        return Ok(Step::Finish("奋威：取消".into()));
                    }
                    skills::mark_limited_used(g, actor, "fenwei");
                    g.set_mark(
                        &format!("fenwei.invalid.{card}"),
                        serde_json::to_string(&invalid).unwrap_or_default(),
                    );
                    Ok(Step::Finish("奋威：令部分目标无效".into()))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "奋威：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("奋威：目标不合法".into());
                    }
                    let mut next = invalid.clone();
                    next.push(t);
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "pick", "card": card, "targets": targets, "invalid": next }),
                    ))
                }
                Some(o) => Err(format!("奋威：未知选项 {o}")),
            }
        }
        other => Err(format!("奋威：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界吕蒙 · 勤学 / 博图
// ---------------------------------------------------------------------------

fn qinxue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "qinxue") || g.turn != actor {
        return Ok(Step::Done);
    }
    if g.counter(&format!("awaken.{actor}.qinxue")) > 0 {
        return Ok(Step::Done);
    }
    if g.phase != Phase::Prepare && g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let need = g.hand_size(actor) as i32 >= g.seats[actor].hp + 2;
    if !need {
        return Ok(Step::Done);
    }
    match take_answer(g, "qinxue") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("qinxue");
            b = b.option("opt1", "觉醒：减 1 点体力上限，回复 1 点体力");
            b = b.option("opt2", "觉醒：减 1 点体力上限，摸两张牌");
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "choose" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "opt1" || a == "opt2" => {
            g.set_counter(&format!("awaken.{actor}.qinxue"), 1);
            flow::lose_max_hp(g, actor, 1, "勤学");
            if a == "opt1" {
                flow::heal(g, actor, 1, "勤学");
            } else {
                flow::draw(g, actor, 2);
            }
            skills::grant(g, actor, "gongxin");
            Ok(Step::Finish("勤学：觉醒（获得【攻心】）".into()))
        }
        Some(o) => Err(format!("勤学：未知选项 {o}")),
    }
}

/// 谦逊：延时锦囊判定生效时（`turn::judge_phase` 唯一调用点）。
pub fn on_delayed_judge(g: &mut Game, seat: usize, card: &str) {
    if !skills::active(g, seat, "qianxun") {
        return;
    }
    let delayed = def_of(g, card).map(|d| d.tags.delayed).unwrap_or(false);
    if !delayed || g.hand(seat).is_empty() {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "qianxun",
        Stage::Choose,
        seat,
        json!({ "step": "ask", "card": card, "delayed": true }),
    );
}

pub fn on_turn_end(g: &mut Game, seat: usize) {
    // 谦逊：回合结束归还在本回合移出游戏的手牌。
    let key = format!("qianxun.void.{seat}");
    if let Some(raw) = g.mark(&key).map(|s| s.to_string()) {
        g.marks.remove(&key);
        if let Ok(ids) = serde_json::from_str::<Vec<CardId>>(&raw) {
            for id in ids {
                if matches!(g.cards.get(&id), Some(Zone::Void)) {
                    g.cards.insert(id.clone(), Zone::Hand(seat));
                }
            }
        }
    }
    // 诈降的状态在本回合结束时清空（计数在 turn.* 已经随回合清理）。
    // 博图：额外回合内不再触发。
    if g.counter(&format!("botu.extra.{seat}")) > 0 {
        g.set_counter(&format!("botu.extra.{seat}"), 0);
        return;
    }
    if !skills::active(g, seat, "botu") {
        return;
    }
    let suits: Vec<String> = g
        .mark(&format!("turn.{seat}.discard_suits"))
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
        .unwrap_or_default();
    if suits.len() < 4 {
        return;
    }
    let limit = common::alive_seats(g).len().min(3) as i32;
    let used_key = format!("botu.round.{}.{seat}", g.round);
    if g.counter(&used_key) >= limit {
        return;
    }
    g.bump(&used_key);
    g.set_counter(&format!("botu.extra.{seat}"), 1);
    super::super::turn::extra_turn(g, seat);
}

// ---------------------------------------------------------------------------
// 界黄盖 · 苦肉 / 诈降
// ---------------------------------------------------------------------------

fn can_kurou(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "kurou")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "kurou")) == 0
        && !own_cards(g, seat).is_empty()
}

fn kurou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !can_kurou(g, actor) {
        return Ok(Step::Done);
    }
    let cands = own_cards(g, actor);
    match take_answer(g, "kurou") {
        None => {
            let mut b = PendingBuilder::new(actor, "pick_own").skill("kurou");
            for id in &cands {
                b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选一张）")
                    .data(json!({ "step": "choose" }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.iter().any(|c| *c == id) {
                return Err("苦肉：只能弃置自己的牌".into());
            }
            flow::discard(g, actor, &id, "苦肉");
            flow::lose_hp(g, actor, 1, "苦肉");
            g.bump(&phase_key(actor, "kurou"));
            Ok(Step::Finish("苦肉：弃一张牌并失去 1 点体力".into()))
        }
        Some(o) => Err(format!("苦肉：未知选项 {o}")),
    }
}

fn zhaxiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhaxiang") {
        return Ok(Step::Done);
    }
    flow::draw(g, actor, 3);
    if g.phase == Phase::Play && g.turn == actor {
        let k = turn_key(actor, "zhaxiang_sha");
        g.bump(&k);
        g.set_counter(&turn_key(actor, "zhaxiang_red"), 1);
    }
    Ok(Step::Finish("诈降：摸三张牌".into()))
}

// ---------------------------------------------------------------------------
// 界周瑜 · 反间
// ---------------------------------------------------------------------------

fn can_fanjian(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "fanjian")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "fanjian")) == 0
        && !g.hand(seat).is_empty()
        && !common::alive_others(g, seat).is_empty()
}

fn fanjian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "fanjian") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands = common::alive_others(g, actor);
            match take_answer(g, "fanjian") {
                None => common::pick_seat(g, actor, "fanjian", "交给", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "反间：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("反间：目标不合法".into());
                    }
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "card", "target": t })))
                }
                Some(o) => Err(format!("反间：未知选项 {o}")),
            }
        }
        "card" => {
            let t = common::seat_of(ask, "target").ok_or("反间：缺目标")?;
            let hand = g.hand(actor);
            match take_answer(g, "fanjian") {
                None => common::must_pick(
                    g,
                    actor,
                    "fanjian",
                    "pick_own",
                    "交给",
                    &hand,
                    json!({ "step": "card", "target": t }),
                )
                .into_step(),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.iter().any(|c| *c == id) {
                        return Err("反间：只能交出自己的手牌".into());
                    }
                    g.reveal(&[id.clone()], "反间");
                    flow::gain(g, t, &id, actor, "反间");
                    g.bump(&phase_key(actor, "fanjian"));
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "opt", "target": t, "card": id }),
                    ))
                }
                Some(o) => Err(format!("反间：未知选项 {o}")),
            }
        }
        "opt" => {
            let t = common::seat_of(ask, "target").ok_or("反间：缺目标")?;
            let card = common::card_of(ask, "card").unwrap_or_default();
            match take_answer(g, "fanjian") {
                None => {
                    let mut b = PendingBuilder::new(t, "choose").skill("fanjian");
                    b = b.option("opt1", "展示所有手牌，弃置其中与此牌相同花色的牌");
                    b = b.option("opt2", "失去 1 点体力");
                    Ok(Step::Need(
                        b.escape_mandatory("选择一项")
                            .data(json!({ "step": "opt", "target": t, "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a == "opt1" => {
                    let hand = g.hand(t);
                    g.reveal(&hand, "反间");
                    let suit = g.card_defs.get(&card).map(|c| c.suit);
                    let same: Vec<CardId> = hand
                        .into_iter()
                        .filter(|id| g.card_defs.get(id).map(|c| Some(c.suit) == suit).unwrap_or(false))
                        .collect();
                    for id in &same {
                        flow::discard(g, t, id, "反间");
                    }
                    g.conceal(&same, "反间结算完成");
                    Ok(Step::Finish("反间：其弃置同花色手牌".into()))
                }
                Some(a) if a == "opt2" => {
                    flow::lose_hp(g, t, 1, "反间");
                    Ok(Step::Finish("反间：其失去 1 点体力".into()))
                }
                Some(o) => Err(format!("反间：未知选项 {o}")),
            }
        }
        other => Err(format!("反间：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界大乔 · 流离 / 国色
// ---------------------------------------------------------------------------

fn liuli(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "liuli") {
        return Ok(Step::Done);
    }
    let Some(by) = common::seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    let card = common::card_of(ask, "card").ok_or("流离：缺牌")?;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("sha_intercept");
    // 前置只校验首次入口（后续阶段手牌可能已弃空）。
    if step == "sha_intercept" {
        let cands: Vec<usize> = common::alive_others(g, actor)
            .into_iter()
            .filter(|s| *s != by)
            .filter(|s| gear::within_attack_range(g, actor, *s, gear::attack_range(g, actor) as usize))
            .collect();
        if cands.is_empty() || own_cards(g, actor).is_empty() {
            return Ok(Step::Done);
        }
    }
    match step {
        "sha_intercept" => match take_answer(g, "liuli") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("liuli");
                b = b.option("use", "流离：弃一张牌，将此【杀】转移给其他角色");
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "sha_intercept", "by": by, "card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "discard", "by": by, "card": card }),
            )),
            Some(o) => Err(format!("流离：未知选项 {o}")),
        },
        "discard" => {
            let cands = own_cards(g, actor);
            match take_answer(g, "liuli") {
                None => common::must_pick(
                    g,
                    actor,
                    "liuli",
                    "pick_own",
                    "弃置",
                    &cands,
                    json!({ "step": "discard", "by": by, "card": card }),
                )
                .into_step(),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("流离：只能弃置自己的牌".into());
                    }
                    flow::discard(g, actor, &id, "流离");
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "pick", "by": by, "card": card }),
                    ))
                }
                Some(o) => Err(format!("流离：未知选项 {o}")),
            }
        }
        "pick" => {
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|s| *s != by)
                .filter(|s| gear::within_attack_range(g, actor, *s, gear::attack_range(g, actor) as usize))
                .collect();
            match take_answer(g, "liuli") {
                None => {
                    let mut b = PendingBuilder::new(actor, "target").skill("liuli");
                    for s in &cands {
                        b = b.option(format!("t:{s}"), format!("转移给{}", common::seat_name(g, *s)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一名）")
                            .data(json!({ "step": "pick", "by": by, "card": card }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "流离：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("流离：目标不合法".into());
                    }
                    super::super::card_play::declare_intercept(
                        g,
                        &card,
                        super::super::ShaIntercept::Replace(t),
                    );
                    Ok(Step::Finish("流离：转移此【杀】".into()))
                }
                Some(o) => Err(format!("流离：未知选项 {o}")),
            }
        }
        other => Err(format!("流离：未知阶段 {other}")),
    }
}

fn can_guose(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "guose") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.counter(&phase_key(seat, "guose")) > 0 {
        return false;
    }
    own_cards(g, seat)
        .iter()
        .any(|id| g.card_defs.get(id).map(|c| c.suit == Suit::Diamond).unwrap_or(false))
}

fn first_indulgence(g: &Game) -> Option<(usize, CardId)> {
    for seat in 0..g.seats.len() {
        if let Some(stack) = g.judge_stacks.get(&seat) {
            for id in stack {
                if g.card_defs.get(id).map(|c| c.def == "lebu_sishu").unwrap_or(false) {
                    return Some((seat, id.clone()));
                }
            }
        }
    }
    None
}

fn guose(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "guose") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    match step {
        "choose" => {
            let cands = own_cards(g, actor)
                .into_iter()
                .filter(|id| g.card_defs.get(id).map(|c| c.suit == Suit::Diamond).unwrap_or(false))
                .collect::<Vec<_>>();
            if cands.is_empty() || g.counter(&phase_key(actor, "guose")) > 0 {
                return Ok(Step::Done);
            }
            let has_le = first_indulgence(g).is_some();
            match take_answer(g, "guose") {
                None => {
                    let mut b = PendingBuilder::new(actor, "choose").skill("guose");
                    b = b.option("opt1", "将一张方片牌当【乐不思蜀】使用，然后摸一张");
                    if has_le {
                        b = b.option("opt2", "弃置一张方片牌，再弃置场上第一张【乐不思蜀】");
                    }
                    Ok(Step::Need(b.escape("不发动").data(json!({ "step": "choose" })).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "opt1" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "opt1", "cands": cands }),
                )),
                Some(a) if a == "opt2" && has_le => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "opt2", "cands": cands }),
                )),
                Some(o) => Err(format!("国色：未知选项 {o}")),
            }
        }
        "opt1" => {
            let cands = common::cards_of(ask, "cands");
            match take_answer(g, "guose") {
                None => common::must_pick(
                    g,
                    actor,
                    "guose",
                    "pick_own",
                    "使用",
                    &cands,
                    json!({ "step": "opt1", "cands": cands }),
                )
                .into_step(),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("国色：只能使用方片牌".into());
                    }
                    super::super::card_play::use_as(g, actor, "lebu_sishu", &[id], &[], false)?;
                    flow::draw(g, actor, 1);
                    g.bump(&phase_key(actor, "guose"));
                    Ok(Step::Finish("国色：方片当【乐不思蜀】并摸一张".into()))
                }
                Some(o) => Err(format!("国色：未知选项 {o}")),
            }
        }
        "opt2" => {
            let cands = common::cards_of(ask, "cands");
            match take_answer(g, "guose") {
                None => common::must_pick(
                    g,
                    actor,
                    "guose",
                    "pick_own",
                    "弃置",
                    &cands,
                    json!({ "step": "opt2", "cands": cands }),
                )
                .into_step(),
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !cands.iter().any(|c| *c == id) {
                        return Err("国色：只能弃置方片牌".into());
                    }
                    flow::discard(g, actor, &id, "国色");
                    if let Some((seat, le)) = first_indulgence(g) {
                        super::super::card_play::remove_card(g, seat, &le, "国色");
                    }
                    g.bump(&phase_key(actor, "guose"));
                    Ok(Step::Finish("国色：弃方片并弃置一张【乐不思蜀】".into()))
                }
                Some(o) => Err(format!("国色：未知选项 {o}")),
            }
        }
        other => Err(format!("国色：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界陆逊 · 谦逊 / 连营
// ---------------------------------------------------------------------------

fn qianxun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "qianxun") {
        return Ok(Step::Done);
    }
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let delayed = ask.data.get("delayed").is_some();
    if delayed {
        // 延时锦囊：判定生效时，唯一目标是判定区持有者。
        if !def_of(g, &card).map(|d| d.tags.delayed).unwrap_or(false) {
            return Ok(Step::Done);
        }
        if g.judge_stacks.get(&actor).map(|s| s.is_empty()).unwrap_or(true)
            && !g.cards.values().any(|z| *z == Zone::Processing)
        {
            // 判定牌已被移出判定区：不再依赖栈内状态，直接按触发处理。
        }
    } else {
        let Some(by) = common::seat_of(ask, "by") else {
            return Ok(Step::Done);
        };
        if by == actor {
            return Ok(Step::Done);
        }
        let Some(d) = def_of(g, &card) else { return Ok(Step::Done) };
        if !d.effect.is_trick() {
            return Ok(Step::Done);
        }
        if g.use_targets.get(&card).map(|t| t.len()).unwrap_or(0) != 1 {
            return Ok(Step::Done);
        }
    }
    let hand = g.hand(actor);
    if hand.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "qianxun") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("qianxun");
            b = b.option("use", "谦逊：将所有手牌移出游戏直到回合结束");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "card": card }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let n = hand.len();
            for id in &hand {
                g.cards.insert(id.clone(), Zone::Void);
                let mut ev = Event::card(EventKind::CardOutOfGame, Some(actor), id.clone(), Some("谦逊".to_string()));
                ev.by = Some(actor);
                let e = ev.snapshot(g);
                g.push_event(e);
            }
            g.set_mark(
                &format!("qianxun.void.{actor}"),
                serde_json::to_string(&hand).unwrap_or_default(),
            );
            // 连营：移出全部手牌 → X = 移出张数。
            super::super::skills::on_hand_empty(g, actor, n);
            Ok(Step::Finish(format!("谦逊：移出 {n} 张手牌直到回合结束")))
        }
        Some(o) => Err(format!("谦逊：未知选项 {o}")),
    }
}

fn lianying(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "lianying") {
        return Ok(Step::Done);
    }
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    let cands = common::alive_seats(g);
    match take_answer(g, "lianying") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("lianying");
            for s in &cands {
                b = b.option(
                    format!("t:{s}"),
                    format!("令{}摸一张牌（剩余 {} 名）", common::seat_name(g, *s), left),
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
            let t: usize = a[2..].parse().map_err(|_| "连营：目标不合法")?;
            if !cands.contains(&t) {
                return Err("连营：目标不合法".into());
            }
            flow::draw(g, t, 1);
            let next = left - 1;
            if next == 0 {
                Ok(Step::Finish("连营：结算完成".into()))
            } else {
                Ok(Step::Next(Stage::Target, json!({ "left": next })))
            }
        }
        Some(o) => Err(format!("连营：未知选项 {o}")),
    }
}

pub fn on_hand_empty(g: &mut Game, seat: usize, x: usize) {
    if !skills::active(g, seat, "lianying") {
        return;
    }
    if !g.hand(seat).is_empty() {
        return;
    }
    if x == 0 {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "lianying",
        Stage::Target,
        seat,
        json!({ "left": x }),
    );
}

// ---------------------------------------------------------------------------
// 界孙权 · 制衡 / 救援
// ---------------------------------------------------------------------------

fn can_zhiheng(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "zhiheng")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "zhiheng")) == 0
        && !own_cards(g, seat).is_empty()
}

fn zhiheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "zhiheng") {
        return Ok(Step::Done);
    }
    let picked = common::cards_of(ask, "picked");
    let cands: Vec<CardId> = own_cards(g, actor)
        .into_iter()
        .filter(|id| !picked.contains(id))
        .collect();
    if cands.is_empty() {
        return Ok(zhiheng_finish(g, actor, &picked));
    }
    match take_answer(g, "zhiheng") {
        None => {
            let mut b = PendingBuilder::new(actor, "pick_own").skill("zhiheng");
            for id in &cands {
                b = b.option(
                    format!("card:{id}"),
                    format!("弃置【{}】（已选 {} 张）", card_label(g, id), picked.len()),
                );
            }
            let escape = if picked.is_empty() { "取消".to_string() } else { "完成制衡".to_string() };
            Ok(Step::Need(
                b.escape(escape)
                    .data(json!({ "step": "pick", "picked": picked }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => {
            if picked.is_empty() {
                return Ok(Step::Done);
            }
            Ok(zhiheng_finish(g, actor, &picked))
        }
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.iter().any(|c| *c == id) {
                return Err("制衡：只能弃置自己的牌".into());
            }
            let mut next = picked.clone();
            next.push(id);
            Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "picked": next }),
            ))
        }
        Some(o) => Err(format!("制衡：未知选项 {o}")),
    }
}

fn zhiheng_finish(g: &mut Game, actor: usize, picked: &[CardId]) -> Step {
    let n = picked.len();
    for id in picked {
        flow::discard(g, actor, id, "制衡");
    }
    // 「弃置后手牌为空」在摸牌前判定。
    let emptied = g.hand(actor).is_empty();
    flow::draw(g, actor, if emptied { n + 1 } else { n });
    g.bump(&phase_key(actor, "zhiheng"));
    Step::Finish(format!("制衡：弃 {n} 张摸 {n} 张"))
}

// ---------------------------------------------------------------------------
// 勤学授予 · 攻心（观看一名其他角色手牌，可展示其一张红桃牌并弃置/置顶）
// ---------------------------------------------------------------------------

fn can_gongxin(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "gongxin")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "gongxin")) == 0
        && common::alive_others(g, seat)
            .into_iter()
            .any(|t| !g.hand(t).is_empty())
}

fn gongxin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "gongxin") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    // 主动入口首帧（`active_actions` 压 `step=choose`）与目标选择同义。
    let step = if step == "choose" { "target" } else { step };
    match step {
        "target" => {
            if g.counter(&phase_key(actor, "gongxin")) > 0 {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|t| !g.hand(*t).is_empty())
                .collect();
            match take_answer(g, "gongxin") {
                None => common::pick_seat(g, actor, "gongxin", "观看", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "攻心：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("攻心：目标不合法".into());
                    }
                    // 私密观看：pending 只发给发动者，选项文案即该角色的手牌信息。
                    let hearts: Vec<CardId> = g
                        .hand(t)
                        .into_iter()
                        .filter(|id| g.card_defs.get(id).map(|c| c.suit == Suit::Heart).unwrap_or(false))
                        .collect();
                    let mut b = PendingBuilder::new(actor, "choose").skill("gongxin");
                    for id in &hearts {
                        b = b.option(format!("discard:{id}"), format!("展示并弃置其【{}】", card_label(g, id)));
                        b = b.option(format!("top:{id}"), format!("展示并将其【{}】置于牌堆顶", card_label(g, id)));
                    }
                    Ok(Step::NeedWith(
                        Stage::Choose,
                        json!({ "step": "pick", "target": t, "hearts": hearts }),
                        b.escape("放弃（不展示红桃牌）")
                            .data(json!({ "step": "pick", "target": t, "hearts": hearts }))
                            .build(),
                    ))
                }
                Some(o) => Err(format!("攻心：未知选项 {o}")),
            }
        }
        "pick" => {
            let t = common::seat_of(ask, "target").ok_or("攻心：缺目标")?;
            match take_answer(g, "gongxin") {
                None => Ok(Step::Done),
                Some(a) if a == "pass" => {
                    g.bump(&phase_key(actor, "gongxin"));
                    Ok(Step::Finish("攻心：观看其手牌".into()))
                }
                Some(a) if a.starts_with("discard:") => {
                    let id = a[8..].to_string();
                    g.reveal(&[id.clone()], "攻心");
                    flow::discard(g, t, &id, "攻心");
                    g.bump(&phase_key(actor, "gongxin"));
                    Ok(Step::Finish("攻心：展示并弃置其红桃牌".into()))
                }
                Some(a) if a.starts_with("top:") => {
                    let id = a[4..].to_string();
                    g.reveal(&[id.clone()], "攻心");
                    if matches!(g.cards.get(&id), Some(Zone::Hand(s)) if *s == t) {
                        flow::to_deck_top(g, &id);
                    }
                    g.bump(&phase_key(actor, "gongxin"));
                    Ok(Step::Finish("攻心：展示并将其红桃牌置于牌堆顶".into()))
                }
                Some(o) => Err(format!("攻心：未知选项 {o}")),
            }
        }
        other => Err(format!("攻心：未知阶段 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 界孙尚香 · 枭姬 / 结姻
// ---------------------------------------------------------------------------

pub fn on_equip_lost(g: &mut Game, owner: usize, id: &str) {
    let _ = id;
    if !skills::active(g, owner, "xiaoji") {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "xiaoji",
        Stage::Choose,
        owner,
        json!({ "step": "ask" }),
    );
}

fn xiaoji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "xiaoji") {
        return Ok(Step::Done);
    }
    match take_answer(g, "xiaoji") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("xiaoji");
            b = b.option("use", "枭姬：摸两张牌");
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "ask" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, actor, 2);
            Ok(Step::Finish("枭姬：摸两张牌".into()))
        }
        Some(o) => Err(format!("枭姬：未知选项 {o}")),
    }
}

fn can_jieyin(g: &Game, seat: usize) -> bool {
    if !skills::active(g, seat, "jieyin") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.counter(&phase_key(seat, "jieyin")) > 0 {
        return false;
    }
    if g.hand(seat).is_empty() && common::equip_cards(g, seat).is_empty() {
        return false;
    }
    common::alive_others(g, seat)
        .into_iter()
        .any(|s| g.seats[s].male)
}

fn jieyin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if !skills::active(g, actor, "jieyin") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("target");
    match step {
        "target" => {
            let cands: Vec<usize> = common::alive_others(g, actor)
                .into_iter()
                .filter(|s| g.seats[*s].male)
                .collect();
            if cands.is_empty() || g.counter(&phase_key(actor, "jieyin")) > 0 {
                return Ok(Step::Done);
            }
            match take_answer(g, "jieyin") {
                None => common::pick_seat(g, actor, "jieyin", "与", &cands, json!({ "step": "target" })).into_step(),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "结姻：目标不合法")?;
                    if !cands.contains(&t) {
                        return Err("结姻：目标不合法".into());
                    }
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "pay", "target": t })))
                }
                Some(o) => Err(format!("结姻：未知选项 {o}")),
            }
        }
        "pay" => {
            let t = common::seat_of(ask, "target").ok_or("结姻：缺目标")?;
            let mut b = PendingBuilder::new(actor, "pick_own").skill("jieyin");
            for id in g.hand(actor) {
                b = b.option(format!("hand:{id}"), format!("弃置手牌【{}】", card_label(g, &id)));
            }
            for id in common::equip_cards(g, actor) {
                b = b.option(
                    format!("equip:{id}"),
                    format!("将装备【{}】置入其装备区", card_label(g, &id)),
                );
            }
            match take_answer(g, "jieyin") {
                None => Ok(Step::Need(
                    b.escape_mandatory("（必选一张）")
                        .data(json!({ "step": "pay", "target": t }))
                        .build(),
                )),
                Some(a) if a.starts_with("hand:") => {
                    let id = a[5..].to_string();
                    if !g.hand(actor).iter().any(|c| *c == id) {
                        return Err("结姻：只能弃置自己的手牌".into());
                    }
                    flow::discard(g, actor, &id, "结姻");
                    Ok(jieyin_settle(g, actor, t))
                }
                Some(a) if a.starts_with("equip:") => {
                    let id = a[6..].to_string();
                    if !common::equip_cards(g, actor).contains(&id) {
                        return Err("结姻：只能用自己的装备".into());
                    }
                    equip_to(g, actor, t, &id)?;
                    Ok(jieyin_settle(g, actor, t))
                }
                Some(o) => Err(format!("结姻：未知选项 {o}")),
            }
        }
        other => Err(format!("结姻：未知阶段 {other}")),
    }
}

/// 将一张装备牌置入另一名角色的装备区（手的牌或自己装备区的牌）。
fn equip_to(g: &mut Game, from: usize, to: usize, id: &str) -> Result<(), String> {
    let Some(def) = def_of(g, id) else {
        return Err("结姻：牌不存在".into());
    };
    let Some(slot) = def.slot else {
        return Err("结姻：不是装备牌".into());
    };
    if g.slot_abolished(to, slot) {
        return Err("结姻：目标装备栏已废除".into());
    }
    let zone = g.cards.get(id).cloned();
    let was_equip = matches!(zone, Some(Zone::Equip(s, _)) if s == from);
    if !was_equip && !matches!(zone, Some(Zone::Hand(s)) if s == from) {
        return Err("结姻：装备牌必须来自自己的手牌或装备区".into());
    }
    let mut lost = Event::card(EventKind::CardLost, Some(from), id.to_string(), Some("结姻".to_string()));
    lost.by = Some(from);
    lost.from = Some(from);
    let e = lost.snapshot(g);
    g.push_event(e);
    if was_equip {
        flow::leave_equip(g, from, id, "结姻");
    }
    // 顶掉目标同槽旧装备。
    if let Some(old) = gear::equip_in(g, to, slot).cloned() {
        flow::discard(g, to, &old, "结姻");
    }
    g.cards.insert(id.to_string(), Zone::Equip(to, slot));
    let ev = Event::simple(EventKind::CardIntoGame, Some(to), Some("结姻"));
    let mut ev = ev;
    ev.card = Some(id.to_string());
    let e = ev.snapshot(g);
    g.push_event(e);
    Ok(())
}

fn jieyin_settle(g: &mut Game, actor: usize, target: usize) -> Step {
    g.bump(&phase_key(actor, "jieyin"));
    let a = g.seats[actor].hp;
    let b = g.seats[target].hp;
    if a > b {
        flow::draw(g, actor, 1);
        flow::heal(g, target, 1, "结姻");
    } else if b > a {
        flow::draw(g, target, 1);
        flow::heal(g, actor, 1, "结姻");
    } else {
        flow::draw(g, actor, 1);
        flow::draw(g, target, 1);
        flow::heal(g, actor, 1, "结姻");
        flow::heal(g, target, 1, "结姻");
    }
    Step::Finish("结姻：结算体力比较".into())
}

// 保留：与 Identity/Slot 相关的边界声明（避免未使用 import 噪音）。
#[allow(dead_code)]
fn _unused(_i: Identity, _s: Slot, _c: Category) {}
