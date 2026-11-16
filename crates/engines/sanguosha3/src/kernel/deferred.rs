//! 单一延迟队列 + **显式顺序表**（mechanics `deferred-timing`）。
//!
//! v2 病灶：旧引擎 6 条 per-skill 队列，顺序靠代码位置隐式决定；且 v2 的 `DeferredKind`
//! 混入技能名（LiangYinHeal / KongShengUse）。V3 只保留**通用** kind，顺序由 `ORDER` 常量
//! 决定（常量即文档，可被测试断言）。技能内部的多阶段顺序由技能自己的 `steps[]` 表达，
//! 不进内核延迟队列。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CardId, Phase, Zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeferredKind {
    /// 帧落盘：牌从 `Zone::Frame` 落盘到目标区（守恒；必须先于技能触发）。
    Land,
    /// 技能触发询问（栈空后逐个问）。
    SkillTrigger,
    /// 判定阶段：延时锦囊 LIFO 结算（排在技能触发之后）。
    JudgePhase,
    /// **回合结束校验**（审时阴面「其未失去此牌」等）：只在「当前回合的结束阶段」派发，
    /// 未到结束阶段时**保留在队列**（不提前派发），跨回合后作为陈旧项清理。
    TurnEndCheck,
    /// 死亡清理（排在所有死亡时技能之后）。
    DeathCleanup,
}

/// **顺序表**（常量即文档）。改动必须同步 `docs/games/sanguosha/align/mechanics.json`。
pub const ORDER: [DeferredKind; 5] = [
    DeferredKind::Land,
    DeferredKind::SkillTrigger,
    DeferredKind::JudgePhase,
    DeferredKind::TurnEndCheck,
    DeferredKind::DeathCleanup,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    /// 技能触发：事件快照（owner + 事件类型 + seat/by/from + 数值）。
    Trigger {
        owner: usize,
        skill: String,
        event: String,
        seat: Option<usize>,
        card: Option<CardId>,
        cause: Option<String>,
        #[serde(default)]
        amount: Option<i32>,
        #[serde(default)]
        turn: usize,
        #[serde(default)]
        phase: Phase,
        #[serde(default)]
        by: Option<usize>,
        #[serde(default)]
        from: Option<usize>,
    },
    /// 通用「待栈空后询问」：技能名/阶段/数据由调用方给定，内核不认识具体技能。
    Ask {
        skill: String,
        stage: crate::kernel::resolve::Stage,
        actor: usize,
        data: Value,
    },
    /// 帧落盘（防牌滞留 Frame 区）。
    Land {
        card: CardId,
        to: Zone,
    },
    /// 判定阶段：对某座位的延时锦囊发起 LIFO 结算。
    JudgePhase {
        seat: usize,
    },
    /// **死亡清理**（在死亡时技能/死亡弃牌订阅都结算完之后）：
    /// 手牌+装备进弃牌堆 → `DeathDiscarded`（行殇订阅）→ 判定区清理 → 奖惩/胜负。
    DeathCleanup {
        dead: usize,
        killer: Option<usize>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deferred {
    pub kind: DeferredKind,
    pub payload: Payload,
}

impl Deferred {
    pub fn ask(
        kind: DeferredKind,
        skill: impl Into<String>,
        stage: crate::kernel::resolve::Stage,
        actor: usize,
        data: Value,
    ) -> Self {
        Self {
            kind,
            payload: Payload::Ask {
                skill: skill.into(),
                stage,
                actor,
                data,
            },
        }
    }

    pub fn order_index(&self) -> usize {
        ORDER
            .iter()
            .position(|k| *k == self.kind)
            .unwrap_or(ORDER.len())
    }

    /// 延迟项的 `turn`（仅 `Ask` 载荷带）；`TurnEndCheck` 用来判定「是否本回合」。
    pub fn turn_of(&self) -> Option<usize> {
        match &self.payload {
            Payload::Ask { data, .. } => data.get("turn").and_then(|v| v.as_u64()).map(|v| v as usize),
            _ => None,
        }
    }

    /// 当前是否**可派发**（`TurnEndCheck` 只在当前回合的结束阶段派发）。
    pub fn ready(&self, g: &super::Game) -> bool {
        match self.kind {
            DeferredKind::TurnEndCheck => {
                g.phase == super::Phase::End && self.turn_of() == Some(g.turn)
            }
            _ => true,
        }
    }

    /// 是否**陈旧**（回合已过的结束校验）：调用方负责丢弃。
    pub fn stale(&self, g: &super::Game) -> bool {
        matches!(self.kind, DeferredKind::TurnEndCheck)
            && self.turn_of().map(|t| t < g.turn).unwrap_or(true)
    }
}
