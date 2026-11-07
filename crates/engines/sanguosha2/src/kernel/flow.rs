//! 规则流程（内核级、与具体技能无关）：伤害 / 濒死 / 死亡清理 / 判定 / 无懈窗口。
//!
//! 与技能的接口只有两类：
//! - **钩子**（`skills::dying_hooks` / `skills::on_flip_to_face_up` / `skills::wuxie_options`）；
//! - **事件**（Death / DamageAfter / JudgeEffective …，技能订阅后自行询问）。
//!
//! 本文件**不得**出现任何武将技能名（由 `tests/b0.rs::gate_kernel_has_no_skill_branch` 断言）。

use serde_json::json;

use super::deferred::{Deferred, DeferredKind, Payload};
use super::event::{Event, EventKind};
use super::resolve::{Ask, Frame, Stage, Step};
use super::{CardId, Game, Zone};
use crate::presentation::{POutcome, Presentation};
use crate::skills;

// ---------------------------------------------------------------------------
// 伤害 / 濒死 / 死亡
// ---------------------------------------------------------------------------

/// 统一伤害出口：扣血 → 展示 → 濒死。
pub fn damage(g: &mut Game, source: Option<usize>, victim: usize, amount: i32) {
    if g.seats[victim].hp <= 0 {
        return;
    }
    if crate::skills::damage_prevent(g, victim) {
        return;
    }
    if let Some(s) = source {
        g.bump(&format!("turn.damage_by:{s}"));
    }
    g.seats[victim].hp -= amount;
    g.presentation.push(
        Presentation::new(
            "damage",
            POutcome::Damage {
                amount: amount.max(0),
            },
        )
        .target(victim)
        .fx("damage"),
    );
    let src = source.map(|s| g.seats[s].name.clone()).unwrap_or_default();
    g.push_log(format!(
        "{src} 对 {} 造成 {amount} 点伤害（{}）",
        g.seats[victim].name, g.seats[victim].hp
    ));
    // 「受到伤害后」事件（放逐/反馈/遗计…由此订阅）。
    g.events.push(Event::simple(
        EventKind::DamageAfter,
        Some(victim),
        source.map(|s| s.to_string()).as_deref(),
    ));
    if g.seats[victim].hp <= 0 {
        start_dying(g, victim, source);
    }
}

pub fn start_dying(g: &mut Game, victim: usize, source: Option<usize>) {
    g.stack.push(Frame::Flow(super::resolve::FlowFrame {
        kind: super::resolve::FlowKind::Dying,
        skill: "__dying".to_string(),
        actor: victim,
        stage: Stage::DyingAsk,
        data: json!({ "victim": victim, "source": source, "cursor": 0, "tried": false, "round": 0 }),
    }));
}

/// 濒死询问顺序：自己 → 自己左手侧依次。
fn dying_order(g: &Game, victim: usize) -> Vec<usize> {
    let mut v = vec![victim];
    v.extend(g.seats_from_left_of(victim));
    v.retain(|i| g.seats[*i].hp > 0 || *i == victim);
    v
}

pub fn step_dying(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let victim = a.data["victim"].as_u64().unwrap_or(0) as usize;
    if g.seats[victim].hp > 0 {
        return Ok(Step::Done);
    }
    let tried = a.data["tried"].as_bool().unwrap_or(false);
    let cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;

    // 锁定技濒死钩子（如不屈）：每次濒死尝试一次。
    if !tried && skills::dying_hooks(g, victim) {
        let mut next = a.data.clone();
        next["tried"] = json!(true);
        return Ok(Step::Next(Stage::DyingAsk, next));
    }

    let order = dying_order(g, victim);
    if cursor >= order.len() {
        die(g, victim);
        return Ok(Step::Done);
    }
    let asker = order[cursor];
    let mut b = skills::builder(asker, "dying").skill("dying");
    let mut has = false;
    for id in g.hand(asker) {
        let name = g.cards.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        let tags = crate::cards::tags(&name);
        let usable = tags.tao || (tags.jiu && asker == victim);
        if usable {
            has = true;
            b = b.option(
                format!("respond:{id}"),
                format!("用【{}】救援（{}）", g.card_label(&id), g.seats[victim].name),
            );
        }
    }
    if !has {
        let mut next = a.data.clone();
        next["cursor"] = json!(cursor + 1);
        return Ok(Step::Next(Stage::DyingAsk, next));
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(b.escape("放弃救援").build())),
        Some("pass") => {
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor + 1);
            Ok(Step::Next(Stage::DyingAsk, next))
        }
        Some(act) => {
            let id = act
                .strip_prefix("respond:")
                .ok_or_else(|| format!("濒死：未知选项 {act}"))?
                .to_string();
            g.to_discard(&id, "dying_heal")?;
            g.seats[victim].hp += 1;
            g.presentation.push(
                Presentation::new("dying", POutcome::Heal { amount: 1 })
                    .actor(asker)
                    .target(victim)
                    .cards(vec![id])
                    .fx("dying_heal"),
            );
            g.push_log(format!(
                "{} 救援 {}（体力 {}）",
                g.seats[asker].name, g.seats[victim].name, g.seats[victim].hp
            ));
            // 离开濒死；剩余结算点由栈恢复。
            Ok(Step::Done)
        }
    }
}

