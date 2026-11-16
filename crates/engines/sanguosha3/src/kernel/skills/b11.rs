//! B11 技能（界限突破·界标·魏：8 将 / 14 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B11.json` 的 `agreed`（权威）。
//!
//! 内核新增（本批）：`judge-rewrite`（`kernel/quick_judge.rs`：亮牌 → 鬼才改判 → 生效 → 天妒），
//! 摸牌阶段替代（突袭/裸衣/恂恂经 `PhaseStart` 询问 + `skip_draw`），伤害后逐点触发（反馈/遗计）。

use serde_json::json;

use super::super::cards::{Category, ResponseKind};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::relay;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, DamageKind, DamageSpec, Game, Identity, Phase, SourceKind, Zone};
use super::common::{self, card_label, def_of, own_cards, seat_name, turn_key};
use super::{CardAs, Face, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界曹操 ——
    SkillDef { code: "jianxiong", name: "奸雄", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "hujia", name: "护驾", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::CardUsed, subject: Subject::Other },
            Subscription { event: EventKind::CardPlayed, subject: Subject::Other },
        ], hooks: &[], can_active: false }] },
    // —— 界司马懿 ——
    SkillDef { code: "fankui", name: "反馈", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "guicai", name: "鬼才", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[super::Hook::JudgeRewrite], can_active: false }] },
    // —— 界夏侯惇 ——
    SkillDef { code: "ganglie", name: "刚烈", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "qingjian", name: "清俭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardGained, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    // —— 界张辽 ——
    SkillDef { code: "tuxi", name: "突袭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界许褚 ——
    SkillDef { code: "luoyi", name: "裸衣", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    // —— 界郭嘉 ——
    SkillDef { code: "tiandu", name: "天妒", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "yiji", name: "遗计", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    // —— 界李典 ——
    SkillDef { code: "xunxun", name: "恂恂", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "wangxi", name: "忘隙", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ },
            Subscription { event: EventKind::DamageAfter, subject: Subject::By },
        ], hooks: &[], can_active: false }] },
    // —— 界甄姬 ——
    SkillDef { code: "qingguo", name: "倾国", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "luoshen", name: "洛神", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "jianxiong" => jianxiong,
        "hujia" => hujia,
        "fankui" => fankui,
        "guicai" => guicai,
        "ganglie" => ganglie,
        "qingjian" => qingjian,
        "tuxi" => tuxi,
        "luoyi" => luoyi,
        "tiandu" => tiandu,
        "yiji" => yiji,
        "xunxun" => xunxun,
        "wangxi" => wangxi,
        "luoshen" => luoshen,
        _ => return None,
    })
}

/// B11 无出牌阶段主动技（全部为触发/替代/转化）。
pub fn availability(_code: &str) -> Option<fn(&Game, usize) -> bool> {
    None
}

fn step_of(ask: &Ask) -> &str {
    ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask")
}

