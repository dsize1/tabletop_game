//! B15（界限突破·J3）——按 `align/B15.json` 第 7 段实现。
//!
//! 覆盖：界太史慈（天义/酣战）、界董卓（肉林/崩坏/酒池/暴虐）、界邓艾（屯田/凿险）、
//! 界祝融（巨象/烈刃/长标）、界姜维（挑衅/志继）、界贾诩（帷幕/乱武/完杀）、
//! 界鲁肃（好施/缔盟）、界张郃（巧变）。
//!
//! 用户裁定（2026-11-09，63 条疑点）要点：
//! - 天义：视为使用的【杀】同样享受次数/无距离/多目标加成；没赢只禁「使用」不限「打出」。
//! - 酣战：拼点牌亮出前询问是否发动（v1 未询问即强制 = 缺陷）；点数最大【杀】由酣战者选；只认实体【杀】。
//! - 肉林：与【无双】相加（3 张【闪】）；连续响应；虚拟【闪】可计入。
//! - 崩坏：并列最小也触发；上限最低 1；失去体力走 `flow::lose_hp`。
//! - 酒池：仅手牌；连喝多杯伤害叠加；濒死可当【酒】自救；酒【杀】伤害后本回合崩坏失效。
//! - 暴虐：按点数逐点触发；询问是否判定；获得判定牌走 `gain`。
//! - 屯田：失去牌 = 手牌/装备区（不含判定区）；回合内只看弃置【杀】（含弃牌阶段）。
//! - 凿险：觉醒后本回合结束执行额外回合（额外回合正常清「本回合」加成）。
//! - 巨象：不能成为【南蛮入侵】目标；其余角色使用后结算结束即获得（被无懈也获得）；座位序最先者获得。
//! - 烈刃：保留独立 code `liereng`（与 `liewen` 共用实现，钩子同时识别）；询问是否拼点；获得范围＝随机手牌/装备区（不含判定区）。
//! - 长标：仅手牌；虚拟【杀】无花色无点数；摸牌数 = 使用的牌数 X；被【闪】抵消不摸。
//! - 挑衅：伤害必须来自那张【杀】；两种失败分支统一由姜维选（随机手牌/装备区，不含判定区）；首次成功后不能再发动。
//! - 志继：准备/结束两处检查；必须二选一；满体力只显示摸 2。
//! - 帷幕：只防自己回合内的伤害；不防失去体力；黑色按实体牌花色。
//! - 乱武：距离最小有并列时由该角色选；最终【杀】询问是否发动；不计次数、可被响应；失去体力可濒死。
//! - 完杀：只有濒死角色本人可用【桃】；濒死中其他角色非锁定技失效。
//! - 好施：询问回礼；到下回合开始清除；多次好施覆盖回礼者；一半 = 当前手牌 ÷2 向下取整。
//! - 缔盟：出牌阶段结束时弃 X；不足弃光；预算按手牌数；交换触发获得/失去事件。
//! - 巧变：阶段开始时逐一询问；支付含装备区；摸牌获得随机手牌；移动落点同曹仁·解围。

use serde_json::json;

use crate::cards::{CardKind, Effect};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{self, Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, builder, grant_skill, other_alive, SkillDef};

const SLOTS: [Slot; 5] = [
    Slot::Weapon,
    Slot::Armor,
    Slot::HorseOffense,
    Slot::HorseDefense,
    Slot::Treasure,
];

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    resolve::push_ask(g, code, Stage::Choose, actor, data);
}
fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}
fn is_lord(g: &Game, seat: usize) -> bool {
    g.seats.get(seat).is_some_and(|s| s.flags.contains("lord"))
}
fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}
fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}
fn is_sha(g: &Game, id: &CardId) -> bool {
    g.cards.contains_key(id) && g.def_of(id).tags.sha
}
fn has_cards(g: &Game, seat: usize) -> bool {
    !g.hand(seat).is_empty() || !equip_ids(g, seat).is_empty()
}
fn tian_count(g: &Game, seat: usize) -> usize {
    g.mark_cards(seat, "tian").len()
}
fn bian(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get("bian").copied())
        .unwrap_or(0)
}
fn add_bian(g: &mut Game, seat: usize, n: u32) {
    *g.seats[seat].counters.entry("bian".to_string()).or_insert(0) += n;
}
fn random_hand(g: &mut Game, seat: usize) -> Option<CardId> {
    let n = g.hand(seat).len();
    if n == 0 {
        return None;
    }
    let idx = g.rand_below(n);
    Some(g.hand(seat)[idx].clone())
}

// ---------------------------------------------------------------------------
// 取牌/弃牌候选（烈刃/挑衅）：随机手牌 + 装备区（不含判定区）
// ---------------------------------------------------------------------------

fn take_one_options(g: &Game, target: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for slot in SLOTS {
        if let Some(id) = g.equip_in(target, slot) {
            out.push((
                format!("pick:equip:{id}"),
                format!("{}区：{}", slot.label(), label(g, &id)),
            ));
        }
    }
    if !g.hand(target).is_empty() {
        out.push((
            "pick:hand".to_string(),
            format!("手牌：随机 1 张（共 {} 张）", g.hand(target).len()),
        ));
    }
    out
}

/// 解析「从目标取一张牌」的选项（不改归属）：调用方决定去向（获得/弃置）。
fn take_one(g: &mut Game, target: usize, act: &str) -> Result<CardId, String> {
    if act == "pick:hand" {
        return random_hand(g, target).ok_or_else(|| "目标没有手牌".to_string());
    }
    if let Some(id) = act.strip_prefix("pick:equip:") {
        let id = id.to_string();
        for slot in SLOTS {
            if g.equip_in(target, slot).as_deref() == Some(id.as_str()) {
                return Ok(id);
            }
        }
        return Err("目标装备区没有该牌".into());
    }
    Err(format!("未知取牌选项 {act}"))
}

// ---------------------------------------------------------------------------
// 天义（界太史慈）
// ---------------------------------------------------------------------------

pub const TIANYI: SkillDef = SkillDef {
    code: "tianyi",
    name: "天义",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_tianyi),
    handler: tianyi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_tianyi(g: &Game, seat: usize) -> bool {
    play_phase(g, seat)
        && g.counter("phase.cancel_tianyi") == 0
        && g.counter("turn.tianyi_used") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat)
            .into_iter()
            .any(|t| !g.hand(t).is_empty())
}

fn tianyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("after") {
        if let Some(res) = g.pindian_result.take() {
            if res.winner == Some(owner) {
                g.bump("turn.tianyi");
                g.push_log(format!("天义：{} 拼点赢，本回合【杀】次数 +1、无距离、可多指定一个目标", g.seats[owner].name));
            } else {
                g.bump("turn.tianyi_lose");
                g.push_log(format!("天义：{} 拼点没赢，本回合不能使用【杀】", g.seats[owner].name));
            }
        }
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = other_alive(g, owner)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty())
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("tianyi");
            for t in &cands {
                b = b.option(format!("target:{t}"), format!("与 {} 拼点", g.seats[*t].name));
            }
            Ok(Step::Need(b.escape("不发动天义").build()))
        }
        Some("pass") => {
            g.bump("phase.cancel_tianyi");
            Ok(Step::Done)
        }
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("天义：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("天义：目标必须是手牌非空的其他角色".into());
            }
            g.bump("turn.tianyi_used");
            g.push_log(format!("天义：{} 与 {} 拼点", g.seats[owner].name, g.seats[t].name));
            push(g, "tianyi", owner, json!({ "stage": "after" }));
            crate::kernel::pindian::start(g, owner, t, "tianyi");
            Ok(Step::Done)
        }
    }
}

/// 天义：赢 → 【杀】次数 +1。
pub fn sha_limit_bonus(g: &Game, seat: usize) -> usize {
    if active(g, seat, "tianyi") && g.counter("turn.tianyi") > 0 {
        1
    } else {
        0
    }
}

