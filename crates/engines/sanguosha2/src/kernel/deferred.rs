//! 单一延迟队列 + **显式顺序表**。
//!
//! 旧引擎有 6 条「按技能划分」的队列，顺序靠代码位置隐式决定。这里合成 1 条，
//! 顺序由 `ORDER` 常量表决定（常量即文档，可被测试断言）。

use serde::{Deserialize, Serialize};

use super::{CardId, Zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeferredKind {
    /// ③ 良姻回血判定（必须在「使用剩余装备」之前）。
    LiangYinHeal,
    /// ④ 箜声：令一名角色使用剩余装备并失去 1 点体力。
    KongShengUse,
    /// 技能触发询问。
    SkillTrigger,
    /// 判定阶段：延时锦囊 LIFO 结算（必须在「阶段开始」技能触发之后）。
    JudgePhase,
    /// 死亡清理（必须排在所有死亡时技能之后）。
    DeathCleanup,
}

/// **顺序表**（常量即文档）。改动必须同步 `docs/games/sanguosha/align/mechanics.json`。
pub const ORDER: [DeferredKind; 5] = [
    DeferredKind::LiangYinHeal,
    DeferredKind::KongShengUse,
    DeferredKind::SkillTrigger,
    DeferredKind::JudgePhase,
    DeferredKind::DeathCleanup,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Trigger {
        owner: usize,
        skill: String,
        /// 事件类型（`EventKind` 的 Debug 名），技能据此分流。
        event: String,
        /// 事件主体座位（受击者 / 判定者 / 死亡者…）。
        seat: Option<usize>,
        card: Option<CardId>,
        cause: Option<String>,
        /// 事件数值（伤害点数 / 失去体力数 / 上限变化量）。
        #[serde(default)]
        amount: Option<i32>,
        /// 事件发生时刻的回合座位（快照）。
        #[serde(default)]
        turn: usize,
        /// 事件发生时刻的阶段（快照）。
        #[serde(default)]
        phase: crate::kernel::Phase,
        /// 施动者（弃置者/使用者/获得者）。
        #[serde(default)]
        by: Option<usize>,
        /// 来源座位（牌此前属于谁）。
        #[serde(default)]
        from: Option<usize>,
    },
    /// 通用「待栈空后询问」：技能名/阶段/数据均由调用方给定，内核不认识具体技能。
    Ask {
        skill: String,
        stage: crate::kernel::resolve::Stage,
        actor: usize,
        data: serde_json::Value,
    },
    /// 从帧区落盘（防止牌滞留在 Frame 区）。
    Land {
        card: CardId,
        to: Zone,
    },
    /// 判定阶段：对某座位的延时锦囊发起 LIFO 结算。
    JudgePhase {
        seat: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deferred {
    pub kind: DeferredKind,
    pub payload: Payload,
}

impl Deferred {
    /// 统一构造入口（技能侧使用）。
    pub fn ask(
        kind: DeferredKind,
        skill: impl Into<String>,
        stage: crate::kernel::resolve::Stage,
        actor: usize,
        data: serde_json::Value,
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
}
