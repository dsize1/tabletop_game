//! B17（界限突破·J5）——按 `align/B17.json` 第 7 段实现。
//!
//! 覆盖：界凌统（旋风）、界陈宫（智迟/明策）、界曹彰（将驰）、界简雍（纵适/巧说）、
//! 界虞翻（纵玄/直言）、界高顺（陷阵/禁酒）、界法正（眩惑/恩怨）、界程普（疠火/醇醪）。
//!
//! 用户裁定（2026-11-10，71 条疑点）要点：
//! - 旋风：单次动作失去 ≥2 张（含非弃置路径）；至多 2 名角色、可重复选同一名（共 2 张）；弃哪张随机。
//! - 智迟：「无效」= 不能成为目标（目标枚举裁剪）；普通锦囊按牌类别；濒死救起后仍开启。
//! - 明策：交牌含装备区；虚拟【杀】受距离限制；伤害后陈宫与其各摸 1；无合法目标不提供①。
//! - 将驰：②重铸含装备区；次数 -1 与其它加成同层；弃置计入弃置；无距离只作用于【杀】。
//! - 纵适：code = `zongshi_jie`（与宗室解耦）；询问是否获得；按有效点数比较；从弃牌堆取回。
//! - 巧说：无次数限制（可连胜）；下一张有目标牌 ±1；没赢 → 不能使用锦囊且技能失效。
//! - 纵玄：实现「上家每回合首次弃置」；每回合一次；置于牌堆顶按选择顺序。
//! - 直言：走 reveal；失去体力走 lose_hp；装备自动使用；上家/自己结束阶段各一次。
//! - 陷阵：赢 → 对其无距离/无次数/无视防具/可多指定其为目标；没赢 → 只对其禁用【杀】。
//! - 禁酒：点数 K；高顺自己不能把【酒】当【酒】；其他角色在其回合不能用【酒】（含濒死/技能视为）。
//! - 眩惑：按官方界版重做（摸牌阶段结束交两张手牌 + 观看手牌获得两张）。
//! - 恩怨：单次获得 ≥2 张即触发；询问；红色按物理花色；失去体力走 lose_hp。
//! - 疠火：非火【杀】（含雷）可转化；代价在「此牌结算后」询问（弃牌或失去体力）。
//! - 醇醪：相邻角色弃置的【杀】自动置「醇」（≤9）；濒死移去 X 张视为【酒】（X 本轮递增）。

use serde_json::json;

use crate::cards::{CardKind, DamageKind, Effect};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};

use super::{active, builder, other_alive, SkillDef};

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
    crate::kernel::resolve::push_ask(g, code, Stage::Choose, actor, data);
}
fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}
fn alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect()
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
fn physical_effect(g: &Game, id: &CardId) -> Effect {
    g.cards
        .get(id)
        .map(|c| crate::cards::def(&c.name).effect)
        .unwrap_or(Effect::None)
}
fn random_hand(g: &mut Game, seat: usize) -> Option<CardId> {
    let n = g.hand(seat).len();
    if n == 0 {
        return None;
    }
    let idx = g.rand_below(n);
    Some(g.hand(seat)[idx].clone())
}
fn sha_in_hand(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| is_sha(g, id))
        .collect()
}
fn upper_seat_alive(g: &Game, seat: usize) -> Option<usize> {
    let alive = alive(g);
    let n = alive.len();
    if n <= 1 {
        return None;
    }
    let idx = alive.iter().position(|i| *i == seat)?;
    Some(alive[(idx + n - 1) % n])
}
fn adjacent_alive(g: &Game, a: usize, b: usize) -> bool {
    if a == b {
        return false;
    }
    let alive = alive(g);
    let n = alive.len();
    if n <= 1 {
        return false;
    }
    match (
        alive.iter().position(|i| *i == a),
        alive.iter().position(|i| *i == b),
    ) {
        (Some(ia), Some(ib)) => {
            let d = ia.abs_diff(ib);
            d == 1 || d + 1 == n
        }
        _ => false,
    }
}

/// 目标可选牌：装备区具体牌 + 手牌（随机）。不含判定区。
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

/// 随机弃置目标一张牌（先手牌，无手牌再取装备区第一张）。
fn discard_random(g: &mut Game, target: usize, cause: &str) -> Option<CardId> {
    let card = if !g.hand(target).is_empty() {
        random_hand(g, target)?
    } else {
        equip_ids(g, target).into_iter().next()?
    };
    g.to_discard(&card, cause).ok()?;
    Some(card)
}

// ---------------------------------------------------------------------------
// 旋风（界凌统）
// ---------------------------------------------------------------------------

pub const XUANFENG: SkillDef = SkillDef {
    code: "xuanfeng",
    name: "旋风",
    triggers: &[EventKind::CardLost],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: xuanfeng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn xuanfeng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("CardLost")
        {
            return Ok(Step::Done);
        }
        if !active(g, owner, "xuanfeng") || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        let Some(lost_by) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        if lost_by != owner {
            return Ok(Step::Done);
        }
        let lost_card = a.data["card"].as_str().unwrap_or_default().to_string();
        // 失去装备区里的牌 → 触发；一次性失去 ≥2 张（本动作差分）→ 触发。
        let equip_lost = g
            .equip_lost_batch
            .iter()
            .any(|(s, c)| *s == owner && *c == lost_card);
        let batch = g.lost_batch.get(&owner).map(Vec::len).unwrap_or(0);
        if !equip_lost && batch < 2 {
            return Ok(Step::Done);
        }
        if !g.action_seen.insert("xuanfeng".to_string()) {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = alive(g)
            .into_iter()
            .filter(|t| *t != owner && !take_one_options(g, *t).is_empty())
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "target", "owner": owner, "chosen": [] }),
        ));
    }
    // 目标选择：先选至多 2 名（可重复）→ 结算。
    let cands: Vec<usize> = alive(g)
        .into_iter()
        .filter(|t| *t != owner && !take_one_options(g, *t).is_empty())
        .collect();
    let mut chosen: Vec<usize> = a.data["chosen"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    if chosen.len() >= 2 || (chosen.is_empty() && cands.is_empty()) {
        return xuanfeng_resolve(g, owner, &chosen);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("xuanfeng");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("旋风：弃置 {} 的随机一张牌", g.seats[*t].name),
                );
            }
            if !chosen.is_empty() {
                b = b.option("confirm", "完成选择（开始弃牌）");
            }
            Ok(Step::Need(b.escape("不发动旋风").build()))
        }
        Some("pass") => {
            if chosen.is_empty() {
                Ok(Step::Done)
            } else {
                xuanfeng_resolve(g, owner, &chosen)
            }
        }
        Some("confirm") => xuanfeng_resolve(g, owner, &chosen),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("旋风：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("旋风：目标不合法".into());
            }
            chosen.push(t);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "target", "owner": owner, "chosen": chosen }),
            ))
        }
    }
}

