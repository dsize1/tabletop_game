//! B12（界华佗 / 界吕布 / 界公孙瓒 / 界貂蝉）——按 `align/B12.json` 第 7 段实现。
//!
//! 用户口径（2026-11-09，28 条疑点裁定）：
//! - 急救：红色**手牌 + 装备区**（判定区不可）当【桃】，仅回合外；
//! - 除疠：各参与者**自己**选手牌/装备区的一张弃置（判定区不参与），然后黑桃者各摸 1（含自己）；
//! - 无双：`multi-response`（连续 N 张响应；决斗两侧都按「与吕布决斗的对方」计）；
//! - 利驭：决斗对象由**受伤角色**指定；判定区不可选；【决斗】可被无懈；
//! - 义从：距离双向修正（机制 `distance-modifier`，逐项累加、下限 1）；
//! - 趫猛：询问发动；手牌随机/装备区可选；坐骑牌改为获得；
//! - 离间：手牌 + 装备区弃牌；`no-wuxie`（该【决斗】跳过无懈窗口）；只有目标受空城限制；
//! - 闭月：询问发动；发动瞬间判「无手牌 → 2 张，否则 1 张」。
//!
//! 本批新增跨批能力：`multi-response`（`skills::response_need`）与
//! `no-wuxie`（`UseOpts::no_wuxie` → 使用帧 `data["no_wuxie"]`）。

use serde_json::json;

use crate::cards::{Effect, ResponseKind};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Step};
use crate::kernel::{CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, builder, other_alive, SkillDef};

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
}
fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}
fn equip_ids(g: &Game, seat: usize) -> Vec<CardId> {
    g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat))
}
/// 「区域里的一张牌」但**不含判定区**（B12 裁定：判定区离开不算弃置，也不参与取牌）。
fn region_ids(g: &Game, seat: usize) -> Vec<CardId> {
    let mut out = g.hand(seat);
    out.extend(equip_ids(g, seat));
    out
}
fn has_region_cards(g: &Game, seat: usize) -> bool {
    !region_ids(g, seat).is_empty()
}
fn is_mount(g: &Game, id: &CardId) -> bool {
    g.cards.get(id).is_some_and(|c| {
        matches!(
            c.slot,
            Some(crate::kernel::Slot::HorseOffense) | Some(crate::kernel::Slot::HorseDefense)
        )
    })
}
fn is_spade(g: &Game, id: &CardId) -> bool {
    g.cards
        .get(id)
        .is_some_and(|c| c.suit == crate::kernel::Suit::Spade)
}
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, crate::kernel::resolve::Stage::Choose, actor, data);
}
fn is_u64_list(v: &serde_json::Value) -> Vec<usize> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default()
}
fn pick_card_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in g.hand(seat) {
        out.push((format!("pick:{id}"), format!("手牌：{}", label(g, &id))));
    }
    for id in equip_ids(g, seat) {
        out.push((format!("pick:{id}"), format!("装备区：{}", label(g, &id))));
    }
    out
}

// ---------------------------------------------------------------------------
// 急救（界华佗）：回合外红色手牌/装备区牌当【桃】
// ---------------------------------------------------------------------------

