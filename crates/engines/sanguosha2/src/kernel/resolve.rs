//! 结算栈 + 推进器。
//!
//! 不变量（plan §2.1）：**技能不得就地压/弹帧**。技能 handler 只返回 `Step`，
//! 由 `advance` 统一改栈 → 「父帧 pop 把子帧弹掉」类缺陷在结构上不可能发生。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::deferred::{Deferred, Payload};
use super::event::{EventKind, Window};
use super::pending::Pending;
use super::{CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};
use crate::skills;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// 通用：是/否/选项询问。
    Choose,
    /// 选择目标或候选牌。
    Target,
    /// 良姻·弃置：发动者弃 1。
    DiscardOwner,
    /// 良姻·弃置：目标弃 1。
    DiscardTarget,
    /// 良姻·回血判定。
    Heal,
    /// 箜声·准备阶段置牌。
    PreparePick,
    /// 箜声·结束阶段取回/归还装备。
    EndReturn,
    /// 箜声·使用剩余装备。
    UseEquip,
    /// 贞良（阴）·选择摸牌目标。
    YinTarget,
    /// 贞良（阳）·选择目标。
    YangTarget,
    /// 贞良（阳）·弃同色牌。
    YangCard,
    /// 濒死流程（内核）。
    DyingAsk,
    /// 无懈窗口（内核）。
    WuxieAsk,
    /// 翻面结算完（解围②等子流程返回后继续）。
    AfterFlip,
    /// 濒死结算完（奋激：脱困后才摸牌）。
    AfterDying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowKind {
    Dying,
    Wuxie,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFrame {
    pub kind: FlowKind,
    pub skill: String,
    pub actor: usize,
    pub stage: Stage,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub skill: String,
    pub stage: Stage,
    pub actor: usize,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Ask(Ask),
    /// 内核流程帧（濒死 / 无懈）：handler 在 `kernel::flow`，与技能无关。
    Flow(FlowFrame),
    /// 伤害后钩子（压在濒死之下，救起后仍会执行）。
    DamageAfter { victim: usize, source: Option<usize> },
}

#[derive(Debug)]
pub enum Step {
    Next(Stage, Value),
    Done,
    Need(Pending),
    /// 切换阶段并立刻询问（`Need` 不会改阶段，容易导致「答完又走同一段」）。
    NeedAt(Stage, Pending),
    Finish(String),
}

pub fn push_ask(
    g: &mut Game,
    skill: impl Into<String>,
    stage: Stage,
    actor: usize,
    data: Value,
) {
    g.stack.push(Frame::Ask(Ask {
        skill: skill.into(),
        stage,
        actor,
        data,
    }));
}

/// 推进到「需要输入」或「无事可做」。
pub fn advance(g: &mut Game) -> Result<(), String> {
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 2000 {
            return Err("结算推进次数异常（疑似死循环）".into());
        }
        if g.pending.is_some() || g.finished {
            return Ok(());
        }
        if !g.stack.is_empty() {
            // 记录当前帧索引：handler 可能压入子帧（如翻面触发解围），
            // Step::Next/Done 必须作用在**发起帧**上，而不是栈顶。
            let idx = g.stack.len() - 1;
            let frame = g.stack[idx].clone();
            let step = match frame {
                Frame::Ask(a) => {
                    let h = skills::handler(&a.skill)
                        .ok_or_else(|| format!("未知技能处理器：{}", a.skill))?;
                    h(g, &a)?
                }
                Frame::Flow(f) => match f.kind {
                    FlowKind::Dying => super::flow::step_dying(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Wuxie => super::flow::step_wuxie(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                },
                Frame::DamageAfter { victim, source } => {
                    g.stack.pop();
                    let ev = super::event::Event::simple(
                        EventKind::DamageAfter,
                        source,
                        Some(&format!("victim={victim}")),
                    );
                    g.events.push(ev);
                    Step::Done
                }
            };
            match step {
                Step::Next(stage, data) => {
                    match &mut g.stack[idx] {
                        Frame::Ask(ask) => {
                            ask.stage = stage;
                            ask.data = data;
                        }
                        Frame::Flow(f) => {
                            f.stage = stage;
                            f.data = data;
                        }
                        _ => return Err("栈顶不可替换阶段".into()),
                    }
                    continue;
                }
                Step::Done => {
                    g.stack.remove(idx);
                    continue;
                }
                Step::Need(p) => {
                    g.pending = Some(p);
                    return Ok(());
                }
                Step::NeedAt(stage, p) => {
                    match &mut g.stack[idx] {
                        Frame::Ask(ask) => ask.stage = stage,
                        Frame::Flow(f) => f.stage = stage,
                        _ => return Err("栈顶不可替换阶段".into()),
                    }
                    g.pending = Some(p);
                    return Ok(());
                }
                Step::Finish(msg) => {
                    g.stack.remove(idx);
                    g.push_log(msg);
                    continue;
                }
            }
        }
        // 事件先于延迟队列：事件是「刚发生的事实」，延迟队列是「事实之后要补的询问」。
        // 否则死亡清理会跑在行殇之前（B0 实测）。
        if !g.events.is_empty() {
            // 镇骨的「第二个时刻」：被选角色回合结束时结算（框架层，不认识技能细节）
            let ended = g.events[0]
                .kind
                == crate::kernel::event::EventKind::PhaseEnd
                && g.events[0].cause.as_deref() == Some("End");
            if ended {
                if let Some(seat) = g.events[0].seat {
                    crate::skills::on_turn_end_for_scheduled(g, seat);
                }
            }
            route_one_event(g);
            continue;
        }
        // 单一延迟队列：按显式顺序表取最早的一项。
        if let Some(idx) = next_deferred_index(g) {
            let d = g.deferred.remove(idx);
            run_deferred(g, d)?;
            continue;
        }
        break;
    }
    Ok(())
}

fn next_deferred_index(g: &Game) -> Option<usize> {
    g.deferred
        .iter()
        .enumerate()
        .min_by_key(|(i, d)| (d.order_index(), *i))
        .map(|(i, _)| i)
}

fn run_deferred(g: &mut Game, d: Deferred) -> Result<(), String> {
    match d.payload {
        Payload::Trigger {
            owner,
            skill,
            event,
            seat,
            card,
            cause,
        } => {
            let mut data = json!({ "cause": cause, "event": event, "seat": seat });
            if let Some(card) = card {
                data["card"] = json!(card);
            }
            push_ask(g, skill, Stage::Choose, owner, data);
            Ok(())
        }
        Payload::Ask {
            skill,
            stage,
            actor,
            data,
        } => {
            if skill == "__death_cleanup" {
                let victim = data["victim"].as_u64().unwrap_or(0) as usize;
                super::flow::cleanup_dead(g, victim)?;
                return Ok(());
            }
            push_ask(g, skill, stage, actor, data);
            Ok(())
        }
        Payload::Land { card, to } => {
            // 帧暂存牌落盘：唯一出口。
            if let Some(frame) = g.zone_of(&card).cloned() {
                if frame.is_frame() {
                    let mut tmp = json!({});
                    let _ = &mut tmp;
                    // 通过 from_void/gain 之外的低层落盘需要 kernel 内部 API：
                    // 这里用专用 helper（Land 是框架层，不属于任何技能）。
                    g.land_card(&card, to)?;
                }
            }
            Ok(())
        }
    }
}

fn route_one_event(g: &mut Game) {
    if g.events.is_empty() {
        return;
    }
    let ev = g.events.remove(0);
    let mut matched: Vec<super::event::Sub> = g
        .subs
        .iter()
        .filter(|s| s.accepts(&ev, g.turn, g.phase))
        .cloned()
        .collect();
    super::event::sort_subs(&mut matched, g.turn, g.seats.len());
    for s in matched {
        g.deferred.push(Deferred {
            kind: super::deferred::DeferredKind::SkillTrigger,
            payload: Payload::Trigger {
                owner: s.owner,
                skill: s.skill,
                event: format!("{:?}|side={}", ev.kind, s.side),
                seat: ev.seat,
                card: ev.card.clone(),
                cause: ev.cause.clone(),
            },
        });
    }
}

/// 提交一个动作（当前 pending 的回答）。返回执行后的结果文案。
pub fn submit(g: &mut Game, actor: usize, action: &str) -> Result<String, String> {
    let Some(p) = g.pending.clone() else {
        return Err("当前没有待办".into());
    };
    if p.actor() != actor {
        return Err("当前等待其他玩家响应".into());
    }
    if !p.has_option(action) {
        return Err(format!("非法选项：{action}"));
    }
    g.pending = None;
    g.answer = Some(action.to_string());
    if p.escape_id() == action {
        let sk = p.skill().unwrap_or("pending").to_string();
        g.presentation.push(
            Presentation::new(sk.clone(), POutcome::Declined { skill: sk }).fx("pending_cancel"),
        );
    }
    advance(g)?;
    Ok(g.log.last().map(|e| e.text.clone()).unwrap_or_default())
}

/// 回合/阶段切换（测试与引擎共用）：清一次性计数器 + 发事件。
pub fn begin_phase(g: &mut Game, seat: usize, phase: Phase) -> Result<(), String> {
    if g.turn != seat {
        for i in 0..g.seats.len() {
            g.unfreeze_hand(i);
        }
        g.clear_prefix("turn.");
        g.turn = seat;
        g.events.push(super::event::Event::simple(
            EventKind::TurnStart,
            Some(seat),
            None,
        ));
    }
    if g.phase != phase {
        g.phase = phase;
        g.clear_prefix("phase.");
        g.events.push(super::event::Event::simple(
            EventKind::PhaseStart,
            Some(seat),
            Some(&format!("{phase:?}")),
        ));
    }
    advance(g)
}

/// 供 kernel 内部落盘使用（Land 延迟项）。
pub fn land(g: &mut Game, card: &CardId, to: Zone) -> Result<(), String> {
    g.land_card(card, to)
}

/// 便捷：把窗口断言写进测试。
pub fn window_allows(w: Window, turn: usize, phase: Phase, owner: usize) -> bool {
    w.allows(turn, phase, owner)
}
