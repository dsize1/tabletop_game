//! B0 七个技能（按 `docs/games/sanguosha/align/B0.md` 的「对齐后描述」实现）。
//!
//! 用户裁定要点（复验 40–45）：
//! - 据守：翻面 = **取反**；翻至正面 → 立即结算解围② → 再摸 4 / 弃 1（装备只能使用）。
//! - 解围：装备区牌当【无懈可击】；翻回正面时可弃 1 移 1（不可移回来源角色）。
//! - 不屈：判定牌 → 点数不同则作为「创」**移出游戏**并回复至 1；重复则进弃牌堆（不算移出/移入）。
//! - 奋激：仅回合外；排除摸牌阶段摸牌；只问座位顺序第一个；**脱离濒死之后**才摸 2。
//! - 行殇：只拿手牌 + 装备区（**不含判定区**）。
//! - 放逐：X = 已损失体力，**无上限**；先翻面（可触发解围）再摸牌。
//! - 颂威：**询问判定者**是否令主公摸 1。

use serde_json::{json, Value};

use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, Game, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, SkillDef, SkillKind};

// ---------------------------------------------------------------------------
// 据守
// ---------------------------------------------------------------------------

pub const JUSHOU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jushou",
    name: "据守",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 1,
    can_active: None,
    handler: jushou,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jushou(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            // 只在结束阶段触发（PhaseStart 的 cause 是阶段名）。
            if a.data["cause"].as_str() != Some("End") {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("jushou")
                        .option("choose:jushou", "据守：翻面并摸 4 张，然后弃 1 张（装备改为使用）")
                        .escape("不发动据守")
                        .build(),
                )),
                Some("pass") => Ok(Step::Done),
                Some("choose:jushou") => {
                    // 取反；若翻至正面，翻面会压入解围②的子帧。
                    g.flip(a.actor);
                    Ok(Step::Next(Stage::AfterFlip, json!({})))
                }
                Some(other) => Err(format!("据守：未知选项 {other}")),
            }
        }
        Stage::AfterFlip => {
            g.draw(a.actor, 4);
            g.presentation.push(
                Presentation::new("jushou", POutcome::Draw { n: 4 })
                    .actor(a.actor)
                    .fx("jushou_draw"),
            );
            if g.hand(a.actor).is_empty() {
                return Ok(Step::Done);
            }
            let mut b = super::builder(a.actor, "choose_many").skill("jushou");
            for id in g.hand(a.actor) {
                b = b.option(
                    format!("choose:{id}"),
                    format!("弃置/使用【{}】", g.card_label(&id)),
                );
            }
            // 对齐单裁定：④ 必须弃置 1 张（装备牌强制使用）；出口只是类型占位，不下发。
            Ok(Step::NeedAt(
                Stage::Target,
                b.escape_mandatory("不弃置（跳过）").build(),
            ))
        }
        Stage::Target => {
            let act = match g.answer.take() {
                None => return Ok(Step::Done),
                Some(act) => act,
            };
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("据守：未知选项 {act}"))?
                .to_string();
            match g.cards.get(&id).and_then(|c| c.slot) {
                Some(slot) => {
                    // 装备牌：只能使用（旧装备进弃牌堆）。
                    let old = g.ids_in(|z| *z == Zone::Equip(a.actor, slot));
                    if let Some(old_id) = old.first() {
                        g.to_discard(old_id, "jushou_replace")?;
                    }
                    g.to_equip(&id, a.actor, slot)?;
                    g.presentation.push(
                        Presentation::new("jushou", POutcome::Activated {
                            skill: "据守".to_string(),
                        })
                        .actor(a.actor)
                        .cards(vec![id])
                        .fx("jushou_use_equip"),
                    );
                }
                None => {
                    g.to_discard(&id, "jushou")?;
                    g.presentation.push(
                        Presentation::new("jushou", POutcome::Discard { n: 1 })
                            .actor(a.actor)
                            .cards(vec![id])
                            .fx("jushou_discard"),
                    );
                }
            }
            Ok(Step::Done)
        }
        _ => Err("据守：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 解围（翻回正面时：弃 1 移 1；装备当无懈）
// ---------------------------------------------------------------------------

pub const JIEWEI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jiewei",
    name: "解围",
    triggers: &[],
    window: Window::AnyTime,
    priority: 2,
    can_active: None,
    handler: jiewei,
    dying_hook: None,
    flip_hook: Some(jiewei_on_face_up),
    wuxie_opts: Some(jiewei_wuxie_options),
    wuxie_use: Some(jiewei_wuxie_use),
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn jiewei_wuxie_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    if !active(g, seat, "jiewei") {
        return Vec::new();
    }
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
        .into_iter()
        .map(|id| {
            (
                format!("opt:{id}"),
                format!("解围：将装备【{}】当【无懈可击】", g.card_label(&id)),
            )
        })
        .collect()
}