fn event_card(ask: &Ask) -> Option<CardId> {
    ask.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn is_owner_in(zone: Option<&Zone>, seat: usize) -> bool {
    matches!(zone, Some(Zone::Hand(s)) | Some(Zone::Equip(s, _)) if *s == seat)
}

/// 从目标座位的「一张牌」里选：装备区可见可选，手牌只给一个「随机」选项（对齐 B11 反馈/刚烈）。
fn pick_one_from(
    g: &Game,
    actor: usize,
    owner: usize,
    skill: &'static str,
    verb: &str,
    data: serde_json::Value,
) -> Option<Step> {
    let mut b = PendingBuilder::new(actor, "pick_other").skill(skill);
    let mut any = false;
    for id in common::equip_cards(g, owner) {
        b = b.option(
            format!("equip:{id}"),
            format!("{verb}{}的【{}】", seat_name(g, owner), card_label(g, &id)),
        );
        any = true;
    }
    if !g.hand(owner).is_empty() {
        b = b.option(
            "hand",
            format!("{verb}{}一张手牌（随机）", seat_name(g, owner)),
        );
        any = true;
    }
    if !any {
        return None;
    }
    Some(Step::Need(b.escape("不选").data(data).build()))
}

fn resolve_pick(g: &mut Game, owner: usize, ans: &str) -> Option<CardId> {
    if ans == "hand" {
        let hand = g.hand(owner);
        if hand.is_empty() {
            return None;
        }
        let i = g.rand_below(hand.len());
        return hand.get(i).cloned();
    }
    ans.strip_prefix("equip:").map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// 界曹操 · 奸雄（受到伤害后：摸 1 + 可选获得造成伤害的牌；两件事解耦）
// ---------------------------------------------------------------------------

fn jianxiong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "jianxiong") {
        return Ok(Step::Done);
    }
    if step_of(ask) == "gain" {
        let Some(card) = event_card(ask) else {
            return Ok(Step::Done);
        };
        return match take_answer(g, "jianxiong") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("jianxiong");
                b = b.option("gain", format!("获得【{}】", card_label(g, &card)));
                Ok(Step::Need(
                    b.escape("不获得")
                        .data(json!({ "step": "gain", "card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "gain" => {
                if matches!(g.cards.get(&card), Some(Zone::Processing)) {
                    flow::gain(g, owner, &card, owner, "奸雄");
                }
                Ok(Step::Finish("奸雄：获得造成伤害的牌".into()))
            }
            Some(o) => Err(format!("奸雄：未知选项 {o}")),
        };
    }
    // ①「摸一张牌」与「获得伤害牌」解耦：受到伤害即摸 1。
    flow::draw(g, owner, 1);
    // ② 伤害牌仍在处理区 → 独立询问是否获得。
    let Some(card) = event_card(ask) else {
        return Ok(Step::Done);
    };
    if !matches!(g.cards.get(&card), Some(Zone::Processing)) {
        return Ok(Step::Finish("奸雄：摸一张（伤害牌已离场）".into()));
    }
    Ok(Step::Next(
        Stage::Choose,
        json!({ "step": "gain", "card": card }),
    ))
}

// ---------------------------------------------------------------------------
// 界曹操 · 护驾（主公技：魏势力代出【闪】；其回合外使用/打出【闪】可令主公摸 1）
// ---------------------------------------------------------------------------

fn hujia(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    let step = step_of(ask);
    // ① 响应窗口接力（流程帧用统一 `alt:hujia` 选项压入本帧）。
    if step == "alt" || step == "alt_wait" {
        if let Some(r) = g.mark(&format!("relay.result.{owner}")).map(|s| s.to_string()) {
            g.marks.remove(&format!("relay.result.{owner}"));
            if r.starts_with("played:") {
                g.set_mark(&format!("response.sub.{owner}"), "1");
            }
            return Ok(Step::Finish("护驾：响应结算".into()));
        }
        if relay::order(g, owner, "wei", ResponseKind::Shan).is_empty() {
            return Ok(Step::Done);
        }
        let started = relay::start(
            g,
            owner,
            relay::RelayOpts {
                kingdom: "wei".to_string(),
                kind: ResponseKind::Shan,
                for_use: false,
                cause: "护驾",
            },
        );
        if !started {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Choose, json!({ "step": "alt_wait" })));
    }
    // ② 其他魏势力角色于其回合外使用/打出（含替主公打出）【闪】→ 其可选择令主公摸 1。
    let by = ask
        .data
        .get("by")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(owner);
    if by == owner || g.turn == by {
        return Ok(Step::Done);
    }
    if g.seats.get(by).map(|s| s.kingdom.as_str()) != Some("wei") {
        return Ok(Step::Done);
    }
    let Some(card) = event_card(ask) else {
        return Ok(Step::Done);
    };
    if !def_of(g, &card).map(|d| d.tags.shan).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let Some(lord) = g
        .seats
        .iter()
        .position(|s| s.alive && s.identity == Some(Identity::Lord) && skills::active(g, s.id, "hujia"))
    else {
        return Ok(Step::Done);
    };
    if g.counter(&turn_key(lord, "hujia_draw")) > 0 {
        return Ok(Step::Done);
    }
    return match take_answer(g, "hujia") {
        None => {
            let mut b = PendingBuilder::new(by, "choose").skill("hujia");
            b = b.option("draw", "令主公摸一张牌");
            Ok(Step::Need(
                b.escape("不令主公摸牌")
                    .data(json!({ "step": "trigger", "lord": lord }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "draw" => {
            g.bump(&turn_key(lord, "hujia_draw"));
            flow::draw(g, lord, 1);
            Ok(Step::Finish("护驾：主公摸一张".into()))
        }
        Some(o) => Err(format!("护驾：未知选项 {o}")),
    };
}

/// 护驾：需要【闪】时的响应替代（主公 + 有魏势力可代出）。
pub fn response_alternatives(
    g: &Game,
    seat: usize,
    kind: ResponseKind,
    _ctx: &str,
) -> Vec<(&'static str, String)> {
    if kind != ResponseKind::Shan {
        return Vec::new();
    }
    if !skills::active(g, seat, "hujia") || g.seats.get(seat).and_then(|s| s.identity) != Some(Identity::Lord)
    {
        return Vec::new();
    }
    if relay::order(g, seat, "wei", ResponseKind::Shan).is_empty() {
        return Vec::new();
    }
    vec![("hujia", "护驾：令魏势力角色代你打出【闪】".to_string())]
}

// ---------------------------------------------------------------------------
// 界司马懿 · 反馈（每受到 1 点伤害：获得来源一张牌；手牌随机 / 装备区可选）
// ---------------------------------------------------------------------------

fn fankui(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "fankui") {
        return Ok(Step::Done);
    }
    let (src, left) = if step_of(ask) == "pick" || step_of(ask) == "more" {
        (
            common::seat_of(ask, "src").unwrap_or(owner),
            ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        )
    } else {
        let src = ask
            .data
            .get("by")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let Some(src) = src else { return Ok(Step::Done) };
        if src == owner || !g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        // 「每受到 1 点伤害」触发一次（多点伤害分别触发；对齐 B11 Q3=B）。
        let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        (src, amount)
    };
    if left == 0 || own_cards(g, src).is_empty() {
        return Ok(Step::Finish("反馈：结算完成".into()));
    }
    let data = json!({ "step": "pick", "src": src, "left": left });
    if step_of(ask) != "pick" {
        if own_cards(g, src).is_empty() {
            return Ok(Step::Finish("反馈：结算完成".into()));
        }
        // 先确认是否发动（可选技能必须给出口）。
        return match take_answer(g, "fankui") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("fankui");
                b = b.option(
                    "use",
                    format!("获得 {} 一张牌（剩 {left}）", seat_name(g, src)),
                );
                Ok(Step::Need(b.escape("不发动反馈").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::PickOther, data)),
            Some(o) => Err(format!("反馈：未知选项 {o}")),
        };
    }
    // pick：选一张（装备区可选，手牌随机）。
    let data = json!({ "step": "pick", "src": src, "left": left });
    if let Some(ans) = take_answer(g, "fankui") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(id) = resolve_pick(g, src, &ans) else {
            return Err(format!("反馈：非法选项 {ans}"));
        };
        if !is_owner_in(g.cards.get(&id), src) {
            return Err("反馈：该牌不在目标区域".into());
        }
        flow::gain(g, owner, &id, owner, "反馈");
        let rest = left - 1;
        if rest == 0 || own_cards(g, src).is_empty() {
            return Ok(Step::Finish("反馈：结算完成".into()));
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "more", "src": src, "left": rest }),
        ));
    }
    let Some(st) = pick_one_from(g, owner, src, "fankui", "获得", data.clone()) else {
        return Ok(Step::Finish("反馈：目标无牌可拿".into()));
    };
    Ok(st)
}

// ---------------------------------------------------------------------------
// 界司马懿 · 鬼才（判定牌生效前，用手牌或装备区的牌代替）
// ---------------------------------------------------------------------------

fn guicai(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "guicai") {
        return Ok(Step::Done);
    }
    let Some(judge_card) = event_card(ask) else {
        return Ok(Step::Done);
    };
    let data = json!({ "step": "ask", "judge_seat": common::seat_of(ask, "judge_seat"), "card": judge_card });
    let cands = own_cards(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    return match take_answer(g, "guicai") {
        None => {
            let mut b = PendingBuilder::new(owner, "pick_own").skill("guicai");
            for id in &cands {
                b = b.option(
                    format!("card:{id}"),
                    format!("用【{}】代替判定牌", card_label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不改判").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("鬼才：该牌不在手牌/装备区".into());
            }
            // 手牌/装备牌都先进处理区（装备区离开触发枭姬等唯一钩子）。
            if matches!(g.cards.get(&id), Some(Zone::Equip(..))) {
                flow::leave_equip(g, owner, &id, "鬼才");
            }
            g.cards.insert(id.clone(), Zone::Processing);
            g.judge_rewrite = Some(id.clone());
            Ok(Step::Finish(format!(
                "鬼才：用【{}】代替判定牌",
                card_label(g, &id)
            )))
        }
        Some(o) => Err(format!("鬼才：未知选项 {o}")),
    };
}

/// 鬼才可用的替代牌（手牌 + 装备区）。
pub fn judge_rewrite_cards(g: &Game, seat: usize) -> Vec<CardId> {
    own_cards(g, seat)
}

/// 天妒：判定牌生效后是否可获得。
pub fn judge_gain_available(g: &Game, seat: usize) -> bool {
    judge_gain_owner(g, seat).is_some()
}

/// 判定牌生效后可以获得判定牌的技能（天妒）code：内核按 code 压询问，不写技能名。
pub fn judge_gain_owner(g: &Game, seat: usize) -> Option<&'static str> {
    (skills::active(g, seat, "tiandu") && skills::enabled("tiandu")).then_some("tiandu")
}

fn tiandu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "tiandu") {
        return Ok(Step::Done);
    }
    let Some(card) = event_card(ask) else {
        return Ok(Step::Done);
    };
    if !matches!(g.cards.get(&card), Some(Zone::Processing)) {
        return Ok(Step::Done);
    }
    match take_answer(g, "tiandu") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("tiandu");
            b = b.option("gain", format!("获得判定牌【{}】", card_label(g, &card)));
            Ok(Step::Need(
                b.escape("放弃").data(json!({ "card": card })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "gain" => {
            flow::gain(g, owner, &card, owner, "天妒");
            Ok(Step::Finish("天妒：获得判定牌".into()))
        }
        Some(o) => Err(format!("天妒：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界夏侯惇 · 刚烈（受到伤害后判定：红 → 对来源 1 点伤害；黑 → 弃置其一张牌）
// ---------------------------------------------------------------------------

fn ganglie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "ganglie") {
        return Ok(Step::Done);
    }
    if step_of(ask) == "result" {
        let src = common::seat_of(ask, "src").unwrap_or(owner);
        let Some(r) = g.judge_result.take() else {
            return Ok(Step::Finish("刚烈：无判定结果".into()));
        };
        if r.is_red() {
            let spec = DamageSpec {
                from: Some(owner),
                target: src,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Skill,
            };
            let armor = gear::armor_of(g, src);
            let _ = flow::damage(g, spec, armor, 0);
            return Ok(Step::Finish("刚烈：判定红色，对来源造成 1 点伤害".into()));
        }
        // 黑：弃置其一张牌（装备区可选、手牌随机）。
        if own_cards(g, src).is_empty() {
            return Ok(Step::Finish("刚烈：来源无牌可弃".into()));
        }
        return Ok(Step::Next(
            Stage::PickOther,
            json!({ "step": "discard", "src": src }),
        ));
    }
    if step_of(ask) == "discard" {
        let src = common::seat_of(ask, "src").unwrap_or(owner);
        let data = json!({ "step": "discard", "src": src });
        if let Some(ans) = take_answer(g, "ganglie") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = resolve_pick(g, src, &ans) else {
                return Err(format!("刚烈：非法选项 {ans}"));
            };
            if !is_owner_in(g.cards.get(&id), src) {
                return Err("刚烈：该牌不在目标区域".into());
            }
            flow::discard(g, owner, &id, "刚烈");
            return Ok(Step::Finish("刚烈：判定黑色，弃置来源一张牌".into()));
        }
        let Some(st) = pick_one_from(g, owner, src, "ganglie", "弃置", data) else {
            return Ok(Step::Finish("刚烈：来源无牌可弃".into()));
        };
        return Ok(st);
    }
    let src = ask
        .data
        .get("by")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let Some(src) = src else { return Ok(Step::Done) };
    if src == owner || !g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    match take_answer(g, "ganglie") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("ganglie");
            b = b.option("use", format!("发动刚烈（判定，目标 {}）", seat_name(g, src)));
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "src": src }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            super::super::quick_judge::start(
                g,
                owner,
                "刚烈",
                Some(("ganglie", owner, json!({ "src": src }))),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("刚烈：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界夏侯惇 · 清俭（摸牌阶段外获得牌后：展示并交给任意张；每类型 +1 手牌上限）
// ---------------------------------------------------------------------------

fn qingjian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "qingjian") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    // ③ 逐张给出（可随时结束；结束时按「给出的类型数」给当前回合角色 +手牌上限）。
    if step == "pick" {
        let t = common::seat_of(ask, "target").unwrap_or(owner);
        let kinds = ask.data.get("kinds").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let data = json!({ "step": "pick", "target": t, "kinds": kinds });
        return match take_answer(g, "qingjian") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("qingjian");
                for id in own_cards(g, owner) {
                    b = b.option(
                        format!("card:{id}"),
                        format!("展示并交出【{}】", card_label(g, &id)),
                    );
                }
                Ok(Step::Need(b.escape("结束清俭").data(data).build()))
            }
            Some(a) if a == "pass" => {
                let n = kinds.count_ones() as i32;
                if n > 0 {
                    let cur = g.turn;
                    g.set_counter(&turn_key(cur, "qingjian_limit"), n);
                }
                Ok(Step::Finish(format!("清俭：给出 {n} 种类型，手牌上限 +{n}")))
            }
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !is_owner_in(g.cards.get(&id), owner) {
                    return Err("清俭：该牌不在手牌/装备区".into());
                }
                // 公示（mechanics `reveal`）。
                g.reveal(&[id.clone()], "清俭");
                let bit = match def_of(g, &id).map(|d| d.category) {
                    Some(Category::Basic) => 1u32,
                    Some(Category::Trick) | Some(Category::DelayedTrick) => 2,
                    Some(Category::Equip) => 4,
                    None => 0,
                };
                flow::gain(g, t, &id, owner, "清俭");
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "target": t, "kinds": kinds | bit }),
                ))
            }
            Some(o) => Err(format!("清俭：未知选项 {o}")),
        };
    }
    // ② 选一名其他角色。
    if step == "target" {
        let others = common::alive_others(g, owner);
        return match take_answer(g, "qingjian") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("qingjian");
                for t in &others {
                    b = b.option(format!("t:{t}"), format!("交给 {}", seat_name(g, *t)));
                }
                let data = json!({ "step": "target" });
                Ok(Step::Need(b.escape("不发动清俭").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "清俭：非法目标")?;
                if !others.contains(&t) {
                    return Err("清俭：目标非法".into());
                }
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "pick", "target": t, "kinds": 0 }),
                ))
            }
            Some(o) => Err(format!("清俭：未知选项 {o}")),
        };
    }
    // ① 事件触发：是否发动（每回合一次；无可交的牌不询问）。
    if g.counter(&turn_key(owner, "qingjian_used")) > 0
        || own_cards(g, owner).is_empty()
        || common::alive_others(g, owner).is_empty()
    {
        return Ok(Step::Done);
    }
    if ask.data.get("event").is_some() {
        return Ok(Step::Next(Stage::Choose, json!({ "step": "ask" })));
    }
    match take_answer(g, "qingjian") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("qingjian");
            b = b.option("use", "发动清俭（展示并交出任意张牌）");
            let data = json!({ "step": "ask" });
            Ok(Step::Need(b.escape("不发动清俭").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            g.bump(&turn_key(owner, "qingjian_used"));
            Ok(Step::Next(Stage::Choose, json!({ "step": "target" })))
        }
        Some(o) => Err(format!("清俭：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界张辽 · 突袭（摸牌阶段：少摸任意张 + 向等量角色各拿一张手牌）
// ---------------------------------------------------------------------------

fn tuxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "tuxi") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "take" {
        let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if left == 0 {
            return Ok(Step::Finish("突袭：结算完成".into()));
        }
        let cands: Vec<usize> = common::alive_others(g, owner)
            .into_iter()
            .filter(|i| !g.hand(*i).is_empty())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Finish("突袭：无可拿牌的目标".into()));
        }
        let data = json!({ "step": "take", "left": left });
        return match take_answer(g, "tuxi") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("tuxi");
                for t in &cands {
                    b = b.option(
                        format!("t:{t}"),
                        format!("获得 {} 一张手牌（剩 {left}）", seat_name(g, *t)),
                    );
                }
                Ok(Step::Need(b.escape("结束结算").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Finish("突袭：结算完成".into())),
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "突袭：非法目标")?;
                if !cands.contains(&t) {
                    return Err("突袭：目标非法".into());
                }
                if let Some(id) = g.hand(t).first().cloned() {
                    flow::gain(g, owner, &id, owner, "突袭");
                }
                if left > 1 {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "take", "left": left - 1 }),
                    ));
                }
                Ok(Step::Finish("突袭：结算完成".into()))
            }
            Some(o) => Err(format!("突袭：未知选项 {o}")),
        };
    }
    // 首次进入（摸牌阶段开始）：N = 正常摸牌数（含其他加成）。
    let n = 2 + skills::draw_bonus(g, owner);
    let cands: Vec<usize> = common::alive_others(g, owner)
        .into_iter()
        .filter(|i| !g.hand(*i).is_empty())
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "tuxi") {
        None => {
            let mut b = PendingBuilder::new(owner, "choice").skill("tuxi");
            for x in 0..=n {
                b = b.option(
                    format!("draw:{x}"),
                    format!("摸 {x} 张，并向 {} 名角色各拿一张手牌", n - x),
                );
            }
            let data = json!({ "step": "ask" });
            Ok(Step::Need(b.escape("不发动突袭").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("draw:") => {
            let x: usize = a[5..].parse().map_err(|_| "突袭：非法摸牌数")?;
            if x > n {
                return Err("突袭：摸牌数超过上限".into());
            }
            if x > 0 {
                flow::draw(g, owner, x);
            }
            // 本阶段摸牌由突袭替代（不再走自动摸牌）。
            g.set_counter(&turn_key(owner, "skip_draw"), 1);
            if n > x {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "take", "left": n - x }),
                ));
            }
            Ok(Step::Finish("突袭：只正常摸牌".into()))
        }
        Some(o) => Err(format!("突袭：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界许褚 · 裸衣（亮出牌堆顶 3 张：可选择跳过摸牌；伤害 +1 到下回合开始）
// ---------------------------------------------------------------------------

fn luoyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "luoyi") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "choose" {
        let ids = common::cards_of(ask, "cards");
        let data = json!({ "step": "choose", "cards": ids });
        return match take_answer(g, "luoyi") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("luoyi");
                b = b.option("skip", "跳过摸牌阶段，获得其中的基本牌/武器/【决斗】");
                Ok(Step::Need(b.escape("放弃（正常摸牌）").data(data).build()))
            }
            Some(a) if a == "pass" => {
                for id in &ids {
                    if matches!(g.cards.get(id), Some(Zone::Processing)) {
                        super::super::card_play::to_discard(g, id);
                    }
                }
                Ok(Step::Finish("裸衣：放弃，展示牌进弃牌堆并正常摸牌".into()))
            }
            Some(a) if a == "skip" => {
                let mut kept = vec![];
                for id in &ids {
                    if !matches!(g.cards.get(id), Some(Zone::Processing)) {
                        continue;
                    }
                    let d = def_of(g, id);
                    let keep = d
                        .map(|d| {
                            d.category == Category::Basic
                                || d.tags.duel
                                || d.slot == Some(super::super::Slot::Weapon)
                        })
                        .unwrap_or(false);
                    if keep {
                        flow::gain(g, owner, id, owner, "裸衣");
                        kept.push(card_label(g, id));
                    } else {
                        super::super::card_play::to_discard(g, id);
                    }
                }
                g.set_counter(&turn_key(owner, "skip_draw"), 1);
                g.set_counter(&format!("luoyi.{owner}"), 1);
                Ok(Step::Finish(format!(
                    "裸衣：跳过摸牌，获得【{}】；【杀】/【决斗】伤害 +1",
                    kept.join("、")
                )))
            }
            Some(o) => Err(format!("裸衣：未知选项 {o}")),
        };
    }
    match take_answer(g, "luoyi") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("luoyi");
            b = b.option("use", "亮出牌堆顶三张");
            let data = json!({ "step": "ask" });
            Ok(Step::Need(b.escape("不发动裸衣").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            if g.deck.is_empty() {
                flow::reshuffle(g);
            }
            let mut ids = vec![];
            for _ in 0..3 {
                let Some(id) = g.deck.pop() else { break };
                g.cards.insert(id.clone(), Zone::Processing);
                ids.push(id);
            }
            if ids.is_empty() {
                return Ok(Step::Done);
            }
            g.reveal(&ids, "裸衣");
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "choose", "cards": ids }),
            ))
        }
        Some(o) => Err(format!("裸衣：未知选项 {o}")),
    }
}

