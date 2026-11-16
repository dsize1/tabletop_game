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
    SkillDef { code: "jinjiu", name: "禁酒", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "enyuan", name: "恩怨", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::CardGained, subject: Subject::Self_ },
            Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "lihuo", name: "疠火", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "xianzhen", name: "陷阵", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "chunlao", name: "醇醪", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::CardDiscarded, subject: Subject::Any },
            Subscription { event: EventKind::PhaseStart, subject: Subject::Any },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "mingce", name: "明策", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[
            Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true },
            Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false },
        ] },
    SkillDef { code: "xuanhuo", name: "眩惑", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[], hooks: &[], can_active: false }] },
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
        "enyuan" => enyuan,
        "lihuo" => lihuo,
        "xianzhen" => xianzhen,
        "chunlao" => chunlao,
        "mingce" => mingce,
        "xuanhuo" => xuanhuo,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "qiaoshuo" => Some(can_qiaoshuo),
        "xianzhen" => Some(can_xianzhen),
        "mingce" => Some(can_mingce),
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
    xuanhuo_after_draw(g, seat);
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
    let no_sha_limit = g.counter(&turn_key(seat, "jiangchi_no_sha_limit")) > 0
        || g.counter(&turn_key(seat, "xianzhen_no_sha_limit")) > 0;
    if !no_sha_limit {
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
    // 疠火：火杀可多指定一个目标；转化牌标记「结算后代价」。
    if has(g, actor, "lihuo") {
        if let Some(d) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) {
            if d.tags.sha && d.damage_kind == super::super::DamageKind::Fire {
                g.use_opts_mut(card).extra_targets += 1;
            }
        }
        if let Some(origins) = g.virtual_origins.get(card).cloned() {
            if let Some(origin) = origins.first() {
                let was_non_fire_sha = g
                    .card_defs
                    .get(origin)
                    .and_then(|c| cards::def(&c.def))
                    .map(|x| x.tags.sha && x.damage_kind != super::super::DamageKind::Fire)
                    .unwrap_or(false);
                if was_non_fire_sha {
                    g.bump(&turn_key(actor, &format!("lihuo_card.{card}")));
                }
            }
        }
    }
    // 陷阵（赢）：对拼点目标使用【杀】/普通锦囊可多指定其为目标。
    if has(g, actor, "xianzhen")
        && (0..g.seats.len()).any(|t| g.counter(&turn_key(actor, &format!("xianzhen_t.{t}"))) > 0)
    {
        if let Some(d) = g.card_defs.get(card).and_then(|c| cards::def(&c.def)) {
            if d.tags.sha || (d.category == cards::Category::Trick && !d.tags.delayed) {
                g.use_opts_mut(card).extra_targets += 1;
            }
        }
    }
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


// ---------------------------------------------------------------------------
// 禁酒（界高顺）：【酒】视为点数 K 的【杀】；高顺回合内其他角色不能使用【酒】
// ---------------------------------------------------------------------------

pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    let own = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == seat)
        || matches!(g.cards.get(id), Some(Zone::Equip(s, _)) if *s == seat);
    if !own {
        return None;
    }
    let d = g.card_defs.get(id).and_then(|c| cards::def(&c.def))?;
    // 禁酒：【酒】视为【杀】。
    if has(g, seat, "jinjiu") && d.tags.jiu {
        return Some("sha");
    }
    // 疠火：非火【杀】改为火【杀】。
    if has(g, seat, "lihuo") && d.tags.sha && d.damage_kind != super::super::DamageKind::Fire {
        return Some("huo_sha");
    }
    None
}

pub fn jiu_blocked(g: &Game, user: usize) -> bool {
    if has(g, user, "jinjiu") {
        return true;
    }
    (0..g.seats.len()).any(|i| {
        i != user && g.turn == i && g.seats[i].alive && has(g, i, "jinjiu")
    })
}