pub const JIJIU: SkillDef = SkillDef {
    code: "jijiu",
    name: "急救",
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

/// 被动转化技（只作濒死救援的【桃】；`usable=false`）。
pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "jijiu") || g.turn == seat {
        return Vec::new();
    }
    region_ids(g, seat)
        .into_iter()
        .filter(|id| g.is_red(id))
        .map(|id| super::Conversion {
            skill: "jijiu",
            from: id,
            to: "桃",
            usable: false,
            playable: true,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 除疠（界华佗）：势力去重多选 → 各参与者自选一张弃置 → 黑桃者各摸 1
// ---------------------------------------------------------------------------

pub const CHULI: SkillDef = SkillDef {
    code: "chuli",
    name: "除疠",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_chuli),
    handler: chuli,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn chuli_candidates(g: &Game, owner: usize, selected: &[usize]) -> Vec<usize> {
    other_alive(g, owner)
        .into_iter()
        .filter(|i| {
            has_region_cards(g, *i)
                && !selected
                    .iter()
                    .any(|s| g.seats[*s].kingdom == g.seats[*i].kingdom)
        })
        .collect()
}

fn can_chuli(g: &Game, seat: usize) -> bool {
    g.turn == seat
        && g.phase == Phase::Play
        && g.counter("turn.chuli_used") == 0
        && has_region_cards(g, seat)
        && !chuli_candidates(g, seat, &[]).is_empty()
}

fn chuli(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["stage"].as_str() == Some("pick") {
        chuli_pick(g, a)
    } else {
        chuli_select(g, a)
    }
}

fn chuli_select(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let selected = is_u64_list(&a.data["selected"]);
    let cands = chuli_candidates(g, owner, &selected);
    match g.answer.take().as_deref() {
        None => {
            if cands.is_empty() && selected.is_empty() {
                return Ok(Step::Done);
            }
            let mut b = builder(owner, "choice").skill("chuli");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("{}（{}）", g.seats[*i].name, g.seats[*i].kingdom),
                );
            }
            if !selected.is_empty() {
                b = b.option("confirm", format!("确认（{} 名角色 + 你）", selected.len()));
            }
            Ok(Step::Need(b.escape("取消除疠").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some("confirm") => {
            if selected.is_empty() {
                return Err("除疠：至少选择 1 名其他角色".into());
            }
            g.bump("turn.chuli_used");
            let mut queue = vec![owner];
            queue.extend(selected.iter().copied());
            let first = queue[0];
            push(
                g,
                "chuli",
                first,
                json!({ "stage": "pick", "owner": owner, "queue": queue, "i": 0, "spades": [] }),
            );
            Ok(Step::Done)
        }
        Some(act) => {
            let target = act
                .strip_prefix("target:")
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("除疠：未知选项 {act}"))?;
            if !cands.contains(&target) {
                return Err("除疠：目标不合法（势力重复或无牌）".into());
            }
            let mut sel = selected.clone();
            sel.push(target);
            push(g, "chuli", owner, json!({ "stage": "select", "selected": sel }));
            Ok(Step::Done)
        }
    }
}

fn chuli_pick(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
    let queue = is_u64_list(&a.data["queue"]);
    let mut spades = is_u64_list(&a.data["spades"]);
    let i = a.data["i"].as_u64().unwrap_or(0) as usize;
    let Some(&asker) = queue.get(i) else {
        return Ok(Step::Done);
    };
    if !has_region_cards(g, asker) {
        return chuli_advance(g, owner, &queue, i + 1, &spades);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(asker, "choice").skill("chuli");
            for (id, lab) in pick_card_options(g, asker) {
                b = b.option(id, lab);
            }
            Ok(Step::Need(b.escape("跳过（不弃置）").build()))
        }
        Some("pass") => {
            g.push_log(format!("除疠：{} 放弃弃牌", g.seats[asker].name));
            chuli_advance(g, owner, &queue, i + 1, &spades)
        }
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("除疠：未知选项 {act}"))?
                .to_string();
            if !region_ids(g, asker).iter().any(|c| c == &id) {
                return Err("除疠：该牌不在其手牌/装备区".into());
            }
            let spade = is_spade(g, &id);
            let lab = label(g, &id);
            g.to_discard(&id, "chuli")?;
            g.push_log(format!("除疠：{} 弃置【{lab}】", g.seats[asker].name));
            if spade {
                spades.push(asker);
            }
            chuli_advance(g, owner, &queue, i + 1, &spades)
        }
    }
}

