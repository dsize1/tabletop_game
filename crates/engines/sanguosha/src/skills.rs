//! 技能内核（P2.4，第一批）：技能注册表 + 触发钩子。
//!
//! 设计：
//! - 技能按**武将**绑定（`sgs_generals.code` → 技能列表），引擎用固定顺序派发
//!   （技能声明优先级 → 当前回合玩家左手侧座位顺序 → 注册顺序），不依赖 HashMap 迭代。
//! - 需要玩家决策的技能压入结算栈帧（`Jianxiong` / `MultiSelect`），与锦囊/无懈共用同一套 pending。
//! - 本批实现：奸雄、制衡、救援、闭月、集智、奇才、克己。
//!   尚未实现（P2.4 后续）：仁德、激将、护驾、离间、突袭、奇袭。

use crate::data;
use crate::state::*;
use game_engine_core::EngineEvent;
use serde_json::json;

/// 技能类型（与中台 `sgs_skills.type` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Limited / Awaken 预留给后续武将
pub enum SkillType {
    Normal,
    Lord,
    Locked,
    Limited,
    Awaken,
}

impl SkillType {
    pub fn code(self) -> &'static str {
        match self {
            SkillType::Normal => "normal",
            SkillType::Lord => "lord",
            SkillType::Locked => "locked",
            SkillType::Limited => "limited",
            SkillType::Awaken => "awaken",
        }
    }
}

pub struct SkillDef {
    // name/kind 供视图与后续技能派发使用
    pub code: &'static str,
    pub name: &'static str,
    pub kind: SkillType,
    pub implemented: bool,
}

