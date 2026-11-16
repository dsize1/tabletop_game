//! B16 技能（界限突破·J4：8 将 / 14 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B16.json` 的 `agreed`（权威）。
//! 本文件按批次推进；`SKILLS` 只声明**已实现并收口**的技能（`check:sgs3-registry` 报 ⏳）。

use serde_json::json;

use super::super::cards::{self, Category};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{Game, Phase, Suit, Zone};
use super::common::{self, card_label, phase_key, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    SkillDef { code: "huoshou", name: "祸首", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "zaizhi", name: "再起", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "duanliang", name: "断粮", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "jiezi", name: "截辎", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    SkillDef { code: "duanchang", name: "断肠", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::Death, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "quhu", name: "驱虎", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "jieming", name: "节命", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ },
            Subscription { event: EventKind::Death, subject: Subject::Self_ },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "qiangxi", name: "强袭", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "ninge", name: "狞恶", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "zhijian", name: "直谏", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "guzheng", name: "固政", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::CardDiscarded, subject: Subject::Other }], hooks: &[], can_active: false }] },
    SkillDef { code: "shuangxiong", name: "双雄", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[
            Face { window: Window::OwnPhase(Phase::Draw), subscriptions: &[], hooks: &[], can_active: false },
            Face { window: Window::OwnPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false },
        ] },
    SkillDef { code: "pojun", name: "破军", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaTargeted], can_active: false }] },
    SkillDef { code: "beige", name: "悲歌", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Any }], hooks: &[], can_active: false }] },
];

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    match code {
        "quhu" => Some(can_quhu),
        "qiangxi" => Some(can_qiangxi),
        "zhijian" => Some(can_zhijian),
        _ => None,
    }
}

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "zaizhi" => zaizhi,
        "jiezi" => jiezi,
        "duanchang" => duanchang,
        "quhu" => quhu,
        "jieming" => jieming,
        "qiangxi" => qiangxi,
        "ninge" => ninge,
        "zhijian" => zhijian,
        "guzheng" => guzheng,
        "shuangxiong" => shuangxiong,
        "pojun" => pojun,
        "beige" => beige,
        _ => return None,
    })
}

fn has(g: &Game, seat: usize, code: &str) -> bool {
    skills::active(g, seat, code) && skills::enabled(code)
}

fn def_of(g: &Game, id: &str) -> Option<&'static cards::CardDef> {
    g.card_defs.get(id).and_then(|c| cards::def(&c.def))
}

fn is_black(g: &Game, id: &str) -> bool {
    matches!(
        g.card_defs.get(id).map(|c| c.suit),
        Some(Suit::Spade) | Some(Suit::Club)
    )
}

// ---------------------------------------------------------------------------
// 祸首（界孟获）：南蛮不能指定你；其他角色使用的南蛮伤害来源改为祸首持有者
// ---------------------------------------------------------------------------

/// 祸首：AOE 目标排除（不能成为【南蛮入侵】目标）。
pub fn aoe_excluded(g: &Game, _user: usize, target: usize, def_code: &str) -> bool {
    def_code == "nanman_ruqin" && has(g, target, "huoshou")
}

/// 祸首：其他角色使用【南蛮入侵】时，伤害来源改为座位序最先的祸首持有者。
/// 自己使用的【南蛮入侵】不替换（用户 Q3=B）。
pub fn aoe_damage_source(g: &Game, user: usize, card: &str) -> Option<usize> {
    let is_nanman = def_of(g, card)
        .map(|d| d.code == "nanman_ruqin")
        .or_else(|| cards::def(card).map(|d| d.code == "nanman_ruqin"))
        .unwrap_or(false);
    if !is_nanman {
        return None;
    }
    if has(g, user, "huoshou") {
        return None;
    }
    (0..g.seats.len()).find(|i| {
        *i != user && g.seats[*i].alive && has(g, *i, "huoshou")
    })
}

// ---------------------------------------------------------------------------
// 断粮（界徐晃）：黑色基本牌/装备牌当【兵粮寸断】；本回合未造成伤害则无距离
// ---------------------------------------------------------------------------

pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    // 双雄：本回合弃置牌后，异色牌当【决斗】使用（颜色按物理花色）。
    if has(g, seat, "shuangxiong") {
        if let Some(color) = g.mark(&format!("turn.{}.{}.sx_color", g.turn, seat)) {
            let own = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == seat)
                || matches!(g.cards.get(id), Some(Zone::Equip(s, _)) if *s == seat);
            if own {
                let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
                let matches = (color == "red" && red) || (color == "black" && !red);
                if matches {
                    return Some("juedou");
                }
            }
        }
    }
    if !has(g, seat, "duanliang") {
        return None;
    }
    let zone = g.cards.get(id)?;
    let own = matches!(zone, Zone::Hand(s) if *s == seat)
        || matches!(zone, Zone::Equip(s, _) if *s == seat);
    if !own || !is_black(g, id) {
        return None;
    }
    let d = def_of(g, id)?;
    if d.category == Category::Basic || d.slot.is_some() {
        return Some("bingliang_cunduan");
    }
    None
}

