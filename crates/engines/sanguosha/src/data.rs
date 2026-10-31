//! 三国杀引擎内置数据（**自动生成，勿手改**）。
//!
//! 生成：`node apps/scripts/gen-sanguosha-engine-data.mjs`
//! 校验：`node apps/scripts/gen-sanguosha-engine-data.mjs --check`
//! 来源：`docs/games/sanguosha/data/cards.json`（standard + junzheng）+ `heroes.json`（按包生成）。

use crate::state::{Category, EquipSlot, RangeType, Suit, TargetMode};

pub const STANDARD_DECK_SIZE: usize = 108;
pub const JUNZHENG_DECK_SIZE: usize = 52;
/// 全部牌堆包合计（standard + junzheng = 160）。
pub const FULL_DECK_SIZE: usize = 160;
pub const STANDARD_GENERAL_COUNT: usize = 25;
#[allow(dead_code)]
pub const WIND_FIRE_FOREST_MOUNTAIN_GENERAL_COUNT: usize = 48;
#[allow(dead_code)]
pub const YIJIANG_GENERAL_COUNT: usize = 72;
#[allow(dead_code)]
pub const JIEXIAN_GENERAL_COUNT: usize = 87;
/// 已生成的全部武将数（所有包合计）。
pub const GENERAL_COUNT: usize = 232;

/// 支持的牌堆包（顺序即建堆顺序）。
pub static DECK_PACKS: &[&str] = &["standard", "junzheng"];

/// 牌的定义（同一 `code` 只有一条；物理牌见各 `*_DECK`）。
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
    CardDef { code: "huo_gong", name: "火攻", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: Some(TargetMode::Single), delayed: false,},
    CardDef { code: "huo_sha", name: "火杀", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "baiyin_shizi", name: "白银狮子", category: Category::Armor, equip_slot: Some(EquipSlot::Armor), range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "tengjia", name: "藤甲", category: Category::Armor, equip_slot: Some(EquipSlot::Armor), range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "jiu", name: "酒", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "bingliang_cunduan", name: "兵粮寸断", category: Category::DelayedTrick, equip_slot: None, range_value: Some(1), range_type: None, target_mode: None, delayed: true,},
    CardDef { code: "lei_sha", name: "雷杀", category: Category::Basic, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "tiesuo_lianhuan", name: "铁索连环", category: Category::Trick, equip_slot: None, range_value: None, range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "guding_dao", name: "古锭刀", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(2), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "zhuque_yushan", name: "朱雀羽扇", category: Category::Weapon, equip_slot: Some(EquipSlot::Weapon), range_value: Some(4), range_type: None, target_mode: None, delayed: false,},
    CardDef { code: "hualiu", name: "骅骝", category: Category::Horse, equip_slot: Some(EquipSlot::HorseDefense), range_value: Some(1), range_type: Some(RangeType::Defense), target_mode: None, delayed: false,},
];

/// 牌堆中的每一张物理牌（所属包 + 花色 + 点数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalCard {
    /// 所属牌堆包（`standard` / `junzheng`）。
    pub pack: &'static str,
    pub def: &'static str,
    pub suit: Suit,
    pub rank: u8,
}