fn xuanfeng_resolve(g: &mut Game, owner: usize, chosen: &[usize]) -> Result<Step, String> {
    if chosen.is_empty() {
        return Ok(Step::Done);
    }
    // 选择 1 名角色 → 该角色弃 2 张；选择 2 名 → 各弃 1 张（重复选择即同一人两张）。
    let picks: Vec<usize> = if chosen.len() == 1 {
        vec![chosen[0], chosen[0]]
    } else {
        chosen.to_vec()
    };
    let mut n = 0;
    for t in picks {
        if g.seats[t].hp <= 0 {
            continue;
        }
        if let Some(card) = discard_random(g, t, "xuanfeng") {
            n += 1;
            g.push_log(format!(
                "旋风：弃置 {} 的【{}】",
                g.seats[t].name,
                label(g, &card)
            ));
        }
    }
    let _ = owner;
    if n > 0 {
        g.push_log(format!("旋风：共弃置 {n} 张牌"));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 智迟（界陈宫，锁定）
// ---------------------------------------------------------------------------

pub const ZHICHI: SkillDef = SkillDef {
    code: "zhichi",
    name: "智迟",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhichi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(zhichi_blocker),
    yang_window: None,
    yin_window: None,
};

fn zhichi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !a.data["event"]
        .as_str()
        .unwrap_or("")
        .starts_with("DamageAfter")
    {
        return Ok(Step::Done);
    }
    if !active(g, owner, "zhichi") || g.seats[owner].hp <= 0 || g.turn == owner {
        return Ok(Step::Done);
    }
    if g.counter(&format!("turn.zhichi:{owner}")) > 0 {
        return Ok(Step::Done);
    }
    g.bump(&format!("turn.zhichi:{owner}"));
    g.push_log(format!(
        "智迟：{} 本回合【杀】与普通锦囊对其无效",
        g.seats[owner].name
    ));
    Ok(Step::Done)
}

fn zhichi_blocker(
    g: &Game,
    target: usize,
    _source: usize,
    name: &str,
    _carrier: Option<&CardId>,
) -> bool {
    if g.counter(&format!("turn.zhichi:{target}")) == 0 {
        return false;
    }
    let def = crate::cards::def(name);
    def.tags.sha || (def.kind == CardKind::Trick && !def.tags.delayed && !def.tags.wuxie)
}

// ---------------------------------------------------------------------------
// 明策（界陈宫）
// ---------------------------------------------------------------------------

pub const MINGCE: SkillDef = SkillDef {
    code: "mingce",
    name: "明策",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_mingce),
    handler: mingce,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn mingce_pool(g: &Game, seat: usize) -> Vec<CardId> {
    hand_and_equip(g, seat)
        .into_iter()
        .filter(|id| {
            let def = g.def_of(id);
            def.tags.sha || def.slot.is_some()
        })
        .collect()
}

fn can_mingce(g: &Game, seat: usize) -> bool {
    active(g, seat, "mingce")
        && play_phase(g, seat)
        && g.counter("phase.mingce_used") == 0
        && !mingce_pool(g, seat).is_empty()
        && !other_alive(g, seat).is_empty()
}

fn mingce(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let ev = a.data["event"].as_str().unwrap_or("").to_string();
    if ev.starts_with("DamageAfter") {
        // 界明策：虚拟【杀】造成伤害 → 陈宫与其各摸 1。
        let owner = a.actor;
        let watch = g.counter(&format!("turn.mingce_watch:{owner}"));
        if watch == 0 {
            return Ok(Step::Done);
        }
        let victim = a.data["seat"].as_u64().unwrap_or(u64::MAX) as usize;
        if watch != (victim + 1) as u32 {
            return Ok(Step::Done);
        }
        let helper = g
            .counter(&format!("turn.mingce_helper:{owner}"))
            .checked_sub(1)
            .map(|n| n as usize)
            .unwrap_or(owner);
        g.counters.insert(format!("turn.mingce_watch:{owner}"), 0);
        let x = g.draw_with_cause(helper, 1, Some("mingce")).len();
        let y = g.draw_with_cause(owner, 1, Some("mingce")).len();
        g.push_log(format!(
            "明策：此【杀】造成伤害，双方各摸 1 张（{x} / {y}）"
        ));
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    match stage.as_str() {
        "" => {
            let pool = mingce_pool(g, owner);
            if pool.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("mingce");
                    for id in &pool {
                        b = b.option(
                            format!("pick:{id}"),
                            format!("明策：交给一张【{}】", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动明策").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_mingce");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("明策：未知选项 {act}"))?
                        .to_string();
                    if !pool.contains(&id) {
                        return Err("明策：该牌不在手牌/装备区".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "helper", "owner": owner, "card": id }),
                    ))
                }
            }
        }
        "helper" => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            if !hand_and_equip(g, owner).contains(&card) {
                return Ok(Step::Done);
            }
            let cands = other_alive(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("mingce");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("明策：交给 {}", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消明策").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_mingce");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("明策：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("明策：目标不合法".into());
                    }
                    g.gain(&card, t)?;
                    g.bump("phase.mingce_used");
                    g.push_log(format!(
                        "明策：{} 将【{}】交给 {}",
                        g.seats[owner].name,
                        label(g, &card),
                        g.seats[t].name
                    ));
                    push(
                        g,
                        "mingce",
                        t,
                        json!({ "stage": "choice", "owner": owner, "helper": t }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "choice" => {
            let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
            if a.actor != helper || g.seats[helper].hp <= 0 {
                return Ok(Step::Done);
            }
            let has_target = other_alive(g, helper).into_iter().any(|t| {
                g.in_attack_range(helper, t) && super::can_target(g, helper, "杀", t, None)
            });
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(helper, "choice").skill("mingce");
                    if has_target {
                        b = b.option("opt1", "明策：视为对陈宫指定的角色使用【杀】");
                    }
                    b = b.option("opt2", "明策：与陈宫各摸一张牌");
                    Ok(Step::Need(b.escape("（不会出现）不选择").build()))
                }
                Some("opt1") => {
                    if !has_target {
                        return Err("明策：没有合法【杀】目标".into());
                    }
                    push(
                        g,
                        "mingce",
                        owner,
                        json!({ "stage": "victim", "owner": owner, "helper": helper }),
                    );
                    Ok(Step::Done)
                }
                Some(_) => {
                    let x = g.draw_with_cause(helper, 1, Some("mingce")).len();
                    let y = g.draw_with_cause(owner, 1, Some("mingce")).len();
                    g.push_log(format!(
                        "明策：{} 与 {} 各摸 1 张（{x} / {y}）",
                        g.seats[helper].name, g.seats[owner].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        "victim" => {
            let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
            let cands: Vec<usize> = other_alive(g, helper)
                .into_iter()
                .filter(|t| {
                    *t != owner
                        && g.in_attack_range(helper, *t)
                        && super::can_target(g, helper, "杀", *t, None)
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("mingce");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!(
                                "明策：令 {} 对 {} 使用【杀】",
                                g.seats[helper].name, g.seats[*t].name
                            ),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃（视为选择摸牌）").build()))
                }
                Some("pass") => {
                    let x = g.draw_with_cause(helper, 1, Some("mingce")).len();
                    let y = g.draw_with_cause(owner, 1, Some("mingce")).len();
                    g.push_log(format!(
                        "明策：{} 与 {} 各摸 1 张（{x} / {y}）",
                        g.seats[helper].name, g.seats[owner].name
                    ));
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("明策：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("明策：目标不合法".into());
                    }
                    g.counters
                        .insert(format!("turn.mingce_watch:{owner}"), (t + 1) as u32);
                    g.counters
                        .insert(format!("turn.mingce_helper:{owner}"), (helper + 1) as u32);
                    g.push_log(format!(
                        "明策：{} 视为对 {} 使用【杀】",
                        g.seats[helper].name, g.seats[t].name
                    ));
                    g.use_virtual(helper, "杀", vec![t])?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 将驰（界曹彰）
// ---------------------------------------------------------------------------

pub const JIANGCHI: SkillDef = SkillDef {
    code: "jiangchi",
    name: "将驰",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiangchi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 摸牌阶段结束时询问（`skills::after_draw_phase`）。
pub fn after_draw_phase(g: &mut Game, seat: usize) {
    // 将驰（界曹彰）：摸牌阶段结束时二选一。
    if active(g, seat, "jiangchi")
        && g.counter("turn.jiangchi_offered") == 0
        && !hand_and_equip(g, seat).is_empty()
    {
        g.bump("turn.jiangchi_offered");
        push(g, "jiangchi", seat, json!({ "stage": "choice" }));
    }
    // 眩惑（界法正）：摸牌阶段结束时，交给一名其他角色两张手牌。
    if active(g, seat, "xuanhuo")
        && g.counter("turn.xuanhuo_offered") == 0
        && g.hand(seat).len() >= 2
        && !other_alive(g, seat).is_empty()
    {
        g.bump("turn.xuanhuo_offered");
        push(g, "xuanhuo", seat, json!({ "stage": "offer" }));
    }
}

fn jiangchi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "choice" {
        if !active(g, owner, "jiangchi") {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiangchi");
                b = b.option(
                    "opt1",
                    "将驰①：摸一张牌，本回合【杀】次数 -1 且【杀】不计手牌上限",
                );
                b = b.option(
                    "opt2",
                    "将驰②：重铸一张牌，本回合【杀】次数 +1 且【杀】无距离限制",
                );
                Ok(Step::Need(b.escape("不发动将驰").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("opt1") => {
                let n = g.draw_with_cause(owner, 1, Some("jiangchi")).len();
                g.bump("turn.jiangchi_penalty");
                g.bump("turn.jiangchi_sha_free");
                g.push_log(format!(
                    "将驰：{} 摸 {n} 张，本回合【杀】次数 -1 且不计手牌上限",
                    g.seats[owner].name
                ));
                Ok(Step::Done)
            }
            Some("opt2") => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick", "owner": owner }),
            )),
            Some(act) => Err(format!("将驰：未知选项 {act}")),
        }
    } else {
        let pool = hand_and_equip(g, owner);
        if pool.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jiangchi");
                for id in &pool {
                    b = b.option(format!("pick:{id}"), format!("重铸【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("放弃重铸").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("将驰：未知选项 {act}"))?
                    .to_string();
                if !pool.contains(&id) {
                    return Err("将驰：该牌不在手牌/装备区".into());
                }
                g.to_discard(&id, "jiangchi")?;
                let n = g.draw_with_cause(owner, 1, Some("jiangchi")).len();
                g.bump("turn.jiangchi_bonus");
                g.bump("turn.jiangchi_no_distance");
                g.push_log(format!(
                    "将驰：{} 重铸【{}】摸 {n} 张，本回合【杀】次数 +1 且无距离限制",
                    g.seats[owner].name,
                    label(g, &id)
                ));
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 纵适（界简雍；code `zongshi_jie`，与界刘表【宗室】`zongshi` 解耦）
// ---------------------------------------------------------------------------

pub const ZONGSHI_JIE: SkillDef = SkillDef {
    code: "zongshi_jie",
    name: "纵适",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zongshi_jie,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 拼点结算后：纵适（界简雍）。
pub fn pindian_after(
    g: &mut Game,
    initiator: usize,
    target: usize,
    a: &str,
    b: &str,
    winner: Option<usize>,
) {
    for seat in [initiator, target] {
        if !active(g, seat, "zongshi_jie") || g.seats[seat].hp <= 0 {
            continue;
        }
        let (own, other) = if seat == initiator {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        };
        let want = if winner == Some(seat) {
            let ro = super::pindian_rank(g, seat, &other);
            let rs = super::pindian_rank(g, seat, &own);
            if ro < rs {
                other.clone()
            } else {
                own.clone()
            }
        } else {
            own.clone()
        };
        if !g.cards.contains_key(&want) {
            continue;
        }
        if !matches!(g.zone_of(&want), Some(Zone::Discard)) {
            continue;
        }
        push(
            g,
            "zongshi_jie",
            seat,
            json!({ "stage": "ask", "card": want }),
        );
    }
}

fn zongshi_jie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "zongshi_jie") {
        return Ok(Step::Done);
    }
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    if !matches!(g.zone_of(&card), Some(Zone::Discard)) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("zongshi_jie");
            b = b.option(
                format!("gain:{card}"),
                format!("纵适：获得拼点牌【{}】", label(g, &card)),
            );
            Ok(Step::Need(b.escape("不获得").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            if act != format!("gain:{card}") {
                return Err(format!("纵适：未知选项 {act}"));
            }
            g.gain(&card, owner)?;
            g.push_log(format!(
                "纵适：{} 获得拼点牌【{}】",
                g.seats[owner].name,
                label(g, &card)
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 巧说（界简雍）
// ---------------------------------------------------------------------------

pub const QIAOSHUO: SkillDef = SkillDef {
    code: "qiaoshuo",
    name: "巧说",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_qiaoshuo),
    handler: qiaoshuo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_qiaoshuo(g: &Game, seat: usize) -> bool {
    active(g, seat, "qiaoshuo")
        && play_phase(g, seat)
        && g.counter("turn.qiaoshuo_invalid") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat)
            .into_iter()
            .any(|t| !g.hand(t).is_empty() && !super::blocks_pindian(g, t))
}

fn qiaoshuo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "after" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let Some(res) = g.pindian_result.take() else {
            return Ok(Step::Done);
        };
        if res.a.is_empty() || res.b.is_empty() {
            // 取消拼点（视为未发生）：不判定输赢。
            return Ok(Step::Done);
        }
        if res.winner == Some(owner) {
            g.bump(&format!("turn.qiao:{owner}"));
            let n = qiao(g, owner);
            g.push_log(format!(
                "巧说：{} 拼点赢，获得 1 枚「巧」（共 {n}），下一张牌可 +1/-1 目标",
                g.seats[owner].name
            ));
        } else {
            g.counters.insert("turn.qiaoshuo_invalid".to_string(), 1);
            g.counters.insert("turn.qiaoshuo_fail".to_string(), 1);
            g.counters
                .insert("turn.qiaoshuo_fail_owner".to_string(), (owner + 1) as u32);
            g.counters.insert(format!("turn.qiao:{owner}"), 0);
            g.push_log(format!(
                "巧说：{} 拼点没赢（对 {}），本回合不能使用锦囊、巧说失效、「巧」清空",
                g.seats[owner].name, g.seats[target].name
            ));
        }
        return Ok(Step::Done);
    }
    if !active(g, owner, "qiaoshuo") {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = other_alive(g, owner)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty() && !super::blocks_pindian(g, *t))
        .collect();
    if cands.is_empty() || g.hand(owner).is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("qiaoshuo");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("巧说：与 {} 拼点", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动巧说").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("巧说：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("巧说：目标不合法".into());
            }
            g.push_log(format!(
                "巧说：{} 与 {} 拼点",
                g.seats[owner].name, g.seats[t].name
            ));
            push(
                g,
                "qiaoshuo",
                owner,
                json!({ "stage": "after", "target": t }),
            );
            crate::kernel::pindian::start(g, owner, t, "qiaoshuo");
            Ok(Step::Done)
        }
    }
}

/// 「巧」标记数量（回合内累计；回合切换由 `turn.` 前缀清空）。
pub fn qiao(g: &Game, seat: usize) -> u32 {
    g.counter(&format!("turn.qiao:{seat}"))
}

/// 巧说：本回合下一张有目标牌可多指定 1 个目标（+1；每张牌消耗 1 枚「巧」）。
pub fn qiaoshuo_extra_targets(g: &Game, seat: usize, name: &str) -> usize {
    if !play_phase(g, seat) || qiao(g, seat) == 0 {
        return 0;
    }
    let def = crate::cards::def(name);
    // 用户裁定（2026-11-10 修订）：废除延时锦囊的额外目标支持——延时锦囊不适用 ±1，
    // 使用延时锦囊也不消耗该效果（顺延到下一张可适用的牌）。
    if def.tags.delayed {
        return 0;
    }
    // 仅「目标数可被 +1」的牌：单目标【杀】/普通锦囊。
    // 借刀（固定 2 目标）与铁索（1–2 目标，另走自定义流程）不参与 +1。
    usize::from(
        def.tags.sha
            || matches!(
                def.effect,
                Effect::Duel | Effect::Dismantle | Effect::Snatch | Effect::FireAttack
            ),
    )
}

/// 巧说：该牌是否属于「巧」可修正目标的牌（非延时）。
///
/// +1：单目标【杀】/普通锦囊（决斗/拆/顺/火攻）；-1：群体锦囊（南蛮/万箭/桃园/五谷）。
/// 借刀（固定 2 目标）与铁索（1–2 目标，另走自定义流程）不参与，因此**不消耗**「巧」。
fn qiaoshuo_eligible(name: &str) -> bool {
    let def = crate::cards::def(name);
    def.tags.sha
        || matches!(
            def.effect,
            Effect::Duel
                | Effect::Dismantle
                | Effect::Snatch
                | Effect::FireAttack
                | Effect::SavageAssault
                | Effect::ArcheryAttack
                | Effect::PeachGarden
                | Effect::Harvest
        )
}

/// 一张牌被使用（内核 `start_use` 调用）：每出一张符合条件的**手牌**消耗 1 枚「巧」。
pub fn on_card_use(g: &mut Game, seat: usize, name: &str, has_carrier: bool) {
    if !has_carrier || !active(g, seat, "qiaoshuo") {
        return;
    }
    let n = qiao(g, seat);
    if n == 0 || !qiaoshuo_eligible(name) {
        return;
    }
    g.counters.insert(format!("turn.qiao:{seat}"), n - 1);
    g.push_log(format!(
        "巧说：消耗 1 枚「巧」（剩 {}）",
        n.saturating_sub(1)
    ));
}

/// 巧说：能否对群体锦囊「少指定一个目标」（-1）。
pub fn can_trim_targets(g: &Game, seat: usize) -> bool {
    play_phase(g, seat) && active(g, seat, "qiaoshuo") && qiao(g, seat) > 0
}

/// 巧说没赢：本回合不能使用锦囊牌（含延时锦囊）。
pub fn trick_blocked(g: &Game, seat: usize) -> bool {
    g.counter("turn.qiaoshuo_fail") > 0
        && g.counter("turn.qiaoshuo_fail_owner") == (seat + 1) as u32
}

// ---------------------------------------------------------------------------
// 纵玄（界虞翻）
// ---------------------------------------------------------------------------

pub const ZONGXUAN: SkillDef = SkillDef {
    code: "zongxuan",
    name: "纵玄",
    triggers: &[EventKind::CardDiscarded],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zongxuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zongxuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("CardDiscarded")
        {
            return Ok(Step::Done);
        }
        if !active(g, owner, "zongxuan") || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        let Some(discarder) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        let branch = if discarder == owner {
            "own"
        } else if upper_seat_alive(g, owner) == Some(discarder) {
            if g.counter(&format!("turn.zongxuan_upper:{owner}")) > 0 {
                return Ok(Step::Done);
            }
            "upper"
        } else {
            return Ok(Step::Done);
        };
        let key = format!("zongxuan:{branch}");
        if !g.action_seen.insert(key) {
            return Ok(Step::Done);
        }
        if branch == "upper" {
            g.bump(&format!("turn.zongxuan_upper:{owner}"));
        }
        let cards: Vec<CardId> = g
            .discard_batch
            .get(&discarder)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| matches!(g.zone_of(id), Some(Zone::Discard)))
            .collect();
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "owner": owner, "cards": cards, "sel": [] }),
        ));
    }
    let cards: Vec<String> = a.data["cards"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let avail: Vec<String> = cards
        .into_iter()
        .filter(|id| matches!(g.zone_of(id), Some(Zone::Discard)))
        .collect();
    if avail.is_empty() {
        return Ok(Step::Done);
    }
    let mut sel: Vec<String> = a.data["sel"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("zongxuan");
            for id in &avail {
                let mark = if sel.contains(id) { "✔ " } else { "" };
                b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
            }
            if !sel.is_empty() {
                b = b.option("confirm", format!("确认置于牌堆顶（{} 张）", sel.len()));
            }
            Ok(Step::Need(b.escape("不发动纵玄").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            let mut n = 0;
            for id in &sel {
                if matches!(g.zone_of(id), Some(Zone::Discard)) {
                    g.put_on_top(id)?;
                    n += 1;
                }
            }
            if n > 0 {
                g.push_log(format!(
                    "纵玄：{} 将 {n} 张牌置于牌堆顶",
                    g.seats[owner].name
                ));
            }
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("纵玄：未知选项 {act}"))?
                .to_string();
            if !avail.contains(&id) {
                return Err("纵玄：该牌不在本次弃置中".into());
            }
            if let Some(pos) = sel.iter().position(|c| *c == id) {
                sel.remove(pos);
            } else {
                sel.push(id);
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "pick", "owner": owner, "cards": avail, "sel": sel }),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 直言（界虞翻）
// ---------------------------------------------------------------------------

pub const ZHIYAN: SkillDef = SkillDef {
    code: "zhiyan",
    name: "直言",
    triggers: &[EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhiyan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhiyan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "zhiyan") || g.seats[owner].hp <= 0 {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("PhaseStart")
            || g.phase != Phase::End
        {
            return Ok(Step::Done);
        }
        let Some(turn_seat) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        if turn_seat == owner {
            if g.counter(&format!("turn.zhiyan_own:{owner}")) > 0 {
                return Ok(Step::Done);
            }
            g.bump(&format!("turn.zhiyan_own:{owner}"));
        } else if upper_seat_alive(g, owner) == Some(turn_seat) {
            if g.counter(&format!("turn.zhiyan_upper:{owner}")) > 0 {
                return Ok(Step::Done);
            }
            g.bump(&format!("turn.zhiyan_upper:{owner}"));
        } else {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "target", "owner": owner }),
        ));
    }
    let cands = alive(g);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("zhiyan");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("直言：令 {} 摸一张并展示", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动直言").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("直言：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("直言：目标不合法".into());
            }
            let drawn = g.draw_with_cause(t, 1, Some("zhiyan"));
            let Some(card) = drawn.first().cloned() else {
                g.push_log("直言：没有摸到牌".to_string());
                return Ok(Step::Done);
            };
            g.reveal(&card, Some(t), "zhiyan");
            let def = g.def_of(&card);
            if def.slot.is_some() {
                let slot = def.slot.unwrap_or(Slot::Weapon);
                if let Some(old) = g.equip_in(t, slot) {
                    g.to_discard(&old, "zhiyan")?;
                    super::on_equip_lost(g, t);
                }
                if g.to_equip(&card, t, slot).is_err() {
                    g.to_discard(&card, "zhiyan")?;
                }
                if g.seats[t].hp < g.seats[t].max_hp {
                    flow::heal(g, t, 1, "zhiyan");
                }
                g.push_log(format!(
                    "直言：{} 使用装备【{}】并回复 1 点体力",
                    g.seats[t].name,
                    label(g, &card)
                ));
            } else if g.seats[t].hp != g.seats[owner].hp {
                g.push_log(format!(
                    "直言：{} 展示非装备牌【{}】，体力值不等于 {}，失去 1 点体力",
                    g.seats[t].name,
                    label(g, &card),
                    g.seats[owner].name
                ));
                flow::lose_hp(g, t, 1, "zhiyan");
            } else {
                g.push_log(format!(
                    "直言：{} 展示【{}】",
                    g.seats[t].name,
                    label(g, &card)
                ));
            }
            g.conceal(&card, "zhiyan");
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 陷阵（界高顺）
// ---------------------------------------------------------------------------

pub const XIANZHEN: SkillDef = SkillDef {
    code: "xianzhen",
    name: "陷阵",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_xianzhen),
    handler: xianzhen,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_xianzhen(g: &Game, seat: usize) -> bool {
    active(g, seat, "xianzhen")
        && play_phase(g, seat)
        && g.counter("phase.xianzhen_used") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat)
            .into_iter()
            .any(|t| !g.hand(t).is_empty() && !super::blocks_pindian(g, t))
}

fn xianzhen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage == "after" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let Some(res) = g.pindian_result.take() else {
            return Ok(Step::Done);
        };
        if res.a.is_empty() || res.b.is_empty() {
            return Ok(Step::Done);
        }
        if res.winner == Some(owner) {
            g.counters.insert("turn.xianzhen_win".to_string(), 1);
            g.counters
                .insert("turn.xianzhen_target".to_string(), (target + 1) as u32);
            g.push_log(format!(
                "陷阵：{} 拼点赢，本回合对 {} 无距离/次数限制并无视其防具",
                g.seats[owner].name, g.seats[target].name
            ));
        } else {
            g.counters.insert("turn.xianzhen_lose".to_string(), 1);
            g.counters
                .insert("turn.xianzhen_lose_target".to_string(), (target + 1) as u32);
            g.push_log(format!(
                "陷阵：{} 拼点没赢，本回合不能对 {} 使用【杀】且【杀】不计手牌上限",
                g.seats[owner].name, g.seats[target].name
            ));
        }
        return Ok(Step::Done);
    }
    if !can_xianzhen(g, owner) {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = other_alive(g, owner)
        .into_iter()
        .filter(|t| !g.hand(*t).is_empty() && !super::blocks_pindian(g, *t))
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("xianzhen");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("陷阵：与 {} 拼点", g.seats[*t].name),
                );
            }
            Ok(Step::Need(b.escape("不发动陷阵").build()))
        }
        Some("pass") => {
            g.bump("phase.cancel_xianzhen");
            Ok(Step::Done)
        }
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("陷阵：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("陷阵：目标不合法".into());
            }
            g.bump("phase.xianzhen_used");
            g.push_log(format!(
                "陷阵：{} 与 {} 拼点",
                g.seats[owner].name, g.seats[t].name
            ));
            push(
                g,
                "xianzhen",
                owner,
                json!({ "stage": "after", "target": t }),
            );
            crate::kernel::pindian::start(g, owner, t, "xianzhen");
            Ok(Step::Done)
        }
    }
}

pub fn xianzhen_win_target(g: &Game) -> Option<usize> {
    if g.counter("turn.xianzhen_win") == 0 {
        return None;
    }
    g.counter("turn.xianzhen_target")
        .checked_sub(1)
        .map(|n| n as usize)
}

pub fn xianzhen_lose_target(g: &Game) -> Option<usize> {
    if g.counter("turn.xianzhen_lose") == 0 {
        return None;
    }
    g.counter("turn.xianzhen_lose_target")
        .checked_sub(1)
        .map(|n| n as usize)
}

/// 陷阵（赢）：对该角色使用牌无距离限制。
pub fn target_no_distance(g: &Game, from: usize, to: usize) -> bool {
    active(g, from, "xianzhen") && xianzhen_win_target(g) == Some(to)
}

/// 陷阵（赢）：无视该角色防具。
pub fn armor_ignored(g: &Game, source: usize, victim: usize) -> bool {
    active(g, source, "xianzhen") && xianzhen_win_target(g) == Some(victim)
}

/// 陷阵（赢）：对其使用【杀】不受次数上限。
pub fn sha_limit_exempt(g: &Game, seat: usize, targets: &[usize]) -> bool {
    active(g, seat, "xianzhen") && xianzhen_win_target(g).is_some_and(|t| targets.contains(&t))
}

/// 陷阵（赢）：【杀】/普通锦囊可多指定其为目标（+1）。
pub fn xianzhen_extra_targets(g: &Game, seat: usize, name: &str) -> usize {
    if !play_phase(g, seat) || xianzhen_win_target(g).is_none() {
        return 0;
    }
    let def = crate::cards::def(name);
    let eligible = def.tags.sha || (def.kind == CardKind::Trick && !def.tags.delayed);
    usize::from(eligible)
}

/// 陷阵（没赢）：不能对其使用【杀】。
pub fn sha_blocked_for(g: &Game, seat: usize, target: usize) -> bool {
    active(g, seat, "xianzhen") && xianzhen_lose_target(g) == Some(target)
}

// ---------------------------------------------------------------------------
// 禁酒（界高顺，锁定）
// ---------------------------------------------------------------------------

pub const JINJIU: SkillDef = SkillDef {
    code: "jinjiu",
    name: "禁酒",
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

/// 禁酒：其他角色不能在高顺的回合使用【酒】；高顺自己不能把【酒】当【酒】。
pub fn jiu_blocked(g: &Game, user: usize) -> bool {
    if active(g, user, "jinjiu") {
        return true;
    }
    (0..g.seats.len())
        .any(|i| i != user && g.turn == i && g.seats[i].hp > 0 && active(g, i, "jinjiu"))
}

/// 禁酒：视为【杀】的【酒】有效点数为 K。
pub fn effective_rank(g: &Game, seat: usize, card: &CardId) -> Option<u8> {
    if !active(g, seat, "jinjiu") {
        return None;
    }
    if physical_effect(g, card) == Effect::Jiu {
        Some(13)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 眩惑（界法正）
// ---------------------------------------------------------------------------

pub const XUANHUO: SkillDef = SkillDef {
    code: "xuanhuo",
    name: "眩惑",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: xuanhuo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn xuanhuo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() || stage == "offer" {
        if !active(g, owner, "xuanhuo") || g.hand(owner).len() < 2 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("xuanhuo");
                b = b.option("opt1", "眩惑：交给一名其他角色两张手牌");
                Ok(Step::Need(b.escape("不发动眩惑").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "cards", "owner": owner, "sel": [] }),
            )),
        }
    } else if stage == "cards" {
        let mut sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let hand = g.hand(owner);
        if hand.len() < 2 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("xuanhuo");
                for id in &hand {
                    let mark = if sel.contains(id) { "✔ " } else { "" };
                    b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
                }
                if sel.len() == 2 {
                    b = b.option("confirm", "确认（交给两张）");
                }
                Ok(Step::Need(b.escape("取消眩惑").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("confirm") => {
                if sel.len() != 2 {
                    return Err("眩惑：需要选择两张手牌".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "helper", "owner": owner, "cards": sel }),
                ))
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("眩惑：未知选项 {act}"))?
                    .to_string();
                if !hand.contains(&id) {
                    return Err("眩惑：该牌不在手牌".into());
                }
                if let Some(pos) = sel.iter().position(|c| *c == id) {
                    sel.remove(pos);
                } else if sel.len() < 2 {
                    sel.push(id);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "cards", "owner": owner, "sel": sel }),
                ))
            }
        }
    } else if stage == "helper" {
        let cards: Vec<String> = a.data["cards"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if cards.len() != 2 || !cards.iter().all(|c| g.hand(owner).contains(c)) {
            return Ok(Step::Done);
        }
        let cands = other_alive(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("xuanhuo");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("眩惑：交给 {}", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("取消眩惑").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("眩惑：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("眩惑：目标不合法".into());
                }
                for id in &cards {
                    g.gain(id, t)?;
                }
                g.push_log(format!(
                    "眩惑：{} 交给 {} 两张手牌",
                    g.seats[owner].name, g.seats[t].name
                ));
                push(
                    g,
                    "xuanhuo",
                    t,
                    json!({ "stage": "choice", "owner": owner, "helper": t }),
                );
                Ok(Step::Done)
            }
        }
    } else if stage == "choice" {
        let helper = a.actor;
        let has_target = other_alive(g, helper)
            .into_iter()
            .any(|t| g.in_attack_range(helper, t) && super::can_target(g, helper, "杀", t, None));
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(helper, "choice").skill("xuanhuo");
                if has_target {
                    b = b.option("opt1", "眩惑：对法正指定的角色使用【杀】");
                }
                b = b.option("opt2", "眩惑：令法正观看你的手牌并获得其中两张");
                Ok(Step::Need(b.escape("（不会出现）不选择").build()))
            }
            Some("opt1") => {
                if !has_target {
                    return Err("眩惑：没有合法【杀】目标".into());
                }
                push(
                    g,
                    "xuanhuo",
                    owner,
                    json!({ "stage": "victim", "owner": owner, "helper": helper }),
                );
                Ok(Step::Done)
            }
            Some(_) => {
                let hand = g.hand(helper);
                for id in &hand {
                    g.reveal_private(owner, id, "xuanhuo");
                }
                g.push_log(format!(
                    "眩惑：{} 观看 {} 的手牌（{} 张）",
                    g.seats[owner].name,
                    g.seats[helper].name,
                    hand.len()
                ));
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "gain", "owner": owner, "helper": helper, "sel": [] }),
                ))
            }
        }
    } else if stage == "victim" {
        let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
        let cands: Vec<usize> = other_alive(g, helper)
            .into_iter()
            .filter(|t| {
                g.in_attack_range(helper, *t) && super::can_target(g, helper, "杀", *t, None)
            })
            .collect();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("xuanhuo");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!(
                            "眩惑：令 {} 对 {} 使用【杀】",
                            g.seats[helper].name, g.seats[*t].name
                        ),
                    );
                }
                Ok(Step::Need(b.escape("取消（视为不执行）").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("眩惑：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("眩惑：目标不合法".into());
                }
                g.push_log(format!(
                    "眩惑：{} 视为对 {} 使用【杀】",
                    g.seats[helper].name, g.seats[t].name
                ));
                g.use_virtual(helper, "杀", vec![t])?;
                Ok(Step::Done)
            }
        }
    } else {
        // 观看并获得其两张手牌。
        let helper = a.data["helper"].as_u64().unwrap_or(0) as usize;
        let mut sel: Vec<String> = a.data["sel"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let hand = g.hand(helper);
        if hand.is_empty() {
            g.clear_private_reveals(owner);
            g.push_log("眩惑：协助者没有手牌".to_string());
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("xuanhuo");
                for id in &hand {
                    let mark = if sel.contains(id) { "✔ " } else { "" };
                    b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
                }
                if sel.len() == 2 {
                    b = b.option("confirm", "确认获得这两张手牌");
                }
                Ok(Step::Need(b.escape_with("pass", "放弃获得").build()))
            }
            Some("pass") => {
                g.clear_private_reveals(owner);
                Ok(Step::Done)
            }
            Some("confirm") => {
                if sel.len() != 2 {
                    return Err("眩惑：需要选择两张手牌".into());
                }
                for id in &sel {
                    if g.hand(helper).contains(id) {
                        g.gain(id, owner)?;
                    }
                }
                g.clear_private_reveals(owner);
                g.push_log(format!(
                    "眩惑：{} 获得 {} 两张手牌",
                    g.seats[owner].name, g.seats[helper].name
                ));
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("眩惑：未知选项 {act}"))?
                    .to_string();
                if !hand.contains(&id) {
                    return Err("眩惑：该牌不在其手牌".into());
                }
                if let Some(pos) = sel.iter().position(|c| *c == id) {
                    sel.remove(pos);
                } else if sel.len() < 2 {
                    sel.push(id);
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "gain", "owner": owner, "helper": helper, "sel": sel }),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 恩怨（界法正）
// ---------------------------------------------------------------------------

pub const ENYUAN: SkillDef = SkillDef {
    code: "enyuan",
    name: "恩怨",
    triggers: &[EventKind::DamageAfter, EventKind::CardGained],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: enyuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn enyuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(a.actor as u64) as usize;
    let ev = a.data["event"].as_str().unwrap_or("").to_string();
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if ev.starts_with("CardGained") && stage.is_empty() {
        if !active(g, owner, "enyuan") || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        // 单次获得「一名其他角色」≥2 张（按动作批次）。
        let mut from = None;
        for (src, to, _) in g.gain_batch.iter() {
            if *to != owner || *src == owner {
                continue;
            }
            let n = g
                .gain_batch
                .iter()
                .filter(|(s2, t2, _)| s2 == src && t2 == to)
                .count();
            if n >= 2 {
                from = Some(*src);
                break;
            }
        }
        let Some(from) = from else {
            return Ok(Step::Done);
        };
        if !g.action_seen.insert(format!("enyuan:gain:{from}")) {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "gain", "owner": owner, "from": from }),
        ));
    }
    if ev.starts_with("DamageAfter") && stage.is_empty() {
        if !active(g, owner, "enyuan") || g.seats[owner].hp <= 0 {
            return Ok(Step::Done);
        }
        let victim = a.data["seat"].as_u64().unwrap_or(u64::MAX) as usize;
        if victim != owner {
            return Ok(Step::Done);
        }
        let Some(src) = a.data["cause"]
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return Ok(Step::Done);
        };
        if src == owner || g.seats[src].hp <= 0 {
            return Ok(Step::Done);
        }
        let amount = a.data["amount"].as_i64().unwrap_or(1).max(1) as u32;
        if !g.action_seen.insert(format!("enyuan:dmg:{src}")) {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "owner": owner, "source": src, "left": amount }),
        ));
    }
    if stage == "gain" {
        let from = a.data["from"].as_u64().unwrap_or(0) as usize;
        if !active(g, owner, "enyuan") || g.seats[from].hp <= 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("enyuan");
                b = b.option("opt1", format!("恩怨：令 {} 摸一张牌", g.seats[from].name));
                Ok(Step::Need(b.escape("不发动恩怨").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let n = g.draw_with_cause(from, 1, Some("enyuan")).len();
                g.push_log(format!(
                    "恩怨：{} 因你获得其牌摸 {n} 张",
                    g.seats[from].name
                ));
                Ok(Step::Done)
            }
        }
    } else if stage == "ask" {
        let src = a.data["source"].as_u64().unwrap_or(0) as usize;
        let left = a.data["left"].as_u64().unwrap_or(0) as u32;
        if g.seats[src].hp <= 0 {
            return Ok(Step::Done);
        }
        let red: Vec<CardId> = g.hand(src).into_iter().filter(|id| g.is_red(id)).collect();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(src, "choice").skill("enyuan");
                if !red.is_empty() {
                    b = b.option(
                        "opt1",
                        format!("恩怨：交给 {} 一张红色手牌", g.seats[owner].name),
                    );
                }
                b = b.option("opt2", "恩怨：失去 1 点体力");
                Ok(Step::Need(b.escape("（不会出现）不选择").build()))
            }
            Some("opt1") => {
                if red.is_empty() {
                    return Err("恩怨：没有红色手牌".into());
                }
                push(
                    g,
                    "enyuan",
                    owner,
                    json!({ "stage": "pick", "owner": owner, "source": src, "left": left }),
                );
                Ok(Step::Done)
            }
            Some("opt2") => {
                g.push_log(format!("恩怨：{} 失去 1 点体力", g.seats[src].name));
                flow::lose_hp(g, src, 1, "enyuan");
                enyuan_next(g, owner, src, left)
            }
            Some(act) => Err(format!("恩怨：未知选项 {act}")),
        }
    } else {
        // 交红色手牌（由来源选择）。
        let src = a.data["source"].as_u64().unwrap_or(0) as usize;
        let left = a.data["left"].as_u64().unwrap_or(0) as u32;
        let red: Vec<CardId> = g.hand(src).into_iter().filter(|id| g.is_red(id)).collect();
        if red.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(src, "choice").skill("enyuan");
                for id in &red {
                    b = b.option(
                        format!("pick:{id}"),
                        format!("交给 {}：{}", g.seats[owner].name, label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("改为失去 1 点体力").build()))
            }
            Some("pass") => {
                g.push_log(format!("恩怨：{} 失去 1 点体力", g.seats[src].name));
                flow::lose_hp(g, src, 1, "enyuan");
                enyuan_next(g, owner, src, left)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("恩怨：未知选项 {act}"))?
                    .to_string();
                if !red.contains(&id) {
                    return Err("恩怨：该牌不是红色手牌".into());
                }
                g.gain(&id, owner)?;
                g.push_log(format!(
                    "恩怨：{} 交给 {} 一张红色手牌【{}】",
                    g.seats[src].name,
                    g.seats[owner].name,
                    label(g, &id)
                ));
                enyuan_next(g, owner, src, left)
            }
        }
    }
}

