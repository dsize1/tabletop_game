//! B17 技能（界限突破·J5：8 将 / 14 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B17.json` 的 `agreed`（权威）。
//! 本文件按批次推进；`SKILLS` 只声明**已实现并收口**的技能（`check:sgs3-registry` 报 ⏳）。

use serde_json::json;

use super::super::cards;
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{Game, Phase, Zone};
use super::common::{card_label, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "xuanfeng", name: "旋风", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardLost, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "zhichi", name: "智迟", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "jiangchi", name: "将驰", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "zongshi_jie", name: "纵适", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "qiaoshuo", name: "巧说", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "zongxuan", name: "纵玄", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardDiscarded, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "zhiyan", name: "直言", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "xuanfeng" => xuanfeng,
        "zhichi" => zhichi,
        "jiangchi" => jiangchi,
        "zongshi_jie" => zongshi_jie,
        "qiaoshuo" => qiaoshuo,
        "zongxuan" => zongxuan,
        "zhiyan" => zhiyan,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "qiaoshuo" => Some(can_qiaoshuo),
        _ => None,
    }
}

fn has(g: &Game, seat: usize, code: &str) -> bool {
    skills::active(g, seat, code) && skills::enabled(code)
}

fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut v: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// 旋风（界凌统）：失去装备区牌 / 单次失去 ≥2 张 → 依次弃至多两名其他角色共 2 张
// ---------------------------------------------------------------------------

fn xuanfeng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "xuanfeng") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if ask.data.get("event").and_then(|v| v.as_str()) != Some("CardLost") {
            return Ok(Step::Done);
        }
        let lost = skills::lost_batch(g, owner).len();
        let equip_lost = g
            .equip_lost_batch
            .iter()
            .any(|(o, _)| *o == owner);
        if lost < 2 && !equip_lost {
            return Ok(Step::Done);
        }
        // 同一动作只触发一次（多次 CardLost 事件）。
        let key = format!("xuanfeng.action.{}", g.action_seq);
        if !skills::action_seen(g, key) {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "left": 2 }),
        ));
    }
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = (0..g.seats.len())
        .filter(|t| *t != owner && g.seats[*t].alive && !own_cards(g, *t).is_empty())
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "xuanfeng") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("旋风：未知选项".into());
        };
        if !cands.contains(&t) {
            return Err("旋风：目标不合法".into());
        }
        // 弃置哪张由引擎随机：先随机手牌，无手牌再取装备区第一张。
        let hand = g.hand(t);
        let id = if !hand.is_empty() {
            let idx = g.rand_below(hand.len());
            hand.get(idx).cloned()
        } else {
            super::super::gear::equips_of(g, t).first().cloned()
        };
        if let Some(id) = id {
            flow::discard(g, owner, &id, "旋风");
        }
        let left = left - 1;
        if left == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "left": left }),
        ));
    }
    let mut b = PendingBuilder::new(owner, "xuanfeng_pick").skill("xuanfeng");
    for t in &cands {
        b = b.option(
            format!("target:{t}"),
            format!("旋风：弃置 {} 一张牌", seat_name(g, *t)),
        );
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "pick", "left": left }),
        b.escape("停止弃置").build(),
    ))
}

// ---------------------------------------------------------------------------
// 智迟（界陈宫）：回合外受伤后，本回合【杀】/普通锦囊不能指定你
// ---------------------------------------------------------------------------

fn zhichi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhichi") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    if g.turn != owner {
        g.set_counter(&turn_key(owner, "zhichi"), 1);
    }
    Ok(Step::Done)
}

/// 智迟：目标枚举裁剪（杀 / 普通锦囊；延时锦囊与装备不拦）。
pub fn deny_target_instance(
    g: &Game,
    _user: usize,
    target: usize,
    _card: Option<&str>,
    def: &cards::CardDef,
) -> bool {
    if g.counter(&turn_key(target, "zhichi")) <= 0 || !has(g, target, "zhichi") {
        return false;
    }
    def.tags.sha
        || (def.category == cards::Category::Trick && !def.tags.delayed)
}

// ---------------------------------------------------------------------------
// 将驰（界曹彰）：摸牌阶段结束二选一
// ---------------------------------------------------------------------------