/// 死亡：发事件（行殇等订阅）+ 排一个「死亡清理」（在技能触发之后执行）。
pub fn die(g: &mut Game, victim: usize) {
    let name = g.seats[victim].name.clone();
    g.push_log(format!("{name} 死亡"));
    g.events
        .push(Event::simple(EventKind::Death, Some(victim), None));
    g.deferred.push(Deferred {
        kind: DeferredKind::DeathCleanup,
        payload: Payload::Ask {
            skill: "__death_cleanup".to_string(),
            stage: Stage::Choose,
            actor: victim,
            data: json!({ "victim": victim }),
        },
    });
}

/// 死亡清理：剩余牌进弃牌堆（行殇等已在此之前拿走）。
pub fn cleanup_dead(g: &mut Game, victim: usize) -> Result<(), String> {
    let ids: Vec<CardId> = g
        .ids_in(|z| {
            matches!(
                z,
                Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) | Zone::Mark(s, _) if *s == victim
            )
        })
        .into_iter()
        .collect();
    for id in ids {
        g.to_discard(&id, "death_cleanup")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 无懈窗口（解围：装备区牌当【无懈可击】）
// ---------------------------------------------------------------------------

pub fn start_wuxie(g: &mut Game, source: usize, card: CardId) {
    g.stack.push(Frame::Flow(super::resolve::FlowFrame {
        kind: super::resolve::FlowKind::Wuxie,
        skill: "__wuxie".to_string(),
        actor: source,
        stage: Stage::WuxieAsk,
        data: json!({ "source": source, "card": card, "cursor": 0, "count": 0 }),
    }));
}

pub fn step_wuxie(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let source = a.data["source"].as_u64().unwrap_or(0) as usize;
    let card = a.data["card"].as_str().unwrap_or("").to_string();
    let mut cursor = a.data["cursor"].as_u64().unwrap_or(0) as usize;
    let mut count = a.data["count"].as_u64().unwrap_or(0) as usize;
    let order: Vec<usize> = g
        .seats_from_left_of(source)
        .into_iter()
        .filter(|i| g.seats[*i].hp > 0)
        .collect();
    if cursor >= order.len() {
        let outcome = if count % 2 == 1 {
            POutcome::TrickNegated
        } else {
            POutcome::TrickEffective
        };
        g.presentation.push(
            Presentation::new("wuxie", outcome.clone())
                .cards(vec![card.clone()])
                .fx("wuxie_result"),
        );
        g.push_log(format!(
            "无懈结算：共 {count} 张 → {}",
            if count % 2 == 1 {
                "锦囊被抵消"
            } else {
                "锦囊照常生效"
            }
        ));
        return Ok(Step::Done);
    }
    let asker = order[cursor];
    let mut b = skills::builder(asker, "wuxie").skill("wuxie");
    let mut has = false;
    for id in g.hand(asker) {
        if g.cards.get(&id).map(|c| crate::cards::tags(&c.name).wuxie).unwrap_or(false) {
            has = true;
            b = b.option(
                format!("respond:{id}"),
                format!("打出【{}】", g.card_label(&id)),
            );
        }
    }
    for (id, label) in skills::wuxie_options(g, asker) {
        has = true;
        b = b.option(id, label);
    }
    if !has {
        cursor += 1;
        let mut next = a.data.clone();
        next["cursor"] = json!(cursor);
        return Ok(Step::Next(Stage::WuxieAsk, next));
    }
    match g.answer.take().as_deref() {
        None => Ok(Step::Need(b.escape("不使用无懈").build())),
        Some("pass") => {
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor + 1);
            Ok(Step::Next(Stage::WuxieAsk, next))
        }
        Some(act) => {
            if let Some(id) = act.strip_prefix("respond:") {
                g.to_discard(&id.to_string(), "wuxie")?;
            } else if !skills::wuxie_use(g, asker, act)? {
                // 技能提供的无懈替代选项（如解围：装备区牌当无懈）。
                return Err(format!("无懈：未知选项 {act}"));
            }
            count += 1;
            cursor = 0; // 重新从使用者左手侧开始（嵌套）
            let mut next = a.data.clone();
            next["cursor"] = json!(cursor);
            next["count"] = json!(count);
            Ok(Step::Next(Stage::WuxieAsk, next))
        }
    }
}

// ---------------------------------------------------------------------------
// 判定
// ---------------------------------------------------------------------------

/// 亮出牌堆顶一张牌作为判定牌（进入处理区），发 `JudgeRevealed`。
pub fn judge_card(g: &mut Game, owner: usize) -> Option<CardId> {
    let id = g.draw_top()?;
    g.to_frame(&id).ok()?;
    g.events.push(Event::card(
        EventKind::JudgeRevealed,
        Some(owner),
        id.clone(),
        None,
    ));
    Some(id)
}

/// 判定生效（发 `JudgeEffective`；颂威等订阅）。
pub fn judge_effective(g: &mut Game, owner: usize, card: &CardId) {
    g.events.push(Event::card(
        EventKind::JudgeEffective,
        Some(owner),
        card.clone(),
        None,
    ));
}

/// 判定牌落盘（生效后进弃牌堆）。
pub fn judge_to_discard(g: &mut Game, card: &CardId) -> Result<(), String> {
    g.to_discard(card, "judge")
}
