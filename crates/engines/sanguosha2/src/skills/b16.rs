//! B16（界限突破·J4）——按 `align/B16.json` 第 7 段实现。
//!
//! 覆盖：界孟获（祸首/再起）、界徐晃（断粮/截辎）、界蔡文姬（断肠/悲歌）、界荀彧（驱虎/节命）、
//! 界典韦（强袭/狞恶）、界颜良文丑（双雄）、界张昭张纮（直谏/固政）、界徐盛（破军）。
//!
//! 用户裁定（2026-11-09，42 条疑点）要点：
//! - 祸首：不能成为【南蛮入侵】目标；其他角色使用南蛮时伤害来源=座位序最先的祸首持有者；自己使用不替换。
//! - 再起：任何方式进入弃牌堆的红色牌都计数；可重复选同一名角色；摸 1/回复 1 由被选者选。
//! - 断粮：黑色基本牌/装备牌（手牌 + 装备区）当【兵粮寸断】；本回合未造成伤害 → 无距离（实时）。
//! - 截辎：「辎」在摸牌阶段**结束**时移除，然后执行**完整额外摸牌阶段**；并列最少都算；含徐晃自己。
//! - 断肠：失去所有技能（含锁定/主公技）；无来源死亡不触发；双蔡文姬不再互相触发。
//! - 悲歌：弃牌含装备区；受伤时检查有牌；界版「花色相同获得判定牌」+「点数相同获得弃置牌」可叠加。
//! - 驱虎：目标须手牌非空；可选荀彧自己受伤；伤害走伤害入口但**不被防具减免**。
//! - 节命：实现「死亡时」触发；X=当前体力上限（≤5）；濒死也能发动（救起/死亡两条路径各一次）。
//! - 强袭：代价伤害无来源；本回合计数；代价濒死后救起仍继续。
//! - 狞恶：弃置其「装备区一张或随机手牌」（不含判定区）；每名角色各自第 2 次；由典韦选。
//! - 双雄：弃牌含装备区；颜色按实体牌；结束阶段只从弃牌堆/处理区取回（含自己发起的转化【决斗】）。
//! - 直谏：无次数限制；旧装备视为弃置并触发失去装备钩子；摸 1 走正常摸牌。
//! - 固政：每角色每阶段限一次；按**单次弃置动作 ≥2 张**触发；由固政持有者选交给弃牌者的那张。
//! - 破军：移出不含判定区；伤害 +1 结算时实时比较；本回合结束自动归还。

use serde_json::json;

use crate::cards::{CardKind, Effect};
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, CardId, Game, Phase, Slot, Zone};
use crate::presentation::{POutcome, Presentation};

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
fn is_red(g: &Game, id: &CardId) -> bool {
    g.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false)
}
fn random_hand(g: &mut Game, seat: usize) -> Option<CardId> {
    let n = g.hand(seat).len();
    if n == 0 {
        return None;
    }
    let idx = g.rand_below(n);
    Some(g.hand(seat)[idx].clone())
}
fn alive(g: &Game) -> Vec<usize> {
    (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect()
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
// 祸首（界孟获）
// ---------------------------------------------------------------------------

pub const HUOSHOU: SkillDef = SkillDef {
    code: "huoshou",
    name: "祸首",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(huoshou_blocker),
    yang_window: None,
    yin_window: None,
};

fn huoshou_blocker(
    _g: &Game,
    seat: usize,
    _source: usize,
    name: &str,
    _carrier: Option<&CardId>,
) -> bool {
    let _ = seat;
    name == "南蛮入侵"
}

/// 祸首：其他角色使用【南蛮入侵】时，伤害来源改为座位序最先的祸首持有者。
pub fn huoshou_source(g: &Game, user: usize, card: Option<&CardId>) -> Option<usize> {
    let card = card?;
    if g.def_of(card).effect != Effect::SavageAssault {
        return None;
    }
    // 自己就是祸首持有者 → 来源仍为自己（用户裁定 Q3=B）。
    if active(g, user, "huoshou") {
        return None;
    }
    (0..g.seats.len()).find(|i| *i != user && g.seats[*i].hp > 0 && active(g, *i, "huoshou"))
}

// ---------------------------------------------------------------------------
// 再起（界孟获）：结束阶段，按本回合红色牌进弃牌堆数逐次结算
// ---------------------------------------------------------------------------

pub const ZAIZHI: SkillDef = SkillDef {
    code: "zaizhi",
    name: "再起",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: zaizhi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zaizhi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if g.phase != Phase::End || !active(g, owner, "zaizhi") {
        return Ok(Step::Done);
    }
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    let remaining = a.data["remaining"].as_u64().unwrap_or(0) as usize;
    if stage.is_empty() {
        let n = g.counter("turn.red_discarded") as usize;
        if n == 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "target", "remaining": n }),
        ));
    }
    if remaining == 0 {
        return Ok(Step::Done);
    }
    if stage == "target" {
        let cands = alive(g);
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "target").skill("zaizhi");
                for t in &cands {
                    b = b.option(format!("target:{t}"), format!("再起：选择 {}", g.seats[*t].name));
                }
                Ok(Step::Need(b.escape("结束再起").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t = act
                    .strip_prefix("target:")
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or_else(|| format!("再起：未知选项 {act}"))?;
                if !cands.contains(&t) {
                    return Err("再起：目标不合法".into());
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "choice", "remaining": remaining, "target": t }),
                ))
            }
        }
    } else {
        // choice：由被选中的角色选择摸 1 / 令孟获回复 1。
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(target, "choice").skill("zaizhi");
                b = b.option("opt1", "再起：摸一张牌");
                b = b.option("opt2", format!("再起：令 {} 回复 1 点体力", g.seats[owner].name));
                Ok(Step::Need(b.escape_mandatory("（不能放弃）不选择").build()))
            }
            Some("opt1") => {
                let n = g.draw_with_cause(target, 1, Some("zaizhi")).len();
                g.push_log(format!("再起：{} 摸 {n} 张", g.seats[target].name));
                next_round(g, owner, remaining)
            }
            Some(_) => {
                flow::heal(g, owner, 1, "zaizhi");
                g.push_log(format!("再起：{} 回复 1 点体力", g.seats[owner].name));
                next_round(g, owner, remaining)
            }
        }
    }
}

