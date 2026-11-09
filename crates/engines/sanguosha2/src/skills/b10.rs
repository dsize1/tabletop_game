//! B10（界甘宁 / 界吕蒙 / 界黄盖 / 界周瑜 / 界大乔 / 界陆逊 / 界孙权 / 界孙尚香）——按 `align/B10.json` 第 7 段实现。
//!
//! 用户口径（延续 B9）：标将已废弃 → 只实现界版语义，不保留 `is_jiexian` 分支。
//!
//! 本批新增的跨批能力：
//! - **额外回合**（博图）：`Game.extra_turn` + 驱动器插入一个 Prepare 起的额外回合。
//! - **跳过弃牌阶段**（克己）：`skills::skip_discard_phase` + 显式阶段播报。
//! - **赋值型手牌上限**（界英姿）：`skills::hand_limit_override`。
//! - **多目标无效**（奋威）：任意「目标列表为多人」的牌 → `Game.fenwei_void`（逐目标跳过）。
//! - **结算前手牌移出**（谦逊）：普通锦囊唯一目标 + 延时锦囊生效，`Game.void_return` 回合末归还。
//! - **手牌归零批次**（连营 X）：`Game.hand_loss_batch` / `lianying_pending`（每动作一档）。
//! - **回复前改向**（救援）：`skills::start_jiuyuan`（可选询问）。
//! - **响应过滤**（诈降）：`skills::block_real_shan`（只挡真实打出的【闪】）。

use serde_json::json;

use crate::cards::{self, CardKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};
use crate::presentation::POutcome;

use super::{active, builder, other_alive, SkillDef};

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

fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}

fn phase_cause(a: &Ask) -> String {
    a.data["cause"].as_str().unwrap_or_default().to_string()
}

// ---------------------------------------------------------------------------
// 奇袭（界甘宁）：黑色牌（手牌/装备区）当【过河拆桥】
// ---------------------------------------------------------------------------

