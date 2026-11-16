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
    /// 铁索连环（横置/重置，P4）。
    pub lianhuan: bool,
}

/// 结算效果（B0 前置项 P4：内核按**效果字段**分派，禁止按牌名 match）。
///
/// 与 v2 `cards::Effect` 同口径（同一份生成数据），V3 只保留「无技能接入也能完整结算」的部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Effect {
    /// 装备牌（按槽位处理）或未登记效果。
    #[default]
    None,
    Sha,
    Shan,
    Tao,
    Jiu,
    Wuxie,
    Duel,
    Dismantle,
    Snatch,
    ExNihilo,
    SavageAssault,
    ArcheryAttack,
    PeachGarden,
    Harvest,
    FireAttack,
    Collateral,
    IronChain,
    Indulgence,
    SupplyShortage,
    Lightning,
}

impl Effect {
    /// 延时锦囊（进判定区，判定阶段按 LIFO 结算）。
    pub fn is_delayed(self) -> bool {
        matches!(
            self,
            Effect::Indulgence | Effect::SupplyShortage | Effect::Lightning
        )
    }

    /// 需要「无懈可击」窗口的效果（非延时锦囊的锦囊效果；延时锦囊在放置时也要无懈）。
    pub fn is_trick(self) -> bool {
        !matches!(
            self,
            Effect::None | Effect::Sha | Effect::Shan | Effect::Tao | Effect::Jiu
        )
    }
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
    /// 结算效果（P4 唯一分派依据；内核不按牌名判定）。
    pub effect: Effect,
    /// 该牌**要求**的响应牌（杀→闪 / 南蛮→杀 / 万箭→闪 / 决斗→杀）。
    pub respond: Option<ResponseKind>,
    /// 装备槽位（武器/防具/进攻马/防御马/宝物；非装备为 None）。
    pub slot: Option<super::Slot>,
    /// 武器攻击范围（仅武器）。
    pub weapon_range: Option<u8>,
    /// 坐骑距离（仅坐骑）。
    pub horse: Option<u8>,
    /// 防具效果（八卦/仁王/藤甲/白银）。
    pub armor: super::ArmorKind,
    /// 武器效果（青釭/连弩/丈八/朱雀…）。
    pub weapon: super::WeaponKind,
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

/// 牌定义查表（生成物 `cards_generated::DEFS`；内核不按牌名字面量判定，按 code 查表）。
pub fn def(code: &str) -> Option<&'static CardDef> {
    crate::cards_generated::DEFS.iter().find(|d| d.code == code)
}

/// 卡牌属性（比较语义的唯一入口）：`None` = 该牌**没有**花色/点数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attrs {
    pub suit: Option<Suit>,
    pub rank: Option<u8>,
}

/// **卡牌属性读取（比较语义单入口）**。
///
/// 口径（2026-11-16 用户裁定）：
/// - 实体牌 → 实体花色/点数；
/// - **虚拟牌**（`Game.virtual_ids`）：0 张承载（纯视为使用）或 ≥2 张承载（长标/丈八）→ **无属性**；
///   恰好 1 张承载（武圣/龙胆/禁酒 K 点杀/蛊惑单张）→ 继承该承载牌。
///
/// 无属性牌在**一切比较**中判否：颜色/花色判定恒 false（仁王盾「黑色【杀】无效」→ 无属性杀**有效**），
/// 点数比较恒 0 / 不相等。
pub fn attrs(g: &super::Game, id: &str) -> Attrs {
    if g.virtual_ids.contains(id) {
        let carriers = g.virtual_origins.get(id).map(|o| o.len()).unwrap_or(0);
        if carriers == 1 {
            // 单张承载：**读实例属性**（创建时已套用花色/点数覆盖：
            // 红颜「黑桃→红桃」、禁酒「【酒】视为 K 点」），不要回头读承载牌原始值。
            return match g.card_defs.get(id) {
                Some(c) => Attrs {
                    suit: Some(c.suit),
                    rank: Some(c.rank),
                },
                None => Attrs::default(),
            };
        }
        // 0 张（纯视为使用）/ ≥2 张（长标、丈八）→ 无属性。
        return Attrs::default();
    }
    match g.card_defs.get(id) {
        Some(c) => Attrs {
            suit: Some(c.suit),
            rank: Some(c.rank),
        },
        None => Attrs::default(),
    }
}

/// 是否为红色（无属性 → false）。
pub fn is_red(g: &super::Game, id: &str) -> bool {
    attrs(g, id).suit.map(|s| s.is_red()).unwrap_or(false)
}

/// 是否为黑色（无属性 → false；**不是** `!is_red`）。
pub fn is_black(g: &super::Game, id: &str) -> bool {
    attrs(g, id).suit.map(|s| s.is_black()).unwrap_or(false)
}

/// 点数（无属性 → None；比较方按 0 / 不相等处理）。
pub fn rank_of(g: &super::Game, id: &str) -> Option<u8> {
    attrs(g, id).rank
}

/// 花色（无属性 → None）。
pub fn suit_of(g: &super::Game, id: &str) -> Option<Suit> {
    attrs(g, id).suit
}

/// 花色是否等于指定值（无属性 → false）。
pub fn suit_is(g: &super::Game, id: &str, suit: Suit) -> bool {
    attrs(g, id).suit == Some(suit)
}

/// 两张牌是否同色（任一无属性 → false，不是「都非红」）。
pub fn same_color(g: &super::Game, a: &str, b: &str) -> bool {
    match (attrs(g, a).suit, attrs(g, b).suit) {
        (Some(x), Some(y)) => x.is_red() == y.is_red(),
        _ => false,
    }
}

/// 两张牌是否同花色（任一无属性 → false）。
pub fn same_suit(g: &super::Game, a: &str, b: &str) -> bool {
    match (attrs(g, a).suit, attrs(g, b).suit) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// 两张牌点数是否相同（任一无属性 → false；点数比较判否）。
pub fn same_rank(g: &super::Game, a: &str, b: &str) -> bool {
    match (attrs(g, a).rank, attrs(g, b).rank) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// 全量牌定义（**技能枚举「可选牌名」的唯一数据源**）。
///
/// 教训（2026-11-16 审计）：矫诏把 17 个牌 code 硬编码在技能里，
/// 其中 `huogong` 与数据里的 `huo_gong` 不一致 → 【火攻】静默不可声明。
pub fn all_defs() -> &'static [CardDef] {
    crate::cards_generated::DEFS
}

/// 牌名（中文）→ 定义 code（生成物唯一来源；中台同步/回放对拍用）。
pub fn code_of(name: &str) -> Option<&'static str> {
    crate::cards_generated::DEFS
        .iter()
        .find(|d| d.name == name)
        .map(|d| d.code)
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