/// 标准包牌堆（108 张）。
#[rustfmt::skip]
pub static STANDARD_DECK: &[PhysicalCard] = &[
    PhysicalCard { pack: "standard", def: "taoyuan_jieyi", suit: Suit::Heart, rank: 1 },
    PhysicalCard { pack: "standard", def: "wanjian_qifa", suit: Suit::Heart, rank: 1 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Heart, rank: 2 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Heart, rank: 2 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 3 },
    PhysicalCard { pack: "standard", def: "wugu_fengdeng", suit: Suit::Heart, rank: 3 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 4 },
    PhysicalCard { pack: "standard", def: "wugu_fengdeng", suit: Suit::Heart, rank: 4 },
    PhysicalCard { pack: "standard", def: "qilin_gong", suit: Suit::Heart, rank: 5 },
    PhysicalCard { pack: "standard", def: "chitu", suit: Suit::Heart, rank: 5 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 6 },
    PhysicalCard { pack: "standard", def: "lebu_sishu", suit: Suit::Heart, rank: 6 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 7 },
    PhysicalCard { pack: "standard", def: "wuzhong_shengyou", suit: Suit::Heart, rank: 7 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 8 },
    PhysicalCard { pack: "standard", def: "wuzhong_shengyou", suit: Suit::Heart, rank: 8 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 9 },
    PhysicalCard { pack: "standard", def: "wuzhong_shengyou", suit: Suit::Heart, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Heart, rank: 10 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Heart, rank: 10 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Heart, rank: 11 },
    PhysicalCard { pack: "standard", def: "wuzhong_shengyou", suit: Suit::Heart, rank: 11 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Heart, rank: 12 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Heart, rank: 12 },
    PhysicalCard { pack: "standard", def: "shandian", suit: Suit::Heart, rank: 12 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Heart, rank: 13 },
    PhysicalCard { pack: "standard", def: "zhuahuang_feidian", suit: Suit::Heart, rank: 13 },
    PhysicalCard { pack: "standard", def: "juedou", suit: Suit::Spade, rank: 1 },
    PhysicalCard { pack: "standard", def: "shandian", suit: Suit::Spade, rank: 1 },
    PhysicalCard { pack: "standard", def: "cixiong_shuanggujian", suit: Suit::Spade, rank: 2 },
    PhysicalCard { pack: "standard", def: "bagua_zhen", suit: Suit::Spade, rank: 2 },
    PhysicalCard { pack: "standard", def: "hanbing_jian", suit: Suit::Spade, rank: 2 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Spade, rank: 3 },
    PhysicalCard { pack: "standard", def: "shunshou_qianyang", suit: Suit::Spade, rank: 3 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Spade, rank: 4 },
    PhysicalCard { pack: "standard", def: "shunshou_qianyang", suit: Suit::Spade, rank: 4 },
    PhysicalCard { pack: "standard", def: "qinglong_yanyue", suit: Suit::Spade, rank: 5 },
    PhysicalCard { pack: "standard", def: "jueying", suit: Suit::Spade, rank: 5 },
    PhysicalCard { pack: "standard", def: "lebu_sishu", suit: Suit::Spade, rank: 6 },
    PhysicalCard { pack: "standard", def: "qinggang_jian", suit: Suit::Spade, rank: 6 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 7 },
    PhysicalCard { pack: "standard", def: "nanman_ruqin", suit: Suit::Spade, rank: 7 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 8 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 8 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 10 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Spade, rank: 10 },
    PhysicalCard { pack: "standard", def: "shunshou_qianyang", suit: Suit::Spade, rank: 11 },
    PhysicalCard { pack: "standard", def: "wuxie_keji", suit: Suit::Spade, rank: 11 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Spade, rank: 12 },
    PhysicalCard { pack: "standard", def: "zhangba_shemao", suit: Suit::Spade, rank: 12 },
    PhysicalCard { pack: "standard", def: "nanman_ruqin", suit: Suit::Spade, rank: 13 },
    PhysicalCard { pack: "standard", def: "dawan", suit: Suit::Spade, rank: 13 },
    PhysicalCard { pack: "standard", def: "zhuge_liannu", suit: Suit::Diamond, rank: 1 },
    PhysicalCard { pack: "standard", def: "juedou", suit: Suit::Diamond, rank: 1 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 2 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 2 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 3 },
    PhysicalCard { pack: "standard", def: "shunshou_qianyang", suit: Suit::Diamond, rank: 3 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 4 },
    PhysicalCard { pack: "standard", def: "shunshou_qianyang", suit: Suit::Diamond, rank: 4 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 5 },
    PhysicalCard { pack: "standard", def: "guanshi_fu", suit: Suit::Diamond, rank: 5 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 6 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 6 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 7 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 7 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 8 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 8 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 9 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 10 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 10 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 11 },
    PhysicalCard { pack: "standard", def: "shan", suit: Suit::Diamond, rank: 11 },
    PhysicalCard { pack: "standard", def: "tao", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { pack: "standard", def: "fangtian_huaji", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { pack: "standard", def: "wuxie_keji", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Diamond, rank: 13 },
    PhysicalCard { pack: "standard", def: "zixing", suit: Suit::Diamond, rank: 13 },
    PhysicalCard { pack: "standard", def: "juedou", suit: Suit::Club, rank: 1 },
    PhysicalCard { pack: "standard", def: "zhuge_liannu", suit: Suit::Club, rank: 1 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 2 },
    PhysicalCard { pack: "standard", def: "bagua_zhen", suit: Suit::Club, rank: 2 },
    PhysicalCard { pack: "standard", def: "renwang_dun", suit: Suit::Club, rank: 2 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 3 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Club, rank: 3 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 4 },
    PhysicalCard { pack: "standard", def: "guohe_chaiqiao", suit: Suit::Club, rank: 4 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 5 },
    PhysicalCard { pack: "standard", def: "dilu", suit: Suit::Club, rank: 5 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 6 },
    PhysicalCard { pack: "standard", def: "lebu_sishu", suit: Suit::Club, rank: 6 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 7 },
    PhysicalCard { pack: "standard", def: "nanman_ruqin", suit: Suit::Club, rank: 7 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 8 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 8 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 9 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 10 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 10 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 11 },
    PhysicalCard { pack: "standard", def: "sha", suit: Suit::Club, rank: 11 },
    PhysicalCard { pack: "standard", def: "jiedao_sharen", suit: Suit::Club, rank: 12 },
    PhysicalCard { pack: "standard", def: "wuxie_keji", suit: Suit::Club, rank: 12 },
    PhysicalCard { pack: "standard", def: "jiedao_sharen", suit: Suit::Club, rank: 13 },
    PhysicalCard { pack: "standard", def: "wuxie_keji", suit: Suit::Club, rank: 13 },
];

/// 军争包牌堆（52 张）。
#[rustfmt::skip]
pub static JUNZHENG_DECK: &[PhysicalCard] = &[
    PhysicalCard { pack: "junzheng", def: "wuxie_keji", suit: Suit::Heart, rank: 1 },
    PhysicalCard { pack: "junzheng", def: "huo_gong", suit: Suit::Heart, rank: 2 },
    PhysicalCard { pack: "junzheng", def: "huo_gong", suit: Suit::Heart, rank: 3 },
    PhysicalCard { pack: "junzheng", def: "huo_sha", suit: Suit::Heart, rank: 4 },
    PhysicalCard { pack: "junzheng", def: "tao", suit: Suit::Heart, rank: 5 },
    PhysicalCard { pack: "junzheng", def: "tao", suit: Suit::Heart, rank: 6 },
    PhysicalCard { pack: "junzheng", def: "huo_sha", suit: Suit::Heart, rank: 7 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Heart, rank: 8 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Heart, rank: 9 },
    PhysicalCard { pack: "junzheng", def: "huo_sha", suit: Suit::Heart, rank: 10 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Heart, rank: 11 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Heart, rank: 12 },
    PhysicalCard { pack: "junzheng", def: "wuxie_keji", suit: Suit::Heart, rank: 13 },
    PhysicalCard { pack: "junzheng", def: "baiyin_shizi", suit: Suit::Club, rank: 1 },
    PhysicalCard { pack: "junzheng", def: "tengjia", suit: Suit::Club, rank: 2 },
    PhysicalCard { pack: "junzheng", def: "jiu", suit: Suit::Club, rank: 3 },
    PhysicalCard { pack: "junzheng", def: "bingliang_cunduan", suit: Suit::Club, rank: 4 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Club, rank: 5 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Club, rank: 6 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Club, rank: 7 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Club, rank: 8 },
    PhysicalCard { pack: "junzheng", def: "jiu", suit: Suit::Club, rank: 9 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Club, rank: 10 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Club, rank: 11 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Club, rank: 12 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Club, rank: 13 },
    PhysicalCard { pack: "junzheng", def: "guding_dao", suit: Suit::Spade, rank: 1 },
    PhysicalCard { pack: "junzheng", def: "tengjia", suit: Suit::Spade, rank: 2 },
    PhysicalCard { pack: "junzheng", def: "jiu", suit: Suit::Spade, rank: 3 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Spade, rank: 4 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Spade, rank: 5 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Spade, rank: 6 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Spade, rank: 7 },
    PhysicalCard { pack: "junzheng", def: "lei_sha", suit: Suit::Spade, rank: 8 },
    PhysicalCard { pack: "junzheng", def: "jiu", suit: Suit::Spade, rank: 9 },
    PhysicalCard { pack: "junzheng", def: "bingliang_cunduan", suit: Suit::Spade, rank: 10 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Spade, rank: 11 },
    PhysicalCard { pack: "junzheng", def: "tiesuo_lianhuan", suit: Suit::Spade, rank: 12 },
    PhysicalCard { pack: "junzheng", def: "wuxie_keji", suit: Suit::Spade, rank: 13 },
    PhysicalCard { pack: "junzheng", def: "zhuque_yushan", suit: Suit::Diamond, rank: 1 },
    PhysicalCard { pack: "junzheng", def: "tao", suit: Suit::Diamond, rank: 2 },
    PhysicalCard { pack: "junzheng", def: "tao", suit: Suit::Diamond, rank: 3 },
    PhysicalCard { pack: "junzheng", def: "huo_sha", suit: Suit::Diamond, rank: 4 },
    PhysicalCard { pack: "junzheng", def: "huo_sha", suit: Suit::Diamond, rank: 5 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Diamond, rank: 6 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Diamond, rank: 7 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Diamond, rank: 8 },
    PhysicalCard { pack: "junzheng", def: "jiu", suit: Suit::Diamond, rank: 9 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Diamond, rank: 10 },
    PhysicalCard { pack: "junzheng", def: "shan", suit: Suit::Diamond, rank: 11 },
    PhysicalCard { pack: "junzheng", def: "huo_gong", suit: Suit::Diamond, rank: 12 },
    PhysicalCard { pack: "junzheng", def: "hualiu", suit: Suit::Diamond, rank: 13 },
];

/// 全部牌堆包（顺序即建堆顺序）。
#[rustfmt::skip]
pub static ALL_DECKS: &[(&str, &[PhysicalCard])] = &[
    ("standard", STANDARD_DECK),
    ("junzheng", JUNZHENG_DECK),
];

/// 武将（`code` 与中台 `sgs_generals.code` 一致；`pack` 为武将包 code）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralDef {
    pub code: &'static str,
    pub gid: u32,
    pub name: &'static str,
    /// 武将包 code（`standard` / `wind_fire_forest_mountain`；选将池按 `Config.general_packs` 过滤）。
    pub pack: &'static str,
    pub kingdom: &'static str,
    pub max_hp: i32,
    /// 性别（male / female；用于离间、雌雄双股剑等）。
    pub gender: &'static str,
    /// 是否主公将（技能含主公技）。
    pub is_lord: bool,
    /// 技能名（展示与技能注册表对照用）。
    pub skills: &'static [&'static str],
}