/// 裸衣：为伤害来源的【杀】/【决斗】伤害 +1（直到自己下回合开始，`turn::begin_turn` 清理）。
pub fn damage_bonus(g: &Game, spec: &DamageSpec) -> i32 {
    let Some(from) = spec.from else { return 0 };
    if g.counter(&format!("luoyi.{from}")) <= 0 {
        return 0;
    }
    if !skills::active(g, from, "luoyi") {
        return 0;
    }
    match spec.source {
        SourceKind::Slash | SourceKind::Duel => 1,
        _ => 0,
    }
}

/// 摸牌阶段替代（突袭/裸衣）：本阶段跳过自动摸牌。
pub fn skip_draw(g: &Game, seat: usize) -> bool {
    g.counter(&turn_key(seat, "skip_draw")) > 0
}

/// 手牌上限加成：清俭（每给出一种类型 +1）、洛神（本回合获得的牌不计入上限）。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    let qingjian = g.counter(&turn_key(seat, "qingjian_limit"));
    let luoshen = g.counter(&turn_key(seat, "luoshen_free"));
    qingjian + luoshen
}

/// 倾国：黑色牌（手牌/装备区）当【闪】。
pub fn response_as(g: &Game, seat: usize, id: &str, kind: ResponseKind) -> CardAs {
    if kind != ResponseKind::Shan || !skills::active(g, seat, "qingguo") {
        return CardAs::NoOpinion;
    }
    let black = g
        .card_defs
        .get(id)
        .map(|c| !c.suit.is_red())
        .unwrap_or(false);
    if !black {
        return CardAs::NoOpinion;
    }
    if def_of(g, id).map(|d| d.tags.shan).unwrap_or(false) {
        return CardAs::NoOpinion;
    }
    CardAs::As("shan")
}

