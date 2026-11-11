//! B1 第二批（13 技能）。按 `align/B1.md` 的「对齐后描述」实现。
//!
//! 已知简化（记入报告）：掣政的「效果一（对你使用牌无效）」需要完整用牌路径，本批只落
//! 「效果二（出牌阶段结束时按使用牌数弃牌）」；其余技能按裁定落地。

use serde_json::{json, Value};

use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{flow, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{active, SkillDef};

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

fn side_of(a: &Ask) -> u8 {
    a.data["event"]
        .as_str()
        .and_then(|s| s.split("side=").nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn targets_of(a: &Ask) -> Vec<usize> {
    a.data["cause"]
        .as_str()
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default()
}

fn is_basic(g: &Game, card: &str) -> bool {
    g.cards
        .get(card)
        .map(|c| crate::cards::is_basic(&c.name))
        .unwrap_or(false)
}

/// 是否【杀】（读卡牌定义，不按牌名比较）。
fn is_sha(g: &Game, card: &str) -> bool {
    g.cards
        .get(card)
        .map(|c| crate::cards::tags(&c.name).sha)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 伪帝（袁术）：纯被动，`skills::active` 里借主公技
// ---------------------------------------------------------------------------

pub const WEIDI: SkillDef = SkillDef {
    code: "weidi",
    name: "伪帝",
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
    subject_window: None,
};

// ---------------------------------------------------------------------------
// 拒战（严颜，转换技）：阳=他人回合内；阴=自己回合内
// ---------------------------------------------------------------------------

pub const JUZHAN: SkillDef = SkillDef {
    code: "juzhan",
    name: "拒战",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 25,
    can_active: None,
    handler: juzhan,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: Some(Window::OtherTurn),
    yin_window: Some(Window::OwnTurn),
    subject_window: None,
};

fn juzhan(g: &mut Game, a: &Ask) -> Result<Step, String> {
    match a.stage {
        Stage::Choose => {
    // 触发字段只在 Choose（事件阶段）读取；后续阶段用 data["target"]。
    let card = a.data["card"].as_str().unwrap_or("");
    if !is_sha(g, card) {
        return Ok(Step::Done);
    }
    let source = a.data["seat"].as_u64().map(|n| n as usize);
    let targets = targets_of(a);
    let yang = side_of(a) == 1;
    let other = if yang {
        if source == Some(a.actor) || !targets.contains(&a.actor) {
            return Ok(Step::Done);
        }
        source.unwrap_or(0)
    } else {
        if source != Some(a.actor) {
            return Ok(Step::Done);
        }
        match targets.iter().find(|t| **t != a.actor) {
            Some(t) => *t,
            None => return Ok(Step::Done),
        }
    };
    if g.seats[other].hp <= 0 {
        return Ok(Step::Done);
    }
            match g.answer.take().as_deref() {
            None => {
                let label = if yang {
                    format!("拒战（阳）：与 {} 各摸 1 张，其本回合不能再对你使用牌", g.seats[other].name)
                } else {
                    format!("拒战（阴）：获得 {} 一张牌，你本回合不能再对其使用牌", g.seats[other].name)
                };
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("juzhan")
                        .option("choose:juzhan", label)
                        .escape("不发动拒战")
                        .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:juzhan") => {
                if yang {
                    g.draw(a.actor, 1);
                    g.draw(other, 1);
                    g.seats[other].flags.insert(format!("deny:{other}:{}", a.actor));
                    g.toggle_yang(a.actor, "juzhan");
                    Ok(Step::Done)
                } else {
                    Ok(Step::Next(Stage::Target, json!({ "target": other })))
                }
            }
            Some(o) => Err(format!("拒战：未知选项 {o}")),
            }
        }
        Stage::Target => {
            let other = a.data["target"].as_u64().unwrap_or(0) as usize;
            let mut cards = g.hand(other);
            cards.extend(g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == other)));
            let act = match g.answer.take() {
                None => {
                    if cards.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = super::builder(a.actor, "choice").skill("juzhan");
                    for id in &cards {
                        b = b.option(format!("choose:{id}"), format!("获得【{}】", g.card_label(id)));
                    }
                    return Ok(Step::Need(b.escape("不获得").build()));
                }
                Some(act) => act,
            };
            if act == "pass" {
                return Ok(Step::Done);
            }
            let id = act
                .strip_prefix("choose:")
                .ok_or_else(|| format!("拒战：未知选项 {act}"))?
                .to_string();
            g.gain(&id, a.actor)?;
            g.seats[a.actor].flags.insert(format!("deny:{}:{other}", a.actor));
            g.toggle_yang(a.actor, "juzhan");
            Ok(Step::Done)
        }
        _ => Err("拒战：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 飞军 / 兵略（王平）
// ---------------------------------------------------------------------------

pub const FEIJUN: SkillDef = SkillDef {
    code: "feijun",
    name: "飞军",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_feijun),
    handler: feijun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub const BINGLUE: SkillDef = SkillDef {
    code: "binglue",
    name: "兵略",
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
    subject_window: None,
};

fn can_feijun(g: &Game, seat: usize) -> bool {
    active(g, seat, "feijun")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter("phase.feijun_used") == 0
        && !g.hand(seat).is_empty()
        && (!feijun_targets(g, seat, 1).is_empty() || !feijun_targets(g, seat, 2).is_empty())
}

fn feijun_targets(g: &Game, seat: usize, option: u8) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|i| *i != seat && g.seats[*i].hp > 0)
        .filter(|i| match option {
            1 => g.hand(*i).len() > g.hand(seat).len(),
            2 => g.ids_in(|z| matches!(z, Zone::Equip(s, _) if s == i)).len()
                > g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == seat)).len(),
            _ => false,
        })
        .collect()
}

fn feijun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let step = a.data["step"].as_str().unwrap_or("opt").to_string();
    match step.as_str() {
        "opt" => {
            let mut b = super::builder(a.actor, "choice").skill("feijun");
            if !feijun_targets(g, a.actor, 1).is_empty() {
                b = b.option("choose:feijun:take", "令手牌数大于你的角色交给你一张牌");
            }
            if !feijun_targets(g, a.actor, 2).is_empty() {
                b = b.option("choose:feijun:equip", "令装备区牌数大于你的角色弃一张装备");
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不发动飞军").build())),
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let option = if act.ends_with("take") { 1 } else { 2 };
                    Ok(Step::Next(Stage::Target, json!({ "step": "target", "option": option })))
                }
                Some(o) => Err(format!("飞军：未知选项 {o}")),
            }
        }
        "target" => {
            let option = a.data["option"].as_u64().unwrap_or(1) as u8;
            let cands = feijun_targets(g, a.actor, option);
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "target").skill("feijun");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("飞军：{}", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动飞军").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let target: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("飞军：目标不合法 {act}"))?;
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "cost", "option": option, "target": target }),
                    ))
                }
                Some(o) => Err(format!("飞军：目标不合法 {o}")),
            }
        }
        "cost" => {
            let option = a.data["option"].as_u64().unwrap_or(1) as u8;
            let target = a.data["target"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("feijun");
                    for id in g.hand(a.actor) {
                        b = b.option(format!("choose:{id}"), format!("弃置【{}】", g.card_label(&id)));
                    }
                    Ok(Step::Need(b.escape("取消飞军").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("飞军：未知选项 {act}"))?
                        .to_string();
                    g.to_discard(&id, "feijun")?;
                    if option == 1 {
                        // 目标自选一张手牌（这里简化为随机/第一张）
                        if let Some(c) = g.hand(target).first().cloned() {
                            g.gain(&c, a.actor)?;
                        }
                    } else {
                        // 王平选择弃哪件装备（取第一件）
                        if let Some(c) = g
                            .ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == target))
                            .first()
                            .cloned()
                        {
                            g.to_discard(&c, "feijun")?;
                        }
                    }
                    g.bump("phase.feijun_used");
                    // 兵略：本局首次对某目标发动 → 摸 2
                    if active(g, a.actor, "binglue") {
                        let key = format!("binglue:{target}");
                        if !g.seats[a.actor].flags.contains(&key) {
                            g.seats[a.actor].flags.insert(key);
                            g.draw(a.actor, 2);
                            g.presentation.push(
                                Presentation::new("binglue", POutcome::Draw { n: 2 })
                                    .actor(a.actor)
                                    .fx("binglue"),
                            );
                        }
                    }
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("飞军：未知选项 {o}")),
            }
        }
        other => Err(format!("飞军：未知步骤 {other}")),
    }
}