/// 标准包武将（25 位）。
#[rustfmt::skip]
pub static STANDARD_GENERALS: &[GeneralDef] = &[
    GeneralDef { code: "sgs_hero_1", gid: 1, name: "刘备", pack: "standard", kingdom: "shu", max_hp: 4, gender: "male", is_lord: true, skills: &["仁德", "激将"],},
    GeneralDef { code: "sgs_hero_2", gid: 2, name: "关羽", pack: "standard", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["武圣"],},
    GeneralDef { code: "sgs_hero_3", gid: 3, name: "张飞", pack: "standard", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["咆哮"],},
    GeneralDef { code: "sgs_hero_4", gid: 4, name: "诸葛亮", pack: "standard", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["观星", "空城"],},
    GeneralDef { code: "sgs_hero_5", gid: 5, name: "赵云", pack: "standard", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["龙胆"],},
    GeneralDef { code: "sgs_hero_6", gid: 6, name: "马超", pack: "standard", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["铁骑", "马术"],},
    GeneralDef { code: "sgs_hero_7", gid: 7, name: "黄月英", pack: "standard", kingdom: "shu", max_hp: 3, gender: "female", is_lord: false, skills: &["集智", "奇才"],},
    GeneralDef { code: "sgs_hero_8", gid: 8, name: "孙权", pack: "standard", kingdom: "wu", max_hp: 4, gender: "male", is_lord: true, skills: &["制衡", "救援"],},
    GeneralDef { code: "sgs_hero_9", gid: 9, name: "甘宁", pack: "standard", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["奇袭"],},
    GeneralDef { code: "sgs_hero_10", gid: 10, name: "吕蒙", pack: "standard", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["克己"],},
    GeneralDef { code: "sgs_hero_11", gid: 11, name: "黄盖", pack: "standard", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["苦肉"],},
    GeneralDef { code: "sgs_hero_12", gid: 12, name: "周瑜", pack: "standard", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["英姿", "反间"],},
    GeneralDef { code: "sgs_hero_13", gid: 13, name: "大乔", pack: "standard", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["国色", "流离"],},
    GeneralDef { code: "sgs_hero_14", gid: 14, name: "陆逊", pack: "standard", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["谦逊", "连营"],},
    GeneralDef { code: "sgs_hero_15", gid: 15, name: "曹操", pack: "standard", kingdom: "wei", max_hp: 4, gender: "male", is_lord: true, skills: &["奸雄", "护驾"],},
    GeneralDef { code: "sgs_hero_16", gid: 16, name: "司马懿", pack: "standard", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["反馈", "鬼才"],},
    GeneralDef { code: "sgs_hero_17", gid: 17, name: "夏侯惇", pack: "standard", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["刚烈"],},
    GeneralDef { code: "sgs_hero_18", gid: 18, name: "张辽", pack: "standard", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["突袭"],},
    GeneralDef { code: "sgs_hero_19", gid: 19, name: "许褚", pack: "standard", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["裸衣"],},
    GeneralDef { code: "sgs_hero_20", gid: 20, name: "郭嘉", pack: "standard", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["天妒", "遗计"],},
    GeneralDef { code: "sgs_hero_21", gid: 21, name: "甄姬", pack: "standard", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["洛神", "倾国"],},
    GeneralDef { code: "sgs_hero_22", gid: 22, name: "华佗", pack: "standard", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["急救", "青囊"],},
    GeneralDef { code: "sgs_hero_23", gid: 23, name: "吕布", pack: "standard", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["无双"],},
    GeneralDef { code: "sgs_hero_24", gid: 24, name: "貂蝉", pack: "standard", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["闭月", "离间"],},
    GeneralDef { code: "sgs_hero_25", gid: 25, name: "孙尚香", pack: "standard", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["枭姬", "结姻"],},
];