fn next_round(g: &mut Game, owner: usize, remaining: usize) -> Result<Step, String> {
    if remaining > 1 {
        Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "target", "remaining": remaining - 1 }),
        ))
    } else {
        let _ = owner;
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 断粮（界徐晃）：转化 + 无距离
// ---------------------------------------------------------------------------

pub const DUANLIANG: SkillDef = SkillDef {
    code: "duanliang",
    name: "断粮",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
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

/// 断粮：本回合未造成过伤害 → 【兵粮寸断】无距离限制。
pub fn duanliang_no_distance(g: &Game, seat: usize) -> bool {
    active(g, seat, "duanliang") && g.counter(&format!("turn.damage_by:{seat}")) == 0
}

// ---------------------------------------------------------------------------
// 截辎（界徐晃）
// ---------------------------------------------------------------------------

pub const JIEZI: SkillDef = SkillDef {
    code: "jiezi",
    name: "截辎",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jiezi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zai(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get("zai").copied())
        .unwrap_or(0)
}
fn add_zai(g: &mut Game, seat: usize) {
    *g.seats[seat].counters.entry("zai".to_string()).or_insert(0) += 1;
}
fn remove_zai(g: &mut Game, seat: usize) {
    let n = zai(g, seat);
    if n == 0 {
        return;
    }
    if n == 1 {
        g.seats[seat].counters.remove("zai");
    } else {
        g.seats[seat].counters.insert("zai".to_string(), n - 1);
    }
}

/// 一名角色跳过摸牌阶段后（`draw_phase` 跳过分支）。
pub fn on_skip_draw(g: &mut Game, skipper: usize) {
    let owner = (0..g.seats.len()).find(|i| g.seats[*i].hp > 0 && active(g, *i, "jiezi"));
    if let Some(owner) = owner {
        push(g, "jiezi", owner, json!({ "stage": "ask", "skipper": skipper }));
    }
}

fn jiezi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if !active(g, owner, "jiezi") {
        return Ok(Step::Done);
    }
    let cands = alive(g);
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "target").skill("jiezi");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("截辎：选择 {}（手牌 {}）", g.seats[*t].name, g.hand(*t).len()),
                );
            }
            Ok(Step::Need(b.escape("不发动截辎").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t = act
                .strip_prefix("target:")
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or_else(|| format!("截辎：未知选项 {act}"))?;
            if !cands.contains(&t) {
                return Err("截辎：目标不合法".into());
            }
            let min = alive(g).into_iter().map(|i| g.hand(i).len()).min().unwrap_or(0);
            if g.hand(t).len() == min && zai(g, t) == 0 {
                add_zai(g, t);
                g.push_log(format!(
                    "截辎：{} 获得「辎」（手牌全场最少）",
                    g.seats[t].name
                ));
            } else {
                let n = g.draw_with_cause(t, 1, Some("jiezi")).len();
                g.push_log(format!("截辎：{} 摸 {n} 张（不满足「辎」条件）", g.seats[t].name));
            }
            Ok(Step::Done)
        }
    }
}