/// 断粮：【兵粮寸断】无距离限制（本回合未造成伤害）。
pub fn trick_no_distance(g: &Game, seat: usize, card: &str) -> bool {
    if !has(g, seat, "duanliang") {
        return false;
    }
    let is_bingliang = def_of(g, card)
        .map(|d| d.code == "bingliang_cunduan")
        .or_else(|| cards::def(card).map(|d| d.code == "bingliang_cunduan"))
        .unwrap_or(false);
    if !is_bingliang {
        return false;
    }
    g.counter(&turn_key(seat, "damage_dealt")) == 0
}

// ---------------------------------------------------------------------------
// 再起（界孟获）：结束阶段，本回合每有 1 张红色牌进弃牌堆，逐次令一名角色二选一
// ---------------------------------------------------------------------------

fn zaizhi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if g.phase != Phase::End || !has(g, owner, "zaizhi") {
        return Ok(Step::Done);
    }
    let stage = ask
        .data
        .get("stage")
        .or_else(|| ask.data.get("step"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let remaining = ask.data.get("remaining").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    if stage.is_empty() {
        let n = g.counter(&format!("turn.{}.red_to_discard", g.turn)).max(0) as usize;
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
        let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].alive).collect();
        if let Some(ans) = take_answer(g, "zaizhi") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|x| x.parse::<usize>().ok()) else {
                return Err("再起：未知选项".into());
            };
            if !cands.contains(&t) {
                return Err("再起：目标不合法".into());
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "choice", "remaining": remaining, "target": t }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "zaizhi_target").skill("zaizhi");
        for t in &cands {
            b = b.option(
                format!("target:{t}"),
                format!("再起：选择 {}（已损失 {} 点体力）", seat_name(g, *t), g.seats[*t].max_hp - g.seats[*t].hp),
            );
        }
        return Ok(Step::NeedAt(
            Stage::Choose,
            b.escape("结束再起").build(),
        ));
    }

    // choice：由被选中的角色选择 摸 1 / 令孟获回复 1（强制二选一）。
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if let Some(ans) = take_answer(g, "zaizhi") {
        match ans.as_str() {
            "opt1" => {
                flow::draw(g, target, 1);
            }
            "opt2" => {
                flow::heal(g, owner, 1, "再起");
            }
            _ => return Err("再起：未知选项".into()),
        }
        return next_round(g, owner, remaining);
    }
    let mut b = PendingBuilder::new(target, "zaizhi_choice")
        .skill("zaizhi")
        .option("opt1", "再起：摸一张牌")
        .option("opt2", format!("再起：令 {} 回复 1 点体力", seat_name(g, owner)));
    Ok(Step::NeedAt(
        Stage::Choose,
        b.escape_mandatory("（不能放弃）").build(),
    ))
}

fn next_round(g: &mut Game, _owner: usize, remaining: usize) -> Result<Step, String> {
    if remaining > 1 {
        Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "target", "remaining": remaining - 1 }),
        ))
    } else {
        let _ = g;
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 截辎（界徐晃）：跳过摸牌阶段后；有「辎」者摸牌阶段结束执行额外摸牌阶段
// ---------------------------------------------------------------------------

fn zai(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get("zai").copied())
        .unwrap_or(0)
        .max(0) as u32
}

fn add_zai(g: &mut Game, seat: usize) {
    if let Some(s) = g.seats.get_mut(seat) {
        *s.counters.entry("zai".to_string()).or_insert(0) += 1;
    }
}

fn remove_zai(g: &mut Game, seat: usize) {
    if let Some(s) = g.seats.get_mut(seat) {
        let n = s.counters.get("zai").copied().unwrap_or(0);
        if n <= 1 {
            s.counters.remove("zai");
        } else {
            s.counters.insert("zai".to_string(), n - 1);
        }
    }
}

/// 任意角色跳过摸牌阶段后（`turn` 跳过分支）：座位序最先的截辎持有者询问。
pub fn on_skip_draw(g: &mut Game, skipper: usize) {
    let Some(owner) = (0..g.seats.len()).find(|i| g.seats[*i].alive && has(g, *i, "jiezi")) else {
        return;
    };
    super::super::resolve::push_ask(
        g,
        "jiezi",
        Stage::Choose,
        owner,
        json!({ "stage": "ask", "skipper": skipper }),
    );
}

/// 摸牌阶段结束：有「辎」→ 移除并重跑一次 Draw 阶段（K15 单出口）。
pub fn take_extra_draw(g: &mut Game, seat: usize) -> bool {
    if zai(g, seat) == 0 {
        return false;
    }
    remove_zai(g, seat);
    true
}

fn jiezi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jiezi") {
        return Ok(Step::Done);
    }
    let skipper = ask.data.get("skipper").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if let Some(ans) = take_answer(g, "jiezi") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(t) = ans.strip_prefix("target:").and_then(|x| x.parse::<usize>().ok()) else {
            return Err("截辎：未知选项".into());
        };
        if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
            return Err("截辎：目标不合法".into());
        }
        let min = (0..g.seats.len())
            .filter(|i| g.seats[*i].alive)
            .map(|i| g.hand_size(i))
            .min()
            .unwrap_or(0);
        if g.hand_size(t) == min && zai(g, t) == 0 {
            add_zai(g, t);
        } else {
            flow::draw(g, t, 1);
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "jiezi_target").skill("jiezi");
    for t in 0..g.seats.len() {
        if !g.seats[t].alive {
            continue;
        }
        b = b.option(
            format!("target:{t}"),
            format!(
                "截辎：选择 {}（手牌 {}，{}）",
                seat_name(g, t),
                g.hand_size(t),
                if zai(g, t) > 0 { "已有辎" } else { "无辎" }
            ),
        );
    }
    let _ = skipper;
    Ok(Step::NeedAt(
        Stage::Choose,
        b.escape("不发动截辎").build(),
    ))
}