/// 风林火山包武将（48 位）。
#[rustfmt::skip]
pub static WIND_FIRE_FOREST_MOUNTAIN_GENERALS: &[GeneralDef] = &[
    GeneralDef { code: "sgs_hero_27", gid: 27, name: "魏延", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["狂骨", "奇谋"],},
    GeneralDef { code: "sgs_hero_28", gid: 28, name: "夏侯渊", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["神速"],},
    GeneralDef { code: "sgs_hero_29", gid: 29, name: "曹仁", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["据守", "解围"],},
    GeneralDef { code: "sgs_hero_30", gid: 30, name: "小乔", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["红颜", "天香"],},
    GeneralDef { code: "sgs_hero_34", gid: 34, name: "典韦", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["强袭"],},
    GeneralDef { code: "sgs_hero_35", gid: 35, name: "荀彧", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["驱虎", "节命"],},
    GeneralDef { code: "sgs_hero_36", gid: 36, name: "庞统", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["连环", "涅槃"],},
    GeneralDef { code: "sgs_hero_37", gid: 37, name: "卧龙诸葛亮", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["八阵", "火计", "看破"],},
    GeneralDef { code: "sgs_hero_38", gid: 38, name: "太史慈", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["天义"],},
    GeneralDef { code: "sgs_hero_39", gid: 39, name: "庞德", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["马术", "鞬出"],},
    GeneralDef { code: "sgs_hero_40", gid: 40, name: "颜良文丑", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["双雄"],},
    GeneralDef { code: "sgs_hero_41", gid: 41, name: "袁绍", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 4, gender: "male", is_lord: true, skills: &["乱击", "血裔"],},
    GeneralDef { code: "sgs_hero_42", gid: 42, name: "徐晃", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["断粮", "截辎"],},
    GeneralDef { code: "sgs_hero_43", gid: 43, name: "曹丕", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 3, gender: "male", is_lord: true, skills: &["行殇", "放逐", "颂威"],},
    GeneralDef { code: "sgs_hero_44", gid: 44, name: "孙坚", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["英魂"],},
    GeneralDef { code: "sgs_hero_45", gid: 45, name: "董卓", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 8, gender: "male", is_lord: true, skills: &["酒池", "肉林", "崩坏", "暴虐"],},
    GeneralDef { code: "sgs_hero_46", gid: 46, name: "祝融", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "female", is_lord: false, skills: &["巨象", "烈刃"],},
    GeneralDef { code: "sgs_hero_47", gid: 47, name: "孟获", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["祸首", "再起"],},
    GeneralDef { code: "sgs_hero_48", gid: 48, name: "贾诩", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["完杀", "乱武", "帷幕"],},
    GeneralDef { code: "sgs_hero_49", gid: 49, name: "鲁肃", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["好施", "缔盟"],},
    GeneralDef { code: "sgs_hero_50", gid: 50, name: "张郃", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["巧变"],},
    GeneralDef { code: "sgs_hero_51", gid: 51, name: "邓艾", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["屯田", "凿险"],},
    GeneralDef { code: "sgs_hero_52", gid: 52, name: "姜维", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["挑衅", "志继"],},
    GeneralDef { code: "sgs_hero_53", gid: 53, name: "刘禅", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 3, gender: "male", is_lord: true, skills: &["享乐", "放权", "若愚"],},
    GeneralDef { code: "sgs_hero_54", gid: 54, name: "孙策", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 4, gender: "male", is_lord: true, skills: &["激昂", "魂姿", "制霸"],},
    GeneralDef { code: "sgs_hero_55", gid: 55, name: "张昭张弘", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["直谏", "固政"],},
    GeneralDef { code: "sgs_hero_56", gid: 56, name: "左慈", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["化身", "新生"],},
    GeneralDef { code: "sgs_hero_57", gid: 57, name: "蔡文姬", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["悲歌", "断肠"],},
    GeneralDef { code: "sgs_hero_61", gid: 61, name: "黄忠", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["烈弓"],},
    GeneralDef { code: "sgs_hero_100", gid: 100, name: "袁术", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["庸肆", "伪帝"],},
    GeneralDef { code: "sgs_hero_210", gid: 210, name: "周泰", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["不屈", "奋激"],},
    GeneralDef { code: "sgs_hero_211", gid: 211, name: "于吉", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["蛊惑"],},
    GeneralDef { code: "sgs_hero_212", gid: 212, name: "张角", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: true, skills: &["鬼道", "黄天", "雷击"],},
    GeneralDef { code: "sgs_hero_362", gid: 362, name: "王基", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["奇制", "进趋"],},
    GeneralDef { code: "sgs_hero_400", gid: 400, name: "严颜", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["拒战"],},
    GeneralDef { code: "sgs_hero_401", gid: 401, name: "王平", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["飞军", "兵略"],},
    GeneralDef { code: "sgs_hero_402", gid: 402, name: "陆绩", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["怀橘", "遗礼", "整论"],},
    GeneralDef { code: "sgs_hero_403", gid: 403, name: "孙亮", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "male", is_lord: true, skills: &["溃诛", "掣政", "立军"],},
    GeneralDef { code: "sgs_hero_404", gid: 404, name: "蒯越蒯良", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["荐降", "审时"],},
    GeneralDef { code: "sgs_hero_406", gid: 406, name: "许攸", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["成略", "恃才", "寸目"],},
    GeneralDef { code: "sgs_hero_407", gid: 407, name: "卢植", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["明任", "贞良"],},
    GeneralDef { code: "sgs_hero_408", gid: 408, name: "郝昭", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["镇骨"],},
    GeneralDef { code: "sgs_hero_409", gid: 409, name: "陈到", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["往烈"],},
    GeneralDef { code: "sgs_hero_410", gid: 410, name: "诸葛瞻", pack: "wind_fire_forest_mountain", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["罪论", "父荫"],},
    GeneralDef { code: "sgs_hero_411", gid: 411, name: "周妃", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["良姻", "箜声"],},
    GeneralDef { code: "sgs_hero_413", gid: 413, name: "毌丘俭", pack: "wind_fire_forest_mountain", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["征荣", "鸿举"],},
    GeneralDef { code: "sgs_hero_414", gid: 414, name: "陆抗", pack: "wind_fire_forest_mountain", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["谦节", "决堰", "破势"],},
    GeneralDef { code: "sgs_hero_415", gid: 415, name: "张绣", pack: "wind_fire_forest_mountain", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["雄乱", "从谏"],},
];

