//! 事件 + 生效窗口 + 主体（subject）。
//!
//! 两条不变量（v2 踩坑后固化）：
//! 1. **每个订阅都必须声明窗口**（`Window`）；`AnyTime` 仅用于「无归属」的全局事实，
//!    且技能仍需按需自过滤。教训（良姻）：全局事件 + 未定义窗口 = 误触发。
//! 2. **每个订阅都必须声明主体**（`Subject`）；这是 M105 教训（屯田/燕语在「他人失去牌」时
//!    误触发）的类型级对策——事件归属是**声明**，不是 handler 首行守卫。

use serde::{Deserialize, Serialize};

use super::{CardId, Phase};

/// 事件所属生命周期：`Setup` 阶段的系统事件（发牌/组牌堆）不触发技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Lifecycle {
    #[default]
    Turn,
    Setup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    CardOutOfGame,
    CardIntoGame,
    CardGained,
    CardDiscarded,
    /// 失去牌：牌离开**手牌或装备区**（含被获得/弃置/使用/打出）。与 `CardDiscarded` 区别：
    /// 弃置只是失去的一种原因；与 `CardGained` 区别：获得只说明「谁拿到」，不说明「谁失去」。
    CardLost,
    CardUsed,
    CardPlayed,
    /// 指定目标（source=使用者，targets 见 cause）。
    TargetChosen,
    /// 受到伤害后（**不是**失去体力，见 `damage-vs-hp-loss`）。
    DamageAfter,
    /// 失去（流失）体力：不是伤害。
    HpLost,
    /// 回复体力。
    Healed,
    /// 体力上限变化。
    MaxHpChanged,
    /// 进入连环状态。
    ChainedEnter,
    /// 手牌冻结 / 解冻。
    HandFrozen,
    HandUnfrozen,
    /// 牌被公开（展示）。
    CardRevealed,
    CardConcealed,
    /// 进入 / 离开判定区（离开**不算弃置**）。
    JudgeZoneEntered,
    JudgeZoneLeft,
    Death,
    JudgeRevealed,
    JudgeEffective,
    /// 护甲变化。
    ArmorChanged,
    TurnStart,
    PhaseStart,
    PhaseEnd,
    TurnEnd,
}

/// 事件快照：**窗口与主体判定只读快照，不读 live 状态**（路由可能晚于状态推进）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    /// 事件主体（谁的手牌被弃置 / 谁使用牌 / 谁受到伤害）。
    pub seat: Option<usize>,
    pub card: Option<CardId>,
    pub cause: Option<String>,
    #[serde(default)]
    pub amount: Option<i32>,
    /// 事件发生时刻的回合座位。
    #[serde(default)]
    pub turn: usize,
    /// 事件发生时刻的阶段。
    #[serde(default)]
    pub phase: Phase,
    /// 生命周期。
    #[serde(default)]
    pub lifecycle: Lifecycle,
    /// 施动者（谁触发这次移动：弃置者/使用者/获得者）。
    #[serde(default)]
    pub by: Option<usize>,
    /// 来源座位（这张牌此前属于谁；牌堆/弃牌堆为 None）。
    #[serde(default)]
    pub from: Option<usize>,
}

impl Event {
    pub fn card(kind: EventKind, seat: Option<usize>, card: CardId, cause: Option<String>) -> Self {
        Self {
            kind,
            seat,
            card: Some(card),
            cause,
            amount: None,
            turn: 0,
            phase: Phase::Prepare,
            lifecycle: Lifecycle::Turn,
            by: None,
            from: None,
        }
    }

    pub fn simple(kind: EventKind, seat: Option<usize>, cause: Option<&str>) -> Self {
        Self {
            kind,
            seat,
            card: None,
            cause: cause.map(|s| s.to_string()),
            amount: None,
            turn: 0,
            phase: Phase::Prepare,
            lifecycle: Lifecycle::Turn,
            by: None,
            from: None,
        }
    }
}

/// 生效窗口：订阅方声明「我在什么窗口内响应」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Window {
    /// 任意时刻（无归属的全局事实；技能仍需自过滤）。
    AnyTime,
    /// 仅自己的回合内。
    OwnTurn,
    /// 仅自己的回合内的指定阶段。
    OwnPhase(Phase),
    /// 仅自己的回合外。
    OtherTurn,
    /// 仅他人回合内的指定阶段（樵拾：其他角色结束阶段；献图：其他角色出牌阶段）。
    OtherPhase(Phase),
}

impl Window {
    pub fn allows(self, turn: usize, phase: Phase, owner: usize) -> bool {
        match self {
            Window::AnyTime => true,
            Window::OwnTurn => turn == owner,
            Window::OwnPhase(p) => turn == owner && phase == p,
            Window::OtherTurn => turn != owner,
            Window::OtherPhase(p) => turn != owner && phase == p,
        }
    }
}

/// 事件主体声明（M105 教训的类型级对策）。
///
/// 订阅必须声明「事件里的**谁**与我有关」，handler 首行按此校验，
/// 否则「他人失去牌」也会触发「自己失去牌才应触发」的技能（屯田/燕语/奋激同源缺陷）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subject {
    /// 事件主体是持有者自己（"当你失去牌"）。
    Self_,
    /// 事件主体是其他角色（"当其他角色失去牌"）。
    Other,
    /// 事件的施动者（`by`）是持有者。
    By,
    /// 事件的来源（`from`）是持有者。
    From,
    /// 无座位主体（回合/阶段类事件，`seat=None`）：主体由 `Window`（turn/phase）决定，这里恒真。
    Any,
}

/// 订阅（由技能注册表派生，禁止手写清单）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sub {
    pub owner: usize,
    pub skill: String,
    pub kind: EventKind,
    pub window: Window,
    /// 事件主体声明（V3 新增，v2 缺失）。
    pub subject: Subject,
    pub priority: i32,
    /// 注册顺序（同优先级、同座位时的稳定排序键）。
    pub seq: usize,
    /// 技能面：0 = 技能自身窗口，1 = 阳面窗口（face 化，转换技）。
    #[serde(default)]
    pub side: u8,
}

impl Sub {
    /// 窗口判定**只读事件快照**。
    pub fn window_accepts(&self, ev: &Event) -> bool {
        self.kind == ev.kind && self.window.allows(ev.turn, ev.phase, self.owner)
    }

    /// 主体判定：事件里的「谁」是否匹配我声明的主体（handler 首行必须过这里）。
    pub fn subject_matches(&self, ev: &Event) -> bool {
        match self.subject {
            Subject::Self_ => ev.seat == Some(self.owner),
            Subject::Other => ev.seat.is_some() && ev.seat != Some(self.owner),
            Subject::By => ev.by == Some(self.owner),
            Subject::From => ev.from == Some(self.owner),
            Subject::Any => true,
        }
    }

    /// 完整接受判定：窗口 + 主体都匹配才路由给该订阅。
    pub fn accepts(&self, ev: &Event) -> bool {
        self.window_accepts(ev) && self.subject_matches(ev)
    }
}

/// 确定性排序：优先级小先 → 距当前回合玩家近先 → 注册序小先。
pub fn sort_subs(subs: &mut [Sub], turn: usize, seats: usize) {
    subs.sort_by_key(|s| {
        let dist = if s.owner >= turn {
            s.owner - turn
        } else {
            seats - turn + s.owner
        };
        (s.priority, dist, s.seq)
    });
}
