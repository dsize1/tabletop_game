//! 结算栈 + 推进器 + **发起帧归属**。
//!
//! 不变量（spike §2 B 类 / constitution 硬约束 B）：
//! 1. **技能不得就地压/弹帧**：handler 只返回 `Step`，栈只由 `advance` 改；
//! 2. **进度数据只写发起帧**：`Step::Next/NeedAt` 用**唯一帧 id** 定位发起帧，
//!    不是 `stack.last_mut()`（栈顶可能是 handler 刚压入的子帧，如濒死）。
//!    v2 用「位置 idx」记录发起帧，V3 改为「唯一帧 id」，子帧压/弹不再影响归属。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::pending::Pending;

/// 通用交互阶段（**不含技能名**；技能多阶段顺序由技能 `steps[]` 表达，不进内核）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// 二选一 / 选项询问。
    Choose,
    /// 选目标。
    Target,
    /// 选自己的牌（手牌/装备区）。
    PickOwn,
    /// 选别人的牌。
    PickOther,
    /// 多选 + 确认/取消。
    MultiSelect,
    /// 多模式选一。
    Mode,
    /// 确认 / 取消（二阶段目标确认）。
    Confirm,
    // —— 内核流程阶段（牌结算 / 濒死 / 无懈 / 拼点）——
    CardAsk,
    CardPick,
    CardOpt,
    CardTarget,
    DyingAsk,
    WuxieAsk,
    PindianInit,
    PindianResp,
    PindianReveal,
    JudgeResolve,
    QuickJudge,
    // —— P1/P3 回合驱动与出牌/弃牌交互 ——
    /// 回合流程帧的阶段推进。
    PhaseStep,
    /// 出牌阶段行动选择（出牌/技能/结束）。
    PlayAct,
    /// 弃牌阶段逐张弃置。
    DiscardPick,
    /// 牌结算的「效果结算」阶段（P4）。
    CardResolve,
}

/// 内核流程帧类型（牌结算 / 濒死 / 无懈 / 拼点 / 判定 / 弃牌）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowKind {
    /// 回合流程（P1：阶段机）。
    Turn,
    Dying,
    Wuxie,
    Card,
    Judge,
    Pindian,
    Guhuo,
    QuickJudge,
    Discard,
    /// 判定阶段（延时锦囊 LIFO；每张牌走 `quick_judge` 子帧）。
    JudgeZone,
    /// 代出/代响应接力（激将/勤王）。
    Relay,
}

/// 技能帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub skill: String,
    pub stage: Stage,
    pub actor: usize,
    pub data: Value,
}

/// 流程帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFrame {
    pub kind: FlowKind,
    pub actor: usize,
    pub stage: Stage,
    pub data: Value,
}

/// 结算帧：唯一 `id` 是发起帧归属的锚点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub id: u64,
    pub inner: FrameInner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameInner {
    Ask(Ask),
    Flow(FlowFrame),
}

impl Frame {
    pub fn new(id: u64, inner: FrameInner) -> Self {
        Self { id, inner }
    }

    /// 只切阶段（保留 data）：`Step::NeedAt` 用（询问后 handler 仍需读发起帧数据）。
    pub fn set_stage(&mut self, stage: Stage) {
        match &mut self.inner {
            FrameInner::Ask(a) => a.stage = stage,
            FrameInner::Flow(f) => f.stage = stage,
        }
    }

    pub fn set_stage_data(&mut self, stage: Stage, data: Value) {
        match &mut self.inner {
            FrameInner::Ask(a) => {
                a.stage = stage;
                a.data = data;
            }
            FrameInner::Flow(f) => {
                f.stage = stage;
                f.data = data;
            }
        }
    }
}