// ---------------------------------------------------------------------------
// 断肠（界蔡文姬）：死亡时杀死者失去所有技能（先于死亡弃牌）
// ---------------------------------------------------------------------------

fn duanchang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "duanchang") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("Death") {
        return Ok(Step::Done);
    }
    let Some(killer) = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize) else {
        return Ok(Step::Done);
    };
    if killer == owner {
        return Ok(Step::Done);
    }
    // 断肠：杀死者失去所有技能（含锁定技/主公技；本局永久）。
    skills::revoke_all(g, killer);
    Ok(Step::Finish("断肠：杀死者失去所有技能".into()))
}

// ---------------------------------------------------------------------------
// 节命（界荀彧）：受伤后 / 死亡时，令一名角色摸 X 并弃至 X（X=min(5,体力上限)）
// ---------------------------------------------------------------------------

fn jieming(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "jieming") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    // 受伤后：已死亡时交给 Death 分支，避免双触发。
    if event == "DamageAfter" && !g.seats.get(owner).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    if stage.is_empty() {
        if event != "DamageAfter" && event != "Death" {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "ask", "event": event }),
        ));
    }
    if stage == "ask" {
        if let Some(ans) = take_answer(g, "jieming") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(t) = ans.strip_prefix("target:").and_then(|x| x.parse::<usize>().ok()) else {
                return Err("节命：未知选项".into());
            };
            if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                return Err("节命：目标不合法".into());
            }
            let x = (g.seats[t].max_hp.max(0) as usize).min(5);
            flow::draw(g, t, x);
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "discard", "target": t, "x": x }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "jieming_target").skill("jieming");
        for t in 0..g.seats.len() {
            if !g.seats[t].alive {
                continue;
            }
            let x = (g.seats[t].max_hp.max(0) as usize).min(5);
            b = b.option(
                format!("target:{t}"),
                format!("节命：令 {} 摸 {x} 张并弃至 {x} 张", seat_name(g, t)),
            );
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "ask" }),
            b.escape("不发动节命").build(),
        ));
    }
    // discard：目标把超出 X 的手牌弃掉（强制选择，逐张）。
    let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if g.hand_size(t) <= x {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "jieming") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("节命：未知选项".into());
        };
        if !g.hand(t).iter().any(|c| c == id) {
            return Err("节命：该牌不在手牌".into());
        }
        flow::discard(g, t, id, "节命");
        if g.hand_size(t) <= x {
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "discard", "target": t, "x": x }),
        ));
    }
    let mut b = PendingBuilder::new(t, "jieming_discard").skill("jieming");
    for id in g.hand(t) {
        b = b.option(format!("pick:{id}"), format!("弃置【{}】", card_label(g, &id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "discard", "target": t, "x": x }),
        b.escape_mandatory("（必须弃至 X 张）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 驱虎（界荀彧）：拼点；赢 → 令目标对其攻击范围内角色造成 1 点伤害；没赢 → 目标对荀彧造成 1 点
// ---------------------------------------------------------------------------

fn can_quhu(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "quhu") {
        return false;
    }
    if g.phase != Phase::Play || g.turn != seat {
        return false;
    }
    if g.counter(&turn_key(seat, "quhu_used")) > 0 {
        return false;
    }
    let hp = g.seats.get(seat).map(|s| s.hp).unwrap_or(0);
    g.seats
        .iter()
        .any(|s| s.id != seat && s.alive && s.hp > hp && !g.hand(s.id).is_empty())
}

fn quhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "quhu") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if !can_quhu(g, owner) {
                return Ok(Step::Done);
            }
            let hp = g.seats[owner].hp;
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|t| *t != owner && g.seats[*t].hp > hp && !g.hand(*t).is_empty())
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "quhu") {
                if ans == "pass" {
                    return Ok(Step::Done);
                }
                let Some(t) = ans.strip_prefix("target:").and_then(|x| x.parse::<usize>().ok()) else {
                    return Err("驱虎：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("驱虎：目标不合法".into());
                }
                if !super::super::pindian::can_start(g, owner, t) {
                    return Err("驱虎：双方需有手牌".into());
                }
                g.bump(&turn_key(owner, "quhu_used"));
                super::super::pindian::start(g, owner, t, "驱虎");
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": t }),
                ));
            }
            let mut b = PendingBuilder::new(owner, "quhu_target").skill("quhu");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("驱虎：与 {} 拼点（体力 {}）", seat_name(g, *t), g.seats[*t].hp),
                );
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动驱虎").build(),
            ));
        }
        "wait" => {
            let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let Some(r) = g.take_pindian() else {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "stage": "wait", "target": t }),
                ));
            };
            if !r.initiator_won() {
                // 没赢：目标对荀彧造成 1 点伤害（不被防具减免）。
                let spec = super::super::DamageSpec {
                    from: Some(t),
                    target: owner,
                    amount: 1,
                    kind: super::super::DamageKind::Normal,
                    source: super::super::SourceKind::Skill,
                };
                g.damage_card = None;
                flow::damage(g, spec, super::super::ArmorKind::None, 0);
                return Ok(Step::Done);
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "damage", "target": t }),
            ));
        }
        "damage" => {
            let t = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            // 由荀彧选择：目标攻击范围内一名角色（可含荀彧）承受 1 点伤害。
            let reach = super::super::gear::attack_range(g, t) as usize;
            let cands: Vec<usize> = g
                .alive_seats()
                .into_iter()
                .filter(|x| super::super::gear::within_attack_range(g, t, *x, reach))
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "quhu") {
                let Some(x) = ans.strip_prefix("damage:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("驱虎：未知选项".into());
                };
                if !cands.contains(&x) {
                    return Err("驱虎：伤害目标不合法".into());
                }
                let spec = super::super::DamageSpec {
                    from: Some(t),
                    target: x,
                    amount: 1,
                    kind: super::super::DamageKind::Normal,
                    source: super::super::SourceKind::Skill,
                };
                g.damage_card = None;
                flow::damage(g, spec, super::super::ArmorKind::None, 0);
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(owner, "quhu_damage").skill("quhu");
            for x in &cands {
                b = b.option(
                    format!("damage:{x}"),
                    format!("驱虎：令 {} 对 {} 造成 1 点伤害", seat_name(g, t), seat_name(g, *x)),
                );
            }
            Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "damage", "target": t }),
                b.escape("不选择（不造成伤害）").build(),
            ))
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 强袭（界典韦）：出牌阶段限两次；失 1 体力或弃武器 → 对本回合未指定过的角色造成 1 点伤害
// ---------------------------------------------------------------------------