/// 摸牌阶段结束：有「辎」者移除「辎」并执行一个额外摸牌阶段。
pub fn after_draw_phase(g: &mut Game, seat: usize) {
    // ① 截辎：移除「辎」→ 额外摸牌阶段（完整的一次摸牌，含摸牌加成）。
    if zai(g, seat) > 0 {
        remove_zai(g, seat);
        let n = 2 + super::draw_bonus(g, seat);
        let drawn = g
            .draw_with_cause(seat, n, Some("extra_draw_phase"))
            .len();
        g.push_log(format!(
            "截辎：{} 移除「辎」，执行额外摸牌阶段（摸 {drawn} 张）",
            g.seats[seat].name
        ));
    }
    // ② 双雄（界颜良文丑）：摸牌阶段结束时弃一张牌（每回合一次，可选）。
    if active(g, seat, "shuangxiong") && g.counter("turn.sx_offered") == 0 {
        let opts = hand_and_equip(g, seat);
        if !opts.is_empty() {
            g.bump("turn.sx_offered");
            push(g, "shuangxiong", seat, json!({ "stage": "offer" }));
        }
    }
}

// ---------------------------------------------------------------------------
// 断肠（界蔡文姬）：死亡时杀死者失去所有技能
// ---------------------------------------------------------------------------