pub fn rank_for_use(g: &Game, seat: usize, card: &str, rank: u8) -> u8 {
    if !has(g, seat, "jinjiu") {
        return rank;
    }
    let is_jiu = g
        .card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.tags.jiu)
        .unwrap_or(false);
    if is_jiu {
        13
    } else {
        rank
    }
}

// ---------------------------------------------------------------------------
// 恩怨（界法正）：单次获得他人 ≥2 张 → 其摸 1；受伤后来源交红手牌或失 1 体力
// ---------------------------------------------------------------------------

fn enyuan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "enyuan") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if event == "CardGained" {
            let mut froms: Vec<usize> = vec![];
            for (from, _) in skills::gain_batch(g, owner) {
                if from != owner && !froms.contains(&from) {
                    let n = skills::gain_batch(g, owner)
                        .into_iter()
                        .filter(|(f, _)| *f == from)
                        .count();
                    if n >= 2 {
                        froms.push(from);
                    }
                }
            }
            let Some(from) = froms.first().copied() else {
                return Ok(Step::Done);
            };
            if !skills::action_seen(g, format!("enyuan.gain.{from}")) {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "gain", "from": from, "event": "CardGained" }),
            ));
        }
        if event == "DamageAfter" {
            let Some(from) = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize) else {
                return Ok(Step::Done);
            };
            if from == owner || !g.seats.get(from).map(|s| s.alive).unwrap_or(false) {
                return Ok(Step::Done);
            }
            if !skills::action_seen(g, format!("enyuan.dmg.{from}")) {
                return Ok(Step::Done);
            }
            let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "ask", "source": from, "left": amount, "event": "DamageAfter" }),
            ));
        }
        return Ok(Step::Done);
    }
    if stage == "gain" {
        let from = ask.data.get("from").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if let Some(ans) = take_answer(g, "enyuan") {
            if ans == "gain" {
                flow::draw(g, from, 1);
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "enyuan_gain").skill("enyuan")
            .option("gain", format!("恩怨：令 {} 摸一张牌", seat_name(g, from)));
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "gain", "from": from, "event": "CardGained" }),
            b.escape("不发动恩怨").build(),
        ));
    }
    // ask：来源选择交红色手牌 / 失去 1 点体力（每点一次）。
    let src = ask.data.get("source").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let reds: Vec<String> = g
        .hand(src)
        .into_iter()
        .filter(|id| g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false))
        .collect();
    if let Some(ans) = take_answer(g, "enyuan") {
        if ans == "hp" {
            flow::lose_hp(g, src, 1, "恩怨");
        } else if let Some(id) = ans.strip_prefix("red:") {
            if !reds.iter().any(|c| c == id) {
                return Err("恩怨：该牌不是来源的红色手牌".into());
            }
            flow::gain(g, owner, id, src, "恩怨");
        } else {
            return Err("恩怨：未知选项".into());
        }
        let left = left - 1;
        if left == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "source": src, "left": left, "event": "DamageAfter" }),
        ));
    }
    if reds.is_empty() {
        flow::lose_hp(g, src, 1, "恩怨");
        let left = left - 1;
        if left == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "source": src, "left": left, "event": "DamageAfter" }),
        ));
    }
    let mut b = PendingBuilder::new(src, "enyuan_ask").skill("enyuan");
    b = b.option("hp", "恩怨：失去 1 点体力");
    for id in &reds {
        b = b.option(format!("red:{id}"), format!("恩怨：交给法正红色手牌【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "ask", "source": src, "left": left, "event": "DamageAfter" }),
        b.escape_mandatory("（必须选择一项）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 疠火（界程普）：非火杀改火杀；火杀可多指定一个目标；造成伤害后付代价
// ---------------------------------------------------------------------------



/// 疠火：此牌结算后若造成伤害 → 弃一张牌或失去 1 点体力。
pub fn on_card_finished(g: &mut Game, card: &str, user: usize) {
    if !has(g, user, "lihuo") {
        return;
    }
    let key = turn_key(user, &format!("lihuo_card.{card}"));
    if g.counter(&key) <= 0 {
        return;
    }
    g.set_counter(&key, 0);
    if g.counter(&turn_key(user, &format!("damage_card.{card}"))) <= 0 {
        return;
    }
    super::super::resolve::push_ask(
        g,
        "lihuo",
        Stage::Choose,
        user,
        json!({ "stage": "cost" }),
    );
}

fn lihuo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "lihuo") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage == "cost" {
        let cards = own_cards(g, owner);
        if let Some(ans) = take_answer(g, "lihuo") {
            match ans.as_str() {
                "hp" => {
                    flow::lose_hp(g, owner, 1, "疠火");
                }
                other if other.starts_with("pick:") => {
                    let id = &other["pick:".len()..];
                    if !cards.iter().any(|c| c == id) {
                        return Err("疠火：该牌不在手牌/装备区".into());
                    }
                    flow::discard(g, owner, id, "疠火");
                }
                _ => return Err("疠火：未知选项".into()),
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "lihuo_cost").skill("lihuo");
        b = b.option("hp", "疠火：失去 1 点体力");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("疠火：弃置【{}】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "cost" }),
            b.escape_mandatory("（必须支付代价）").build(),
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 陷阵（界高顺）：拼点；赢 → 对目标无距离/杀不限/无视防具/可多指定；
// 没赢 → 不能对其用杀且杀不计手牌上限。
// ---------------------------------------------------------------------------

fn can_xianzhen(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "xianzhen") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    g.counter(&turn_key(seat, "xianzhen_used")) == 0
        && !g.hand(seat).is_empty()
        && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive && !g.hand(t).is_empty())
}

fn xianzhen(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "xianzhen") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if !can_xianzhen(g, owner) {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|t| *t != owner && g.seats[*t].alive && !g.hand(*t).is_empty())
                .collect();
            if let Some(ans) = take_answer(g, "xianzhen") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("陷阵：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("陷阵：目标不合法".into());
                }
                g.bump(&turn_key(owner, "xianzhen_used"));
                super::super::pindian::start(g, owner, t, "陷阵");
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": t }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "xianzhen_target").skill("xianzhen");
            for t in &cands {
                b = b.option(format!("target:{t}"), format!("陷阵：与 {} 拼点", seat_name(g, *t)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动陷阵").build(),
            ));
        }
        "wait" => {
            let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let Some(r) = g.take_pindian() else {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": t }),
                ));
            };
            if r.initiator_won() {
                g.set_counter(&turn_key(owner, &format!("xianzhen_t.{t}")), 1);
            } else {
                g.set_counter(&turn_key(owner, &format!("xianzhen_block.{t}")), 1);
                g.set_counter(&turn_key(owner, "xianzhen_no_sha_limit"), 1);
            }
            Ok(Step::Done)
        }
        _ => Ok(Step::Done),
    }
}