/// 武将 → 技能（按 `sgs_generals.code`；本批只列 P2 池内武将）。
pub fn skills_of(general: &str) -> &'static [SkillDef] {
    const LIUBEI: &[SkillDef] = &[
        SkillDef {
            code: "rende",
            name: "仁德",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jijiang",
            name: "激将",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const CAOCAO: &[SkillDef] = &[
        SkillDef {
            code: "jianxiong",
            name: "奸雄",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "hujia",
            name: "护驾",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const SUNQUAN: &[SkillDef] = &[
        SkillDef {
            code: "zhiheng",
            name: "制衡",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiuyuan",
            name: "救援",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const HUANGYUEYING: &[SkillDef] = &[
        SkillDef {
            code: "jizhi",
            name: "集智",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qicai",
            name: "奇才",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const LVMENG: &[SkillDef] = &[SkillDef {
        code: "keji",
        name: "克己",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const DIAOCHAN: &[SkillDef] = &[
        SkillDef {
            code: "biyue",
            name: "闭月",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "lijian",
            name: "离间",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const ZHANGLIAO: &[SkillDef] = &[SkillDef {
        code: "tuxi",
        name: "突袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const GANNING: &[SkillDef] = &[SkillDef {
        code: "qixi",
        name: "奇袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    // P2.5：观星（特殊区 `guanxing`）；空城留 P4。
    const ZHUGELIANG: &[SkillDef] = &[
        SkillDef {
            code: "guanxing",
            name: "观星",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "kongcheng",
            name: "空城",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    // P4.1：C1 批次（锁定/触发类：咆哮、马术、英姿、裸衣、天妒、谦逊、连营、枭姬）。
    const HUANGGAI: &[SkillDef] = &[SkillDef {
        code: "kurou",
        name: "苦肉",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const LUBU: &[SkillDef] = &[SkillDef {
        code: "wushuang",
        name: "无双",
        kind: SkillType::Locked,
        implemented: true,
    }];
    const XIAHOUDUN: &[SkillDef] = &[SkillDef {
        code: "ganglie",
        name: "刚烈",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHANGFEI: &[SkillDef] = &[SkillDef {
        code: "paoxiao",
        name: "咆哮",
        kind: SkillType::Locked,
        implemented: true,
    }];
    const MA_CHAO: &[SkillDef] = &[
        SkillDef {
            code: "tieqi",
            name: "铁骑",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "mashu",
            name: "马术",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const ZHOUYU: &[SkillDef] = &[
        SkillDef {
            code: "yingzi",
            name: "英姿",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "fanjian",
            name: "反间",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const LUXUN: &[SkillDef] = &[
        SkillDef {
            code: "qianxun",
            name: "谦逊",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "lianying",
            name: "连营",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const XUCHU: &[SkillDef] = &[SkillDef {
        code: "luoyi",
        name: "裸衣",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const GUOJIA: &[SkillDef] = &[
        SkillDef {
            code: "tiandu",
            name: "天妒",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "yiji",
            name: "遗计",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const SUNSHANGXIANG: &[SkillDef] = &[
        SkillDef {
            code: "xiaoji",
            name: "枭姬",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jieyin",
            name: "结姻",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const DAQIAO: &[SkillDef] = &[
        SkillDef {
            code: "guose",
            name: "国色",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "liuli",
            name: "流离",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // P4.1：转化/防御批次（武圣/龙胆/倾国/急救）。其余技能分批实现。
    const GUANYU: &[SkillDef] = &[SkillDef {
        code: "wusheng",
        name: "武圣",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHAOYUN: &[SkillDef] = &[SkillDef {
        code: "longdan",
        name: "龙胆",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHENJI: &[SkillDef] = &[
        SkillDef {
            code: "luoshen",
            name: "洛神",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qingguo",
            name: "倾国",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const HUATUO: &[SkillDef] = &[
        SkillDef {
            code: "jijiu",
            name: "急救",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qingnang",
            name: "青囊",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // P4.1：司马懿（反馈留 P4.1 后续批次；鬼才 = 判定改判窗口）。
    const SIMAYI: &[SkillDef] = &[
        SkillDef {
            code: "fankui",
            name: "反馈",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "guicai",
            name: "鬼才",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    match general {
        "sgs_hero_1" => LIUBEI,
        "sgs_hero_15" => CAOCAO,
        "sgs_hero_8" => SUNQUAN,
        "sgs_hero_7" => HUANGYUEYING,
        "sgs_hero_10" => LVMENG,
        "sgs_hero_24" => DIAOCHAN,
        "sgs_hero_18" => ZHANGLIAO,
        "sgs_hero_9" => GANNING,
        "sgs_hero_4" => ZHUGELIANG,
        "sgs_hero_16" => SIMAYI,
        "sgs_hero_2" => GUANYU,
        "sgs_hero_5" => ZHAOYUN,
        "sgs_hero_21" => ZHENJI,
        "sgs_hero_22" => HUATUO,
        "sgs_hero_11" => HUANGGAI,
        "sgs_hero_17" => XIAHOUDUN,
        "sgs_hero_23" => LUBU,
        "sgs_hero_3" => ZHANGFEI,
        "sgs_hero_6" => MA_CHAO,
        "sgs_hero_12" => ZHOUYU,
        "sgs_hero_13" => DAQIAO,
        "sgs_hero_14" => LUXUN,
        "sgs_hero_19" => XUCHU,
        "sgs_hero_20" => GUOJIA,
        "sgs_hero_25" => SUNSHANGXIANG,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// 装备技能注册表（P4.5 / ADR-0022）
// ---------------------------------------------------------------------------

/// 技能触发时机：窗口由引擎开，效果由注册表实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// 被动/锁定（距离、次数、无视防具等）。
    Passive,
    /// 出牌阶段主动（转化出杀等）。
    PlayPhase,
    /// 指定目标后（雌雄双股剑）。
    TargetChosen,
    /// 造成伤害前（寒冰剑）。
    DamageBefore,
    /// 被【闪】抵消后（青龙偃月刀 / 贯石斧）。
    AfterDodge,
    /// 判定前（八卦阵等）。
    JudgeBefore,
}

pub struct EquipmentSkillDef {
    /// 技能 code（当前与装备牌定义码一致）。
    pub code: &'static str,
    pub name: &'static str,
    /// 装备牌定义码（`st.has_equip` 用）。
    pub card: &'static str,
    pub trigger: Trigger,
}

/// 标准包装备技能（17 件；装备槽由牌定义决定）。
pub static EQUIPMENT_SKILLS: &[EquipmentSkillDef] = &[
    EquipmentSkillDef {
        code: "zhuge_liannu",
        name: "诸葛连弩",
        card: "zhuge_liannu",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "cixiong_shuanggujian",
        name: "雌雄双股剑",
        card: "cixiong_shuanggujian",
        trigger: Trigger::TargetChosen,
    },
    EquipmentSkillDef {
        code: "qinggang_jian",
        name: "青釭剑",
        card: "qinggang_jian",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "qinglong_yanyue",
        name: "青龙偃月刀",
        card: "qinglong_yanyue",
        trigger: Trigger::AfterDodge,
    },
    EquipmentSkillDef {
        code: "zhangba_shemao",
        name: "丈八蛇矛",
        card: "zhangba_shemao",
        trigger: Trigger::PlayPhase,
    },
    EquipmentSkillDef {
        code: "guanshi_fu",
        name: "贯石斧",
        card: "guanshi_fu",
        trigger: Trigger::AfterDodge,
    },
    EquipmentSkillDef {
        code: "fangtian_huaji",
        name: "方天画戟",
        card: "fangtian_huaji",
        trigger: Trigger::TargetChosen,
    },
    EquipmentSkillDef {
        code: "qilin_gong",
        name: "麒麟弓",
        card: "qilin_gong",
        trigger: Trigger::DamageBefore,
    },
    EquipmentSkillDef {
        code: "hanbing_jian",
        name: "寒冰剑",
        card: "hanbing_jian",
        trigger: Trigger::DamageBefore,
    },
    EquipmentSkillDef {
        code: "bagua_zhen",
        name: "八卦阵",
        card: "bagua_zhen",
        trigger: Trigger::JudgeBefore,
    },
    EquipmentSkillDef {
        code: "renwang_dun",
        name: "仁王盾",
        card: "renwang_dun",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "chitu",
        name: "赤兔",
        card: "chitu",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "dawan",
        name: "大宛",
        card: "dawan",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "zixing",
        name: "紫骍",
        card: "zixing",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "dilu",
        name: "的卢",
        card: "dilu",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "jueying",
        name: "绝影",
        card: "jueying",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "zhuahuang_feidian",
        name: "爪黄飞电",
        card: "zhuahuang_feidian",
        trigger: Trigger::Passive,
    },
    // 军争装备（P4.3）
    EquipmentSkillDef {
        code: "hualiu",
        name: "骅骝",
        card: "hualiu",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "tengjia",
        name: "藤甲",
        card: "tengjia",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "baiyin_shizi",
        name: "白银狮子",
        card: "baiyin_shizi",
        trigger: Trigger::Passive,
    },
    EquipmentSkillDef {
        code: "guding_dao",
        name: "古锭刀",
        card: "guding_dao",
        trigger: Trigger::DamageBefore,
    },
    EquipmentSkillDef {
        code: "zhuque_yushan",
        name: "朱雀羽扇",
        card: "zhuque_yushan",
        trigger: Trigger::PlayPhase,
    },
];

/// 是否拥有某装备技能（装备效果统一判断入口）。
pub fn has_equipment_skill(st: &SgsState, seat: usize, code: &str) -> bool {
    EQUIPMENT_SKILLS
        .iter()
        .any(|d| d.code == code && st.has_equip(seat, d.card))
}

/// 按时机收集持有者（座位升序 → 注册序，确定性）。
pub fn triggered(st: &SgsState, trigger: Trigger) -> Vec<(usize, &'static EquipmentSkillDef)> {
    let mut out = Vec::new();
    for seat in 0..st.seats.len() {
        for def in EQUIPMENT_SKILLS.iter().filter(|d| d.trigger == trigger) {
            if st.has_equip(seat, def.card) {
                out.push((seat, def));
            }
        }
    }
    out
}

/// 是否拥有某技能（只有已实现的技能会触发）。
pub fn has(st: &SgsState, seat: usize, code: &str) -> bool {
    let Some(general) = st.seats.get(seat).and_then(|s| s.general.as_deref()) else {
        return false;
    };
    skills_of(general)
        .iter()
        .any(|s| s.code == code && s.implemented)
}

/// 当前玩家（按座位顺序）中第一个拥有该技能的玩家（主公技救援等使用）。
#[allow(dead_code)]
pub fn find_owner(st: &SgsState, code: &str) -> Option<usize> {
    st.alive_indices().into_iter().find(|i| has(st, *i, code))
}

/// 摸牌并检查持有上限（超出时压入弃牌帧）。
fn draw_and_cap(st: &mut SgsState, seat: usize, n: u32, events: &mut Vec<EngineEvent>) -> usize {
    let drawn = crate::turn::draw_cards(st, seat, n, events);
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
    drawn
}

/// 是否主公。
pub fn is_lord(st: &SgsState, seat: usize) -> bool {
    st.seats.get(seat).is_some_and(|s| s.identity.is_lord())
}

/// 红色（红桃/方块）。
pub fn is_red(suit: Suit) -> bool {
    matches!(suit, Suit::Heart | Suit::Diamond)
}

/// 黑色（黑桃/梅花）。
pub fn is_black(suit: Suit) -> bool {
    matches!(suit, Suit::Spade | Suit::Club)
}

/// 转化技（P4.1）：能否把 `card_id` 当作 `virtual_code` 使用/打出。
///
/// - `sha`：武圣（红色牌）/ 龙胆（【闪】）
/// - `shan`：龙胆（【杀】）/ 倾国（黑色牌）
/// - `tao`：急救（回合外红色牌）
///
/// 返回 `(技能 code, 技能名)`；不合法返回 `None`。实际牌与目标牌相同时不视为转化
/// （由调用方按真实牌处理）。
pub fn conversion_for(
    st: &SgsState,
    seat: usize,
    card_id: &str,
    virtual_code: &str,
) -> Option<(&'static str, &'static str)> {
    let inst = st.cards.get(card_id)?;
    if inst.def == virtual_code {
        return None;
    }
    let red = is_red(inst.suit);
    let black = is_black(inst.suit);
    match virtual_code {
        "sha" => {
            if has(st, seat, "wusheng") && red {
                return Some(("wusheng", "武圣"));
            }
            if has(st, seat, "longdan") && inst.def == "shan" {
                return Some(("longdan", "龙胆"));
            }
        }
        "shan" => {
            if has(st, seat, "longdan") && inst.def == "sha" {
                return Some(("longdan", "龙胆"));
            }
            if has(st, seat, "qingguo") && black {
                return Some(("qingguo", "倾国"));
            }
        }
        "tao" => {
            // 急救：回合外（非自己的回合）红色牌当【桃】。
            if has(st, seat, "jijiu") && red && st.turn != seat {
                return Some(("jijiu", "急救"));
            }
        }
        _ => {}
    }
    None
}

/// 谦逊（锁定技）：不能成为【顺手牵羊】/【过河拆桥】的目标。
pub fn qianxun_blocks(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qianxun")
}

/// 连营：失去最后的手牌后摸 1 张（自动发动）。
pub fn lianying_on_lose_last(st: &mut SgsState, seat: usize) {
    if !st.seats.get(seat).is_some_and(|s| s.hand.is_empty()) || !has(st, seat, "lianying") {
        return;
    }
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 1, &mut ev);
    if drawn > 0 {
        let pid = st.seats[seat].player_id.clone();
        st.push_log(Some(&pid), "连营：失去最后的手牌，摸 1 张");
        let over = st.seats[seat]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, seat, over, "hand_cap");
        }
    }
}

/// 枭姬：失去装备区的牌后摸 2 张（自动发动）。
pub fn xiaoji_on_lose_equip(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "xiaoji") {
        return;
    }
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 2, &mut ev);
    if drawn > 0 {
        let pid = st.seats[seat].player_id.clone();
        st.push_log(Some(&pid), format!("枭姬：失去装备，摸 {drawn} 张"));
        let over = st.seats[seat]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, seat, over, "hand_cap");
        }
    }
}

/// 天妒：判定牌生效后获得之（自动获得；口径见 `skills.md`）。返回是否已拿走判定牌。
pub fn tiandu_gain(st: &mut SgsState, owner: usize, judge_id: &str) -> bool {
    if !has(st, owner, "tiandu") {
        return false;
    }
    crate::resolve::leave_processing(st, judge_id);
    st.seats[owner].hand.push(judge_id.to_string());
    let label = crate::turn::card_label(st, judge_id);
    let pid = st.seats[owner].player_id.clone();
    st.push_log(Some(&pid), format!("天妒：获得判定牌【{label}】"));
    let over = st.seats[owner]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, owner, over, "hand_cap");
    }
    true
}

/// 空城（锁定技）：无手牌时不能成为【杀】/【决斗】的目标。
pub fn kongcheng_blocks(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .is_some_and(|s| s.hand.is_empty() && has(st, seat, "kongcheng"))
}

/// 是否男性（用于离间等）。
pub fn is_male(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(data::general)
        .map(|g| g.gender == "male")
        .unwrap_or(false)
}

fn kingdom(st: &SgsState, seat: usize) -> &'static str {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(data::general)
        .map(|g| g.kingdom)
        .unwrap_or("")
}

/// 主公技接力：`needed` = sha（激将/蜀）或 shan（护驾/魏）。
pub fn requisition_available(st: &SgsState, seat: usize, needed: &str) -> bool {
    let (skill, faction) = match needed {
        "sha" => ("jijiang", "shu"),
        "shan" => ("hujia", "wei"),
        _ => return false,
    };
    is_lord(st, seat)
        && has(st, seat, skill)
        && !requisition_order(st, seat, needed).is_empty()
        && st
            .seats_from_left_of(seat)
            .into_iter()
            .any(|i| st.seats[i].alive && kingdom(st, i) == faction)
}

/// 接力询问顺序：从主公左手侧开始的同势力存活角色。
///
/// **无手牌的同势力角色不进入询问队列**（无牌可代出；手牌数是公共信息，
/// 与 `special-zones.md` §4.1 的响应询问口径一致）。
pub fn requisition_order(st: &SgsState, seat: usize, needed: &str) -> Vec<String> {
    let faction = match needed {
        "sha" => "shu",
        "shan" => "wei",
        _ => return Vec::new(),
    };
    st.seats_from_left_of(seat)
        .into_iter()
        .filter(|i| {
            st.seats[*i].alive && kingdom(st, *i) == faction && !st.seats[*i].hand.is_empty()
        })
        .map(|i| st.seats[i].player_id.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// 钩子
// ---------------------------------------------------------------------------

/// 阶段开始钩子：闭月（结束阶段摸 1）。
pub fn on_phase_start(st: &mut SgsState, phase: TurnPhase, events: &mut Vec<EngineEvent>) {
    if phase != TurnPhase::End {
        return;
    }
    let idx = st.turn;
    if !has(st, idx, "biyue") {
        return;
    }
    let drawn = draw_and_cap(st, idx, 1, events);
    st.push_log(
        Some(&st.seats[idx].player_id.clone()),
        format!("闭月：摸 {drawn} 张"),
    );
}

/// 使用锦囊后的钩子：集智（使用非延时锦囊时摸 1）。
pub fn on_non_delayed_trick_used(st: &mut SgsState, seat: usize, events: &mut Vec<EngineEvent>) {
    if !has(st, seat, "jizhi") {
        return;
    }
    let drawn = draw_and_cap(st, seat, 1, events);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("集智：摸 {drawn} 张"),
    );
}

/// 弃牌阶段上限：克己（出牌阶段未使用过杀 → 跳过弃牌）。
pub fn hand_limit(st: &SgsState, seat: usize) -> usize {
    if has(st, seat, "keji") && st.counter("sha_used") == 0 {
        usize::MAX
    } else {
        st.hand_limit(seat)
    }
}

/// 奇才：使用锦囊无距离限制。
pub fn trick_distance_unlimited(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qicai")
}

/// 救援：其他吴势力角色对拥有此技的主公使用【桃】时，回复 +1。
pub fn tao_bonus(st: &SgsState, healer: usize, target: usize) -> i32 {
    if healer == target {
        return 0;
    }
    if has(st, target, "jiuyuan") && kingdom(st, healer) == "wu" {
        1
    } else {
        0
    }
}

/// 奸雄：受伤后可以获得造成伤害的牌。
pub fn jianxiong_enabled(st: &SgsState, victim: usize) -> bool {
    has(st, victim, "jianxiong")
}

/// 技能生效事件（便于回放/调试）。
#[allow(dead_code)]
pub fn skill_event(
    st: &SgsState,
    seat: usize,
    code: &str,
    extra: serde_json::Value,
) -> EngineEvent {
    EngineEvent::new(
        "skill_triggered",
        json!({
            "player": st.seats.get(seat).map(|s| s.player_id.clone()),
            "skill": code,
            "detail": extra,
        }),
    )
}