/// 翻至正面：若拥有解围 → 压入询问帧（在父帧之上）。
fn jiewei_on_face_up(g: &mut Game, seat: usize) {
    if !active(g, seat, "jiewei") {
        return;
    }
    if g.hand(seat).is_empty() && g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat)).is_empty()
    {
        return;
    }
    crate::kernel::resolve::push_ask(
        g,
        "jiewei",
        Stage::Choose,
        seat,
        json!({ "step": "ask" }),
    );
}

fn jiewei_wuxie_use(g: &mut Game, seat: usize, action: &str) -> Result<bool, String> {
    let Some(id) = action.strip_prefix("opt:") else {
        return Ok(false);
    };
    g.to_discard(&id.to_string(), "jiewei_wuxie")?;
    g.presentation.push(
        Presentation::new("jiewei", POutcome::Activated {
            skill: "解围".to_string(),
        })
        .actor(seat)
        .cards(vec![id.to_string()])
        .fx("jiewei_wuxie"),
    );
    Ok(true)
}

fn jiewei(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("ask").to_string();
    match step.as_str() {
        "ask" => match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("jiewei")
                    .option("choose:jiewei", "解围：弃置 1 张牌，然后移动场上一张牌")
                    .escape("不发动解围")
                    .build(),
            )),
            Some("pass") => Ok(Step::Done),
            Some("choose:jiewei") => Ok(Step::Next(Stage::Choose, json!({ "step": "discard" }))),
            Some(other) => Err(format!("解围：未知选项 {other}")),
        },
        "discard" => {
            let mut hand = g.hand(a.actor);
            let equips = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == a.actor));
            let cands: Vec<String> = {
                let mut v = hand.clone();
                v.extend(equips);
                v
            };
            hand.clear();
            match g.answer.take().as_deref() {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Next(Stage::Choose, json!({ "step": "src" })));
                    }
                    let mut b = super::builder(a.actor, "choice").skill("jiewei");
                    for id in &cands {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("弃置【{}】", g.card_label(id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动解围").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("解围：未知选项 {act}"))?
                        .to_string();
                    g.to_discard(&id, "jiewei")?;
                    Ok(Step::Next(Stage::Choose, json!({ "step": "src" })))
                }
            }
        }
        "src" => {
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| g.seats[*i].hp > 0)
                .filter(|i| {
                    !g.hand(*i).is_empty()
                        || !g.ids_in(|z| matches!(z, Zone::Equip(s, _) if s == i)).is_empty()
                        || !g.judge_zone(*i).is_empty()
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("jiewei");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("来源：{}", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不移动").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let src: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("解围：来源不合法 {act}"))?;
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "pick", "source": src }),
                    ))
                }
            }
        }
        "pick" => {
            let src = a.data["source"].as_u64().unwrap_or(0) as usize;
            let equips = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == src));
            let judges = g.judge_zone(src);
            let has_hand = !g.hand(src).is_empty();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("jiewei");
                    for id in equips.iter().chain(judges.iter()) {
                        b = b.option(
                            format!("move:{id}"),
                            format!("移动【{}】", g.card_label(id)),
                        );
                    }
                    if has_hand {
                        b = b.option("move:hand", "随机移动一张手牌");
                    }
                    Ok(Step::Need(b.escape("不移动").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    if act == "move:hand" {
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "dest", "source": src, "hand": true }),
                        ))
                    } else {
                        let card = act
                            .strip_prefix("move:")
                            .ok_or_else(|| format!("解围：未知选项 {act}"))?
                            .to_string();
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "dest", "source": src, "card": card }),
                        ))
                    }
                }
            }
        }
        "dest" => {
            let src = a.data["source"].as_u64().unwrap_or(0) as usize;
            let is_hand = a.data["hand"].as_bool().unwrap_or(false);
            let card = a.data["card"].as_str().map(|s| s.to_string());
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != src && g.seats[*i].hp > 0)
                .filter(|i| {
                    if is_hand {
                        return true;
                    }
                    let Some(card) = card.as_deref() else {
                        return false;
                    };
                    match g.cards.get(card).and_then(|c| c.slot) {
                        // 装备：目标同槽位必须为空
                        Some(slot) => g
                            .ids_in(|z| *z == Zone::Equip(*i, slot))
                            .is_empty(),
                        // 判定区：目标不得已有同名延时锦囊
                        None => !g
                            .judge_zone(*i)
                            .iter()
                            .any(|c| g.cards.get(c).map(|x| x.name.clone()) == g.cards.get(card).map(|x| x.name.clone())),
                    }
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("jiewei");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("移动到 {}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不移动").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let dest: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("解围：目的地不合法 {act}"))?;
                    if is_hand {
                        let hand = g.hand(src);
                        if let Some(id) = hand.first() {
                            g.to_hand(id, dest)?;
                        }
                    } else if let Some(card) = card {
                        match g.cards.get(&card).and_then(|c| c.slot) {
                            Some(slot) => g.to_equip(&card, dest, slot)?,
                            None => g.to_judge(&card, dest)?,
                        }
                    }
                    g.presentation.push(
                        Presentation::new("jiewei", POutcome::Activated {
                            skill: "解围".to_string(),
                        })
                        .actor(a.actor)
                        .fx("jiewei_move"),
                    );
                    Ok(Step::Done)
                }
            }
        }
        other => Err(format!("解围：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 不屈（濒死锁定技）
// ---------------------------------------------------------------------------

pub const BUQU: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "buqu",
    name: "不屈",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: buqu_noop,
    dying_hook: Some(buqu_dying),
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn buqu_noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

fn buqu_dying(g: &mut Game, seat: usize) -> bool {
    if g.seats[seat].hp > 0 {
        return false;
    }
    let Some(card) = flow::judge_card(g, seat) else {
        return false;
    };
    flow::judge_effective(g, seat, &card);
    let rank = g.cards.get(&card).map(|c| c.rank).unwrap_or(0);
    // 「创」已移出游戏，点数用计数器记录（不复制牌归属）。
    let rank_key = format!("buqu_rank_{rank}");
    if g.seats[seat].counters.get(&rank_key).copied().unwrap_or(0) > 0 {
        // 重复：进弃牌堆（不算移出/移入游戏）。
        let _ = flow::judge_to_discard(g, &card);
        g.push_log(format!(
            "不屈：判定牌点数 {rank} 重复，置入弃牌堆（仍濒死）"
        ));
        return true;
    }
    // 不同：作为「创」移出游戏，回复至 1。
    let _ = g.to_void(&card);
    *g.seats[seat].counters.entry(rank_key).or_insert(0) += 1;
    *g.seats[seat].counters.entry("buqu_total".to_string()).or_insert(0) += 1;
    flow::revive_to(g, seat, 1, "buqu");
    g.presentation.push(
        Presentation::new("buqu", POutcome::Heal { amount: 1 })
            .target(seat)
            .cards(vec![card])
            .fx("buqu_save"),
    );
    g.push_log(format!("不屈：判定牌点数 {rank}，作为「创」移出游戏，回复至 1 点体力"));
    true
}

// ---------------------------------------------------------------------------
// 奋激
// ---------------------------------------------------------------------------

pub const FENJI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "fenji",
    name: "奋激",
    triggers: &[EventKind::CardGained, EventKind::CardDiscarded],
    // 口径 B（2026-11-11 裁定修正）：不按回合，按「被其他角色弃置 / 从其他角色·区域获得」。
    window: Window::AnyTime,
    priority: 30,
    can_active: None,
    handler: fenji,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn first_fenji_owner(g: &Game, seat: usize) -> Option<usize> {
    let mut order = vec![seat];
    order.extend(g.seats_from_left_of(seat));
    order.into_iter().find(|i| active(g, *i, "fenji") && g.seats[*i].hp > 0)
}

/// 奋激的触发主体（2026-11-11 裁定 B）：
/// - `CardDiscarded`：被**其他角色**弃置（`by != 受影响的座位`）；自己弃牌/弃牌阶段不触发；
/// - `CardGained`：被其他角色获得（`from` 是牌的原持有者且不等于获得者）→ 主体 = 原持有者；
///   其余来源的「获得」（含技能摸牌、五谷等）按「从区域获得」计入 → 主体 = 获得者；
///   但**摸牌阶段的摸牌**不计（`cause == "draw_phase"`）。
fn fenji_subject(a: &Ask) -> Option<usize> {
    let event = a.data["event"].as_str().unwrap_or_default();
    let seat = a.data["seat"].as_u64().map(|n| n as usize);
    if event.starts_with("CardDiscarded") {
        let victim = seat?;
        let by = a.data["by"].as_u64().map(|n| n as usize);
        return (by.is_some() && by != Some(victim)).then_some(victim);
    }
    if event.starts_with("CardGained") {
        let gainer = seat?;
        let from = a.data["from"].as_u64().map(|n| n as usize);
        if let Some(src) = from {
            if src != gainer {
                return Some(src); // 被其他角色获得 → 主体是原持有者
            }
        }
        if a.data["cause"].as_str() == Some("draw_phase") {
            return None; // 摸牌阶段的摸牌不算「获得」
        }
        return Some(gainer); // 其他来源获得（技能摸牌/五谷/弃牌堆等）
    }
    None
}

fn fenji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let Some(seat) = fenji_subject(a) else {
                return Ok(Step::Done);
            };
            if first_fenji_owner(g, seat) != Some(a.actor) {
                return Ok(Step::Done); // 只问座位顺序第一个
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("fenji")
                        .option(
                            "choose:fenji",
                            format!("奋激：失去 1 点体力，令 {} 摸 2 张", g.seats[seat].name),
                        )
                        .escape("不发动奋激")
                        .build(),
                )),
                Some("pass") => Ok(Step::Done),
                Some("choose:fenji") => {
                    flow::lose_hp(g, a.actor, 1, "fenji");
                    if g.seats[a.actor].hp <= 0 {
                        // 先完成濒死结算，脱困后才摸 2（用户裁定 Q9）。
                        flow::start_dying(g, a.actor, None);
                        Ok(Step::Next(Stage::AfterDying, json!({ "target": seat })))
                    } else {
                        g.draw(seat, 2);
                        g.presentation.push(
                            Presentation::new("fenji", POutcome::Draw { n: 2 })
                                .actor(a.actor)
                                .target(seat)
                                .fx("fenji_draw"),
                        );
                        Ok(Step::Done)
                    }
                }
                Some(other) => Err(format!("奋激：未知选项 {other}")),
            }
        }
        Stage::AfterDying => {
            let seat = a.data["target"].as_u64().unwrap_or(0) as usize;
            if g.seats[a.actor].hp > 0 {
                g.draw(seat, 2);
                g.presentation.push(
                    Presentation::new("fenji", POutcome::Draw { n: 2 })
                        .actor(a.actor)
                        .target(seat)
                        .fx("fenji_draw"),
                );
            }
            Ok(Step::Done)
        }
        _ => Err("奋激：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 行殇
// ---------------------------------------------------------------------------

pub const XINGSHANG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "xingshang",
    name: "行殇",
    triggers: &[EventKind::Death],
    window: Window::AnyTime,
    priority: 5,
    can_active: None,
    handler: xingshang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn xingshang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let Some(victim) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    if victim == a.actor {
        return Ok(Step::Done);
    }
    // 手牌 + 装备区（**不含判定区**）。
    let mut cards = g.hand(victim);
    cards.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == victim)));
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("xingshang")
                .option(
                    "choose:xingshang",
                    format!("行殇：获得 {} 的 {} 张牌", g.seats[victim].name, cards.len()),
                )
                .escape("不发动行殇")
                .build(),
        )),
        Some("pass") => Ok(Step::Done),
        Some("choose:xingshang") => {
            let n = cards.len();
            for id in &cards {
                g.gain(id, a.actor)?;
            }
            g.presentation.push(
                Presentation::new("xingshang", POutcome::Draw { n: n as u32 })
                    .actor(a.actor)
                    .cards(cards)
                    .fx("xingshang_gain"),
            );
            Ok(Step::Done)
        }
        Some(other) => Err(format!("行殇：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 放逐
// ---------------------------------------------------------------------------

pub const FANGZHU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "fangzhu",
    name: "放逐",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 10,
    can_active: None,
    handler: fangzhu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn fangzhu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            // 只有「受击者是技能拥有者」才触发。
            if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
                return Ok(Step::Done);
            }
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("fangzhu");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("放逐：{} 翻面并摸牌", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动放逐").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("放逐：目标不合法 {act}"))?;
                    // 先翻面（翻至正面会触发解围②，作为子帧先结算）。
                    g.flip(target);
                    Ok(Step::Next(Stage::AfterFlip, json!({ "target": target })))
                }
            }
        }
        Stage::AfterFlip => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            // X = 已损失体力，**无上限**（用户裁定 Q12-B）。
            let lost = (g.seats[a.actor].max_hp - g.seats[a.actor].hp).max(0) as usize;
            g.draw(target, lost);
            g.presentation.push(
                Presentation::new("fangzhu", POutcome::Draw { n: lost as u32 })
                    .actor(a.actor)
                    .target(target)
                    .fx("fangzhu_draw"),
            );
            Ok(Step::Done)
        }
        _ => Err("放逐：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 颂威（主公技）
// ---------------------------------------------------------------------------

pub const SONGWEI: SkillDef = SkillDef { kind: SkillKind::Lord,
    code: "songwei",
    name: "颂威",
    triggers: &[EventKind::JudgeEffective],
    window: Window::AnyTime,
    priority: 50,
    can_active: None,
    handler: songwei,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn songwei(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let Some(judge_seat) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if judge_seat == a.actor {
        return Ok(Step::Done);
    }
    if g.seats[judge_seat].kingdom != "wei" {
        return Ok(Step::Done);
    }
    let black = g.cards.get(&card).map(|c| !c.suit.is_red()).unwrap_or(false);
    if !black {
        return Ok(Step::Done);
    }
    // **询问判定者**（用户裁定 Q14-A）。
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(judge_seat, "choice")
                .skill("songwei")
                .option(
                    "choose:songwei",
                    format!("颂威：令主公 {} 摸 1 张", g.seats[a.actor].name),
                )
                .escape("不发动颂威")
                .build(),
        )),
        Some("pass") => Ok(Step::Done),
        Some("choose:songwei") => {
            g.draw(a.actor, 1);
            g.presentation.push(
                Presentation::new("songwei", POutcome::Draw { n: 1 })
                    .actor(judge_seat)
                    .target(a.actor)
                    .fx("songwei_draw"),
            );
            Ok(Step::Done)
        }
        Some(other) => Err(format!("颂威：未知选项 {other}")),
    }
}

#[allow(dead_code)]
fn _unused(_: Value) {}
