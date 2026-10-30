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
/// 已生成的全部武将数（所有包合计）。
pub const GENERAL_COUNT: usize = 25;

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
    /// 武将包 code（`standard`；P4.4 包开关过滤时消费）。
    #[allow(dead_code)]
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

/// 已生成的武将包（按包顺序拼接即选将池）。
#[rustfmt::skip]
pub static GENERAL_PACKS: &[&[GeneralDef]] = &[
    STANDARD_GENERALS,
];

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

/// 全部武将码（按包顺序 + gid 升序），用于选将池。
pub fn all_general_codes() -> Vec<&'static str> {
    GENERAL_PACKS
        .iter()
        .flat_map(|p| p.iter())
        .map(|g| g.code)
        .collect()
}