/// 一将成名包武将（72 位）。
#[rustfmt::skip]
pub static YIJIANG_GENERALS: &[GeneralDef] = &[
    GeneralDef { code: "sgs_hero_151", gid: 151, name: "曹植", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["落英", "酒诗"],},
    GeneralDef { code: "sgs_hero_152", gid: 152, name: "高顺", pack: "yijiang", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["陷阵", "禁酒"],},
    GeneralDef { code: "sgs_hero_153", gid: 153, name: "陈宫", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["明策", "智迟"],},
    GeneralDef { code: "sgs_hero_155", gid: 155, name: "凌统", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["旋风"],},
    GeneralDef { code: "sgs_hero_156", gid: 156, name: "马谡", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["散谣", "制蛮"],},
    GeneralDef { code: "sgs_hero_157", gid: 157, name: "吴国太", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["甘露", "补益"],},
    GeneralDef { code: "sgs_hero_158", gid: 158, name: "徐盛", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["破军"],},
    GeneralDef { code: "sgs_hero_160", gid: 160, name: "于禁", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["镇军"],},
    GeneralDef { code: "sgs_hero_161", gid: 161, name: "张春华", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["绝情", "伤逝"],},
    GeneralDef { code: "sgs_hero_162", gid: 162, name: "曹彰", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["将驰"],},
    GeneralDef { code: "sgs_hero_164", gid: 164, name: "荀攸", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["奇策", "智愚"],},
    GeneralDef { code: "sgs_hero_166", gid: 166, name: "廖化", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["当先", "伏枥"],},
    GeneralDef { code: "sgs_hero_167", gid: 167, name: "马岱", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["马术", "潜袭"],},
    GeneralDef { code: "sgs_hero_168", gid: 168, name: "步练师", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["安恤", "追忆"],},
    GeneralDef { code: "sgs_hero_169", gid: 169, name: "程普", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["疠火", "醇醪"],},
    GeneralDef { code: "sgs_hero_170", gid: 170, name: "韩当", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["弓骑", "解烦"],},
    GeneralDef { code: "sgs_hero_171", gid: 171, name: "将华雄", pack: "yijiang", kingdom: "qun", max_hp: 6, gender: "male", is_lord: false, skills: &["恃勇"],},
    GeneralDef { code: "sgs_hero_172", gid: 172, name: "刘表", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["自守", "宗室"],},
    GeneralDef { code: "sgs_hero_173", gid: 173, name: "钟会", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["权计", "自立"],},
    GeneralDef { code: "sgs_hero_174", gid: 174, name: "曹冲", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["称象", "仁心"],},
    GeneralDef { code: "sgs_hero_175", gid: 175, name: "郭淮", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["精策"],},
    GeneralDef { code: "sgs_hero_176", gid: 176, name: "满宠", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["峻刑", "御策"],},
    GeneralDef { code: "sgs_hero_177", gid: 177, name: "关平", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["龙吟"],},
    GeneralDef { code: "sgs_hero_178", gid: 178, name: "简雍", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["巧说", "纵适"],},
    GeneralDef { code: "sgs_hero_179", gid: 179, name: "刘封", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["陷嗣"],},
    GeneralDef { code: "sgs_hero_180", gid: 180, name: "潘璋马忠", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["夺刀", "暗箭"],},
    GeneralDef { code: "sgs_hero_181", gid: 181, name: "虞翻", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["纵玄", "直言"],},
    GeneralDef { code: "sgs_hero_182", gid: 182, name: "朱然", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["胆守"],},
    GeneralDef { code: "sgs_hero_183", gid: 183, name: "伏皇后", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["惴恐", "求援"],},
    GeneralDef { code: "sgs_hero_184", gid: 184, name: "李儒", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["灭计", "绝策", "焚城"],},
    GeneralDef { code: "sgs_hero_185", gid: 185, name: "法正", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["恩怨", "眩惑"],},
    GeneralDef { code: "sgs_hero_186", gid: 186, name: "徐庶", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["无言", "举荐"],},
    GeneralDef { code: "sgs_hero_187", gid: 187, name: "王异", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["贞烈", "秘计"],},
    GeneralDef { code: "sgs_hero_188", gid: 188, name: "关兴张苞", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["父魂"],},
    GeneralDef { code: "sgs_hero_321", gid: 321, name: "曹真", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["司敌"],},
    GeneralDef { code: "sgs_hero_322", gid: 322, name: "韩浩史涣", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["慎断", "勇略"],},
    GeneralDef { code: "sgs_hero_323", gid: 323, name: "陈群", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["品第", "法恩"],},
    GeneralDef { code: "sgs_hero_324", gid: 324, name: "吴懿", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["奔袭"],},
    GeneralDef { code: "sgs_hero_325", gid: 325, name: "周仓", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["忠勇"],},
    GeneralDef { code: "sgs_hero_326", gid: 326, name: "张松", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["强识", "献图"],},
    GeneralDef { code: "sgs_hero_327", gid: 327, name: "孙鲁班", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["谮毁", "骄矜"],},
    GeneralDef { code: "sgs_hero_328", gid: 328, name: "朱桓", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["奋励", "平寇"],},
    GeneralDef { code: "sgs_hero_329", gid: 329, name: "顾雍", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["慎行", "秉壹"],},
    GeneralDef { code: "sgs_hero_330", gid: 330, name: "沮授", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["渐营", "矢北"],},
    GeneralDef { code: "sgs_hero_331", gid: 331, name: "蔡夫人", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["窃听", "献州"],},
    GeneralDef { code: "sgs_hero_332", gid: 332, name: "曹叡", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: true, skills: &["恢拓", "明鉴", "兴衰"],},
    GeneralDef { code: "sgs_hero_333", gid: 333, name: "曹休", pack: "yijiang", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["千驹", "倾袭"],},
    GeneralDef { code: "sgs_hero_334", gid: 334, name: "钟繇", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["活墨", "佐定"],},
    GeneralDef { code: "sgs_hero_335", gid: 335, name: "刘谌", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: true, skills: &["战绝", "勤王"],},
    GeneralDef { code: "sgs_hero_336", gid: 336, name: "夏侯氏", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "female", is_lord: false, skills: &["樵拾", "燕语"],},
    GeneralDef { code: "sgs_hero_337", gid: 337, name: "张嶷", pack: "yijiang", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["怃戎", "矢志"],},
    GeneralDef { code: "sgs_hero_338", gid: 338, name: "孙休", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "male", is_lord: true, skills: &["宴诛", "兴学", "诏缚"],},
    GeneralDef { code: "sgs_hero_339", gid: 339, name: "朱治", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["安国"],},
    GeneralDef { code: "sgs_hero_340", gid: 340, name: "全琮", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["邀名"],},
    GeneralDef { code: "sgs_hero_341", gid: 341, name: "公孙渊", pack: "yijiang", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["怀异"],},
    GeneralDef { code: "sgs_hero_342", gid: 342, name: "郭图逢纪", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["急攻", "饰非"],},
    GeneralDef { code: "sgs_hero_380", gid: 380, name: "郭皇后", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["矫诏", "殚心"],},
    GeneralDef { code: "sgs_hero_381", gid: 381, name: "李严", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["督粮", "腹鳞"],},
    GeneralDef { code: "sgs_hero_382", gid: 382, name: "孙登", pack: "yijiang", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["匡弼"],},
    GeneralDef { code: "sgs_hero_383", gid: 383, name: "刘虞", pack: "yijiang", kingdom: "qun", max_hp: 2, gender: "male", is_lord: false, skills: &["止戈", "宗祚"],},
    GeneralDef { code: "sgs_hero_384", gid: 384, name: "岑昏", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["极奢", "链祸"],},
    GeneralDef { code: "sgs_hero_385", gid: 385, name: "孙资刘放", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["瑰藻", "讥谀"],},
    GeneralDef { code: "sgs_hero_386", gid: 386, name: "黄皓", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["寝情", "贿生"],},
    GeneralDef { code: "sgs_hero_387", gid: 387, name: "张让", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["滔乱"],},
    GeneralDef { code: "sgs_hero_388", gid: 388, name: "辛宪英", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["忠鉴", "才识"],},
    GeneralDef { code: "sgs_hero_389", gid: 389, name: "吴苋", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "female", is_lord: false, skills: &["福绵", "怠宴"],},
    GeneralDef { code: "sgs_hero_390", gid: 390, name: "徐氏", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["问卦", "伏诛"],},
    GeneralDef { code: "sgs_hero_391", gid: 391, name: "曹节", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["守玺", "惠民"],},
    GeneralDef { code: "sgs_hero_392", gid: 392, name: "嵇康", pack: "yijiang", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["清弦", "绝响"],},
    GeneralDef { code: "sgs_hero_393", gid: 393, name: "秦宓", pack: "yijiang", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["谏征", "专对", "天辩"],},
    GeneralDef { code: "sgs_hero_394", gid: 394, name: "薛综", pack: "yijiang", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["复难", "诫训"],},
    GeneralDef { code: "sgs_hero_395", gid: 395, name: "蔡邕", pack: "yijiang", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["辟撰", "通博"],},
];

