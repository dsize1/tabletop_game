//! B0 技能 handler（据守/解围/不屈/奋激/行殇/放逐/颂威）。
//!
//! 每个 handler 是「声明 → 行为」的分发落点：声明在 `b0::SKILLS`（窗口/订阅/钩子），
//! 行为在本模块。handler 只返回 `Step`，栈由 `resolve::advance` 改（spike B 类）。
//! 上下文（谁失牌/谁死亡/伤害来源）从 `Ask.data` 读事件快照，不读 live 状态。

use serde_json::Value;

use super::super::flow;
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

/// 据守：结束阶段翻面（取反）+ 摸 4 张。
fn jushou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let seat = ask.actor;
    g.seats[seat].flipped = !g.seats[seat].flipped; // 取反（B0 Q1 裁定 B）
    flow::draw(g, seat, 4);
    Ok(Step::Done)
}

/// 奋激：其他角色回合外手牌被弃置/获得 → 失去 1 体力令其摸 2 张。
/// 上下文：`data.victim` = 失牌/得牌的角色。
fn fenji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .ok_or("奋激缺少 victim")? as usize;
    flow::lose_hp(g, ask.actor, 1, "奋激");
    flow::draw(g, victim, 2);
    Ok(Step::Done)
}

/// 行殇：其他角色死亡 → 获得其手牌 + 装备区牌（**不含判定区**）。
/// 上下文：`data.victim` = 死亡角色。
fn xingshang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let victim = ask
        .data
        .get("victim")
        .and_then(|v| v.as_u64())
        .ok_or("行殇缺少 victim")? as usize;
    let owned: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| {
            z.seat() == Some(victim) && matches!(z, Zone::Hand(_) | Zone::Equip(..))
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in owned {
        g.cards.insert(id, Zone::Hand(ask.actor));
    }
    Ok(Step::Done)
}

/// 放逐：受到伤害后 → 令一名其他角色翻面摸 X（X = 已损失体力）。
/// 简化：直接对 data.target 翻面摸 X（M4.2 先做核心，选目标 pending 后续补）。
fn fangzhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let target = ask
        .data
        .get("target")
        .and_then(|v| v.as_u64())
        .ok_or("放逐缺少 target")? as usize;
    let lost = g.seats[ask.actor].max_hp - g.seats[ask.actor].hp;
    g.seats[target].flipped = !g.seats[target].flipped;
    flow::draw(g, target, lost.max(0) as usize);
    Ok(Step::Done)
}

/// 颂威（主公技）：其他魏势力黑色判定生效 → 询问判定者是否令主公摸 1。
/// 简化：直接令主公摸 1（询问 pending 后续补）。
fn songwei(g: &mut Game, _ask: &Ask) -> Result<Step, String> {
    // 主公 = 座位 0（简化；M4 后续按身份找主公）。
    flow::draw(g, 0, 1);
    Ok(Step::Done)
}

/// 解围：装备区牌当【无懈可击】（转化）+ 翻回正面移动装备（M4.2 先做转化骨架）。
fn jiewei(_g: &mut Game, _ask: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

/// 不屈（锁定）：濒死翻「创」判定——判定牌点数与现有「创」均不同则回复至 1，否则弃。
/// 简化：摸一张判定牌作为「创」，点数重复进弃牌堆（完整求桃流程 M4.2 后续）。
fn buqu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let seat = ask.actor;
    // 摸一张作为「创」判定牌（简化：用牌堆顶）。
    let Some(judge) = g.deck.pop() else {
        return Ok(Step::Done);
    };
    let rank = g.card_defs.get(&judge).map(|c| c.rank).unwrap_or(0);
    // 现有「创」的点数
    let dup = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Mark(s, name) if *s == seat && name == "创"))
        .any(|(id, _)| g.card_defs.get(id).map(|c| c.rank) == Some(rank));
    if dup {
        g.cards.insert(judge, Zone::Discard); // 点数重复进弃牌堆
    } else {
        g.cards.insert(judge, Zone::Mark(seat, "创".to_string()));
        g.seats[seat].hp = g.seats[seat].hp.max(1); // 回复至 1
    }
    Ok(Step::Done)
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
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "jiewei",
        name: "解围",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::WuxieOpt, Hook::Flip],
            can_active: false,
        }],
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
            // 奋激：其他角色的手牌被弃置/获得 → subject = Other（M105 教训的类型级对策）。
            subscriptions: &[
                Subscription { event: EventKind::CardDiscarded, subject: Subject::Other },
                Subscription { event: EventKind::CardGained, subject: Subject::Other },
            ],
            hooks: &[],
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
            subscriptions: &[Subscription { event: EventKind::Death, subject: Subject::Other }],
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
            subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
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
            subscriptions: &[Subscription { event: EventKind::JudgeEffective, subject: Subject::Other }],
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