fn chuli_advance(
    g: &mut Game,
    owner: usize,
    queue: &[usize],
    mut i: usize,
    spades: &[usize],
) -> Result<Step, String> {
    while i < queue.len() && !has_region_cards(g, queue[i]) {
        i += 1;
    }
    if i < queue.len() {
        let asker = queue[i];
        push(
            g,
            "chuli",
            asker,
            json!({ "stage": "pick", "owner": owner, "queue": queue, "i": i, "spades": spades }),
        );
        return Ok(Step::Done);
    }
    for s in spades {
        let n = g.draw(*s, 1).len();
        g.push_log(format!(
            "除疠：{} 的被弃置牌为黑桃，摸 {n} 张",
            g.seats[*s].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 无双（界吕布）：连续 N 张响应（multi-response）
// ---------------------------------------------------------------------------

pub const WUSHUANG: SkillDef = SkillDef {
    code: "wushuang",
    name: "无双",
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

/// 响应张数：无双使「你使用的【杀】」需两张【闪】，「与你【决斗】的角色」每次需两张【杀】。
pub fn response_need(
    g: &Game,
    _responder: usize,
    opponent: usize,
    kind: ResponseKind,
    effect: Effect,
) -> usize {
    if !active(g, opponent, "wushuang") {
        return 1;
    }
    match (kind, effect) {
        (ResponseKind::Shan, Effect::Sha) => 2,
        (ResponseKind::Sha, Effect::Duel) => 2,
        _ => 1,
    }
}

// ---------------------------------------------------------------------------
// 利驭（界吕布）：杀造成伤害后获得其一张牌；装备牌则由受伤角色指定决斗
// ---------------------------------------------------------------------------

pub const LIYU: SkillDef = SkillDef {
    code: "liyu",
    name: "利驭",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: liyu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 伤害后事件的公共解析：`(来源, 受伤者, 伤害牌)`，只接受「自己用【杀】造成」的伤害。
fn sha_damage_of(g: &Game, a: &Ask, owner: usize) -> Option<(usize, CardId)> {
    let src = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok())?;
    if src != owner {
        return None;
    }
    let victim = a.data["seat"].as_u64().map(|n| n as usize)?;
    if victim == owner || g.seats.get(victim).map(|s| s.hp <= 0).unwrap_or(true) {
        return None;
    }
    let card = a.data["card"].as_str()?.to_string();
    if !g.cards.contains_key(&card) || !g.def_of(&card).tags.sha {
        return None;
    }
    Some((victim, card))
}

fn liyu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let stage = a.data["stage"].as_str().unwrap_or("trigger").to_string();
    let owner = a.data["owner"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    match stage.as_str() {
        "trigger" => {
            let owner = a.actor;
            let Some((victim, _card)) = sha_damage_of(g, a, owner) else {
                return Ok(Step::Done);
            };
            if !has_region_cards(g, victim) {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("liyu");
                    b = b.option(
                        "opt1",
                        format!("发动利驭：获得 {} 一张牌", g.seats[victim].name),
                    );
                    Ok(Step::Need(b.escape("不发动利驭").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(_) => {
                    push(
                        g,
                        "liyu",
                        owner,
                        json!({ "stage": "pick", "owner": owner, "victim": victim }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "pick" => {
            let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("liyu");
                    if !g.hand(victim).is_empty() {
                        b = b.option(
                            "pick:hand",
                            format!("手牌：随机 1 张（共 {} 张）", g.hand(victim).len()),
                        );
                    }
                    for id in equip_ids(g, victim) {
                        b = b.option(
                            format!("pick:{id}"),
                            format!("装备区：{}", label(g, &id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不获得").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some("pick:hand") => {
                    if g.hand(victim).is_empty() {
                        return Err("利驭：目标没有手牌".into());
                    }
                    let n = g.hand(victim).len();
                    let idx = g.rand_below(n);
                    let card = g.hand(victim)[idx].clone();
                    g.gain(&card, owner)?;
                    g.push_log(format!(
                        "利驭：获得 {} 的【{}】",
                        g.seats[victim].name,
                        label(g, &card)
                    ));
                    liyu_after_gain(g, owner, victim, &card)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("利驭：未知选项 {act}"))?
                        .to_string();
                    if !g
                        .zone_of(&id)
                        .is_some_and(|z| matches!(z, Zone::Equip(s, _) if *s == victim))
                    {
                        return Err("利驭：该装备不在其装备区".into());
                    }
                    g.gain(&id, owner)?;
                    g.push_log(format!(
                        "利驭：获得 {} 的【{}】",
                        g.seats[victim].name,
                        label(g, &id)
                    ));
                    liyu_after_gain(g, owner, victim, &id)
                }
            }
        }
        "designate" => {
            let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != owner && *i != victim && g.seats[*i].hp > 0)
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(a.actor, "target").skill("liyu");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("令 {} 与 {} 决斗", g.seats[owner].name, g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("放弃（不发动决斗）").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| format!("利驭：未知选项 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("利驭：决斗对象不合法".into());
                    }
                    g.presentation.push(
                        Presentation::new(
                            "liyu",
                            POutcome::Activated {
                                skill: "利驭".to_string(),
                            },
                        )
                        .actor(owner)
                        .target(target)
                        .fx("liyu"),
                    );
                    g.push_log(format!(
                        "利驭：由 {} 指定，{} 视为对 {} 使用【决斗】",
                        g.seats[victim].name, g.seats[owner].name, g.seats[target].name
                    ));
                    g.use_virtual(owner, "决斗", vec![target])?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

fn liyu_after_gain(
    g: &mut Game,
    owner: usize,
    victim: usize,
    card: &CardId,
) -> Result<Step, String> {
    let is_equip = g.cards.get(card).is_some_and(|c| c.slot.is_some());
    if !is_equip {
        let n = g.draw(victim, 1).len();
        g.push_log(format!(
            "利驭：获得的不是装备牌，{} 摸 {n} 张",
            g.seats[victim].name
        ));
        return Ok(Step::Done);
    }
    let has_other = (0..g.seats.len())
        .any(|i| i != owner && i != victim && g.seats[i].hp > 0);
    if !has_other {
        return Ok(Step::Done);
    }
    // 由**受伤角色**指定另一名角色（文本「由其指定的另一名角色」）。
    push(
        g,
        "liyu",
        victim,
        json!({ "stage": "designate", "owner": owner, "victim": victim }),
    );
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 义从（界公孙瓒）：距离双向修正（锁定技）
// ---------------------------------------------------------------------------

pub const YICONG: SkillDef = SkillDef {
    code: "yicong",
    name: "义从",
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

/// 「我 → 他人」距离 -1。
pub fn distance_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "yicong") {
        -1
    } else {
        0
    }
}

/// 「他人 → 我」距离 +1（当前体力 ≤ 2 实时判定）。
pub fn distance_to_delta(g: &Game, seat: usize) -> i32 {
    if active(g, seat, "yicong") && g.seats.get(seat).is_some_and(|s| s.hp <= 2) {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// 趫猛（界公孙瓒）：杀造成伤害后弃其一张牌；坐骑牌改为获得
// ---------------------------------------------------------------------------

pub const QIAOMENG: SkillDef = SkillDef {
    code: "qiaomeng",
    name: "趫猛",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: qiaomeng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn qiaomeng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let stage = a.data["stage"].as_str().unwrap_or("trigger").to_string();
    let owner = a.data["owner"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    match stage.as_str() {
        "pick" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qiaomeng");
                if !g.hand(victim).is_empty() {
                    b = b.option(
                        "pick:hand",
                        format!("手牌：随机 1 张（共 {} 张）", g.hand(victim).len()),
                    );
                }
                for id in equip_ids(g, victim) {
                    b = b.option(format!("pick:{id}"), format!("装备区：{}", label(g, &id)));
                }
                Ok(Step::Need(b.escape("不弃置").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some("pick:hand") => {
                if g.hand(victim).is_empty() {
                    return Err("趫猛：目标没有手牌".into());
                }
                let n = g.hand(victim).len();
                let idx = g.rand_below(n);
                let card = g.hand(victim)[idx].clone();
                qiaomeng_take(g, owner, victim, &card)
            }
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("趫猛：未知选项 {act}"))?
                    .to_string();
                if !g
                    .zone_of(&id)
                    .is_some_and(|z| matches!(z, Zone::Equip(s, _) if *s == victim))
                {
                    return Err("趫猛：该装备不在其装备区".into());
                }
                qiaomeng_take(g, owner, victim, &id)
            }
        },
        _ => {
            let owner = a.actor;
            let Some((victim, _card)) = sha_damage_of(g, a, owner) else {
                return Ok(Step::Done);
            };
            if !has_region_cards(g, victim) {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("qiaomeng");
                    b = b.option(
                        "opt1",
                        format!("发动趫猛：弃置 {} 一张牌", g.seats[victim].name),
                    );
                    Ok(Step::Need(b.escape("不发动趫猛").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(_) => {
                    push(
                        g,
                        "qiaomeng",
                        owner,
                        json!({ "stage": "pick", "owner": owner, "victim": victim }),
                    );
                    Ok(Step::Done)
                }
            }
        }
    }
}

fn qiaomeng_take(
    g: &mut Game,
    owner: usize,
    victim: usize,
    card: &CardId,
) -> Result<Step, String> {
    let lab = label(g, card);
    if is_mount(g, card) {
        g.gain(card, owner)?;
        g.push_log(format!(
            "趫猛：{} 的【{lab}】是坐骑牌，{} 获得之",
            g.seats[victim].name, g.seats[owner].name
        ));
    } else {
        g.to_discard(card, "qiaomeng")?;
        g.push_log(format!(
            "趫猛：弃置 {} 的【{lab}】",
            g.seats[victim].name
        ));
    }
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 离间（界貂蝉）：弃一张牌（手牌/装备区）→ 两名男性视为【决斗】（不能被无懈）
// ---------------------------------------------------------------------------

pub const LIJIAN: SkillDef = SkillDef {
    code: "lijian",
    name: "离间",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_lijian),
    handler: lijian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn male_others(g: &Game, owner: usize) -> Vec<usize> {
    other_alive(g, owner)
        .into_iter()
        .filter(|i| g.seats[*i].male)
        .collect()
}

fn can_lijian(g: &Game, seat: usize) -> bool {
    if g.turn != seat || g.phase != Phase::Play || g.counter("turn.lijian_used") > 0 {
        return false;
    }
    if !has_region_cards(g, seat) {
        return false;
    }
    let males = male_others(g, seat);
    males.iter().any(|a| {
        males.iter().any(|b| {
            a != b && crate::skills::can_target(g, *a, "决斗", *b)
        })
    })
}

fn lijian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let stage = a.data["stage"].as_str().unwrap_or("discard").to_string();
    let owner = a.data["owner"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(a.actor);
    let attacker = a.data["attacker"].as_u64().map(|n| n as usize);
    match stage.as_str() {
        "discard" => match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("lijian");
                for (id, lab) in pick_card_options(g, owner) {
                    b = b.option(id, lab);
                }
                Ok(Step::Need(b.escape("取消离间").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("离间：未知选项 {act}"))?
                    .to_string();
                if !region_ids(g, owner).iter().any(|c| c == &id) {
                    return Err("离间：该牌不在你的手牌/装备区".into());
                }
                push(
                    g,
                    "lijian",
                    owner,
                    json!({ "stage": "attacker", "owner": owner, "card": id }),
                );
                Ok(Step::Done)
            }
        },
        "attacker" => {
            let males = male_others(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("lijian");
                    for i in &males {
                        b = b.option(format!("target:{i}"), g.seats[*i].name.clone());
                    }
                    Ok(Step::Need(b.escape("取消离间").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| format!("离间：未知选项 {act}"))?;
                    if !males.contains(&target) {
                        return Err("离间：发起者必须是其他男性角色".into());
                    }
                    let card = a.data["card"].as_str().unwrap_or_default().to_string();
                    push(
                        g,
                        "lijian",
                        owner,
                        json!({ "stage": "defender", "owner": owner, "card": card, "attacker": target }),
                    );
                    Ok(Step::Done)
                }
            }
        }
        "defender" => {
            let Some(attacker) = attacker else {
                return Ok(Step::Done);
            };
            let cands: Vec<usize> = male_others(g, owner)
                .into_iter()
                .filter(|i| *i != attacker && crate::skills::can_target(g, attacker, "决斗", *i))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = builder(owner, "target").skill("lijian");
                    for i in &cands {
                        b = b.option(
                            format!("target:{i}"),
                            format!("{} 对 {} 使用【决斗】", g.seats[attacker].name, g.seats[*i].name),
                        );
                    }
                    Ok(Step::Need(b.escape("取消离间").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse::<usize>().ok())
                        .ok_or_else(|| format!("离间：未知选项 {act}"))?;
                    if !cands.contains(&target) {
                        return Err("离间：目标不合法".into());
                    }
                    let card = a.data["card"].as_str().unwrap_or_default().to_string();
                    if !region_ids(g, owner).iter().any(|c| c == &card) {
                        return Err("离间：承载牌已不在你的区域".into());
                    }
                    let lab = label(g, &card);
                    g.to_discard(&card, "lijian")?;
                    g.bump("turn.lijian_used");
                    g.presentation.push(
                        Presentation::new(
                            "lijian",
                            POutcome::Activated {
                                skill: "离间".to_string(),
                            },
                        )
                        .actor(owner)
                        .targets(vec![attacker, target])
                        .cards(vec![card])
                        .fx("lijian"),
                    );
                    g.push_log(format!(
                        "离间：弃置【{lab}】，{} 视为对 {} 使用【决斗】（不能被无懈可击抵消）",
                        g.seats[attacker].name, g.seats[target].name
                    ));
                    g.use_virtual_no_wuxie(attacker, "决斗", vec![target])?;
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 闭月（界貂蝉）：结束阶段摸牌（无手牌 2，否则 1）
// ---------------------------------------------------------------------------

pub const BIYUE: SkillDef = SkillDef {
    code: "biyue",
    name: "闭月",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnPhase(Phase::End),
    priority: 0,
    can_active: None,
    handler: biyue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn biyue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let n = if g.hand(owner).is_empty() { 2 } else { 1 };
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("biyue");
            b = b.option(
                "opt1",
                if n == 2 {
                    "摸两张牌（你没有手牌）".to_string()
                } else {
                    "摸一张牌".to_string()
                },
            );
            Ok(Step::Need(b.escape("不发动闭月").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            let drawn = g.draw(owner, n).len();
            g.push_log(format!("闭月：摸 {drawn} 张"));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 注册表
// ---------------------------------------------------------------------------

pub const B12_SKILLS: &[SkillDef] = &[
    JIJIU, CHULI, WUSHUANG, LIYU, YICONG, QIAOMENG, LIJIAN, BIYUE,
];