/// 界限突破包武将（87 位）。
#[rustfmt::skip]
pub static JIEXIAN_GENERALS: &[GeneralDef] = &[
    GeneralDef { code: "sgs_hero_26", gid: 26, name: "界黄忠", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["烈弓"],},
    GeneralDef { code: "sgs_hero_299", gid: 299, name: "界刘备", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: true, skills: &["仁德", "激将"],},
    GeneralDef { code: "sgs_hero_300", gid: 300, name: "界关羽", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["义绝", "武圣"],},
    GeneralDef { code: "sgs_hero_301", gid: 301, name: "界张飞", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["替身", "咆哮"],},
    GeneralDef { code: "sgs_hero_302", gid: 302, name: "界赵云", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["涯角", "龙胆"],},
    GeneralDef { code: "sgs_hero_303", gid: 303, name: "界马超", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["马术", "铁骑"],},
    GeneralDef { code: "sgs_hero_304", gid: 304, name: "界徐庶", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["诛害", "潜心"],},
    GeneralDef { code: "sgs_hero_305", gid: 305, name: "界甘宁", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["奇袭", "奋威"],},
    GeneralDef { code: "sgs_hero_306", gid: 306, name: "界吕蒙", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["克己", "勤学", "博图"],},
    GeneralDef { code: "sgs_hero_307", gid: 307, name: "界黄盖", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["苦肉", "诈降"],},
    GeneralDef { code: "sgs_hero_308", gid: 308, name: "界周瑜", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["英姿", "反间"],},
    GeneralDef { code: "sgs_hero_309", gid: 309, name: "界大乔", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["流离", "国色"],},
    GeneralDef { code: "sgs_hero_310", gid: 310, name: "界陆逊", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["谦逊", "连营"],},
    GeneralDef { code: "sgs_hero_311", gid: 311, name: "界曹操", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: true, skills: &["奸雄", "护驾"],},
    GeneralDef { code: "sgs_hero_312", gid: 312, name: "界司马懿", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["反馈", "鬼才"],},
    GeneralDef { code: "sgs_hero_313", gid: 313, name: "界夏侯惇", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["刚烈", "清俭"],},
    GeneralDef { code: "sgs_hero_314", gid: 314, name: "界张辽", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["突袭"],},
    GeneralDef { code: "sgs_hero_315", gid: 315, name: "界许褚", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["裸衣"],},
    GeneralDef { code: "sgs_hero_316", gid: 316, name: "界郭嘉", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["天妒", "遗计"],},
    GeneralDef { code: "sgs_hero_317", gid: 317, name: "界李典", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["恂恂", "忘隙"],},
    GeneralDef { code: "sgs_hero_318", gid: 318, name: "界华佗", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["急救", "除疠"],},
    GeneralDef { code: "sgs_hero_319", gid: 319, name: "界吕布", pack: "jiexian", kingdom: "qun", max_hp: 5, gender: "male", is_lord: false, skills: &["无双", "利驭"],},
    GeneralDef { code: "sgs_hero_320", gid: 320, name: "界公孙瓒", pack: "jiexian", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["义从", "趫猛"],},
    GeneralDef { code: "sgs_hero_440", gid: 440, name: "界诸葛亮", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["空城", "观星"],},
    GeneralDef { code: "sgs_hero_441", gid: 441, name: "界黄月英", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["集智", "奇才"],},
    GeneralDef { code: "sgs_hero_442", gid: 442, name: "界孙权", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: true, skills: &["制衡", "救援"],},
    GeneralDef { code: "sgs_hero_443", gid: 443, name: "界孙尚香", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["枭姬", "结姻"],},
    GeneralDef { code: "sgs_hero_444", gid: 444, name: "界甄姬", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["倾国", "洛神"],},
    GeneralDef { code: "sgs_hero_445", gid: 445, name: "界貂蝉", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["离间", "闭月"],},
    GeneralDef { code: "sgs_hero_446", gid: 446, name: "界华雄", pack: "jiexian", kingdom: "qun", max_hp: 6, gender: "male", is_lord: false, skills: &["耀武", "势斩"],},
    GeneralDef { code: "sgs_hero_447", gid: 447, name: "界左慈", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["化身", "新生"],},
    GeneralDef { code: "sgs_hero_448", gid: 448, name: "界张角", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: true, skills: &["雷击", "鬼道", "黄天"],},
    GeneralDef { code: "sgs_hero_449", gid: 449, name: "界于吉", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["蛊惑"],},
    GeneralDef { code: "sgs_hero_450", gid: 450, name: "界袁绍", pack: "jiexian", kingdom: "qun", max_hp: 4, gender: "male", is_lord: true, skills: &["乱击", "血裔"],},
    GeneralDef { code: "sgs_hero_451", gid: 451, name: "界刘禅", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: true, skills: &["享乐", "放权", "若愚"],},
    GeneralDef { code: "sgs_hero_452", gid: 452, name: "界孙策", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: true, skills: &["制霸", "激昂", "魂姿"],},
    GeneralDef { code: "sgs_hero_453", gid: 453, name: "界夏侯渊", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["神速", "设变"],},
    GeneralDef { code: "sgs_hero_454", gid: 454, name: "界卧龙诸葛亮", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["八阵", "藏拙", "火计", "看破"],},
    GeneralDef { code: "sgs_hero_455", gid: 455, name: "界庞统", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["连环", "涅槃"],},
    GeneralDef { code: "sgs_hero_456", gid: 456, name: "界魏延", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["狂骨", "奇谋"],},
    GeneralDef { code: "sgs_hero_457", gid: 457, name: "界小乔", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["红颜", "天香", "飘零"],},
    GeneralDef { code: "sgs_hero_458", gid: 458, name: "界孙坚", pack: "jiexian", kingdom: "wu", max_hp: 5, gender: "male", is_lord: false, skills: &["英魂", "武烈"],},
    GeneralDef { code: "sgs_hero_462", gid: 462, name: "界庞德", pack: "jiexian", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["马术", "鞬出"],},
    GeneralDef { code: "sgs_hero_463", gid: 463, name: "界太史慈", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["天义", "酣战"],},
    GeneralDef { code: "sgs_hero_470", gid: 470, name: "界董卓", pack: "jiexian", kingdom: "qun", max_hp: 8, gender: "male", is_lord: true, skills: &["肉林", "崩坏", "酒池", "暴虐"],},
    GeneralDef { code: "sgs_hero_471", gid: 471, name: "界邓艾", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["屯田", "凿险"],},
    GeneralDef { code: "sgs_hero_479", gid: 479, name: "界祝融", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "female", is_lord: false, skills: &["巨象", "烈刃", "长标"],},
    GeneralDef { code: "sgs_hero_480", gid: 480, name: "界姜维", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["挑衅", "志继"],},
    GeneralDef { code: "sgs_hero_484", gid: 484, name: "界贾诩", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["帷幕", "乱武", "完杀"],},
    GeneralDef { code: "sgs_hero_485", gid: 485, name: "界鲁肃", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["好施", "缔盟"],},
    GeneralDef { code: "sgs_hero_486", gid: 486, name: "界张郃", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["巧变"],},
    GeneralDef { code: "sgs_hero_492", gid: 492, name: "界孟获", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["祸首", "再起"],},
    GeneralDef { code: "sgs_hero_496", gid: 496, name: "界徐晃", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["断粮", "截辎"],},
    GeneralDef { code: "sgs_hero_505", gid: 505, name: "界蔡文姬", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["断肠", "悲歌"],},
    GeneralDef { code: "sgs_hero_515", gid: 515, name: "界荀彧", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["驱虎", "节命"],},
    GeneralDef { code: "sgs_hero_526", gid: 526, name: "界典韦", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["强袭", "狞恶"],},
    GeneralDef { code: "sgs_hero_550", gid: 550, name: "界颜良文丑", pack: "jiexian", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["双雄"],},
    GeneralDef { code: "sgs_hero_563", gid: 563, name: "界张昭张纮", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["直谏", "固政"],},
    GeneralDef { code: "sgs_hero_587", gid: 587, name: "界徐盛", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["破军"],},
    GeneralDef { code: "sgs_hero_588", gid: 588, name: "界凌统", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["旋风"],},
    GeneralDef { code: "sgs_hero_597", gid: 597, name: "界陈宫", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["智迟", "明策"],},
    GeneralDef { code: "sgs_hero_599", gid: 599, name: "界曹彰", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["将驰"],},
    GeneralDef { code: "sgs_hero_600", gid: 600, name: "界简雍", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["纵适", "巧说"],},
    GeneralDef { code: "sgs_hero_603", gid: 603, name: "界虞翻", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "male", is_lord: false, skills: &["纵玄", "直言"],},
    GeneralDef { code: "sgs_hero_604", gid: 604, name: "界高顺", pack: "jiexian", kingdom: "qun", max_hp: 4, gender: "male", is_lord: false, skills: &["陷阵", "禁酒"],},
    GeneralDef { code: "sgs_hero_610", gid: 610, name: "界法正", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["眩惑", "恩怨"],},
    GeneralDef { code: "sgs_hero_620", gid: 620, name: "界程普", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["疠火", "醇醪"],},
    GeneralDef { code: "sgs_hero_621", gid: 621, name: "界王异", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["贞烈", "秘计"],},
    GeneralDef { code: "sgs_hero_625", gid: 625, name: "界张春华", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["绝情", "伤逝", "翦灭"],},
    GeneralDef { code: "sgs_hero_626", gid: 626, name: "界刘表", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["自守", "宗室"],},
    GeneralDef { code: "sgs_hero_627", gid: 627, name: "界蔡夫人", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["献州", "窃听"],},
    GeneralDef { code: "sgs_hero_628", gid: 628, name: "界曹冲", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["称象", "仁心"],},
    GeneralDef { code: "sgs_hero_629", gid: 629, name: "界曹植", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "male", is_lord: false, skills: &["落英", "酒诗"],},
    GeneralDef { code: "sgs_hero_640", gid: 640, name: "界吴国太", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["甘露", "补益"],},
    GeneralDef { code: "sgs_hero_641", gid: 641, name: "界廖化", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["当先", "伏枥"],},
    GeneralDef { code: "sgs_hero_642", gid: 642, name: "界李儒", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["灭计", "焚城", "绝策"],},
    GeneralDef { code: "sgs_hero_658", gid: 658, name: "界郭淮", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["精策"],},
    GeneralDef { code: "sgs_hero_671", gid: 671, name: "界伏皇后", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "female", is_lord: false, skills: &["惴恐", "求援"],},
    GeneralDef { code: "sgs_hero_676", gid: 676, name: "界韩当", pack: "jiexian", kingdom: "wu", max_hp: 4, gender: "male", is_lord: false, skills: &["弓骑", "解烦"],},
    GeneralDef { code: "sgs_hero_690", gid: 690, name: "界关兴张苞", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["父魂"],},
    GeneralDef { code: "sgs_hero_696", gid: 696, name: "界马岱", pack: "jiexian", kingdom: "shu", max_hp: 4, gender: "male", is_lord: false, skills: &["马术", "潜袭"],},
    GeneralDef { code: "sgs_hero_726", gid: 726, name: "界郭皇后", pack: "jiexian", kingdom: "wei", max_hp: 3, gender: "female", is_lord: false, skills: &["矫诏", "殚心"],},
    GeneralDef { code: "sgs_hero_749", gid: 749, name: "界沮授", pack: "jiexian", kingdom: "qun", max_hp: 3, gender: "male", is_lord: false, skills: &["矢北", "渐营"],},
    GeneralDef { code: "sgs_hero_752", gid: 752, name: "界夏侯氏", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "female", is_lord: false, skills: &["樵拾", "燕语"],},
    GeneralDef { code: "sgs_hero_753", gid: 753, name: "界张松", pack: "jiexian", kingdom: "shu", max_hp: 3, gender: "male", is_lord: false, skills: &["强识", "献图"],},
    GeneralDef { code: "sgs_hero_756", gid: 756, name: "界于禁", pack: "jiexian", kingdom: "wei", max_hp: 4, gender: "male", is_lord: false, skills: &["镇军", "毅重"],},
    GeneralDef { code: "sgs_hero_775", gid: 775, name: "界步练师", pack: "jiexian", kingdom: "wu", max_hp: 3, gender: "female", is_lord: false, skills: &["安恤", "追忆"],},
];

