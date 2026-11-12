//! B3（马谡 / 荀攸 / 钟会 / 满宠 / 关平）——按 `docs/games/sanguosha/align/B3.json` 第 7 段实现。
//!
//! 本批验证的机制：
//! - **造成伤害 ≠ 失去体力**（`damage-vs-hp-loss`）：权计/智愚/御策只订阅 `DamageAfter`；
//!   失去体力走 `flow::lose_hp`（不产生伤害事件），减少上限走 `flow::lose_max_hp`（独立事件）。
//! - **卡牌属性**（`card-attributes`）：类型/子类/颜色只读 `cards::def` 与物理牌花色。
//! - **virtual-card**：奇策把全部手牌作为一张虚拟牌的 origin（可追溯），结算后整组进弃牌堆。
//! - **「不计次数」用标记**：龙吟给该【杀】打 `uncounted`，不回退任何全局计数。
//! - **「可以」= 两选项**：所有可选技能都有 `pass` 出口；觉醒技（自立）把两个真实收益当出口。

use serde_json::json;

use crate::cards::{self, CardKind, TargetMode};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{self, Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, quan, SkillDef, SkillKind};

// ---------------------------------------------------------------------------
// 通用小工具（都只读卡牌属性 / 派生查询，不按牌名分支）
// ---------------------------------------------------------------------------

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn kind_bit_of(kind: CardKind) -> u32 {
    match kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    }
}

/// 该牌的类型位（基本 1 / 锦囊 2 / 装备 4）；虚拟牌读生效定义。
fn kind_bit(g: &Game, id: &CardId) -> u32 {
    kind_bit_of(g.def_of(id).kind)
}

fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat)));
    out
}

fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}

/// 区域里的牌（手牌 + 装备 + 判定）；制蛮的可获取范围。
fn zone_cards(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = hand_and_equip(g, seat);
    out.extend(g.judge_zone(seat));
    out
}

fn alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect()
}

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

// ---------------------------------------------------------------------------
// 散谣（马谡）：出牌阶段各限一次，弃一张牌（手牌/装备区）对最大者造成 1 点伤害
// ---------------------------------------------------------------------------

pub const SANYAO: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "sanyao",
    name: "散谣",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_sanyao),
    handler: sanyao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

const MODES: [&str; 2] = ["hp", "hand"];

fn mode_counter(mode: &str) -> String {
    format!("turn.sanyao_{mode}")
}

/// 某模式下「最大者」的候选（**含自己**：用户 2026-11-06 裁定 Q1=B）。
fn mode_candidates(g: &Game, mode: &str) -> Vec<usize> {
    let alive = alive(g);
    if alive.is_empty() {
        return Vec::new();
    }
    let key = |i: usize| -> i32 {
        if mode == "hp" {
            g.seats[i].hp
        } else {
            g.hand(i).len() as i32
        }
    };
    let best = alive.iter().map(|i| key(*i)).max().unwrap_or(0);
    alive.into_iter().filter(|i| key(*i) == best).collect()
}

fn unused_modes(g: &Game, _seat: usize) -> Vec<&'static str> {
    MODES
        .into_iter()
        .filter(|m| g.counter(&mode_counter(m)) == 0 && !mode_candidates(g, m).is_empty())
        .collect()
}

fn can_sanyao(g: &Game, seat: usize) -> bool {
    active(g, seat, "sanyao")
        && play_phase(g, seat)
        && !hand_and_equip(g, seat).is_empty()
        && !unused_modes(g, seat).is_empty()
}

