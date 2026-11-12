//! B9（界刘备 / 界关羽 / 界张飞 / 界赵云 / 界马超 / 界诸葛亮 / 界黄月英）——按 `align/B9.json` 第 7 段实现。
//!
//! **用户口径（2026-11-09）**：标将已基本废弃，界将技能按 v1 实现即可；本批只做界将技能，
//! 不为「标/界共存」保留 `is_jiexian` 分支——同一 code 只实现界版语义。
//!
//! 本批新增的跨批能力：
//! - **被动转化技**（武圣 / 龙胆）：`legal_uses` 追加 `convert:<skill>:<承载牌>:<生效牌名>`，
//!   响应窗口追加 `vrespond:<skill>:<承载牌>`；内核不含技能名。
//! - **主公技接力**（激将）：响应窗口选项 `jijiang` → 按座位顺序询问同势力角色代出【杀】；
//!   结果经 `Game.relay_played` / `relay_none` 回传响应流程。
//! - **非锁定技失效**（义绝·黑 / 铁骑）：`skills::active` 统一闸门 + `LOCKED_SKILLS` 清单；
//!   手牌封印走 `flags["hand_sealed"]` + `Game::can_use_card`。
//! - **距离/次数/无距离** 三个数值钩子（马术 / 咆哮 / 界武圣·方片杀 / 奇才）。

use serde_json::json;

use crate::cards::{self, CardKind, ResponseKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, builder, other_alive, SkillDef, SkillKind};

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}

fn phase_cause(a: &Ask) -> String {
    a.data["cause"].as_str().unwrap_or_default().to_string()
}

/// 牌所在区域的展示标签（涯角选牌用）。
fn zone_label(g: &Game, id: &CardId, owner: usize) -> &'static str {
    match g.zone_of(id) {
        Some(Zone::Judge(_)) => "判定区",
        Some(Zone::Equip(s, _)) if *s == owner => "装备区",
        _ => "手牌",
    }
}

fn type_group(g: &Game, id: &CardId) -> u8 {
    match g.def_of(id).kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 3,
    }
}

// ---------------------------------------------------------------------------
// 仁德（界刘备）
// ---------------------------------------------------------------------------

pub const RENDE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "rende",
    name: "仁德",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_rende),
    handler: rende,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn rende_targets(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|i| g.counter(&format!("turn.rende_target:{i}")) == 0)
        .collect()
}

fn can_rende(g: &Game, seat: usize) -> bool {
    active(g, seat, "rende")
        && play_phase(g, seat)
        && !g.hand(seat).is_empty()
        && !rende_targets(g, seat).is_empty()
}