// ---------------------------------------------------------------------------
// 界郭嘉 · 遗计（每受到 1 点伤害：摸 2，交给至多两名其他角色、合计至多两张）
// ---------------------------------------------------------------------------

fn yiji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "yiji") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "give" {
        let ids = common::cards_of(ask, "cards");
        let given = common::seats_of(ask, "given");
        let Some(card) = ids.first().cloned() else {
            return Ok(Step::Finish("遗计：分配完成".into()));
        };
        let rest: Vec<CardId> = ids.iter().skip(1).cloned().collect();
        let data = json!({ "step": "give", "cards": ids, "given": given });
        if !is_owner_in(g.cards.get(&card), owner) {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "give", "cards": rest, "given": given }),
            ));
        }
        let mut cands = common::alive_others(g, owner);
        // 至多两名：已收过牌的角色最多 2 张；已用两名时只允许这两人。
        cands.retain(|t| given.iter().filter(|x| *x == t).count() < 2);
        if given.len() >= 2 && !cands.is_empty() {
            cands.retain(|t| given.contains(t));
        }
        return match take_answer(g, "yiji") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("yiji");
                for t in &cands {
                    b = b.option(
                        format!("t:{t}"),
                        format!("把【{}】交给 {}", card_label(g, &card), seat_name(g, *t)),
                    );
                }
                Ok(Step::Need(b.escape("自己保留").data(data).build()))
            }
            Some(a) if a == "pass" => {
                if rest.is_empty() {
                    Ok(Step::Finish("遗计：分配完成".into()))
                } else {
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "give", "cards": rest, "given": given }),
                    ))
                }
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "遗计：非法目标")?;
                if !cands.contains(&t) {
                    return Err("遗计：目标非法".into());
                }
                flow::gain(g, t, &card, owner, "遗计");
                let mut gv = given;
                gv.push(t);
                if rest.is_empty() {
                    return Ok(Step::Finish("遗计：分配完成".into()));
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "give", "cards": rest, "given": gv }),
                ))
            }
            Some(o) => Err(format!("遗计：未知选项 {o}")),
        };
    }
    match take_answer(g, "yiji") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("yiji");
            b = b.option("use", "发动遗计（摸两张并分配）");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let ids = flow::draw(g, owner, 2);
            if ids.is_empty() {
                return Ok(Step::Finish("遗计：无牌可摸".into()));
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "give", "cards": ids, "given": [] }),
            ))
        }
        Some(o) => Err(format!("遗计：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界李典 · 恂恂（摸牌阶段：看牌堆顶 4 张，选 2 张置顶（顺序可调）、其余置底）
// ---------------------------------------------------------------------------

fn xunxun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "xunxun") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "pick" {
        let ids = common::cards_of(ask, "cards");
        let top = common::cards_of(ask, "top");
        let bottom = common::cards_of(ask, "bottom");
        // 全部选完：两张置顶（顺序可调）→ 剩下置底。
        if top.len() >= 2 || top.len() + bottom.len() >= ids.len() {
            let order: Vec<CardId> = top.into_iter().take(2).collect();
            let mut top_set: Vec<CardId> = order.clone();
            let rest: Vec<CardId> = ids
                .iter()
                .filter(|c| !top_set.iter().any(|t| t == *c))
                .cloned()
                .collect();
            // 置底（直接压牌堆底）；置顶：先选的先摸到（最后 push 的在顶）。
            for id in rest.iter() {
                flow::to_deck_bottom(g, id);
            }
            top_set.reverse();
            for id in top_set.iter() {
                flow::to_deck_top(g, id);
            }
            return Ok(Step::Finish("恂恂：两张置顶、其余置底".into()));
        }
        let Some(card) = ids
            .iter()
            .find(|c| !top.contains(c) && !bottom.contains(c))
            .cloned()
        else {
            return Ok(Step::Finish("恂恂：结算完成".into()));
        };
        let data = json!({ "step": "pick", "cards": ids, "top": top, "bottom": bottom });
        return match take_answer(g, "xunxun") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("xunxun");
                b = b.option(format!("top:{card}"), format!("【{}】置于牌堆顶", card_label(g, &card)));
                b = b.option(format!("bottom:{card}"), format!("【{}】置于牌堆底", card_label(g, &card)));
                Ok(Step::Need(b.escape("不发动恂恂").data(data).build()))
            }
            Some(a) if a == "pass" => {
                for id in ids.iter() {
                    if matches!(g.cards.get(id), Some(Zone::Processing)) {
                        flow::to_deck_bottom(g, id);
                    }
                }
                Ok(Step::Finish("恂恂：全部置底".into()))
            }
            Some(a) if a.starts_with("top:") => {
                let mut nt = top;
                nt.push(card);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "pick", "cards": ids, "top": nt, "bottom": bottom }),
                ))
            }
            Some(a) if a.starts_with("bottom:") => {
                let mut nb = bottom;
                nb.push(card);
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "pick", "cards": ids, "top": top, "bottom": nb }),
                ))
            }
            Some(o) => Err(format!("恂恂：未知选项 {o}")),
        };
    }
    // 首次进入（摸牌阶段开始）：看牌堆顶 4 张（不足 2 张不发动）。
    if g.deck.is_empty() {
        flow::reshuffle(g);
    }
    let mut ids = vec![];
    for _ in 0..4 {
        let Some(id) = g.deck.pop() else { break };
        g.cards.insert(id.clone(), Zone::Processing);
        ids.push(id);
    }
    if ids.len() < 2 {
        for id in ids.iter() {
            flow::to_deck_top(g, id);
        }
        return Ok(Step::Done);
    }
    Ok(Step::Next(
        Stage::Choose,
        json!({ "step": "pick", "cards": ids, "top": [], "bottom": [] }),
    ))
}

