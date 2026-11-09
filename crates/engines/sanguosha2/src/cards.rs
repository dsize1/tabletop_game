//! 卡牌属性表（**唯一**的牌类/标签/效果来源）。
//!
//! 数据来自生成物 [`crate::cards_generated`]：`npm run gen:sgs-engine-data` 把
//! `docs/games/sanguosha/data/cards.json`（160 张）生成成 Rust 常量，本文件只做
//! 「表 → `CardDef`」的索引，**不含任何牌名判定**。
//!
//! 规则（`docs/conventions/skill-spec-rules.md` §3）：技能的「基本牌/锦囊/是否伤害锦囊/
//! 是否杀/闪/桃/酒/无懈」判定**只能**查这里的属性，禁止在技能/内核里 `match` 牌名。

use serde::{Deserialize, Serialize};

use crate::kernel::{Card, Slot};

pub use crate::cards_generated::DECK_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardKind {
    Basic,
    Trick,
    Equip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetMode {
    /// 使用/打出时必须选择目标（杀、决斗、顺手…）
    Chosen,
    /// 目标为全场存活角色（五谷丰登、桃园结义、万箭齐发、南蛮入侵）
    AllAlive,
    /// 不指定其他目标（无中生有、闪电、装备、桃、酒）
    SelfOnly,
}

/// 伤害属性（普通 / 火 / 雷）。技能判定只读该字段，禁止按牌名/技能名硬编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageKind {
    Normal,
    Fire,
    Thunder,
}

/// 响应场景（由「该环节的响应牌」驱动，禁止按技能名硬编码）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseKind {
    /// 需要【杀】（南蛮入侵 / 决斗 / 借刀）
    Sha,
    /// 需要【闪】（杀 / 万箭齐发）
    Shan,
    /// 需要【无懈可击】（无懈链）
    Wuxie,
}

/// 防具类型（伤害数值链/响应窗口只读该字段，见 `docs/games/sanguosha/rules.md` §7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArmorKind {
    #[default]
    None,
    /// 八卦阵：需要【闪】时可判定，红色视为打出【闪】。
    Bagua,
    /// 仁王盾：黑色【杀】对你无效。
    Renwang,
    /// 藤甲：普通【杀】/【南蛮入侵】/【万箭齐发】对你无效；火焰伤害 +1。
    Tengjia,
    /// 白银狮子：受到伤害时，若伤害 > 1 则改为 1；失去时回复 1 点体力。
    Baiyin,
}

/// 武器类型（只有需要内核介入的武器才登记；其余武器只提供攻击范围）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WeaponKind {
    #[default]
    None,
    /// 青釭剑：无视防具。
    Qinggang,
    Cixiong,
    Hanbing,
    Qinglong,
    Guanshi,
    Fangtian,
    Qilin,
    Zhugeliannu,
    Gudingdao,
    Zhangba,
    Zhuque,
}