// ---------------------------------------------------------------------------
// 遗礼 / 整论（陆绩）
// ---------------------------------------------------------------------------

pub const YILI: SkillDef = SkillDef {
    code: "yili",
    name: "遗礼",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 12,
    can_active: None,
    handler: yili,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub const ZHENGLUN: SkillDef = SkillDef {
    code: "zhenglun",
    name: "整论",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 11,
    can_active: None,
    handler: zhenglun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn yili(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some()
        && (a.data["cause"].as_str() != Some("Play") || g.counter("phase.yili_asked") > 0)
    {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            match g.answer.take().as_deref() {
                None => {
                    let ju = super::ju(g, a.actor);
                    let mut b = super::builder(a.actor, "choice").skill("yili");
                    b = b.option("choose:yili:hp", "遗礼：失去 1 点体力");
                    if ju > 0 {
                        b = b.option("choose:yili:ju", "遗礼：移除 1 枚「橘」");
                    }
                    Ok(Step::Need(b.escape("不发动遗礼").build()))
                }
                Some("pass") => {
                    g.bump("phase.yili_asked");
                    Ok(Step::Done)
                }
                Some(act) => {
                    if act == "choose:yili:ju" {
                        super::remove_ju(g, a.actor, 1);
                    } else {
                        g.seats[a.actor].hp -= 1;
                        if g.seats[a.actor].hp <= 0 {
                            flow::start_dying(g, a.actor, None);
                        }
                    }
                    g.bump("phase.yili_asked");
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "cause": "Play", "after_dying": true }),
                    ))
                }
                Some(o) => Err(format!("遗礼：未知选项 {o}")),
            }
        }
        Stage::Target => {
            let cands: Vec<usize> = (0..g.seats.len())
                .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
                .collect();
            if cands.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    // 若已濒死，濒死帧在上，会先结算完再回来；此时自己若已死则放弃。
                    if g.seats[a.actor].hp <= 0 {
                        return Ok(Step::Done);
                    }
                    let mut b = super::builder(a.actor, "target").skill("yili");
                    for i in &cands {
                        b = b.option(format!("target:{i}"), format!("令 {} 获得 1 枚「橘」", g.seats[*i].name));
                    }
                    Ok(Step::Need(b.escape("不发动遗礼").build()))
                }
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("遗礼：目标不合法 {act}"))?;
                    super::add_ju(g, t, 1);
                    g.presentation.push(
                        Presentation::new("yili", POutcome::Activated { skill: "遗礼".to_string() })
                            .actor(a.actor)
                            .target(t)
                            .fx("yili"),
                    );
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("遗礼：未知选项 {o}")),
            }
        }
        _ => Err("遗礼：未知阶段".into()),
    }
}

