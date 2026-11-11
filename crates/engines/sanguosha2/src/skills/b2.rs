//! B2 首批（征荣 / 从谏）：都基于「目标列表」判定。

use serde_json::json;

use crate::cards;
use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, Game, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{SkillDef, SkillKind};

fn targets_of(a: &Ask) -> Vec<usize> {
    a.data["cause"]
        .as_str()
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default()
}

pub const WANGLIE: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "wanglie",
    name: "往烈",
    triggers: &[EventKind::CardUsed],
    window: Window::OwnTurn,
    priority: 44,
    can_active: None,
    handler: wanglie,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 往烈（主动部分）：使用基本牌/普通锦囊时，可以令其不能被响应，然后本阶段不能再使用牌。
fn wanglie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
        return Ok(Step::Done);
    }
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    // 只对基本牌或普通锦囊（读卡牌定义，不按牌名）
    let usable = g
        .cards
        .get(&card)
        .map(|c| {
            let def = cards::def(&c.name);
            def.kind == cards::CardKind::Basic || def.kind == cards::CardKind::Trick
        })
        .unwrap_or(false);
    if !usable || g.counter("turn.wanglie_done") > 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("wanglie")
                .option("choose:wanglie", "往烈：令此牌不能被响应，然后本阶段不能再使用牌")
                .escape("不发动往烈")
                .build(),
        )),
        Some("pass") => {
            g.bump("turn.wanglie_done");
            Ok(Step::Done)
        }
        Some("choose:wanglie") => {
            g.mark_unresponsive(&card);
            g.freeze_hand(a.actor);
            g.bump("turn.wanglie_done");
            g.presentation.push(
                Presentation::new("wanglie", POutcome::Activated {
                    skill: "往烈".to_string(),
                })
                .actor(a.actor)
                .cards(vec![card])
                .fx("wanglie"),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("往烈：未知选项 {o}")),
    }
}

pub const FUYIN: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "fuyin",
    name: "父荫",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 60,
    can_active: None,
    handler: fuyin,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 父荫（锁定）：每回合首次成为他人【杀】/【决斗】目标后，若其手牌数 ≥ 你，此牌对你无效。
fn fuyin(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let targets = targets_of(a);
    let Some(source) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    if source == a.actor || !targets.contains(&a.actor) || g.counter("turn.fuyin_done") > 0 {
        return Ok(Step::Done);
    }
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    let qualifies = g
        .cards
        .get(&card)
        .map(|c| {
            let tags = cards::tags(&c.name);
            tags.sha || tags.duel
        })
        .unwrap_or(false);
    if !qualifies {
        return Ok(Step::Done);
    }
    if g.hand(source).len() < g.hand(a.actor).len() {
        return Ok(Step::Done);
    }
    g.mark_unresponsive(&card);
    g.bump("turn.fuyin_done");
    g.presentation.push(
        Presentation::new("fuyin", POutcome::Activated {
            skill: "父荫".to_string(),
        })
        .actor(a.actor)
        .target(source)
        .cards(vec![card])
        .fx("fuyin"),
    );
    Ok(Step::Done)
}

pub const JUEYAN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "jueyan",
    name: "决堰",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_jueyan),
    handler: jueyan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_jueyan(g: &Game, seat: usize) -> bool {
    super::active(g, seat, "jueyan")
        && g.turn == seat
        && g.phase == crate::kernel::Phase::Play
        && g.counter("turn.jueyan_used") == 0
        && jueyan_slots(g, seat).len() > 0
}

fn jueyan_slots(g: &Game, seat: usize) -> Vec<crate::kernel::Slot> {
    use crate::kernel::Slot::*;
    [Weapon, Armor, HorseOffense, HorseDefense, Treasure]
        .into_iter()
        .filter(|s| !g.is_slot_abolished(seat, *s))
        .collect()
}

