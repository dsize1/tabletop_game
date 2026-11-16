//! B0 技能 handler（据守/解围/不屈/奋激/行殇/放逐/颂威）。
//!
//! 每个 handler 是「声明 → 行为」的分发落点：声明在 `b0::SKILLS`（窗口/订阅/钩子），
//! 行为在本模块。handler 只返回 `Step`，栈由 `resolve::advance` 改（spike B 类）。
//! 上下文（谁失牌/谁死亡/伤害来源）从 `Ask.data` 读事件快照，不读 live 状态。

use serde_json::{json, Value};

use super::super::cards;
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Stage};
use super::super::resolve::{Ask, Step};
use super::super::{Game, Zone};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::event::{EventKind, Subject, Window};
use crate::kernel::Phase;

/// B0 handler 分发。
pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "jushou" => jushou,
        "jiewei" => jiewei,
        "buqu" => buqu,
        "fenji" => fenji,
        "xingshang" => xingshang,
        "fangzhu" => fangzhu,
        "songwei" => songwei,
        _ => return None,
    })
}

fn step_of(ask: &Ask) -> &str {
    ask.data
        .get("step")
        .and_then(|v| v.as_str())
        .unwrap_or("ask")
}

fn card_label(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| format!("【{}】", d.name))
        .unwrap_or_else(|| id.to_string())
}

fn seat_label(g: &Game, seat: usize) -> String {
    g.seats
        .get(seat)
        .map(|s| s.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("座位{}", seat + 1))
}

/// 通用收尾：按持有上限逐张强制弃牌（技能摸牌后）；`next` = 收尾后继续的步骤数据。
fn discard_to_limit(
    g: &mut Game,
    skill: &'static str,
    actor: usize,
    next: Value,
) -> Result<Step, String> {
    if g.hand_size(actor) <= g.hand_limit(actor) {
        return Ok(Step::Next(Stage::DiscardPick, next));
    }
    let mut b = PendingBuilder::new(actor, "skill_limit").skill(skill);
    for id in g.hand(actor) {
        b = b.option(format!("card:{id}"), card_label(g, &id));
    }
    Ok(Step::NeedAt(
        Stage::DiscardPick,
        b.escape_mandatory("弃至手牌上限").build(),
    ))
}

/// 技能上限弃牌的实际执行（返回 true = 本次消费了一个弃牌回答）。
fn take_limit_discard(g: &mut Game, skill: &str, actor: usize) -> bool {
    if let Some(ans) = take_answer(g, skill) {
        if let Some(id) = ans.strip_prefix("card:") {
            flow::discard(g, actor, id, skill);
            return true;
        }
    }
    false
}

/// 据守（`align/B0.json`，Q1=B 取反）：结束阶段 → 翻面 → 摸 4 → 弃 1 张手牌（装备牌改为使用）
/// → 按手牌上限弃牌收尾。
fn jushou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let seat = ask.actor;
    match step_of(ask) {
        "ask" => {
            if let Some(ans) = take_answer(g, "jushou") {
                if ans != "choose:jushou" {
                    return Ok(Step::Done);
                }
                // Q1=B：翻面是「取反」（背面→正面会触发解围②等翻面钩子）。
                super::super::skills::flip_seat(g, seat);
                flow::draw(g, seat, 4);
                return Ok(Step::Next(Stage::Choose, json!({ "step": "pick" })));
            }
            let b = PendingBuilder::new(seat, "jushou_ask")
                .skill("jushou")
                .option(
                    "choose:jushou",
                    "发动【据守】：翻面并摸四张牌，然后弃置一张手牌（装备牌改为使用）",
                );
            Ok(Step::NeedAt(Stage::Choose, b.escape("不发动").build()))
        }
        "pick" => {
            if let Some(ans) = take_answer(g, "jushou") {
                if let Some(id) = ans.strip_prefix("card:") {
                    if !g.hand(seat).iter().any(|c| c == id) {
                        return Err("据守：只能选择自己的手牌".into());
                    }
                    let is_equip = g
                        .card_defs
                        .get(id)
                        .and_then(|c| cards::def(&c.def))
                        .map(|d| d.slot.is_some())
                        .unwrap_or(false);
                    if is_equip {
                        super::super::card_play::equip_from_hand(g, seat, id)?;
                    } else {
                        flow::discard(g, seat, id, "据守");
                    }
                } else if ans != "pass" {
                    return Err(format!("据守：未知选项 {ans}"));
                }
                return Ok(Step::Next(Stage::DiscardPick, json!({ "step": "limit" })));
            }
            if g.hand(seat).is_empty() {
                return Ok(Step::Next(Stage::DiscardPick, json!({ "step": "limit" })));
            }
            let mut b = PendingBuilder::new(seat, "jushou_pick").skill("jushou");
            for id in g.hand(seat) {
                b = b.option(format!("card:{id}"), card_label(g, &id));
            }
            Ok(Step::NeedAt(
                Stage::PickOwn,
                b.escape_mandatory("弃置一张手牌").build(),
            ))
        }
        _ => {
            let _ = take_limit_discard(g, "jushou", seat);
            if g.hand_size(seat) <= g.hand_limit(seat) {
                return Ok(Step::Done);
            }
            discard_to_limit(g, "jushou", seat, json!({ "step": "limit" }))
        }
    }
}