pub const DUANCHANG: SkillDef = SkillDef {
    code: "duanchang",
    name: "断肠",
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
// 悲歌（界蔡文姬）
// ---------------------------------------------------------------------------

pub const BEIGE: SkillDef = SkillDef {
    code: "beige",
    name: "悲歌",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: beige,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn beige(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    if a.data["stage"].as_str() == Some("discard") {
        // 梅花分支：伤害来源弃置两张牌（由来源自己逐张选择）。
        let left = a.data["left"].as_u64().unwrap_or(0) as usize;
        if left == 0 || !active(g, owner, "beige") && false {
            return Ok(Step::Done);
        }
        let opts = hand_and_equip(g, owner);
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        return match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("beige");
                for id in &opts {
                    b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                }
                Ok(Step::Need(b.escape("无牌可弃").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("悲歌：未知选项 {act}"))?
                    .to_string();
                if !opts.contains(&id) {
                    return Err("悲歌：该牌不在手牌/装备区".into());
                }
                g.to_discard(&id, "beige")?;
                if left > 1 && !hand_and_equip(g, owner).is_empty() {
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "discard", "left": left - 1 }),
                    ))
                } else {
                    Ok(Step::Done)
                }
            }
        };
    } else if a.data["stage"].as_str() == Some("after") {
        let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
        let source = a.data["source"].as_u64().map(|n| n as usize);
        let discarded = a.data["discarded"].as_str().unwrap_or_default().to_string();
        let Some(card) = g.judge_result.take() else {
            return Ok(Step::Done);
        };
        let suit = g.cards.get(&card).map(|c| c.suit);
        let rank = g.cards.get(&card).map(|c| c.rank);
        let text = format!("悲歌：判定【{}】", label(g, &card));
        match suit {
            Some(crate::kernel::Suit::Heart) => {
                flow::heal(g, victim, 1, "beige");
                g.push_log(format!("{text}红桃，{} 回复 1 点体力", g.seats[victim].name));
            }
            Some(crate::kernel::Suit::Diamond) => {
                let n = g.draw_with_cause(victim, 2, Some("beige")).len();
                g.push_log(format!("{text}方片，{} 摸 {n} 张", g.seats[victim].name));
            }
            Some(crate::kernel::Suit::Club) => {
                if let Some(src) = source {
                    g.push_log(format!("{text}梅花，{} 需弃置 2 张牌", g.seats[src].name));
                    push(g, "beige", src, json!({ "stage": "discard", "left": 2 }));
                }
            }
            Some(crate::kernel::Suit::Spade) => {
                if let Some(src) = source {
                    g.flip(src);
                    g.push_log(format!("{text}黑桃，{} 翻面", g.seats[src].name));
                }
            }
            None => {}
        }
        // 界版：花色相同 → 获得判定牌；点数相同 → 获得弃置牌（可叠加）。
        if active(g, owner, "beige") {
            let disc_suit = g.cards.get(&discarded).map(|c| c.suit);
            let disc_rank = g.cards.get(&discarded).map(|c| c.rank);
            if suit.is_some() && suit == disc_suit && matches!(g.zone_of(&card), Some(Zone::Discard)) {
                g.gain(&card, owner)?;
                g.push_log(format!("悲歌：判定牌与弃置牌花色相同，{} 获得判定牌", g.seats[owner].name));
            }
            if rank.is_some()
                && rank == disc_rank
                && matches!(g.zone_of(&discarded), Some(Zone::Discard))
            {
                g.gain(&discarded, owner)?;
                g.push_log(format!("悲歌：点数相同，{} 获得弃置的牌", g.seats[owner].name));
            }
        }
        return Ok(Step::Done);
    }
    if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
        return Ok(Step::Done);
    }
    if !active(g, owner, "beige") {
        return Ok(Step::Done);
    }
    let Some(src) = a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok()) else {
        return Ok(Step::Done);
    };
    let Some(card) = a.data["card"].as_str().map(str::to_string) else {
        return Ok(Step::Done);
    };
    if !is_sha(g, &card) {
        return Ok(Step::Done);
    }
    let victim = a.data["seat"].as_u64().unwrap_or(0) as usize;
    let opts = hand_and_equip(g, owner);
    if opts.is_empty() {
        return Ok(Step::Done);
    }
    let key = format!("turn.beige_asked:{card}:{victim}");
    if g.counter(&key) > 0 && g.answer.is_none() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            g.bump(&key);
            let mut b = builder(owner, "choice").skill("beige");
            for id in &opts {
                b = b.option(format!("pick:{id}"), format!("悲歌：弃置【{}】", label(g, id)));
            }
            Ok(Step::Need(b.escape("不发动悲歌").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("悲歌：未知选项 {act}"))?
                .to_string();
            if !opts.contains(&id) {
                return Err("悲歌：该牌不在手牌/装备区".into());
            }
            g.to_discard(&id, "beige")?;
            g.push_log(format!("悲歌：{} 弃置【{}】并令其判定", g.seats[owner].name, label(g, &id)));
            push(
                g,
                "beige",
                owner,
                json!({ "stage": "after", "victim": victim, "source": src, "discarded": id }),
            );
            crate::kernel::card_play::start_quick_judge(g, owner, "beige");
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 驱虎（界荀彧）
// ---------------------------------------------------------------------------

pub const QUHU: SkillDef = SkillDef {
    code: "quhu",
    name: "驱虎",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_quhu),
    handler: quhu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn can_quhu(g: &Game, seat: usize) -> bool {
    active(g, seat, "quhu")
        && play_phase(g, seat)
        && g.counter("phase.cancel_quhu") == 0
        && g.counter("turn.quhu_used") == 0
        && !g.hand(seat).is_empty()
        && other_alive(g, seat).into_iter().any(|t| {
            g.seats[t].hp > g.seats[seat].hp && !g.hand(t).is_empty()
        })
}

fn quhu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    match stage.as_str() {
        "" => {
            let cands: Vec<usize> = other_alive(g, owner)
                .into_iter()
                .filter(|t| g.seats[*t].hp > g.seats[owner].hp && !g.hand(*t).is_empty())
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("quhu");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("驱虎：与 {} 拼点", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动驱虎").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_quhu");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("驱虎：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("驱虎：目标不合法".into());
                    }
                    g.bump("turn.quhu_used");
                    g.push_log(format!("驱虎：{} 与 {} 拼点", g.seats[owner].name, g.seats[t].name));
                    push(g, "quhu", owner, json!({ "stage": "after", "target": t }));
                    crate::kernel::pindian::start(g, owner, t, "quhu");
                    Ok(Step::Done)
                }
            }
        }
        "after" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let Some(res) = g.pindian_result.take() else {
                return Ok(Step::Done);
            };
            if res.winner == Some(owner) {
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "victim", "target": target }),
                ))
            } else {
                // 没赢 → 其对荀彧造成 1 点伤害（不被防具减免）。
                g.push_log(format!(
                    "驱虎：拼点没赢，{} 对 {} 造成 1 点伤害",
                    g.seats[target].name, g.seats[owner].name
                ));
                flow::damage_now_no_armor(g, Some(target), owner, 1);
                Ok(Step::Done)
            }
        }
        "victim" => {
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| g.seats[*i].hp > 0 && g.distance(target, *i) <= g.attack_range(target))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("quhu");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("令 {} 对 {} 造成 1 点伤害", g.seats[target].name, g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不造成伤害").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("驱虎：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("驱虎：受伤角色不合法".into());
                    }
                    g.push_log(format!(
                        "驱虎：{} 对 {} 造成 1 点伤害",
                        g.seats[target].name, g.seats[t].name
                    ));
                    flow::damage_now_no_armor(g, Some(target), t, 1);
                    Ok(Step::Done)
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 节命（界荀彧）：受伤后 / 死亡时
// ---------------------------------------------------------------------------