/// 牌的结算效果（**属性化**的唯一入口：内核按效果分派，不按牌名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// 装备（按槽位处理）。
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Tags {
    pub sha: bool,
    pub shan: bool,
    pub tao: bool,
    pub jiu: bool,
    pub wuxie: bool,
    /// 【决斗】（父荫等按此判定，不比较牌名）
    pub duel: bool,
    /// 延时锦囊（乐不思蜀/兵粮寸断/闪电）
    pub delayed: bool,
    /// 【铁索连环】
    pub lianhuan: bool,
    /// 目标模式（禁止再退化成「有/无目标」布尔）。
    #[serde(default)]
    pub target_mode: Option<TargetMode>,
    /// 该牌要求的响应（杀→闪 / 南蛮→杀 / 万箭→闪 / 决斗→杀）。
    #[serde(default)]
    pub respond: Option<ResponseKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDef {
    pub kind: CardKind,
    /// 伤害锦囊（决斗/万箭/南蛮/火攻）——征荣、往烈等按此判定。
    pub damage_trick: bool,
    pub tags: Tags,
    /// 伤害属性（普通/火/雷）。
    pub damage_kind: DamageKind,
    /// 结算效果（内核按此分派）。
    pub effect: Effect,
    pub armor: ArmorKind,
    pub weapon: WeaponKind,
    /// 装备槽位（非装备为 `None`）。
    pub slot: Option<Slot>,
    /// 武器攻击距离。
    pub weapon_range: Option<usize>,
    /// 坐骑距离修正（进攻马/防御马都是 1）。
    pub horse: Option<usize>,
    /// 是否在生成表中（未知牌 = false；门禁会断言所有实体牌都已知）。
    pub known: bool,
}

fn from_row(r: &'static crate::cards_generated::Row) -> CardDef {
    CardDef {
        kind: r.kind,
        damage_trick: r.damage_trick,
        tags: Tags {
            sha: r.sha,
            shan: r.shan,
            tao: r.tao,
            jiu: r.jiu,
            wuxie: r.wuxie,
            duel: r.duel,
            delayed: r.delayed,
            lianhuan: r.lianhuan,
            target_mode: Some(r.target_mode),
            respond: r.respond,
        },
        damage_kind: r.damage_kind,
        effect: r.effect,
        armor: r.armor,
        weapon: r.weapon,
        slot: r.slot,
        weapon_range: r.weapon_range,
        horse: r.horse,
        known: true,
    }
}

/// 未登记的牌名：按「无效果锦囊」处理，并由门禁在测试里报警（不 panic，避免存档/沙盒崩）。
fn unknown() -> CardDef {
    CardDef {
        kind: CardKind::Trick,
        damage_trick: false,
        tags: Tags {
            target_mode: Some(TargetMode::SelfOnly),
            ..Tags::default()
        },
        damage_kind: DamageKind::Normal,
        effect: Effect::None,
        armor: ArmorKind::None,
        weapon: WeaponKind::None,
        slot: None,
        weapon_range: None,
        horse: None,
        known: false,
    }
}

/// 牌名 → 定义（唯一定义查询入口）。
pub fn def(name: &str) -> CardDef {
    crate::cards_generated::DEFS
        .iter()
        .find(|r| r.name == name)
        .map(from_row)
        .unwrap_or_else(unknown)
}

pub fn is_known(name: &str) -> bool {
    crate::cards_generated::DEFS.iter().any(|r| r.name == name)
}

/// 该牌能否作为某响应场景的响应牌（读属性，不按牌名枚举）。
pub fn responds_as(name: &str, kind: ResponseKind) -> bool {
    let t = tags(name);
    match kind {
        ResponseKind::Sha => t.sha,
        ResponseKind::Shan => t.shan,
        ResponseKind::Wuxie => t.wuxie,
    }
}

/// 全部锦囊牌名（牌定义表 = 唯一来源；技能只能按属性过滤，不得各自列白名单）。
pub fn trick_names() -> Vec<&'static str> {
    crate::cards_generated::DEFS
        .iter()
        .filter(|r| r.kind == CardKind::Trick)
        .map(|r| r.name)
        .collect()
}

/// 非延迟锦囊（奇策的候选项）：读 `tags.delayed` 属性，不按牌名写死。
pub fn non_delayed_tricks() -> Vec<&'static str> {
    trick_names()
        .into_iter()
        .filter(|n| !tags(n).delayed)
        .collect()
}

/// 牌名的中文标签（守玺/督粮等「声明牌名」类技能的展示用）。
pub fn label_of(name: &str) -> String {
    // 生成表的 `name` 就是中文牌名（如「杀」「无中生有」）。
    name.to_string()
}

/// 牌名的中文标签（守玺/督粮等「声明牌名」类技能的展示用）。
/// 牌名的中文标签（守玺/督粮等「声明牌名」类技能的展示用）。
/// 全部**非装备**牌名（守玺的声明候选：读类别属性，不枚举牌名）。
pub fn non_equip_names() -> Vec<&'static str> {
    crate::cards_generated::DEFS
        .iter()
        .filter(|r| r.kind != CardKind::Equip)
        .map(|r| r.name)
        .collect()
}

pub fn is_basic(name: &str) -> bool {
    def(name).kind == CardKind::Basic
}

pub fn is_damage_trick(name: &str) -> bool {
    def(name).damage_trick
}

pub fn is_equip(name: &str) -> bool {
    def(name).kind == CardKind::Equip
}

pub fn tags(name: &str) -> Tags {
    def(name).tags
}

pub fn target_mode(name: &str) -> TargetMode {
    tags(name).target_mode.unwrap_or(TargetMode::Chosen)
}

/// 是否需要玩家手动选目标（全场牌自动取全体，不手动选）。
pub fn needs_choice(name: &str) -> bool {
    target_mode(name) == TargetMode::Chosen
}

/// 武器攻击范围（与旧引擎 `data.rs` 的 `range_value` 一致；来自生成表）。
pub fn weapon_range(name: &str) -> Option<usize> {
    def(name).weapon_range
}