fn rende(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("select").to_string();
    match step.as_str() {
        "select" => {
            let mut selected: Vec<CardId> = a.data["selected"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            selected.retain(|id| g.hand(a.actor).contains(id));
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choose_many").skill("rende");
                    for id in g.hand(a.actor) {
                        let mark = if selected.contains(&id) { "✔ " } else { "" };
                        b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, &id)));
                    }
                    if !selected.is_empty() {
                        b = b.option("confirm", format!("确认（已选 {} 张）", selected.len()));
                    }
                    Ok(Step::Need(b.escape("不发动仁德").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("confirm") => {
                    if selected.is_empty() {
                        return Err("仁德：至少选择 1 张牌".into());
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "target", "selected": selected }),
                    ))
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("仁德：未知选项 {act}"))?
                        .to_string();
                    if !g.hand(a.actor).contains(&id) {
                        return Err("仁德：只能交出自己的手牌".into());
                    }
                    if selected.contains(&id) {
                        selected.retain(|c| *c != id);
                    } else {
                        selected.push(id);
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "select", "selected": selected }),
                    ))
                }
            }
        }
        "target" => {
            let selected: Vec<CardId> = a.data["selected"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let cands = rende_targets(g, a.actor);
            match g.answer.take().as_deref() {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(a.actor, "target").skill("rende");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("交给 {}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动仁德").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("仁德：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("仁德：本阶段已对该角色发动过".into());
                    }
                    let n = selected.len();
                    for id in &selected {
                        g.gain(id, t)?;
                    }
                    g.bump(&format!("turn.rende_target:{t}"));
                    let before = g.counter("turn.rende_given");
                    let given = before + n as u32;
                    g.counters.insert("turn.rende_given".to_string(), given);
                    g.push_log(format!("仁德：交给 {} {n} 张牌", g.seats[t].name));
                    // 界仁德：本阶段给出**第二张**时，可视为使用一张基本牌（一次性）。
                    if before < 2 && given >= 2 && g.counter("turn.rende_basic_offered") == 0 {
                        g.bump("turn.rende_basic_offered");
                        return Ok(Step::Next(Stage::Mode, json!({ "step": "basic" })));
                    }
                    Ok(Step::Done)
                }
            }
        }
        "basic" => {
            // 候选：杀 / 火杀 / 雷杀 / 桃 / 酒（【闪】无主动使用场景 → 不提供）。
            let mut opts: Vec<(&'static str, String)> = Vec::new();
            if g.sha_used(a.actor) < g.sha_limit(a.actor)
                && !other_alive(g, a.actor).is_empty()
            {
                opts.push(("杀", "视为使用【杀】".into()));
                opts.push(("火杀", "视为使用【火杀】".into()));
                opts.push(("雷杀", "视为使用【雷杀】".into()));
            }
            if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                opts.push(("桃", "视为使用【桃】".into()));
            }
            if g.counter(&format!("turn.jiu_used:{}", a.actor)) == 0 {
                opts.push(("酒", "视为使用【酒】".into()));
            }
            match g.answer.take().as_deref() {
                None => {
                    if opts.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(a.actor, "choice").skill("rende");
                    for (name, text) in &opts {
                        b = b.option(format!("use:{name}"), text.clone());
                    }
                    Ok(Step::Need(b.escape("不视为使用").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let name = act
                        .strip_prefix("use:")
                        .ok_or_else(|| format!("仁德：未知选项 {act}"))?;
                    if !opts.iter().any(|(n, _)| *n == name) {
                        return Err("仁德：该基本牌不可用".into());
                    }
                    if name == "杀" || name == "火杀" || name == "雷杀" {
                        return Ok(Step::Next(
                            Stage::Target,
                            json!({ "step": "basic_target", "name": name }),
                        ));
                    }
                    let nm = name.to_string();
                    g.use_virtual(a.actor, &nm, Vec::new())?;
                    Ok(Step::Done)
                }
            }
        }
        "basic_target" => {
            let name = a.data["name"].as_str().unwrap_or("杀").to_string();
            let cands = other_alive(g, a.actor);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "target").skill("rende");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("对 {} 使用【{name}】", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不视为使用").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("仁德：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("仁德：目标不合法".into());
                    }
                    g.use_virtual(a.actor, &name, vec![t])?;
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("仁德：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 激将（界刘备·主公技）
// ---------------------------------------------------------------------------

pub const JIJIANG: SkillDef = SkillDef { kind: SkillKind::Lord,
    code: "jijiang",
    name: "激将",
    triggers: &[EventKind::CardUsed, EventKind::CardPlayed],
    // 触发钩子（界激将摸牌）需要 AnyTime；「主动使用」的时机由 `can_active` 自己把关。
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_jijiang),
    handler: jijiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 界激将：代出【杀】的同势力角色（非主公、蜀势力、有手牌）。
pub fn relay_order(g: &Game, lord: usize) -> Vec<usize> {
    g.seats_from_left_of(lord)
        .into_iter()
        .filter(|i| {
            *i != lord
                && g.seats[*i].hp > 0
                && g.seats[*i].kingdom == "shu"
                && !g.hand(*i).is_empty()
        })
        .map(|i| i)
        .collect()
}

pub fn relay_options(g: &Game, seat: usize, kind: ResponseKind) -> Vec<(String, String)> {
    if kind != ResponseKind::Sha
        || !active(g, seat, "jijiang")
        || g.seats[seat].identity != "lord"
        || relay_order(g, seat).is_empty()
    {
        return Vec::new();
    }
    vec![("jijiang".to_string(), "激将：令蜀势力角色代出【杀】".to_string())]
}

/// 发起接力（响应窗口选择 `jijiang` 后调用）：压入激将帧。
pub fn start_relay(
    g: &mut Game,
    lord: usize,
    _kind: ResponseKind,
    used: Option<&CardId>,
) {
    start_relay_for(g, lord, used, false);
}

/// `for_use = true`：出牌阶段主动发动（代出的【杀】视为**主公使用**，牌不转手）；
/// `for_use = false`：响应窗口内发动（代出的【杀】视为**主公打出**，牌直接进弃牌堆）。
pub fn start_relay_for(
    g: &mut Game,
    lord: usize,
    used: Option<&CardId>,
    for_use: bool,
) {
    let order = relay_order(g, lord);
    crate::kernel::resolve::push_ask(
        g,
        "jijiang",
        Stage::Choose,
        lord,
        json!({
            "step": "relay",
            "order": order,
            "cursor": 0,
            "used": used.cloned(),
            "for_use": for_use,
        }),
    );
}

/// 合法【杀】目标集（技能侧给出牌名与牌定义，内核只做统一复核）。
pub fn sha_target_sets(
    g: &crate::kernel::Game,
    seat: usize,
    carrier: Option<&CardId>,
) -> Vec<Vec<usize>> {
    let def = crate::cards::def("杀");
    crate::kernel::card_play::target_sets_for_skill(g, seat, "杀", &def, carrier)
}

/// 主动激将的合法入口：出牌阶段、主公、有可代出的同势力角色、且【杀】有合法目标。
pub fn can_jijiang(g: &crate::kernel::Game, seat: usize) -> bool {
    g.turn == seat
        && g.phase == Phase::Play
        && active(g, seat, "jijiang")
        && g.seats[seat].identity == "lord"
        && !relay_order(g, seat).is_empty()
        && !sha_target_sets(g, seat, None).is_empty()
}

fn jijiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let ev = a.data["event"].as_str().unwrap_or_default();
    // 三种入口：主动（无 step/event）/ 事件触发（有 event）/ 响应接力（step=relay）。
    let step = a.data["step"].as_str().unwrap_or("").to_string();

    // ---- 主动形态首次发动（execute_active：无 step、无 event、无 seat）----
    if step.is_empty() && a.data.get("seat").is_none() && ev.is_empty() {
        let lord = a.actor;
        if !can_jijiang(g, lord) {
            return Ok(Step::Done);
        }
        start_relay_for(g, lord, None, true);
        return Ok(Step::Next(Stage::Choose, json!({ "step": "active" })));
    }

    // ---- 主动形态（出牌阶段「需要使用【杀】」）：代出 → 主公选目标 → 视为主公使用 ----
    if step == "active" {
        let lord = a.actor;
        // ① 子流程结果（唯一通道）：先于读回答检查。
        if let Some(r) = g.relay_result.take() {
            match r {
                crate::kernel::RelayOutcome::Played { card, by } => {
                    // `Step::Need` 不带 pending.data → 用 Step::Next 持久化帧数据。
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({
                            "step": "active",
                            "phase": "pick_target",
                            "card": card,
                            "by": by,
                        }),
                    ));
                }
                crate::kernel::RelayOutcome::Declined => {
                    g.push_log("激将：无人代出【杀】".to_string());
                    return Ok(Step::Done);
                }
            }
        }
        // ② 选目标
        if a.data["phase"].as_str() == Some("pick_target") {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let by = a.data["by"].as_u64().map(|n| n as usize);
            if let Some(ans) = g.answer.take() {
                let targets: Vec<usize> = ans
                    .strip_prefix("target:")
                    .ok_or_else(|| format!("激将：未知选项 {ans}"))?
                    .split(',')
                    .filter_map(|t| t.parse::<usize>().ok())
                    .collect();
                if targets.is_empty() {
                    return Err("激将：缺少目标".into());
                }
                // 视为**主公**使用【杀】：次数/攻击范围按主公算，承载牌来自代出者。
                crate::kernel::card_play::start_use(
                    g,
                    lord,
                    Some(&card),
                    "杀",
                    targets,
                    crate::kernel::card_play::UseOpts::relay(),
                )?;
                let by_name = by
                    .and_then(|b| g.seats.get(b).map(|s| s.name.clone()))
                    .unwrap_or_default();
                g.push_log(format!(
                    "激将：{by_name} 代出【{}】，视为 {} 使用",
                    g.card_label(&card),
                    g.seats[lord].name
                ));
                return Ok(Step::Done);
            }
            let sets = sha_target_sets(g, lord, Some(&card));
            if sets.is_empty() {
                g.push_log("激将：无合法目标（视为使用【杀】取消）".to_string());
                return Ok(Step::Done);
            }
            let mut b = builder(lord, "target").skill("jijiang");
            for ts in sets {
                b = b.option(
                    format!(
                        "target:{}",
                        ts.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",")
                    ),
                    format!(
                        "视为对 {} 使用【杀】",
                        ts.iter()
                            .map(|t| g.seats[*t].name.clone())
                            .collect::<Vec<_>>()
                            .join("、")
                    ),
                );
            }
            return Ok(Step::Need(
                b.data(json!({
                    "step": "active",
                    "phase": "pick_target",
                    "card": card,
                    "by": by,
                }))
                .escape_mandatory("（必须选择目标）")
                .build(),
            ));
        }
        // ③ 首次：起子流程（回到本入口读结果）
        if !can_jijiang(g, lord) {
            return Ok(Step::Done);
        }
        start_relay_for(g, lord, None, true);
        return Ok(Step::Next(Stage::Choose, json!({ "step": "active" })));
    }


    if step.is_empty() && (a.data.get("seat").is_some() || !ev.is_empty()) {
        // 界激将摸牌：其他蜀势力角色**于其回合外**使用/打出【杀】时，其可令主公摸 1（每回合限一次）。
        let lord = a.actor;
        let by = a.data["seat"].as_u64().map(|n| n as usize);
        let Some(by) = by else { return Ok(Step::Done) };
        if by == lord || g.seats[by].kingdom != "shu" {
            return Ok(Step::Done);
        }
        if g.turn == by {
            return Ok(Step::Done); // 必须是其回合外
        }
        if !(ev.starts_with("CardUsed") || ev.starts_with("CardPlayed")) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        if !g.def_of(&card).tags.sha {
            return Ok(Step::Done);
        }
        if g.counter("turn.jijiang_draw") > 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(by, "choice").skill("jijiang");
                b = b.option("opt1", "令主公摸一张牌");
                Ok(Step::Need(b.escape("不令主公摸牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(opt1) => {
                g.bump("turn.jijiang_draw");
                let n = g.draw(lord, 1).len();
                g.push_log(format!(
                    "激将：{} 令主公摸 {n} 张牌",
                    g.seats[by].name
                ));
                let _ = opt1;
                Ok(Step::Done)
            }
            Some(other) => Err(format!("激将：未知选项 {other}")),
        }
    } else if step == "relay" {
        // 接力：按座位顺序询问同势力角色代出【杀】。
        let lord = a.actor;
        let mut order: Vec<usize> = a.data["order"]
            .as_array()
            .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
        let used = a.data["used"].as_str().map(|s| s.to_string());
        let for_use = a.data["for_use"].as_bool().unwrap_or(false);
        while cursor < order.len() {
            let by = order[cursor];
            if g.seats[by].hp <= 0 || g.hand(by).is_empty() {
                cursor += 1;
                continue;
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(by, "choice").skill("jijiang");
                    for id in g.hand(by) {
                        if g.def_of(&id).tags.sha {
                            b = b.option(
                                format!("respond:{id}"),
                                format!("代替主公打出【{}】", label(g, &id)),
                            );
                        }
                    }
                    return Ok(Step::Need(b.escape("不代替打出").build()));
                }
                Some("pass") => {
                    cursor += 1;
                    order = order.clone();
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "relay", "order": order, "cursor": cursor, "used": used }),
                    ));
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("respond:")
                        .ok_or_else(|| format!("激将：未知选项 {act}"))?;
                    let id = id.to_string();
                    if !g.hand(by).contains(&id) || !g.def_of(&id).tags.sha {
                        return Err("激将：只能代出【杀】".into());
                    }
                    // 响应形态：牌直接进弃牌堆（视为打出）；使用形态：牌作为承载牌交给主公使用。
                    if !for_use {
                        g.to_discard(&id, "jijiang")?;
                    }
                    g.push_log(format!(
                        "激将：{} 代替主公{}【{}】",
                        g.seats[by].name,
                        if for_use { "使用" } else { "打出" },
                        label(g, &id)
                    ));
                    g.presentation.push(
                        Presentation::new(
                            "jijiang",
                            POutcome::Played { label: g.name_of(&id) },
                        )
                        .actor(by)
                        .cards(vec![id.clone()])
                        .fx("jijiang"),
                    );
                    // 唯一结果通道：响应流程消费后视为本次响应已成立。
                    g.relay_result = Some(crate::kernel::RelayOutcome::Played {
                        card: id.clone(),
                        by,
                    });
                    let _ = lord;
                    return Ok(Step::Done);
                }
            }
        }
        // 全部放弃 → 回到主公本人的响应窗口（不是「未响应」）。
        g.relay_result = Some(crate::kernel::RelayOutcome::Declined);
        Ok(Step::Done)
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 义绝（界关羽）
// ---------------------------------------------------------------------------