pub fn target_no_distance(g: &Game, user: usize, target: usize) -> bool {
    has(g, user, "xianzhen") && g.counter(&turn_key(user, &format!("xianzhen_t.{target}"))) > 0
}

pub fn sha_limit_exempt(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "xianzhen") {
        return false;
    }
    (0..g.seats.len()).any(|t| g.counter(&turn_key(seat, &format!("xianzhen_t.{t}"))) > 0)
}

pub fn sha_blocked_for(g: &Game, user: usize, target: usize) -> bool {
    has(g, user, "xianzhen") && g.counter(&turn_key(user, &format!("xianzhen_block.{target}"))) > 0
}

pub fn ignore_armor_for(g: &Game, user: usize, target: usize) -> bool {
    has(g, user, "xianzhen") && g.counter(&turn_key(user, &format!("xianzhen_t.{target}"))) > 0
}


// ---------------------------------------------------------------------------
// 醇醪（界程普）：相邻/自己弃置【杀】→「醇」（≤9）；结束阶段补「醇」；
// 濒死时移去 X 张当【酒】（X=本轮以此法使用次数）
// ---------------------------------------------------------------------------

fn adjacent(g: &Game, a: usize, b: usize) -> bool {
    if a == b {
        return true;
    }
    let n = g.seats.len();
    if n <= 1 {
        return false;
    }
    let left = (a + n - 1) % n;
    let right = (a + 1) % n;
    b == left || b == right
}

