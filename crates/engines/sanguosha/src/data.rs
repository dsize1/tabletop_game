//! 三国杀引擎内置数据（**自动生成，勿手改**）。
//!
//! 生成：`node apps/scripts/gen-sanguosha-engine-data.mjs`
//! 校验：`node apps/scripts/gen-sanguosha-engine-data.mjs --check`
//! 来源：`docs/games/sanguosha/data/cards.json`（standard）+ `heroes.json`（标准包 25 将）。

use crate::state::{Category, EquipSlot, RangeType, Suit, TargetMode};

pub const STANDARD_DECK_SIZE: usize = 108;
pub const STANDARD_GENERAL_COUNT: usize = 25;

/// 牌的定义（同一 `code` 只有一条；物理牌见 `STANDARD_DECK`）。
#[derive(Debug, Clone, Copy)]
pub struct CardDef {
    pub code: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub equip_slot: Option<EquipSlot>,
    /// 武器攻击距离 / 坐骑距离；非装备为 `None`。
    pub range_value: Option<i8>,
    /// 坐骑方向：进攻马 / 防御马。
    pub range_type: Option<RangeType>,
    /// 目标模式：单目标 / AOE。
    pub target_mode: Option<TargetMode>,
    pub delayed: bool,
}

#[rustfmt::skip]
pub static CARD_DEFS: &[CardDef] = &[
    CardDef { code: "taoyuan_jieyi", name: "桃园结义", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Aoe), delayed: false,},
    CardDef { code: "wanjian_qifa", name: "万箭齐发", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Aoe), delayed: false,},
    CardDef { code: "shan", name: "闪", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "tao", name: "桃", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "wugu_fengdeng", name: "五谷丰登", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Aoe), delayed: false,},
    CardDef { code: "qilin_gong", name: "麒麟弓", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(5), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "chitu", name: "赤兔", category: Category::Horse, equip_slot: Some(EquipSlot::HorseOffense), range_value: Some(1), range_type: Some(RangeType::Attack), target_mode: None, delayed: false,},
    CardDef { code: "lebu_sishu", name: "乐不思蜀", category: Category::DelayedTrick, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: true,},
    CardDef { code: "wuzhong_shengyou", name: "无中生有", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
    CardDef { code: "sha", name: "杀", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "guohe_chaiqiao", name: "过河拆桥", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
    CardDef { code: "shandian", name: "闪电", category: Category::DelayedTrick, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: true,},
    CardDef { code: "zhuahuang_feidian", name: "爪黄飞电", category: Category::Horse, equip_slot: Some(EquipSlot::HorseDefense), range_value: Some(1), range_type: Some(RangeType::Defense), target_mode: None, delayed: false,},
    CardDef { code: "juedou", name: "决斗", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
    CardDef { code: "cixiong_shuanggujian", name: "雌雄双股剑", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(2), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "bagua_zhen", name: "八卦阵", category: Category::Armor, equip_slot: Some(EquipSlot::Armor), range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "hanbing_jian", name: "寒冰剑", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(2), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "shunshou_qianyang", name: "顺手牵羊", category: Category::Trick, equip_slot: None, range_value: Some(1), range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
    CardDef { code: "qinglong_yanyue", name: "青龙偃月刀", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(3), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "jueying", name: "绝影", category: Category::Horse, equip_slot: Some(EquipSlot::HorseDefense), range_value: Some(1), range_type: Some(RangeType::Defense), target_mode: None, delayed: false,},
    CardDef { code: "qinggang_jian", name: "青釭剑", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(2), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "nanman_ruqin", name: "南蛮入侵", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Aoe), delayed: false,},
    CardDef { code: "wuxie_keji", name: "无懈可击", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "zhangba_shemao", name: "丈八蛇矛", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(3), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "dawan", name: "大宛", category: Category::Horse, equip_slot: Some(EquipSlot::HorseOffense), range_value: Some(1), range_type: Some(RangeType::Attack), target_mode: None, delayed: false,},
    CardDef { code: "zhuge_liannu", name: "诸葛连弩", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(1), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "guanshi_fu", name: "贯石斧", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(3), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "fangtian_huaji", name: "方天画戟", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(4), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "zixing", name: "紫骍", category: Category::Horse, equip_slot: Some(EquipSlot::HorseOffense), range_value: Some(1), range_type: Some(RangeType::Attack), target_mode: None, delayed: false,},
    CardDef { code: "renwang_dun", name: "仁王盾", category: Category::Armor, equip_slot: Some(EquipSlot::Armor), range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "dilu", name: "的卢", category: Category::Horse, equip_slot: Some(EquipSlot::HorseDefense), range_value: Some(1), range_type: Some(RangeType::Defense), target_mode: None, delayed: false,},
    CardDef { code: "jiedao_sharen", name: "借刀杀人", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
];

/// 牌堆中的每一张物理牌（花色 + 点数）。
#[derive(Debug, Clone, Copy)]
pub struct PhysicalCard {
    pub def: &'static str,
    pub suit: Suit,
    pub rank: u8,
}

#[rustfmt::skip]
pub static STANDARD_DECK: &[PhysicalCard] = &[
    PhysicalCard { def: "taoyuan_jieyi", suit: Suit::Heart, rank: 1 },
    PhysicalCard { def: "wanjian_qifa", suit: Suit::Heart, rank: 1 },
    PhysicalCard { def: "shan", suit: Suit::Heart, rank: 2 },
    PhysicalCard { def: "shan", suit: Suit::Heart, rank: 2 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 3 },
    PhysicalCard { def: "wugu_fengdeng", suit: Suit::Heart, rank: 3 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 4 },
    PhysicalCard { def: "wugu_fengdeng", suit: Suit::Heart, rank: 4 },
    PhysicalCard { def: "qilin_gong", suit: Suit::Heart, rank: 5 },
    PhysicalCard { def: "chitu", suit: Suit::Heart, rank: 5 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 6 },
    PhysicalCard { def: "lebu_sishu", suit: Suit::Heart, rank: 6 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 7 },
    PhysicalCard { def: "wuzhong_shengyou", suit: Suit::Heart, rank: 7 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 8 },
    PhysicalCard { def: "wuzhong_shengyou", suit: Suit::Heart, rank: 8 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 9 },
    PhysicalCard { def: "wuzhong_shengyou", suit: Suit::Heart, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Heart, rank: 10 },
    PhysicalCard { def: "sha", suit: Suit::Heart, rank: 10 },
    PhysicalCard { def: "sha", suit: Suit::Heart, rank: 11 },
    PhysicalCard { def: "wuzhong_shengyou", suit: Suit::Heart, rank: 11 },
    PhysicalCard { def: "tao", suit: Suit::Heart, rank: 12 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Heart, rank: 12 },
    PhysicalCard { def: "shandian", suit: Suit::Heart, rank: 12 },
    PhysicalCard { def: "shan", suit: Suit::Heart, rank: 13 },
    PhysicalCard { def: "zhuahuang_feidian", suit: Suit::Heart, rank: 13 },
    PhysicalCard { def: "juedou", suit: Suit::Spade, rank: 1 },
    PhysicalCard { def: "shandian", suit: Suit::Spade, rank: 1 },
    PhysicalCard { def: "cixiong_shuanggujian", suit: Suit::Spade, rank: 2 },
    PhysicalCard { def: "bagua_zhen", suit: Suit::Spade, rank: 2 },
    PhysicalCard { def: "hanbing_jian", suit: Suit::Spade, rank: 2 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Spade, rank: 3 },
    PhysicalCard { def: "shunshou_qianyang", suit: Suit::Spade, rank: 3 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Spade, rank: 4 },
    PhysicalCard { def: "shunshou_qianyang", suit: Suit::Spade, rank: 4 },
    PhysicalCard { def: "qinglong_yanyue", suit: Suit::Spade, rank: 5 },
    PhysicalCard { def: "jueying", suit: Suit::Spade, rank: 5 },
    PhysicalCard { def: "lebu_sishu", suit: Suit::Spade, rank: 6 },
    PhysicalCard { def: "qinggang_jian", suit: Suit::Spade, rank: 6 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 7 },
    PhysicalCard { def: "nanman_ruqin", suit: Suit::Spade, rank: 7 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 8 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 8 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 10 },
    PhysicalCard { def: "sha", suit: Suit::Spade, rank: 10 },
    PhysicalCard { def: "shunshou_qianyang", suit: Suit::Spade, rank: 11 },
    PhysicalCard { def: "wuxie_keji", suit: Suit::Spade, rank: 11 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Spade, rank: 12 },
    PhysicalCard { def: "zhangba_shemao", suit: Suit::Spade, rank: 12 },
    PhysicalCard { def: "nanman_ruqin", suit: Suit::Spade, rank: 13 },
    PhysicalCard { def: "dawan", suit: Suit::Spade, rank: 13 },
    PhysicalCard { def: "zhuge_liannu", suit: Suit::Diamond, rank: 1 },
    PhysicalCard { def: "juedou", suit: Suit::Diamond, rank: 1 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 2 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 2 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 3 },
    PhysicalCard { def: "shunshou_qianyang", suit: Suit::Diamond, rank: 3 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 4 },
    PhysicalCard { def: "shunshou_qianyang", suit: Suit::Diamond, rank: 4 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 5 },
    PhysicalCard { def: "guanshi_fu", suit: Suit::Diamond, rank: 5 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 6 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 6 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 7 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 7 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 8 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 8 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 9 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 10 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 10 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 11 },
    PhysicalCard { def: "shan", suit: Suit::Diamond, rank: 11 },
    PhysicalCard { def: "tao", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { def: "fangtian_huaji", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { def: "wuxie_keji", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { def: "sha", suit: Suit::Diamond, rank: 13 },
    PhysicalCard { def: "zixing", suit: Suit::Diamond, rank: 13 },
    PhysicalCard { def: "juedou", suit: Suit::Club, rank: 1 },
    PhysicalCard { def: "zhuge_liannu", suit: Suit::Club, rank: 1 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 2 },
    PhysicalCard { def: "bagua_zhen", suit: Suit::Club, rank: 2 },
    PhysicalCard { def: "renwang_dun", suit: Suit::Club, rank: 2 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 3 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Club, rank: 3 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 4 },
    PhysicalCard { def: "guohe_chaiqiao", suit: Suit::Club, rank: 4 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 5 },
    PhysicalCard { def: "dilu", suit: Suit::Club, rank: 5 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 6 },
    PhysicalCard { def: "lebu_sishu", suit: Suit::Club, rank: 6 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 7 },
    PhysicalCard { def: "nanman_ruqin", suit: Suit::Club, rank: 7 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 8 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 8 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 9 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 10 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 10 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 11 },
    PhysicalCard { def: "sha", suit: Suit::Club, rank: 11 },
    PhysicalCard { def: "jiedao_sharen", suit: Suit::Club, rank: 12 },
    PhysicalCard { def: "wuxie_keji", suit: Suit::Club, rank: 12 },
    PhysicalCard { def: "jiedao_sharen", suit: Suit::Club, rank: 13 },
    PhysicalCard { def: "wuxie_keji", suit: Suit::Club, rank: 13 },
];

/// 标准包武将（`code` 与中台 `sgs_generals.code` 一致）。
#[derive(Debug, Clone, Copy)]
pub struct GeneralDef {
    pub code: &'static str,
    pub gid: u32,
    pub name: &'static str,
    pub kingdom: &'static str,
    pub max_hp: i32,
    /// 性别（male / female；用于离间、雌雄双股剑等）。
    pub gender: &'static str,
    /// 是否主公将（技能含主公技）。
    pub is_lord: bool,
    /// 技能名（P2.1 仅展示用；技能实现见 P2.4）。
    pub skills: &'static [&'static str],
}

#[rustfmt::skip]
pub static STANDARD_GENERALS: &[GeneralDef] = &[
    GeneralDef { code: "sgs_hero_1", gid: 1, name: "刘备", kingdom: "shu", max_hp: 4, gender: "male", is_lord: true, skills: &["仁德", "激将"],},
    GeneralDef { code: "sgs_hero_2", gid: 2, name: "关羽", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["武圣"],},
    GeneralDef { code: "sgs_hero_3", gid: 3, name: "张飞", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["咆哮"],},
    GeneralDef { code: "sgs_hero_4", gid: 4, name: "诸葛亮", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["观星", "空城"],},
    GeneralDef { code: "sgs_hero_5", gid: 5, name: "赵云", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["龙胆"],},
    GeneralDef { code: "sgs_hero_6", gid: 6, name: "马超", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["铁骑", "马术"],},
    GeneralDef { code: "sgs_hero_7", gid: 7, name: "黄月英", kingdom: "shu", max_hp: 3, gender: "female", is_lord: false, skills: &["集智", "奇才"],},
    GeneralDef { code: "sgs_hero_8", gid: 8, name: "孙权", kingdom: "wu", max_hp: 4, gender: "male", is_lord: true, skills: &["制衡", "救援"],},
    GeneralDef { code: "sgs_hero_9", gid: 9, name: "甘宁", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["奇袭"],},
    GeneralDef { code: "sgs_hero_10", gid: 10, name: "吕蒙", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["克己"],},
    GeneralDef { code: "sgs_hero_11", gid: 11, name: "黄盖", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["苦肉"],},
    GeneralDef { code: "sgs_hero_12", gid: 12, name: "周瑜", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["英姿", "反间"],},
    GeneralDef { code: "sgs_hero_13", gid: 13, name: "大乔", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["国色", "流离"],},
    GeneralDef { code: "sgs_hero_14", gid: 14, name: "陆逊", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["谦逊", "连营"],},
    GeneralDef { code: "sgs_hero_15", gid: 15, name: "曹操", kingdom: "wei", max_hp: 4, gender: "male", is_lord: true, skills: &["奸雄", "护驾"],},
    GeneralDef { code: "sgs_hero_16", gid: 16, name: "司马懿", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["反馈", "鬼才"],},
    GeneralDef { code: "sgs_hero_17", gid: 17, name: "夏侯惇", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["刚烈"],},
    GeneralDef { code: "sgs_hero_18", gid: 18, name: "张辽", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["突袭"],},
    GeneralDef { code: "sgs_hero_19", gid: 19, name: "许褚", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["裸衣"],},
    GeneralDef { code: "sgs_hero_20", gid: 20, name: "郭嘉", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["天妒", "遗计"],},
    GeneralDef { code: "sgs_hero_21", gid: 21, name: "甄姬", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["洛神", "倾国"],},
    GeneralDef { code: "sgs_hero_22", gid: 22, name: "华佗", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["急救", "青囊"],},
    GeneralDef { code: "sgs_hero_23", gid: 23, name: "吕布", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["无双"],},
    GeneralDef { code: "sgs_hero_24", gid: 24, name: "貂蝉", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["闭月", "离间"],},
    GeneralDef { code: "sgs_hero_25", gid: 25, name: "孙尚香", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["枭姬", "结姻"],},
];

pub fn card_def(code: &str) -> Option<&'static CardDef> {
    CARD_DEFS.iter().find(|d| d.code == code)
}

pub fn general(code: &str) -> Option<&'static GeneralDef> {
    STANDARD_GENERALS.iter().find(|g| g.code == code)
}

/// 全部武将码（按 gid 升序），用于选将池。
pub fn all_general_codes() -> Vec<&'static str> {
    STANDARD_GENERALS.iter().map(|g| g.code).collect()
}