pub fn after_draw_phase(g: &mut Game, seat: usize) {
    if !has(g, seat, "jiangchi") {
        return;
    }
    if g.counter(&turn_key(seat, "jiangchi_offered")) > 0 {
        return;
    }
    g.bump(&turn_key(seat, "jiangchi_offered"));
    super::super::resolve::push_ask(
        g,
        "jiangchi",
        Stage::Choose,
        seat,
        json!({ "stage": "choose" }),
    );
}

fn jiangchi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiangchi") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "choose" {
        if let Some(ans) = take_answer(g, "jiangchi") {
            match ans.as_str() {
                "pass" => return Ok(Step::Done),
                "draw" => {
                    flow::draw(g, owner, 1);
                    g.set_counter(&turn_key(owner, "jiangchi_penalty"), 1);
                    g.set_counter(&turn_key(owner, "jiangchi_no_sha_limit"), 1);
                    return Ok(Step::Done);
                }
                "recast" => {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "recast" }),
                    ));
                }
                _ => return Err("将驰：未知选项".into()),
            }
        }
        let mut b = PendingBuilder::new(owner, "jiangchi_choose").skill("jiangchi");
        b = b.option("draw", "将驰：摸一张，本回合【杀】次数 -1 且【杀】不计入手牌上限");
        b = b.option("recast", "将驰：重铸一张牌，本回合【杀】次数 +1 且【杀】无距离限制");
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "choose" }),
            b.escape("不发动将驰").build(),
        ));
    }
    // recast：选择手牌/装备区一张牌重铸。
    let cards = own_cards(g, owner);
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "jiangchi") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("将驰：未知选项".into());
        };
        if !cards.iter().any(|c| c == id) {
            return Err("将驰：该牌不在手牌/装备区".into());
        }
        g.set_counter(&turn_key(owner, "jiangchi_recast_ok"), 1);
        super::super::card_play::recast_any(g, owner, id)?;
        g.set_counter(&turn_key(owner, "jiangchi_recast_ok"), 0);
        g.set_counter(&turn_key(owner, "jiangchi_bonus"), 1);
        g.set_counter(&turn_key(owner, "jiangchi_no_distance"), 1);
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "jiangchi_recast").skill("jiangchi");
    for id in &cards {
        b = b.option(format!("pick:{id}"), format!("将驰：重铸【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "recast" }),
        b.escape("不重铸（不发动）").build(),
    ))
}

/// 将驰：【杀】次数 ±1。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    if !has(g, seat, "jiangchi") {
        return 0;
    }
    g.counter(&turn_key(seat, "jiangchi_bonus"))
        - g.counter(&turn_key(seat, "jiangchi_penalty"))
}

/// 将驰：重铸选项（一次）。
pub fn recast_allowed(g: &Game, seat: usize, _id: &str) -> bool {
    has(g, seat, "jiangchi") && g.counter(&turn_key(seat, "jiangchi_recast_ok")) > 0
}

/// 将驰：【杀】无距离。
pub fn ignore_distance(g: &Game, seat: usize) -> bool {
    has(g, seat, "jiangchi") && g.counter(&turn_key(seat, "jiangchi_no_distance")) > 0
}

/// 将驰：①【杀】不计入手牌上限。
pub fn hand_size_for_discard(g: &Game, seat: usize) -> usize {
    let total = g.hand_size(seat);
    if !has(g, seat, "jiangchi") || g.counter(&turn_key(seat, "jiangchi_no_sha_limit")) <= 0 {
        return total;
    }
    let sha = g
        .hand(seat)
        .iter()
        .filter(|id| {
            g.card_defs
                .get(*id)
                .and_then(|c| cards::def(&c.def))
                .map(|d| d.tags.sha)
                .unwrap_or(false)
        })
        .count();
    total.saturating_sub(sha)
}

// ---------------------------------------------------------------------------
// 纵适（界简雍）：拼点赢获得对方拼点牌；没赢获得自己的拼点牌
// ---------------------------------------------------------------------------

/// 拼点后窗口（`pindian` 唯一调用点）。
pub fn pindian_after(g: &mut Game, r: &super::super::pindian::PindianResult) {
    for owner in [r.initiator, r.target] {
        if !has(g, owner, "zongshi_jie") {
            continue;
        }
        let role = if owner == r.initiator { "initiator" } else { "target" };
        super::super::resolve::push_ask(
            g,
            "zongshi_jie",
            Stage::Choose,
            owner,
            json!({
                "a": r.a,
                "b": r.b,
                "winner": r.winner,
                "owner": owner,
                "role": role,
            }),
        );
    }
}