fn chunlao_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut v: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, n) if *s == seat && n == "chunlao"))
        .map(|(id, _)| id.clone())
        .collect();
    v.sort();
    v
}

fn chunlao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "chunlao") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if event == "CardDiscarded" {
            let from = ask
                .data
                .get("victim")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(owner);
            if !adjacent(g, owner, from) {
                return Ok(Step::Done);
            }
            let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) else {
                return Ok(Step::Done);
            };
            if !matches!(g.cards.get(card), Some(Zone::Discard)) {
                return Ok(Step::Done);
            }
            let is_sha = g
                .card_defs
                .get(card)
                .and_then(|c| cards::def(&c.def))
                .map(|d| d.tags.sha)
                .unwrap_or(false);
            if !is_sha || chunlao_cards(g, owner).len() >= 9 {
                return Ok(Step::Done);
            }
            g.cards.insert(card.to_string(), Zone::Mark(owner, "chunlao".to_string()));
            let mut ev = super::super::event::Event::card(
                EventKind::CardIntoGame,
                Some(owner),
                card.to_string(),
                Some("醇醪".to_string()),
            );
            ev.by = Some(owner);
            let ev = ev.snapshot(g);
            g.push_event(ev);
            return Ok(Step::Done);
        }
        if event == "PhaseStart" && g.phase == Phase::End && g.turn == owner {
            if !chunlao_cards(g, owner).is_empty() {
                return Ok(Step::Done);
            }
            let sha: Vec<String> = g
                .hand(owner)
                .into_iter()
                .filter(|id| {
                    g.card_defs
                        .get(id)
                        .and_then(|c| cards::def(&c.def))
                        .map(|d| d.tags.sha)
                        .unwrap_or(false)
                })
                .collect();
            if sha.is_empty() {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "store", "event": "PhaseStart" }),
            ));
        }
        return Ok(Step::Done);
    }
    if stage == "store" {
        let sha: Vec<String> = g
            .hand(owner)
            .into_iter()
            .filter(|id| {
                g.card_defs
                    .get(id)
                    .and_then(|c| cards::def(&c.def))
                    .map(|d| d.tags.sha)
                    .unwrap_or(false)
            })
            .collect();
        if sha.is_empty() || chunlao_cards(g, owner).len() >= 9 {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "chunlao") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("store:") else {
                return Err("醇醪：未知选项".into());
            };
            if !sha.iter().any(|c| c == id) {
                return Err("醇醪：该牌不是手牌【杀】".into());
            }
            g.cards.insert(id.to_string(), Zone::Mark(owner, "chunlao".to_string()));
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "store", "event": "PhaseStart" }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "chunlao_store").skill("chunlao");
        for id in &sha {
            b = b.option(format!("store:{id}"), format!("醇醪：将【{}】置于武将牌上", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "store", "event": "PhaseStart" }),
            b.escape("结束放置").build(),
        ));
    }
    // 濒死：移去 X 张「醇」视为其使用一张【酒】。
    if stage == "dying" {
        let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(owner as u64) as usize;
        let x = (g.counter("round.chunlao") + 1).max(1) as usize;
        let cards = chunlao_cards(g, owner);
        if cards.len() < x {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "chunlao") {
            let Some(id) = ans.strip_prefix("use:") else {
                return Err("醇醪：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("醇醪：该牌不是「醇」".into());
            }
            super::super::card_play::to_discard_with(g, id, "醇醪", Some(owner));
            let used = ask.data.get("used").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(x as u64) as usize;
            if left > 1 {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "dying", "target": target, "left": left - 1, "used": used + 1 }),
                ));
            }
            flow::rescue(g, target, 1, "醇醪");
            g.bump("round.chunlao");
            g.set_mark(&format!("dying.chunlao.{owner}"), "ok");
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "chunlao_dying").skill("chunlao");
        for id in &cards {
            b = b.option(format!("use:{id}"), format!("醇醪：移去【{}】当【酒】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "dying", "target": target, "left": x, "used": 0 }),
            b.escape("不发动醇醪").build(),
        ));
    }
    Ok(Step::Done)
}