pub const YIJUE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "yijue",
    name: "义绝",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_yijue),
    handler: yijue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_yijue(g: &Game, seat: usize) -> bool {
    active(g, seat, "yijue")
        && play_phase(g, seat)
        && g.counter("turn.yijue_used") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat).into_iter().any(|i| !g.hand(i).is_empty())
}

fn yijue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("card").to_string();
    match step.as_str() {
        "card" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "choice").skill("yijue");
                for id in g.hand(a.actor) {
                    b = b.option(format!("choose:{id}"), format!("弃置【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("不发动义绝").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("义绝：未知选项 {act}"))?
                    .to_string();
                if !g.hand(a.actor).contains(&id) {
                    return Err("义绝：只能弃置自己的手牌".into());
                }
                g.to_discard(&id, "yijue")?;
                g.bump("turn.yijue_used");
                g.push_log(format!("义绝：弃置【{}】", label(g, &id)));
                Ok(Step::Next(Stage::Target, json!({ "step": "target" })))
            }
        },
        "target" => {
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !g.hand(*i).is_empty())
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(a.actor, "target").skill("yijue");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("义绝 {}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动义绝").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("义绝：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("义绝：目标须有手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "step": "show", "target": t }),
                    ))
                }
            }
        }
        "show" => {
            // 用户裁定：由**目标自己**选一张手牌展示。
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let hand = g.hand(target);
                    if hand.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(target, "choice").skill("yijue");
                    for id in &hand {
                        b = b.option(format!("pick:{id}"), format!("展示【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("无法展示（跳过义绝）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("义绝：未知选项 {act}"))?
                        .to_string();
                    if !g.hand(target).contains(&id) {
                        return Err("义绝：只能展示自己的手牌".into());
                    }
                    g.reveal(&id, Some(target), "yijue");
                    let red = g.is_red(&id);
                    if red {
                        g.gain(&id, a.actor)?;
                        g.push_log(format!(
                            "义绝：{} 展示【{}】（红色），界关羽获得之",
                            g.seats[target].name,
                            label(g, &id)
                        ));
                        Ok(Step::Next(
                            Stage::PickOther,
                            json!({ "step": "heal", "target": target }),
                        ))
                    } else {
                        g.bump(&format!("turn.nonlocked_lost:{target}"));
                        g.seats[target].flags.insert("hand_sealed".to_string());
                        g.bump(&format!("turn.yijue_heart:{target}"));
                        g.push_log(format!(
                            "义绝：{} 展示【{}】（黑色），本回合非锁定技失效且不能使用/打出手牌；界关羽对其红桃【杀】伤害+1",
                            g.seats[target].name,
                            label(g, &id)
                        ));
                        Ok(Step::Done)
                    }
                }
            }
        }
        "heal" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("yijue");
                    b = b.option("opt1", format!("令 {} 回复 1 点体力", g.seats[target].name));
                    Ok(Step::Need(b.escape("不回复体力").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("opt1") => {
                    flow::heal(g, target, 1, "yijue");
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("义绝：未知选项 {other}")),
            }
        }
        other => Err(format!("义绝：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 武圣（界关羽）——被动转化：红牌当【杀】；方片【杀】无距离
// ---------------------------------------------------------------------------

pub const WUSHENG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "wusheng",
    name: "武圣",
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
    subject_window: None,
};

pub fn wusheng_conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "wusheng") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for id in g.hand(seat) {
        if !g.is_red(&id) || g.def_of(&id).tags.sha {
            continue;
        }
        out.push(super::Conversion {
            skill: "wusheng",
            from: id,
            to: "杀",
            usable: true,
            playable: true,
        });
    }
    out
}