fn can_qiangxi(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "qiangxi") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    if g.counter(&phase_key(seat, "qiangxi_used")) >= 2 {
        return false;
    }
    let has_weapon = super::super::gear::equip_in(g, seat, super::super::Slot::Weapon).is_some();
    if !has_weapon && g.seats[seat].hp <= 0 {
        return false;
    }
    // 至少有一个本回合未以此法指定过的其他角色。
    (0..g.seats.len()).any(|t| {
        t != seat
            && g.seats[t].alive
            && g.counter(&turn_key(seat, &format!("qiangxi_t.{t}"))) == 0
    })
}

fn qiangxi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "qiangxi") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    match stage {
        "" => {
            if !can_qiangxi(g, owner) {
                return Ok(Step::Done);
            }
            let weapon = super::super::gear::equip_in(g, owner, super::super::Slot::Weapon).cloned();
            if let Some(ans) = take_answer(g, "qiangxi") {
                match ans.as_str() {
                    "pass" => return Ok(Step::Done),
                    "hp" => {
                        flow::lose_hp(g, owner, 1, "强袭");
                    }
                    other if other.starts_with("weapon:") => {
                        let id = &other["weapon:".len()..];
                        if weapon.as_deref() != Some(id) {
                            return Err("强袭：该武器不在装备区".into());
                        }
                        flow::discard(g, owner, id, "强袭");
                    }
                    _ => return Err("强袭：未知选项".into()),
                }
                return Ok(Step::Next(Stage::Choose, json!({ "stage": "target" })));
            }
            let mut b = PendingBuilder::new(owner, "qiangxi_cost").skill("qiangxi");
            b = b.option("hp", "强袭：受到 1 点伤害（无来源）");
            if let Some(id) = &weapon {
                b = b.option(format!("weapon:{id}"), format!("强袭：弃置武器【{}】", card_label(g, id)));
            }
            return Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "" }),
                b.escape("不发动强袭").build(),
            ));
        }
        "target" => {
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|t| {
                    *t != owner
                        && g.seats[*t].alive
                        && g.counter(&turn_key(owner, &format!("qiangxi_t.{t}"))) == 0
                })
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            if let Some(ans) = take_answer(g, "qiangxi") {
                let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
                    return Err("强袭：未知选项".into());
                };
                if !cands.contains(&t) {
                    return Err("强袭：目标不合法".into());
                }
                g.bump(&phase_key(owner, "qiangxi_used"));
                g.bump(&turn_key(owner, &format!("qiangxi_t.{t}")));
                let spec = super::super::DamageSpec {
                    from: Some(owner),
                    target: t,
                    amount: 1,
                    kind: super::super::DamageKind::Normal,
                    source: super::super::SourceKind::Skill,
                };
                g.damage_card = None;
                flow::damage(g, spec, super::super::ArmorKind::None, 0);
                return Ok(Step::Done);
            }
            let mut b = PendingBuilder::new(owner, "qiangxi_target").skill("qiangxi");
            for t in &cands {
                b = b.option(
                    format!("target:{t}"),
                    format!("强袭：对 {} 造成 1 点伤害（无距离）", seat_name(g, *t)),
                );
            }
            Ok(Step::NeedWith(
                Stage::Choose,
                json!({ "stage": "target" }),
                b.escape("不选择（本技能不结算）").build(),
            ))
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 狞恶（界典韦）：一名角色每回合第二次受到伤害后，若其/来源为典韦 → 摸 1 并弃其一张牌
// ---------------------------------------------------------------------------