/// 解围①（使用面）：装备区牌当【无懈可击】（`step_wuxie` 里生成 `equip:<id>` 选项）。
/// 解围②（触发面）：从背面翻至正面时，弃 1 张牌 → 移动场上一张牌。
fn jiewei(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let seat = ask.actor;
    match step_of(ask) {
        "ask" => {
            let has_cards =
                g.hand_size(seat) > 0 || !super::super::gear::equips_of(g, seat).is_empty();
            if !has_cards {
                return Ok(Step::Done); // ③ 手牌与装备区都为空 → 不询问
            }
            if let Some(ans) = take_answer(g, "jiewei") {
                if ans != "choose:jiewei" {
                    return Ok(Step::Done);
                }
                return Ok(Step::Next(Stage::PickOwn, json!({ "step": "discard" })));
            }
            let b = PendingBuilder::new(seat, "jiewei_ask")
                .skill("jiewei")
                .option(
                    "choose:jiewei",
                    "发动【解围】：弃置一张牌，然后移动场上一张牌",
                );
            Ok(Step::NeedAt(Stage::Choose, b.escape("不发动").build()))
        }
        "discard" => {
            if let Some(ans) = take_answer(g, "jiewei") {
                if let Some(id) = ans.strip_prefix("card:") {
                    flow::discard(g, seat, id, "解围");
                } else if let Some(id) = ans.strip_prefix("equip:") {
                    super::super::card_play::remove_card(g, seat, id, "解围");
                } else if ans != "pass" {
                    return Err(format!("解围：未知选项 {ans}"));
                }
                return Ok(Step::Next(Stage::Target, json!({ "step": "source" })));
            }
            // ① 弃 1 张：手牌或自己装备区（不含判定区）。
            let mut b = PendingBuilder::new(seat, "jiewei_discard").skill("jiewei");
            for id in g.hand(seat) {
                b = b.option(format!("card:{id}"), card_label(g, &id));
            }
            for id in super::super::gear::equips_of(g, seat) {
                b = b.option(
                    format!("equip:{id}"),
                    format!("装备区 {}", card_label(g, &id)),
                );
            }
            Ok(Step::NeedAt(
                Stage::PickOwn,
                b.escape_mandatory("弃置一张牌").build(),
            ))
        }
        "source" => {
            let sources: Vec<usize> = (0..g.seats.len())
                .filter(|&s| {
                    g.seats[s].alive
                        && (g.hand_size(s) > 0
                            || !super::super::gear::equips_of(g, s).is_empty()
                            || !g.judge_stacks.get(&s).map(|v| v.is_empty()).unwrap_or(true))
                })
                .collect();
            if sources.is_empty() {
                return Ok(Step::Done); // ⑧ 无可选来源：结束交互
            }
            if let Some(ans) = take_answer(g, "jiewei") {
                if let Some(t) = ans.strip_prefix("t:") {
                    let src: usize = t.parse().map_err(|_| "解围：非法来源".to_string())?;
                    if !sources.contains(&src) {
                        return Err("解围：来源不合法".into());
                    }
                    return Ok(Step::Next(
                        Stage::CardPick,
                        json!({ "step": "pick", "src": src }),
                    ));
                }
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(seat, "jiewei_source").skill("jiewei");
            for s in sources {
                b = b.option(format!("t:{s}"), seat_label(g, s));
            }
            Ok(Step::NeedAt(Stage::Target, b.escape("结束解围").build()))
        }
        "pick" => {
            let src = ask.data.get("src").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if let Some(ans) = take_answer(g, "jiewei") {
                let kind = if ans.strip_prefix("equip:").is_some() {
                    "equip"
                } else if ans.strip_prefix("judge:").is_some() {
                    "judge"
                } else if ans == "hand" {
                    "hand"
                } else {
                    return Ok(Step::Done);
                };
                return Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "dest", "src": src, "kind": kind, "pick": ans }),
                ));
            }
            let mut b = PendingBuilder::new(seat, "jiewei_pick").skill("jiewei");
            for id in super::super::gear::equips_of(g, src) {
                b = b.option(format!("equip:{id}"), card_label(g, &id));
            }
            for id in g.judge_stacks.get(&src).cloned().unwrap_or_default() {
                b = b.option(format!("judge:{id}"), card_label(g, &id));
            }
            if g.hand_size(src) > 0 {
                b = b.option("hand", "随机一张手牌");
            }
            Ok(Step::NeedAt(Stage::CardPick, b.escape("结束解围").build()))
        }
        _ => {
            // "dest"
            let src = ask.data.get("src").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let kind = ask
                .data
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("hand");
            let pick = ask
                .data
                .get("pick")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(ans) = take_answer(g, "jiewei") {
                let Some(t) = ans.strip_prefix("t:") else {
                    return Ok(Step::Done);
                };
                let dest: usize = t.parse().map_err(|_| "解围：非法目的地".to_string())?;
                match kind {
                    "equip" => {
                        let id = pick.trim_start_matches("equip:").to_string();
                        super::super::gear::equip(g, dest, &id);
                    }
                    "judge" => {
                        let id = pick.trim_start_matches("judge:").to_string();
                        let code = g
                            .card_defs
                            .get(&id)
                            .map(|c| c.def.clone())
                            .unwrap_or_default();
                        super::super::card_play::place_delayed(g, dest, &id, &code);
                    }
                    _ => {
                        // 随机一张手牌（信息口径：不列明细）。
                        let hand = g.hand(src);
                        if let Some(id) = hand.first().cloned() {
                            flow::gain(g, dest, &id, dest, "解围");
                        }
                    }
                }
                return Ok(Step::Done);
            }
            // 目的地：来源一律排除。
            let mut dests: Vec<usize> = Vec::new();
            for s in 0..g.seats.len() {
                if !g.seats[s].alive || s == src {
                    continue;
                }
                let ok = match kind {
                    "equip" => {
                        let id = pick.trim_start_matches("equip:");
                        let slot = g
                            .card_defs
                            .get(id)
                            .and_then(|c| cards::def(&c.def))
                            .and_then(|d| d.slot);
                        slot.map(|sl| super::super::gear::equip_in(g, s, sl).is_none())
                            .unwrap_or(false)
                    }
                    "judge" => {
                        let id = pick.trim_start_matches("judge:");
                        let code = g
                            .card_defs
                            .get(id)
                            .map(|c| c.def.clone())
                            .unwrap_or_default();
                        !g.judge_stacks
                            .get(&s)
                            .map(|stack| {
                                stack.iter().any(|c| {
                                    g.card_defs.get(c).map(|d| d.def == code).unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                    }
                    _ => true,
                };
                if ok {
                    dests.push(s);
                }
            }
            if dests.is_empty() {
                return Ok(Step::Done); // ⑧ 无目的地：结束交互
            }
            let mut b = PendingBuilder::new(seat, "jiewei_dest").skill("jiewei");
            for s in dests {
                b = b.option(format!("t:{s}"), seat_label(g, s));
            }
            Ok(Step::NeedAt(Stage::Target, b.escape("结束解围").build()))
        }
    }
}

/// 奋激：**回合外**手牌被弃置/获得（或非摸牌阶段摸牌）→ 询问失牌者起第一个持有者：
/// 失去 1 点体力令其摸 2 张。
fn fenji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let holder = ask.actor;
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .ok_or("奋激缺少 victim")? as usize;
    match step_of(ask) {
        "ask" => {
            if !g.seats[holder].alive {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "fenji") {
                if ans != "choose:fenji" {
                    return Ok(Step::Done);
                }
                flow::lose_hp(g, holder, 1, "奋激");
                // 防递归：奋激自己摸的两张不再触发奋激。
                let suppress = format!("fenji.suppress.{victim}");
                g.set_counter(&suppress, 2);
                flow::draw(g, victim, 2);
                g.set_counter(&suppress, 0);
                return Ok(Step::Next(
                    Stage::DiscardPick,
                    json!({ "step": "limit", "victim": victim }),
                ));
            }
            let b = PendingBuilder::new(holder, "fenji_ask")
                .skill("fenji")
                .option(
                    "choose:fenji",
                    format!(
                        "发动【奋激】：失去 1 点体力，令 {} 摸两张牌",
                        seat_label(g, victim)
                    ),
                );
            Ok(Step::NeedAt(Stage::Choose, b.escape("不发动").build()))
        }
        _ => {
            let _ = take_limit_discard(g, "fenji", victim);
            if g.hand_size(victim) <= g.hand_limit(victim) {
                return Ok(Step::Done);
            }
            discard_to_limit(
                g,
                "fenji",
                victim,
                json!({ "step": "limit", "victim": victim }),
            )
        }
    }
}

/// 行殇：其他角色死亡时（清理前）→ 询问获得其手牌+装备（**不含判定区**）→ 收口死亡结算。
fn xingshang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let holder = ask.actor;
    if !g.seats.get(holder).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step != "ask" {
        let _ = take_limit_discard(g, "xingshang", holder);
        if g.hand_size(holder) > g.hand_limit(holder) {
            return discard_to_limit(g, "xingshang", holder, json!({ "step": "limit" }));
        }
        return Ok(Step::Done);
    }
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .ok_or("行殇缺少 victim")? as usize;
    if victim == holder {
        return Ok(Step::Done);
    }
    let from_event = ask.data.get("cards").is_some();
    // 死亡弃牌批次携带的牌（事件 `DeathDiscarded.cards`）；旧存档/直接调用时回退到死者区域。
    let cards: Vec<String> = ask
        .data
        .get("cards")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| {
            let mut v: Vec<String> = g
                .cards
                .iter()
                .filter(|(_, z)| {
                    z.seat() == Some(victim) && matches!(z, Zone::Hand(_) | Zone::Equip(..))
                })
                .map(|(id, _)| id.clone())
                .collect();
            v.sort();
            v
        });
    match step {
        "ask" => {
            if let Some(ans) = take_answer(g, "xingshang") {
                if ans == "choose:xingshang" {
                    for id in &cards {
                        // 死亡弃牌已在弃牌堆；行殇从弃牌堆获得（事件驱动，不再由 settle_death 预取）。
                        if !from_event || matches!(g.cards.get(id), Some(Zone::Discard)) {
                            flow::gain(g, holder, id, holder, "行殇");
                        }
                    }
                    return Ok(Step::Next(Stage::DiscardPick, json!({ "step": "limit" })));
                }
                return Ok(Step::Done);
            }
            if cards.is_empty() {
                return Ok(Step::Done);
            }
            let b = PendingBuilder::new(holder, "xingshang_ask")
                .skill("xingshang")
                .option(
                    "choose:xingshang",
                    format!(
                        "发动【行殇】：获得 {} 的 {} 张牌（不含判定区）",
                        seat_label(g, victim),
                        cards.len()
                    ),
                );
            Ok(Step::NeedAt(Stage::Choose, b.escape("不发动").build()))
        }
        _ => Ok(Step::Done),
    }
}