fn sanyao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let modes = unused_modes(g, a.actor);
            if modes.is_empty() || hand_and_equip(g, a.actor).is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("sanyao");
                    for m in &modes {
                        b = b.option(
                            format!("choose:{m}"),
                            if *m == "hp" {
                                "散谣：弃一张牌，对体力值最大的角色造成 1 点伤害".to_string()
                            } else {
                                "散谣：弃一张牌，对手牌数最多的角色造成 1 点伤害".to_string()
                            },
                        );
                    }
                    Ok(Step::Need(b.escape("不发动散谣").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let mode = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("散谣：未知选项 {act}"))?
                        .to_string();
                    if !modes.contains(&mode.as_str()) {
                        return Err(format!("散谣：该模式当前不可发动（{mode}）"));
                    }
                    Ok(Step::Next(Stage::DiscardOwner, json!({ "mode": mode })))
                }
            }
        }
        Stage::DiscardOwner => {
            let mode = a.data["mode"].as_str().unwrap_or("hp").to_string();
            let cards = hand_and_equip(g, a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("sanyao");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("弃置 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不弃置（放弃散谣）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("散谣：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("散谣：只能弃置自己的手牌或装备区牌".into());
                    }
                    g.to_discard(&id, "sanyao")?;
                    Ok(Step::Next(Stage::Target, json!({ "mode": mode })))
                }
            }
        }
        Stage::Target => {
            let mode = a.data["mode"].as_str().unwrap_or("hp").to_string();
            // 代价已付：候选按弃置后的局面重算。
            let cands = mode_candidates(g, &mode);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("sanyao");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("散谣：对 {} 造成 1 点伤害", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃选择目标（代价不退回）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("散谣：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("散谣：目标不是最大者".into());
                    }
                    g.bump(&mode_counter(&mode));
                    g.presentation.push(
                        Presentation::new("sanyao", POutcome::Damage { amount: 1 })
                            .actor(a.actor)
                            .target(target)
                            .fx("sanyao"),
                    );
                    g.push_log(format!(
                        "散谣（{}）：对 {} 造成 1 点伤害",
                        if mode == "hp" { "体力值最大" } else { "手牌数最多" },
                        g.seats[target].name
                    ));
                    flow::damage(g, Some(a.actor), target, 1);
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("散谣：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 制蛮（马谡）：造成伤害时（结算之前）可防止并获得其区域里一张牌
// ---------------------------------------------------------------------------

pub const ZHIMAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhiman",
    name: "制蛮",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: zhiman,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 「造成伤害时」拦截：返回 true = 已压入询问帧、本次伤害暂不结算。
///
/// 裁定（B3 Q1=A）：目标只有判定区有牌时**不能发动** → 前置 = 手牌或装备区非空。
pub fn zhiman_before(g: &mut Game, source: usize, victim: usize, amount: i32) -> bool {
    if !active(g, source, "zhiman") || amount <= 0 {
        return false;
    }
    if g.hand(victim).is_empty() && equip_ids(g, victim).is_empty() {
        return false;
    }
    resolve::push_ask(
        g,
        "zhiman",
        Stage::Choose,
        source,
        json!({ "victim": victim, "amount": amount }),
    );
    true
}

fn zhiman(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    let amount = a.data["amount"].as_i64().unwrap_or(0) as i32;
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("zhiman")
                    .option(
                        "choose:zhiman",
                        format!("制蛮：防止此伤害，然后获得 {} 一张牌", g.seats[victim].name),
                    )
                    .escape("不发动制蛮")
                    .build(),
            )),
            Some("pass") => {
                // 不发动：按原伤害立即结算（`damage_now` 不再走前置钩子，避免递归）。
                flow::damage_now(g, Some(a.actor), victim, amount, None);
                Ok(Step::Done)
            }
            Some("choose:zhiman") => {
                Ok(Step::Next(Stage::Target, json!({ "victim": victim, "amount": amount })))
            }
            Some(other) => Err(format!("制蛮：未知选项 {other}")),
        },
        Stage::Target => {
            let cards = zone_cards(g, victim);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("zhiman");
                    for id in &cards {
                        let zone = match g.zone_of(id) {
                            Some(Zone::Judge(_)) => "判定区",
                            Some(Zone::Equip(_, _)) => "装备区",
                            _ => "手牌",
                        };
                        b = b.option(
                            format!("choose:{id}"),
                            format!("获得其{zone}的 {}", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃获得（伤害已防止）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("制蛮：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("制蛮：只能获得其区域里的牌".into());
                    }
                    g.gain(&id, a.actor)?;
                    g.presentation.push(
                        Presentation::new("zhiman", POutcome::Activated {
                            skill: "制蛮".to_string(),
                        })
                        .actor(a.actor)
                        .target(victim)
                        .cards(vec![id])
                        .fx("zhiman"),
                    );
                    g.push_log(format!(
                        "制蛮：防止伤害并获得 {} 一张牌（伤害不再发生）",
                        g.seats[victim].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("制蛮：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 奇策（荀攸）：所有手牌当任意非延迟锦囊使用
// ---------------------------------------------------------------------------

pub const QICE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qice",
    name: "奇策",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_qice),
    handler: qice,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: Some(qice_wuxie_opts),
    wuxie_use: Some(qice_wuxie_use),
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_qice(g: &Game, seat: usize) -> bool {
    active(g, seat, "qice")
        && play_phase(g, seat)
        && g.counter("turn.qice_used") == 0
        && !g.hand(seat).is_empty()
        && !qice_names().is_empty()
}

/// 奇策可声明的牌名：**非延迟锦囊**（属性过滤）。【无懈可击】走无懈链入口（见下）。
fn qice_names() -> Vec<&'static str> {
    cards::non_delayed_tricks()
        .into_iter()
        .filter(|n| !cards::tags(n).wuxie)
        .collect()
}

fn qice(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let names = qice_names();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("qice");
                    for n in &names {
                        b = b.option(
                            format!("choose:{n}"),
                            format!("奇策：将全部手牌当【{n}】使用"),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动奇策").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let name = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("奇策：未知选项 {act}"))?
                        .to_string();
                    if !names.contains(&name.as_str()) {
                        return Err(format!("奇策：{name} 不是非延迟锦囊"));
                    }
                    let mode = cards::target_mode(&name);
                    if mode == TargetMode::Chosen {
                        Ok(Step::Next(Stage::Target, json!({ "name": name })))
                    } else {
                        let targets = if mode == TargetMode::AllAlive {
                            alive(g)
                        } else {
                            vec![a.actor]
                        };
                        resolve_qice(g, a.actor, &name, targets)?;
                        Ok(Step::Done)
                    }
                }
            }
        }
        Stage::Target => {
            let name = a.data["name"].as_str().unwrap_or_default().to_string();
            let cands = other_alive(g, a.actor);
            if cands.is_empty() {
                // 无合法目标 = 未进入结算：origin 整组进弃牌堆，**不计入**本阶段使用。
                return finish_qice_without_resolution(g, a.actor);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("qice");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("对 {} 使用【{name}】", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("取消（手牌作废，不计入次数）").build()))
                }
                Some("pass") => finish_qice_without_resolution(g, a.actor),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("奇策：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("奇策：目标不合法".into());
                    }
                    resolve_qice(g, a.actor, &name, vec![target])?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("奇策：未知阶段".into()),
    }
}

/// 把全部手牌作为一张虚拟牌使用；origin 整组在结算后进弃牌堆（`virtual-card` 不变量）。
fn resolve_qice(g: &mut Game, actor: usize, name: &str, targets: Vec<usize>) -> Result<(), String> {
    let origins = g.hand(actor);
    let Some(base) = origins.first().cloned() else {
        return Err("奇策：没有手牌".into());
    };
    // M2：真实结算（承载牌 + 全部手牌 origins 在结算后一并进弃牌堆）。
    g.set_virtual(&base, name, origins.clone());
    g.use_as(actor, &base, name, targets, true, None)?;
    g.bump("turn.qice_used");
    g.presentation.push(
        Presentation::new("qice", POutcome::Activated {
            skill: "奇策".to_string(),
        })
        .actor(actor)
        .cards(origins.clone())
        .fx("qice"),
    );
    g.push_log(format!(
        "奇策：将 {} 张手牌当【{name}】使用",
        origins.len()
    ));
    Ok(())
}

/// 未进入结算（无目标 / 被取消）：手牌仍作废，但**不计入**「本阶段已使用」。
fn finish_qice_without_resolution(g: &mut Game, actor: usize) -> Result<Step, String> {
    let origins = g.hand(actor);
    for id in &origins {
        g.to_discard(id, "qice_cancel")?;
    }
    g.push_log(format!(
        "奇策：未进入结算（{} 张手牌作废，不计入本阶段次数）",
        origins.len()
    ));
    Ok(Step::Done)
}

/// 无懈链里的奇策：只在自己的出牌阶段可把全部手牌当【无懈可击】。
fn qice_wuxie_ready(g: &Game, seat: usize) -> bool {
    active(g, seat, "qice")
        && play_phase(g, seat)
        && g.counter("turn.qice_used") == 0
        && !g.hand(seat).is_empty()
}

fn qice_wuxie_opts(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !qice_wuxie_ready(g, seat) {
        return Vec::new();
    }
    vec![(
        "qice:wuxie".to_string(),
        "奇策：将全部手牌当【无懈可击】使用".to_string(),
    )]
}

fn qice_wuxie_use(g: &mut Game, seat: usize, action: &str) -> Result<bool, String> {
    if action != "qice:wuxie" {
        return Ok(false);
    }
    if !qice_wuxie_ready(g, seat) {
        return Err("奇策：当前不能当【无懈可击】使用".into());
    }
    let origins = g.hand(seat);
    let base = origins.first().cloned().ok_or("奇策：没有手牌")?;
    g.set_virtual(&base, "无懈可击", origins.clone());
    for id in &origins {
        g.to_discard(id, "qice_wuxie")?;
    }
    g.clear_virtual(&base);
    g.bump("turn.qice_used");
    g.push_log(format!(
        "奇策：将 {} 张手牌当【无懈可击】使用",
        origins.len()
    ));
    Ok(true)
}

// ---------------------------------------------------------------------------
// 智愚（荀攸）：受到伤害后，可以摸 1 张并展示手牌；颜色均相同则来源弃 1 张手牌
// ---------------------------------------------------------------------------

pub const ZHIYU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhiyu",
    name: "智愚",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 30,
    can_active: None,
    handler: zhiyu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhiyu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 只在触发阶段校验「主角/点数」，后续阶段 data 已换成 source。
    if a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if a.data["amount"].as_i64().unwrap_or(0) <= 0 || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("zhiyu")
                    .option("choose:zhiyu", "智愚：摸 1 张牌，然后展示所有手牌")
                    .escape("不发动智愚")
                    .build(),
            )),
            Some("pass") => Ok(Step::Done),
            Some("choose:zhiyu") => {
                g.draw(a.actor, 1);
                let hand = g.hand(a.actor);
                let reds: Vec<bool> = hand
                    .iter()
                    .map(|id| g.is_red(id))
                    .collect();
                let same = !reds.is_empty() && reds.iter().all(|r| *r == reds[0]);
                // 「展示」= **立即**进公共可见区（mechanics `reveal`，用户 2026-11-06 裁定）。
                for id in &hand {
                    g.reveal(id, Some(a.actor), "zhiyu");
                }
                let shown: Vec<String> = hand.iter().map(|id| label(g, id)).collect();
                g.push_log(format!(
                    "智愚：{} 展示手牌（{}）",
                    g.seats[a.actor].name,
                    shown.join("、")
                ));
                g.presentation.push(
                    Presentation::new("zhiyu", POutcome::Draw { n: 1 })
                        .actor(a.actor)
                        .cards(hand)
                        .fx("zhiyu_reveal"),
                );
                let source = a
                    .data
                    .get("cause")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<usize>().ok());
                let Some(source) = source else {
                    return Ok(Step::Done);
                };
                if !same || source == a.actor || g.hand(source).is_empty() {
                    return Ok(Step::Done);
                }
                Ok(Step::Next(Stage::DiscardTarget, json!({ "source": source })))
            }
            Some(other) => Err(format!("智愚：未知选项 {other}")),
        },
        Stage::DiscardTarget => {
            let source = a.data["source"].as_u64().unwrap_or(0) as usize;
            let cards = g.hand(source);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(source, "choice").skill("zhiyu");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("智愚：弃置一张手牌 {}", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不弃置（视为无牌可弃）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("智愚：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("智愚：只能弃置自己的手牌".into());
                    }
                    for c in g.hand(a.actor).iter().chain([].iter()) {
                        let _ = c;
                    }
                    g.to_discard(&id, "zhiyu")?;
                    g.presentation.push(
                        Presentation::new("zhiyu", POutcome::Discard { n: 1 })
                            .actor(source)
                            .target(a.actor)
                            .fx("zhiyu_discard"),
                    );
                    g.push_log(format!(
                        "智愚：颜色均相同，{} 弃置一张手牌",
                        g.seats[source].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("智愚：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 权计（钟会）：受到 1 点伤害后，可以摸 1 张，然后将一张手牌置为「权」（移出游戏）
// ---------------------------------------------------------------------------

pub const QUANJI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "quanji",
    name: "权计",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 40,
    can_active: None,
    handler: quanji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn quanji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let amount = a.data["amount"].as_i64().unwrap_or(0) as i64;
        if amount <= 0 || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
    }
    // 触发时没有 `remaining`：第一次询问 = 本次伤害点数（逐点各一次）。
    let remaining = a
        .data
        .get("remaining")
        .and_then(|v| v.as_i64())
        .or_else(|| a.data["amount"].as_i64())
        .unwrap_or(0);
    match a.stage {
        Stage::Choose => {
            if remaining <= 0 {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("quanji")
                        .option(
                            "choose:quanji",
                            "权计：摸 1 张牌，然后将一张手牌置于武将牌上为「权」",
                        )
                        .escape("不发动权计")
                        .build(),
                )),
                Some("pass") => next_quanji_point(g, remaining - 1),
                Some("choose:quanji") => {
                    g.draw(a.actor, 1);
                    g.presentation.push(
                        Presentation::new("quanji", POutcome::Draw { n: 1 })
                            .actor(a.actor)
                            .fx("quanji_draw"),
                    );
                    Ok(Step::Next(Stage::PickOwn, json!({ "remaining": remaining })))
                }
                Some(other) => Err(format!("权计：未知选项 {other}")),
            }
        }
        Stage::PickOwn => {
            let cards = g.hand(a.actor);
            if cards.is_empty() {
                return next_quanji_point(g, remaining - 1);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("quanji");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("将 {} 置于武将牌上为「权」", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃置「权」").build()))
                }
                Some("pass") => next_quanji_point(g, remaining - 1),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("权计：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("权计：只能置自己的手牌".into());
                    }
                    // 「权」按移出游戏处理（用户 2026-11-06 裁定 Q3=B，与不屈「创」同口径）。
                    g.to_void(&id)?;
                    super::add_quan(g, a.actor, 1);
                    g.presentation.push(
                        Presentation::new("quanji", POutcome::MarkPlaced { n: 1 })
                            .actor(a.actor)
                            .cards(vec![id.clone()])
                            .fx("quanji_quan"),
                    );
                    g.push_log(format!(
                        "权计：将 {} 置于武将牌上为「权」（共 {}，手牌上限 {}）",
                        label(g, &id),
                        quan(g, a.actor),
                        g.hand_limit(a.actor)
                    ));
                    next_quanji_point(g, remaining - 1)
                }
            }
        }
        _ => Err("权计：未知阶段".into()),
    }
}

