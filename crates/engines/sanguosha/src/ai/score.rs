//! 动作打分的统一口径：`(priority, score)` 二元组。
//!
//! - `priority` 是**硬优先级**（越大越优先）：用于表达「必须做的事」——
//!   濒死自救 / 救队友 / 必选响应 / 确认提交；
//! - `score` 是同优先级内的**期望收益**（越大越好）。
//!
//! 选动作 = 取 `(priority, score)` 最大者；同分按合法动作顺序（确定性）。
//! 这样既能让「必选响应」压过一切，又能在常规动作之间做收益比较。

/// 动作得分。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub priority: i32,
    pub score: f64,
}

impl Score {
    pub const fn new(priority: i32, score: f64) -> Self {
        Self { priority, score }
    }

    /// 中性分（未知动作的兜底）。
    pub const fn neutral() -> Self {
        Self::new(0, 1.0)
    }

    /// 是否优于另一个得分。
    pub fn better_than(&self, other: &Score) -> bool {
        if self.priority != other.priority {
            return self.priority > other.priority;
        }
        self.score > other.score
    }

    /// 排序键（用于剪枝：优先级降序，同优先级分数降序）。
    pub fn sort_key(&self) -> (i32, f64) {
        (self.priority, self.score)
    }
}

/// 常用优先级常量（语义化，避免魔法数字散落）。
pub mod prio {
    /// 自救 / 不救则死。
    pub const MUST: i32 = 10;
    /// 救队友 / 必选响应。
    pub const URGENT: i32 = 8;
    /// 提交类（多选 confirm / 观星 arrange）。
    pub const COMMIT: i32 = 6;
    /// 强收益（必出的一击）。
    pub const STRONG: i32 = 4;
    /// 常规收益。
    pub const NORMAL: i32 = 0;
    /// 放弃 / 空过（通常分数决定是否比常规动作好）。
    pub const IDLE: i32 = -2;
}