/// 濒死技能选项（`step_dying` 聚合）。
pub fn dying_options(g: &Game, asker: usize, _target: usize) -> Vec<(&'static str, String, &'static str)> {
    if !has(g, asker, "chunlao") {
        return vec![];
    }
    let x = (g.counter("round.chunlao") + 1).max(1) as usize;
    if chunlao_cards(g, asker).len() < x {
        return vec![];
    }
    vec![("chunlao", format!("醇醪：移去 {x} 张「醇」视为使用【酒】"), "dying")]
}


// ---------------------------------------------------------------------------
// 明策（界陈宫）：出牌阶段限一次，交给一名其他角色一张【杀】/装备，
// 其选择：①视为对你指定的另一名角色使用【杀】，若造成伤害你与其各摸1；②你与其各摸1。
// ---------------------------------------------------------------------------

fn can_mingce(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "mingce") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.counter(&turn_key(seat, "mingce_used")) > 0 {
        return false;
    }
    let has = own_cards(g, seat).into_iter().any(|id| {
        g.card_defs
            .get(&id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.tags.sha || d.slot.is_some())
            .unwrap_or(false)
    });
    has && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive)
}

fn mingce(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "mingce") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if !can_mingce(g, owner) {
                return Ok(Step::Done);
            }
            let cards: Vec<String> = own_cards(g, owner)
                .into_iter()
                .filter(|id| {
                    g.card_defs
                        .get(id)
                        .and_then(|c| cards::def(&c.def))
                        .map(|d| d.tags.sha || d.slot.is_some())
                        .unwrap_or(false)
                })
                .collect();
            if let Some(ans) = take_answer(g, "mingce") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let Some(id) = ans.strip_prefix("pick:") else {
                    return Err("明策：未知选项".into());
                };
                if !cards.iter().any(|c| c == id) {
                    return Err("明策：该牌不是【杀】/装备".into());
                }
                g.bump(&turn_key(owner, "mingce_used"));
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "target", "card": id }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "mingce_pick").skill("mingce");
            for id in &cards {
                b = b.option(format!("pick:{id}"), format!("明策：交给【{}】", card_label(g, id)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动明策").build(),
            ));
        }
        "target" => {
            let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|t| *t != owner && g.seats[*t].alive)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "mingce") {
                let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("明策：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("明策：目标不合法".into());
                }
                flow::gain(g, t, &card, owner, "明策");
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "assist", "helper": t }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "mingce_target").skill("mingce");
            for t in &cands {
                b = b.option(format!("target:{t}"), format!("明策：交给 {}", seat_name(g, *t)));
            }
            let mut data = json!({ "stage": "target", "card": card });
            data["event"] = json!("");
            return Ok(Step::NeedWith(
                Stage::Choose,
                data,
                b.escape("不选择（明策不结算）").build(),
            ));
        }
        "assist" => {
            let helper = ask.data.get("helper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if let Some(ans) = take_answer(g, "mingce") {
                match ans.as_str() {
                    "opt2" => {
                        flow::draw(g, owner, 1);
                        flow::draw(g, helper, 1);
                        return Ok(Step::Done);
                    }
                    "opt1" => {
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "damage_target", "helper": helper }),
                        ));
                    }
                    _ => return Err("明策：未知选项".into()),
                }
            }
            let reach = super::super::gear::attack_range(g, helper) as usize;
            let has_target = g
                .alive_seats()
                .into_iter()
                .any(|x| x != helper && super::super::gear::within_attack_range(g, helper, x, reach));
            let mut b = PendingBuilder::new(helper, "mingce_assist").skill("mingce");
            if has_target {
                b = b.option("opt1", "明策：视为对陈宫指定的角色使用【杀】");
            }
            b = b.option("opt2", "明策：你与陈宫各摸一张牌");
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "assist", "helper": helper }),
                b.escape("不选择（明策不结算）").build(),
            ));
        }
        "damage_target" => {
            let helper = ask.data.get("helper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let reach = super::super::gear::attack_range(g, helper) as usize;
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|x| *x != helper && super::super::gear::within_attack_range(g, helper, *x, reach))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "mingce") {
                let Some(t) = ans.strip_prefix("damage:") .and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("明策：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("明策：目标不在协助者攻击范围".into());
                }
                let vid = super::super::card_play::use_virtual(g, helper, "sha", &[t], false)
                    .map_err(|e| format!("明策：{e}"))?;
                g.set_mark(&turn_key(owner, "mingce_card"), vid);
                g.set_mark(&turn_key(owner, "mingce_helper"), helper.to_string());
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(owner, "mingce_damage").skill("mingce");
            for t in &cands {
                b = b.option(format!("damage:{t}"), format!("明策：令 {} 对 {} 使用【杀】", seat_name(g, helper), seat_name(g, *t)));
            }
            Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "damage_target", "helper": helper }),
                b.escape("不选择（明策不结算）").build(),
            ))
        }
        _ => Ok(Step::Done),
    }
}