/// 放逐：受到伤害后 → 选一名其他存活角色：翻面（取反）→ 摸 X（X = 已损失体力，无上限）→ 上限弃牌。
fn fangzhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let seat = ask.actor;
    match step_of(ask) {
        "ask" => {
            let targets: Vec<usize> = (0..g.seats.len())
                .filter(|&s| s != seat && g.seats[s].alive)
                .collect();
            if targets.is_empty() || !g.seats[seat].alive {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "fangzhu") {
                let Some(t) = ans.strip_prefix("t:") else {
                    return Ok(Step::Done);
                };
                let target: usize = t.parse().map_err(|_| "放逐：非法目标".to_string())?;
                if !targets.contains(&target) {
                    return Err("放逐：目标不合法".into());
                }
                return Ok(Step::Next(
                    Stage::CardResolve,
                    json!({ "step": "flip", "target": target }),
                ));
            }
            let mut b = PendingBuilder::new(seat, "fangzhu_target").skill("fangzhu");
            for t in targets {
                b = b.option(format!("t:{t}"), seat_label(g, t));
            }
            Ok(Step::NeedAt(Stage::Target, b.escape("不发动").build()))
        }
        "flip" => {
            let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            // 先翻面（翻回正面会触发解围②，其帧在栈顶先结算），再摸牌。
            super::super::skills::flip_seat(g, target);
            Ok(Step::Next(
                Stage::CardResolve,
                json!({ "step": "draw", "target": target }),
            ))
        }
        "draw" => {
            let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let lost = (g.seats[seat].max_hp - g.seats[seat].hp).max(0) as usize;
            flow::draw(g, target, lost);
            Ok(Step::Next(
                Stage::DiscardPick,
                json!({ "step": "limit", "target": target }),
            ))
        }
        _ => {
            let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let _ = take_limit_discard(g, "fangzhu", target);
            if g.hand_size(target) <= g.hand_limit(target) {
                return Ok(Step::Done);
            }
            discard_to_limit(
                g,
                "fangzhu",
                target,
                json!({ "step": "limit", "target": target }),
            )
        }
    }
}

