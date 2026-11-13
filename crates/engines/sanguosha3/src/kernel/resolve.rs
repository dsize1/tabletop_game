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
    JudgeResolve,
    QuickJudge,
}

/// 内核流程帧类型（牌结算 / 濒死 / 无懈 / 拼点 / 判定 / 弃牌）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowKind {
    Dying,
    Wuxie,
    Card,
    Judge,
    Pindian,
    QuickJudge,
    Discard,
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
    /// 切阶段并立刻询问（避免「答完又走同一段」）。
    NeedAt(Stage, Pending),
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
            f.set_stage_data(stage, serde_json::Value::Null);
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