fn jueyan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let slots = jueyan_slots(g, a.actor);
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "choice").skill("jueyan");
                for s in &slots {
                    b = b.option(
                        format!("choose:{}", s.label()),
                        format!("废除{}栏", s.label()),
                    );
                }
                Ok(Step::Need(b.escape("不发动决堰").build()))
            }
            Some("pass") => {
                g.bump("turn.jueyan_used");
                Ok(Step::Done)
            }
            Some(act) => {
                let label = act.trim_start_matches("choose:");
                let slot = slots
                    .iter()
                    .find(|s| s.label() == label)
                    .copied()
                    .ok_or_else(|| format!("决堰：未知栏位 {act}"))?;
                g.abolish_slot(a.actor, slot)?;
                g.bump("turn.jueyan_used");
                match slot {
                    crate::kernel::Slot::Weapon => {
                        for _ in 0..3 {
                            g.bump("turn.jueyan_sha_bonus");
                        }
                    }
                    crate::kernel::Slot::Armor => {
                        for _ in 0..3 {
                            g.bump("turn.jueyan_hand_limit");
                        }
                    }
                    crate::kernel::Slot::HorseOffense | crate::kernel::Slot::HorseDefense => {
                        g.bump("turn.jueyan_no_distance")
                    }
                    crate::kernel::Slot::Treasure => g.bump("turn.jueyan_jizhi"),
                }
                g.presentation.push(
                    Presentation::new("jueyan", POutcome::Activated {
                        skill: "决堰".to_string(),
                    })
                    .actor(a.actor)
                    .fx("jueyan"),
                );
                Ok(Step::Done)
            }
            Some(o) => Err(format!("决堰：未知选项 {o}")),
        },
        _ => Err("决堰：未知阶段".into()),
    }
}

pub const POSHI: SkillDef = SkillDef { kind: SkillKind::Awaken,
    code: "poshi",
    name: "破势",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 2,
    can_active: None,
    handler: poshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 破势（觉醒，窗口=准备阶段）：体力值为 1 时减 1 点体力上限并废除**全部**装备栏。
fn poshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["cause"].as_str() != Some("Prepare") || g.seats[a.actor].hp != 1 {
        return Ok(Step::Done);
    }
    if g.seats[a.actor].flags.contains("awaken_poshi") {
        return Ok(Step::Done);
    }
    g.seats[a.actor].flags.insert("awaken_poshi".to_string());
    g.seats[a.actor].max_hp -= 1;
    use crate::kernel::Slot::*;
    for slot in [Weapon, Armor, HorseOffense, HorseDefense, Treasure] {
        g.abolish_slot(a.actor, slot)?;
    }
    g.presentation.push(
        Presentation::new("poshi", POutcome::Activated {
            skill: "破势".to_string(),
        })
        .actor(a.actor)
        .fx("poshi_awaken"),
    );
    g.push_log(format!("破势：{} 觉醒，废除全部装备栏", g.seats[a.actor].name));
    Ok(Step::Done)
}

pub const XIONGLUAN: SkillDef = SkillDef { kind: SkillKind::Limited,
    code: "xiongluan",
    name: "雄乱",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_xiongluan),
    handler: xiongluan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_xiongluan(g: &Game, seat: usize) -> bool {
    super::active(g, seat, "xiongluan")
        && g.turn == seat
        && g.phase == crate::kernel::Phase::Play
        && !g.seats[seat].flags.contains("xiongluan_used")
}

/// 雄乱（限定技）：废除张绣的装备区与判定区；此后其对目标的牌不能被响应。
fn xiongluan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    use crate::kernel::Slot::*;
    for slot in [Weapon, Armor, HorseOffense, HorseDefense, Treasure] {
        g.abolish_slot(a.actor, slot)?;
    }
    g.seats[a.actor].judge_abolished = true;
    g.seats[a.actor].flags.insert("xiongluan_used".to_string());
    g.seats[a.actor].flags.insert("xiongluan_active".to_string());
    g.presentation.push(
        Presentation::new("xiongluan", POutcome::Activated {
            skill: "雄乱".to_string(),
        })
        .actor(a.actor)
        .fx("xiongluan"),
    );
    g.push_log(format!("雄乱：{} 废除装备区与判定区", g.seats[a.actor].name));
    Ok(Step::Done)
}