fn enyuan_next(g: &mut Game, owner: usize, source: usize, left: u32) -> Result<Step, String> {
    if left > 1 && g.seats[source].hp > 0 && g.seats[owner].hp > 0 {
        push(
            g,
            "enyuan",
            owner,
            json!({ "stage": "ask", "owner": owner, "source": source, "left": left - 1 }),
        );
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 疠火（界程普）
// ---------------------------------------------------------------------------

pub const LIHUO: SkillDef = SkillDef {
    code: "lihuo",
    name: "疠火",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: lihuo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 转化牌被使用（`lib.rs` 的 `convert:` 入口调用）。疠火记录「此牌是疠火火杀」。
pub fn on_conversion_used(g: &mut Game, seat: usize, skill: &str, card: &CardId, name: &str) {
    if skill == "lihuo" && name == "火杀" {
        g.counters.insert(format!("turn.lihuo:{card}"), 1);
        g.counters
            .insert(format!("turn.lihuo_owner:{card}"), (seat + 1) as u32);
    }
}

fn lihuo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"]
            .as_str()
            .unwrap_or("")
            .starts_with("DamageAfter")
        {
            return Ok(Step::Done);
        }
        if !active(g, owner, "lihuo") {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(str::to_string) else {
            return Ok(Step::Done);
        };
        if g.counter(&format!("turn.lihuo:{card}")) > 0 {
            g.counters.insert(format!("turn.lihuo_dmg:{card}"), 1);
        }
        return Ok(Step::Done);
    }
    // 代价：弃置一张牌（手牌/装备区）或失去 1 点体力。
    if stage == "cost" {
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        let pool = hand_and_equip(g, owner);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("lihuo");
                if !pool.is_empty() {
                    b = b.option("opt1", "疠火：弃置一张牌");
                }
                b = b.option("opt2", "疠火：失去 1 点体力");
                Ok(Step::Need(b.escape("（不会出现）不选择").build()))
            }
            Some("opt1") => {
                if pool.is_empty() {
                    return Err("疠火：没有可弃置的牌".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "card": card }),
                ))
            }
            Some(_) => {
                g.push_log(format!("疠火：{} 失去 1 点体力", g.seats[owner].name));
                flow::lose_hp(g, owner, 1, "lihuo");
                Ok(Step::Done)
            }
        }
    } else {
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        let pool = hand_and_equip(g, owner);
        if pool.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("lihuo");
                for id in &pool {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("改为失去 1 点体力").build()))
            }
            Some("pass") => {
                g.push_log(format!("疠火：{} 失去 1 点体力", g.seats[owner].name));
                flow::lose_hp(g, owner, 1, "lihuo");
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("疠火：未知选项 {act}"))?
                    .to_string();
                if !pool.contains(&id) {
                    return Err("疠火：该牌不在手牌/装备区".into());
                }
                g.to_discard(&id, "lihuo")?;
                g.push_log(format!(
                    "疠火：弃置【{}】作为代价（{}）",
                    label(g, &id),
                    label(g, &card)
                ));
                Ok(Step::Done)
            }
        }
    }
}