pub const QIXI: SkillDef = SkillDef {
    code: "qixi",
    name: "奇袭",
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

pub fn qixi_conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "qixi") {
        return Vec::new();
    }
    hand_and_equip(g, seat)
        .into_iter()
        .filter(|id| !g.is_red(id))
        .map(|id| super::Conversion {
            skill: "qixi",
            from: id,
            to: "过河拆桥",
            usable: true,
            playable: false,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 奋威（界甘宁，限定技）：多目标牌 → 令任意个目标无效
// ---------------------------------------------------------------------------

pub const FENWEI: SkillDef = SkillDef {
    code: "fenwei",
    name: "奋威",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: fenwei,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn fenwei_owner(g: &Game) -> Option<usize> {
    (0..g.seats.len()).find(|i| {
        g.seats[*i].hp > 0
            && active(g, *i, "fenwei")
            && !g.seats[*i].flags.contains("fenwei_used")
    })
}

/// 结算前钩子：多目标牌 → 询问奋威（返回 true = 已压入询问帧）。
pub fn start_fenwei(g: &mut Game, targets: &[usize], user: usize) -> bool {
    let Some(owner) = fenwei_owner(g) else {
        return false;
    };
    if targets.len() < 2 {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "fenwei",
        Stage::Choose,
        owner,
        json!({ "targets": targets, "user": user, "selected": [] }),
    );
    true
}

fn fenwei(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let targets: Vec<usize> = a.data["targets"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    // 已选目标用座位位掩码持久化（data 在 Step::Need 间不可靠）。
    let key = "fenwei_sel";
    let sel = g.seats[owner].counters.get(key).copied().unwrap_or(0);
    let selected: Vec<usize> = (0..32)
        .filter(|i| sel & (1u32 << i) != 0)
        .collect();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choose_many").skill("fenwei");
            for t in &targets {
                let mark = if selected.contains(t) { "✔ " } else { "" };
                b = b.option(
                    format!("choose:{t}"),
                    format!("{mark}{} 无效", g.seats[*t].name),
                );
            }
            if !selected.is_empty() {
                b = b.option("confirm", format!("确认（已选 {} 个）", selected.len()));
            }
            Ok(Step::Need(b.escape("不发动奋威").build()))
        }
        Some("pass") => {
            g.seats[owner].counters.insert(key.to_string(), 0);
            Ok(Step::Done)
        }
        Some("confirm") => {
            g.seats[owner].counters.insert(key.to_string(), 0);
            g.seats[owner].flags.insert("fenwei_used".to_string());
            for t in &selected {
                g.fenwei_void.insert(*t);
            }
            g.push_log(format!("奋威：令 {} 个目标对此牌无效", selected.len()));
            Ok(Step::Done)
        }
        Some(act) => {
            let t: usize = act
                .strip_prefix("choose:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("奋威：未知选项 {act}"))?;
            if !targets.contains(&t) || t >= 32 {
                return Err("奋威：该目标不在此牌目标内".into());
            }
            let next = if sel & (1u32 << t) != 0 {
                sel & !(1u32 << t)
            } else {
                sel | (1u32 << t)
            };
            g.seats[owner].counters.insert(key.to_string(), next);
            Ok(Step::Next(Stage::Choose, json!({ "targets": targets })))
        }
    }
}

// ---------------------------------------------------------------------------
// 克己（界吕蒙）：未使用【杀】→ 跳过弃牌阶段
// ---------------------------------------------------------------------------

pub const KEJI: SkillDef = SkillDef {
    code: "keji",
    name: "克己",
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

pub fn skip_discard_phase(g: &Game, seat: usize) -> bool {
    active(g, seat, "keji") && g.sha_used(seat) == 0
}

// ---------------------------------------------------------------------------
// 勤学（界吕蒙，觉醒）
// ---------------------------------------------------------------------------

pub const QINXUE: SkillDef = SkillDef {
    code: "qinxue",
    name: "勤学",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: qinxue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn qinxue_ready(g: &Game, seat: usize) -> bool {
    active(g, seat, "qinxue")
        && !g.seats[seat].flags.contains("qinxue_awakened")
        && g.hand(seat).len() as i32 >= g.seats[seat].hp + 2
}

fn qinxue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let cause = phase_cause(a);
    if cause != "Prepare" && cause != "End" {
        return Ok(Step::Done);
    }
    if g.answer.is_none() && !qinxue_ready(g, seat) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("qinxue");
            b = b.option("opt1", "减 1 点体力上限，回复 1 点体力");
            b = b.option("opt2", "减 1 点体力上限，摸 2 张牌");
            Ok(Step::Need(b.escape("不发动勤学").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(opt) => {
            g.seats[seat].max_hp -= 1;
            if g.seats[seat].hp > g.seats[seat].max_hp {
                g.seats[seat].hp = g.seats[seat].max_hp;
            }
            g.seats[seat].flags.insert("qinxue_awakened".to_string());
            if opt == "opt1" {
                flow::heal(g, seat, 1, "qinxue");
            } else {
                g.draw(seat, 2);
            }
            // 觉醒后获得授予型技能【攻心】。
            crate::skills::grant_skill(g, seat, "gongxin");
            g.push_log(format!(
                "勤学：觉醒（体力上限 {}，{}）",
                g.seats[seat].max_hp,
                if opt == "opt1" { "回复 1 点体力" } else { "摸 2 张" }
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 攻心（界吕蒙，勤学觉醒后**授予**；授予型技能，对齐单不设技能行）
// ---------------------------------------------------------------------------

pub const GONGXIN: SkillDef = SkillDef {
    code: "gongxin",
    name: "攻心",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_gongxin),
    handler: gongxin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_gongxin(g: &Game, seat: usize) -> bool {
    active(g, seat, "gongxin")
        && play_phase(g, seat)
        && g.counter("turn.gongxin_used") == 0
        && other_alive(g, seat).into_iter().any(|i| !g.hand(i).is_empty())
}

fn gongxin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("target").to_string();
    match step.as_str() {
        "target" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "target").skill("gongxin");
                for i in other_alive(g, a.actor) {
                    if !g.hand(i).is_empty() {
                        b = b.option(
                            format!("target:{i}"),
                            format!("观看 {} 的手牌", g.seats[i].name),
                        );
                    }
                }
                Ok(Step::Need(b.escape("不发动攻心").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("攻心：目标不合法 {act}"))?;
                if t == a.actor || g.seats[t].hp <= 0 || g.hand(t).is_empty() {
                    return Err("攻心：目标不合法".into());
                }
                g.bump("turn.gongxin_used");
                // 「观看」= 私密展示给使用者（不进公共区）。
                for id in g.hand(t) {
                    g.reveal_private(a.actor, &id, "gongxin");
                }
                g.push_log(format!("攻心：观看 {} 的手牌", g.seats[t].name));
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "card", "target": t }),
                ))
            }
        },
        "card" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hearts: Vec<CardId> = g
                .hand(target)
                .into_iter()
                .filter(|id| {
                    g.cards
                        .get(id)
                        .is_some_and(|c| c.suit == crate::kernel::Suit::Heart)
                })
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("gongxin");
                    for id in &hearts {
                        b = b.option(
                            format!("discard:{id}"),
                            format!("弃置【{}】", label(g, id)),
                        );
                        b = b.option(
                            format!("top:{id}"),
                            format!("置于牌堆顶【{}】", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不处理红桃牌").build()))
                }
                Some("pass") => {
                    g.clear_private_reveals(a.actor);
                    Ok(Step::Done)
                }
                Some(act) => {
                    g.clear_private_reveals(a.actor);
                    if let Some(id) = act.strip_prefix("discard:") {
                        let id = id.to_string();
                        if !hearts.contains(&id) {
                            return Err("攻心：只能选择其红桃手牌".into());
                        }
                        g.to_discard(&id, "gongxin")?;
                        g.push_log(format!(
                            "攻心：弃置 {} 的【{}】",
                            g.seats[target].name,
                            label(g, &id)
                        ));
                        return Ok(Step::Done);
                    }
                    if let Some(id) = act.strip_prefix("top:") {
                        let id = id.to_string();
                        if !hearts.contains(&id) {
                            return Err("攻心：只能选择其红桃手牌".into());
                        }
                        g.put_on_top(&id)?;
                        g.push_log(format!(
                            "攻心：将 {} 的【{}】置于牌堆顶",
                            g.seats[target].name,
                            label(g, &id)
                        ));
                        return Ok(Step::Done);
                    }
                    Err(format!("攻心：未知选项 {act}"))
                }
            }
        }
        other => Err(format!("攻心：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 博图（界吕蒙）：本回合弃牌含四花色 → 额外回合
// ---------------------------------------------------------------------------

pub const BOTU: SkillDef = SkillDef {
    code: "botu",
    name: "博图",
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

pub fn botu_maybe(g: &mut Game, seat: usize) {
    if !active(g, seat, "botu") || g.extra_turn_active {
        return;
    }
    if g.counter("turn.discard_suits") != 15 {
        return;
    }
    let key = format!("botu:{}", g.round);
    let used = g.seats[seat].counters.get(&key).copied().unwrap_or(0);
    let limit = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).count().min(3) as u32;
    if used >= limit {
        return;
    }
    g.seats[seat].counters.insert(key, used + 1);
    g.extra_turn = Some(seat);
    g.push_log(format!(
        "博图：本回合弃牌含四花色 → 额外回合（{} / {limit}）",
        used + 1
    ));
}

// ---------------------------------------------------------------------------
// 苦肉（界黄盖）
// ---------------------------------------------------------------------------

pub const KUROU: SkillDef = SkillDef {
    code: "kurou",
    name: "苦肉",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_kurou),
    handler: kurou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_kurou(g: &Game, seat: usize) -> bool {
    active(g, seat, "kurou")
        && play_phase(g, seat)
        && g.counter("turn.kurou_used") == 0
        && !hand_and_equip(g, seat).is_empty()
}

fn kurou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["step"].as_str() == Some("lose") {
        flow::lose_hp(g, a.actor, 1, "kurou");
        g.push_log("苦肉：失去 1 点体力".to_string());
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(a.actor, "choice").skill("kurou");
            for id in hand_and_equip(g, a.actor) {
                b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, &id)));
            }
            Ok(Step::Need(b.escape("不发动苦肉").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("苦肉：未知选项 {act}"))?
                .to_string();
            if !hand_and_equip(g, a.actor).contains(&id) {
                return Err("苦肉：该牌不在可用区域".into());
            }
            g.to_discard(&id, "kurou")?;
            g.bump("turn.kurou_used");
            g.push_log(format!("苦肉：弃置【{}】", label(g, &id)));
            Ok(Step::Next(Stage::Choose, json!({ "step": "lose" })))
        }
    }
}

// ---------------------------------------------------------------------------
// 诈降（界黄盖，锁定）
// ---------------------------------------------------------------------------

pub const ZHAXIANG: SkillDef = SkillDef {
    code: "zhaxiang",
    name: "诈降",
    triggers: &[EventKind::HpLost],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhaxiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhaxiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(seat) {
        return Ok(Step::Done);
    }
    let n = g.draw(seat, 3).len();
    if play_phase(g, seat) {
        g.bump("turn.zhaxiang_sha_bonus");
        g.seats[seat].flags.insert("zhaxiang_red".to_string());
    }
    g.push_log(format!("诈降：失去体力，摸 {n} 张"));
    Ok(Step::Done)
}

pub fn sha_limit_bonus(g: &Game, seat: usize) -> usize {
    g.counter("turn.zhaxiang_sha_bonus") as usize
}

/// 诈降的红色【杀】：只挡**真实打出的【闪】**（八卦/技能视为【闪】仍可抵消）。
///
/// 判定时机在【闪】响应窗口：从当前牌结算帧取「正在结算的牌」，读其花色 + 使用者的
/// `zhaxiang_red` 标记（本回合出牌阶段失去体力后置位）。
pub fn block_real_shan(g: &Game, _target: usize) -> bool {
    let Some(card) = g.stack.iter().rev().find_map(|f| match f {
        crate::kernel::resolve::Frame::Flow(ff)
            if ff.kind == crate::kernel::resolve::FlowKind::Card =>
        {
            ff.data["card"].as_str().map(|s| s.to_string())
        }
        _ => None,
    }) else {
        return false;
    };
    let Some(owner) = g.stack.iter().rev().find_map(|f| match f {
        crate::kernel::resolve::Frame::Flow(ff)
            if ff.kind == crate::kernel::resolve::FlowKind::Card =>
        {
            Some(ff.actor)
        }
        _ => None,
    }) else {
        return false;
    };
    if !active(g, owner, "zhaxiang") || !g.seats[owner].flags.contains("zhaxiang_red") {
        return false;
    }
    let def = g.def_of(&card);
    def.tags.sha && g.is_red(&card)
}

// ---------------------------------------------------------------------------
// 英姿（界周瑜，锁定）
// ---------------------------------------------------------------------------

pub const YINGZI: SkillDef = SkillDef {
    code: "yingzi",
    name: "英姿",
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

pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if active(g, seat, "yingzi") {
        Some(g.seats[seat].max_hp.max(0) as usize)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 反间（界周瑜）
// ---------------------------------------------------------------------------

pub const FANJIAN: SkillDef = SkillDef {
    code: "fanjian",
    name: "反间",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_fanjian),
    handler: fanjian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_fanjian(g: &Game, seat: usize) -> bool {
    active(g, seat, "fanjian")
        && play_phase(g, seat)
        && g.counter("turn.fanjian_used") == 0
        && !g.hand(seat).is_empty()
        && !other_alive(g, seat).is_empty()
}

fn fanjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("target").to_string();
    match step.as_str() {
        "target" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "target").skill("fanjian");
                for i in other_alive(g, a.actor) {
                    b = b.option(format!("target:{i}"), format!("对 {} 发动反间", g.seats[i].name));
                }
                Ok(Step::Need(b.escape("不发动反间").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("反间：目标不合法 {act}"))?;
                if t == a.actor || g.seats[t].hp <= 0 {
                    return Err("反间：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "card", "target": t }),
                ))
            }
        },
        "card" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("fanjian");
                    for id in g.hand(a.actor) {
                        b = b.option(format!("choose:{id}"), format!("交给【{}】", label(g, &id)));
                    }
                    Ok(Step::Need(b.escape("不发动反间").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("反间：未知选项 {act}"))?
                        .to_string();
                    if !g.hand(a.actor).contains(&id) {
                        return Err("反间：只能交出自己的手牌".into());
                    }
                    g.reveal(&id, Some(a.actor), "fanjian");
                    g.gain(&id, target)?;
                    g.bump("turn.fanjian_used");
                    let suit = g.cards.get(&id).map(|c| c.suit);
                    g.push_log(format!(
                        "反间：交给 {} 【{}】",
                        g.seats[target].name,
                        label(g, &id)
                    ));
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "choice", "target": target, "suit": format!("{suit:?}") }),
                    ))
                }
            }
        }
        "choice" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let suit = a.data["suit"].as_str().unwrap_or_default().to_string();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(target, "choice").skill("fanjian");
                    b = b.option("opt1", "展示所有手牌，弃置同花色的牌");
                    b = b.option("opt2", "失去 1 点体力");
                    Ok(Step::Need(b.escape("（默认）失去 1 点体力").build()))
                }
                Some("opt1") => {
                    let same: Vec<CardId> = g
                        .hand(target)
                        .into_iter()
                        .filter(|c| format!("{:?}", g.cards.get(c).map(|x| x.suit)) == suit)
                        .collect();
                    for c in g.hand(target) {
                        g.reveal(&c, Some(target), "fanjian");
                    }
                    for c in &same {
                        g.to_discard(c, "fanjian")?;
                    }
                    g.push_log(format!(
                        "反间：{} 展示手牌并弃置 {} 张同花色牌",
                        g.seats[target].name,
                        same.len()
                    ));
                    Ok(Step::Done)
                }
                Some(_) => {
                    flow::lose_hp(g, target, 1, "fanjian");
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("反间：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 流离（界大乔）：成为【杀】目标时弃一张牌转移
// ---------------------------------------------------------------------------

pub const LIULI: SkillDef = SkillDef {
    code: "liuli",
    name: "流离",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: liuli,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn liuli(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    if step == "ask" {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("liuli");
                b = b.option("opt1", "弃置一张牌并转移此【杀】");
                Ok(Step::Need(b.escape("不发动流离").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some(_) => Ok(Step::Next(Stage::PickOwn, json!({ "step": "card", "user": user }))),
        }
    } else if step == "card" {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("liuli");
                for id in hand_and_equip(g, owner) {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("不发动流离").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("流离：未知选项 {act}"))?
                    .to_string();
                if !hand_and_equip(g, owner).contains(&id) {
                    return Err("流离：该牌不在可用区域".into());
                }
                g.to_discard(&id, "liuli")?;
                g.push_log(format!("流离：弃置【{}】", label(g, &id)));
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "user": user }),
                ))
            }
        }
    } else {
        let user = a.data["user"].as_u64().unwrap_or(0) as usize;
        let cands: Vec<usize> = other_alive(g, owner)
            .into_iter()
            .filter(|i| *i != user && g.in_attack_range(owner, *i))
            .collect();
        match g.answer.take().as_deref() {
            None => {
                if cands.is_empty() {
                    g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                    return Ok(Step::Done);
                }
                let mut b = builder(owner, "target").skill("liuli");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("转移给 {}", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不转移").build()))
            }
            Some("pass") => {
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Continue);
                Ok(Step::Done)
            }
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("流离：目标不合法 {act}"))?;
                if !cands.contains(&t) {
                    return Err("流离：目标不合法".into());
                }
                g.push_log(format!("流离：此【杀】转移给 {}", g.seats[t].name));
                g.sha_intercept = Some(crate::kernel::card_play::ShaIntercept::Replace(vec![t]));
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 国色（界大乔）
// ---------------------------------------------------------------------------

pub const GUOSE: SkillDef = SkillDef {
    code: "guose",
    name: "国色",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_guose),
    handler: guose,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn diamonds(g: &Game, seat: usize) -> Vec<CardId> {
    hand_and_equip(g, seat)
        .into_iter()
        .filter(|id| {
            g.cards
                .get(id)
                .is_some_and(|c| c.suit == crate::kernel::Suit::Diamond)
        })
        .collect()
}

fn find_lebu(g: &Game) -> Option<CardId> {
    for seat in 0..g.seats.len() {
        for id in g.judge_zone(seat) {
            if g.def_of(&id).tags.delayed && g.name_of(&id) == "乐不思蜀" {
                return Some(id);
            }
        }
    }
    None
}

fn can_guose(g: &Game, seat: usize) -> bool {
    active(g, seat, "guose")
        && play_phase(g, seat)
        && g.counter("turn.guose_used") == 0
        && !diamonds(g, seat).is_empty()
}

fn guose(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("opt").to_string();
    match step.as_str() {
        "opt" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "choice").skill("guose");
                b = b.option("opt1", "将一张方片牌当【乐不思蜀】使用，然后摸一张牌");
                if find_lebu(g).is_some() {
                    b = b.option("opt2", "弃置一张方片牌，再弃置场上的一张【乐不思蜀】");
                }
                Ok(Step::Need(b.escape("不发动国色").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(opt) => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "card", "opt": opt }),
            )),
        },
        "card" => {
            let opt = a.data["opt"].as_str().unwrap_or("opt1").to_string();
            let ds = diamonds(g, a.actor);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("guose");
                    for id in &ds {
                        b = b.option(format!("pick:{id}"), format!("选择【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不发动国色").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("国色：未知选项 {act}"))?
                        .to_string();
                    if !ds.contains(&id) {
                        return Err("国色：只能选择方片牌".into());
                    }
                    g.bump("turn.guose_used");
                    if opt == "opt1" {
                        return Ok(Step::Next(
                            Stage::Target,
                            json!({ "step": "target", "card": id }),
                        ));
                    }
                    // 选项 2：弃方片 + 弃场上一张乐（自动取第一张），不摸牌。
                    let Some(lebu) = find_lebu(g) else {
                        return Err("国色：场上没有【乐不思蜀】".into());
                    };
                    g.to_discard(&id, "guose")?;
                    g.to_discard(&lebu, "guose")?;
                    g.push_log(format!(
                        "国色：弃置【{}】与场上的【{}】",
                        label(g, &id),
                        label(g, &lebu)
                    ));
                    Ok(Step::Done)
                }
            }
        }
        "target" => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let cands: Vec<usize> = other_alive(g, a.actor);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "target").skill("guose");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("对 {} 使用", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动国色").build()))
                }
                Some("pass") => {
                    g.bump("turn.guose_used");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("国色：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("国色：目标不合法".into());
                    }
                    g.use_as(a.actor, &card, "乐不思蜀", vec![t], false, None)?;
                    let n = g.draw(a.actor, 1).len();
                    g.push_log(format!("国色：方片当【乐不思蜀】使用，摸 {n} 张"));
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("国色：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 谦逊（界陆逊）：唯一目标时移出所有手牌
// ---------------------------------------------------------------------------

pub const QIANXUN: SkillDef = SkillDef {
    code: "qianxun",
    name: "谦逊",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qianxun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 结算前钩子：普通锦囊仅以持有者为唯一目标（返回 true = 已压入询问帧）。
pub fn start_qianxun(g: &mut Game, user: usize, targets: &[usize], name: &str) -> bool {
    let def = cards::def(name);
    if def.kind != CardKind::Trick || def.tags.delayed {
        return false;
    }
    let Some(&t) = targets.first() else {
        return false;
    };
    if targets.len() != 1 || t == user {
        return false;
    }
    if !active(g, t, "qianxun") || g.hand(t).is_empty() {
        return false;
    }
    crate::kernel::resolve::push_ask(g, "qianxun", Stage::Choose, t, json!({}));
    true
}

fn qianxun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("qianxun");
            b = b.option("opt1", "将所有手牌移出游戏直到回合结束");
            Ok(Step::Need(b.escape("不移出").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            let cards = g.hand(seat);
            let n = cards.len();
            let due = g.turn_seq;
            for id in cards {
                g.to_void(&id)?;
                g.void_return.push((seat, id, due));
            }
            g.push_log(format!(
                "谦逊：{} 将 {n} 张手牌移出游戏直到回合结束",
                g.seats[seat].name
            ));
            Ok(Step::Done)
        }
    }
}

/// 回合切换：归还谦逊移出的牌。
pub fn return_void_cards(g: &mut Game, turn_seq: u32) {
    let due: Vec<(usize, CardId, u32)> = g
        .void_return
        .iter()
        .filter(|(_, _, d)| *d <= turn_seq)
        .cloned()
        .collect();
    if due.is_empty() {
        return;
    }
    g.void_return.retain(|(_, _, d)| *d > turn_seq);
    for (seat, id, _) in due {
        let _ = g.from_void(&id, Zone::Hand(seat));
    }
}

// ---------------------------------------------------------------------------
// 连营（界陆逊）：失去所有手牌后令至多 X 名角色各摸 1
// ---------------------------------------------------------------------------

pub const LIANYING: SkillDef = SkillDef {
    code: "lianying",
    name: "连营",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: lianying,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 栈空后由 `advance` 调用：压入连营询问（返回 true = 已压入）。
pub fn start_lianying(g: &mut Game, seat: usize, x: usize) -> bool {
    if !active(g, seat, "lianying") || g.seats[seat].hp <= 0 || x == 0 {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "lianying",
        Stage::Choose,
        seat,
        json!({ "x": x, "selected": [] }),
    );
    true
}

fn lianying(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let x = a.data["x"].as_u64().unwrap_or(0) as usize;
    let mut selected: Vec<usize> = a.data["selected"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    if selected.len() >= x {
        for t in &selected {
            g.draw(*t, 1);
        }
        g.push_log(format!(
            "连营：令 {} 名角色各摸 1 张",
            selected.len()
        ));
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choose_many").skill("lianying");
            for i in (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0) {
                if selected.contains(&i) {
                    continue;
                }
                b = b.option(format!("target:{i}"), format!("令 {} 摸 1 张", g.seats[i].name));
            }
            if !selected.is_empty() {
                b = b.option("confirm", format!("确认（已选 {} / {x}）", selected.len()));
            }
            Ok(Step::Need(b.escape("不发动连营").build()))
        }
        Some("pass") => {
            for t in &selected {
                g.draw(*t, 1);
            }
            Ok(Step::Done)
        }
        Some("confirm") => {
            for t in &selected {
                g.draw(*t, 1);
            }
            g.push_log(format!("连营：令 {} 名角色各摸 1 张", selected.len()));
            Ok(Step::Done)
        }
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("连营：未知选项 {act}"))?;
            if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) {
                return Err("连营：目标不合法".into());
            }
            if !selected.contains(&t) {
                selected.push(t);
            }
            if selected.len() >= x {
                for s in &selected {
                    g.draw(*s, 1);
                }
                return Ok(Step::Done);
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "x": x, "selected": selected }),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 制衡（界孙权）
// ---------------------------------------------------------------------------

pub const ZHIHENG: SkillDef = SkillDef {
    code: "zhiheng",
    name: "制衡",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhiheng),
    handler: zhiheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_zhiheng(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhiheng")
        && play_phase(g, seat)
        && g.counter("turn.zhiheng_used") == 0
        && !hand_and_equip(g, seat).is_empty()
}

fn zhiheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let mut selected: Vec<CardId> = a.data["selected"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    selected.retain(|id| hand_and_equip(g, a.actor).contains(id));
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(a.actor, "choose_many").skill("zhiheng");
            for id in hand_and_equip(g, a.actor) {
                let mark = if selected.contains(&id) { "✔ " } else { "" };
                b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, &id)));
            }
            if !selected.is_empty() {
                b = b.option("confirm", format!("确认（已选 {} 张）", selected.len()));
            }
            Ok(Step::Need(b.escape("不发动制衡").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            if selected.is_empty() {
                return Err("制衡：至少选择 1 张牌".into());
            }
            let n = selected.len();
            for id in &selected {
                g.to_discard(id, "zhiheng")?;
            }
            g.bump("turn.zhiheng_used");
            let extra = if g.hand(a.actor).is_empty() { 1 } else { 0 };
            let drawn = g.draw(a.actor, n + extra).len();
            g.push_log(format!(
                "制衡：弃置 {n} 张，摸 {drawn} 张{}",
                if extra > 0 { "（弃光手牌额外摸 1）" } else { "" }
            ));
            Ok(Step::Done)
        }
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("制衡：未知选项 {act}"))?
                .to_string();
            if !hand_and_equip(g, a.actor).contains(&id) {
                return Err("制衡：该牌不在可用区域".into());
            }
            if selected.contains(&id) {
                selected.retain(|c| *c != id);
            } else {
                selected.push(id);
            }
            Ok(Step::Next(
                Stage::Choose,
                json!({ "selected": selected }),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// 救援（界孙权，主公技）：回复前改为令主公回复
// ---------------------------------------------------------------------------

pub const JIUYUAN: SkillDef = SkillDef {
    code: "jiuyuan",
    name: "救援",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiuyuan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// `flow::heal` 的前置钩子：满足条件时压入询问帧并返回 true（本次回复由技能接管）。
pub fn start_jiuyuan(g: &mut Game, seat: usize, n: i32, cause: &str) -> bool {
    if n <= 0 || g.turn != seat {
        return false;
    }
    let Some(lord) = g
        .seats
        .iter()
        .position(|s| s.identity == "lord" && active_key(s, "jiuyuan"))
    else {
        return false;
    };
    if lord == seat || g.seats[seat].kingdom != "wu" || g.seats[seat].hp < g.seats[lord].hp {
        return false;
    }
    if g.seats[lord].hp >= g.seats[lord].max_hp || g.seats[lord].hp <= 0 {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "jiuyuan",
        Stage::Choose,
        seat,
        json!({ "n": n, "cause": cause, "lord": lord }),
    );
    true
}

fn active_key(s: &crate::kernel::Seat, code: &str) -> bool {
    s.skills.iter().any(|x| x == code) || s.granted.iter().any(|x| x == code)
}

fn jiuyuan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    let lord = a.data["lord"].as_u64().unwrap_or(0) as usize;
    let n = a.data["n"].as_i64().unwrap_or(1) as i32;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(seat, "choice").skill("jiuyuan");
            b = b.option("opt1", format!("改为令主公 {} 回复 1 点体力，然后你摸 1 张", g.seats[lord].name));
            Ok(Step::Need(b.escape("照常自己回复").build()))
        }
        Some("pass") => {
            // 照常自己回复（绕过救援钩子，避免递归）。
            let healed = (g.seats[seat].hp + n).min(g.seats[seat].max_hp);
            let d = healed - g.seats[seat].hp;
            g.seats[seat].hp = healed;
            if d > 0 {
                g.events.push(crate::kernel::event::Event::with_amount(
                    crate::kernel::event::EventKind::Healed,
                    Some(seat),
                    Some("jiuyuan_self"),
                    d,
                ));
                g.push_log(format!("{} 回复 {d} 点体力", g.seats[seat].name));
            }
            Ok(Step::Done)
        }
        Some(_) => {
            flow::heal(g, lord, 1, "jiuyuan");
            g.draw(seat, 1);
            g.push_log(format!(
                "救援：{} 改为令主公回复 1 点体力，摸 1 张",
                g.seats[seat].name
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 枭姬（界孙尚香）：失去装备区的牌 → 摸 2
// ---------------------------------------------------------------------------

pub const XIAOJI: SkillDef = SkillDef {
    code: "xiaoji",
    name: "枭姬",
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

pub fn xiaoji_on_lose_equip(g: &mut Game, seat: usize) {
    if !active(g, seat, "xiaoji") || g.seats[seat].hp <= 0 {
        return;
    }
    let n = g.draw(seat, 2).len();
    g.push_log(format!("枭姬：失去装备，摸 {n} 张"));
}

// ---------------------------------------------------------------------------
// 结姻（界孙尚香）
// ---------------------------------------------------------------------------

pub const JIEYIN: SkillDef = SkillDef {
    code: "jieyin",
    name: "结姻",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jieyin),
    handler: jieyin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_jieyin(g: &Game, seat: usize) -> bool {
    active(g, seat, "jieyin")
        && play_phase(g, seat)
        && g.counter("turn.jieyin_used") == 0
        && other_alive(g, seat)
            .into_iter()
            .any(|i| g.seats[i].is_male())
}

fn jieyin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("target").to_string();
    match step.as_str() {
        "target" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(a.actor, "target").skill("jieyin");
                for i in other_alive(g, a.actor) {
                    if g.seats[i].is_male() {
                        b = b.option(format!("target:{i}"), format!("与 {} 结姻", g.seats[i].name));
                    }
                }
                Ok(Step::Need(b.escape("不发动结姻").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("结姻：目标不合法 {act}"))?;
                if t == a.actor || !g.seats[t].is_male() || g.seats[t].hp <= 0 {
                    return Err("结姻：目标必须是男性角色".into());
                }
                Ok(Step::Next(Stage::Mode, json!({ "step": "mode", "target": t })))
            }
        },
        "mode" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let has_equip_in_hand = g
                .hand(a.actor)
                .into_iter()
                .any(|id| g.def_of(&id).kind == CardKind::Equip);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("jieyin");
                    if !g.hand(a.actor).is_empty() {
                        b = b.option("opt1", "弃置一张手牌");
                    }
                    if has_equip_in_hand || !equip_ids(g, a.actor).is_empty() {
                        b = b.option("opt2", "将一张装备牌置入其装备区");
                    }
                    Ok(Step::Need(b.escape("不发动结姻").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(opt) => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "card", "target": target, "opt": opt }),
                )),
            }
        }
        "card" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let opt = a.data["opt"].as_str().unwrap_or("opt1").to_string();
            let cands: Vec<CardId> = if opt == "opt1" {
                g.hand(a.actor)
            } else {
                hand_and_equip(g, a.actor)
                    .into_iter()
                    .filter(|id| g.def_of(id).kind == CardKind::Equip)
                    .collect()
            };
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(a.actor, "choice").skill("jieyin");
                    for id in &cands {
                        b = b.option(format!("pick:{id}"), format!("选择【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不发动结姻").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("结姻：未知选项 {act}"))?
                        .to_string();
                    if !cands.contains(&id) {
                        return Err("结姻：该牌不可用".into());
                    }
                    g.bump("turn.jieyin_used");
                    if opt == "opt1" {
                        g.to_discard(&id, "jieyin")?;
                        g.push_log(format!("结姻：弃置【{}】", label(g, &id)));
                    } else {
                        g.gain(&id, target)?;
                        let s = &mut g.seats[target];
                        let _ = s;
                        if let Some(slot) = g.def_of(&id).slot {
                            g.to_equip(&id, target, slot)?;
                        } else {
                            g.to_hand(&id, target)?;
                        }
                        g.push_log(format!(
                            "结姻：将装备【{}】置入 {} 的装备区",
                            label(g, &id),
                            g.seats[target].name
                        ));
                    }
                    // 体力比较：大者摸 1，小者回 1，相等则都摸 1 且都回 1。
                    let a_hp = g.seats[a.actor].hp;
                    let b_hp = g.seats[target].hp;
                    if a_hp == b_hp {
                        g.draw(a.actor, 1);
                        g.draw(target, 1);
                        flow::heal(g, a.actor, 1, "jieyin");
                        flow::heal(g, target, 1, "jieyin");
                    } else if a_hp > b_hp {
                        g.draw(a.actor, 1);
                        flow::heal(g, target, 1, "jieyin");
                    } else {
                        g.draw(target, 1);
                        flow::heal(g, a.actor, 1, "jieyin");
                    }
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("结姻：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 注册表 + 钩子汇总
// ---------------------------------------------------------------------------

pub const B10_SKILLS: &[SkillDef] = &[
    QIXI,
    FENWEI,
    KEJI,
    QINXUE,
    GONGXIN,
    BOTU,
    KUROU,
    ZHAXIANG,
    YINGZI,
    FANJIAN,
    LIULI,
    GUOSE,
    QIANXUN,
    LIANYING,
    ZHIHENG,
    JIUYUAN,
    XIAOJI,
    JIEYIN,
];

pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    qixi_conversions(g, seat)
}