/// 界武圣：**方片**【杀】无距离限制（花色读承载牌）。
pub fn sha_no_distance(g: &Game, seat: usize, card: &CardId) -> bool {
    active(g, seat, "wusheng")
        && g.cards
            .get(card)
            .is_some_and(|c| c.suit == crate::kernel::Suit::Diamond)
}

// ---------------------------------------------------------------------------
// 替身（界张飞）
// ---------------------------------------------------------------------------

pub const TISHEN: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "tishen",
    name: "替身",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 0,
    can_active: None,
    handler: tishen,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn tishen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if phase_cause(a) != "Prepare" {
        return Ok(Step::Done);
    }
    if g.answer.is_none() {
        if g.seats[seat].flags.contains("tishen_used")
            || g.seats[seat].hp >= g.seats[seat].max_hp
            || g.seats[seat].hp <= 0
        {
            return Ok(Step::Done);
        }
    }
    match g.answer.take().as_deref() {
        None => {
            let heal = g.seats[seat].max_hp - g.seats[seat].hp;
            let mut b = builder(seat, "choice").skill("tishen");
            b = b.option(
                "opt1",
                format!("回复 {heal} 点体力，并摸 {heal} 张牌"),
            );
            Ok(Step::Need(b.escape("不发动替身").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("opt1") => {
            g.seats[seat].flags.insert("tishen_used".to_string());
            let need = g.seats[seat].max_hp - g.seats[seat].hp;
            let mut healed = 0;
            for _ in 0..need {
                let before = g.seats[seat].hp;
                flow::heal(g, seat, 1, "tishen");
                if g.seats[seat].hp == before {
                    break;
                }
                healed += 1;
            }
            let drawn = g.draw(seat, healed as usize).len();
            g.push_log(format!(
                "替身：回复 {healed} 点体力，摸 {drawn} 张"
            ));
            Ok(Step::Done)
        }
        Some(other) => Err(format!("替身：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 咆哮（界张飞）
// ---------------------------------------------------------------------------

pub const PAOXIAO: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "paoxiao",
    name: "咆哮",
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
    subject_window: None,
};

pub fn sha_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if active(g, seat, "paoxiao") {
        Some(usize::MAX)
    } else {
        None
    }
}

/// 界咆哮：被抵消后本回合下一次【杀】伤害 +1（消费即清零）。
pub fn paoxiao_bonus(g: &Game, source: usize, card: Option<&CardId>) -> i32 {
    if !active(g, source, "paoxiao") {
        return 0;
    }
    if g.counter("turn.paoxiao_bonus") == 0 {
        return 0;
    }
    let is_sha = card.map(|c| g.def_of(c).tags.sha).unwrap_or(false);
    if is_sha {
        1
    } else {
        0
    }
}

/// 义绝：本回合对其使用的**红桃**【杀】伤害 +1。
pub fn yijue_heart_bonus(g: &Game, source: usize, victim: usize, card: Option<&CardId>) -> i32 {
    if !active(g, source, "yijue") {
        return 0;
    }
    if g.counter(&format!("turn.yijue_heart:{victim}")) == 0 {
        return 0;
    }
    let heart_sha = card
        .map(|c| {
            g.def_of(c).tags.sha
                && g.cards
                    .get(c)
                    .is_some_and(|inst| inst.suit == crate::kernel::Suit::Heart)
        })
        .unwrap_or(false);
    if heart_sha {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 涯角（界赵云）
// ---------------------------------------------------------------------------

pub const YAJIAO: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "yajiao",
    name: "涯角",
    triggers: &[EventKind::CardUsed, EventKind::CardPlayed],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: yajiao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yajiao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        if g.turn == owner {
            return Ok(Step::Done); // 仅回合外
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        // 只认「使用/打出的手牌」：牌已进弃牌堆/处理区，用事件里的 card 判类型组。
        let Some(top) = g.draw_top() else {
            return Ok(Step::Done);
        };
        g.to_processing(&top)?;
        g.reveal(&top, Some(owner), "yajiao");
        let same = type_group(g, &card) == type_group(g, &top);
        let eligible: Vec<usize> = if same {
            Vec::new()
        } else {
            other_alive(g, owner)
                .into_iter()
                .filter(|v| {
                    g.in_attack_range(*v, owner)
                        && (!g.hand(*v).is_empty()
                            || !g.judge_zone(*v).is_empty()
                            || !equip_ids(g, *v).is_empty())
                })
                .collect()
        };
        g.push_log(format!(
            "涯角：展示牌堆顶【{}】（{}）",
            label(g, &top),
            if same { "类型相同" } else { "类型不同" }
        ));
        if !same && eligible.is_empty() {
            g.to_discard(&top, "yajiao")?;
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({
                "step": "choose",
                "revealed": top,
                "same": same,
                "eligible": eligible,
            }),
        ));
    }
    if step == "discard_pick" {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let _ = owner;
        let revealed = a.data["revealed"].as_str().unwrap_or_default().to_string();
        let ids: Vec<CardId> = g
            .judge_zone(target)
            .into_iter()
            .chain(equip_ids(g, target))
            .chain(g.hand(target))
            .collect();
        return match g.answer.take().as_deref() {
            None => {
                if ids.is_empty() {
                    if g.zone_of(&revealed) == Some(&Zone::Processing) {
                        g.to_discard(&revealed, "yajiao")?;
                    }
                    return Ok(Step::Done);
                }
                let mut b = builder(owner, "choice").skill("yajiao");
                for id in &ids {
                    b = b.option(
                        format!("pick:{id}"),
                        format!("弃置{}【{}】", zone_label(g, id, target), label(g, id)),
                    );
                }
                Ok(Step::Need(b.escape("放弃弃置").build()))
            }
            Some("pass") => {
                if g.zone_of(&revealed) == Some(&Zone::Processing) {
                    g.to_discard(&revealed, "yajiao")?;
                }
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("涯角：未知选项 {act}"))?
                    .to_string();
                if !ids.contains(&id) {
                    return Err("涯角：该牌不在其区域".into());
                }
                let l = label(g, &id);
                g.to_discard(&id, "yajiao")?;
                g.push_log(format!("涯角：弃置 {} 的【{l}】", g.seats[target].name));
                if g.zone_of(&revealed) == Some(&Zone::Processing) {
                    g.to_discard(&revealed, "yajiao")?;
                }
                Ok(Step::Done)
            }
        }
    }
    // choose
    let revealed = a.data["revealed"].as_str().unwrap_or_default().to_string();
    let same = a.data["same"].as_bool().unwrap_or(false);
    let eligible: Vec<usize> = a.data["eligible"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("yajiao");
            if same {
                for i in (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0) {
                    b = b.option(
                        format!("target:{i}"),
                        format!("令 {} 获得展示牌", g.seats[i].name),
                    );
                }
            } else {
                for i in &eligible {
                    b = b.option(
                        format!("discard:{i}"),
                        format!("弃置 {} 一张牌", g.seats[*i].name),
                    );
                }
            }
            Ok(Step::Need(b.escape("不发动涯角").build()))
        }
        Some("pass") => {
            if g.zone_of(&revealed) == Some(&Zone::Processing) {
                g.to_discard(&revealed, "yajiao")?;
            }
            Ok(Step::Done)
        }
        Some(act) => {
            if let Some(t) = act.strip_prefix("target:") {
                let t: usize = t.parse().map_err(|_| "涯角：目标不合法".to_string())?;
                if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) {
                    return Err("涯角：目标不合法".into());
                }
                g.gain(&revealed, t)?;
                g.push_log(format!(
                    "涯角：令 {} 获得【{}】",
                    g.seats[t].name,
                    label(g, &revealed)
                ));
                return Ok(Step::Done);
            }
            if let Some(t) = act.strip_prefix("discard:") {
                let t: usize = t.parse().map_err(|_| "涯角：目标不合法".to_string())?;
                if !eligible.contains(&t) {
                    return Err("涯角：该角色不在范围内".into());
                }
                // 用户裁定：由赵云选择区域与具体牌（手牌随机）。
                let ids: Vec<CardId> = g
                    .judge_zone(t)
                    .into_iter()
                    .chain(equip_ids(g, t))
                    .chain(g.hand(t))
                    .collect();
                if ids.is_empty() {
                    return Ok(Step::Done);
                }
                return Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "discard_pick", "target": t, "revealed": revealed }),
                ));
            }
            Err(format!("涯角：未知选项 {act}"))
        }
    }
}