/// 一张牌结算结束（内核 `sha_effect` 调用）：疠火代价。
pub fn on_card_finished(g: &mut Game, source: usize, card: &CardId) {
    if g.counter(&format!("turn.lihuo:{card}")) == 0 {
        return;
    }
    let dmg = g.counter(&format!("turn.lihuo_dmg:{card}")) > 0
        || g.counter(&format!("turn.damage_card:{card}")) > 0;
    let owner = g
        .counter(&format!("turn.lihuo_owner:{card}"))
        .checked_sub(1)
        .map(|n| n as usize)
        .unwrap_or(source);
    g.counters.insert(format!("turn.lihuo:{card}"), 0);
    g.counters.insert(format!("turn.lihuo_dmg:{card}"), 0);
    if dmg && g.seats.get(owner).is_some_and(|s| s.hp > 0) {
        push(g, "lihuo", owner, json!({ "stage": "cost", "card": card }));
    }
}

/// 疠火：火【杀】可多指定一个目标（不因此获得无距离）。
pub fn lihuo_extra_targets(g: &Game, seat: usize, name: &str) -> usize {
    if play_phase(g, seat) && active(g, seat, "lihuo") && name == "火杀" {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 醇醪（界程普）
// ---------------------------------------------------------------------------

pub const CHUNLAO: SkillDef = SkillDef {
    code: "chunlao",
    name: "醇醪",
    triggers: &[EventKind::CardDiscarded, EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: chunlao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn chun_count(g: &Game, seat: usize) -> usize {
    g.mark_cards(seat, "chun").len()
}

/// 本轮以此法使用【酒】的次数（每轮清零）。
fn chunlao_x(g: &Game) -> u32 {
    if g.counter("chunlao_round") != g.round {
        0
    } else {
        g.counter("chunlao_x")
    }
}

fn chunlao_next_x(g: &mut Game) -> u32 {
    if g.counter("chunlao_round") != g.round {
        g.counters.insert("chunlao_round".to_string(), g.round);
        g.counters.insert("chunlao_x".to_string(), 0);
    }
    chunlao_x(g) + 1
}

fn chunlao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "chunlao") {
        return Ok(Step::Done);
    }
    let ev = a.data["event"].as_str().unwrap_or("").to_string();
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if ev.starts_with("CardDiscarded") {
            let Some(card) = a.data["card"].as_str().map(str::to_string) else {
                return Ok(Step::Done);
            };
            let Some(discarder) = a.data["seat"].as_u64().map(|n| n as usize) else {
                return Ok(Step::Done);
            };
            if !is_sha(g, &card) || !matches!(g.zone_of(&card), Some(Zone::Discard)) {
                return Ok(Step::Done);
            }
            if chun_count(g, owner) >= 9 {
                return Ok(Step::Done);
            }
            if !(owner == discarder || adjacent_alive(g, owner, discarder)) {
                return Ok(Step::Done);
            }
            g.to_mark(&card, owner, "chun")?;
            g.push_log(format!(
                "醇醪：将 {} 的【{}】置于武将牌上为「醇」（{}）",
                g.seats[discarder].name,
                label(g, &card),
                chun_count(g, owner)
            ));
            return Ok(Step::Done);
        }
        if ev.starts_with("PhaseStart") && g.turn == owner && g.phase == Phase::End {
            if chun_count(g, owner) > 0 || sha_in_hand(g, owner).is_empty() {
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "select", "sel": [] }),
            ));
        }
        return Ok(Step::Done);
    }
    // 结束阶段：将任意张手牌【杀】置于武将牌上。
    let avail: Vec<CardId> = sha_in_hand(g, owner);
    if avail.is_empty() || chun_count(g, owner) >= 9 {
        return Ok(Step::Done);
    }
    let mut sel: Vec<String> = a.data["sel"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("chunlao");
            for id in &avail {
                let mark = if sel.contains(id) { "✔ " } else { "" };
                b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
            }
            if !sel.is_empty() {
                b = b.option("confirm", format!("确认（{} 张）", sel.len()));
            }
            Ok(Step::Need(b.escape("不发动醇醪").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            let mut n = 0;
            for id in &sel {
                if chun_count(g, owner) >= 9 {
                    break;
                }
                if g.hand(owner).contains(id) {
                    g.to_mark(id, owner, "chun")?;
                    n += 1;
                }
            }
            if n > 0 {
                g.push_log(format!(
                    "醇醪：{} 将 {n} 张【杀】置于武将牌上（共 {}）",
                    g.seats[owner].name,
                    chun_count(g, owner)
                ));
            }
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("醇醪：未知选项 {act}"))?
                .to_string();
            if !avail.contains(&id) {
                return Err("醇醪：该牌不是手牌【杀】".into());
            }
            if let Some(pos) = sel.iter().position(|c| *c == id) {
                sel.remove(pos);
            } else {
                sel.push(id);
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "select", "sel": sel }),
            ))
        }
    }
}