fn ninge(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "ninge") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("DamageAfter") {
        return Ok(Step::Done);
    }
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(owner);
    let by = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if victim != owner && by != Some(owner) {
            return Ok(Step::Done);
        }
        let key = turn_key(victim, "ninge_hits");
        g.bump(&key);
        if g.counter(&key) != 2 {
            return Ok(Step::Done);
        }
        flow::draw(g, owner, 1);
        let equips = super::super::gear::equips_of(g, victim);
        let has_hand = !g.hand(victim).is_empty();
        if equips.is_empty() && !has_hand {
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "ninge_pick").skill("ninge");
        if has_hand {
            b = b.option("hand", "狞恶：弃置其一张随机手牌");
        }
        for id in &equips {
            b = b.option(format!("equip:{id}"), format!("狞恶：弃置其装备【{}】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "pick", "victim": victim, "event": "DamageAfter" }),
            b.escape("不弃置").build(),
        ));
    }
    // pick：典韦选择弃置受害者一张牌（装备区具体牌 / 手牌随机；不含判定区）。
    let victim = ask.data.get("victim").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let equips = super::super::gear::equips_of(g, victim);
    let has_hand = !g.hand(victim).is_empty();
    if equips.is_empty() && !has_hand {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "ninge") {
        if ans == "pass" {
            return Ok(Step::Done);
        }
        if ans == "hand" {
            if !has_hand {
                return Err("狞恶：其没有手牌".into());
            }
            let idx = g.rand_below(g.hand(victim).len());
            if let Some(id) = g.hand(victim).get(idx).cloned() {
                flow::discard(g, owner, &id, "狞恶");
            }
            return Ok(Step::Done);
        }
        let Some(id) = ans.strip_prefix("equip:") else {
            return Err("狞恶：未知选项".into());
        };
        if !equips.iter().any(|e| e == id) {
            return Err("狞恶：该装备不在其装备区".into());
        }
        flow::discard(g, owner, id, "狞恶");
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "ninge_pick").skill("ninge");
    if has_hand {
        b = b.option("hand", "狞恶：弃置其一张随机手牌");
    }
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("狞恶：弃置其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "pick", "victim": victim, "event": "DamageAfter" }),
        b.escape("不弃置").build(),
    ))
}

// ---------------------------------------------------------------------------
// 直谏（界张昭张纮）：出牌阶段，将一张手牌装备置入其他角色装备区，然后摸一张
// ---------------------------------------------------------------------------

fn can_zhijian(g: &Game, seat: usize) -> bool {
    if !has(g, seat, "zhijian") || g.turn != seat || g.phase != Phase::Play {
        return false;
    }
    let has_equip = g.hand(seat).iter().any(|id| {
        def_of(g, id).map(|d| d.slot.is_some()).unwrap_or(false)
    });
    has_equip && (0..g.seats.len()).any(|t| t != seat && g.seats[t].alive)
}

fn zhijian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "zhijian") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        let cands: Vec<String> = g
            .hand(owner)
            .into_iter()
            .filter(|id| def_of(g, id).map(|d| d.slot.is_some()).unwrap_or(false))
            .collect();
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "zhijian") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("直谏：未知选项".into());
            };
            if !cands.iter().any(|c| c == id) {
                return Err("直谏：该牌不是手牌装备".into());
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "stage": "target", "card": id }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "zhijian_pick").skill("zhijian");
        for id in &cands {
            b = b.option(format!("pick:{id}"), format!("直谏：置入【{}】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "" }),
            b.escape("不发动直谏").build(),
        ));
    }
    let card = ask.data.get("card").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let cands: Vec<usize> = (0..g.seats.len())
        .filter(|t| *t != owner && g.seats[*t].alive)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "zhijian") {
        let Some(t) = ans.strip_prefix("target:").and_then(|v| v.parse::<usize>().ok()) else {
            return Err("直谏：未知选项".into());
        };
        if !cands.contains(&t) {
            return Err("直谏：目标不合法".into());
        }
        super::super::gear::place_from_hand(g, owner, &card, t)?;
        flow::draw(g, owner, 1);
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "zhijian_target").skill("zhijian");
    for t in &cands {
        b = b.option(format!("target:{t}"), format!("直谏：置入 {} 的装备区", seat_name(g, *t)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "target", "card": card }),
        b.escape("不选择（该牌不置入）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 固政（界张昭张纮）：其他角色单次弃置 ≥2 张后，令其获得一张，你获得其余
// ---------------------------------------------------------------------------

fn guzheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "guzheng") {
        return Ok(Step::Done);
    }
    if ask.data.get("event").and_then(|v| v.as_str()) != Some("CardDiscarded") {
        return Ok(Step::Done);
    }
    let discarder = ask
        .data
        .get("discarder")
        .or_else(|| ask.data.get("victim"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(owner);
    if discarder == owner {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    if stage.is_empty() {
        if g.counter(&phase_key(owner, "guzheng_offered")) > 0 {
            return Ok(Step::Done);
        }
        let batch: Vec<String> = skills::discard_batch(g, discarder)
            .iter()
            .filter(|id| matches!(g.cards.get(*id), Some(Zone::Discard)))
            .cloned()
            .collect();
        if batch.len() < 2 {
            return Ok(Step::Done);
        }
        g.bump(&phase_key(owner, "guzheng_offered"));
        let mut b = PendingBuilder::new(owner, "guzheng_give").skill("guzheng");
        for id in &batch {
            b = b.option(
                format!("give:{id}"),
                format!("固政：令 {} 获得【{}】，你获得其余", seat_name(g, discarder), card_label(g, id)),
            );
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "give", "discarder": discarder, "cards": batch, "event": "CardDiscarded" }),
            b.escape("不发动固政").build(),
        ));
    }
    let discarder = ask.data.get("discarder").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let cards: Vec<String> = ask
        .data
        .get("cards")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "guzheng") {
            if ans == "pass" {
            return Ok(Step::Done);
        }
        let Some(give) = ans.strip_prefix("give:") else {
            return Err("固政：未知选项".into());
        };
        if !cards.iter().any(|c| c == give) {
            return Err("固政：该牌不在本批弃置牌中".into());
        }
        if matches!(g.cards.get(give), Some(Zone::Discard)) {
            flow::gain(g, discarder, give, owner, "固政");
        }
        for id in &cards {
            if id != give && matches!(g.cards.get(id), Some(Zone::Discard)) {
                flow::gain(g, owner, id, owner, "固政");
            }
        }
        return Ok(Step::Done);
    }
    let mut b = PendingBuilder::new(owner, "guzheng_give").skill("guzheng");
    for id in &cards {
        b = b.option(
            format!("give:{id}"),
            format!("固政：令 {} 获得【{}】，你获得其余", seat_name(g, discarder), card_label(g, id)),
        );
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "give", "discarder": discarder, "cards": cards, "event": "CardDiscarded" }),
        b.escape("不发动固政").build(),
    ))
}