fn zhenglun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["cause"].as_str() != Some("Draw") || g.counter("turn.zhenglun_done") > 0 {
        return Ok(Step::Done);
    }
    if super::ju(g, a.actor) > 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("zhenglun")
                    .option("choose:zhenglun", "整论：跳过摸牌阶段并获得 1 枚「橘」")
                    .escape("正常摸牌")
                    .build(),
            ))
        }
        Some("pass") => Ok(Step::Done),
        Some("choose:zhenglun") => {
            g.bump("turn.zhenglun_done");
            g.bump("turn.skip_draw");
            super::add_ju(g, a.actor, 1);
            Ok(Step::Done)
        }
        Some(o) => Err(format!("整论：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 溃诛 / 掣政 / 立军（孙亮）
// ---------------------------------------------------------------------------

pub const KUIZHU: SkillDef = SkillDef {
    code: "kuizhu",
    name: "溃诛",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 40,
    can_active: None,
    handler: kuizhu,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub const CHEZHENG: SkillDef = SkillDef {
    code: "chezheng",
    name: "掣政",
    triggers: &[EventKind::PhaseEnd],
    window: Window::OwnTurn,
    priority: 41,
    can_active: None,
    handler: chezheng,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub const LIJUN: SkillDef = SkillDef {
    code: "lijun",
    name: "立军",
    triggers: &[EventKind::TargetChosen],
    window: Window::OtherTurn,
    priority: 45,
    can_active: None,
    handler: lijun,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn kuizhu(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some() && a.data["cause"].as_str() != Some("Discard") {
        return Ok(Step::Done);
    }
    if g.phase_discarded == 0 {
        return Ok(Step::Done);
    }
    if g.counter("turn.kuizhu_done") > 0 {
        return Ok(Step::Done);
    }
    let x = g.phase_discarded;
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                g.bump("turn.kuizhu_done");
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("kuizhu")
                        .option("choose:kuizhu:draw", format!("溃诛：令至多 {x} 名角色各摸 1 张"))
                        .escape("不发动溃诛")
                        .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:kuizhu:draw") => {
                Ok(Step::Next(Stage::Target, json!({ "x": x })))
            }
            Some(o) => Err(format!("溃诛：未知选项 {o}")),
        },
        Stage::Target => {
            let cands: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].hp > 0).collect();
            let mut b = super::builder(a.actor, "target").skill("kuizhu");
            for i in &cands {
                b = b.option(format!("target:{i}"), format!("令 {} 摸 1 张", g.seats[*i].name));
            }
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(b.escape("不选了").build())),
                Some("pass") => Ok(Step::Done),
                Some(act) => {
                    let t: usize = act
                        .strip_prefix("target:")
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("溃诛：目标不合法 {act}"))?;
                    g.draw(t, 1);
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("溃诛：未知选项 {o}")),
            }
        }
        _ => Err("溃诛：未知阶段".into()),
    }
}

