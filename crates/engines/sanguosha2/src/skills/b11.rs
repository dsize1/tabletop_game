//! B11（界曹操/界司马懿/界夏侯惇/界张辽/界许褚/界郭嘉/界李典/界甄姬）。
//! 约定：多步技能「答完即压新帧」（push_ask），不依赖 `Step::Next` 传状态。

use serde_json::json;

use crate::cards::CardKind;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Zone};

use super::{active, builder, other_alive, SkillDef, SkillKind};

fn label(g: &Game, id: &CardId) -> String {
    g.card_label(id)
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
fn push(g: &mut Game, code: &'static str, actor: usize, data: serde_json::Value) {
    crate::kernel::resolve::push_ask(g, code, Stage::Choose, actor, data);
}

// ------------------------------------------------------------------ 奸雄
pub const JIANXIONG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jianxiong", name: "奸雄", triggers: &[EventKind::DamageAfter], window: Window::AnyTime,
    priority: 0, can_active: None, handler: jianxiong, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn jianxiong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("gain") {
        let card = a.data["card"].as_str().unwrap_or_default().to_string();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("jianxiong");
                b = b.option("opt1", format!("获得【{}】", label(g, &card)));
                Ok(Step::Need(b.escape("不获得").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                if g.cards.contains_key(&card) {
                    g.gain(&card, owner)?;
                    g.push_log(format!("奸雄：获得【{}】", label(g, &card)));
                }
                Ok(Step::Done)
            }
        }
    } else {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        let n = g.draw(owner, 1).len();
        g.push_log(format!("奸雄：受到伤害，摸 {n} 张"));
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
            return Ok(Step::Done);
        };
        if g.cards.contains_key(&card) {
            push(g, "jianxiong", owner, json!({ "stage": "gain", "card": card }));
        }
        Ok(Step::Done)
    }
}