// ---------------------------------------------------------------------------
// 双雄（界颜良文丑）：摸牌阶段结束弃一张牌 → 本回合异色牌当【决斗】；
// 结束阶段取回本回合对你造成过伤害的牌。
// ---------------------------------------------------------------------------

fn shuangxiong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "shuangxiong") {
        return Ok(Step::Done);
    }
    let stage = ask.data.get("stage").and_then(|v| v.as_str()).unwrap_or("");
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    // 摸牌阶段结束：弃一张牌，记录相反颜色。
    if stage == "offer" {
        let cands: Vec<String> = common::own_cards(g, owner);
        if cands.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "shuangxiong") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("双雄：未知选项".into());
            };
            if !cands.iter().any(|c| c == id) {
                return Err("双雄：该牌不在手牌/装备区".into());
            }
            let red = g.card_defs.get(id).map(|c| c.suit.is_red()).unwrap_or(false);
            flow::discard(g, owner, id, "双雄");
            g.set_mark(
                &format!("turn.{}.{}.sx_color", g.turn, owner),
                if red { "black" } else { "red" },
            );
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "shuangxiong_pick").skill("shuangxiong");
        for id in &cands {
            b = b.option(format!("pick:{id}"), format!("双雄：弃置【{}】", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "offer" }),
            b.escape("不发动双雄").build(),
        ));
    }
    // 结束阶段：取回本回合对你造成过伤害的牌（可选）。
    if stage == "recover" || event == "PhaseStart" {
        let prefix = format!("turn.{}.damage_card_taken.", owner);
        let cards: Vec<String> = g
            .counters
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .filter_map(|k| k.rsplit('.').next().map(|s| s.to_string()))
            .filter(|id| matches!(g.cards.get(id), Some(Zone::Discard) | Some(Zone::Processing) | Some(Zone::Void)))
            .collect();
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "shuangxiong") {
            if ans == "gain" {
                for id in &cards {
                    flow::gain(g, owner, id, owner, "双雄");
                }
            }
            for k in g.counters.keys().filter(|k| k.starts_with(&prefix)).cloned().collect::<Vec<_>>() {
                g.set_counter(&k, 0);
            }
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "shuangxiong_recover").skill("shuangxiong");
        b = b.option(
            "gain",
            format!("双雄：获得本回合对你造成过伤害的 {} 张牌", cards.len()),
        );
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "recover" }),
            b.escape("不获得").build(),
        ));
    }
    Ok(Step::Done)
}

/// 摸牌阶段结束（`turn::finish_phase`）：每回合一次询问是否弃牌发动双雄。
pub fn after_draw_phase(g: &mut Game, seat: usize) {
    if !has(g, seat, "shuangxiong") || g.counter(&turn_key(seat, "sx_offered")) > 0 {
        return;
    }
    if common::own_cards(g, seat).is_empty() {
        return;
    }
    g.bump(&turn_key(seat, "sx_offered"));
    super::super::resolve::push_ask(
        g,
        "shuangxiong",
        Stage::Choose,
        seat,
        json!({ "stage": "offer" }),
    );
}

// ---------------------------------------------------------------------------
// 破军（界徐盛）：杀指定目标后移出至多 X 张牌至回合末；牌数不多于你时杀伤害 +1
// ---------------------------------------------------------------------------

/// `Hook::ShaTargeted`：为每个目标压入破军询问。
pub fn fire_sha_targeted(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    if !has(g, user, "pojun") || targets.is_empty() {
        return false;
    }
    super::super::resolve::push_ask(
        g,
        "pojun",
        Stage::Choose,
        user,
        json!({ "step": "targets", "targets": targets, "idx": 0, "card": card }),
    );
    true
}

/// 破军：手牌+装备数不多于你 → 此【杀】伤害 +1。
pub fn damage_bonus(g: &Game, spec: &super::super::DamageSpec) -> i32 {
    let Some(from) = spec.from else { return 0 };
    if spec.source != super::super::SourceKind::Slash || !has(g, from, "pojun") {
        return 0;
    }
    let my_cards = g.hand_size(from) + super::super::gear::equips_of(g, from).len();
    let their_cards =
        g.hand_size(spec.target) + super::super::gear::equips_of(g, spec.target).len();
    if their_cards <= my_cards {
        1
    } else {
        0
    }
}