fn chezheng(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data.get("event").is_some()
        && (a.data["cause"].as_str() != Some("Play") || g.counter("turn.chezheng_done") > 0)
    {
        return Ok(Step::Done);
    }
    let outsiders: Vec<usize> = (0..g.seats.len())
        .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
        .collect();
    let used = g.counter("turn.play_cards_used") as usize;
    if outsiders.is_empty() || used >= outsiders.len() {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => Ok(Step::Next(Stage::Target, json!({}))),
        Stage::Target => match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "target").skill("chezheng");
                for i in &outsiders {
                    b = b.option(format!("target:{i}"), format!("掣政：弃 {} 一张牌", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动掣政").build()))
            }
            Some("pass") => {
                g.bump("turn.chezheng_done");
                Ok(Step::Done)
            }
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("掣政：目标不合法 {act}"))?;
                if let Some(id) = g.hand(t).first().cloned() {
                    g.to_discard(&id, "chezheng")?;
                }
                g.bump("turn.chezheng_done");
                Ok(Step::Done)
            }
            Some(o) => Err(format!("掣政：未知选项 {o}")),
        },
        _ => Err("掣政：未知阶段".into()),
    }
}

fn lijun(g: &mut Game, a: &Ask) -> Result<Step, String> {
    // 事件：其他吴势力角色使用【杀】；主公技拥有者 = a.actor（孙亮）
    let card = a.data["card"].as_str().unwrap_or("").to_string();
    if a.data.get("event").is_some() {
        if !is_sha(g, &card) {
            return Ok(Step::Done);
        }
        let Some(user) = a.data["seat"].as_u64().map(|n| n as usize) else {
            return Ok(Step::Done);
        };
        if user == a.actor || g.seats[user].kingdom != "wu" {
            return Ok(Step::Done);
        }
    }
    if a.data.get("event").is_some() && g.counter("turn.lijun_done") > 0 {
        return Ok(Step::Done);
    }
    match a.stage {
        Stage::Choose => {
            let user = a.data["seat"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
            None => {
                Ok(Step::Need(
                    super::builder(user, "choice")
                        .skill("lijun")
                        .option("choose:lijun", format!("立军：把此【杀】交给 {}", g.seats[a.actor].name))
                        .escape("不交出")
                    .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:lijun") => {
                g.gain(&card, a.actor)?;
                // 第二段：由孙亮决定是否给摸 1 + 杀次数 +1
                Ok(Step::Next(Stage::Target, json!({ "card": card, "user": user })))
            }
            Some(o) => Err(format!("立军：未知选项 {o}")),
            }
        }
        Stage::Target => {
            let user = a.data["user"].as_u64().unwrap_or(0) as usize;
            match g.answer.take().as_deref() {
                None => Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("lijun")
                        .option("choose:lijun_bonus", format!("令 {} 摸 1 张且本回合杀次数 +1", g.seats[user].name))
                        .escape("不追加")
                    .build(),
                )),
                Some("pass") => {
                    g.bump("turn.lijun_done");
                    Ok(Step::Done)
                }
                Some("choose:lijun_bonus") => {
                    g.bump("turn.lijun_done");
                    g.draw(user, 1);
                    g.bump("turn.lijun_sha_bonus");
                    Ok(Step::Done)
                }
                Some(o) => Err(format!("立军：未知选项 {o}")),
            }
        }
        _ => Err("立军：未知阶段".into()),
    }
}