// ---------------------------------------------------------------------------
// 界李典 · 忘隙（造成/受到 1 点伤害后：摸 2 并交给该角色其中一张）
// ---------------------------------------------------------------------------

fn wangxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "wangxi") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "pick" {
        let other = common::seat_of(ask, "other").unwrap_or(owner);
        let data = json!({ "step": "pick", "other": other });
        return match take_answer(g, "wangxi") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("wangxi");
                for id in g.hand(owner) {
                    b = b.option(format!("card:{id}"), format!("交给【{}】", card_label(g, &id)));
                }
                Ok(Step::Need(b.escape("不交牌").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Finish("忘隙：不交牌".into())),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !matches!(g.cards.get(&id), Some(Zone::Hand(s)) if *s == owner) {
                    return Err("忘隙：该牌不在手牌".into());
                }
                flow::gain(g, other, &id, owner, "忘隙");
                Ok(Step::Finish("忘隙：交给其一张".into()))
            }
            Some(o) => Err(format!("忘隙：未知选项 {o}")),
        };
    }
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let by = ask
        .data
        .get("by")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let other = if victim == Some(owner) {
        match by {
            Some(s) if s != owner => s,
            _ => return Ok(Step::Done),
        }
    } else if by == Some(owner) {
        match victim {
            Some(v) if v != owner => v,
            _ => return Ok(Step::Done),
        }
    } else {
        return Ok(Step::Done);
    };
    match take_answer(g, "wangxi") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("wangxi");
            b = b.option("use", "摸两张并交给其一张");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "other": other }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            flow::draw(g, owner, 2);
            if !g.seats.get(other).map(|s| s.alive).unwrap_or(false) {
                return Ok(Step::Finish("忘隙：对方已离场，两张都归自己".into()));
            }
            Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "other": other }),
            ))
        }
        Some(o) => Err(format!("忘隙：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界甄姬 · 洛神（准备阶段判定：黑色获得并可重复；无次数上限）
// ---------------------------------------------------------------------------

fn luoshen(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "luoshen") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "result" {
        let Some(r) = g.judge_result.take() else {
            return Ok(Step::Finish("洛神：无判定结果".into()));
        };
        if r.is_red() {
            return Ok(Step::Finish("洛神：判定为红色，结束洛神".into()));
        }
        if matches!(g.cards.get(&r.card), Some(Zone::Processing) | Some(Zone::Discard)) {
            flow::gain(g, owner, &r.card, owner, "洛神");
            g.bump(&turn_key(owner, "luoshen_free"));
        }
        return Ok(Step::Next(Stage::Choose, json!({ "step": "ask" })));
    }
    if owner == g.turn && g.phase != Phase::Prepare {
        // 只在准备阶段进行（事件路由可能晚于阶段；数据以 step 为准，这里只做安全出口）。
    }
    match take_answer(g, "luoshen") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("luoshen");
            b = b.option("use", "判定（黑色则获得并继续）");
            Ok(Step::Need(
                b.escape("结束洛神")
                    .data(json!({ "step": "ask" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("洛神：结束".into())),
        Some(a) if a == "use" => {
            super::super::quick_judge::start(
                g,
                owner,
                "洛神",
                Some(("luoshen", owner, json!({ "step": "result" }))),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("洛神：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 导出/视图辅助
// ---------------------------------------------------------------------------

/// 视图用：洛神本回合的「不计入上限」张数（供 UI 显示）。
pub fn luoshen_free(g: &Game, seat: usize) -> i32 {
    g.counter(&turn_key(seat, "luoshen_free"))
}