/// 已生成的武将包（按包顺序拼接即选将池）。
#[rustfmt::skip]
pub static GENERAL_PACKS: &[&[GeneralDef]] = &[
    STANDARD_GENERALS,
    WIND_FIRE_FOREST_MOUNTAIN_GENERALS,
    YIJIANG_GENERALS,
    JIEXIAN_GENERALS,
];

/// 支持的武将包 key（顺序即选将池包顺序，用于 `Config.general_packs` 归一化）。
pub static GENERAL_PACK_KEYS: &[&str] = &["standard", "wind_fire_forest_mountain", "yijiang", "jiexian"];

/// 按 `Config.packs` 组合牌堆（顺序固定：standard → junzheng）。
///
/// 未知包或空组合返回错误（`lobby:set_packs` 与建堆共用同一套校验）。
pub fn deck_cards(packs: &[String]) -> Result<Vec<PhysicalCard>, String> {
    if packs.is_empty() {
        return Err("牌堆包组合不能为空".into());
    }
    for pack in packs {
        if !DECK_PACKS.contains(&pack.as_str()) {
            return Err(format!("未知牌堆包：{pack}"));
        }
    }
    let mut out = Vec::new();
    for (name, deck) in ALL_DECKS {
        if packs.iter().any(|p| p == name) {
            // 生成脚本契约：同一牌堆里的物理牌 `pack` 必须与牌堆名一致。
            if let Some(bad) = deck.iter().find(|c| c.pack != *name) {
                return Err(format!("牌堆 {name} 含跨包牌：{}", bad.def));
            }
            out.extend_from_slice(deck);
        }
    }
    Ok(out)
}

pub fn card_def(code: &str) -> Option<&'static CardDef> {
    CARD_DEFS.iter().find(|d| d.code == code)
}

pub fn general(code: &str) -> Option<&'static GeneralDef> {
    GENERAL_PACKS
        .iter()
        .flat_map(|p| p.iter())
        .find(|g| g.code == code)
}

/// 全部武将码（按包顺序 + gid 升序；调试/断言用）。
#[allow(dead_code)]
pub fn all_general_codes() -> Vec<&'static str> {
    GENERAL_PACKS
        .iter()
        .flat_map(|p| p.iter())
        .map(|g| g.code)
        .collect()
}

/// 指定武将包组合内的武将码（按包顺序 + gid 升序）；未启用的包不参与。
pub fn general_codes_in_packs(packs: &[String]) -> Vec<&'static str> {
    GENERAL_PACKS
        .iter()
        .flat_map(|p| p.iter())
        .filter(|g| packs.iter().any(|p| p == g.pack))
        .map(|g| g.code)
        .collect()
}