fn zongshi_jie(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zongshi_jie") {
        return Ok(Step::Done);
    }
    let a = ask.data.get("a").and_then(|v| v.as_str()).unwrap_or("");
    let b = ask.data.get("b").and_then(|v| v.as_str()).unwrap_or("");
    let winner = ask.data.get("winner").and_then(|v| v.as_u64()).map(|v| v as usize);
    let role = ask.data.get("role").and_then(|v| v.as_str()).unwrap_or("initiator");
    let (mine, other) = if role == "initiator" { (a, b) } else { (b, a) };
    let target = if winner == Some(owner) { other } else { mine };
    if target.is_empty() || !matches!(g.cards.get(target), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zongshi_jie") {
        if ans == "gain" {
            flow::gain(g, owner, target, owner, "纵适");
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "zongshi_jie_gain").skill("zongshi_jie");
    b = b.option("gain", format!("纵适：获得拼点牌【{}】", card_label(g, target)));
    Ok(Step::NeedWith(
        Stage::Choose,
        ask.data.clone(),
        b.escape("不获得").build(),
    ))
}

// ---------------------------------------------------------------------------
// 巧说（界简雍）：拼点；赢 → 本回合下一张牌 ±1 目标；没赢 → 本回合不能使用锦囊
// ---------------------------------------------------------------------------

fn can_qiaoshuo(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "qiaoshuo") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.hand(seat).is_empty() {
        return false;
    }
    (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive && !g.hand(t).is_empty())
}

fn qiaoshuo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qiaoshuo") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if !can_qiaoshuo(g, owner) {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "qiaoshuo") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("巧说：未知选项".into());
                };
                if t == owner || !g.seats.get(t).map(|s| s.alive).unwrap_or(false) || g.hand(t).is_empty() {
                    return Err("巧说：目标不合法".into());
                }
                super::super::pindian::start(g, owner, t, "巧说");
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": t }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "qiaoshuo_target").skill("qiaoshuo");
            for t in 0..g.seats.len() {
                if t != owner && g.seats[t].alive && !g.hand(t).is_empty() {
                    b = b.option(format!("target:{t}"), format!("巧说：与 {} 拼点", seat_name(g, t)));
                }
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动巧说").build(),
            ));
        }
        "wait" => {
            let Some(r) = g.take_pindian() else {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": ask.data.get("target") }),
                ));
            };
            if r.initiator_won() {
                g.bump(&turn_key(owner, "qiao"));
            } else {
                g.set_counter(&turn_key(owner, "qiao"), 0);
                g.set_counter(&turn_key(owner, "qiao_lost"), 1);
            }
            Ok(Step::Done)
        }
        _ => Ok(Step::Done),
    }
}

/// 巧说：使用手牌时消耗 1 枚「巧」，本张牌多/少指定一个目标。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !has(g, actor, "qiaoshuo") {
        return;
    }
    if g.counter(&turn_key(actor, "qiao")) <= 0 {
        return;
    }
    if !matches!(g.cards.get(card), Some(Zone::Hand(s)) if *s == actor) {
        return;
    }
    let Some(d) = g
        .card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
    else {
        return;
    };
    if d.tags.delayed || matches!(d.target_mode, cards::TargetMode::None | cards::TargetMode::SelfOnly) {
        return;
    }
    let mut opts = g.use_opts_mut(card);
    if d.target_mode == cards::TargetMode::AllAlive {
        opts.trim_targets = 1;
    } else {
        opts.extra_targets = 1;
    }
    g.set_counter(&turn_key(actor, "qiao"), g.counter(&turn_key(actor, "qiao")) - 1);
}

/// 巧说没赢：本回合不能使用锦囊牌。
pub fn trick_use_blocked(g: &Game, seat: usize, _card: &str) -> bool {
    has(g, seat, "qiaoshuo") && g.counter(&turn_key(seat, "qiao_lost")) > 0
}

// ---------------------------------------------------------------------------
// 纵玄（界虞翻）：自己弃置 / 上家每回合首次弃置后，将其中任意张置于牌堆顶
// ---------------------------------------------------------------------------

fn prev_alive(g: &Game, seat: usize) -> Option<usize> {
    let n = g.seats.len();
    (1..=n)
        .map(|k| (seat + n - k) % n)
        .find(|&s| s != seat && g.seats.get(s).map(|x| x.alive).unwrap_or(false))
}