// ---------------------------------------------------------------------------
// 龙胆（界赵云）——被动转化：杀↔闪 / 酒↔桃
// ---------------------------------------------------------------------------

pub const LONGDAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "longdan",
    name: "龙胆",
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
    subject_window: None,
};

pub fn longdan_conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "longdan") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for id in g.hand(seat) {
        let def = g.def_of(&id);
        let (to, usable, playable): (&'static str, bool, bool) = if def.tags.sha {
            ("闪", false, true)
        } else if def.tags.shan {
            ("杀", true, true)
        } else if def.tags.jiu {
            ("桃", true, false)
        } else if def.tags.tao {
            ("酒", true, false)
        } else {
            continue;
        };
        out.push(super::Conversion {
            skill: "longdan",
            from: id,
            to,
            usable,
            playable,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// 马术（界马超）
// ---------------------------------------------------------------------------

pub const MASHU: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "mashu",
    name: "马术",
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
    subject_window: None,
};

pub fn distance_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "mashu") {
        -1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 铁骑（界马超）
// ---------------------------------------------------------------------------

pub const TIEQI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "tieqi",
    name: "铁骑",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: tieqi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn tieqi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    if step == "ask" {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("tieqi");
                b = b.option("opt1", format!("令 {} 本回合非锁定技失效并判定", g.seats[target].name));
                Ok(Step::Need(b.escape("不发动铁骑").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some("opt1") => {
                g.bump(&format!("turn.nonlocked_lost:{target}"));
                let Some(judge) = flow::judge_card(g, owner) else {
                    g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                    return Ok(Step::Done);
                };
                flow::judge_effective(g, owner, &judge);
                let suit = g.cards.get(&judge).map(|c| c.suit);
                let jlabel = label(g, &judge);
                flow::judge_to_discard(g, &judge)?;
                g.push_log(format!("铁骑：判定【{jlabel}】"));
                let suit = suit.ok_or_else(|| "铁骑：判定牌缺失".to_string())?;
                let has = g
                    .hand(target)
                    .into_iter()
                    .chain(equip_ids(g, target))
                    .any(|id| g.cards.get(&id).map(|c| c.suit == suit).unwrap_or(false));
                if !has {
                    g.push_log(format!(
                        "铁骑：{} 没有同花色牌，不能使用【闪】",
                        g.seats[target].name
                    ));
                    g.sha_intercept =
                        Some(crate::kernel::card_play::ShaIntercept::Unresponsive(target));
                    return Ok(Step::Done);
                }
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "discard", "target": target, "suit": format!("{suit:?}") }),
                ))
            }
            Some(other) => Err(format!("铁骑：未知选项 {other}")),
        }
    } else {
        let suit = a.data["suit"].as_str().unwrap_or_default().to_string();
        let hand: Vec<CardId> = g
            .hand(target)
            .into_iter()
            .chain(equip_ids(g, target))
            .filter(|id| format!("{:?}", g.cards.get(id).map(|c| c.suit)) == suit)
            .collect();
        match g.answer.take().as_deref() {
            None => {
                if hand.is_empty() {
                    g.sha_intercept =
                        Some(crate::kernel::card_play::ShaIntercept::Unresponsive(target));
                    return Ok(Step::Done);
                }
                let mut b = builder(target, "choice").skill("tieqi");
                for id in &hand {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("不弃置（不能使用【闪】）").build()))
            }
            Some("pass") => {
                g.sha_intercept =
                    Some(crate::kernel::card_play::ShaIntercept::Unresponsive(target));
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("铁骑：未知选项 {act}"))?
                    .to_string();
                if !hand.contains(&id) {
                    return Err("铁骑：只能弃置同花色的牌".into());
                }
                g.to_discard(&id, "tieqi")?;
                g.push_log(format!(
                    "铁骑：{} 弃置【{}】，可以使用【闪】",
                    g.seats[target].name,
                    label(g, &id)
                ));
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 空城（界诸葛亮）
// ---------------------------------------------------------------------------

pub const KONGCHENG: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "kongcheng",
    name: "空城",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(kongcheng_block),
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn kongcheng_block(
    g: &Game,
    target: usize,
    _source: usize,
    card: &str,
    _carrier: Option<&crate::kernel::CardId>,
) -> bool {
    if !active(g, target, "kongcheng") || !g.hand(target).is_empty() {
        return false;
    }
    let def = cards::def(card);
    def.tags.sha || def.tags.duel
}

// ---------------------------------------------------------------------------
// 观星（界诸葛亮）
// ---------------------------------------------------------------------------

pub const GUANXING: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "guanxing",
    name: "观星",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: guanxing,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn guanxing_n(g: &Game) -> usize {
    let alive = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).count();
    if alive < 4 {
        3
    } else {
        5
    }
}