/// 颂威（主公技）：其他魏势力角色的**黑色判定生效后** → 询问该判定者是否令主公摸 1 张。
fn songwei(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let lord = ask.actor;
    let judge_seat = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .ok_or("颂威缺少判定者")? as usize;
    let card = ask
        .data
        .get("card")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    match step_of(ask) {
        "ask" => {
            // 条件：判定者魏势力、判定牌黑色、判定者不是主公本人、主公存活。
            let wei = g
                .seats
                .get(judge_seat)
                .map(|s| s.kingdom == "wei")
                .unwrap_or(false);
            let black = g
                .card_defs
                .get(&card)
                .map(|c| c.suit.is_black())
                .unwrap_or(false);
            if !g.seats[lord].alive || judge_seat == lord || !wei || !black {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "songwei") {
                if ans == "choose:songwei" {
                    flow::draw(g, lord, 1);
                }
                return Ok(Step::Done);
            }
            let b = PendingBuilder::new(judge_seat, "songwei_ask")
                .skill("songwei")
                .option(
                    "choose:songwei",
                    format!("发动【颂威】：令主公 {} 摸一张牌", seat_label(g, lord)),
                );
            Ok(Step::NeedAt(Stage::Choose, b.escape("不发动").build()))
        }
        _ => Ok(Step::Done),
    }
}