pub const QIANJIE: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "qianjie",
    name: "谦节",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: None,
    handler: noop,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: Some(qianjie_blocker),
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 谦节：不能成为 ①延时锦囊 ②铁索连环 的目标（拼点由 `skills::blocks_pindian` 拦截）。
fn qianjie_blocker(
    g: &Game,
    _seat: usize,
    _source: usize,
    card: &str,
    carrier: Option<&crate::kernel::CardId>,
) -> bool {
    // `card` 一般是牌名；兼容旧调用（传牌 id）时按 id 查实体牌名。
    let name = if g.cards.contains_key(card) && !cards::def(card).known {
        g.name_of(&card.to_string())
    } else {
        card.to_string()
    };
    let _ = carrier;
    let tags = cards::tags(&name);
    tags.delayed || tags.lianhuan
}

pub const ZHENGU: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhengu",
    name: "镇骨",
    triggers: &[EventKind::PhaseEnd],
    window: Window::AnyTime,
    priority: 42,
    can_active: Some(can_zhengu),
    handler: zhengu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_zhengu(g: &Game, seat: usize) -> bool {
    super::active(g, seat, "zhengu")
        && g.turn == seat
        && g.phase == crate::kernel::Phase::Play
        && g.counter("turn.zhengu_used") == 0
}

/// 手牌数向郝昭看齐：多则弃、少则摸。
fn align_hand(g: &mut Game, owner: usize, target: usize) {
    let want = g.hand(owner).len();
    let cur = g.hand(target).len();
    if cur > want {
        for id in g.hand(target).into_iter().take(cur - want) {
            let _ = g.to_discard(&id, "zhengu");
        }
    } else if cur < want {
        g.draw(target, want - cur);
    }
}

/// 镇骨：出牌阶段限一次，选目标 → **立刻结算一次** + 记下「其回合结束再结算一次」。
fn zhengu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 事件通道（PhaseEnd）不是主动入口：第二时刻由 `on_turn_end_for_scheduled` 统一结算。
    if a.data["event"].is_string() {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            // 主动发动（事件为空）
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("zhengu");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("镇骨：{}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动镇骨").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("镇骨：目标不合法 {act}"))?;
                    g.bump("turn.zhengu_used");
                    align_hand(g, a.actor, t);
                    g.scheduled.push((a.actor, t));
                    g.presentation.push(
                        Presentation::new("zhengu", POutcome::Activated {
                            skill: "镇骨".to_string(),
                        })
                        .actor(a.actor)
                        .target(t)
                        .fx("zhengu_now"),
                    );
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("镇骨：未知选项 {o}")),
            }
        }
        _ => Ok(Step::Done),
    }
}

/// 第二时刻：被选角色的回合结束时再结算一次（由 PhaseEnd 事件驱动）。
pub(crate) fn zhengu_end(g: &mut Game, owner: usize, ended: usize) {
    if let Some(pos) = g.scheduled.iter().position(|(o, t)| *o == owner && *t == ended) {
        g.scheduled.remove(pos);
        align_hand(g, owner, ended);
        g.presentation.push(
            Presentation::new("zhengu", POutcome::Activated {
                skill: "镇骨".to_string(),
            })
            .actor(owner)
            .target(ended)
            .fx("zhengu_end"),
        );
    }
}

pub const ZUILUN: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zuilun",
    name: "罪论",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 46,
    can_active: None,
    handler: zuilun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 罪论（按官方原文）：结束阶段观看牌堆顶 3 张；你每满足一项条件，获得其中一张牌；
/// 剩余牌以任意顺序置于牌堆顶：2 张 → 由玩家在两种排列中选一（`order:a,b` / `order:b,a`）；
/// 1 张 → 直接回牌堆顶；0 张 → 无剩余。「均不满足」→ 你选择一名其他角色，双方各失去 1 点体力。
fn zuilun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.data["step"].as_str().unwrap_or("reveal") {
        "pick" => zuilun_pick(g, a),
        "order" => zuilun_order(g, a),
        "victim" => zuilun_victim(g, a),
        _ => zuilun_reveal(g, a),
    }
}