/// 明策：此虚拟【杀】造成伤害后，陈宫与协助者各摸一张。
pub fn on_damage_after(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "mingce") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) else {
        return Ok(Step::Done);
    };
    let Some(expected) = g.mark(&turn_key(owner, "mingce_card")).map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if card != expected {
        return Ok(Step::Done);
    }
    let helper = g
        .mark(&turn_key(owner, "mingce_helper"))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(owner);
    g.marks.remove(&turn_key(owner, "mingce_card"));
    g.marks.remove(&turn_key(owner, "mingce_helper"));
    flow::draw(g, owner, 1);
    flow::draw(g, helper, 1);
    Ok(Step::Done)
}


// ---------------------------------------------------------------------------
// 眩惑（界法正）：摸牌阶段结束，交给一名其他角色两张手牌；
// 其选择：①视为对你指定的另一名角色使用【杀】；②你观看其手牌并获得其中两张。
// ---------------------------------------------------------------------------

pub fn xuanhuo_after_draw(g: &mut Game, seat: usize) {
    if !has(g, seat, "xuanhuo") {
        return;
    }
    if g.counter(&turn_key(seat, "xuanhuo_offered")) > 0 {
        return;
    }
    if g.hand_size(seat) < 2 {
        return;
    }
    if !(0..g.seats.len()).any(|t| t != seat && g.seats[t].alive) {
        return;
    }
    g.bump(&turn_key(seat, "xuanhuo_offered"));
    super::super::resolve::push_ask(
        g,
        "xuanhuo",
        Stage::Choose,
        seat,
        json!({ "stage": "" }),
    );
}