fn zongxuan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zongxuan") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("CardDiscarded") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    let discarded_by = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(owner);
    let prev = prev_alive(g, owner);
    if discarded_by != owner && Some(discarded_by) != prev {
        return Ok(Step::Done);
    }
    if stage.is_empty() {
        if discarded_by != owner {
            if g.counter(&turn_key(owner, "zongxuan_prev")) > 0 {
                return Ok(Step::Done);
            }
            g.set_counter(&turn_key(owner, "zongxuan_prev"), 1);
        }
        let key = format!("zongxuan.action.{}", g.action_seq);
        if !skills::action_seen(g, key) {
            return Ok(Step::Done);
        }
        let cards: Vec<String> = skills::discard_batch(g, discarded_by)
            .iter()
            .filter(|id| matches!(g.cards.get(*id), Some(Zone::Discard)))
            .cloned()
            .collect();
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "discarder": discarded_by, "cards": cards, "event": "CardDiscarded" }),
        ));
    }
    let discarder = ask.data.get("discarder").and_then(|v| v.as_u64()).unwrap_or(owner as u64) as usize;
    let cards: Vec<String> = ask
        .data
        .get("cards")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zongxuan") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("top:") else {
            return Err("纵玄：未知选项".into());
        };
        if !cards.iter().any(|c| c == id) {
            return Err("纵玄：该牌不在本批弃置牌中".into());
        }
        flow::to_deck_top(g, id);
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "discarder": discarder, "cards": cards, "event": "CardDiscarded" }),
        ));
    }
    let mut b = PendingBuilder::new(owner, "zongxuan_pick").skill("zongxuan");
    for id in &cards {
        b = b.option(format!("top:{id}"), format!("纵玄：将【{}】置于牌堆顶", card_label(g, id)));
    }
    let mut data = json!({ "stage": "pick", "discarder": discarder, "cards": cards });
    data["event"] = json!("CardDiscarded");
    Ok(Step::NeedWith(
        Stage::Choose,
        data,
        b.escape("结束置顶").build(),
    ))
}

// ---------------------------------------------------------------------------
// 直言（界虞翻）：自己/上家的结束阶段，令一名角色摸 1 展示；
// 装备 → 使用并回复 1；非装备且体力不等于虞翻 → 失去 1 点体力。
// ---------------------------------------------------------------------------

fn zhiyan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhiyan") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("PhaseStart") {
        return Ok(Step::Done);
    }
    if g.phase != Phase::End {
        return Ok(Step::Done);
    }
    let prev = prev_alive(g, owner);
    let source = if g.turn == owner {
        "self"
    } else if Some(g.turn) == prev {
        "prev"
    } else {
        return Ok(Step::Done);
    };
    let key = turn_key(owner, &format!("zhiyan_{source}"));
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if g.counter(&key) > 0 {
            return Ok(Step::Done);
        }
        g.set_counter(&key, 1);
        let mut b = PendingBuilder::new(owner, "zhiyan_target").skill("zhiyan");
        for t in 0..g.seats.len() {
            if g.seats[t].alive {
                b = b.option(format!("target:{t}"), format!("直言：令 {} 摸一张并展示", seat_name(g, t)));
            }
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "target", "event": "PhaseStart" }),
            b.escape("不发动直言").build(),
        ));
    }
    let Some(ans) = take_answer(g, "zhiyan") else {
        return Ok(Step::Done);
    };
    if ans == "pass" {
        return Ok(Step::Done);
    }
    let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
        return Err("直言：未知选项".into());
    };
    if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
        return Err("直言：目标不合法".into());
    }
    let drawn = flow::draw(g, t, 1);
    let Some(card) = drawn.first().cloned() else {
        return Ok(Step::Done);
    };
    g.reveal(&[card.clone()], "直言");
    let equip = g
        .card_defs
        .get(&card)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.slot.is_some())
        .unwrap_or(false);
    if equip {
        let _ = super::super::gear::place_from_hand(g, t, &card, t);
        flow::heal(g, t, 1, "直言");
    } else {
        let my_hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0);
        if g.seats.get(t).map(|s| s.hp).unwrap_or(0) != my_hp {
            flow::lose_hp(g, t, 1, "直言");
        }
    }
    g.conceal(&[card], "直言结算完成");
    Ok(Step::Done)
}