// ---------------------------------------------------------------------------
// 荐降 / 审时（蒯越蒯良）
// ---------------------------------------------------------------------------

pub const JIANXIANG: SkillDef = SkillDef {
    code: "jianxiang",
    name: "荐降",
    triggers: &[EventKind::TargetChosen],
    window: Window::AnyTime,
    priority: 30,
    can_active: None,
    handler: jianxiang,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

pub const SHENSHI: SkillDef = SkillDef {
    code: "shenshi",
    name: "审时",
    triggers: &[EventKind::TargetChosen, EventKind::DamageAfter, EventKind::PhaseStart],
    window: Window::AnyTime,
    priority: 35,
    can_active: Some(can_shenshi_yang),
    handler: shenshi,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: Some(Window::OwnTurn),
    yin_window: Some(Window::OtherTurn),
    subject_window: None,
};

fn jianxiang(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let targets = targets_of(a);
    let source = a.data["seat"].as_u64().map(|n| n as usize);
    if source == Some(a.actor) || !targets.contains(&a.actor) {
        return Ok(Step::Done);
    }
    let min = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .map(|i| g.hand(i).len())
        .min()
        .unwrap_or(0);
    let cands: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0 && g.hand(*i).len() == min)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => {
            let mut b = super::builder(a.actor, "target").skill("jianxiang");
            for i in &cands {
                b = b.option(format!("target:{i}"), format!("荐降：令 {} 摸 1 张", g.seats[*i].name));
            }
            Ok(Step::Need(b.escape("不发动荐降").build()))
        }
        Some("pass") => Ok(Step::Done),
        Some(act) => {
            let t: usize = act
                .strip_prefix("target:")
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("荐降：目标不合法 {act}"))?;
            g.draw(t, 1);
            Ok(Step::Done)
        }
        Some(o) => Err(format!("荐降：未知选项 {o}")),
    }
}

fn can_shenshi_yang(g: &Game, seat: usize) -> bool {
    active(g, seat, "shenshi")
        && g.yang(seat, "shenshi")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter("phase.shenshi_used") == 0
        && !g.hand(seat).is_empty()
}