fn guanxing(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    let cause = phase_cause(a);
    if step == "start" {
        let prepare = cause == "Prepare";
        let end_re = cause == "End" && g.counter("turn.guanxing_all_bottom") > 0;
        if !prepare && !end_re {
            return Ok(Step::Done);
        }
        if prepare && g.counter("turn.guanxing_done") > 0 {
            return Ok(Step::Done);
        }
        if prepare {
            g.bump("turn.guanxing_done");
        }
        let n = guanxing_n(g);
        let ids: Vec<CardId> = g.ids_in(|z| *z == Zone::Draw).into_iter().take(n).collect();
        if ids.is_empty() {
            return Ok(Step::Done);
        }
        for id in &ids {
            g.to_processing(id)?;
            g.reveal_private(seat, id, "guanxing");
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "place", "cause": cause, "cards": ids, "cursor": 0, "top": [], "bottom": [] }),
        ));
    }
    // place：逐张决定置于牌堆顶 / 牌堆底（顺序即放置顺序）。
    let cards: Vec<CardId> = a.data["cards"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let mut top: Vec<CardId> = a.data["top"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let mut bottom: Vec<CardId> = a.data["bottom"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if cursor >= cards.len() {
        // 落盘：底（按选择顺序 → 先放的靠上）与顶（后放的更靠上）。
        for id in bottom.iter().rev() {
            g.put_on_bottom(id)?;
        }
        for id in top.iter().rev() {
            g.put_on_top(id)?;
        }
        for id in &cards {
            g.conceal(id, "guanxing");
            g.clear_private_reveals(seat);
        }
        let all_bottom = top.is_empty();
        if all_bottom && cause == "Prepare" {
            g.bump("turn.guanxing_all_bottom");
        } else if all_bottom && cause == "End" {
            g.counters.insert("turn.guanxing_all_bottom".to_string(), 0);
        }
        g.push_log(format!(
            "观星：{} 张置于牌堆顶、{} 张置于牌堆底",
            top.len(),
            bottom.len()
        ));
        return Ok(Step::Done);
    }
    let id = cards[cursor].clone();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("guanxing");
            b = b.option("top", format!("【{}】置于牌堆顶", label(g, &id)));
            b = b.option("bottom", format!("【{}】置于牌堆底", label(g, &id)));
            Ok(Step::Need(b.escape("全部置于牌堆底").build()))
        }
        Some(ans) => {
            if ans == "top" {
                top.push(id);
            } else {
                bottom.push(id);
            }
            cursor += 1;
            if cursor >= cards.len() {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "place", "cause": cause, "cards": cards, "cursor": cursor, "top": top, "bottom": bottom }),
                ));
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "place", "cause": cause, "cards": cards, "cursor": cursor, "top": top, "bottom": bottom }),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 集智（界黄月英）
// ---------------------------------------------------------------------------