fn pojun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "pojun") {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("targets");
    let targets: Vec<usize> = ask
        .data
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect())
        .unwrap_or_default();
    let idx = ask.data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if idx >= targets.len() {
        return Ok(Step::Done);
    }
    let target = targets[idx];
    if !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
        ));
    }
    if step == "targets" {
        let x = g.seats[target].hp.max(0) as usize;
        if x == 0 {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
            ));
        }
        if let Some(ans) = take_answer(g, "pojun") {
            if ans == "pass" {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
                ));
            }
            if ans != "use" {
                return Err("破军：未知选项".into());
            }
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "exile", "targets": targets, "idx": idx, "target": target, "x": x, "picked": 0, "card": ask.data.get("card") }),
            ));
        }
        let mut b = PendingBuilder::new(owner, "pojun_ask").skill("pojun");
        b = b.option(
            "use",
            format!("破军：移出 {} 至多 {} 张牌直到回合结束", seat_name(g, target), x),
        );
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "step": "targets", "targets": targets, "idx": idx, "card": ask.data.get("card") }),
            b.escape("不发动破军").build(),
        ));
    }
    // exile：逐张选择（手牌随机 / 装备区具体牌），至多 x 张。
    let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let picked = ask.data.get("picked").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if picked >= x {
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
        ));
    }
    let equips = super::super::gear::equips_of(g, target);
    let has_hand = !g.hand(target).is_empty();
    if equips.is_empty() && !has_hand {
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
        ));
    }
    if let Some(ans) = take_answer(g, "pojun") {
        if ans == "pass" {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
            ));
        }
        let id = if ans == "hand" {
            let idx2 = g.rand_below(g.hand(target).len().max(1));
            g.hand(target).get(idx2).cloned()
        } else {
            ans.strip_prefix("equip:").map(|s| s.to_string())
        };
        let Some(id) = id else {
            return Err("破军：未知选项".into());
        };
        flow::exile_until_turn_end(g, target, &id, "破军")?;
        let picked = picked + 1;
        if picked >= x {
            return Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "targets", "targets": targets, "idx": idx + 1, "card": ask.data.get("card") }),
            ));
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "step": "exile", "targets": targets, "idx": idx, "target": target, "x": x, "picked": picked, "card": ask.data.get("card") }),
        ));
    }
    let mut b = PendingBuilder::new(owner, "pojun_pick").skill("pojun");
    if has_hand {
        b = b.option("hand", "破军：移出其一张随机手牌");
    }
    for id in &equips {
        b = b.option(format!("equip:{id}"), format!("破军：移出其装备【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "step": "exile", "targets": targets, "idx": idx, "target": target, "x": x, "picked": picked, "card": ask.data.get("card") }),
        b.escape("停止移出").build(),
    ))
}

// ---------------------------------------------------------------------------
// 悲歌（界蔡文姬）：一名角色受到【杀】伤害后，弃一张牌令其判定，按花色结算；
// 界版追加：判定牌与弃置牌同花色 → 获得判定牌；同点数 → 获得弃置牌。
// ---------------------------------------------------------------------------

fn beige(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !has(g, owner, "beige") {
        return Ok(Step::Done);
    }
    let event = ask.data.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let stage = ask
        .data
        .get("stage")
        .or_else(|| ask.data.get("step"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if stage == "club" {
        return beige_club(g, ask);
    }
    if stage.is_empty() {
        if event != "DamageAfter" {
            return Ok(Step::Done);
        }
        if ask.data.get("damage_source").and_then(|v| v.as_str()) != Some("Slash") {
            return Ok(Step::Done);
        }
        let target = ask
            .data
            .get("victim")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(owner);
        let by = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
        let cards: Vec<String> = common::own_cards(g, owner);
        if cards.is_empty() {
            return Ok(Step::Done);
        }
        if let Some(ans) = take_answer(g, "beige") {
            if ans == "pass" {
                return Ok(Step::Done);
            }
            let Some(id) = ans.strip_prefix("pick:") else {
                return Err("悲歌：未知选项".into());
            };
            if !cards.iter().any(|c| c == id) {
                return Err("悲歌：该牌不在手牌/装备区".into());
            }
            let suit = g.card_defs.get(id).map(|c| c.suit).unwrap_or(Suit::Spade);
            let rank = g.card_defs.get(id).map(|c| c.rank).unwrap_or(0);
            flow::discard(g, owner, id, "悲歌");
            let data = json!({
                "step": "result",
                "target": target,
                "by": by,
                "cost": id,
                "cost_suit": format!("{:?}", suit),
                "cost_rank": rank,
            });
            super::super::quick_judge::start(
                g,
                target,
                "悲歌",
                Some(("beige", owner, data)),
            );
            return Ok(Step::Done);
        }
        let mut b = PendingBuilder::new(owner, "beige_cost").skill("beige");
        for id in &cards {
            b = b.option(format!("pick:{id}"), format!("悲歌：弃置【{}】并令其判定", card_label(g, id)));
        }
        return Ok(Step::NeedWith(
            Stage::Choose,
            json!({ "stage": "", "event": "DamageAfter", "victim": target, "by": by, "damage_source": "Slash" }),
            b.escape("不发动悲歌").build(),
        ));
    }

    // result：quick_judge 注入 suit/rank/judge_card；按花色结算。
    let target = ask.data.get("target").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let by = ask.data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
    let suit = ask.data.get("suit").and_then(|v| v.as_str()).unwrap_or("");
    let rank = ask.data.get("rank").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let judge_card = ask.data.get("judge_card").and_then(|v| v.as_str()).unwrap_or("");
    let cost = ask.data.get("cost").and_then(|v| v.as_str()).unwrap_or("");
    let cost_suit = ask.data.get("cost_suit").and_then(|v| v.as_str()).unwrap_or("");
    let cost_rank = ask.data.get("cost_rank").and_then(|v| v.as_u64()).unwrap_or(0) as u8;

    match suit {
        "Heart" => {
            flow::heal(g, target, 1, "悲歌");
        }
        "Diamond" => {
            flow::draw(g, target, 2);
        }
        "Club" => {
            // 伤害来源弃置两张牌（不足则弃光）；进入 club 阶段询问来源。
            if let Some(src) = by {
                let cards: Vec<String> = common::own_cards(g, src);
                if !cards.is_empty() {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "stage": "club", "target": target, "by": by, "club_left": 2, "cost": cost, "cost_suit": cost_suit, "cost_rank": cost_rank, "judge_card": judge_card, "suit": suit, "rank": rank }),
                    ));
                }
            }
        }
        "Spade" => {
            if let Some(src) = by {
                skills::flip_seat(g, src);
            }
        }
        _ => {}
    }
    beige_bonus(g, owner, judge_card, suit, cost, cost_suit, rank, cost_rank);
    Ok(Step::Done)
}