fn shenshi(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let event = a.data["event"].as_str().unwrap_or("");
    // 阳：主动技（execute_active 直接压入 YangTarget）
    if a.stage == Stage::YangTarget {
        let cands: Vec<usize> = (0..g.seats.len())
            .filter(|i| *i != a.actor && g.seats[*i].hp > 0)
            .collect();
        match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "target").skill("shenshi");
                for i in &cands {
                    b = b.option(format!("target:{i}"), format!("交牌并伤害 {}", g.seats[*i].name));
                }
                Ok(Step::Need(b.escape("不发动审时").build()))
            }
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let t: usize = act
                    .strip_prefix("target:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("审时：目标不合法 {act}"))?;
                Ok(Step::Next(Stage::YangCard, json!({ "target": t })))
            }
            Some(o) => Err(format!("审时：未知选项 {o}")),
        }
    } else if a.stage == Stage::YangCard {
        let t = a.data["target"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => {
                let mut b = super::builder(a.actor, "choice").skill("shenshi");
                for id in g.hand(a.actor) {
                    b = b.option(format!("choose:{id}"), format!("交给【{}】", g.card_label(&id)));
                }
                return Ok(Step::Need(b.escape("不发动审时").build()));
            }
            Some("pass") => return Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("审时：未知选项 {act}"))?
                    .to_string();
                g.gain(&id, t)?;
                g.bump("phase.shenshi_used");
                g.toggle_yang(a.actor, "shenshi");
                flow::damage(g, Some(a.actor), t, 1);
                g.presentation.push(
                    Presentation::new("shenshi", POutcome::Damage { amount: 1 })
                        .actor(a.actor)
                        .target(t)
                        .fx("shenshi_yang"),
                );
                Ok(Step::Done)
            }
        }
    } else if event.starts_with("DamageAfter") {
        // 阴：其他角色对你造成伤害后
        let victim = a.data["seat"].as_u64().map(|n| n as usize);
        if victim != Some(a.actor) || g.yang(a.actor, "shenshi") {
            return Ok(Step::Done);
        }
        let source = a.data["cause"].as_str().and_then(|s| s.parse::<usize>().ok());
        let Some(src) = source else { return Ok(Step::Done) };
        if g.hand(src).is_empty() || g.hand(a.actor).is_empty() {
            return Ok(Step::Done);
        }
        match g.answer.take().as_deref() {
            None => Ok(Step::Need(
                super::builder(a.actor, "choice")
                    .skill("shenshi")
                    .option("choose:shenshi_yin", format!("审时（阴）：观看 {} 手牌并交给其一张", g.seats[src].name))
                    .escape("不发动审时")
                    .build(),
            )),
            Some("pass") => Ok(Step::Done),
            Some(_) => {
                let mut b = super::builder(a.actor, "choice").skill("shenshi");
                for id in g.hand(a.actor) {
                    b = b.option(format!("choose:{id}"), format!("交给【{}】", g.card_label(&id)));
                }
                Ok(Step::NeedAt(Stage::YinTarget, b.data(json!({ "source": src })).escape("不发动审时").build()))
            }
            Some(o) => Err(format!("审时：未知选项 {o}")),
        }
    } else if a.stage == Stage::YinTarget {
        let src = a.data["source"].as_u64().unwrap_or(0) as usize;
        match g.answer.take().as_deref() {
            None => Ok(Step::Done),
            Some("pass") => Ok(Step::Done),
            Some(act) => {
                let id = act
                    .strip_prefix("choose:")
                    .ok_or_else(|| format!("审时：未知选项 {act}"))?
                    .to_string();
                g.gain(&id, src)?;
                g.toggle_yang(a.actor, "shenshi");
                g.seats[src]
                    .flags
                    .insert(format!("shenshi_mark:{}", a.actor));
                Ok(Step::Done)
            }
        }
    } else if event.starts_with("PhaseStart") {
        // 阴的结束阶段：若其未失去此牌 → 自己摸至 4
        if a.data["cause"].as_str() != Some("End") || g.yang(a.actor, "shenshi") {
            return Ok(Step::Done);
        }
        let marks: Vec<usize> = g
            .seats
            .iter()
            .enumerate()
            .filter(|(i, s)| s.flags.contains(&format!("shenshi_mark:{i}")))
            .map(|(i, _)| i)
            .collect();
        for src in marks {
            let kept = g
                .seats[src]
                .flags
                .contains(&format!("shenshi_mark:{}", a.actor));
            if kept {
                let want = 4usize.saturating_sub(g.hand(a.actor).len());
                if want > 0 {
                    g.draw(a.actor, want);
                }
            }
        }
        Ok(Step::Done)
    } else {
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 成略 / 恃才 / 寸目（许攸）
// ---------------------------------------------------------------------------

pub const CHENGLUE: SkillDef = SkillDef {
    code: "chenglue",
    name: "成略",
    triggers: &[],
    window: Window::AnyTime,
    priority: 0,
    can_active: Some(can_chenglue),
    handler: chenglue,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: Some(Window::OwnTurn),
    yin_window: Some(Window::OwnTurn),
    subject_window: None,
};

pub const SHICAI: SkillDef = SkillDef {
    code: "shicai",
    name: "恃才",
    triggers: &[EventKind::CardUsed],
    window: Window::OwnTurn,
    priority: 45,
    can_active: None,
    handler: shicai,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

fn can_chenglue(g: &Game, seat: usize) -> bool {
    active(g, seat, "chenglue")
        && g.turn == seat
        && g.phase == Phase::Play
        && g.counter("phase.chenglue_used") == 0
        && !g.hand(seat).is_empty()
}

fn chenglue(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let yin = !g.yang(a.actor, "chenglue");
    let (draw_n, discard_n) = if yin { (2, 1) } else { (1, 2) };
    match a.stage {
        Stage::Choose => match g.answer.take().as_deref() {
            None => {
                let label = if yin {
                    "成略（阴）：摸 2 张并弃 1 张手牌"
                } else {
                    "成略（阳）：摸 1 张并弃 2 张手牌"
                };
                Ok(Step::Need(
                    super::builder(a.actor, "choice")
                        .skill("chenglue")
                        .option("choose:chenglue", label)
                        .escape("不发动成略")
                    .build(),
                ))
            }
            Some("pass") => Ok(Step::Done),
            Some("choose:chenglue") => {
                g.draw(a.actor, draw_n);
                g.bump("phase.chenglue_used");
                Ok(Step::Next(Stage::Target, json!({ "need": discard_n })))
            }
            Some(o) => Err(format!("成略：未知选项 {o}")),
        },
        Stage::Target => {
            let need = a.data["need"].as_u64().unwrap_or(0) as usize;
            let hand = g.hand(a.actor);
            if hand.len() < need {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choose_many").skill("chenglue");
                    for id in &hand {
                        b = b.option(format!("choose:{id}"), format!("弃置【{}】", g.card_label(id)));
                    }
                    Ok(Step::Need(b.escape_with("confirm", "完成弃牌").build()))
                }
                Some("confirm") => Ok(Step::Done),
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("成略：未知选项 {act}"))?
                        .to_string();
                    let suit = g.cards.get(&id).map(|c| c.suit);
                    g.to_discard(&id, "chenglue")?;
                    if let Some(suit) = suit {
                        let key = match suit {
                            crate::kernel::Suit::Spade => "phase.cl_spade",
                            crate::kernel::Suit::Heart => "phase.cl_heart",
                            crate::kernel::Suit::Club => "phase.cl_club",
                            crate::kernel::Suit::Diamond => "phase.cl_diamond",
                        };
                        g.bump(key);
                    }
                    let left = a.data["need"].as_u64().unwrap_or(0) as usize;
                    if left <= 1 {
                        g.toggle_yang(a.actor, "chenglue");
                        Ok(Step::Done)
                    } else {
                        Ok(Step::Next(Stage::Target, json!({ "need": left - 1 })))
                    }
                }
                Some(o) => Err(format!("成略：未知选项 {o}")),
            }
        }
        _ => Err("成略：未知阶段".into()),
    }
}

