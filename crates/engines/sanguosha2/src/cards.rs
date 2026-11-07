//! 卡牌定义表（**唯一**的牌类/标签来源）。
//!
//! 规则（`docs/conventions/skill-spec-rules.md` §3）：技能的「基本牌/锦囊/是否伤害锦囊/
//! 是否杀/闪/桃/酒/无懈」判定**只能**查这里的属性，禁止在技能里 `match` 牌名。

use serde::{Deserialize, Serialize};

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
    /// 不指定其他目标（无中生有、闪电等）
    SelfOnly,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDef {
    pub kind: CardKind,
    /// 伤害锦囊（决斗/万箭/南蛮/火攻）——征荣、往烈等按此判定。
    pub damage_trick: bool,
    pub tags: Tags,
}

const EQUIP_NAMES: &[&str] = &[
    "青釭剑", "八卦阵", "赤兔", "的卢", "木牛流马", "寒冰剑", "麒麟弓", "贯石斧", "青龙偃月刀",
    "雌雄双股剑", "方天画戟", "诸葛连弩", "仁王盾", "藤甲", "白银狮子", "爪黄飞电", "绝影",
];
const DAMAGE_TRICKS: &[&str] = &["决斗", "万箭齐发", "南蛮入侵", "火攻"];

/// 牌名 → 定义。未收录的按锦囊处理（并在测试里报警）。
pub fn def(name: &str) -> CardDef {
    let equip = EQUIP_NAMES.contains(&name);
    let damage_trick = DAMAGE_TRICKS.contains(&name);
    let kind = if equip {
        CardKind::Equip
    } else if matches!(
        name,
        "杀" | "闪" | "桃" | "酒"
    ) {
        CardKind::Basic
    } else {
        CardKind::Trick
    };
    let tags = Tags {
        sha: name == "杀",
        shan: name == "闪",
        tao: name == "桃",
        jiu: name == "酒",
        wuxie: name == "无懈可击",
        duel: name == "决斗",
        delayed: matches!(name, "乐不思蜀" | "兵粮寸断" | "闪电"),
        lianhuan: name == "铁索连环",
        target_mode: Some(match name {
            "五谷丰登" | "桃园结义" | "万箭齐发" | "南蛮入侵" => TargetMode::AllAlive,
            "无中生有" | "闪电" => TargetMode::SelfOnly,
            _ => TargetMode::Chosen,
        }),
    };
    CardDef {
        kind,
        damage_trick,
        tags,
    }
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
        assert!(!needs_choice("五谷丰登") && needs_choice("杀"));
    }
}
