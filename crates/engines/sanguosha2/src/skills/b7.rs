//! B7（李严 / 孙登 / 刘虞 / 岑昏 / 孙资刘放 / 黄皓 / 张让）——按 `align/B7.json` 第 7 段实现。
//!
//! 本批落实的两条跨批机制：
//! - **伤害属性字段化**：`cards::DamageKind`（普通/火/雷）+ `flow::damage_typed` 的「连环传导起点」标记；
//!   链祸只按属性与起点判定，不硬编码「火杀/火攻/某技能」。
//! - **响应牌机制**：`cards::ResponseKind` + `Game.response_context`；滔乱在响应场景只提供该环节的响应牌
//!   （无懈链→无懈；万箭→闪；南蛮→杀），不硬编码技能名。
//!
//! 其它裁定：匡弼牌按移出/移入游戏；讥谀花色封锁用**手牌冻结**；失去体力一律 `flow::lose_hp`；
//! 腹鳞统计「本回合获得的牌」但排除摸牌阶段摸牌；督粮/贿生的展示是**私密展示**。

use serde_json::json;

use crate::cards::{self, CardKind, DamageKind, ResponseKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, other_alive, SkillDef};

fn play_phase(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Play
}

fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}

fn hand_and_equip(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

fn kind_bit(g: &Game, id: &CardId) -> u32 {
    match g.def_of(id).kind {
        CardKind::Basic => 1,
        CardKind::Trick => 2,
        CardKind::Equip => 4,
    }
}

// ---------------------------------------------------------------------------
// 督粮（李严）
// ---------------------------------------------------------------------------

pub const DULIANG: SkillDef = SkillDef {
    code: "duliang",
    name: "督粮",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_duliang),
    handler: duliang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_duliang(g: &Game, seat: usize) -> bool {
    active(g, seat, "duliang")
        && play_phase(g, seat)
        && g.counter("turn.duliang_used") == 0
        && other_alive(g, seat).into_iter().any(|i| !g.hand(i).is_empty())
}

fn duliang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !g.hand(*i).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("duliang");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("获得 {} 一张手牌", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动督粮").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("督粮：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("督粮：目标不合法".into());
                    }
                    // 随机获得其一张手牌（裁定 Q1=B）
                    let card = g.hand(t).last().cloned().unwrap_or_default();
                    let label = label(g, &card);
                    g.suppress_gain_count = true;
                    g.gain(&card, a.actor)?;
                    g.suppress_gain_count = false;
                    g.bump("turn.duliang_used");
                    g.push_log(format!(
                        "督粮：获得 {} 的【{label}】",
                        g.seats[t].name
                    ));
                    Ok(Step::Next(Stage::PickOther, json!({ "target": t })))
                }
            }
        }
        Stage::PickOther => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("duliang")
                        .option("opt1", "令其观看牌堆顶两张牌，获得其中的基本牌")
                        .escape_with("opt2", "令其下回合摸牌阶段多摸一张牌")
                        .build(),
                )),
                Some("opt1") => {
                    // 私密展示牌堆顶两张（裁定 Q2=A）
                    let tops: Vec<CardId> = g.ids_in(|z| *z == Zone::Draw).into_iter().take(2).collect();
                    for id in &tops {
                        g.reveal_private(t, id, "duliang");
                    }
                    let mut gained = 0;
                    for id in &tops {
                        if g.def_of(id).kind == CardKind::Basic {
                            g.gain(id, t)?;
                            gained += 1;
                        }
                    }
                    g.push_log(format!(
                        "督粮：{} 观看牌堆顶两张，获得其中 {gained} 张基本牌",
                        g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
                Some("opt2") => {
                    g.bump(&format!("turn.duliang_extra_draw:{t}"));
                    g.push_log(format!(
                        "督粮：{} 下回合摸牌阶段多摸 1 张",
                        g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("督粮：未知选项 {other}")),
            }
        }
        _ => Err("督粮：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 腹鳞（李严，锁定）：手牌上限逻辑在 `Game::hand_limit`（turn.gained 派生）
// ---------------------------------------------------------------------------

pub const FULIN: SkillDef = SkillDef {
    code: "fulin",
    name: "腹鳞",
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

// ---------------------------------------------------------------------------
// 匡弼（孙登）：目标置至多三张牌到武将牌上（移出游戏），下回合获得（移入游戏）
// ---------------------------------------------------------------------------

pub const KUANGBI: SkillDef = SkillDef {
    code: "kuangbi",
    name: "匡弼",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::Prepare),
    priority: 25,
    can_active: Some(can_kuangbi),
    handler: kuangbi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn kuangbi_marks(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| *z == Zone::Void)
        .into_iter()
        .filter(|id| g.void_tags.get(id).is_some_and(|(s, c)| *s == seat && c == "kuangbi"))
        .collect()
}

fn can_kuangbi(g: &Game, seat: usize) -> bool {
    active(g, seat, "kuangbi")
        && play_phase(g, seat)
        && g.counter("turn.kuangbi_used") == 0
        && other_alive(g, seat)
            .into_iter()
            .any(|i| !hand_and_equip(g, i).is_empty())
}

fn kuangbi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 自己回合开始：取回「匡弼」牌（移入游戏）+ 目标摸等量
    if g.answer.is_none() && a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("Prepare") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        let marks = kuangbi_marks(g, a.actor);
        if marks.is_empty() {
            return Ok(Step::Done);
        }
        let helper = g.counter("turn.kuangbi_helper").checked_sub(1).map(|n| n as usize);
        for id in &marks {
            g.void_tags.remove(id);
            g.from_void(id, Zone::Hand(a.actor))?;
        }
        if let Some(h) = helper {
            if g.seats[h].hp > 0 {
                let n = g.draw(h, marks.len()).len();
                g.push_log(format!(
                    "匡弼：孙登获得 {} 张「匡弼」，{} 摸 {n} 张",
                    marks.len(),
                    g.seats[h].name
                ));
            }
        }
        g.counters.insert("turn.kuangbi_helper".to_string(), 0);
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            let cands: Vec<usize> = other_alive(g, a.actor)
                .into_iter()
                .filter(|i| !hand_and_equip(g, *i).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("kuangbi");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 将至多三张牌置于你武将牌上", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动匡弼").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("匡弼：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("匡弼：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "target": t, "selected": [] }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let selected: Vec<CardId> = a.data["selected"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let cards = hand_and_equip(g, t);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choose_many").skill("kuangbi");
                    for id in &cards {
                        if selected.len() >= 3 && !selected.contains(id) {
                            continue;
                        }
                        let mark = if selected.contains(id) { "✔ " } else { "" };
                        b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
                    }
                    if !selected.is_empty() {
                        b = b.option("confirm", format!("确认（{} 张）", selected.len()));
                    }
                    Ok(Step::Need(b.escape("不放置（本次不发动）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("confirm") => {
                    if selected.is_empty() {
                        return Ok(Step::Done);
                    }
                    for id in &selected {
                        g.to_void(id)?;
                        super::tag_void(g, id, a.actor, "kuangbi");
                    }
                    g.bump("turn.kuangbi_used");
                    g.counters
                        .insert("turn.kuangbi_helper".to_string(), t as u32 + 1);
                    g.push_log(format!(
                        "匡弼：{} 将 {} 张牌置于孙登武将牌上（移出游戏）",
                        g.seats[t].name,
                        selected.len()
                    ));
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("匡弼：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("匡弼：只能选其手牌或装备区牌".into());
                    }
                    let mut next = selected.clone();
                    match next.iter().position(|c| c == &id) {
                        Some(pos) => {
                            next.remove(pos);
                        }
                        None => {
                            if next.len() >= 3 {
                                return Err("匡弼：最多三张".into());
                            }
                            next.push(id);
                        }
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "target": t, "selected": next }),
                    ))
                }
            }
        }
        _ => Err("匡弼：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 止戈（刘虞）
// ---------------------------------------------------------------------------

pub const ZHIGE: SkillDef = SkillDef {
    code: "zhige",
    name: "止戈",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhige),
    handler: zhige,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhige_targets(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|i| g.in_attack_range(*i, seat))
        .filter(|i| {
            g.hand(*i).iter().any(|c| g.def_of(c).tags.sha) || !equip_ids(g, *i).is_empty()
        })
        .collect()
}

fn can_zhige(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhige")
        && play_phase(g, seat)
        && g.counter("turn.zhige_used") == 0
        && g.hand(seat).len() > g.seats[seat].hp.max(0) as usize
        && !zhige_targets(g, seat).is_empty()
}

fn zhige(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cands = zhige_targets(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("zhige");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("令 {} 选择", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动止戈").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("止戈：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("止戈：目标不合法".into());
                    }
                    g.bump("turn.zhige_used");
                    Ok(Step::Next(Stage::PickOther, json!({ "target": t })))
                }
            }
        }
        Stage::PickOther => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let shas: Vec<CardId> = g.hand(t).into_iter().filter(|c| g.def_of(c).tags.sha).collect();
            let equips = equip_ids(g, t);
            let mut b = super::builder(t, "choice").skill("zhige");
            if !shas.is_empty() {
                b = b.option("opt1", format!("对 {} 使用一张【杀】（可选牌）", g.seats[a.actor].name));
            }
            match g.answer.take().as_deref() {
                None => {
                    if equips.is_empty() {
                        return Ok(Step::Need(
                            b.escape("没有可交出的装备牌（放弃）").build(),
                        ));
                    }
                    Ok(Step::Need(
                        b.escape_with("opt2", "交给其一张装备区里的牌").build(),
                    ))
                }
                Some("opt2") => {
                    if equips.is_empty() {
                        return Err("止戈：其装备区没有牌".into());
                    }
                    Ok(Step::Next(Stage::DiscardTarget, json!({ "target": t })))
                }
                Some("opt1") => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "target": t, "shas": shas }),
                )),
                Some(other) => Err(format!("止戈：未知选项 {other}")),
            }
        }
        Stage::PickOwn => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let shas: Vec<CardId> = a.data["shas"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            if shas.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("zhige");
                    for id in &shas {
                        b = b.option(format!("choose:{id}"), format!("使用 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("改用交装备（若可）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("止戈：未知选项 {act}"))?
                        .to_string();
                    if !shas.contains(&id) {
                        return Err("止戈：只能使用自己的【杀】".into());
                    }
                    g.to_discard(&id, "zhige")?;
                    g.use_virtual(t, "杀", vec![a.actor])?;
                    g.presentation.push(
                        Presentation::new("zhige", POutcome::Activated {
                            skill: "止戈".to_string(),
                        })
                        .actor(t)
                        .target(a.actor)
                        .fx("zhige_sha"),
                    );
                    g.push_log(format!(
                        "止戈：{} 对刘虞使用【杀】（自选）",
                        g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        Stage::DiscardTarget => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let equips = equip_ids(g, t);
            if equips.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("zhige");
                    for id in &equips {
                        b = b.option(format!("choose:{id}"), format!("获得其装备 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("放弃获得").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("止戈：未知选项 {act}"))?
                        .to_string();
                    if !equips.contains(&id) {
                        return Err("止戈：只能取其装备区的牌".into());
                    }
                    let label = label(g, &id);
                    g.gain(&id, a.actor)?;
                    g.push_log(format!(
                        "止戈：{} 交给刘虞装备【{label}】",
                        g.seats[t].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("止戈：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 宗祚（刘虞，锁定）：开局按势力数 +上限；势力最后一人死亡 -1 上限
// ---------------------------------------------------------------------------

pub const ZONGZUO: SkillDef = SkillDef {
    code: "zongzuo",
    name: "宗祚",
    triggers: &[EventKind::Death],
    window: Window::AnyTime,
    priority: 10,
    can_active: None,
    handler: zongzuo,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 开局初始化（由 `skills::on_game_start` 调用）。
pub fn init_zongzuo(g: &mut Game) {
    let kingdoms: std::collections::BTreeSet<String> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .map(|i| g.seats[i].kingdom.clone())
        .collect();
    let x = kingdoms.len();
    for seat in 0..g.seats.len() {
        if !active(g, seat, "zongzuo") {
            continue;
        }
        let old = g.seats[seat].max_hp;
        let new_max = old + x as i32;
        g.seats[seat].max_hp = new_max;
        if g.seats[seat].hp > 0 {
            g.seats[seat].hp = (g.seats[seat].hp + 1).min(new_max);
        }
        g.events.push(
            crate::kernel::event::Event::with_amount(
                EventKind::MaxHpChanged,
                Some(seat),
                Some("zongzuo_init"),
                x as i32,
            ),
        );
        g.push_log(format!(
            "宗祚：全场 {x} 个势力，体力上限 +{x} 并回复 1 点体力"
        ));
    }
}

fn zongzuo(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() {
        let Some(dead) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        let kingdom = g.seats[dead].kingdom.clone();
        let still_alive = (0..g.seats.len())
            .any(|i| i != dead && g.seats[i].hp > 0 && g.seats[i].kingdom == kingdom);
        if still_alive {
            return Ok(Step::Done);
        }
        if !active(g, a.actor, "zongzuo") || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
        flow::lose_max_hp(g, a.actor, 1, "zongzuo");
        g.push_log(format!("宗祚：{kingdom} 势力最后一人死亡，体力上限 -1"));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 极奢（岑昏）：摸 1 手牌上限 -1；结束阶段无手牌则按体力令角色横置
// ---------------------------------------------------------------------------

pub const JISHE: SkillDef = SkillDef {
    code: "jishe",
    name: "极奢",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 20,
    can_active: Some(can_jishe),
    handler: jishe,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_jishe(g: &Game, seat: usize) -> bool {
    // 裁定 Q1=A：读**实际手牌上限**
    active(g, seat, "jishe") && play_phase(g, seat) && g.hand_limit(seat) > 0
}

fn jishe(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("End") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if !g.hand(a.actor).is_empty() || g.seats[a.actor].hp <= 0 {
            return Ok(Step::Done);
        }
    }
    // 出牌阶段的「摸 1 / 上限 -1」是主动技，没有 remaining 键
    if a.data.get("remaining").is_none() && a.data.get("event").is_none() {
        if !can_jishe(g, a.actor) {
            return Ok(Step::Done);
        }
        let n = g.draw(a.actor, 1).len();
        g.bump("turn.jishe_used");
        g.push_log(format!(
            "极奢：摸 {n} 张，本回合手牌上限 -1（上限 {}）",
            g.hand_limit(a.actor)
        ));
        return Ok(Step::Done);
    }
    let remaining = a
        .data
        .get("remaining")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or_else(|| g.seats[a.actor].hp.max(0) as usize);
    if remaining == 0 {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("jishe");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("令 {} 进入连环状态（剩 {remaining} 次）", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("不发动极奢（放弃剩余次数）").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("极奢：目标不合法 {act}"))?;
            if !cands.contains(&t) {
                return Err("极奢：目标不合法".into());
            }
            flow::chain_enter(g, t);
            if remaining > 1 {
                Ok(Step::Next(Stage::Choose, json!({ "remaining": remaining - 1 })))
            } else {
                Ok(Step::Done)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 链祸（岑昏，锁定）：作为连环传导起点受到火焰伤害时 +1（加成随传导延续）
// ---------------------------------------------------------------------------

pub const LIANHUO: SkillDef = SkillDef {
    code: "lianhuo",
    name: "链祸",
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

/// 链祸：火焰伤害 + 处于连环 + **是传导起点** → +1（裁定 Q1=A）。
pub fn lianhuo_bonus(g: &Game, victim: usize, dtype: DamageKind, chain_origin: bool) -> i32 {
    if dtype == DamageKind::Fire
        && chain_origin
        && active(g, victim, "lianhuo")
        && g.seats[victim].chained
    {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 瑰藻（孙资刘放）
// ---------------------------------------------------------------------------

pub const GUIZAO: SkillDef = SkillDef {
    code: "guizao",
    name: "瑰藻",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 15,
    can_active: None,
    handler: guizao,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn guizao_ready(g: &Game, seat: usize) -> bool {
    if !active(g, seat, "guizao") || g.counter("turn.guizao_offered") > 0 {
        return false;
    }
    let cards = &g.phase_discard_cards;
    if cards.len() < 2 {
        return false;
    }
    let mut suits = std::collections::BTreeSet::new();
    for id in cards {
        if let Some(c) = g.cards.get(id) {
            suits.insert(format!("{:?}", c.suit));
        }
    }
    suits.len() == cards.len()
}

fn guizao(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("Discard") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
        if !guizao_ready(g, a.actor) {
            return Ok(Step::Done);
        }
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("guizao")
                .option("opt1", "瑰藻：回复 1 点体力")
                .escape_with("opt2", "瑰藻：摸一张牌")
                .build(),
        )),
        Some("opt1") => {
            g.bump("turn.guizao_offered");
            if g.seats[a.actor].hp < g.seats[a.actor].max_hp {
                g.seats[a.actor].hp += 1;
            }
            g.push_log(format!(
                "瑰藻：回复 1 点体力（{}）",
                g.seats[a.actor].hp
            ));
            Ok(Step::Done)
        }
        Some("opt2") => {
            g.bump("turn.guizao_offered");
            let n = g.draw(a.actor, 1).len();
            g.push_log(format!("瑰藻：摸 {n} 张"));
            Ok(Step::Done)
        }
        Some("pass") => Ok(Step::Done),
        Some(other) => Err(format!("瑰藻：未知选项 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 讥谀（孙资刘放）：目标自选弃一张手牌；冻结该花色；黑桃则其失去 1 点体力 + 自己翻面
// ---------------------------------------------------------------------------

pub const JIYU: SkillDef = SkillDef {
    code: "jiyu",
    name: "讥谀",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_jiyu),
    handler: jiyu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn jiyu_targets(g: &Game, seat: usize) -> Vec<usize> {
    other_alive(g, seat)
        .into_iter()
        .filter(|i| !g.hand(*i).is_empty())
        .filter(|i| g.counter(&format!("turn.jiyu_target:{i}")) == 0)
        .collect()
}

fn can_jiyu(g: &Game, seat: usize) -> bool {
    active(g, seat, "jiyu")
        && play_phase(g, seat)
        && !g.hand(seat).is_empty()
        && !jiyu_targets(g, seat).is_empty()
}

fn jiyu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cands = jiyu_targets(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("jiyu");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 弃置一张手牌", g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动讥谀").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("讥谀：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("讥谀：目标不合法".into());
                    }
                    Ok(Step::Next(Stage::DiscardTarget, json!({ "target": t })))
                }
            }
        }
        Stage::DiscardTarget => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(t);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("jiyu");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("弃置 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不弃置（无牌可弃）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("讥谀：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("讥谀：只能弃置自己的手牌".into());
                    }
                    let suit = g.cards.get(&id).map(|c| c.suit);
                    let label = label(g, &id);
                    g.to_discard(&id, "jiyu")?;
                    g.bump(&format!("turn.jiyu_target:{t}"));
                    // 花色封锁 = 冻结孙资刘放手上该花色的牌（裁定 Q3=B）
                    if let Some(suit) = suit {
                        let frozen: Vec<CardId> = g
                            .hand(a.actor)
                            .into_iter()
                            .filter(|c| g.cards.get(c).map(|x| x.suit) == Some(suit))
                            .collect();
                        g.freeze_cards_for_phase(a.actor, &frozen);
                        g.push_log(format!(
                            "讥谀：{} 弃置【{label}】，该花色被冻结",
                            g.seats[t].name
                        ));
                        if suit == crate::kernel::Suit::Spade {
                            // 失去 1 点体力（走统一 lose_hp）+ 自己翻面
                            flow::lose_hp(g, t, 1, "jiyu");
                            g.flip(a.actor);
                            g.push_log("讥谀：黑桃，目标失去 1 点体力，孙资刘放翻面".to_string());
                        }
                    }
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("讥谀：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 寝情（黄皓）：结束阶段逐个「弃其一张牌 + 令其摸一张」，比主公手牌多则黄皓摸
// ---------------------------------------------------------------------------

pub const QINQING: SkillDef = SkillDef {
    code: "qinqing",
    name: "寝情",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 22,
    can_active: None,
    handler: qinqing,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn qinqing_lord(g: &Game) -> Option<usize> {
    (0..g.seats.len()).find(|i| g.seats[*i].flags.contains("lord") && g.seats[*i].hp > 0)
}

fn qinqing_cands(g: &Game, actor: usize, lord: usize) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|i| *i != actor && *i != lord && g.seats[*i].hp > 0)
        .filter(|i| g.in_attack_range(*i, lord))
        .filter(|i| !hand_and_equip(g, *i).is_empty())
        .filter(|i| g.counter(&format!("turn.qinqing_done:{i}")) == 0)
        .collect()
}

fn qinqing(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if g.answer.is_none() && a.data.get("event").is_some() {
        if a.data["cause"].as_str() != Some("End") {
            return Ok(Step::Done);
        }
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
            return Ok(Step::Done);
        }
    }
    let Some(lord) = qinqing_lord(g) else {
        return Ok(Step::Done);
    };
    let picked = a.data["picked"].as_u64().unwrap_or(0) as usize;
    match a.stage {
        Stage::Choose => {
            let cands = qinqing_cands(g, a.actor, lord);
            if cands.is_empty() {
                // 没有候选 → 结算黄皓的摸牌
                if picked > 0 {
                    let n = g.draw(a.actor, picked).len();
                    g.push_log(format!("寝情：{picked} 名角色手牌多于主公，黄皓摸 {n} 张"));
                }
                return Ok(Step::Done);
            }
            let mut b = super::builder(a.actor, "target").skill("qinqing");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("弃置 {} 一张牌并令其摸一张", g.seats[*i].name),
                );
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("放弃（结束寝情）").build())),
                Some("pass") => {
                    if picked > 0 {
                        let n = g.draw(a.actor, picked).len();
                        g.push_log(format!(
                            "寝情：{picked} 名角色手牌多于主公，黄皓摸 {n} 张"
                        ));
                    }
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("寝情：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("寝情：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::DiscardTarget,
                        json!({ "target": t, "picked": picked }),
                    ))
                }
            }
        }
        Stage::DiscardTarget => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let equips = equip_ids(g, t);
            let mut b = super::builder(a.actor, "choice").skill("qinqing");
            for id in &equips {
                b = b.option(
                    format!("equip:{id}"),
                    format!("弃置其装备区的 {}", label(g, id)),
                );
            }
            b = b.option("hand", "随机弃置其一张手牌");
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("跳过该角色").build())),
                Some("pass") => Ok(Step::Next(
                    Stage::Choose,
                    json!({ "picked": picked }),
                )),
                Some("hand") => {
                    if g.hand(t).is_empty() {
                        return Err("寝情：其没有手牌".into());
                    }
                    let card = g.hand(t).last().cloned().unwrap_or_default();
                    let label = label(g, &card);
                    g.to_discard(&card, "qinqing")?;
                    g.push_log(format!("寝情：弃置 {} 的手牌【{label}】", g.seats[t].name));
                    settle_qinqing_one(g, a.actor, t, lord, picked)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("equip:")
                        .ok_or_else(|| format!("寝情：未知选项 {act}"))?
                        .to_string();
                    if !equips.contains(&id) {
                        return Err("寝情：只能指定其装备区的牌".into());
                    }
                    let label = label(g, &id);
                    g.to_discard(&id, "qinqing")?;
                    g.push_log(format!("寝情：弃置 {} 的装备【{label}】", g.seats[t].name));
                    settle_qinqing_one(g, a.actor, t, lord, picked)
                }
            }
        }
        _ => Err("寝情：未知阶段".into()),
    }
}

fn settle_qinqing_one(
    g: &mut Game,
    actor: usize,
    t: usize,
    lord: usize,
    picked: usize,
) -> Result<Step, String> {
    let n = g.draw(t, 1).len();
    g.push_log(format!("寝情：{} 摸 {n} 张", g.seats[t].name));
    g.bump(&format!("turn.qinqing_done:{t}"));
    // 裁定 Q2=A：结算后与主公**当前**手牌数比较
    let picked = picked + usize::from(g.hand(t).len() > g.hand(lord).len());
    Ok(Step::Next(Stage::Choose, json!({ "picked": picked })))
}

// ---------------------------------------------------------------------------
// 贿生（黄皓）：受伤时私密展示任意张，令来源二选一
// ---------------------------------------------------------------------------

pub const HUISHENG: SkillDef = SkillDef {
    code: "huisheng",
    name: "贿生",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: huisheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 「造成伤害时」受害者侧拦截（返回 true = 已压帧）。
pub fn huisheng_before(
    g: &mut Game,
    source: Option<usize>,
    victim: usize,
    amount: i32,
    card: Option<&CardId>,
) -> bool {
    let Some(src) = source else { return false };
    if src == victim || amount <= 0 || !active(g, victim, "huisheng") {
        return false;
    }
    if g.counter(&format!("turn.huisheng_banned:{src}")) > 0 {
        return false;
    }
    if hand_and_equip(g, victim).is_empty() {
        return false;
    }
    crate::kernel::resolve::push_ask(
        g,
        "huisheng",
        Stage::Choose,
        victim,
        json!({ "source": src, "amount": amount, "card": card.map(|c| c.to_string()), "selected": [] }),
    );
    true
}

fn huisheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let source = a.data["source"].as_u64().unwrap_or(0) as usize;
    let amount = a.data["amount"].as_i64().unwrap_or(0) as i32;
    let card = a.data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string());
    match a.stage {
        Stage::Choose => {
            // 黄皓选 1..N 张（裁定 Q1=A）
            let cards = hand_and_equip(g, a.actor);
            let selected: Vec<CardId> = a.data["selected"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let mut b = super::builder(a.actor, "choose_many").skill("huisheng");
            for id in &cards {
                let mark = if selected.contains(id) { "✔ " } else { "" };
                b = b.option(format!("choose:{id}"), format!("{mark}{}", label(g, id)));
            }
            if !selected.is_empty() {
                b = b.option("confirm", format!("确认（{} 张）", selected.len()));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不发动贿生").build())),
                Some("pass") => {
                    resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                    Ok(Step::Done)
                }
                Some("confirm") => {
                    if selected.is_empty() {
                        return Ok(Step::Done);
                    }
                    for id in &selected {
                        g.reveal_private(source, id, "huisheng");
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "source": source, "amount": amount, "card": card, "shown": selected }),
                    ))
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("贿生：未知选项 {act}"))?
                        .to_string();
                    if !cards.contains(&id) {
                        return Err("贿生：只能选自己的牌".into());
                    }
                    let mut next = selected.clone();
                    match next.iter().position(|c| c == &id) {
                        Some(pos) => {
                            next.remove(pos);
                        }
                        None => next.push(id),
                    }
                    let mut data = a.data.clone();
                    data["selected"] = json!(next);
                    Ok(Step::Next(Stage::Choose, data))
                }
            }
        }
        Stage::PickOther => {
            let shown: Vec<CardId> = a.data["shown"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            if shown.is_empty() {
                return Ok(Step::Done);
            }
            let builder = super::builder(source, "choice")
                .skill("huisheng")
                .option("opt1", format!("获得其中一张（{} 张可选），防止此伤害", shown.len()));
            // 裁定 Q3=B：来源手牌不足等量时「弃置等量」不可选
            let pending = if g.hand(source).len() >= shown.len() {
                builder
                    .escape_with("opt2", format!("弃置 {} 张手牌，伤害照常结算", shown.len()))
                    .build()
            } else {
                builder.escape("不发动（手牌不足）").build()
            };
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(pending)),
                Some("opt1") => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "source": source, "shown": shown, "amount": amount, "card": card }),
                )),
                Some("opt2") => Ok(Step::Next(
                    Stage::DiscardTarget,
                    json!({ "source": source, "shown": shown, "amount": amount, "card": card, "left": shown.len() }),
                )),
                Some("pass") => {
                    for id in &shown {
                        g.private_reveals.remove(&source);
                        let _ = id;
                    }
                    g.clear_private_reveals(source);
                    resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("贿生：未知选项 {other}")),
            }
        }
        Stage::PickOwn => {
            let shown: Vec<CardId> = a.data["shown"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let mut b = super::builder(source, "choice").skill("huisheng");
            for id in &shown {
                b = b.option(format!("choose:{id}"), format!("获得 {}", label(g, id)));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不获得（伤害照常）").build())),
                Some("pass") => {
                    g.clear_private_reveals(source);
                    resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("贿生：未知选项 {act}"))?
                        .to_string();
                    if !shown.contains(&id) {
                        return Err("贿生：只能获得其中一张".into());
                    }
                    let label = label(g, &id);
                    g.gain(&id, source)?;
                    g.clear_private_reveals(source);
                    // 本局不能对该来源再发动贿生
                    g.bump(&format!("turn.huisheng_banned:{source}"));
                    g.presentation.push(
                        Presentation::new("huisheng", POutcome::Activated {
                            skill: "贿生".to_string(),
                        })
                        .actor(a.actor)
                        .target(source)
                        .cards(vec![id.clone()])
                        .fx("huisheng_avoid"),
                    );
                    g.push_log(format!(
                        "贿生：{} 获得【{label}】，此伤害被防止",
                        g.seats[source].name
                    ));
                    Ok(Step::Done)
                }
            }
        }
        Stage::DiscardTarget => {
            let left = a.data["left"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(source);
            if left == 0 || hand.is_empty() {
                g.clear_private_reveals(source);
                resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                return Ok(Step::Done);
            }
            let mut b = super::builder(source, "choice").skill("huisheng");
            for id in &hand {
                b = b.option(format!("choose:{id}"), format!("弃置 {}", label(g, id)));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("剩余不弃（伤害照常）").build())),
                Some("pass") => {
                    g.clear_private_reveals(source);
                    resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("贿生：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("贿生：只能弃置自己的手牌".into());
                    }
                    g.to_discard(&id, "huisheng")?;
                    if left > 1 && !g.hand(source).is_empty() {
                        let mut data = a.data.clone();
                        data["left"] = json!(left - 1);
                        return Ok(Step::Next(Stage::DiscardTarget, data));
                    }
                    g.clear_private_reveals(source);
                    resume_huisheng_damage(g, a.actor, source, amount, card.as_deref());
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("贿生：未知阶段".into()),
    }
}

fn resume_huisheng_damage(
    g: &mut Game,
    victim: usize,
    source: usize,
    amount: i32,
    card: Option<&str>,
) {
    let card = card.map(|c| c.to_string());
    g.suppress_hook("huisheng");
    flow::damage_with(g, Some(source), victim, amount, card.as_ref());
    g.release_hook("huisheng");
}

// ---------------------------------------------------------------------------
// 滔乱（张让）：一张牌当任意基本牌/普通锦囊（每牌名整局一次）；响应场景只给响应牌
// ---------------------------------------------------------------------------

pub const TAOLUAN: SkillDef = SkillDef {
    code: "taoluan",
    name: "滔乱",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_taoluan),
    handler: taoluan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn taoluan_used(g: &Game, seat: usize, name: &str) -> bool {
    g.seats[seat]
        .flags
        .contains(&format!("taoluan_used:{name}"))
}

fn any_dying(g: &Game) -> bool {
    (0..g.seats.len()).any(|i| g.seats[i].hp <= 0)
}

/// 可声明的牌名：响应场景只给该环节的响应牌；否则 = 基本牌（杀/桃/酒）+ 普通锦囊（属性过滤）。
fn taoluan_names(g: &Game, seat: usize) -> Vec<&'static str> {
    if let Some(kind) = g.response_context {
        return match kind {
            ResponseKind::Sha => vec!["杀"],
            ResponseKind::Shan => vec!["闪"],
            ResponseKind::Wuxie => vec!["无懈可击"],
        };
    }
    let mut out = vec!["杀", "桃", "酒"];
    out.extend(
        cards::non_delayed_tricks()
            .into_iter()
            .filter(|n| cards::target_mode(n) != cards::TargetMode::SelfOnly || *n == "无中生有"),
    );
    out.retain(|n| !taoluan_used(g, seat, n));
    out.retain(|n| {
        !(*n == "桃" && g.seats[seat].hp >= g.seats[seat].max_hp)
            && !(*n == "酒" && g.counter("turn.huomo_jiu") > 0)
    });
    out
}

fn can_taoluan(g: &Game, seat: usize) -> bool {
    active(g, seat, "taoluan")
        && !g.hand(seat).is_empty()
        && !any_dying(g)
        && !taoluan_names(g, seat).is_empty()
}

/// 滔乱声明的牌需要几个目标（min, max）：借刀 2/2；铁索 1/2；其余 1/1。
fn taoluan_target_need(name: &str) -> (usize, usize) {
    match cards::def(name).effect {
        cards::Effect::Collateral => (2, 2),
        cards::Effect::IronChain => (1, 2),
        _ => (1, 1),
    }
}

/// 滔乱选目标的候选池（读牌定义，不比较牌名）。
fn taoluan_target_candidates(g: &Game, actor: usize, name: &str, picked: &[usize]) -> Vec<usize> {
    let alive: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
    match cards::def(name).effect {
        cards::Effect::Collateral => match picked.first() {
            // 第二个目标：任意存活角色（可以是张让自己），但不能与 A 相同。
            Some(a) => alive.into_iter().filter(|i| i != a).collect(),
            // 第一个目标：装备区有武器、且不是自己。
            None => alive
                .into_iter()
                .filter(|i| *i != actor && g.equip_in(*i, crate::kernel::Slot::Weapon).is_some())
                .collect(),
        },
        cards::Effect::IronChain => alive.into_iter().filter(|i| !picked.contains(i)).collect(),
        _ => other_alive(g, actor)
            .into_iter()
            .filter(|i| !picked.contains(i))
            .collect(),
    }
}

/// 滔乱的实际使用（真实结算 + 展示/日志）。
fn taoluan_finish_use(
    g: &mut Game,
    actor: usize,
    id: &CardId,
    name: &str,
    kind: u32,
    label: &str,
    targets: Vec<usize>,
) -> Result<(), String> {
    g.use_as(actor, id, name, targets, true, None)?;
    g.seats[actor]
        .flags
        .insert(format!("taoluan_used:{name}"));
    let _ = kind;
    g.presentation.push(
        Presentation::new("taoluan", POutcome::Activated {
            skill: "滔乱".to_string(),
        })
        .actor(actor)
        .cards(vec![id.clone()])
        .fx("taoluan"),
    );
    g.push_log(format!("滔乱：将【{label}】当【{name}】使用"));
    Ok(())
}

fn taoluan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
            let cards_in_hand = g.hand(a.actor);
            let names = taoluan_names(g, a.actor);
            if cards_in_hand.is_empty() || names.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("taoluan");
                    for id in &cards_in_hand {
                        for n in &names {
                            b = b.option(
                                format!("choose:{id}:{n}"),
                                format!("将 {} 当【{n}】使用", label(g, id)),
                            );
                        }
                    }
                    Ok(Step::Need(b.escape("不发动滔乱").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let rest = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("滔乱：未知选项 {act}"))?;
                    let (id, name) = rest
                        .rsplit_once(':')
                        .ok_or_else(|| format!("滔乱：未知选项 {act}"))?;
                    let id = id.to_string();
                    let name = name.to_string();
                    if !cards_in_hand.contains(&id) || !names.contains(&name.as_str()) {
                        return Err("滔乱：该组合不可用".into());
                    }
                    let kind = kind_bit(g, &id);
                    let label = label(g, &id);
                    // M2：真实结算。需要选目标的牌先进入 CardTarget 选目标（取消不烧牌名）。
                    if cards::target_mode(&name) == cards::TargetMode::Chosen {
                        return Ok(Step::Next(
                            Stage::CardTarget,
                            json!({
                                "id": id,
                                "name": name,
                                "kind": kind,
                                "label": label,
                                "picked": [],
                            }),
                        ));
                    }
                    taoluan_finish_use(g, a.actor, &id, &name, kind, &label, vec![])?;
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "kind": kind, "name": name }),
                    ))
                }
                Some(other) => Err(format!("滔乱：未知选项 {other}")),
            }
        }
        Stage::CardTarget => {
            let id = a.data["id"].as_str().unwrap_or_default().to_string();
            let name = a.data["name"].as_str().unwrap_or_default().to_string();
            let label = a.data["label"].as_str().unwrap_or_default().to_string();
            let kind = a.data["kind"].as_u64().unwrap_or(0) as u32;
            let mut picked: Vec<usize> = a.data["picked"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();
            let (min, max) = taoluan_target_need(&name);
            let cands = taoluan_target_candidates(g, a.actor, &name, &picked);
            if cands.is_empty() && picked.len() < min {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "card_target").skill("taoluan");
                    for i in &cands {
                        b = b.option(
                            format!("cardtarget:{i}"),
                            format!("【{name}】目标：{}", g.seats[*i].name),
                        );
                    }
                    if picked.len() >= min {
                        b = b.option("confirm", "确定（按当前目标使用）");
                    }
                    Ok(Step::Need(b.escape("取消（不使用此牌）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("confirm") => {
                    if picked.len() < min {
                        return Err("滔乱：目标数量不足".into());
                    }
                    taoluan_finish_use(g, a.actor, &id, &name, kind, &label, picked)?;
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "kind": kind, "name": name }),
                    ))
                }
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("cardtarget:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("滔乱：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("滔乱：目标不合法".into());
                    }
                    picked.push(t);
                    if picked.len() < min || (max > picked.len() && max > min) {
                        // 借刀必须选满 2 个；铁索选到第 1 个后仍可继续选第 2 个。
                        return Ok(Step::Next(
                            Stage::CardTarget,
                            json!({
                                "id": id,
                                "name": name,
                                "kind": kind,
                                "label": label,
                                "picked": picked,
                            }),
                        ));
                    }
                    taoluan_finish_use(g, a.actor, &id, &name, kind, &label, picked)?;
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "kind": kind, "name": name }),
                    ))
                }
            }
        }
        Stage::Target => {
            let kind = a.data["kind"].as_u64().unwrap_or(0) as u32;
            let name = a.data["name"].as_str().unwrap_or_default().to_string();
            let cands = other_alive(g, a.actor);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("taoluan");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("令 {} 选择一项", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不令其选择").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("滔乱：目标不合法 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("滔乱：目标不合法".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOther,
                        json!({ "kind": kind, "name": name, "target": t }),
                    ))
                }
            }
        }
        Stage::PickOther => {
            let kind = a.data["kind"].as_u64().unwrap_or(0) as u32;
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let has_diff = g.hand(t).iter().any(|c| kind_bit(g, c) != kind);
            let mut b = super::builder(t, "choice").skill("taoluan");
            if has_diff {
                b = b.option("opt1", "交给张让一张类型不同的手牌");
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    b.escape_with("opt2", "令张让失去 1 点体力，本回合滔乱失效").build(),
                )),
                Some("opt1") => {
                    if !has_diff {
                        return Err("滔乱：没有类型不同的手牌".into());
                    }
                    Ok(Step::Next(Stage::DiscardTarget, json!({ "target": t, "kind": kind })))
                }
                Some("opt2") => {
                    flow::lose_hp(g, a.actor, 1, "taoluan");
                    g.bump("turn.taoluan_disabled");
                    g.push_log("滔乱：张让失去 1 点体力，本回合滔乱失效".to_string());
                    Ok(Step::Done)
                }
                Some(other) => Err(format!("滔乱：未知选项 {other}")),
            }
        }
        Stage::DiscardTarget => {
            let kind = a.data["kind"].as_u64().unwrap_or(0) as u32;
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let hand: Vec<CardId> = g
                .hand(t)
                .into_iter()
                .filter(|c| kind_bit(g, c) != kind)
                .collect();
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(t, "choice").skill("taoluan");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("交给张让 {}", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不交出").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("滔乱：未知选项 {act}"))?
                        .to_string();
                    if !hand.contains(&id) {
                        return Err("滔乱：只能交类型不同的手牌".into());
                    }
                    let label = label(g, &id);
                    g.gain(&id, a.actor)?;
                    g.push_log(format!("滔乱：{} 交给张让【{label}】", g.seats[t].name));
                    Ok(Step::Done)
                }
            }
        }
        _ => Err("滔乱：未知阶段".into()),
    }
}