pub const JIEMING: SkillDef = SkillDef {
    code: "jieming",
    name: "节命",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: jieming,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn jieming(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    match stage.as_str() {
        "" => {
            // 触发入口：受伤后（濒死救起后也能走到这里）；死亡由 `on_death` 单独触发。
            if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
                return Ok(Step::Done);
            }
            if !active(g, owner, "jieming") || g.seats[owner].hp <= 0 {
                return Ok(Step::Done);
            }
            Ok(Step::Next(Stage::Choose, json!({ "stage": "ask" })))
        }
        "ask" => {
            if !active(g, owner, "jieming") {
                return Ok(Step::Done);
            }
            let cands = alive(g);
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("jieming");
                    for t in &cands {
                        let x = (g.seats[*t].max_hp.max(0) as usize).min(5);
                        b = b.option(
                            format!("target:{t}"),
                            format!("节命：令 {} 摸 {x} 张并弃至 {x} 张", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不发动节命").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("节命：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("节命：目标不合法".into());
                    }
                    let x = (g.seats[t].max_hp.max(0) as usize).min(5);
                    let n = g.draw_with_cause(t, x, Some("jieming")).len();
                    g.push_log(format!(
                        "节命：{} 摸 {n} 张（随后弃至 {x} 张）",
                        g.seats[t].name
                    ));
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "discard", "target": t, "x": x }),
                    ))
                }
            }
        }
        "discard" => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let x = a.data["x"].as_u64().unwrap_or(0) as usize;
            if g.hand(t).len() <= x {
                return Ok(Step::Done);
            }
            let avail = g.hand(t);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(t, "choice").skill("jieming");
                    for id in &avail {
                        b = b.option(format!("pick:{id}"), format!("弃置【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape_mandatory("（不能放弃）结束").build()))
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("节命：未知选项 {act}"))?
                        .to_string();
                    if !avail.contains(&id) {
                        return Err("节命：该牌不在手牌".into());
                    }
                    g.to_discard(&id, "jieming")?;
                    if g.hand(t).len() <= x {
                        Ok(Step::Done)
                    } else {
                        Ok(Step::Next(
                            Stage::Choose,
                            json!({ "stage": "discard", "target": t, "x": x }),
                        ))
                    }
                }
            }
        }
        _ => Ok(Step::Done),
    }
}

/// 死亡钩子：断肠（杀死者失去所有技能）+ 节命（死亡时发动）。
pub fn on_death(g: &mut Game, victim: usize, source: Option<usize>) {
    // 断肠：只在有伤害来源时触发；双蔡文姬不再互相触发（用户裁定 Q3=B）。
    if active(g, victim, "duanchang") {
        if let Some(src) = source {
            if src != victim && !active(g, src, "duanchang") {
                g.seats[src].flags.insert("skills_lost".to_string());
                g.push_log(format!(
                    "断肠：{} 死亡，{} 失去所有技能",
                    g.seats[victim].name, g.seats[src].name
                ));
            }
        }
    }
    // 节命：死亡时仍可发动（在死亡清理之前）。
    if active(g, victim, "jieming") {
        push(g, "jieming", victim, json!({ "stage": "ask" }));
    }
}

// ---------------------------------------------------------------------------
// 强袭（界典韦）
// ---------------------------------------------------------------------------

pub const QIANGXI: SkillDef = SkillDef {
    code: "qiangxi",
    name: "强袭",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_qiangxi),
    handler: qiangxi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn qiangxi_mask(g: &Game) -> u32 {
    g.counter("turn.qiangxi_mask")
}
fn qiangxi_targets(g: &Game, actor: usize) -> Vec<usize> {
    let mask = qiangxi_mask(g);
    alive(g)
        .into_iter()
        .filter(|t| *t != actor && mask & (1u32 << *t) == 0)
        .collect()
}
fn can_qiangxi(g: &Game, seat: usize) -> bool {
    active(g, seat, "qiangxi")
        && play_phase(g, seat)
        && g.counter("phase.cancel_qiangxi") == 0
        && g.counter("turn.qiangxi_used") < 2
        && (g.seats[seat].hp > 0 || g.equip_in(seat, Slot::Weapon).is_some())
        && !qiangxi_targets(g, seat).is_empty()
}