/// 濒死窗口：醇醪（程普）可移去 X 张「醇」视为使用【酒】。
pub fn dying_extra_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !active(g, seat, "chunlao") {
        return Vec::new();
    }
    let need = chunlao_x(g) + 1;
    if chun_count(g, seat) < need as usize {
        return Vec::new();
    }
    vec![(
        "chunlao".to_string(),
        format!("醇醪：移去 {need} 张「醇」视为使用【酒】（回复 1）"),
    )]
}

pub fn dying_extra_use(
    g: &mut Game,
    seat: usize,
    victim: usize,
    act: &str,
) -> Result<bool, String> {
    if act != "chunlao" {
        return Ok(false);
    }
    if !active(g, seat, "chunlao") {
        return Err("醇醪：没有该技能".into());
    }
    if jiu_blocked(g, victim) {
        return Err("醇醪：禁酒生效中".into());
    }
    let need = chunlao_next_x(g);
    if chun_count(g, seat) < need as usize {
        return Err("醇醪：「醇」不足".into());
    }
    let mut removed = 0;
    for id in g.mark_cards(seat, "chun").into_iter().take(need as usize) {
        g.to_discard(&id, "chunlao")?;
        removed += 1;
    }
    g.seats[victim].hp += 1;
    g.counters.insert("chunlao_x".to_string(), need);
    g.push_log(format!(
        "醇醪：{} 移去 {removed} 张「醇」，{} 视为使用【酒】（体力 {}）",
        g.seats[seat].name, g.seats[victim].name, g.seats[victim].hp
    ));
    Ok(true)
}