/// handler 返回的下一步指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// 本帧结束。
    Done,
    /// 进入下一阶段（写回**发起帧**的 data）。
    Next(Stage, Value),
    /// 挂起，等待玩家输入（不改阶段）。
    Need(Pending),
    /// 切阶段并立刻询问（避免「答完又走同一段」）。**保留**发起帧原有 data。
    NeedAt(Stage, Pending),
    /// 切阶段 + **写回 data** + 挂起询问：handler 需要把中间状态（已选目标/已亮出的牌）
    /// 存进发起帧数据时必须用它（`Need`/`NeedAt` 都不改 data，本地 Map 改动会被丢弃）。
    NeedWith(Stage, Value, Pending),
    /// 结束 + 播报文案。
    Finish(String),
}

/// 压入一个技能帧（受控 API；handler 通过它压子帧，但**不直接动栈**）。
pub fn push_ask(g: &mut super::Game, skill: impl Into<String>, stage: Stage, actor: usize, data: Value) {
    let id = g.next_frame_id;
    g.next_frame_id += 1;
    g.stack.push(Frame::new(
        id,
        FrameInner::Ask(Ask {
            skill: skill.into(),
            stage,
            actor,
            data,
        }),
    ));
}

/// 压入一个流程帧。
pub fn push_flow(g: &mut super::Game, kind: FlowKind, actor: usize, stage: Stage, data: Value) {
    let id = g.next_frame_id;
    g.next_frame_id += 1;
    g.stack.push(Frame::new(
        id,
        FrameInner::Flow(FlowFrame {
            kind,
            actor,
            stage,
            data,
        }),
    ));
}

/// 应用 Step 到指定 `frame_id` 的帧（发起帧归属的**唯一**落点）。
///
/// 不变量：`frame_id` 是 handler 运行前记录的发起帧 id；handler 压入的子帧不改变归属。
pub fn apply_step(g: &mut super::Game, frame_id: u64, step: Step) -> Result<(), String> {
    // 找到并移除发起帧；不存在则报错（Done 在帧已消失时是 bug，不是 no-op）。
    fn remove_frame(g: &mut super::Game, frame_id: u64) -> Result<(), String> {
        let idx = g
            .stack
            .iter()
            .position(|f| f.id == frame_id)
            .ok_or_else(|| format!("帧 {frame_id} 已不存在"))?;
        g.stack.remove(idx);
        Ok(())
    }
    match step {
        Step::Next(stage, data) => {
            let f = g
                .stack
                .iter_mut()
                .find(|f| f.id == frame_id)
                .ok_or_else(|| format!("帧 {frame_id} 已不存在"))?;
            f.set_stage_data(stage, data);
        }
        Step::Done => {
            remove_frame(g, frame_id)?;
        }
        Step::Need(p) => {
            g.pending = Some(p);
        }
        Step::NeedAt(stage, p) => {
            let f = g
                .stack
                .iter_mut()
                .find(|f| f.id == frame_id)
                .ok_or_else(|| format!("帧 {frame_id} 已不存在"))?;
            // 只切阶段，**不丢 data**：询问回复后 handler 仍按同一份数据继续。
            f.set_stage(stage);
            g.pending = Some(p);
        }
        Step::NeedWith(stage, data, p) => {
            let f = g
                .stack
                .iter_mut()
                .find(|f| f.id == frame_id)
                .ok_or_else(|| format!("帧 {frame_id} 已不存在"))?;
            f.set_stage_data(stage, data);
            g.pending = Some(p);
        }
        Step::Finish(_msg) => {
            remove_frame(g, frame_id)?;
        }
    }
    Ok(())
}

/// 取延迟队列里顺序最早的一项。
pub fn next_deferred_index(g: &super::Game) -> Option<usize> {
    (0..g.deferred.len()).min_by_key(|&i| g.deferred[i].order_index())
}

/// 取延迟队列里**当前可派发**的最早一项（`TurnEndCheck` 未到结束阶段时不可派发）。
pub fn next_ready_deferred_index(g: &super::Game) -> Option<usize> {
    (0..g.deferred.len())
        .filter(|&i| g.deferred[i].ready(g))
        .min_by_key(|&i| g.deferred[i].order_index())
}