/// 坐骑距离修正（进攻马/防御马都是 1；来自生成表）。
pub fn horse_bonus(name: &str) -> Option<usize> {
    def(name).horse
}

/// 完整牌堆（160 张，standard 108 + junzheng 52；id = `c1..c160`）。
///
/// 这是 M1 的「真实牌堆」入口：洗牌/发牌由调用方决定（确定性顺序便于测试）。
pub fn deck() -> Vec<Card> {
    crate::cards_generated::DECK
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d = def(p.name);
            Card {
                id: format!("c{}", i + 1),
                name: p.name.to_string(),
                suit: p.suit,
                rank: p.rank,
                slot: d.slot,
                target_mode: d.tags.target_mode.unwrap_or(TargetMode::Chosen),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_tricks_include_huogong() {
        for n in ["决斗", "万箭齐发", "南蛮入侵", "火攻"] {
            assert!(is_damage_trick(n), "{n} 应为伤害锦囊");
        }
        for n in ["无懈可击", "无中生有", "顺手牵羊"] {
            assert!(!is_damage_trick(n), "{n} 不应为伤害锦囊");
        }
    }

    #[test]
    fn basic_and_tags() {
        assert!(is_basic("杀") && is_basic("桃") && is_basic("酒") && is_basic("闪"));
        assert!(!is_basic("决斗"));
        assert!(tags("杀").sha && tags("桃").tao && tags("酒").jiu && tags("无懈可击").wuxie);
        assert!(is_equip("八卦阵"));
    }

    #[test]
    fn target_modes() {
        assert_eq!(target_mode("无中生有"), TargetMode::SelfOnly);
        assert_eq!(target_mode("五谷丰登"), TargetMode::AllAlive);
        assert_eq!(target_mode("桃园结义"), TargetMode::AllAlive);
        assert_eq!(target_mode("万箭齐发"), TargetMode::AllAlive);
        assert_eq!(target_mode("决斗"), TargetMode::Chosen);
        assert_eq!(target_mode("铁索连环"), TargetMode::Chosen);
        assert_eq!(target_mode("乐不思蜀"), TargetMode::Chosen);
        assert!(!needs_choice("五谷丰登") && needs_choice("杀"));
    }

    #[test]
    fn generated_deck_is_full_and_known() {
        let deck = deck();
        assert_eq!(deck.len(), DECK_SIZE);
        assert_eq!(deck.len(), 160, "standard 108 + junzheng 52");
        // 每张实体牌的牌名都必须在生成表里（未知牌 = 数据缺口）。
        for c in &deck {
            assert!(is_known(&c.name), "未知牌名：{}", c.name);
        }
        // 装备槽位由定义表给出（不再由 `Card::equip` 手工决定）。
        let wuqi = deck.iter().find(|c| c.name == "青釭剑").expect("青釭剑");
        assert_eq!(wuqi.slot, Some(Slot::Weapon));
        let ma = deck.iter().find(|c| c.name == "的卢").expect("的卢");
        assert_eq!(ma.slot, Some(Slot::HorseDefense));
        // id 唯一。
        let ids: std::collections::BTreeSet<_> = deck.iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids.len(), deck.len());
    }

    #[test]
    fn effects_and_equip_attributes_are_data() {
        assert_eq!(def("杀").effect, Effect::Sha);
        assert_eq!(def("火杀").damage_kind, DamageKind::Fire);
        assert_eq!(def("雷杀").damage_kind, DamageKind::Thunder);
        assert_eq!(def("藤甲").armor, ArmorKind::Tengjia);
        assert_eq!(def("白银狮子").armor, ArmorKind::Baiyin);
        assert_eq!(def("青釭剑").weapon, WeaponKind::Qinggang);
        assert_eq!(def("麒麟弓").weapon_range, Some(5));
        assert_eq!(def("赤兔").horse, Some(1));
        assert_eq!(def("顺手牵羊").weapon_range, None);
        assert_eq!(def("南蛮入侵").tags.respond, Some(ResponseKind::Sha));
        assert_eq!(def("万箭齐发").tags.respond, Some(ResponseKind::Shan));
    }

    #[test]
    fn trick_names_cover_generated_table() {
        let names = trick_names();
        assert_eq!(names.len(), 15, "{names:?}");
        assert!(names.contains(&"无懈可击") && names.contains(&"闪电"));
        assert_eq!(non_delayed_tricks().len(), 12);
        // 未知牌不会 panic（存档兼容），但 known=false。
        assert!(!def("不存在的牌").known);
    }
}