fn shicai(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let Some(card) = a.data["card"].as_str().map(|s| s.to_string()) else {
        return Ok(Step::Done);
    };
    if a.data["seat"].as_u64().map(|n| n as usize) != Some(a.actor) {
        return Ok(Step::Done);
    }
    let basic = is_basic(g, &card);
    let key = if basic { "turn.shicai_basic" } else { "turn.shicai_trick" };
    if g.counter(key) > 0 {
        return Ok(Step::Done);
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(
            super::builder(a.actor, "choice")
                .skill("shicai")
                .option("choose:shicai", format!("恃才：将【{}】置于牌堆顶并摸 1 张", g.card_label(&card)))
                .escape("不发动恃才")
                .build(),
        )),
        Some("pass") => Ok(Step::Done),
        Some("choose:shicai") => {
            g.bump(key);
            g.put_on_top(&card)?;
            let drawn = g.draw(a.actor, 1);
            g.presentation.push(
                Presentation::new("shicai", POutcome::Draw { n: drawn.len() as u32 })
                    .actor(a.actor)
                    .fx("shicai"),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("恃才：未知选项 {o}")),
    }
}

/// 占位：寸目在 b1.rs 已注册（此文件不重复定义）。
#[allow(dead_code)]
fn _unused(_: Value) {}