// ------------------------------------------------------------------ 护驾
pub const HUJIA: SkillDef = SkillDef { kind: SkillKind::Lord,
    code: "hujia", name: "护驾", triggers: &[EventKind::CardUsed, EventKind::CardPlayed], window: Window::AnyTime,
    priority: 0, can_active: None, handler: hujia, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

pub fn relay_order(g: &Game, lord: usize) -> Vec<usize> {
    other_alive(g, lord)
        .into_iter()
        .filter(|i| g.seats[*i].kingdom == "wei" && !g.hand(*i).is_empty())
        .collect()
}

pub fn relay_options(g: &Game, seat: usize, kind: crate::cards::ResponseKind) -> Vec<(String, String)> {
    if kind != crate::cards::ResponseKind::Shan
        || !active(g, seat, "hujia")
        || g.seats[seat].identity != "lord"
        || relay_order(g, seat).is_empty()
    {
        return Vec::new();
    }
    vec![("hujia".to_string(), "护驾：令魏势力角色代出【闪】".to_string())]
}

pub fn start_relay(g: &mut Game, lord: usize, _k: crate::cards::ResponseKind, _u: Option<&CardId>) {
    let order = relay_order(g, lord);
    push(g, "hujia", lord, json!({ "stage": "relay", "order": order, "cursor": 0 }));
}

fn hujia(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let lord = a.actor;
    let ev = a.data["event"].as_str().unwrap_or_default();
    if a.data["stage"].as_str() == Some("relay") {
        let order: Vec<usize> = a.data["order"].as_array()
            .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
        if cursor >= order.len() {
            g.relay_none = true;
            return Ok(Step::Done);
        }
        let by = order[cursor];
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(by, "choice").skill("hujia");
                for id in g.hand(by) {
                    if g.def_of(&id).tags.shan {
                        b = b.option(format!("respond:{id}"), format!("代替主公打出【{}】", label(g, &id)));
                    }
                }
                Ok(Step::Need(b.escape("不代替打出").build()))
            }
            Some("pass") => {
                push(g, "hujia", lord, json!({ "stage": "relay", "order": order, "cursor": cursor + 1 }));
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act.strip_prefix("respond:")
                    .ok_or_else(|| format!("护驾：未知选项 {act}"))?
                    .to_string();
                if !g.hand(by).contains(&id) || !g.def_of(&id).tags.shan {
                    return Err("护驾：只能代出【闪】".into());
                }
                g.to_discard(&id, "hujia")?;
                g.push_log(format!("护驾：{} 代替主公打出【{}】", g.seats[by].name, label(g, &id)));
                g.relay_played = Some((id, by));
                Ok(Step::Done)
            }
        }
    } else {
        let Some(by) = a.data["seat"].as_u64().map(|n| n as usize) else { return Ok(Step::Done) };
        if by == lord || g.seats[by].kingdom != "wei" || g.turn == by {
            return Ok(Step::Done);
        }
        if !(ev.starts_with("CardUsed") || ev.starts_with("CardPlayed")) {
            return Ok(Step::Done);
        }
        let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else { return Ok(Step::Done) };
        if !g.def_of(&card).tags.shan || g.counter("turn.hujia_draw") > 0 {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(by, "choice").skill("hujia");
                b = b.option("opt1", "令主公摸一张牌");
                Ok(Step::Need(b.escape("不令主公摸牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.bump("turn.hujia_draw");
                let n = g.draw(lord, 1).len();
                g.push_log(format!("护驾：{} 令主公摸 {n} 张牌", g.seats[by].name));
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 反馈
pub const FANKUI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "fankui", name: "反馈", triggers: &[EventKind::DamageAfter], window: Window::AnyTime,
    priority: 0, can_active: None, handler: fankui, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn fankui(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let (src, left) = if a.data["stage"].as_str() == Some("more") {
        (a.data["src"].as_u64().unwrap_or(0) as usize, a.data["left"].as_u64().unwrap_or(0) as usize)
    } else {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        let Some(src) = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok()) else {
            return Ok(Step::Done);
        };
        if src == owner || g.seats.get(src).map(|s| s.hp <= 0).unwrap_or(true) {
            return Ok(Step::Done);
        }
        (src, a.data["amount"].as_i64().unwrap_or(1).max(1) as usize)
    };
    if left == 0 || hand_and_equip(g, src).is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("fankui");
            b = b.option("opt1", format!("获得 {} 一张牌（剩 {left}）", g.seats[src].name));
            Ok(Step::Need(b.escape("不发动反馈").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            if let Some(id) = hand_and_equip(g, src).first().cloned() {
                let l = label(g, &id);
                g.gain(&id, owner)?;
                g.push_log(format!("反馈：获得 {} 的【{l}】", g.seats[src].name));
            }
            if left > 1 {
                push(g, "fankui", owner, json!({ "stage": "more", "src": src, "left": left - 1 }));
            }
            Ok(Step::Done)
        }
    }
}

// ------------------------------------------------------------------ 鬼才
pub const GUICAI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "guicai", name: "鬼才", triggers: &[], window: Window::AnyTime,
    priority: 0, can_active: None, handler: guicai, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

pub fn active_guicai(g: &Game, seat: usize) -> bool {
    active(g, seat, "guicai")
}

pub fn start_guicai(g: &mut Game, seat: usize) {
    push(g, "guicai", seat, json!({}));
}

fn guicai(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("guicai");
            for id in hand_and_equip(g, owner) {
                b = b.option(format!("pick:{id}"), format!("用【{}】代替判定牌", label(g, &id)));
            }
            Ok(Step::Need(b.escape("不改判").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act.strip_prefix("pick:")
                .ok_or_else(|| format!("鬼才：未知选项 {act}"))?
                .to_string();
            if !hand_and_equip(g, owner).contains(&id) {
                return Err("鬼才：该牌不在手牌/装备区".into());
            }
            if g.zone_of(&id) == Some(&Zone::Hand(owner)) {
                g.to_processing(&id)?;
            } else if let Some(slot) = g.def_of(&id).slot {
                g.discard_equip_slot(owner, slot, "guicai")?;
                g.to_processing(&id)?;
            }
            g.judge_rewrite = Some(id.clone());
            g.push_log(format!("鬼才：用【{}】代替判定牌", label(g, &id)));
            Ok(Step::Done)
        }
    }
}

// ------------------------------------------------------------------ 刚烈
pub const GANGLIE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "ganglie", name: "刚烈", triggers: &[EventKind::DamageAfter], window: Window::AnyTime,
    priority: 0, can_active: None, handler: ganglie, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn ganglie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("resolve") {
        let src = a.data["src"].as_u64().unwrap_or(0) as usize;
        let Some(card) = g.judge_result.take() else { return Ok(Step::Done) };
        if g.cards.contains_key(&card) {
            g.to_discard(&card, "ganglie")?;
        }
        if g.is_red(&card) {
            g.push_log(format!("刚烈：判定为红色 → 对 {} 造成 1 点伤害", g.seats[src].name));
            flow::damage(g, Some(owner), src, 1);
        } else if let Some(id) = hand_and_equip(g, src).first().cloned() {
            let l = label(g, &id);
            g.to_discard(&id, "ganglie")?;
            g.push_log(format!("刚烈：判定为黑色 → 弃置 {} 的【{l}】", g.seats[src].name));
        }
        return Ok(Step::Done);
    }
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
        return Ok(Step::Done);
    }
    let Some(src) = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok()) else {
        return Ok(Step::Done);
    };
    if src == owner || g.seats.get(src).map(|s| s.hp <= 0).unwrap_or(true) {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("ganglie");
            b = b.option("opt1", format!("发动刚烈（对 {} 判定）", g.seats[src].name));
            Ok(Step::Need(b.escape("不发动刚烈").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            push(g, "ganglie", owner, json!({ "stage": "resolve", "src": src }));
            crate::kernel::card_play::start_quick_judge(g, owner, "ganglie");
            Ok(Step::Done)
        }
    }
}

// ------------------------------------------------------------------ 清俭
pub const QINGJIAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qingjian", name: "清俭", triggers: &[EventKind::CardGained], window: Window::AnyTime,
    priority: 0, can_active: None, handler: qingjian, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn qingjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("pick") {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let kinds = a.data["kinds"].as_u64().unwrap_or(0) as u32;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("qingjian");
                for id in hand_and_equip(g, owner) {
                    b = b.option(format!("pick:{id}"), format!("交出【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("结束清俭").build()))
            }
            Some("pass") => {
                let n = kinds.count_ones();
                if n > 0 {
                    g.bump("turn.qingjian_used");
                    let cur = g.turn;
                    g.counters
                        .entry(format!("turn.qingjian_limit:{cur}"))
                        .and_modify(|v| *v += n)
                        .or_insert(n);
                    g.push_log(format!("清俭：共给出 {n} 种类型，当前回合角色手牌上限 +{n}"));
                }
                Ok(Step::Done)
            }
            Some(act) => {
                let id = act.strip_prefix("pick:")
                    .ok_or_else(|| format!("清俭：未知选项 {act}"))?
                    .to_string();
                if !hand_and_equip(g, owner).contains(&id) {
                    return Err("清俭：该牌不在可用区域".into());
                }
                let bit = match g.def_of(&id).kind {
                    CardKind::Basic => 1u32,
                    CardKind::Trick => 2,
                    CardKind::Equip => 4,
                };
                g.reveal(&id, Some(owner), "qingjian");
                g.gain(&id, target)?;
                g.push_log(format!("清俭：交给 {} 【{}】", g.seats[target].name, label(g, &id)));
                push(g, "qingjian", owner, json!({ "stage": "pick", "target": target, "kinds": kinds | bit }));
                Ok(Step::Done)
            }
        }
    } else {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        if g.counter("turn.qingjian_used") > 0 || hand_and_equip(g, owner).is_empty() {
            return Ok(Step::Done);
        }
        // 「摸牌阶段外获得牌」：读事件 cause（发牌/摸牌阶段），不要读 live `g.phase`
        // ——事件在摸牌阶段摸完后才路由，此时 phase 已是 Play（engine-self-audit M1）。
        if a.data["cause"].as_str() == Some("draw_phase") {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("qingjian");
                for i in other_alive(g, owner) {
                    b = b.option(format!("target:{i}"), format!("交给 {}", g.seats[i].name));
                }
                Ok(Step::Need(b.escape("不发动清俭").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("清俭：目标不合法 {act}"))?;
                if t == owner || g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) {
                    return Err("清俭：目标不合法".into());
                }
                push(g, "qingjian", owner, json!({ "stage": "pick", "target": t, "kinds": 0 }));
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 突袭
pub const TUXI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "tuxi", name: "突袭", triggers: &[EventKind::PhaseStart], window: Window::OwnPhase(Phase::Draw),
    priority: 0, can_active: None, handler: tuxi, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn tuxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("take") {
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        if left == 0 {
            return Ok(Step::Done);
        }
        let cands: Vec<usize> = other_alive(g, owner).into_iter().filter(|i| !g.hand(*i).is_empty()).collect();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("tuxi");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("获得 {} 一张手牌（剩 {left}）", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("结束结算").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act.strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("突袭：目标不合法 {act}"))?;
                if !cands.contains(&t) {
                    return Err("突袭：目标不合法".into());
                }
                if let Some(id) = g.hand(t).first().cloned() {
                    let l = label(g, &id);
                    g.gain(&id, owner)?;
                    g.push_log(format!("突袭：获得 {} 的【{l}】", g.seats[t].name));
                }
                if left > 1 {
                    push(g, "tuxi", owner, json!({ "stage": "take", "left": left - 1 }));
                }
                Ok(Step::Done)
            }
        }
    } else {
        let n = 2 + crate::skills::draw_bonus(g, owner);
        let cands: Vec<usize> = other_alive(g, owner).into_iter().filter(|i| !g.hand(*i).is_empty()).collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("tuxi");
                for x in 0..=n {
                    b = b.option(format!("draw:{x}"), format!("摸 {x} 张，并向 {} 名角色各拿一张手牌", n - x));
                }
                Ok(Step::Need(b.escape("不发动突袭").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let x: usize = act.strip_prefix("draw:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("突袭：未知选项 {act}"))?;
                if x > n {
                    return Err("突袭：摸牌数超过上限".into());
                }
                if x > 0 {
                    g.draw(owner, x);
                    g.push_log(format!("突袭：摸 {x} 张"));
                }
                if n - x > 0 {
                    push(g, "tuxi", owner, json!({ "stage": "take", "left": n - x }));
                }
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 裸衣
pub const LUOYI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "luoyi", name: "裸衣", triggers: &[EventKind::PhaseStart], window: Window::OwnPhase(Phase::Draw),
    priority: 5, can_active: None, handler: luoyi, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn luoyi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("choose") {
        let ids: Vec<CardId> = a.data["cards"].as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("luoyi");
                b = b.option("opt1", "跳过摸牌阶段，获得其中的基本牌/武器/【决斗】");
                Ok(Step::Need(b.escape("放弃（正常摸 2 张）").build()))
            }
            Some("pass") => {
                for id in &ids {
                    if g.cards.contains_key(id) {
                        g.to_discard(id, "luoyi")?;
                    }
                }
                g.draw(owner, 2);
                g.push_log("裸衣：放弃，展示牌进弃牌堆并正常摸 2 张".to_string());
                Ok(Step::Done)
            }
            Some(_) => {
                let mut kept = Vec::new();
                for id in &ids {
                    if !g.cards.contains_key(id) {
                        continue;
                    }
                    let def = g.def_of(id);
                    let keep = def.kind == CardKind::Basic
                        || def.tags.duel
                        || def.slot.is_some_and(|s| s == crate::kernel::Slot::Weapon);
                    if keep {
                        g.gain(id, owner)?;
                        kept.push(label(g, id));
                    } else {
                        g.to_discard(id, "luoyi")?;
                    }
                }
                g.counters.insert("turn.skip_draw".to_string(), 1);
                g.seats[owner].flags.insert("luoyi_bonus".to_string());
                g.push_log(format!(
                    "裸衣：跳过摸牌，获得【{}】；直到下回合开始【杀】/【决斗】伤害 +1",
                    kept.join("、")
                ));
                Ok(Step::Done)
            }
        }
    } else {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("luoyi");
                b = b.option("opt1", "亮出牌堆顶三张");
                Ok(Step::Need(b.escape("不发动裸衣").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let ids: Vec<CardId> = g.ids_in(|z| *z == Zone::Draw).into_iter().take(3).collect();
                for id in &ids {
                    g.to_processing(id)?;
                    g.reveal(id, Some(owner), "luoyi");
                }
                push(g, "luoyi", owner, json!({ "stage": "choose", "cards": ids }));
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 天妒
pub const TIANDU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "tiandu", name: "天妒", triggers: &[], window: Window::AnyTime,
    priority: 0, can_active: None, handler: tiandu, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

pub fn start_tiandu(g: &mut Game, owner: usize, card: &CardId) -> bool {
    if !active(g, owner, "tiandu") || !g.cards.contains_key(card) {
        return false;
    }
    push(g, "tiandu", owner, json!({ "card": card }));
    true
}

fn tiandu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let card = a.data["card"].as_str().unwrap_or_default().to_string();
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("tiandu");
            b = b.option("opt1", format!("获得判定牌【{}】", label(g, &card)));
            Ok(Step::Need(b.escape("放弃").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(_) => {
            if g.cards.contains_key(&card) {
                g.gain(&card, owner)?;
                g.push_log(format!("天妒：获得判定牌【{}】", label(g, &card)));
            }
            Ok(Step::Done)
        }
    }
}

// ------------------------------------------------------------------ 遗计
pub const YIJI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "yiji", name: "遗计", triggers: &[EventKind::DamageAfter], window: Window::AnyTime,
    priority: 0, can_active: None, handler: yiji, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn yiji(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("give") {
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        if left == 0 {
            return Ok(Step::Done);
        }
        let Some(card) = g.hand(owner).last().cloned() else { return Ok(Step::Done) };
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("yiji");
                for i in other_alive(g, owner) {
                    b = b.option(format!("target:{i}"),
                        format!("把【{}】交给 {}（剩 {left}）", label(g, &card), g.seats[i].name));
                }
                Ok(Step::Need(b.escape("自己保留").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act.strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("遗计：目标不合法 {act}"))?;
                if t == owner || g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) {
                    return Err("遗计：目标不合法".into());
                }
                g.gain(&card, t)?;
                g.push_log(format!("遗计：把【{}】交给 {}", label(g, &card), g.seats[t].name));
                if left > 1 {
                    push(g, "yiji", owner, json!({ "stage": "give", "left": left - 1 }));
                }
                Ok(Step::Done)
            }
        }
    } else {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("yiji");
                b = b.option("opt1", "发动遗计（摸两张并分配）");
                Ok(Step::Need(b.escape("不发动遗计").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let ids = g.draw(owner, 2);
                if !ids.is_empty() {
                    push(g, "yiji", owner, json!({ "stage": "give", "left": 2 }));
                }
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 恂恂
pub const XUNXUN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "xunxun", name: "恂恂", triggers: &[EventKind::PhaseStart], window: Window::OwnPhase(Phase::Draw),
    priority: 10, can_active: None, handler: xunxun, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn xunxun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("pick") {
        let cards: Vec<CardId> = a.data["cards"].as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let mut top: Vec<CardId> = a.data["top"].as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let mut bottom: Vec<CardId> = a.data["bottom"].as_array()
            .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        if top.len() + bottom.len() >= cards.len() {
            for id in bottom.iter().rev() {
                g.put_on_bottom(id)?;
            }
            for id in top.iter().rev() {
                g.put_on_top(id)?;
            }
            g.clear_private_reveals(owner);
            g.push_log(format!("恂恂：{} 张置于牌堆顶、{} 张置于牌堆底", top.len(), bottom.len()));
            return Ok(Step::Done);
        }
        let Some(card) = cards.iter().find(|c| !top.contains(c) && !bottom.contains(c)).cloned() else {
            return Ok(Step::Done);
        };
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("xunxun");
                b = b.option("top", format!("【{}】置于牌堆顶", label(g, &card)));
                b = b.option("bottom", format!("【{}】置于牌堆底", label(g, &card)));
                Ok(Step::Need(b.escape("全部置于牌堆底").build()))
            }
            Some(ans) => {
                if ans == "top" {
                    top.push(card);
                } else {
                    bottom.push(card);
                }
                push(g, "xunxun", owner,
                     json!({ "stage": "pick", "cards": cards, "top": top, "bottom": bottom }));
                Ok(Step::Done)
            }
        }
    } else {
        let ids: Vec<CardId> = g.ids_in(|z| *z == Zone::Draw).into_iter().take(4).collect();
        if ids.len() < 2 {
            return Ok(Step::Done);
        }
        for id in &ids {
            g.to_processing(id)?;
            g.reveal_private(owner, id, "xunxun");
        }
        push(g, "xunxun", owner, json!({ "stage": "pick", "cards": ids, "top": [], "bottom": [] }));
        Ok(Step::Done)
    }
}

// ------------------------------------------------------------------ 忘隙
pub const WANGXI: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "wangxi", name: "忘隙", triggers: &[EventKind::DamageAfter], window: Window::AnyTime,
    priority: 0, can_active: None, handler: wangxi, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn wangxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("pick") {
        let other = a.data["other"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("wangxi");
                for id in g.hand(owner) {
                    b = b.option(format!("pick:{id}"), format!("交给【{}】", label(g, &id)));
                }
                Ok(Step::Need(b.escape("不交牌").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act.strip_prefix("pick:")
                    .ok_or_else(|| format!("忘隙：未知选项 {act}"))?
                    .to_string();
                if g.zone_of(&id) != Some(&Zone::Hand(owner)) {
                    return Err("忘隙：该牌不在手牌".into());
                }
                g.gain(&id, other)?;
                g.push_log(format!("忘隙：交给 {} 【{}】", g.seats[other].name, label(g, &id)));
                Ok(Step::Done)
            }
        }
    } else {
        let victim = a.data["seat"].as_u64().map(|n| n as usize);
        let source = a.data["cause"].as_str().and_then(|c| c.parse::<usize>().ok());
        let other = if victim == Some(owner) {
            match source { Some(s) if s != owner => s, _ => return Ok(Step::Done) }
        } else if source == Some(owner) {
            match victim { Some(v) if v != owner => v, _ => return Ok(Step::Done) }
        } else {
            return Ok(Step::Done);
        };
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("wangxi");
                b = b.option("opt1", "摸两张并交给其一张");
                Ok(Step::Need(b.escape("不发动忘隙").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                g.draw(owner, 2);
                if g.seats.get(other).map(|s| s.hp <= 0).unwrap_or(true) {
                    g.push_log("忘隙：对方已离场，两张牌都归李典".to_string());
                    return Ok(Step::Done);
                }
                push(g, "wangxi", owner, json!({ "stage": "pick", "other": other }));
                Ok(Step::Done)
            }
        }
    }
}

// ------------------------------------------------------------------ 倾国
pub const QINGGUO: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "qingguo", name: "倾国", triggers: &[], window: Window::AnyTime,
    priority: 0, can_active: None, handler: noop, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    if !active(g, seat, "qingguo") {
        return Vec::new();
    }
    hand_and_equip(g, seat)
        .into_iter()
        .filter(|id| !g.is_red(id))
        .map(|id| super::Conversion { skill: "qingguo", from: id, to: "闪", usable: false, playable: true })
        .collect()
}

// ------------------------------------------------------------------ 洛神
pub const LUOSHEN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "luoshen", name: "洛神", triggers: &[EventKind::PhaseStart], window: Window::OwnPhase(Phase::Prepare),
    priority: 0, can_active: None, handler: luoshen, dying_hook: None, flip_hook: None,
    wuxie_opts: None, wuxie_use: None, target_blocker: None, yang_window: None, yin_window: None,
    subject_window: None,
};

fn luoshen(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("resolve") {
        let Some(card) = g.judge_result.take() else { return Ok(Step::Done) };
        if g.is_red(&card) {
            if g.cards.contains_key(&card) {
                g.to_discard(&card, "luoshen")?;
            }
            g.push_log(format!("洛神：判定【{}】为红色，结束洛神", label(g, &card)));
            return Ok(Step::Done);
        }
        if g.cards.contains_key(&card) {
            g.gain(&card, owner)?;
            g.bump(&format!("turn.luoshen_free:{owner}"));
            g.push_log(format!("洛神：判定【{}】为黑色，获得之（继续）", label(g, &card)));
        }
        push(g, "luoshen", owner, json!({ "stage": "ask" }));
        Ok(Step::Done)
    } else {
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("luoshen");
                b = b.option("opt1", "判定（黑色则获得并继续）");
                Ok(Step::Need(b.escape("结束洛神").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                push(g, "luoshen", owner, json!({ "stage": "resolve" }));
                crate::kernel::card_play::start_quick_judge(g, owner, "luoshen");
                Ok(Step::Done)
            }
        }
    }
}

pub const B11_SKILLS: &[SkillDef] = &[
    JIANXIONG, HUJIA, FANKUI, GUICAI, GANGLIE, QINGJIAN, TUXI, LUOYI, TIANDU, YIJI, XUNXUN,
    WANGXI, QINGGUO, LUOSHEN,
];