/// 天义：没赢 → 本回合【杀】次数归零（不能使用）。
pub fn sha_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if active(g, seat, "tianyi") && g.counter("turn.tianyi_lose") > 0 {
        Some(0)
    } else {
        None
    }
}

/// 天义：赢 → 【杀】可多指定一个目标。
pub fn extra_targets(g: &Game, seat: usize, name: &str) -> usize {
    if active(g, seat, "tianyi")
        && g.counter("turn.tianyi") > 0
        && name == "杀"
        && g.turn == seat
        && g.phase == Phase::Play
    {
        1
    } else {
        0
    }
}

/// 天义：赢 → 【杀】无距离限制。
pub fn sha_no_distance(g: &Game, seat: usize, _card: &CardId) -> bool {
    active(g, seat, "tianyi") && g.counter("turn.tianyi") > 0
}

// ---------------------------------------------------------------------------
// 酣战（界太史慈）：拼点后获得点数最大的【杀】；拼点前令对方随机出牌
// ---------------------------------------------------------------------------

pub const HANZHAN: SkillDef = SkillDef {
    code: "hanzhan",
    name: "酣战",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: hanzhan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn hanzhan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() != Some("after") {
        return Ok(Step::Done);
    }
    let list: Vec<CardId> = a.data["cards"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let best: Vec<CardId> = {
        let mut best = list
            .iter()
            .filter(|id| is_sha(g, id) && matches!(g.zone_of(id), Some(Zone::Discard)))
            .cloned()
            .collect::<Vec<_>>();
        let max = best
            .iter()
            .filter_map(|id| g.cards.get(id).map(|c| c.rank))
            .max()
            .unwrap_or(0);
        best.retain(|id| g.cards.get(id).map(|c| c.rank).unwrap_or(0) == max);
        best
    };
    if best.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("hanzhan");
            for id in &best {
                b = b.option(
                    format!("pick:{id}"),
                    format!("获得点数最大的【{}】（{}）", label(g, id), label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不获得").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("酣战：未知选项 {act}"))?
                .to_string();
            if !best.contains(&id) || !matches!(g.zone_of(&id), Some(Zone::Discard)) {
                return Err("酣战：该牌不在弃牌堆".into());
            }
            let text = format!("酣战：{} 获得拼点牌中点数最大的【{}】", g.seats[owner].name, label(g, &id));
            g.gain(&id, owner)?;
            g.push_log(text);
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 肉林（界董卓）：与女性角色的【杀】需两张【闪】（与无双相加）
// ---------------------------------------------------------------------------

pub const ROULIN: SkillDef = SkillDef {
    code: "roulin",
    name: "肉林",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 肉林：本次【杀】额外需要几张【闪】（与无双相加，用户裁定 Q1=B）。
pub fn response_need_bonus(
    g: &Game,
    responder: usize,
    opponent: usize,
    kind: crate::cards::ResponseKind,
    effect: Effect,
) -> usize {
    if kind != crate::cards::ResponseKind::Shan || effect != Effect::Sha {
        return 0;
    }
    let responder_female = g.seats.get(responder).is_some_and(|s| !s.male);
    let opponent_female = g.seats.get(opponent).is_some_and(|s| !s.male);
    if (active(g, opponent, "roulin") && responder_female)
        || (active(g, responder, "roulin") && opponent_female)
    {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 崩坏（界董卓）
// ---------------------------------------------------------------------------

pub const BENGKUAI: SkillDef = SkillDef {
    code: "bengkuai",
    name: "崩坏",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: bengkuai,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn bengkuai(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::End || g.counter("turn.bengkuai_off") > 0 {
        return Ok(Step::Done);
    }
    if g.counter("turn.bengkuai_done") > 0 && g.answer.is_none() {
        return Ok(Step::Done);
    }
    // 并列最小也触发（用户裁定 Q1=B）：存在其他角色体力 ≤ 你。
    let trigger = other_alive(g, owner)
        .into_iter()
        .any(|i| g.seats[i].hp <= g.seats[owner].hp);
    if !trigger {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            g.bump("turn.bengkuai_done");
            let mut b = builder(owner, "choice").skill("bengkuai");
            b = b.option("opt1", "崩坏：失去 1 点体力");
            b = b.option("opt2", "崩坏：减少 1 点体力上限");
            Ok(Step::Need(b.escape("（不会出现）不结算").build()))
        }
        Some("opt1") => {
            g.push_log(format!("崩坏：{} 失去 1 点体力", g.seats[owner].name));
            flow::lose_hp(g, owner, 1, "bengkuai");
            Ok(Step::Done)
        }
        Some("opt2") => {
            let (hp, max) = (g.seats[owner].hp, g.seats[owner].max_hp);
            g.push_log(format!(
                "崩坏：{} 减少 1 点体力上限（{hp}/{max}）",
                g.seats[owner].name
            ));
            flow::lose_max_hp(g, owner, 1, "bengkuai");
            Ok(Step::Done)
        }
        Some(_) => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 酒池（界董卓）：黑桃手牌当【酒】；不限次数（可叠加）；酒【杀】伤害后崩坏失效
// ---------------------------------------------------------------------------

pub const JIUCHI: SkillDef = SkillDef {
    code: "jiuchi",
    name: "酒池",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiuchi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "jiuchi") {
        return Vec::new();
    }
    g.hand(seat)
        .into_iter()
        .filter(|id| g.cards.get(id).is_some_and(|c| c.suit == crate::kernel::Suit::Spade))
        .map(|id| super::Conversion {
            skill: "jiuchi",
            from: id,
            to: "酒",
            usable: true,
            playable: false,
        })
        .collect()
}

/// 界酒池：使用【酒】无次数限制。
pub fn jiu_unlimited(g: &Game, seat: usize) -> bool {
    active(g, seat, "jiuchi")
}

/// 酒池转化的【酒】可用于濒死自救（返回黑桃手牌）。
pub fn dying_self_jiu(g: &Game, seat: usize) -> Vec<CardId> {
    if !active(g, seat, "jiuchi") {
        return Vec::new();
    }
    g.hand(seat)
        .into_iter()
        .filter(|id| g.cards.get(id).is_some_and(|c| c.suit == crate::kernel::Suit::Spade))
        .collect()
}

fn jiuchi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
        return Ok(Step::Done);
    }
    if !active(g, owner, "jiuchi") {
        return Ok(Step::Done);
    }
    let Some(src) = a.data["cause"]
        .as_str()
        .and_then(|s| s.parse::<usize>().ok())
    else {
        return Ok(Step::Done);
    };
    if src != owner {
        return Ok(Step::Done);
    }
    let Some(card) = a.data["card"].as_str().map(str::to_string) else {
        return Ok(Step::Done);
    };
    if !is_sha(g, &card) {
        return Ok(Step::Done);
    }
    let victim = a.data["seat"].as_u64().unwrap_or(0) as usize;
    if g.counter(&format!("turn.jiu_boost:{victim}")) == 0 {
        return Ok(Step::Done);
    }
    if g.counter("turn.bengkuai_off") == 0 {
        g.counters.insert("turn.bengkuai_off".to_string(), 1);
        g.push_log(format!(
            "酒池：{} 的酒【杀】造成伤害，本回合「崩坏」失效",
            g.seats[owner].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 暴虐（界董卓，主公技）
// ---------------------------------------------------------------------------

pub const BAONUE: SkillDef = SkillDef {
    code: "baonue",
    name: "暴虐",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 10,
    can_active: None,
    handler: baonue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn baonue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("after") {
        let Some(card) = g.judge_result.take() else {
            return Ok(Step::Done);
        };
        let spade = g.cards.get(&card).is_some_and(|c| c.suit == crate::kernel::Suit::Spade);
        if spade {
            flow::heal(g, owner, 1, "baonue");
            if g.cards.contains_key(&card) {
                g.gain(&card, owner)?;
            }
            g.push_log(format!(
                "暴虐：判定【{}】黑桃，{} 回复 1 点体力并获得判定牌",
                label(g, &card),
                g.seats[owner].name
            ));
        } else {
            g.push_log(format!("暴虐：判定【{}】非黑桃，未生效", label(g, &card)));
        }
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        if left > 0 && g.seats[owner].hp > 0 {
            push(g, "baonue", owner, json!({ "stage": "ask", "left": left }));
        }
        return Ok(Step::Done);
    }
    if a.data["stage"].as_str().is_none() {
        // 触发入口：其他群势力角色造成伤害后（按点数逐点询问）。
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if !active(g, owner, "baonue") || !is_lord(g, owner) {
            return Ok(Step::Done);
        }
        let Some(src) = a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok()) else {
            return Ok(Step::Done);
        };
        if src == owner || g.seats.get(src).map(|s| s.kingdom.as_str()) != Some("qun") {
            return Ok(Step::Done);
        }
        let amount = a.data["amount"].as_i64().unwrap_or(0);
        if amount <= 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "left": amount }),
        ));
    }
    let left = a.data["left"].as_u64().unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("baonue");
            b = b.option("opt1", "暴虐：判定（黑桃则回复 1 点并获得判定牌）");
            Ok(Step::Need(b.escape("不判定").build()))
        }
        Some("pass") => {
            if left > 1 && g.seats[owner].hp > 0 {
                push(g, "baonue", owner, json!({ "stage": "ask", "left": left - 1 }));
            }
            Ok(Step::Done)
        }
        Some(_) => {
            push(g, "baonue", owner, json!({ "stage": "after", "left": left - 1 }));
            crate::kernel::card_play::start_quick_judge(g, owner, "baonue");
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 屯田（界邓艾）：失去牌后判定；非红桃为「田」；每张「田」距离 -1
// ---------------------------------------------------------------------------

pub const TUOTIAN: SkillDef = SkillDef {
    code: "tuotian",
    name: "屯田",
    triggers: &[EventKind::CardLost],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: tuotian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn tuotian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("after") {
        let Some(card) = g.judge_result.take() else {
            return Ok(Step::Done);
        };
        let heart = g.cards.get(&card).is_some_and(|c| c.suit == crate::kernel::Suit::Heart);
        if heart {
            if g.cards.contains_key(&card) {
                g.to_discard(&card, "tuotian")?;
            }
            g.push_log(format!("屯田：判定【{}】红桃，未生效", label(g, &card)));
        } else {
            if g.cards.contains_key(&card) {
                g.to_mark(&card, owner, "tian")?;
            }
            g.push_log(format!(
                "屯田：判定【{}】非红桃，置于武将牌上为「田」（{} 张）",
                label(g, &card),
                tian_count(g, owner)
            ));
        }
        return Ok(Step::Done);
    }
    let lost_card = a.data["card"].as_str().map(str::to_string);
    let cause = a.data["cause"].as_str().unwrap_or_default().to_string();
    let own_turn = g.turn == owner;
    let ok = match lost_card {
        None => false,
        Some(card) => {
            if own_turn {
                // 回合内：只看「弃置【杀】」（弃牌阶段/主动弃置）。
                cause == "to:Discard" && is_sha(g, &card)
            } else {
                true
            }
        }
    };
    if !ok {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("tuotian");
            b = b.option("opt1", "屯田：判定（非红桃 → 获得「田」）");
            Ok(Step::Need(b.escape("不发动屯田").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            push(g, "tuotian", owner, json!({ "stage": "after" }));
            crate::kernel::card_play::start_quick_judge(g, owner, "tuotian");
            Ok(Step::Done)
        }
    }
}

/// 屯田：每有一张「田」，距离 -1。
pub fn distance_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "tuotian") {
        tian_count(g, seat) as i32
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 凿险（界邓艾）：觉醒 + 本回合结束后额外回合
// ---------------------------------------------------------------------------

pub const ZAOXIAN: SkillDef = SkillDef {
    code: "zaoxian",
    name: "凿险",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: zaoxian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zaoxian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    match g.phase {
        Phase::Prepare => {
            if !active(g, owner, "zaoxian") || g.seats[owner].flags.contains("awaken_zaoxian") {
                return Ok(Step::Done);
            }
            if tian_count(g, owner) < 3 {
                return Ok(Step::Done);
            }
            g.seats[owner].flags.insert("awaken_zaoxian".to_string());
            g.push_log(format!("凿险：{} 觉醒", g.seats[owner].name));
            flow::lose_max_hp(g, owner, 1, "zaoxian");
            grant_skill(g, owner, "jixi");
            g.seats[owner].flags.insert("zaoxian_extra".to_string());
            Ok(Step::Done)
        }
        Phase::End => {
            if g.seats[owner].flags.remove("zaoxian_extra") {
                g.extra_turn = Some(owner);
                g.push_log(format!("凿险：{} 本回合结束后执行一个额外回合", g.seats[owner].name));
            }
            Ok(Step::Done)
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 巨象（界祝融）
// ---------------------------------------------------------------------------

pub const JUXIANG: SkillDef = SkillDef {
    code: "juxiang",
    name: "巨象",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: juxiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(juxiang_blocker),
    yang_window: None,
    yin_window: None,
};

fn juxiang_blocker(_g: &Game, seat: usize, _source: usize, name: &str, _carrier: Option<&CardId>) -> bool {
    let _ = seat;
    name == "南蛮入侵"
}

fn juxiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() != Some("gain") {
        return Ok(Step::Done);
    }
    if !active(g, owner, "juxiang") {
        return Ok(Step::Done);
    }
    let Some(card) = a.data["card"].as_str().map(str::to_string) else {
        return Ok(Step::Done);
    };
    if !matches!(g.zone_of(&card), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("juxiang");
            b = b.option("opt1", format!("巨象：获得【{}】", label(g, &card)));
            Ok(Step::Need(b.escape("不获得").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            g.gain(&card, owner)?;
            g.push_log(format!("巨象：{} 获得【{}】", g.seats[owner].name, label(g, &card)));
            Ok(Step::Done)
        }
    }
}

/// 【南蛮入侵】结算结束后：座位序最先的巨象持有者获得之（即使被无懈抵消）。
pub fn on_card_finished(g: &mut Game, source: usize, card: &CardId) {
    if !g.cards.contains_key(card) {
        return;
    }
    if g.def_of(card).effect != Effect::SavageAssault {
        return;
    }
    let owner = (0..g.seats.len()).find(|i| {
        *i != source && g.seats[*i].hp > 0 && active(g, *i, "juxiang")
    });
    if let Some(owner) = owner {
        push(g, "juxiang", owner, json!({ "stage": "gain", "card": card }));
    }
}

// ---------------------------------------------------------------------------
// 烈刃（界祝融）：【杀】造成伤害后可与其拼点，赢则获得其一张牌
// ---------------------------------------------------------------------------

pub const LIERENG: SkillDef = SkillDef {
    code: "liereng",
    name: "烈刃",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: liereng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn liereng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    // 拼点后：赢 → 获得目标一张牌（随机手牌/装备区，不含判定区）。
    if a.data["stage"].as_str() == Some("after") {
        let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
        let won = g
            .pindian_result
            .take()
            .map(|r| r.winner == Some(owner))
            .unwrap_or(false);
        if !won || g.seats.get(victim).map(|s| s.hp <= 0).unwrap_or(true) {
            return Ok(Step::Done);
        }
        let opts = take_one_options(g, victim);
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("liereng");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("烈刃：{text}"));
                }
                Ok(Step::Need(b.escape("不获得").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let card = take_one(g, victim, act)?;
                let text = format!("烈刃：{} 获得【{}】", g.seats[owner].name, label(g, &card));
                g.gain(&card, owner)?;
                g.push_log(text);
                Ok(Step::Done)
            }
        }
    } else {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if !active(g, owner, "liereng") {
            return Ok(Step::Done);
        }
        let Some(src) = a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok()) else {
            return Ok(Step::Done);
        };
        if src != owner {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(str::to_string) else {
            return Ok(Step::Done);
        };
        if !is_sha(g, &card) {
            return Ok(Step::Done);
        }
        let victim = a.data["seat"].as_u64().unwrap_or(0) as usize;
        if g.seats.get(victim).map(|s| s.hp <= 0).unwrap_or(true) {
            return Ok(Step::Done);
        }
        if g.hand(owner).is_empty() || g.hand(victim).is_empty() {
            return Ok(Step::Done);
        }
        let key = format!("turn.liereng_asked:{card}:{victim}");
        // 触发入口去重；答复帧（answer 非空）必须继续处理。
        if g.counter(&key) > 0 && g.answer.is_none() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                g.bump(&key);
                let mut b = builder(owner, "choice").skill("liereng");
                b = b.option("opt1", format!("烈刃：与 {} 拼点", g.seats[victim].name));
                Ok(Step::Need(b.escape("不发动烈刃").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                push(g, "liereng", owner, json!({ "stage": "after", "victim": victim }));
                crate::kernel::pindian::start(g, owner, victim, "liereng");
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 长标（界祝融）
// ---------------------------------------------------------------------------

pub const CHANGBIAO: SkillDef = SkillDef {
    code: "changbiao",
    name: "长标",
    triggers: &[EventKind::PhaseStart, EventKind::DamageAfter],
    window: Window::OwnTurn,
    priority: 0,
    can_active: Some(can_changbiao),
    handler: changbiao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_changbiao(g: &Game, seat: usize) -> bool {
    play_phase(g, seat)
        && g.counter("turn.changbiao_used") == 0
        && !g.hand(seat).is_empty()
        && g.sha_used(seat) < g.sha_limit(seat)
        && !other_alive(g, seat).is_empty()
}

fn changbiao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["event"].as_str().is_some() {
        // 触发入口：出牌阶段结束（进入弃牌阶段）结算摸牌；伤害事件记录命中。
        if a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            if a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok()) != Some(owner) {
                return Ok(Step::Done);
            }
            if g.counter("turn.changbiao_pending") > 0 {
                g.counters.insert("turn.changbiao_pending".to_string(), 0);
                g.bump("turn.changbiao_hit");
            }
            return Ok(Step::Done);
        }
        if g.phase == Phase::Discard && g.counter("turn.changbiao_hit") > 0 {
            let x = g.counter("turn.changbiao_x") as usize;
            g.counters.insert("turn.changbiao_hit".to_string(), 0);
            g.counters.insert("turn.changbiao_x".to_string(), 0);
            if x > 0 {
                let n = g.draw_with_cause(owner, x, Some("changbiao")).len();
                g.push_log(format!("长标：出牌阶段结束，摸 {n} 张"));
            }
        }
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("cards").to_string();
    match stage.as_str() {
        "cards" => {
            let sel: Vec<String> = a.data["sel"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let avail: Vec<CardId> = g
                .hand(owner)
                .into_iter()
                .filter(|id| !sel.contains(id))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("changbiao");
                    for id in &avail {
                        b = b.option(format!("pick:{id}"), format!("选【{}】", label(g, id)));
                    }
                    return if sel.is_empty() {
                        Ok(Step::Need(b.escape("取消长标").build()))
                    } else {
                        Ok(Step::Need(b.escape_with("done", "开始选择目标").build()))
                    };
                }
                Some("pass") => Ok(Step::Done),
                Some("done") => Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "targets", "sel": sel, "tg": [] }),
                )),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("长标：未知选项 {act}"))?
                        .to_string();
                    if !avail.contains(&id) {
                        return Err("长标：该牌不在手牌".into());
                    }
                    let mut sel = sel;
                    sel.push(id);
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "cards", "sel": sel }),
                    ))
                }
            }
        }
        "targets" => {
            let sel: Vec<String> = a.data["sel"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let tg: Vec<usize> = a.data["tg"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let max = 1 + super::extra_targets(g, owner, None, "杀");
            let cands: Vec<usize> = other_alive(g, owner)
                .into_iter()
                .filter(|t| !tg.contains(t))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("changbiao");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("对 {} 使用", g.seats[*t].name));
                    }
                    return if tg.is_empty() {
                        Ok(Step::Need(b.escape("取消长标").build()))
                    } else {
                        Ok(Step::Need(b.escape_with("done", "开始结算").build()))
                    };
                }
                Some("pass") => Ok(Step::Done),
                Some("done") => {
                    let x = sel.len();
                    for c in &sel {
                        g.to_discard(c, "changbiao")?;
                    }
                    g.bump("turn.changbiao_used");
                    g.counters.insert("turn.changbiao_x".to_string(), x as u32);
                    g.counters.insert("turn.changbiao_pending".to_string(), 1);
                    g.push_log(format!(
                        "长标：{} 将 {x} 张手牌当无距离【杀】使用",
                        g.seats[owner].name
                    ));
                    g.use_virtual(owner, "杀", tg.clone())?;
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("长标：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("长标：目标不合法".into());
                    }
                    let mut tg = tg;
                    tg.push(t);
                    if tg.len() >= max {
                        let x = sel.len();
                        for c in &sel {
                            g.to_discard(c, "changbiao")?;
                        }
                        g.bump("turn.changbiao_used");
                        g.counters.insert("turn.changbiao_x".to_string(), x as u32);
                        g.counters.insert("turn.changbiao_pending".to_string(), 1);
                        g.push_log(format!(
                            "长标：{} 将 {x} 张手牌当无距离【杀】使用",
                            g.seats[owner].name
                        ));
                        g.use_virtual(owner, "杀", tg)?;
                        return Ok(Step::Done);
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "targets", "sel": sel, "tg": tg }),
                    ))
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 挑衅（界姜维）
// ---------------------------------------------------------------------------

pub const TIAOXIN: SkillDef = SkillDef {
    code: "tiaoxin",
    name: "挑衅",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_tiaoxin),
    handler: tiaoxin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_tiaoxin(g: &Game, seat: usize) -> bool {
    if !play_phase(g, seat) || g.counter("turn.tiaoxin_block") > 0 {
        return false;
    }
    if g.counter("turn.tiaoxin_used") >= 2 {
        return false;
    }
    other_alive(g, seat).into_iter().any(|t| {
        g.distance(t, seat) <= g.attack_range(t) && has_cards(g, t)
    })
}

fn tiaoxin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["event"].as_str().is_some() {
        // 伤害事件：校验来自「挑衅的这张【杀】」。
        if a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
                return Ok(Step::Done);
            }
            let Some(expected) = a.data["card"].as_str().map(str::to_string) else {
                return Ok(Step::Done);
            };
            let key = format!("turn.tiaoxin_expect:{expected}");
            if g.counter(&key) > 0 && g.counter("turn.tiaoxin_wait") > 0 {
                g.bump("turn.tiaoxin_hit");
            }
            return Ok(Step::Done);
        }
        return Ok(Step::Done);
    }
    if a.data["stage"].as_str() == Some("ask") {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let sha = a.data["sha"].as_str().unwrap_or_default().to_string();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(target, "choice").skill("tiaoxin");
                b = b.option(
                    "opt1",
                    format!("挑衅：对 {} 使用【{}】", g.seats[owner].name, label(g, &sha)),
                );
                Ok(Step::Need(
                    b.escape("不出【杀】（姜维弃你一张牌）").build(),
                ))
            }
            Some("pass") => {
                push(g, "tiaoxin", owner, json!({ "stage": "after", "target": target }));
                Ok(Step::Done)
            }
            Some(_) => {
                if !g.hand(target).contains(&sha) {
                    return Err("挑衅：目标已没有该【杀】".into());
                }
                g.counters.insert("turn.tiaoxin_wait".to_string(), 1);
                g.counters
                    .insert(format!("turn.tiaoxin_expect:{sha}"), 1);
                push(g, "tiaoxin", owner, json!({ "stage": "after", "target": target }));
                g.use_as(target, &sha, "杀", vec![owner], false, None)?;
                Ok(Step::Done)
            }
        }
    } else if a.data["stage"].as_str() == Some("after") {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let hit = g.counter("turn.tiaoxin_hit") > 0;
        g.counters.insert("turn.tiaoxin_hit".to_string(), 0);
        g.counters.insert("turn.tiaoxin_wait".to_string(), 0);
        if hit {
            g.counters.insert("turn.tiaoxin_block".to_string(), 1);
            g.push_log(format!(
                "挑衅：{} 的【杀】造成了伤害，不弃其牌（本阶段不能再发动）",
                g.seats.get(target).map(|s| s.name.clone()).unwrap_or_default()
            ));
            return Ok(Step::Done);
        }
        // 未造成伤害：姜维选其随机手牌或装备区牌弃置（不含判定区）。
        let opts = take_one_options(g, target);
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("tiaoxin");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("挑衅：弃置{text}"));
                }
                Ok(Step::Need(b.escape("不弃置").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let card = take_one(g, target, act)?;
                let text = format!("挑衅：{} 弃置【{}】", g.seats[owner].name, label(g, &card));
                g.to_discard(&card, "tiaoxin")?;
                g.push_log(text);
                Ok(Step::Done)
            }
        }
    } else {
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|t| g.distance(*t, owner) <= g.attack_range(*t) && has_cards(g, *t))
            .collect();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("tiaoxin");
                for t in &cands {
                    b = b.option(format!("target:{t}"), format!("挑衅 {}", g.seats[*t].name));
                }
                if g.counter("phase.cancel_tiaoxin") > 0 {
                    return Ok(Step::Done);
                }
                Ok(Step::Need(b.escape("不发动挑衅").build()))
            }
            Some("pass") => {
                g.bump("phase.cancel_tiaoxin");
                Ok(Step::Done)
            }
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("挑衅：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("挑衅：目标不合法".into());
                }
                g.bump("turn.tiaoxin_used");
                let sha = g
                    .hand(t)
                    .into_iter()
                    .find(|id| g.def_of(id).tags.sha);
                match sha {
                    Some(sha) => {
                        // 询问目标：出【杀】/ 不出。
                        push(
                            g,
                            "tiaoxin",
                            owner,
                            json!({ "stage": "ask", "target": t, "sha": sha }),
                        );
                        Ok(Step::Done)
                    }
                    None => {
                        // 没【杀】：直接进入弃牌。
                        push(g, "tiaoxin", owner, json!({ "stage": "after", "target": t }));
                        Ok(Step::Done)
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 志继（界姜维）：觉醒（准备/结束阶段，无手牌）
// ---------------------------------------------------------------------------

pub const ZHIJI: SkillDef = SkillDef {
    code: "zhiji",
    name: "志继",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: zhiji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhiji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !matches!(g.phase, Phase::Prepare | Phase::End) {
        return Ok(Step::Done);
    }
    if !active(g, owner, "zhiji")
        || g.seats[owner].flags.contains("awaken_zhiji")
        || !g.hand(owner).is_empty()
    {
        return Ok(Step::Done);
    }
    let can_heal = g.seats[owner].hp < g.seats[owner].max_hp;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("zhiji");
            if can_heal {
                b = b.option("opt1", "志继：回复 1 点体力");
            }
            b = b.option("opt2", "志继：摸 2 张牌");
            Ok(Step::Need(b.escape("（不会出现）不结算").build()))
        }
        Some("opt1") => {
            g.seats[owner].flags.insert("awaken_zhiji".to_string());
            flow::heal(g, owner, 1, "zhiji");
            finish_zhiji(g, owner);
            Ok(Step::Done)
        }
        Some(_) => {
            g.seats[owner].flags.insert("awaken_zhiji".to_string());
            let n = g.draw_with_cause(owner, 2, Some("zhiji")).len();
            g.push_log(format!("志继：摸 {n} 张"));
            finish_zhiji(g, owner);
            Ok(Step::Done)
        }
    }
}