// ---------------------------------------------------------------------------
// 转化 / 数值钩子
// ---------------------------------------------------------------------------

/// 禁酒：手牌【酒】视为【杀】。
pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    let mut out = Vec::new();
    if active(g, seat, "jinjiu") {
        for id in g.hand(seat) {
            if physical_effect(g, &id) == Effect::Jiu {
                out.push(super::Conversion {
                    skill: "jinjiu",
                    from: id,
                    to: "杀",
                    usable: true,
                    playable: true,
                });
            }
        }
    }
    // 疠火：非火【杀】可改为火【杀】使用。
    if active(g, seat, "lihuo") {
        for id in g.hand(seat) {
            let def = g.def_of(&id);
            if def.tags.sha && def.damage_kind != DamageKind::Fire {
                out.push(super::Conversion {
                    skill: "lihuo",
                    from: id,
                    to: "火杀",
                    usable: true,
                    playable: false,
                });
            }
        }
    }
    out
}

/// 手牌上限修正（将驰① / 陷阵没赢：手牌【杀】不计上限）。
pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    let free = g.counter("turn.jiangchi_sha_free") > 0
        || (active(g, seat, "xianzhen") && xianzhen_lose_target(g).is_some());
    if free {
        sha_in_hand(g, seat).len() as i32
    } else {
        0
    }
}

/// 【杀】次数修正（将驰①：-1）。
pub fn sha_limit_penalty(g: &Game, _seat: usize) -> usize {
    g.counter("turn.jiangchi_penalty") as usize
}

/// 【杀】次数加成（将驰②：+1）。
pub fn sha_limit_bonus(g: &Game, seat: usize) -> usize {
    if active(g, seat, "jiangchi") && g.counter("turn.jiangchi_bonus") > 0 {
        1
    } else {
        0
    }
}

/// 【杀】无距离限制（将驰②）。
pub fn sha_no_distance(g: &Game, seat: usize, card: &CardId) -> bool {
    active(g, seat, "jiangchi")
        && g.counter("turn.jiangchi_no_distance") > 0
        && g.def_of(card).tags.sha
}

pub const B17_SKILLS: &[SkillDef] = &[
    XUANFENG,
    ZHICHI,
    MINGCE,
    JIANGCHI,
    ZONGSHI_JIE,
    QIAOSHUO,
    ZONGXUAN,
    ZHIYAN,
    XIANZHEN,
    JINJIU,
    XUANHUO,
    ENYUAN,
    LIHUO,
    CHUNLAO,
];