fn qiangxi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("cost").to_string();
    match stage.as_str() {
        "cost" => {
            let mut opts: Vec<(String, String)> = Vec::new();
            opts.push(("opt:hp".to_string(), "强袭：受到 1 点伤害".to_string()));
            if let Some(w) = g.equip_in(owner, Slot::Weapon) {
                opts.push((
                    "opt:weapon".to_string(),
                    format!("强袭：弃置武器【{}】", label(g, &w)),
                ));
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("qiangxi");
                    for (id, text) in &opts {
                        b = b.option(id.clone(), text.clone());
                    }
                    Ok(Step::Need(b.escape("取消强袭").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_qiangxi");
                    Ok(Step::Done)
                }
                Some("opt:hp") => {
                    g.push_log(format!("强袭：{} 受到 1 点伤害作为代价", g.seats[owner].name));
                    flow::damage(g, None, owner, 1);
                    if g.seats[owner].hp <= 0 {
                        return Ok(Step::Done);
                    }
                    Ok(Step::Next(Stage::Choose, json!({ "stage": "target" })))
                }
                Some("opt:weapon") => {
                    if let Some(w) = g.equip_in(owner, Slot::Weapon) {
                        g.to_discard(&w, "qiangxi")?;
                        crate::skills::on_equip_lost(g, owner);
                        g.push_log(format!("强袭：{} 弃置武器【{}】", g.seats[owner].name, label(g, &w)));
                    }
                    Ok(Step::Next(Stage::Choose, json!({ "stage": "target" })))
                }
                Some(_) => Ok(Step::Done),
            }
        }
        _ => {
            let cands = qiangxi_targets(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("qiangxi");
                    for t in &cands {
                        b = b.option(
                            format!("target:{t}"),
                            format!("强袭：对 {} 造成 1 点伤害", g.seats[*t].name),
                        );
                    }
                    Ok(Step::Need(b.escape("不指定目标").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("强袭：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("强袭：目标不合法".into());
                    }
                    let mask = qiangxi_mask(g) | (1u32 << t);
                    g.counters.insert("turn.qiangxi_mask".to_string(), mask);
                    g.bump("turn.qiangxi_used");
                    g.push_log(format!("强袭：对 {} 造成 1 点伤害", g.seats[t].name));
                    flow::damage(g, Some(owner), t, 1);
                    Ok(Step::Done)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 狞恶（界典韦，锁定）
// ---------------------------------------------------------------------------

pub const NINGE: SkillDef = SkillDef {
    code: "ninge",
    name: "狞恶",
    triggers: &[EventKind::DamageAfter],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: ninge,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn ninge(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("DamageAfter") {
            return Ok(Step::Done);
        }
        if !active(g, owner, "ninge") {
            return Ok(Step::Done);
        }
        let victim = a.data["seat"].as_u64().unwrap_or(0) as usize;
        let source = a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok());
        if owner != victim && Some(owner) != source {
            return Ok(Step::Done);
        }
        if g.counter(&format!("turn.damage_taken:{victim}")) != 2 {
            return Ok(Step::Done);
        }
        let n = g.draw_with_cause(owner, 1, Some("ninge")).len();
        g.push_log(format!("狞恶：{} 摸 {n} 张", g.seats[owner].name));
        let opts = take_one_options(g, victim);
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "victim": victim }),
        ));
    }
    // pick：由典韦选择弃置其装备区一张或随机手牌。
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    let opts = take_one_options(g, victim);
    if opts.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("ninge");
            for (id, text) in &opts {
                b = b.option(id.clone(), format!("狞恶：弃置{text}"));
            }
            Ok(Step::Need(b.escape_with("none", "无牌可弃").build()))
        }
        Some("none") => Ok(Step::Done),
        Some(act) => {
            let card = take_one(g, victim, act)?;
            g.to_discard(&card, "ninge")?;
            g.push_log(format!(
                "狞恶：弃置 {} 的【{}】",
                g.seats[victim].name,
                label(g, &card)
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 双雄（界颜良文丑）
// ---------------------------------------------------------------------------

pub const SHUANGXIONG: SkillDef = SkillDef {
    code: "shuangxiong",
    name: "双雄",
    triggers: &[EventKind::DamageAfter, EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 0,
    can_active: None,
    handler: shuangxiong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn shuangxiong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let ev = a.data["event"].as_str().unwrap_or("").to_string();
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() && ev.starts_with("DamageAfter") {
        if a.data["seat"].as_u64().map(|n| n as usize) != Some(owner) {
            return Ok(Step::Done);
        }
        if let Some(card) = a.data["card"].as_str() {
            g.seats[owner].flags.insert(format!("sx_dmg:{card}"));
        }
        return Ok(Step::Done);
    }
    if stage.is_empty() && ev.starts_with("PhaseStart") {
        if g.phase != Phase::End || !active(g, owner, "shuangxiong") {
            return Ok(Step::Done);
        }
        let keys: Vec<String> = g.seats[owner]
            .flags
            .iter()
            .filter(|k| k.starts_with("sx_dmg:"))
            .cloned()
            .collect();
        let mut got = 0;
        for k in keys {
            g.seats[owner].flags.remove(&k);
            let card = k.trim_start_matches("sx_dmg:").to_string();
            if matches!(g.zone_of(&card), Some(Zone::Discard) | Some(Zone::Processing)) {
                g.gain(&card, owner)?;
                got += 1;
            }
        }
        if got > 0 {
            g.push_log(format!(
                "双雄：{} 获得本回合对其造成过伤害的 {got} 张牌",
                g.seats[owner].name
            ));
        }
        return Ok(Step::Done);
    }
    // offer：摸牌阶段结束弃一张牌（手牌/装备区），记录颜色。
    let opts = hand_and_equip(g, owner);
    if opts.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("shuangxiong");
            for id in &opts {
                b = b.option(format!("pick:{id}"), format!("双雄：弃置【{}】", label(g, id)));
            }
            Ok(Step::Need(b.escape("不弃牌").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("pick:")
                .ok_or_else(|| format!("双雄：未知选项 {act}"))?
                .to_string();
            if !opts.contains(&id) {
                return Err("双雄：该牌不在手牌/装备区".into());
            }
            let red = is_red(g, &id);
            g.to_discard(&id, "shuangxiong")?;
            g.counters
                .insert("turn.sx_color".to_string(), if red { 1 } else { 2 });
            g.push_log(format!(
                "双雄：{} 弃置{}【{}】，本回合异色牌当【决斗】",
                g.seats[owner].name,
                if red { "红色" } else { "黑色" },
                label(g, &id)
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 直谏（界张昭张纮）
// ---------------------------------------------------------------------------

pub const ZHIJIAN: SkillDef = SkillDef {
    code: "zhijian",
    name: "直谏",
    triggers: &[],
    window: Window::OwnPhase(Phase::Play),
    priority: 0,
    can_active: Some(can_zhijian),
    handler: zhijian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn zhijian_equips(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| g.def_of(id).slot.is_some())
        .collect()
}
fn can_zhijian(g: &Game, seat: usize) -> bool {
    active(g, seat, "zhijian")
        && play_phase(g, seat)
        && g.counter("phase.cancel_zhijian") == 0
        && !other_alive(g, seat).is_empty()
        && !zhijian_equips(g, seat).is_empty()
}

fn zhijian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    match stage.as_str() {
        "" => {
            let opts = zhijian_equips(g, owner);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "choice").skill("zhijian");
                    for id in &opts {
                        b = b.option(format!("pick:{id}"), format!("直谏：置入【{}】", label(g, id)));
                    }
                    Ok(Step::Need(b.escape("不发动直谏").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_zhijian");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("pick:")
                        .ok_or_else(|| format!("直谏：未知选项 {act}"))?
                        .to_string();
                    if !opts.contains(&id) {
                        return Err("直谏：该牌不是手牌装备".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "target", "card": id }),
                    ))
                }
            }
        }
        _ => {
            let card = a.data["card"].as_str().unwrap_or_default().to_string();
            let cands = other_alive(g, owner);
            if !g.hand(owner).contains(&card) || cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = builder(owner, "target").skill("zhijian");
                    for t in &cands {
                        b = b.option(format!("target:{t}"), format!("置入 {} 的装备区", g.seats[*t].name));
                    }
                    Ok(Step::Need(b.escape("取消直谏").build()))
                }
                Some("pass") => {
                    g.bump("phase.cancel_zhijian");
                    Ok(Step::Done)
                }
                Some(act) => {
                    let t = act
                        .strip_prefix("target:")
                        .and_then(|x| x.parse::<usize>().ok())
                        .ok_or_else(|| format!("直谏：未知选项 {act}"))?;
                    if !cands.contains(&t) {
                        return Err("直谏：目标不合法".into());
                    }
                    let Some(slot) = g.def_of(&card).slot else {
                        return Err("直谏：不是装备牌".into());
                    };
                    // 替换：旧装备进弃牌堆并触发失去装备钩子。
                    if let Some(old) = g.equip_in(t, slot) {
                        g.to_discard(&old, "zhijian")?;
                        crate::skills::on_equip_lost(g, t);
                    }
                    g.to_equip(&card, t, slot)?;
                    let n = g.draw_with_cause(owner, 1, Some("zhijian")).len();
                    g.push_log(format!(
                        "直谏：{} 将【{}】置入 {} 的{}区并摸 {n} 张",
                        g.seats[owner].name,
                        label(g, &card),
                        g.seats[t].name,
                        slot.label()
                    ));
                    Ok(Step::Done)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 固政（界张昭张纮）
// ---------------------------------------------------------------------------

pub const GUZHENG: SkillDef = SkillDef {
    code: "guzheng",
    name: "固政",
    triggers: &[EventKind::CardDiscarded],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: guzheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

fn guzheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        if !a.data["event"].as_str().unwrap_or("").starts_with("CardDiscarded") {
            return Ok(Step::Done);
        }
        let Some(discarder) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        if discarder == owner || !active(g, owner, "guzheng") {
            return Ok(Step::Done);
        }
        // 单次弃置动作达到 2 张（用户裁定 Q2=B）。
        let batch: Vec<CardId> = g
            .discard_batch
            .get(&discarder)
            .cloned()
            .unwrap_or_default();
        if batch.len() != 2 || g.counter(&format!("phase.guzheng_used:{owner}")) > 0 {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "pick", "discarder": discarder, "cards": batch }),
        ));
    }
    // pick：由固政持有者选择交给弃牌者的那一张。
    let discarder = a.data["discarder"].as_u64().unwrap_or(0) as usize;
    let cards: Vec<String> = a.data["cards"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let avail: Vec<String> = cards
        .into_iter()
        .filter(|id| matches!(g.zone_of(id), Some(Zone::Discard)))
        .collect();
    if avail.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = builder(owner, "choice").skill("guzheng");
            for id in &avail {
                b = b.option(
                    format!("give:{id}"),
                    format!("交给 {}：{}", g.seats[discarder].name, label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不发动固政").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let given = act
                .strip_prefix("give:")
                .ok_or_else(|| format!("固政：未知选项 {act}"))?
                .to_string();
            if !avail.contains(&given) {
                return Err("固政：该牌不在可分配弃牌中".into());
            }
            g.bump(&format!("phase.guzheng_used:{owner}"));
            g.gain(&given, discarder)?;
            let mut got = 0;
            for id in avail.iter().filter(|c| *c != &given) {
                g.gain(id, owner)?;
                got += 1;
            }
            g.push_log(format!(
                "固政：{} 获得 1 张弃牌，{} 获得其余 {got} 张",
                g.seats[discarder].name, g.seats[owner].name
            ));
            Ok(Step::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// 破军（界徐盛）
// ---------------------------------------------------------------------------

pub const POJUN: SkillDef = SkillDef {
    code: "pojun",
    name: "破军",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: pojun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
};

/// 【杀】指定目标后：破军介入（移出至多 X 张牌直到回合结束）。
pub fn sha_intercept(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&CardId>,
) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let Some(card) = card else { return out };
    if !is_sha(g, card) || target == user || g.seats[target].hp <= 0 {
        return out;
    }
    if !active(g, user, "pojun") {
        return out;
    }
    if g.hand(target).is_empty() && equip_ids(g, target).is_empty() {
        return out;
    }
    out.push((user, "pojun"));
    out
}

fn pojun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.actor;
    let stage = a.data["stage"].as_str().unwrap_or("").to_string();
    if stage.is_empty() {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("pojun");
                b = b.option(
                    "opt1",
                    format!("破军：移出 {} 至多 {} 张牌", g.seats[target].name, g.seats[target].hp.max(0)),
                );
                Ok(Step::Need(b.escape("不发动破军").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let x = g.seats[target].hp.max(0) as usize;
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "pick", "target": target, "remaining": x }),
                ))
            }
        }
    } else {
        let target = a.data["target"].as_u64().unwrap_or(0) as usize;
        let remaining = a.data["remaining"].as_u64().unwrap_or(0) as usize;
        if remaining == 0 || g.seats[target].hp <= 0 {
            return Ok(Step::Done);
        }
        let opts = take_one_options(g, target);
        if opts.is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => {
                let mut b = builder(owner, "choice").skill("pojun");
                for (id, text) in &opts {
                    b = b.option(id.clone(), format!("破军：移出{text}"));
                }
                Ok(Step::Need(b.escape("停止移出").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let card = take_one(g, target, act)?;
                g.to_void(&card)?;
                g.void_return.push((target, card.clone(), g.turn_seq));
                g.push_log(format!(
                    "破军：将 {} 的【{}】移出游戏直到回合结束",
                    g.seats[target].name,
                    label(g, &card)
                ));
                if remaining > 1 {
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "pick", "target": target, "remaining": remaining - 1 }),
                    ))
                } else {
                    Ok(Step::Done)
                }
            }
        }
    }
}

/// 破军：手牌数与装备区牌数皆不多于你 → 【杀】伤害 +1。
pub fn pojun_bonus(g: &Game, source: usize, victim: usize, card: Option<&CardId>) -> i32 {
    let Some(card) = card else { return 0 };
    if !is_sha(g, card) || !active(g, source, "pojun") {
        return 0;
    }
    let src_equip = equip_ids(g, source).len();
    let dst_equip = equip_ids(g, victim).len();
    i32::from(g.hand(victim).len() <= g.hand(source).len() && dst_equip <= src_equip)
}

// ---------------------------------------------------------------------------
// 转化（断粮 / 双雄）
// ---------------------------------------------------------------------------

pub fn conversions(g: &Game, seat: usize) -> Vec<super::Conversion> {
    let mut out = Vec::new();
    if active(g, seat, "duanliang") {
        for id in hand_and_equip(g, seat) {
            if is_red(g, &id) {
                continue;
            }
            let def = g.def_of(&id);
            if def.kind == CardKind::Basic || def.slot.is_some() {
                out.push(super::Conversion {
                    skill: "duanliang",
                    from: id,
                    to: "兵粮寸断",
                    usable: true,
                    playable: false,
                });
            }
        }
    }
    if active(g, seat, "shuangxiong") {
        let color = g.counter("turn.sx_color");
        if color > 0 {
            for id in hand_and_equip(g, seat) {
                let red = is_red(g, &id);
                if (color == 1) != red {
                    out.push(super::Conversion {
                        skill: "shuangxiong",
                        from: id,
                        to: "决斗",
                        usable: true,
                        playable: false,
                    });
                }
            }
        }
    }
    out
}

pub const B16_SKILLS: &[SkillDef] = &[
    HUOSHOU,
    ZAIZHI,
    DUANLIANG,
    JIEZI,
    DUANCHANG,
    BEIGE,
    QUHU,
    JIEMING,
    QIANGXI,
    NINGE,
    SHUANGXIONG,
    ZHIJIAN,
    GUZHENG,
    POJUN,
];