fn zuilun_reveal(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() && a.data["cause"].as_str() != Some("End") {
        return Ok(Step::Done);
    }
    if g.counter("turn.zuilun_done") > 0 {
        return Ok(Step::Done);
    }
    g.bump("turn.zuilun_done");
    let mut revealed: Vec<String> = Vec::new();
    for _ in 0..3 {
        if let Some(id) = g.draw_top() {
            if g.to_processing(&id).is_ok() {
                revealed.push(id);
            }
        }
    }
    let damage = g.counter(&format!("turn.damage_by:{}", a.actor)) > 0;
    let nodiscard = g.counter(&format!("turn.discard_by:{}", a.actor)) == 0;
    let min_hand = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .map(|i| g.hand(i).len())
        .min()
        .unwrap_or(0);
    let least = g.hand(a.actor).len() == min_hand;
    let quota = [damage, nodiscard, least].iter().filter(|x| **x).count();
    if quota == 0 {
        for id in &revealed {
            g.to_discard(id, "zuilun").ok();
        }
        let cands: Vec<usize> = (0..g.seats.len())
            .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
            .collect();
        if cands.is_empty() {
            flow::lose_hp(g, a.actor, 1, "zuilun");
            return Ok(Step::Done);
        }
        return Ok(Step::Next(Stage::Target, json!({ "step": "victim" })));
    }
    Ok(Step::Next(
        Stage::Choose,
        json!({ "step": "pick", "revealed": revealed, "quota": quota, "picked": [] }),
    ))
}

fn zuilun_pick(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let mut revealed: Vec<String> = a.data["revealed"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let quota = a.data["quota"].as_u64().unwrap_or(0) as usize;
    let mut picked: Vec<String> = a.data["picked"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if picked.len() >= quota || revealed.is_empty() {
        return match revealed.len() {
            0 => Ok(Step::Done),
            1 => {
                g.put_on_top(&revealed[0])?;
                Ok(Step::Done)
            }
            _ => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "order", "remaining": revealed }),
            )),
        };
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "choice").skill("zuilun");
            for id in &revealed {
                b = b.option(
                    format!("choose:{id}"),
                    format!("获得【{}】（还可选 {} 张）", g.card_label(id), quota - picked.len()),
                );
            }
            Ok(Step::Need(b.escape("放弃获得").build()))
        }
        Some("pass") => Ok(Step::Next(Stage::Target, json!({ "step": "victim" }))),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("罪论：未知选项 {act}"))?
                .to_string();
            g.to_hand(&id, a.actor)?;
            picked.push(id.clone());
            revealed.retain(|x| x != &id);
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "pick", "revealed": revealed, "quota": quota, "picked": picked }),
            ))
        }
        Some(o) => Err(format!("罪论：未知选项 {o}")),
    }
}

fn zuilun_order(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let remaining: Vec<String> = a.data["remaining"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if remaining.len() != 2 {
        return Ok(Step::Done);
    }
    let (r0, r1) = (remaining[0].clone(), remaining[1].clone());
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "choice").skill("zuilun");
            b = b.option(
                format!("order:{r0},{r1}"),
                format!("第一张【{}】，第二张【{}】", g.card_label(&r0), g.card_label(&r1)),
            );
            b = b.option(
                format!("order:{r1},{r0}"),
                format!("第一张【{}】，第二张【{}】", g.card_label(&r1), g.card_label(&r0)),
            );
            Ok(Step::Need(b.escape("保持原顺序").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let body = act.strip_prefix("order:").unwrap_or("");
            let parts: Vec<&str> = body.split(',').collect();
            if parts.len() != 2 {
                return Err("罪论：排序格式错误".to_string());
            }
            let (first, second) = (parts[0].to_string(), parts[1].to_string());
            let ok = (first == r0 && second == r1) || (first == r1 && second == r0);
            if !ok {
                return Err("罪论：排序必须是剩余两张的排列".to_string());
            }
            g.put_on_top(&second)?;
            g.put_on_top(&first)?;
            Ok(Step::Done)
        }
        Some(o) => Err(format!("罪论：未知选项 {o}")),
    }
}