fn finish_zhiji(g: &mut Game, owner: usize) {
    flow::lose_max_hp(g, owner, 1, "zhiji");
    grant_skill(g, owner, "guanxing");
    g.push_log(format!(
        "志继：{} 觉醒，减少 1 点体力上限并获得「观星」",
        g.seats[owner].name
    ));
}

// ---------------------------------------------------------------------------
// 帷幕（界贾诩）
// ---------------------------------------------------------------------------

pub const WEIMU: SkillDef = SkillDef {
    code: "weimu",
    name: "帷幕",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(weimu_blocker),
    yang_window: None,
    yin_window: None,
};

fn weimu_blocker(
    g: &Game,
    seat: usize,
    _source: usize,
    name: &str,
    carrier: Option<&CardId>,
) -> bool {
    if !active(g, seat, "weimu") {
        return false;
    }
    if crate::cards::def(name).kind != CardKind::Trick {
        return false;
    }
    // 颜色按实体牌（用户裁定 Q3=A）；无色虚拟锦囊不拦。
    carrier.map(|c| !g.is_red(c)).unwrap_or(false)
}

/// 帷幕：自己回合内受到的伤害防止，并摸 2×点数（用户裁定 Q4=A）。
pub fn weimu_prevent(g: &mut Game, source: Option<usize>, victim: usize, amount: i32) -> bool {
    if amount <= 0 || g.turn != victim || !active(g, victim, "weimu") {
        return false;
    }
    let _ = source;
    let n = g.draw_with_cause(victim, (amount as usize) * 2, Some("weimu")).len();
    g.presentation.push(
        Presentation::new("weimu", POutcome::Activated { skill: "帷幕".to_string() })
            .target(victim)
            .fx("weimu_prevent"),
    );
    g.push_log(format!(
        "帷幕：{} 防止回合内受到的 {amount} 点伤害并摸 {n} 张",
        g.seats[victim].name
    ));
    true
}