fn next_quanji_point(_g: &mut Game, remaining: i64) -> Result<Step, String> {
    if remaining > 0 {
        Ok(Step::Next(Stage::Choose, json!({ "remaining": remaining })))
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 自立（钟会·觉醒）：准备阶段「权」≥3 → 回复 1 / 摸 2，然后减上限并获得「排异」
// ---------------------------------------------------------------------------

pub const ZILI: SkillDef = SkillDef { kind: SkillKind::Awaken,
    code: "zili",
    name: "自立",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 10,
    can_active: None,
    handler: zili,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zili(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("Prepare") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if g.seats[a.actor].flags.contains("zili_done") || quan(g, a.actor) < 3 {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let wounded = g.seats[a.actor].hp < g.seats[a.actor].max_hp;
                // 觉醒技锁定触发、不可放弃：出口就是两个真实收益里的「摸 2 张」。
                let mut b = super::builder(a.actor, "choice").skill("zili");
                if wounded {
                    b = b.option(
                        "opt1",
                        "自立：回复 1 点体力，然后减 1 点体力上限并获得「排异」",
                    );
                } else {
                    // 裁定 Q1=B：满体力时隐藏「回复」项。
                }
                Ok(Step::Need(
                    b.escape_with(
                        "opt2",
                        "自立：摸 2 张牌，然后减 1 点体力上限并获得「排异」",
                    )
                    .build(),
                ))
            }
            Some(act @ ("opt1" | "opt2")) => {
                let actor = a.actor;
                if act == "opt1" {
                    flow::heal(g, actor, 1, "zili");
                    g.push_log(format!("自立：回复 1 点体力（体力 {}）", g.seats[actor].hp));
                } else {
                    let n = g.draw(actor, 2).len();
                    g.presentation.push(
                        Presentation::new("zili", POutcome::Draw { n: n as u32 })
                            .actor(actor)
                            .fx("zili_draw"),
                    );
                    g.push_log(format!("自立：摸 {n} 张牌"));
                }
                // 「然后」：减 1 点体力上限（独立事件）→ 获得「排异」。
                flow::lose_max_hp(g, actor, 1, "zili");
                if !g.seats[actor].granted.iter().any(|s| s == "paiyi") {
                    g.seats[actor].granted.push("paiyi".to_string());
                }
                crate::skills::rebuild_subs(g);
                g.seats[actor].flags.insert("zili_done".to_string());
                g.presentation.push(
                    Presentation::new("zili", POutcome::Activated {
                        skill: "自立".to_string(),
                    })
                    .actor(actor)
                    .fx("zili_awaken"),
                );
                g.push_log("自立：觉醒完成，获得「排异」".to_string());
                Ok(Step::Done)
            }
            Some(other) => Err(format!("自立：未知选项 {other}")),
        },
        _ => Err("自立：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 峻刑（满宠）：弃任意张手牌 → 一名其他角色二选一
// ---------------------------------------------------------------------------

pub const JUNXING: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "junxing",
    name: "峻刑",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_junxing),
    handler: junxing,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_junxing(g: &Game, seat: usize) -> bool {
    active(g, seat, "junxing")
        && play_phase(g, seat)
        && g.counter("turn.junxing_used") == 0
        && !g.hand(seat).is_empty()
        && !other_alive(g, seat).is_empty()
}

fn junxing(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let hand = g.hand(a.actor);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            let selected: Vec<CardId> = a.data["selected"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("junxing");
                    for id in &hand {
                        let mark = if selected.contains(id) { "✔ " } else { "" };
                        b = b.option(
                            format!("choose:{id}"),
                            format!("{mark}{}", label(g, id)),
                        );
                    }
                    if !selected.is_empty() {
                        b = b.option("confirm", format!("确认弃置 {} 张", selected.len()));
                    }
                    Ok(Step::Need(b.escape("不发动峻刑").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("confirm") => {
                    if selected.is_empty() {
                        return Err("峻刑：至少弃置 1 张手牌".into());
                    }
                    let mut kinds = 0u32;
                    for id in &selected {
                        kinds |= kind_bit(g, id);
                        g.to_discard(id, "junxing")?;
                    }
                    g.bump("turn.junxing_used");
                    g.presentation.push(
                        Presentation::new("junxing", POutcome::Discard { n: selected.len() as u32 })
                            .actor(a.actor)
                            .fx("junxing_cost"),
                    );
                    g.push_log(format!("峻刑：弃置 {} 张手牌", selected.len()));
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "kinds": kinds, "n": selected.len() }),
                    ))
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("峻刑：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("峻刑：只能弃置自己的手牌".into());
                    }
                    let mut next = selected.clone();
                    match next.iter().position(|c| c == &id) {
                        Some(pos) => {
                            next.remove(pos);
                        }
                        None => next.push(id),
                    }
                    Ok(Step::Next(Stage::Choose, json!({ "selected": next })))
                }
            }
        }
        Stage::Target => {
            let kinds = a.data["kinds"].as_u64().unwrap_or(0) as u32;
            let n = a.data["n"].as_u64().unwrap_or(0) as usize;
            // 裁定 Q1=A：目标**不要求**有手牌。
            let cands = other_alive(g, a.actor);
            if cands.is_empty() {
                g.push_log("峻刑：没有可选目标（代价不退）".to_string());
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("junxing");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 二选一", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消（代价不退）").build()))
                }
                Some("pass") => {
                    g.push_log("峻刑：取消（代价不退）".to_string());
                    Ok(Step::Done)
                }
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("峻刑：目标不合法 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("峻刑：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "kinds": kinds, "n": n, "target": target }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let kinds = a.data["kinds"].as_u64().unwrap_or(0) as u32;
            let n = a.data["n"].as_u64().unwrap_or(0) as usize;
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let legal: Vec<CardId> = g
                .hand(target)
                .into_iter()
                .filter(|id| kind_bit(g, id) & kinds == 0)
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(target, "choice").skill("junxing");
                    if !legal.is_empty() {
                        b = b.option(
                            "opt1",
                            "弃置一张与你弃置牌类型皆不同的手牌",
                        );
                    }
                    Ok(Step::Need(
                        b.escape_with("opt2", format!("翻面，然后摸 {n} 张牌"))
                            .build(),
                    ))
                }
                Some("opt1") => {
                    if legal.is_empty() {
                        return Err("峻刑：没有类型不同的手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::DiscardTarget,
                        json!({ "kinds": kinds, "n": n, "target": target }),
                    ))
                }
                Some("opt2") => {
                    // 裁定 Q2=A：照字面翻面（本就背面朝上 → 翻回正面）。
                    g.flip(target);
                    let drawn = g.draw(target, n).len();
                    g.presentation.push(
                        Presentation::new("junxing", POutcome::Draw { n: drawn as u32 })
                            .actor(target)
                            .target(target)
                            .fx("junxing_flip_draw"),
                    );
                    g.push_log(format!(
                        "峻刑：{} 翻面并摸 {drawn} 张牌",
                        g.seats[target].name
                    ));
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("峻刑：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let kinds = a.data["kinds"].as_u64().unwrap_or(0) as u32;
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let legal: Vec<CardId> = g
                .hand(target)
                .into_iter()
                .filter(|id| kind_bit(g, id) & kinds == 0)
                .collect();
            if legal.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(target, "choice").skill("junxing");
                    for id in &legal {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {}", label(g, id)),
                        );
                    }
                    // 已选「弃置」= 承诺弃置一张：强制选择，不得反悔（I1）。
                    Ok(Step::Need(b.escape_mandatory("（必须弃置一张）").build()))
                }
                Some("pass") => Err("峻刑：已选择弃置，必须弃置一张".into()),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("峻刑：未知选项 {act}"))?
                        .to_string();
                    if !legal.contains(&id) {
                        return Err("峻刑：该牌类型不合法".into());
                    }
                    g.to_discard(&id, "junxing")?;
                    g.presentation.push(
                        Presentation::new("junxing", POutcome::Discard { n: 1 })
                            .actor(target)
                            .fx("junxing_discard"),
                    );
                    g.push_log(format!(
                        "峻刑：{} 弃置一张类型不同的手牌",
                        g.seats[target].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("峻刑：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 御策（满宠）：受到伤害后，可以展示一张手牌令来源二选一
// ---------------------------------------------------------------------------

pub const YUCE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "yuce",
    name: "御策",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 35,
    can_active: None,
    handler: yuce,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yuce(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if a.data["amount"].as_i64().unwrap_or(0) <= 0 || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
        if g.hand(a.actor).is_empty() {
            return Ok(Step::Done);
        }
        let source = a
            .data
            .get("cause")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok());
        let Some(source) = source else {
            return Ok(Step::Done);
        };
        if source == a.actor {
            return Ok(Step::Done);
        }
        // 裁定 Q1/Q3：来源无手牌 → 不询问，直接回复 1 点；此时若体力已满则不能发动。
        if g.hand(source).is_empty() {
            if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                flow::heal(g, a.actor, 1, "yuce");
                g.presentation.push(
                    Presentation::new("yuce", POutcome::Heal { amount: 1 })
                        .actor(a.actor)
                        .fx("yuce_auto_heal"),
                );
                g.push_log(format!(
                    "御策：来源无手牌可弃，{} 回复 1 点体力（体力 {}）",
                    g.seats[a.actor].name, g.seats[a.actor].hp
                ));
            }
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("yuce")
                    .option("choose:yuce", "御策：展示一张手牌，令伤害来源二选一")
                    .escape("不发动御策")
                    .build(),
            )),
            Some("pass") => Ok(Step::Done),
            Some("choose:yuce") => {
                let source = a
                    .data
                    .get("cause")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("御策：缺少伤害来源")?;
                Ok(Step::Next(Stage::PickOwn, json!({ "source": source })))
            }
            Some(other) => Err(format!("御策：未知选项 {other}")),
        },
        Stage::PickOwn => {
            let source = a.data["source"].as_u64().unwrap_or(0) as usize;
            let cards = g.hand(a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose").skill("yuce");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("展示 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不发动御策").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("御策：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("御策：只能展示自己的手牌".into());
                    }
                    // 「展示」= 立即进公共可见区（mechanics `reveal`）。
                    g.reveal(&id, Some(a.actor), "yuce");
                    g.push_log(format!(
                        "御策：{} 展示手牌 {}",
                        g.seats[a.actor].name,
                        label(g, &id)
                    ));
                    g.presentation.push(
                        Presentation::new("yuce", POutcome::Activated {
                            skill: "御策".to_string(),
                        })
                        .actor(a.actor)
                        .target(source)
                        .cards(vec![id.clone()])
                        .fx("yuce_reveal"),
                    );
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "source": source, "card": id }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let source = a.data["source"].as_u64().unwrap_or(0) as usize;
            let shown = a.data["card"].as_str().unwrap_or_default().to_string();
            let shown_bit = kind_bit(g, &shown);
            let legal: Vec<CardId> = g
                .hand(source)
                .into_iter()
                .filter(|id| kind_bit(g, id) != shown_bit)
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(source, "choice").skill("yuce");
                    if !legal.is_empty() {
                        b = b.option("opt1", "弃置一张与展示牌类型不同的手牌");
                    }
                    Ok(Step::Need(
                        b.escape_with(
                            "opt2",
                            format!("令 {} 回复 1 点体力", g.seats[a.actor].name),
                        )
                        .build(),
                    ))
                }
                Some("opt1") => {
                    if legal.is_empty() {
                        return Err("御策：没有类型不同的手牌".into());
                    }
                    Ok(Step::Next(
                        Stage::DiscardTarget,
                        json!({ "source": source, "card": shown }),
                    ))
                }
                Some("opt2") => {
                    if let Some(shown) = a.data.get("card").and_then(|v| v.as_str()) {
                        g.conceal(&shown.to_string(), "yuce_done");
                    }
                    if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                        flow::heal(g, a.actor, 1, "yuce");
                    }
                    let hp = g.seats[a.actor].hp;
                    g.presentation.push(
                        Presentation::new("yuce", POutcome::Heal { amount: 1 })
                            .actor(a.actor)
                            .fx("yuce_heal"),
                    );
                    g.push_log(format!(
                        "御策：{} 回复 1 点体力（体力 {hp}）",
                        g.seats[a.actor].name
                    ));
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("御策：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let source = a.data["source"].as_u64().unwrap_or(0) as usize;
            let shown = a.data["card"].as_str().unwrap_or_default().to_string();
            let shown_bit = kind_bit(g, &shown);
            let legal: Vec<CardId> = g
                .hand(source)
                .into_iter()
                .filter(|id| kind_bit(g, id) != shown_bit)
                .collect();
            if legal.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(source, "choice").skill("yuce");
                    for id in &legal {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {}", label(g, id)),
                        );
                    }
                    // 已选「弃置」= 承诺弃置一张：强制选择，不得反悔（真机 2026-11-12：
                    // 孙策两次选选项1 都没有弃牌，就是因为这里给了「不弃置」出口）。
                    Ok(Step::Need(b.escape_mandatory("（必须弃置一张）").build()))
                }
                Some("pass") => Err("御策：已选择弃置，必须弃置一张".into()),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("御策：未知选项 {act}"))?
                        .to_string();
                    if !legal.contains(&id) {
                        return Err("御策：该牌类型与展示牌相同".into());
                    }
                    g.to_discard(&id, "yuce")?;
                    if let Some(shown) = a.data.get("card").and_then(|v| v.as_str()) {
                        g.conceal(&shown.to_string(), "yuce_done");
                    }
                    g.presentation.push(
                        Presentation::new("yuce", POutcome::Discard { n: 1 })
                            .actor(source)
                            .target(a.actor)
                            .fx("yuce_discard"),
                    );
                    g.push_log(format!(
                        "御策：{} 弃置一张类型不同的手牌",
                        g.seats[source].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("御策：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 龙吟（关平）：一名角色于其出牌阶段内使用【杀】时，弃一张牌令其不计次数
// ---------------------------------------------------------------------------

pub const LONGYIN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "longyin",
    name: "龙吟",
    triggers: &[EventKind::CardUsed],
    window: Window::AnyTime,
    priority: 50,
    can_active: None,
    handler: longyin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 拥有「龙吟」的座位顺序（当前回合玩家优先，然后左手侧）——裁定 Q3=B：只问第一个。
fn longyin_owner(g: &Game, user: usize) -> Option<usize> {
    let mut order = vec![user];
    order.extend(g.seats_from_left_of(user));
    order.into_iter().find(|i| active(g, *i, "longyin"))
}

fn longyin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("CardUsed") {
            return Ok(Step::Done);
        }
        let user = a.data["seat"].as_u64().map(|n| n as usize).unwrap_or(usize::MAX);
        // 窗口：当一名角色于**其出牌阶段内**使用【杀】
        if user != g.turn || g.phase != Phase::Play {
            return Ok(Step::Done);
        }
        if longyin_owner(g, user) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        if !g.def_of(&card).tags.sha {
            return Ok(Step::Done);
        }
        if hand_and_equip(g, a.actor).is_empty() {
            return Ok(Step::Done);
        }
    }
    let user = a.data["user"]
        .as_u64()
        .or_else(|| a.data["seat"].as_u64())
        .map(|n| n as usize)
        .unwrap_or(usize::MAX);
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    match a.stage {
        Stage::Choose => {
            let cards = hand_and_equip(g, a.actor);
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("longyin");
                    for id in &cards {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置 {}，令此【杀】不计入次数", label(g, id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动龙吟").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("龙吟：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("龙吟：只能弃置自己的手牌或装备区牌".into());
                    }
                    g.to_discard(&id, "longyin")?;
                    // 「不计入此阶段使用次数」= 给该【杀】打标记，**不回退**任何全局计数。
                    g.mark_uncounted(&card);
                    g.presentation.push(
                        Presentation::new("longyin", POutcome::Discard { n: 1 })
                            .actor(a.actor)
                            .cards(vec![id.clone()])
                            .fx("longyin"),
                    );
                    g.push_log(format!(
                        "龙吟：{} 弃置 {}，此【杀】不计入本阶段使用次数（本回合已用【杀】{}）",
                        g.seats[a.actor].name,
                        label(g, &id),
                        g.sha_used(user)
                    ));
                    // 若此【杀】为红色 → 摸 1 张（颜色读物理牌）。
                    if g.is_red(&card) {
                        let n = g.draw(a.actor, 1).len();
                        g.presentation.push(
                            Presentation::new("longyin", POutcome::Draw { n: n as u32 })
                                .actor(a.actor)
                                .fx("longyin_draw"),
                        );
                        g.push_log(format!("龙吟：红色【杀】，{} 摸 {n} 张", g.seats[a.actor].name));
                    }
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("龙吟：未知阶段".into()),
    }
}