/// 玩家提交待办选项（**唯一入口**）：校验 actor / 合法选项 / 强制选择不出出口，
/// 然后写入 `answer`（归属 = 发起待办的技能）并推进结算栈。
///
/// 不变量（constitution I1）：提交非法选项/他人待办/强制选择的占位出口 → 报错且**不改状态**。
pub fn submit(g: &mut super::Game, seat: usize, option: &str) -> Result<(), String> {
    let Some(p) = g.pending.as_ref() else {
        return Err("当前没有待办".into());
    };
    if p.actor() != seat {
        return Err(format!("待办不属于座位 {seat}"));
    }
    if !p.has_option(option) {
        return Err(format!("非法选项：{option}"));
    }
    if p.escape_hidden() && p.escape_id() == option {
        return Err("强制选择不接受出口".into());
    }
    let skill = p.skill().map(|s| s.to_string());
    g.pending = None;
    g.answer = Some(option.to_string());
    g.answer_skill = skill;
    advance(g)
}

/// 消费回答：只有「发起当前待办的技能」能取走答案（防止跨技能误取）。
pub fn take_answer(g: &mut super::Game, skill: &str) -> Option<String> {
    if g.answer_skill.as_deref() == Some(skill) {
        let a = g.answer.take();
        g.answer_skill = None;
        a
    } else {
        None
    }
}

/// 推进器：栈循环 → 事件路由 → 延迟队列，直到「需输入」或「无事可做」。
///
/// 不变量：
/// 1. 栈只由本函数改（handler 只返回 Step，`apply_step` 是唯一落点）；
/// 2. 发起帧归属：`frame_id` 在 handler 运行前记录，Step 作用于它（不是栈顶）；
/// 3. 事件先于延迟队列（事件是「刚发生的事实」，延迟队列是「事实之后要补的询问」）。
pub fn advance(g: &mut super::Game) -> Result<(), String> {
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 2000 {
            return Err("结算推进次数异常（疑似死循环）".into());
        }
        // 1. 有 pending → 等待玩家输入。
        if g.pending.is_some() {
            return Ok(());
        }
        // 2. 事件路由：栈空时；或栈顶是**回合流程帧**时（本次动作已结算完，回到回合帧
        //    即按订阅派发，不必等整局无帧——否则常驻的回合帧会让技能永不触发）。
        //    濒死/牌结算等其他帧在栈顶时不路由（濒死中断要先把救援结算完）。
        let top_is_turn = g
            .stack
            .last()
            .map(|f| {
                matches!(&f.inner, FrameInner::Flow(ff) if ff.kind == crate::kernel::resolve::FlowKind::Turn)
            })
            .unwrap_or(true);
        if top_is_turn && !g.events.is_empty() {
            let ev = g.events.remove(0);
            crate::kernel::skills::route_event(g, &ev);
            continue;
        }
        // 3. 栈非空 → 跑栈顶帧。
        if let Some(idx) = g.stack.len().checked_sub(1) {
            let frame_id = g.stack[idx].id;
            let frame = g.stack[idx].clone();
            let step = match &frame.inner {
                FrameInner::Ask(a) => {
                    let h = crate::kernel::skills::handler(&a.skill)
                        .ok_or_else(|| format!("未知技能处理器：{}", a.skill))?;
                    h(g, a)?
                }
                FrameInner::Flow(f) => {
                    crate::kernel::card_play::step_flow(g, f.kind, &f.data, f.actor)?
                }
            };
            apply_step(g, frame_id, step)?;
            continue;
        }
        // 4. 延迟队列：按顺序表派发（`TurnEndCheck` 只在当前回合结束阶段派发；陈旧项丢弃）。
        let turn = g.turn;
        g.deferred.retain(|d| {
            !(matches!(d.kind, crate::kernel::deferred::DeferredKind::TurnEndCheck)
                && d.turn_of().map(|t| t < turn).unwrap_or(true))
        });
        if let Some(i) = next_ready_deferred_index(g) {
            let d = g.deferred.remove(i);
            if let crate::kernel::deferred::Payload::Ask { skill, stage, actor, data } = d.payload {
                push_ask(g, skill, stage, actor, data);
            }
            continue;
        }
        break;
    }
    Ok(())
}