/// 悲歌·界版追加：同花色获得判定牌；同点数获得弃置牌（可叠加）。
fn beige_bonus(
    g: &mut Game,
    owner: usize,
    judge_card: &str,
    suit: &str,
    cost: &str,
    cost_suit: &str,
    rank: u8,
    cost_rank: u8,
) {
    if !judge_card.is_empty() && suit == cost_suit && matches!(g.cards.get(judge_card), Some(Zone::Discard)) {
        flow::gain(g, owner, judge_card, owner, "悲歌");
    }
    if !cost.is_empty() && rank == cost_rank && matches!(g.cards.get(cost), Some(Zone::Discard)) {
        flow::gain(g, owner, cost, owner, "悲歌");
    }
}

/// 悲歌·梅花：伤害来源弃置两张牌（逐张；不足则弃光）。
fn beige_club(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    let _ = owner;
    let src = ask.data.get("by").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let left = ask.data.get("club_left").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if left == 0 {
        return Ok(Step::Done);
    }
    let cards: Vec<String> = common::own_cards(g, src);
    if cards.is_empty() {
        return Ok(Step::Done);
    }
    if let Some(ans) = take_answer(g, "beige") {
        let Some(id) = ans.strip_prefix("pick:") else {
            return Err("悲歌：未知选项".into());
        };
        if !g.hand(src).iter().any(|c| c == id) && !super::super::gear::equips_of(g, src).iter().any(|c| c == id) {
            return Err("悲歌：该牌不在其区域".into());
        }
        flow::discard(g, src, id, "悲歌");
        let left = left - 1;
        if left == 0 {
            let judge_card = ask.data.get("judge_card").and_then(|v| v.as_str()).unwrap_or("");
            let suit = ask.data.get("suit").and_then(|v| v.as_str()).unwrap_or("");
            let cost = ask.data.get("cost").and_then(|v| v.as_str()).unwrap_or("");
            let cost_suit = ask.data.get("cost_suit").and_then(|v| v.as_str()).unwrap_or("");
            let rank = ask.data.get("rank").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            let cost_rank = ask.data.get("cost_rank").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            let owner = ask.actor;
            beige_bonus(g, owner, judge_card, suit, cost, cost_suit, rank, cost_rank);
            return Ok(Step::Done);
        }
        return Ok(Step::Next(
            Stage::Choose,
            json!({ "stage": "club", "by": src, "club_left": left, "target": ask.data.get("target"), "cost": ask.data.get("cost"), "cost_suit": ask.data.get("cost_suit"), "cost_rank": ask.data.get("cost_rank"), "judge_card": ask.data.get("judge_card"), "suit": ask.data.get("suit"), "rank": ask.data.get("rank") }),
        ));
    }
    let mut b = PendingBuilder::new(src, "beige_club").skill("beige");
    for id in &cards {
        b = b.option(format!("pick:{id}"), format!("悲歌：弃置【{}】", card_label(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::Choose,
        json!({ "stage": "club", "by": src, "club_left": left, "target": ask.data.get("target"), "cost": ask.data.get("cost"), "cost_suit": ask.data.get("cost_suit"), "cost_rank": ask.data.get("cost_rank"), "judge_card": ask.data.get("judge_card"), "suit": ask.data.get("suit"), "rank": ask.data.get("rank") }),
        b.escape_mandatory("（必须弃置）").build(),
    ))
}

// ---------------------------------------------------------------------------
// 未实现占位（后续技能组补齐；不注册 = 不进注册表/不挂载）
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _unused(g: &Game, id: &str) -> String {
    card_label(g, id)
}