/// 不屈：锁定技，濒死时自动发动（入口见 `flow::enter_dying`）。
///
/// 判定由 `quick_judge` 驱动（含鬼才改判 / 天妒）；本 handler 在 `step="result"` 收口：
/// 点数与已有「创」不重复 → 作为「创」移出游戏并回复至 1；重复 → 判定牌进弃牌堆、仍濒死
/// （此时再压通用濒死帧，求桃顺序不变）。
fn buqu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    use crate::kernel::event::Event;
    use crate::kernel::resolve::FlowKind;

    let seat = ask.actor;
    let by = ask
        .data
        .get("by")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let Some(r) = g.judge_result.take() else {
        // 无判定牌（牌堆/弃牌堆皆空）：直接进入濒死求桃。
        flow::enter_dying_bypass_buqu(g, seat, by);
        return Ok(Step::Done);
    };
    let key = format!("buqu_rank.{seat}.{}", r.rank);
    if g.counter(&key) > 0 {
        // 重复：判定牌已在弃牌堆（quick_judge 落盘），仍濒死。
        flow::enter_dying_bypass_buqu(g, seat, by);
        return Ok(Step::Done);
    }
    flow::exile(g, &r.card, "不屈·创");
    g.bump(&key);
    g.bump(&format!("buqu_total.{seat}"));
    // 回复至 1（走统一回复出口；不触发救援改向）。
    flow::restore_hp(g, seat, 1, "不屈");
    let _ = FlowKind::Dying;
    Ok(Step::Finish("不屈：翻「创」成功，回复至 1".into()))
}