pub const JIZHI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jizhi",
    name: "集智",
    triggers: &[EventKind::CardUsed],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jizhi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jizhi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let step = a.data["step"].as_str().unwrap_or("start").to_string();
    if step == "start" {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        let def = g.def_of(&card);
        // 只认「非转化、非延时锦囊」。
        if def.kind != CardKind::Trick || def.tags.delayed {
            return Ok(Step::Done);
        }
        if g.virtual_defs.contains_key(&card) {
            return Ok(Step::Done);
        }
        let drawn = g.draw(owner, 1);
        let Some(id) = drawn.first().cloned() else {
            return Ok(Step::Done);
        };
        if g.def_of(&id).kind != CardKind::Basic {
            g.push_log(format!("集智：摸【{}】", label(g, &id)));
            return Ok(Step::Done);
        }
        // 用户裁定：改为询问（可以保留该牌）。
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "ask", "drawn": id }),
        ));
    }
    let drawn = a.data["drawn"].as_str().unwrap_or_default().to_string();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("jizhi");
            b = b.option(
                "opt1",
                format!("弃置【{}】，本回合手牌上限 +1", label(g, &drawn)),
            );
            Ok(Step::Need(b.escape("保留该牌").build()))
        }
        Some("pass") => {
            g.push_log(format!("集智：保留【{}】", label(g, &drawn)));
            Ok(Step::Done)
        }
        Some("opt1") => {
            if g.zone_of(&drawn) == Some(&Zone::Hand(owner)) {
                g.to_discard(&drawn, "jizhi")?;
            }
            g.bump("turn.jizhi_hand_limit");
            g.push_log(format!(
                "集智：弃置【{}】，本回合手牌上限 +1",
                label(g, &drawn)
            ));
            Ok(Step::Done)
        }
        Some(other) => Err(format!("集智：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 奇才（界黄月英）
// ---------------------------------------------------------------------------

pub const QICAI: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "qicai",
    name: "奇才",
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
    subject_window: None,
};

pub fn trick_no_distance(g: &Game, seat: usize) -> bool {
    active(g, seat, "qicai")
}

/// 奇才：防止**其他角色**弃置你装备区里的防具/宝物牌。
pub fn prevent_equip_discard(
    g: &Game,
    owner: usize,
    card: &CardId,
    by: Option<usize>,
) -> bool {
    if !active(g, owner, "qicai") {
        return false;
    }
    if by.map(|b| b == owner).unwrap_or(true) {
        return false;
    }
    matches!(
        g.zone_of(card),
        Some(Zone::Equip(s, crate::kernel::Slot::Armor))
            | Some(Zone::Equip(s, crate::kernel::Slot::Treasure))
            if *s == owner
    )
}

// ---------------------------------------------------------------------------
// 注册表 + 内核钩子汇总
// ---------------------------------------------------------------------------

pub const B9_SKILLS: &[SkillDef] = &[
    RENDE,
    JIJIANG,
    YIJUE,
    WUSHENG,
    TISHEN,
    PAOXIAO,
    YAJIAO,
    LONGDAN,
    MASHU,
    TIEQI,
    KONGCHENG,
    GUANXING,
    JIZHI,
    QICAI,
];

/// 被动转化技（武圣 + 龙胆）。
pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    let mut out = wusheng_conversions(g, seat);
    out.extend(longdan_conversions(g, seat));
    out
}