fn xuanhuo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "xuanhuo") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if g.hand_size(owner) < 2 {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "xuanhuo") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "cards", "sel": [] }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "xuanhuo_ask").skill("xuanhuo")
                .option("use", "眩惑：交给一名其他角色两张手牌");
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动眩惑").build(),
            ));
        }
        "cards" => {
            let mut sel: Vec<String> = ask
                .data
                .get("sel")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            if let Some(ans) = take_answer(g, "xuanhuo") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let Some(id) = ans.strip_prefix("pick:") else {
                    return Err("眩惑：未知选项".into());
                };
                if !g.hand(owner).iter().any(|c| c == id) || sel.iter().any(|c| c == id) {
                    return Err("眩惑：该牌不是可选手牌".into());
                }
                sel.push(id.to_string());
            }
            if sel.len() == 2 {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "target", "cards": sel }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "xuanhuo_cards").skill("xuanhuo");
            for id in g.hand(owner) {
                if sel.iter().any(|c| *c == id) {
                    continue;
                }
                b = b.option(format!("pick:{id}"), format!("眩惑：选择【{}】", card_label(g, &id)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "cards", "sel": sel }),
                b.escape("取消眩惑").build(),
            ));
        }
        "target" => {
            let cards: Vec<String> = ask
                .data
                .get("cards")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|t| *t != owner && g.seats[*t].alive)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "xuanhuo") {
                let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("眩惑：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("眩惑：目标不合法".into());
                }
                for id in &cards {
                    flow::gain(g, t, id, owner, "眩惑");
                }
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "choice", "helper": t }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "xuanhuo_target").skill("xuanhuo");
            for t in &cands {
                b = b.option(format!("target:{t}"), format!("眩惑：交给 {}", seat_name(g, *t)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "target", "cards": cards }),
                b.escape("不选择（眩惑不结算）").build(),
            ));
        }
        "choice" => {
            let helper = ask.data.get("helper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if let Some(ans) = take_answer(g, "xuanhuo") {
                match ans.as_str() {
                    "opt1" => {
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "damage_target", "helper": helper }),
                        ));
                    }
                    "opt2" => {
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "watch", "helper": helper, "left": 2 }),
                        ));
                    }
                    _ => return Err("眩惑：未知选项".into()),
                }
            }
            let reach = super::super::gear::attack_range(g, helper) as usize;
            let has_target = g
                .alive_seats()
                .into_iter()
                .any(|x| x != helper && super::super::gear::within_attack_range(g, helper, x, reach));
            let mut b = PendingBuilder::new(helper, "xuanhuo_choice").skill("xuanhuo");
            if has_target {
                b = b.option("opt1", "眩惑：视为对法正指定的角色使用【杀】");
            }
            b = b.option("opt2", "眩惑：令法正观看你的手牌并获得其中两张");
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "choice", "helper": helper }),
                b.escape("不选择（眩惑不结算）").build(),
            ));
        }
        "damage_target" => {
            let helper = ask.data.get("helper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let reach = super::super::gear::attack_range(g, helper) as usize;
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|x| *x != helper && super::super::gear::within_attack_range(g, helper, *x, reach))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "xuanhuo") {
                let Some(t) = ans.strip_prefix("damage:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("眩惑：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("眩惑：目标不在协助者攻击范围".into());
                }
                super::super::card_play::use_virtual(g, helper, "sha", &[t], false)
                    .map_err(|e| format!("眩惑：{e}"))?;
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(owner, "xuanhuo_damage").skill("xuanhuo");
            for t in &cands {
                b = b.option(format!("damage:{t}"), format!("眩惑：令 {} 对 {} 使用【杀】", seat_name(g, helper), seat_name(g, *t)));
            }
            Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "damage_target", "helper": helper }),
                b.escape("不选择（眩惑不结算）").build(),
            ))
        }
        "watch" => {
            let helper = ask.data.get("helper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let left = ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if left == 0 || g.hand(helper).is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "xuanhuo") {
                let Some(id) = ans.strip_prefix("take:") else {
                    return Err("眩惑：未知选项".into());
                };
                if !g.hand(helper).iter().any(|c| c == id) {
                    return Err("眩惑：该牌不在其手牌".into());
                }
                flow::gain(g, owner, id, helper, "眩惑");
                let left = left - 1;
                if left == 0 {
                    return Ok(Step::Done);
                }
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "watch", "helper": helper, "left": left }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "xuanhuo_watch").skill("xuanhuo");
            for id in g.hand(helper) {
                b = b.option(format!("take:{id}"), format!("眩惑：获得【{}】", card_label(g, &id)));
            }
            Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "watch", "helper": helper, "left": left }),
                b.escape("停止获得").build(),
            ))
        }
        _ => Ok(Step::Done),
    }
}