// ---------------------------------------------------------------------------
// 乱武（界贾诩，限定）
// ---------------------------------------------------------------------------

pub const LUANWU: SkillDef = SkillDef {
    code: "luanwu",
    name: "乱武",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_luanwu),
    handler: luanwu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_luanwu(g: &Game, seat: usize) -> bool {
    play_phase(g, seat) && !g.seats[seat].flags.contains("luanwu_used") && !other_alive(g, seat).is_empty()
}

fn nearest_tied(g: &Game, seat: usize) -> Vec<usize> {
    let alive = other_alive(g, seat);
    let d = alive.iter().map(|i| g.distance(seat, *i)).min().unwrap_or(0);
    alive
        .into_iter()
        .filter(|i| g.distance(seat, *i) == d)
        .collect()
}

fn luanwu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("start").to_string();
    match stage.as_str() {
        "start" => {
            g.seats[owner].flags.insert("luanwu_used".to_string());
            let order: Vec<usize> = g
                .seats_from_left_of(owner)
                .into_iter()
                .filter(|i| *i != owner && g.seats[*i].hp > 0)
                .collect();
            g.push_log(format!("乱武：{} 令所有其他角色依次选择", g.seats[owner].name));
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "seq", "order": order, "cursor": 0 }),
            ))
        }
        "seq" => {
            let order: Vec<usize> = a.data["order"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
                .unwrap_or_default();
            let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
            while cursor < order.len() && g.seats[order[cursor]].hp <= 0 {
                cursor += 1;
            }
            if cursor >= order.len() {
                if g.seats[owner].hp > 0 {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "final" }),
                    ));
                }
                return Ok(Step::Done);
            }
            let seat = order[cursor];
            let nearest = nearest_tied(g, seat);
            let sha = g.hand(seat).into_iter().find(|id| g.def_of(id).tags.sha);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(seat, "choice").skill("luanwu");
                    if let Some(sha) = &sha {
                        for t in &nearest {
                            b = b.option(
                                format!("target:{t}"),
                                format!("对 {}（距离最小）使用【{}】", g.seats[*t].name, label(g, sha)),
                            );
                        }
                    }
                    // 失去体力是必选项（出口就是它，不提供额外放弃）。
                    Ok(Step::Need(
                        b.escape_with("lose", "失去 1 点体力").build(),
                    ))
                }
                Some("lose") => {
                    g.push_log(format!("乱武：{} 失去 1 点体力", g.seats[seat].name));
                    flow::lose_hp(g, seat, 1, "luanwu");
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "seq", "order": order, "cursor": cursor + 1 }),
                    ))
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("乱武：未知选项 {act}"))?;
                    if !nearest.contains(&t) {
                        return Err("乱武：只能对距离最小的角色使用【杀】".into());
                    }
                    let sha = sha.ok_or_else(|| "乱武：没有【杀】".to_string())?;
                    g.push_log(format!(
                        "乱武：{} 对 {} 使用【杀】",
                        g.seats[seat].name, g.seats[t].name
                    ));
                    g.use_as(seat, &sha, "杀", vec![t], false, None)?;
                    push(
                        g,
                        "luanwu",
                        owner,
                        json!({ "stage": "seq", "order": order, "cursor": cursor + 1 }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "final" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("luanwu");
                b = b.option("opt1", "乱武：视为使用一张无距离限制的【杀】");
                Ok(Step::Need(b.escape("不使用").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => Ok(Step::Next(Stage::Choose, json!({ "stage": "final_target" }))),
        },
        "final_target" => {
            let cands = other_alive(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("luanwu");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("对 {} 使用【杀】", g.seats[*t].name));
                    }
                    Ok(Step::Need(b.escape("不使用").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("乱武：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("乱武：目标不合法".into());
                    }
                    g.push_log(format!("乱武：{} 视为使用一张无距离限制的【杀】", g.seats[owner].name));
                    g.use_virtual(owner, "杀", vec![t])?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 完杀（界贾诩）
// ---------------------------------------------------------------------------

pub const WANSHA: SkillDef = SkillDef {
    code: "wansha",
    name: "完杀",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 濒死开始：若当前回合角色有完杀 → 记录锁定状态（其他角色非锁定技失效）。
pub fn entering_dying(g: &mut Game, victim: usize) {
    if active(g, g.turn, "wansha") {
        g.counters.insert("dying.wansha_owner".to_string(), g.turn as u32 + 1);
        g.counters.insert("dying.victim".to_string(), victim as u32 + 1);
        g.counters.insert("dying.active".to_string(), 1);
        g.push_log(format!(
            "完杀：{} 的回合内濒死结算，其他角色的非锁定技失效",
            g.seats[g.turn].name
        ));
    }
}

/// 濒死结束（救起或死亡）：清理锁定状态。
pub fn leaving_dying(g: &mut Game) {
    g.counters.remove("dying.wansha_owner");
    g.counters.remove("dying.victim");
    g.counters.remove("dying.active");
}

/// 完杀：只有贾诩与濒死角色本人可以使用【桃】（用户裁定 Q2=A）。
pub fn dying_peach_blocked(g: &Game, asker: usize, victim: usize) -> bool {
    let owner = g.counter("dying.wansha_owner");
    if owner == 0 {
        return false;
    }
    let owner = owner as usize - 1;
    asker != victim && asker != owner
}

// ---------------------------------------------------------------------------
// 好施（界鲁肃）
// ---------------------------------------------------------------------------

pub const HAOSHI: SkillDef = SkillDef {
    code: "haoshi",
    name: "好施",
    triggers: &[
        EventKind::PhaseStart,
        EventKind::TargetChosen,
        EventKind::TurnStart,
    ],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: haoshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn haoshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 触发项用 `a.actor` 作为鲁肃；回礼询问把 actor 换成回礼者（data.owner 保留鲁肃）。
    let owner = a.data["owner"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let ev = a.data["event"].as_str().unwrap_or("").to_string();

    // ① 到鲁肃的下回合开始：清除回礼者。
    if stage.is_empty() && ev.starts_with("TurnStart") {
        if a.data["seat"].as_u64().map(|n| n as usize) == Some(owner) {
            g.counters.remove(&format!("haoshi_back:{owner}"));
        }
        return Ok(Step::Done);
    }
    // ② 摸牌阶段：是否多摸 2 张。
    if stage.is_empty() && ev.starts_with("PhaseStart") {
        // 触发入口只问一次；答复帧（answer 非空）必须继续处理。
        if g.phase != Phase::Draw
            || (g.counter("turn.haoshi_offered") > 0 && g.answer.is_none())
        {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                g.bump("turn.haoshi_offered");
                let mut b = builder(owner, "choice").skill("haoshi");
                b = b.option("opt1", "好施：本阶段多摸 2 张");
                Ok(Step::Need(b.escape("正常摸牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.bump("turn.haoshi");
                g.push_log(format!("好施：{} 本阶段多摸 2 张", g.seats[owner].name));
                Ok(Step::Done)
            }
        }
    } else if stage.is_empty() && ev.starts_with("TargetChosen") {
        // ③ 成为【杀】/普通锦囊目标后：回礼者可以交给鲁肃一张手牌。
        let helper = g.counter(&format!("haoshi_back:{owner}"));
        if helper == 0 || !active(g, owner, "haoshi") {
            return Ok(Step::Done);
        }
        let helper = helper as usize - 1;
        let Some(card) = a.data["card"].as_str().map(str::to_string) else {
            return Ok(Step::Done);
        };
        let targets: Vec<usize> = a.data["cause"]
            .as_str()
            .map(|s| {
                s.split(',')
                    .filter_map(|t| t.parse::<usize>().ok())
                    .collect()
            })
            .unwrap_or_default();
        if !targets.contains(&owner) {
            return Ok(Step::Done);
        }
        let def = crate::cards::def(&g.name_of(&card));
        let ok = def.tags.sha || (def.kind == CardKind::Trick && !def.tags.delayed);
        if !ok || g.seats.get(helper).map(|s| s.hp <= 0).unwrap_or(true) {
            return Ok(Step::Done);
        }
        if g.hand(helper).is_empty() {
            return Ok(Step::Done);
        }
        push(
            g,
            "haoshi",
            helper,
            json!({ "stage": "back", "owner": owner }),
        );
        Ok(Step::Done)
    } else {
        match stage.as_str() {
            // ④ 回礼者：是否交给鲁肃一张手牌（可以）。
            "back" => match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("haoshi");
                    b = b.option("opt1", format!("好施：交给 {} 一张手牌", g.seats[owner].name));
                    Ok(Step::Need(b.escape("不交").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(_) => {
                    if let Some(card) = random_hand(g, a.actor) {
                        g.gain(&card, owner)?;
                        g.push_log(format!(
                            "好施：{} 交给 {} 一张手牌",
                            g.seats[a.actor].name, g.seats[owner].name
                        ));
                    }
                    Ok(Step::Done)
                }
            },
            // ⑤ 交一半：逐张选牌（n = 当前手牌 ÷2 向下取整）。
            "select" => {
                let sel: Vec<String> = a.data["sel"]
                    .as_array()
                    .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let need = g.hand(owner).len() / 2;
                if need == 0 || sel.len() >= need {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "target", "sel": sel }),
                    ));
                }
                let avail: Vec<CardId> = g
                    .hand(owner)
                    .into_iter()
                    .filter(|id| !sel.contains(id))
                    .collect();
                if avail.is_empty() {
                    return Ok(Step::Done);
                }
                match g.answer.take().as_deref() {
                    None => {
                        let mut b = builder(owner, "choice").skill("haoshi");
                        for id in &avail {
                            b = b.option(format!("pick:{id}"), format!("选【{}】", label(g, id)));
                        }
                        Ok(Step::Need(b.escape("不交牌").build()))
                    }
                    Some("pass") => Ok(Step::Done),
                    Some(act) => {
                        let id = act
                            .strip_prefix("pick:")
                            .ok_or_else(|| format!("好施：未知选项 {act}"))?
                            .to_string();
                        if !avail.contains(&id) {
                            return Err("好施：该牌不在手牌".into());
                        }
                        let mut sel = sel;
                        sel.push(id);
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "select", "sel": sel }),
                        ))
                    }
                }
            }
            // ⑥ 交给手牌最少的一名其他角色。
            "target" => {
                let sel: Vec<String> = a.data["sel"]
                    .as_array()
                    .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let min = other_alive(g, owner)
                    .into_iter()
                    .map(|i| g.hand(i).len())
                    .min()
                    .unwrap_or(0);
                let cands: Vec<usize> = other_alive(g, owner)
                    .into_iter()
                    .filter(|i| g.hand(*i).len() == min)
                    .collect();
                if cands.is_empty() {
                    return Ok(Step::Done);
                }
                match g.answer.take().as_deref() {
                    None => {
                        let mut b = builder(owner, "target").skill("haoshi");
                        for t in &cands {
                            b = b.option(format!("target:{t}"), format!("交给 {}", g.seats[*t].name));
                        }
                        Ok(Step::Need(b.escape("放弃交牌").build()))
                    }
                    Some("pass") => Ok(Step::Done),
                    Some(act) => {
                        let t = act
                            .strip_prefix("target:")
                            .and_then(|x| x.parse::<usize>().ok())
                            .ok_or_else(|| format!("好施：未知选项 {act}"))?;
                        if !cands.contains(&t) {
                            return Err("好施：目标必须是手牌最少的其他角色".into());
                        }
                        let n = sel.len();
                        for id in &sel {
                            if g.hand(owner).contains(id) {
                                g.gain(id, t)?;
                            }
                        }
                        // 回礼者覆盖为最新一名（用户裁定 Q3=A）。
                        g.counters.insert(format!("haoshi_back:{owner}"), t as u32 + 1);
                        g.push_log(format!(
                            "好施：{} 交给 {} {n} 张牌",
                            g.seats[owner].name, g.seats[t].name
                        ));
                        Ok(Step::Done)
                    }
                }
            }
            _ => Ok(Step::Done),
        }
    }
}

/// 好施：摸牌阶段多摸 2 张（可选，询问后置 `turn.haoshi`）。
pub fn draw_bonus(g: &Game, seat: usize) -> usize {
    if active(g, seat, "haoshi") && g.counter("turn.haoshi") > 0 {
        2
    } else {
        0
    }
}

/// 摸牌阶段结束后：好施交一半（手牌 >5）。
pub fn after_draw_phase(g: &mut Game, seat: usize) {
    if active(g, seat, "haoshi")
        && g.counter("turn.haoshi") > 0
        && g.hand(seat).len() > 5
    {
        push(g, "haoshi", seat, json!({ "stage": "select", "sel": [] }));
    }
}

// ---------------------------------------------------------------------------
// 缔盟（界鲁肃）
// ---------------------------------------------------------------------------

pub const DIMENG: SkillDef = SkillDef {
    code: "dimeng",
    name: "缔盟",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: Some(can_dimeng),
    handler: dimeng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_dimeng(g: &Game, seat: usize) -> bool {
    play_phase(g, seat)
        && g.counter("turn.dimeng_used") == 0
        && other_alive(g, seat).len() >= 2
}

fn dimeng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("start").to_string();
    if stage == "start" && a.data["event"].as_str().is_none() {
        let first = a.data["first"].as_u64().map(|n| n as usize);
        let budget = g.hand(owner).len();
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|i| Some(*i) != first)
            .filter(|i| match first {
                Some(f) => g.hand(f).len().abs_diff(g.hand(*i).len()) <= budget,
                None => true,
            })
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("dimeng");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("选择 {}（手牌 {}）", g.seats[*t].name, g.hand(*t).len()),
                    );
                }
                Ok(Step::Need(b.escape("不发动缔盟").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("缔盟：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("缔盟：目标不合法".into());
                }
                match first {
                    None => {
                        g.bump("turn.dimeng_used");
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "start", "first": t }),
                        ))
                    }
                    Some(first) => {
                        let x = g.hand(first).len().abs_diff(g.hand(t).len());
                        // 交换手牌（触发获得/失去事件，用户裁定 Q4=B）。
                        let a_cards = g.hand(first);
                        let b_cards = g.hand(t);
                        for id in &a_cards {
                            g.gain(id, t)?;
                        }
                        for id in &b_cards {
                            g.gain(id, first)?;
                        }
                        if x > 0 {
                            g.counters
                                .insert("turn.dimeng_discard".to_string(), x as u32);
                        }
                        g.push_log(format!(
                            "缔盟：交换 {} 与 {} 的手牌（出牌阶段结束弃 {x} 张）",
                            g.seats[first].name, g.seats[t].name
                        ));
                        Ok(Step::Done)
                    }
                }
            }
        }
    } else if a.data["event"].as_str().is_some() {
        // 出牌阶段结束（进入弃牌阶段）：弃 X 张。
        if g.phase != Phase::Discard {
            return Ok(Step::Done);
        }
        let left = g.counter("turn.dimeng_discard");
        if left == 0 {
            return Ok(Step::Done);
        }
        let mut avail = g.hand(owner);
        if avail.is_empty() {
            g.counters.insert("turn.dimeng_discard".to_string(), 0);
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("dimeng");
                for id in &avail {
                    b = b.option(format!("pick:{id}"), format!("缔盟：弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("手牌不足，弃光结束").build()))
            }
            Some("pass") => {
                g.counters.insert("turn.dimeng_discard".to_string(), 0);
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("缔盟：未知选项 {act}"))?
                    .to_string();
                if !avail.contains(&id) {
                    return Err("缔盟：该牌不在手牌".into());
                }
                g.to_discard(&id, "dimeng")?;
                avail.retain(|c| c != &id);
                let left = left.saturating_sub(1);
                if left == 0 || avail.is_empty() {
                    g.counters.insert("turn.dimeng_discard".to_string(), 0);
                    Ok(Step::Done)
                } else {
                    g.counters
                        .insert("turn.dimeng_discard".to_string(), left);
                    Ok(Step::Next(Stage::Choose, json!({ "stage": "pay" })))
                }
            }
        }
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 巧变（界张郃）
// ---------------------------------------------------------------------------

pub const QIAOBIAN: SkillDef = SkillDef {
    code: "qiaobian",
    name: "巧变",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qiaobian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 开局：巧变持有者获得 2 枚「变」。
pub fn on_game_start(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if active(g, seat, "qiaobian") {
            add_bian(g, seat, 2);
            g.push_log(format!("巧变：{} 开局获得 2 枚「变」", g.seats[seat].name));
        }
    }
}

fn qiaobian_can_pay(g: &Game, seat: usize) -> bool {
    !g.hand(seat).is_empty() || !equip_ids(g, seat).is_empty() || bian(g, seat) > 0
}

/// 阶段开始前询问（准备/结束阶段除外）。返回 true = 已压入询问，调用方应暂停切换阶段。
pub fn before_phase(g: &mut Game, seat: usize, phase: Phase) -> bool {
    if !active(g, seat, "qiaobian") {
        return false;
    }
    if !matches!(phase, Phase::Judge | Phase::Draw | Phase::Play | Phase::Discard) {
        return false;
    }
    if !qiaobian_can_pay(g, seat) {
        return false;
    }
    let key = format!("turn.qiaobian_asked:{}", phase_name(phase));
    if g.counter(&key) > 0 {
        return false;
    }
    g.bump(&key);
    push(
        g,
        "qiaobian",
        seat,
        json!({ "stage": "ask", "phase": phase_name(phase) }),
    );
    true
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Prepare => "prepare",
        Phase::Judge => "judge",
        Phase::Draw => "draw",
        Phase::Play => "play",
        Phase::Discard => "discard",
        Phase::End => "end",
    }
}

fn phase_of(name: &str) -> Option<Phase> {
    Some(match name {
        "judge" => Phase::Judge,
        "draw" => Phase::Draw,
        "play" => Phase::Play,
        "discard" => Phase::Discard,
        _ => return None,
    })
}

fn qiaobian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let phase = a
        .data["phase"]
        .as_str()
        .and_then(phase_of)
        .unwrap_or(Phase::Play);
    match stage.as_str() {
        "ask" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qiaobian");
                b = b.option(
                    "opt1",
                    format!("巧变：跳过的{}阶段", phase_name(phase)),
                );
                Ok(Step::Need(b.escape("不发动巧变").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pay", "phase": phase_name(phase) }),
            )),
        },
        "pay" => {
            let mut opts: Vec<(String, String)> = Vec::new();
            for id in hand_and_equip(g, owner) {
                opts.push((format!("pick:{id}"), format!("弃置【{}】", label(g, &id))));
            }
            if bian(g, owner) > 0 {
                opts.push((
                    "pick:bian".to_string(),
                    format!("移除 1 枚「变」（剩余 {}）", bian(g, owner)),
                ));
            }
            if opts.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("qiaobian");
                    for (id, text) in &opts {
                        b = b.option(id.clone(), text.clone());
                    }
                    Ok(Step::Need(b.escape("不发动巧变").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    if let Some(id) = act.strip_prefix("pick:") {
                        if id == "bian" {
                            remove_bian(g, owner);
                        } else {
                            let id = id.to_string();
                            if !hand_and_equip(g, owner).contains(&id) {
                                return Err("巧变：该牌不在手牌/装备区".into());
                            }
                            g.to_discard(&id, "qiaobian")?;
                        }
                    }
                    // 记录跳过标记。
                    let key = match phase {
                        Phase::Judge => "turn.skip_judge",
                        Phase::Draw => "turn.skip_draw",
                        Phase::Play => "turn.skip_play",
                        Phase::Discard => "turn.skip_discard",
                        _ => return Ok(Step::Done),
                    };
                    g.counters.insert(key.to_string(), 1);
                    g.push_log(format!(
                        "巧变：{} 跳过{}阶段",
                        g.seats[owner].name,
                        phase_name(phase)
                    ));
                    match phase {
                        Phase::Draw => Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "gain", "left": 2 }),
                        )),
                        Phase::Play => Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "move_src" }),
                        )),
                        _ => Ok(Step::Done),
                    }
                }
            }
        }
        "gain" => {
            let left = a.data["left"].as_u64().unwrap_or(0) as usize;
            if left == 0 {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = other_alive(g, owner)
                .into_iter()
                .filter(|t| !g.hand(*t).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("qiaobian");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("获得 {} 一张手牌", g.seats[*t].name));
                    }
                    Ok(Step::Need(b.escape("停止获得").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("巧变：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("巧变：目标不合法".into());
                    }
                    if let Some(card) = random_hand(g, t) {
                        g.gain(&card, owner)?;
                        g.push_log(format!(
                            "巧变：{} 获得 {} 一张手牌【{}】",
                            g.seats[owner].name,
                            g.seats[t].name,
                            label(g, &card)
                        ));
                    }
                    if left > 1 {
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "gain", "left": left - 1 }),
                        ))
                    } else {
                        Ok(Step::Done)
                    }
                }
            }
        }
        "move_src" => {
            let mut opts: Vec<(String, String)> = Vec::new();
            for seat in 0..g.seats.len() {
                if g.seats[seat].hp <= 0 {
                    continue;
                }
                for slot in SLOTS {
                    if let Some(id) = g.equip_in(seat, slot) {
                        opts.push((
                            format!("move:{id}"),
                            format!("{}的{}区：{}", g.seats[seat].name, slot.label(), label(g, &id)),
                        ));
                    }
                }
                for id in g.judge_zone(seat) {
                    opts.push((
                        format!("move:{id}"),
                        format!("{}的判定区：{}", g.seats[seat].name, label(g, &id)),
                    ));
                }
            }
            if opts.is_empty() {
                g.push_log("巧变：场上没有可移动的牌，交互结束".to_string());
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("qiaobian");
                    for (id, text) in &opts {
                        b = b.option(id.clone(), text.clone());
                    }
                    Ok(Step::Need(b.escape("放弃移动").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("move:")
                        .ok_or_else(|| format!("巧变：未知选项 {act}"))?
                        .to_string();
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "move_dst", "card": id }),
                    ))
                }
            }
        }
        "move_dst" => {
            let id = a.data["card"].as_str().unwrap_or_default().to_string();
            if !g.cards.contains_key(&id) {
                return Ok(Step::Done);
            }
            let from_seat = match g.zone_of(&id) {
                Some(Zone::Equip(s, _)) | Some(Zone::Judge(s)) => Some(*s),
                _ => None,
            };
            let Some(from_seat) = from_seat else {
                return Ok(Step::Done);
            };
            let is_equip = matches!(g.zone_of(&id), Some(Zone::Equip(_, _)));
            let slot = g.def_of(&id).slot;
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|t| *t != from_seat && g.seats[*t].hp > 0)
                .filter(|t| {
                    if is_equip {
                        slot.map(|s| g.equip_in(*t, s).is_none()).unwrap_or(false)
                    } else {
                        true
                    }
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("qiaobian");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("移动到 {}", g.seats[*t].name));
                    }
                    Ok(Step::Need(b.escape("放弃移动").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("巧变：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("巧变：落点不合法".into());
                    }
                    if is_equip {
                        let Some(slot) = slot else {
                            return Ok(Step::Done);
                        };
                        g.move_equip(&id, t, slot)?;
                    } else {
                        g.move_judge_to(&id, t)?;
                    }
                    g.push_log(format!(
                        "巧变：将【{}】移动到 {}",
                        label(g, &id),
                        g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

pub const B15_SKILLS: &[SkillDef] = &[
    TIANYI,
    HANZHAN,
    ROULIN,
    BENGKUAI,
    JIUCHI,
    BAONUE,
    TUOTIAN,
    ZAOXIAN,
    JUXIANG,
    LIERENG,
    CHANGBIAO,
    TIAOXIN,
    ZHIJI,
    WEIMU,
    LUANWU,
    WANSHA,
    HAOSHI,
    DIMENG,
    QIAOBIAN,
];

// ---------------------------------------------------------------------------
// 巧变：「变」标记
// ---------------------------------------------------------------------------

fn remove_bian(g: &mut Game, seat: usize) {
    let cur = bian(g, seat);
    if cur == 0 {
        return;
    }
    if cur == 1 {
        g.seats[seat].counters.remove("bian");
    } else {
        g.seats[seat].counters.insert("bian".to_string(), cur - 1);
    }
}

// ---------------------------------------------------------------------------
// 酣战：拼点前/后的内核钩子
// ---------------------------------------------------------------------------

/// 拼点参与者中拥有「拼点前」技能的人（酣战）：返回 (持有者, 对手)。
pub fn pindian_pre_owners(g: &Game, initiator: usize, target: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if active(g, initiator, "hanzhan") && !g.hand(target).is_empty() {
        out.push((initiator, target));
    }
    if active(g, target, "hanzhan") && !g.hand(initiator).is_empty() {
        out.push((target, initiator));
    }
    out
}

/// 酣战发动：对手的拼点牌改为随机手牌。
pub fn pindian_pre_apply(g: &mut Game, owner: usize, other: usize) {
    g.counters.insert(format!("pindian.random:{other}"), 1);
    g.push_log(format!(
        "酣战：{} 令 {} 用随机手牌拼点",
        g.seats[owner].name, g.seats[other].name
    ));
}

/// 该方拼点牌是否被强制随机（读后即清除）。
pub fn pindian_random_forced(g: &mut Game, seat: usize) -> bool {
    let key = format!("pindian.random:{seat}");
    if g.counter(&key) > 0 {
        g.counters.remove(&key);
        true
    } else {
        false
    }
}

/// 拼点结算后：给拥有「拼点后」技能的参与者压入询问（酣战）。
pub fn pindian_after(
    g: &mut Game,
    initiator: usize,
    target: usize,
    a: &str,
    b: &str,
    _winner: Option<usize>,
) {
    for owner in [initiator, target] {
        if active(g, owner, "hanzhan") {
            crate::kernel::resolve::push_ask(
                g,
                "hanzhan",
                Stage::Choose,
                owner,
                serde_json::json!({ "stage": "after", "cards": [a, b] }),
            );
        }
    }
}