/// 不屈：有「创」时手牌上限 = 创数（覆盖体力值；`skills::hand_limit_override` 分发）。
pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    if !super::active(g, seat, "buqu") {
        return None;
    }
    let n = g.counter(&format!("buqu_total.{seat}"));
    (n > 0).then_some(n as usize)
}

/// B0 声明（7 技能）。
pub static SKILLS: &[SkillDef] = &[
    // —— B0：据守/解围/不屈/奋激/行殇/放逐/颂威 ——
    SkillDef {
        code: "jushou",
        name: "据守",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            // 结束阶段开始时触发（`PhaseStart` + 自身结束阶段窗口；`turn::step_turn` 在进入 End 时 emit）。
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[Subscription {
                event: EventKind::PhaseStart,
                subject: Subject::Any,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "jiewei",
        name: "解围",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[
            // 使用面：装备区牌当【无懈可击】（`step_wuxie` 生成 `equip:<id>` 选项）。
            Face {
                window: Window::AnyTime,
                subscriptions: &[],
                hooks: &[Hook::WuxieOpt],
                can_active: false,
            },
            // 触发面：从背面翻至正面（`skills::flip_seat` 触发 `Hook::Flip`）。
            Face {
                window: Window::AnyTime,
                subscriptions: &[],
                hooks: &[Hook::Flip],
                can_active: false,
            },
        ],
    },
    SkillDef {
        code: "buqu",
        name: "不屈",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::Dying],
            can_active: false,
        }],
    },
    SkillDef {
        code: "fenji",
        name: "奋激",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            // 奋激：由 `flow` 的手牌变动钩子（`skills::on_hand_changed`）驱动：
            // 回合外 + 手牌进弃牌堆 / 手牌增加（排除摸牌阶段摸牌）→ 座位序第一个持有者询问。
            subscriptions: &[],
            hooks: &[Hook::HandChanged],
            can_active: false,
        }],
    },
    SkillDef {
        code: "xingshang",
        name: "行殇",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            // 行殇：订阅 `DeathDiscarded`（用户 2026-11-16 裁定：死亡弃牌事件驱动，不含判定区）。
            subscriptions: &[Subscription {
                event: EventKind::DeathDiscarded,
                subject: Subject::Other,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "fangzhu",
        name: "放逐",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription {
                event: EventKind::DamageAfter,
                subject: Subject::Self_,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "songwei",
        name: "颂威",
        kind: SkillKind::Lord,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            // 颂威：其他魏势力黑色判定生效 → subject = Other（主公技他人侧）。
            subscriptions: &[Subscription {
                event: EventKind::JudgeEffective,
                subject: Subject::Other,
            }],
            hooks: &[],
            can_active: false,
        }],
    },
];

/// 主动技可用性判定（`skills::active_actions` 唯一读取）。
pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "jushou" => can_jushou,
        _ => return None,
    })
}

fn can_jushou(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::End
}

// 供测试直接调用核心效果（绕过 Ask 构造）。
#[allow(dead_code)]
fn _data(victim: Option<usize>, target: Option<usize>) -> Value {
    let mut v = serde_json::Map::new();
    if let Some(x) = victim {
        v.insert("victim".into(), x.into());
    }
    if let Some(x) = target {
        v.insert("target".into(), x.into());
    }
    Value::Object(v)
}
