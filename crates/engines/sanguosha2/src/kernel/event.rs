//! 事件 + 订阅 + 生效窗口。
//!
//! 不变量：**每个订阅都必须声明窗口**；`Window::AnyTime` 仅允许用于「无归属」的
//! 全局事实（如牌移出游戏），且技能仍要在自己的 handler 里按需过滤。
//! 教训（良姻）：全局事件 + 未定义窗口 = 误触发。

use serde::{Deserialize, Serialize};

use super::CardId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    CardOutOfGame,
    CardIntoGame,
    CardGained,
    CardDiscarded,
    /// 失去牌：牌离开**手牌或装备区**（含被获得/被弃置/使用/打出）。
    ///
    /// 与 `CardDiscarded` 的区别：弃置只是失去的一种原因；与 `CardGained` 的区别：
    /// 获得只说明「谁拿到了」，不说明「谁失去了」（屯田需要后者）。
    CardLost,
    CardUsed,
    /// 指定目标（source=使用者，card，targets 见 cause 或 seat 列表）。
    TargetChosen,
    CardPlayed,
    DamageAfter,
    /// 失去（流失）体力：**不是**伤害（mechanics `damage-vs-hp-loss`）。
    HpLost,
    /// 回复体力（桃/桃园/白银狮子/技能回血）；不产生伤害事件。
    Healed,
    /// 体力上限变化（自立觉醒 / 减上限类效果）。
    MaxHpChanged,
    /// 进入连环状态（`chained_enter`）：**所有**进入连环状态的对象都发（用户 2026-11-06 裁定）。
    ChainedEnter,
    /// 手牌冻结 / 解冻（mechanics `hand-freeze`；cause 带到期窗口）。
    HandFrozen,
    HandUnfrozen,
    /// 牌被公开（展示）：**立即**进入公共可见区（mechanics `reveal`）。
    CardRevealed,
    /// 牌结束公开。
    CardConcealed,
    /// 进入判定区（mechanics `judge-zone`）。
    JudgeZoneEntered,
    /// 离开判定区（**不算弃置**；用户 2026-11-06 裁定）。
    JudgeZoneLeft,
    Death,
    JudgeRevealed,
    JudgeEffective,
    /// 护甲变化（mechanics `armor-points`；cause = gain / lose / absorb）。
    ArmorChanged,
    TurnStart,
    PhaseStart,
    PhaseEnd,
    TurnEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    /// 事件主体（谁的手牌被弃置 / 谁使用牌）。
    pub seat: Option<usize>,
    pub card: Option<CardId>,
    /// 附加说明（弃置原因、阶段名等）。
    pub cause: Option<String>,
    /// 数值（伤害点数 / 失去体力数 / 上限变化量）。
    #[serde(default)]
    pub amount: Option<i32>,
}

impl Event {
    pub fn card(
        kind: EventKind,
        seat: Option<usize>,
        card: CardId,
        cause: Option<String>,
    ) -> Self {
        Self {
            kind,
            seat,
            card: Some(card),
            cause,
            amount: None,
        }
    }

    pub fn simple(kind: EventKind, seat: Option<usize>, cause: Option<&str>) -> Self {
        Self {
            kind,
            seat,
            card: None,
            cause: cause.map(|s| s.to_string()),
            amount: None,
        }
    }

    pub fn with_amount(kind: EventKind, seat: Option<usize>, cause: Option<&str>, amount: i32) -> Self {
        let mut ev = Self::simple(kind, seat, cause);
        ev.amount = Some(amount);
        ev
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
    OwnPhase(super::Phase),
    /// 仅自己的回合外（回合外技能）。
    OtherTurn,
}

impl Window {
    pub fn allows(self, turn: usize, phase: super::Phase, owner: usize) -> bool {
        match self {
            Window::AnyTime => true,
            Window::OwnTurn => turn == owner,
            Window::OwnPhase(p) => turn == owner && phase == p,
            Window::OtherTurn => turn != owner,
        }
    }
}

/// 订阅（由技能注册表派生，禁止手写清单）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sub {
    pub owner: usize,
    pub skill: String,
    pub kind: EventKind,
    pub window: Window,
    pub priority: i32,
    /// 注册顺序（同优先级、同座位时的稳定排序键）。
    pub seq: usize,
    /// 转换技面：0 = 技能自身窗口，1 = 阳面窗口。
    #[serde(default)]
    pub side: u8,
}

impl Sub {
    pub fn accepts(&self, ev: &Event, turn: usize, phase: super::Phase) -> bool {
        self.kind == ev.kind && self.window.allows(turn, phase, self.owner)
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
