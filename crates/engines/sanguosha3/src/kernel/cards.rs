//! 牌定义 + 目标属性 + 响应牌标签（mechanics `card-attributes` / `card-target-attribute` /
//! `response-cards` / `damage-trick`）。
//!
//! 不变量：牌的类别/子类/颜色/点数/伤害属性/响应牌标签**一律读字段**，禁止按牌名字符串判定。
//! 本模块的 `def(code)` 是按**牌定义 code** 查表（生成物 `cards_generated` 的占位，M2 接入），
//! 内核不得出现 `== "杀"` 这类牌名比较（门禁 `gate_kernel_has_no_card_name_branch`）。

use serde::{Deserialize, Serialize};

use super::{DamageKind, Suit};

/// 牌类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Basic,
    Trick,
    Equip,
    DelayedTrick,
}

/// 目标模式（mechanics `target-list`：必须保留列表成员，不只用 None/One/Many 枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetMode {
    /// 无目标（无懈可击的目标是「一张牌」，不是角色）。
    None,
    /// 单目标（杀/桃/决斗/顺手/过河/火攻…）。
    Single,
    /// 可选目标（1..N，铁索连环 1–2）。
    Chosen,
    /// 全体其他角色（南蛮/万箭/桃园/五谷）。
    AllAlive,
    /// 仅自己（无中生有/闪/桃/酒/闪电）。
    SelfOnly,
}

/// 子类标签（响应牌/伤害牌判定读这些字段，不按牌名）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tags {
    pub sha: bool,
    pub shan: bool,
    pub tao: bool,
    pub jiu: bool,
    pub wuxie: bool,
    pub duel: bool,
    /// 延时锦囊。
    pub delayed: bool,
    /// 伤害锦囊（决斗/南蛮/万箭/火攻）。
    pub damage_trick: bool,
}

/// 牌定义（生成物的行；手写代码只读）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDef {
    pub code: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub target_mode: TargetMode,
    pub tags: Tags,
    /// 伤害属性（杀=Normal，火杀=Fire，雷杀=Thunder，其余=Normal）。
    pub damage_kind: DamageKind,
}

/// 响应上下文：当前响应窗口需要的响应牌种类（mechanics `response-cards`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseKind {
    Shan,
    Sha,
    Wuxie,
    Tao,
    Jiu,
}

/// 目标是否合法（默认：存活、非自己、在可选范围内）。技能可扩展（空城/流离/帷幕），M2 接入钩子。
pub fn valid_target(target_mode: TargetMode, self_seat: usize, target: usize, alive: bool) -> bool {
    if !alive {
        return false;
    }
    match target_mode {
        TargetMode::None => false,
        TargetMode::Single | TargetMode::Chosen | TargetMode::AllAlive => target != self_seat,
        TargetMode::SelfOnly => target == self_seat,
    }
}

impl CardDef {
    /// 是否为基本牌。
    pub fn is_basic(&self) -> bool {
        self.category == Category::Basic
    }
    /// 是否延时锦囊。
    pub fn is_delayed(&self) -> bool {
        self.tags.delayed
    }
    /// 是否能作为响应牌（读标签，不按牌名）。
    pub fn responds_as(&self, kind: ResponseKind) -> bool {
        match kind {
            ResponseKind::Shan => self.tags.shan,
            ResponseKind::Sha => self.tags.sha,
            ResponseKind::Wuxie => self.tags.wuxie,
            ResponseKind::Tao => self.tags.tao,
            ResponseKind::Jiu => self.tags.jiu,
        }
    }
}

/// 牌实例的花色/点数来源（与 def 分离）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Physical {
    pub suit: Suit,
    pub rank: u8,
}