fn zuilun_victim(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let cands: Vec<usize> = (0..g.seats.len())
        .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("zuilun");
            for i in &cands {
                b = b.option(
                    format!("target:{i}"),
                    format!("罪论：与 {} 各失去 1 点体力", g.seats[*i].name),
                );
            }
            Ok(Step::Need(b.escape("只自己失去").build()))
        }
        Some("pass") => {
            flow::lose_hp(g, a.actor, 1, "zuilun");
            Ok(Step::Done)
        }
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("罪论：目标不合法 {act}"))?;
            flow::lose_hp(g, a.actor, 1, "zuilun");
            flow::lose_hp(g, t, 1, "zuilun");
            g.push_log(format!(
                "罪论：{} 与 {} 各失去 1 点体力",
                g.seats[a.actor].name, g.seats[t].name
            ));
            if g.seats[a.actor].hp <= 0 {
                crate::kernel::flow::start_dying(g, a.actor, None);
            }
            Ok(Step::Done)
        }
        Some(o) => Err(format!("罪论：未知选项 {o}")),
    }
}

pub const HONGJU: SkillDef = SkillDef { kind: SkillKind::Awaken,
    code: "hongju",
    name: "鸿举",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 2,
    can_active: None,
    handler: hongju,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 鸿举（觉醒，准备阶段）：荣 ≥ 3 → 减 1 点体力上限，然后由玩家选**等量**手牌替换「荣」
/// （替换 = 选中的手牌移出游戏成为新的「荣」，张数必须与「荣」数相等）。
fn hongju(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() && a.data["cause"].as_str() != Some("Prepare") {
        return Ok(Step::Done);
    }
    let rong = g.counter("turn.rong") as usize;
    // 觉醒标志只在「触发阶段」拦截；否则玩家一选牌就会被自己短路（第 6 次同族缺陷）。
    if a.stage == Stage::Choose && g.seats[a.actor].flags.contains("awaken_hongju") {
        return Ok(Step::Done);
    }
    if rong < 3 {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            g.seats[a.actor].flags.insert("awaken_hongju".to_string());
            g.seats[a.actor].max_hp -= 1;
            g.presentation.push(
                Presentation::new("hongju", POutcome::Activated {
                    skill: "鸿举".to_string(),
                })
                .actor(a.actor)
                .fx("hongju_awaken"),
            );
            let need = rong.min(g.hand(a.actor).len());
            if need == 0 {
                return Ok(Step::Done);
            }
            Ok(Step::Next(Stage::Target, json!({ "need": need, "picked": [] })))
        }
        Stage::Target => {
            let need = a.data["need"].as_u64().unwrap_or(0) as usize;
            let picked: Vec<String> = a.data["picked"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            if picked.len() >= need {
                g.presentation.push(
                    Presentation::new("hongju", POutcome::Activated {
                        skill: "鸿举".to_string(),
                    })
                    .actor(a.actor)
                    .cards(picked.clone())
                    .fx("hongju_swap"),
                );
                return Ok(Step::Done);
            }
            let hand: Vec<String> = g
                .hand(a.actor)
                .into_iter()
                .filter(|id| !picked.contains(id))
                .collect();
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("hongju");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("替换「荣」：【{}】（还需 {} 张）", g.card_label(id), need - picked.len()),
                        );
                    }
                    Ok(Step::NeedAt(Stage::Target, b.escape_with("confirm", "完成替换").build()))
                }
                Some("confirm") => {
                    if picked.len() == need {
                        g.presentation.push(
                            Presentation::new("hongju", POutcome::Activated {
                                skill: "鸿举".to_string(),
                            })
                            .actor(a.actor)
                            .cards(picked.clone())
                            .fx("hongju_swap"),
                        );
                        Ok(Step::Done)
                    } else {
                        // 等量要求：不足则继续问
                        let mut b = super::builder(a.actor, "choose_many").skill("hongju");
                        for id in &hand {
                            b = b.option(
                                format!("choose:{id}"),
                                format!("必须再选 {} 张替换「荣」：【{}】", need - picked.len(), g.card_label(id)),
                            );
                        }
                        Ok(Step::Need(b.escape_with("confirm", "完成替换").build()))
                    }
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("鸿举：未知选项 {act}"))?
                        .to_string();
                    g.to_void(&id)?;
                    let mut picked = picked;
                    picked.push(id);
                    Ok(Step::Next(Stage::Target, json!({ "need": need, "picked": picked })))
                }
                Some(o) => Err(format!("鸿举：未知选项 {o}")),
            }
        }
        _ => Err("鸿举：未知阶段".into()),
    }
}

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 征荣（毌丘俭）：使用【杀】或伤害锦囊指定目标后，取目标一张牌移出游戏作「荣」
// ---------------------------------------------------------------------------

