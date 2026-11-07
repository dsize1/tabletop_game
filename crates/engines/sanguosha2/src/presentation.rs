//! 展示事件（结构化）——文案由**结果枚举**生成，禁止在分支里手写整句。
//!
//! 动机（plan §8.4）：无懈结算曾出现「cancelled=false（无懈被反无懈抵消、锦囊照常生效）」
//! 打印「效果生效」，与「被抵消」措辞相近 → 正确行为被判成 bug。
//! 这里用 `POutcome` 枚举 + 唯一文案表，从结构上消除「语义相反但措辞相近」。

use serde::{Deserialize, Serialize};

use crate::kernel::CardId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum POutcome {
    Activated { skill: String },
    Declined { skill: String },
    Damage { amount: i32 },
    Draw { n: u32 },
    Discard { n: u32 },
    Heal { amount: i32 },
    MarkPlaced { n: u32 },
    /// 无懈层数为奇数：**锦囊被抵消**。
    TrickNegated,
    /// 无懈被反无懈抵消：**锦囊照常生效**。
    TrickEffective,
}

impl POutcome {
    /// 唯一文案表（测试断言「决定性片段」存在且两条互斥）。
    pub fn text(&self, skill_label: &str) -> String {
        match self {
            POutcome::Activated { skill } => format!("【{skill}】发动"),
            POutcome::Declined { skill } => format!("【{skill}】未发动"),
            POutcome::Damage { amount } => format!("造成 {amount} 点伤害"),
            POutcome::Draw { n } => format!("摸 {n} 张牌"),
            POutcome::Discard { n } => format!("弃置 {n} 张牌"),
            POutcome::Heal { amount } => format!("回复 {amount} 点体力"),
            POutcome::MarkPlaced { n } => format!("置于武将牌上 {n} 张"),
            POutcome::TrickNegated => "锦囊被抵消（无懈层数为奇数）".to_string(),
            POutcome::TrickEffective => "锦囊照常生效（无懈被反无懈抵消）".to_string(),
        }
        .replace("{skill}", skill_label)
    }

    /// 是否存在「语义相反但措辞相近」的风险（门禁用）。
    pub fn is_inverse_of(&self, other: &POutcome) -> bool {
        matches!(
            (self, other),
            (POutcome::TrickNegated, POutcome::TrickEffective)
                | (POutcome::TrickEffective, POutcome::TrickNegated)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub skill: String,
    pub actor: Option<usize>,
    pub targets: Vec<usize>,
    pub cards: Vec<CardId>,
    pub outcome: POutcome,
    /// 前端/群侧只认 key，不认中文（文案在上面统一生成）。
    pub fx: String,
}

impl Presentation {
    pub fn new(skill: impl Into<String>, outcome: POutcome) -> Self {
        Self {
            skill: skill.into(),
            actor: None,
            targets: Vec::new(),
            cards: Vec::new(),
            outcome,
            fx: String::new(),
        }
    }

    pub fn actor(mut self, seat: usize) -> Self {
        self.actor = Some(seat);
        self
    }

    pub fn target(mut self, seat: usize) -> Self {
        self.targets.push(seat);
        self
    }

    pub fn cards(mut self, cards: Vec<CardId>) -> Self {
        self.cards = cards;
        self
    }

    pub fn fx(mut self, fx: impl Into<String>) -> Self {
        self.fx = fx.into();
        self
    }

    pub fn text(&self) -> String {
        self.outcome.text(&self.skill)
    }
}