pub const ZHENGRONG: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "zhengrong",
    name: "征荣",
    triggers: &[EventKind::TargetChosen],
    window: Window::OwnTurn,
    priority: 22,
    can_active: None,
    handler: zhengrong,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn zhengrong(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
    // 触发字段只在 Choose（事件阶段）读取；Target 阶段用 data["target"]。
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
        return Ok(Step::Done);
    }
    let ok_card = g
        .cards
        .get(&card)
        .map(|c| cards::tags(&c.name).sha || cards::is_damage_trick(&c.name))
        .unwrap_or(false);
    if !ok_card {
        return Ok(Step::Done);
    }
    let targets = targets_of(a);
    let cands: Vec<usize> = targets
        .into_iter()
        .filter(|t| *t != a.actor && g.seats[*t].hp > 0)
        .filter(|t| g.hand(*t).len() >= g.hand(a.actor).len())
        .filter(|t| {
            !g.hand(*t).is_empty()
                || !g.ids_in(|z| matches!(z, Zone::Equip(s, _) if s == t)).is_empty()
        })
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
            match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "choice").skill("zhengrong");
                for t in &cands {
                    b = b.option(
                        format!("target:{t}"),
                        format!("征荣：取 {} 一张牌作「荣」", g.seats[*t].name),
                    );
                }
                Ok(Step::Need(b.escape("不发动征荣").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("征荣：目标不合法 {act}"))?;
                Ok(Step::Next(Stage::Target, json!({ "target": t })))
            }
            Some(o) => Err(format!("征荣：未知选项 {o}")),
            }
        }
        Stage::Target => {
            let t = a.data["target"].as_u64().unwrap_or(0) as usize;
            let mut cards_v = g.hand(t);
            cards_v.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == t)));
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("zhengrong");
                    for id in &cards_v {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("取【{}】作「荣」", g.card_label(id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不取荣").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("征荣：未知选项 {act}"))?
                        .to_string();
                    // 「荣」置于武将牌上 + 移出游戏（与不屈「创」同口径）
                    g.to_void(&id)?;
                    g.bump("turn.rong");
                    g.presentation.push(
                        Presentation::new("zhengrong", POutcome::Activated {
                            skill: "征荣".to_string(),
                        })
                        .actor(a.actor)
                        .target(t)
                        .cards(vec![id])
                        .fx("zhengrong"),
                    );
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("征荣：未知选项 {o}")),
            }
        }
        _ => Err("征荣：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 从谏（张绣）：成为目标后，交出自己的手牌/装备，然后摸 1
// ---------------------------------------------------------------------------

pub const CONGJIAN: SkillDef = SkillDef { kind: SkillKind::Locked,
    code: "congjian",
    name: "从谏",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 33,
    can_active: None,
    handler: congjian,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn congjian(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let targets = targets_of(a);
    let Some(source) = a.data["seat"].as_u64().map(|n| n as usize) else {
        return Ok(Step::Done);
    };
    if source == a.actor || !targets.contains(&a.actor) {
        return Ok(Step::Done);
    }
    if g.counter("turn.congjian_done") > 0 {
        return Ok(Step::Done);
    }
    let mut own = g.hand(a.actor);
    own.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == a.actor)));
    if own.is_empty() {
        return Ok(Step::Done); // 没有自己的牌 → 不能发动，也不能摸
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "choice").skill("congjian");
            for id in &own {
                b = b.option(
                    format!("choose:{id}"),
                    format!("从谏：交给 {} 【{}】", g.seats[source].name, g.card_label(id)),
                );
            }
            Ok(Step::Need(b.escape("不发动从谏").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("从谏：未知选项 {act}"))?
                .to_string();
            g.gain(&id, source)?;
            // 「然后」：实际交出后才摸 1
            g.draw(a.actor, 1);
            g.bump("turn.congjian_done");
            g.presentation.push(
                Presentation::new("congjian", POutcome::Draw { n: 1 })
                    .actor(a.actor)
                    .target(source)
                    .fx("congjian"),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("从谏：未知选项 {o}")),
    }
}

#[allow(dead_code)]
fn _unused(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    noop(_g, _a)
}
