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
    // P4.4.1 风包：魏延 / 夏侯渊 / 曹仁 / 小乔 / 黄忠 / 周泰 / 于吉 / 张角。
    const WEIYAN: &[SkillDef] = &[
        SkillDef {
            code: "kuanggu",
            name: "狂骨",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qimou",
            name: "奇谋",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    const XIAHOUYUAN: &[SkillDef] = &[SkillDef {
        code: "shensu",
        name: "神速",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const CAOREN: &[SkillDef] = &[
        SkillDef {
            code: "jushou",
            name: "据守",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiewei",
            name: "解围",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const XIAOQIAO: &[SkillDef] = &[
        SkillDef {
            code: "hongyan",
            name: "红颜",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "tianxiang",
            name: "天香",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const HUANGZHONG: &[SkillDef] = &[SkillDef {
        code: "liegong",
        name: "烈弓",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHOUTAI: &[SkillDef] = &[
        SkillDef {
            code: "buqu",
            name: "不屈",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "fenji",
            name: "奋激",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const YUJI: &[SkillDef] = &[SkillDef {
        code: "guhuo",
        name: "蛊惑",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHANGJIAO: &[SkillDef] = &[
        SkillDef {
            code: "guidao",
            name: "鬼道",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "huangtian",
            name: "黄天",
            kind: SkillType::Lord,
            implemented: true,
        },
        SkillDef {
            code: "leiji",
            name: "雷击",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // P4.4.1 火包：典韦 / 荀彧 / 庞统 / 卧龙诸葛亮 / 太史慈 / 庞德 / 颜良文丑 / 袁绍。
    const DIANWEI: &[SkillDef] = &[SkillDef {
        code: "qiangxi",
        name: "强袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const XUNYU: &[SkillDef] = &[
        SkillDef {
            code: "quhu",
            name: "驱虎",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jieming",
            name: "节命",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const PANGTONG: &[SkillDef] = &[
        SkillDef {
            code: "lianhuan",
            name: "连环",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "niepan",
            name: "涅槃",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    const WOLONG: &[SkillDef] = &[
        SkillDef {
            code: "bazhen",
            name: "八阵",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "huoji",
            name: "火计",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "kanpo",
            name: "看破",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const TAISHICI: &[SkillDef] = &[SkillDef {
        code: "tianyi",
        name: "天义",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const PANGDE: &[SkillDef] = &[
        SkillDef {
            code: "mashu",
            name: "马术",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "jiandu",
            name: "鞬出",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const YANLIANGWENCHOU: &[SkillDef] = &[SkillDef {
        code: "shuangxiong",
        name: "双雄",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const YUANSHAO: &[SkillDef] = &[
        SkillDef {
            code: "luanji",
            name: "乱击",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "xueyi",
            name: "血裔",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    // P4.4.1 林包：徐晃 / 曹丕 / 孙坚 / 董卓 / 祝融 / 孟获 / 贾诩 / 鲁肃。
    const XUHUANG: &[SkillDef] = &[
        SkillDef {
            code: "duanliang",
            name: "断粮",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiezi",
            name: "截辎",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const CAOPI: &[SkillDef] = &[
        SkillDef {
            code: "xingshang",
            name: "行殇",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "fangzhu",
            name: "放逐",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "songwei",
            name: "颂威",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const SUNJIAN: &[SkillDef] = &[SkillDef {
        code: "yinghun",
        name: "英魂",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const DONGZHUO: &[SkillDef] = &[
        SkillDef {
            code: "jiuchi",
            name: "酒池",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "roulin",
            name: "肉林",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "bengkuai",
            name: "崩坏",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "baonue",
            name: "暴虐",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const ZHURONG: &[SkillDef] = &[
        SkillDef {
            code: "juxiang",
            name: "巨象",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "liewen",
            name: "烈刃",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const MENG_HUO: &[SkillDef] = &[
        SkillDef {
            code: "huoshou",
            name: "祸首",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "zaizhi",
            name: "再起",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIAXU: &[SkillDef] = &[
        SkillDef {
            code: "wansha",
            name: "完杀",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "luanwu",
            name: "乱武",
            kind: SkillType::Limited,
            implemented: true,
        },
        SkillDef {
            code: "weimu",
            name: "帷幕",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const LUSU: &[SkillDef] = &[
        SkillDef {
            code: "haoshi",
            name: "好施",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "dimeng",
            name: "缔盟",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // P4.4.1 山包：张郃 / 邓艾 / 姜维 / 刘禅 / 孙策 / 张昭张纮 / 左慈 / 蔡文姬。
    const ZHANGHE: &[SkillDef] = &[SkillDef {
        code: "qiaobian",
        name: "巧变",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const DENGAI: &[SkillDef] = &[
        SkillDef {
            code: "tuotian",
            name: "屯田",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zaoxian",
            name: "凿险",
            kind: SkillType::Awaken,
            implemented: true,
        },
    ];
    const JIANGWEI: &[SkillDef] = &[
        SkillDef {
            code: "tiaoxin",
            name: "挑衅",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhiji",
            name: "志继",
            kind: SkillType::Awaken,
            implemented: true,
        },
    ];
    const LIUSHAN: &[SkillDef] = &[
        SkillDef {
            code: "xiangle",
            name: "享乐",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "fangquan",
            name: "放权",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "ruoyu",
            name: "若愚",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const SUNCE: &[SkillDef] = &[
        SkillDef {
            code: "jiang",
            name: "激昂",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "hunzi",
            name: "魂姿",
            kind: SkillType::Awaken,
            implemented: true,
        },
        SkillDef {
            code: "zhiba",
            name: "制霸",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const ZHANGZHAO: &[SkillDef] = &[
        SkillDef {
            code: "zhijian",
            name: "直谏",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "guzheng",
            name: "固政",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const ZUOCI: &[SkillDef] = &[
        SkillDef {
            code: "huashen",
            name: "化身",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "xinsheng",
            name: "新生",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const CAIWENJI: &[SkillDef] = &[
        SkillDef {
            code: "beige",
            name: "悲歌",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "duanchang",
            name: "断肠",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    // ---- P4.4.1 SP-1（8 将 / 18 技能）----
    const YUANSHU: &[SkillDef] = &[
        SkillDef {
            code: "yongsi",
            name: "庸肆",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "weidi",
            name: "伪帝",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const WANGJI: &[SkillDef] = &[
        SkillDef {
            code: "qizhi",
            name: "奇制",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jinzou",
            name: "进趋",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const YANYAN: &[SkillDef] = &[SkillDef {
        code: "juzhan",
        name: "拒战",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const WANGPING: &[SkillDef] = &[
        SkillDef {
            code: "feijun",
            name: "飞军",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "binglue",
            name: "兵略",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const LUJI: &[SkillDef] = &[
        SkillDef {
            code: "huaiju",
            name: "怀橘",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "yili",
            name: "遗礼",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhenglun",
            name: "整论",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const SUNLIANG: &[SkillDef] = &[
        SkillDef {
            code: "kuizhu",
            name: "溃诛",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "chezheng",
            name: "掣政",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "lijun",
            name: "立军",
            kind: SkillType::Lord,
            implemented: true,
        },
    ];
    const KUAIYUE: &[SkillDef] = &[
        SkillDef {
            code: "jianxiang",
            name: "荐降",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "shenshi",
            name: "审时",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // ---- P4.4.1 SP-2（8 将 / 15 技能）----
    const LUZHI: &[SkillDef] = &[
        SkillDef {
            code: "mingren",
            name: "明任",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhenliang",
            name: "贞良",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const HAOZHAO: &[SkillDef] = &[SkillDef {
        code: "zhengu",
        name: "镇骨",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const CHENDAO: &[SkillDef] = &[SkillDef {
        code: "wanglie",
        name: "往烈",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const ZHUGELAN: &[SkillDef] = &[
        SkillDef {
            code: "zuilun",
            name: "罪论",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "fuyin",
            name: "父荫",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const ZHOUFEI: &[SkillDef] = &[
        SkillDef {
            code: "liangyin",
            name: "良姻",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "kongsheng",
            name: "箜声",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const GUANQIUJIAN: &[SkillDef] = &[
        SkillDef {
            code: "zhengrong",
            name: "征荣",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "hongju",
            name: "鸿举",
            kind: SkillType::Awaken,
            implemented: true,
        },
    ];
    const LUKANG: &[SkillDef] = &[
        SkillDef {
            code: "qianjie",
            name: "谦节",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "jueyan",
            name: "决堰",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "poshi",
            name: "破势",
            kind: SkillType::Awaken,
            implemented: true,
        },
    ];
    const ZHANGXIU: &[SkillDef] = &[
        SkillDef {
            code: "xiongluan",
            name: "雄乱",
            kind: SkillType::Limited,
            implemented: true,
        },
        SkillDef {
            code: "congjian",
            name: "从谏",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // ---- P4.4.2 一将成名（批次 1：8 将 / 13 技能）----
    const CAOZHI: &[SkillDef] = &[
        SkillDef {
            code: "luoying",
            name: "落英",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiushi",
            name: "酒诗",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const GAOSHUN: &[SkillDef] = &[
        SkillDef {
            code: "xianzhen",
            name: "陷阵",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jinjiu",
            name: "禁酒",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const CHENGONG: &[SkillDef] = &[
        SkillDef {
            code: "mingce",
            name: "明策",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhichi",
            name: "智迟",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const LINGTONG: &[SkillDef] = &[SkillDef {
        code: "xuanfeng",
        name: "旋风",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const MASU: &[SkillDef] = &[
        SkillDef {
            code: "sanyao",
            name: "散谣",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhiman",
            name: "制蛮",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const WUGUOTAI: &[SkillDef] = &[
        SkillDef {
            code: "ganlu",
            name: "甘露",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "buyi",
            name: "补益",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const XUSHENG: &[SkillDef] = &[SkillDef {
        code: "pojun",
        name: "破军",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const YUJIN: &[SkillDef] = &[SkillDef {
        code: "zhenjun",
        name: "镇军",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const XUYOU: &[SkillDef] = &[
        SkillDef {
            code: "chenglue",
            name: "成略",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "shicai",
            name: "恃才",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "cunmu",
            name: "寸目",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    // ---- P4.4.2 界标 25 将（批次 2：蜀 7 将）----
    const JIE_LIUBEI: &[SkillDef] = &[
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
    const JIE_GUANYU: &[SkillDef] = &[
        SkillDef {
            code: "yijue",
            name: "义绝",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "wusheng",
            name: "武圣",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_ZHANGFEI: &[SkillDef] = &[
        SkillDef {
            code: "tishen",
            name: "替身",
            kind: SkillType::Limited,
            implemented: true,
        },
        SkillDef {
            code: "paoxiao",
            name: "咆哮",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const JIE_ZHAOYUN: &[SkillDef] = &[
        SkillDef {
            code: "yajiao",
            name: "涯角",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "longdan",
            name: "龙胆",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_MA_CHAO: &[SkillDef] = &[
        SkillDef {
            code: "mashu",
            name: "马术",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "tieqi",
            name: "铁骑",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_ZHUGELIANG: &[SkillDef] = &[
        SkillDef {
            code: "kongcheng",
            name: "空城",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "guanxing",
            name: "观星",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_HUANGYUEYING: &[SkillDef] = &[
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
    // ---- P4.4.2 界标 25 将（批次 3：吴 8 将）----
    const JIE_GANNING: &[SkillDef] = &[
        SkillDef {
            code: "qixi",
            name: "奇袭",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "fenwei",
            name: "奋威",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    const JIE_LVMENG: &[SkillDef] = &[
        SkillDef {
            code: "keji",
            name: "克己",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qinxue",
            name: "勤学",
            kind: SkillType::Awaken,
            implemented: true,
        },
        SkillDef {
            code: "botu",
            name: "博图",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_HUANGGAI: &[SkillDef] = &[
        SkillDef {
            code: "kurou",
            name: "苦肉",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhaxiang",
            name: "诈降",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const JIE_ZHOUYU: &[SkillDef] = &[
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
    const JIE_DAQIAO: &[SkillDef] = &[
        SkillDef {
            code: "liuli",
            name: "流离",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "guose",
            name: "国色",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_LUXUN: &[SkillDef] = &[
        SkillDef {
            code: "qianxun",
            name: "谦逊",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "lianying",
            name: "连营",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_SUNQUAN: &[SkillDef] = &[
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
    const JIE_SUNSHANGXIANG: &[SkillDef] = &[
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
    // ---- P4.4.2 界标 25 将（批次 4：魏 8 将）----
    const JIE_CAOCAO: &[SkillDef] = &[
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
    const JIE_SIMAYI: &[SkillDef] = &[
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
    const JIE_XIAHOUDUN: &[SkillDef] = &[
        SkillDef {
            code: "ganglie",
            name: "刚烈",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qingjian",
            name: "清俭",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_ZHANGLIAO: &[SkillDef] = &[SkillDef {
        code: "tuxi",
        name: "突袭",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const JIE_XUCHU: &[SkillDef] = &[SkillDef {
        code: "luoyi",
        name: "裸衣",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const JIE_GUOJIA: &[SkillDef] = &[
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
    const JIE_LIDIAN: &[SkillDef] = &[
        SkillDef {
            code: "xunxun",
            name: "恂恂",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "wangxi",
            name: "忘隙",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_ZHENJI: &[SkillDef] = &[
        SkillDef {
            code: "qingguo",
            name: "倾国",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "luoshen",
            name: "洛神",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // ---- P4.4.2 界标 25 将（批次 5：群 4 将）----
    const JIE_HUATUO: &[SkillDef] = &[
        SkillDef {
            code: "jijiu",
            name: "急救",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "chuli",
            name: "除疠",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_LUBU: &[SkillDef] = &[
        SkillDef {
            code: "wushuang",
            name: "无双",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "liyu",
            name: "利驭",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_GONGSUNZAN: &[SkillDef] = &[
        SkillDef {
            code: "yicong",
            name: "义从",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "qiaomeng",
            name: "趫猛",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const JIE_DIAOCHAN: &[SkillDef] = &[
        SkillDef {
            code: "lijian",
            name: "离间",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "biyue",
            name: "闭月",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // ---- P4.4.2 一将成名（批次 2：8 将 / 16 技能）----
    const ZHANGCHUNHUA: &[SkillDef] = &[
        SkillDef {
            code: "jueqing",
            name: "绝情",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "shangshi",
            name: "伤逝",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const CAOZHANG: &[SkillDef] = &[SkillDef {
        code: "jiangchi",
        name: "将驰",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const XUNYOU: &[SkillDef] = &[
        SkillDef {
            code: "qice",
            name: "奇策",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhiyu",
            name: "智愚",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const LIAOHUA: &[SkillDef] = &[
        SkillDef {
            code: "dangxian",
            name: "当先",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "fuli",
            name: "伏枥",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    const MADAI: &[SkillDef] = &[
        SkillDef {
            code: "mashu",
            name: "马术",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "qianxi",
            name: "潜袭",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const BULIANSHI: &[SkillDef] = &[
        SkillDef {
            code: "anxu",
            name: "安恤",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhuiyi",
            name: "追忆",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const CHENGPU: &[SkillDef] = &[
        SkillDef {
            code: "lihuo",
            name: "疠火",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "chunlao",
            name: "醇醪",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const HANDANG: &[SkillDef] = &[
        SkillDef {
            code: "gongqi",
            name: "弓骑",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "jiefan",
            name: "解烦",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    // ---- P4.4.2 一将成名（批次 Y1：8 将 / 14 技能 + 排异授予）----
    const JIANGHUAXIONG: &[SkillDef] = &[SkillDef {
        code: "shiyong",
        name: "恃勇",
        kind: SkillType::Locked,
        implemented: true,
    }];
    const LIUBIAO: &[SkillDef] = &[
        SkillDef {
            code: "zishou",
            name: "自守",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zongshi",
            name: "宗室",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const ZHONGHUI: &[SkillDef] = &[
        SkillDef {
            code: "quanji",
            name: "权计",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zili",
            name: "自立",
            kind: SkillType::Awaken,
            implemented: true,
        },
    ];
    const CAOCHONG: &[SkillDef] = &[
        SkillDef {
            code: "chengxiang",
            name: "称象",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "renxin",
            name: "仁心",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const GUOHUAI: &[SkillDef] = &[SkillDef {
        code: "jingce",
        name: "精策",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const MANCHONG: &[SkillDef] = &[
        SkillDef {
            code: "junxing",
            name: "峻刑",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "yuce",
            name: "御策",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const GUANPING: &[SkillDef] = &[SkillDef {
        code: "longyin",
        name: "龙吟",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const JIANYONG: &[SkillDef] = &[
        SkillDef {
            code: "qiaoshuo",
            name: "巧说",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zongshi2",
            name: "纵适",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    // ---- P4.4.2 一将成名（批次 Y2：8 将 / 13 技能）----
    const LIUFENG: &[SkillDef] = &[SkillDef {
        code: "xiansi",
        name: "陷嗣",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const PANZHANGMAZHONG: &[SkillDef] = &[
        SkillDef {
            code: "duodao",
            name: "夺刀",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "anjian",
            name: "暗箭",
            kind: SkillType::Locked,
            implemented: true,
        },
    ];
    const YUFAN: &[SkillDef] = &[
        SkillDef {
            code: "zongxuan",
            name: "纵玄",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "zhiyan",
            name: "直言",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const ZHURAN: &[SkillDef] = &[SkillDef {
        code: "danshou",
        name: "胆守",
        kind: SkillType::Normal,
        implemented: true,
    }];
    const FUHOU: &[SkillDef] = &[
        SkillDef {
            code: "zhuikong",
            name: "惴恐",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "qiuyuan",
            name: "求援",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const LIRU: &[SkillDef] = &[
        SkillDef {
            code: "mieji",
            name: "灭计",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "juece",
            name: "绝策",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "fencheng",
            name: "焚城",
            kind: SkillType::Limited,
            implemented: true,
        },
    ];
    const FAZHENG: &[SkillDef] = &[
        SkillDef {
            code: "enyuan",
            name: "恩怨",
            kind: SkillType::Normal,
            implemented: true,
        },
        SkillDef {
            code: "xuanhuo",
            name: "眩惑",
            kind: SkillType::Normal,
            implemented: true,
        },
    ];
    const XUSHU: &[SkillDef] = &[
        SkillDef {
            code: "wuyan",
            name: "无言",
            kind: SkillType::Locked,
            implemented: true,
        },
        SkillDef {
            code: "jujian",
            name: "举荐",
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
        "sgs_hero_27" => WEIYAN,
        "sgs_hero_28" => XIAHOUYUAN,
        "sgs_hero_29" => CAOREN,
        "sgs_hero_30" => XIAOQIAO,
        "sgs_hero_61" => HUANGZHONG,
        "sgs_hero_210" => ZHOUTAI,
        "sgs_hero_211" => YUJI,
        "sgs_hero_212" => ZHANGJIAO,
        "sgs_hero_34" => DIANWEI,
        "sgs_hero_35" => XUNYU,
        "sgs_hero_36" => PANGTONG,
        "sgs_hero_37" => WOLONG,
        "sgs_hero_38" => TAISHICI,
        "sgs_hero_39" => PANGDE,
        "sgs_hero_40" => YANLIANGWENCHOU,
        "sgs_hero_41" => YUANSHAO,
        "sgs_hero_42" => XUHUANG,
        "sgs_hero_43" => CAOPI,
        "sgs_hero_44" => SUNJIAN,
        "sgs_hero_45" => DONGZHUO,
        "sgs_hero_46" => ZHURONG,
        "sgs_hero_47" => MENG_HUO,
        "sgs_hero_48" => JIAXU,
        "sgs_hero_49" => LUSU,
        "sgs_hero_50" => ZHANGHE,
        "sgs_hero_51" => DENGAI,
        "sgs_hero_52" => JIANGWEI,
        "sgs_hero_53" => LIUSHAN,
        "sgs_hero_54" => SUNCE,
        "sgs_hero_55" => ZHANGZHAO,
        "sgs_hero_56" => ZUOCI,
        "sgs_hero_57" => CAIWENJI,
        "sgs_hero_100" => YUANSHU,
        "sgs_hero_362" => WANGJI,
        "sgs_hero_400" => YANYAN,
        "sgs_hero_401" => WANGPING,
        "sgs_hero_402" => LUJI,
        "sgs_hero_403" => SUNLIANG,
        "sgs_hero_404" => KUAIYUE,
        "sgs_hero_406" => XUYOU,
        "sgs_hero_407" => LUZHI,
        "sgs_hero_408" => HAOZHAO,
        "sgs_hero_409" => CHENDAO,
        "sgs_hero_410" => ZHUGELAN,
        "sgs_hero_411" => ZHOUFEI,
        "sgs_hero_413" => GUANQIUJIAN,
        "sgs_hero_414" => LUKANG,
        "sgs_hero_415" => ZHANGXIU,
        "sgs_hero_151" => CAOZHI,
        "sgs_hero_152" => GAOSHUN,
        "sgs_hero_153" => CHENGONG,
        "sgs_hero_155" => LINGTONG,
        "sgs_hero_156" => MASU,
        "sgs_hero_157" => WUGUOTAI,
        "sgs_hero_158" => XUSHENG,
        "sgs_hero_160" => YUJIN,
        "sgs_hero_161" => ZHANGCHUNHUA,
        "sgs_hero_162" => CAOZHANG,
        "sgs_hero_164" => XUNYOU,
        "sgs_hero_166" => LIAOHUA,
        "sgs_hero_167" => MADAI,
        "sgs_hero_168" => BULIANSHI,
        "sgs_hero_169" => CHENGPU,
        "sgs_hero_170" => HANDANG,
        "sgs_hero_171" => JIANGHUAXIONG,
        "sgs_hero_172" => LIUBIAO,
        "sgs_hero_173" => ZHONGHUI,
        "sgs_hero_174" => CAOCHONG,
        "sgs_hero_175" => GUOHUAI,
        "sgs_hero_176" => MANCHONG,
        "sgs_hero_177" => GUANPING,
        "sgs_hero_178" => JIANYONG,
        "sgs_hero_179" => LIUFENG,
        "sgs_hero_180" => PANZHANGMAZHONG,
        "sgs_hero_181" => YUFAN,
        "sgs_hero_182" => ZHURAN,
        "sgs_hero_183" => FUHOU,
        "sgs_hero_184" => LIRU,
        "sgs_hero_185" => FAZHENG,
        "sgs_hero_186" => XUSHU,
        "sgs_hero_299" => JIE_LIUBEI,
        "sgs_hero_300" => JIE_GUANYU,
        "sgs_hero_301" => JIE_ZHANGFEI,
        "sgs_hero_302" => JIE_ZHAOYUN,
        "sgs_hero_303" => JIE_MA_CHAO,
        "sgs_hero_440" => JIE_ZHUGELIANG,
        "sgs_hero_441" => JIE_HUANGYUEYING,
        "sgs_hero_305" => JIE_GANNING,
        "sgs_hero_306" => JIE_LVMENG,
        "sgs_hero_307" => JIE_HUANGGAI,
        "sgs_hero_308" => JIE_ZHOUYU,
        "sgs_hero_309" => JIE_DAQIAO,
        "sgs_hero_310" => JIE_LUXUN,
        "sgs_hero_442" => JIE_SUNQUAN,
        "sgs_hero_443" => JIE_SUNSHANGXIANG,
        "sgs_hero_311" => JIE_CAOCAO,
        "sgs_hero_312" => JIE_SIMAYI,
        "sgs_hero_313" => JIE_XIAHOUDUN,
        "sgs_hero_314" => JIE_ZHANGLIAO,
        "sgs_hero_315" => JIE_XUCHU,
        "sgs_hero_316" => JIE_GUOJIA,
        "sgs_hero_317" => JIE_LIDIAN,
        "sgs_hero_444" => JIE_ZHENJI,
        "sgs_hero_318" => JIE_HUATUO,
        "sgs_hero_319" => JIE_LUBU,
        "sgs_hero_320" => JIE_GONGSUNZAN,
        "sgs_hero_445" => JIE_DIAOCHAN,
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
    EQUIPMENT_SKILLS.iter().any(|d| {
        d.code == code
            && (st.has_equip(seat, d.card) || virtual_equip(st, seat).is_some_and(|v| v == d.card))
    })
}

/// 视为装备（P4.4 八阵）：防具槽为空时视为装备【八卦阵】。
pub fn virtual_equip(st: &SgsState, seat: usize) -> Option<&'static str> {
    if has(st, seat, "bazhen") && st.equip_def(seat, EquipSlot::Armor).is_none() {
        Some("bagua_zhen")
    } else {
        None
    }
}

/// 按时机收集持有者（座位升序 → 注册序，确定性）。
pub fn triggered(st: &SgsState, trigger: Trigger) -> Vec<(usize, &'static EquipmentSkillDef)> {
    let mut out = Vec::new();
    for seat in 0..st.seats.len() {
        for def in EQUIPMENT_SKILLS.iter().filter(|d| d.trigger == trigger) {
            if st.has_equip(seat, def.card)
                || virtual_equip(st, seat).is_some_and(|v| v == def.card)
            {
                out.push((seat, def));
            }
        }
    }
    out
}

/// 武将是否可用于选将池：技能注册表存在且全部已实现（未实现武将不进池，避免白板将）。
pub fn general_ready(general: &str) -> bool {
    let defs = skills_of(general);
    !defs.is_empty() && defs.iter().all(|s| s.implemented)
}

/// 界化武将（`pack = "jiexian"`）：同为技能 code 时的界版分支判定。
pub fn is_jiexian(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(data::general)
        .is_some_and(|g| g.pack == "jiexian")
}

/// 是否拥有某技能（只有已实现的技能会触发）。
pub fn has(st: &SgsState, seat: usize, code: &str) -> bool {
    let Some(seat_state) = st.seats.get(seat) else {
        return false;
    };
    // 断肠：杀死蔡文姬的角色失去所有技能。
    if seat_state.flags.contains_key("skills_lost") {
        return false;
    }
    // 破势等：失去某个基础技能（按 code 禁用）。
    if seat_state.flags.contains_key(&format!("lost:{code}")) {
        return false;
    }
    // 义绝（界关羽）：本回合非锁定技失效。
    if st.counter(&format!("nonlocked_lost:{seat}")) > 0 {
        let locked = seat_state
            .general
            .as_deref()
            .and_then(|g| {
                skills_of(g)
                    .iter()
                    .find(|s| s.code == code)
                    .map(|s| s.kind == SkillType::Locked)
            })
            .unwrap_or(false);
        if !locked {
            return false;
        }
    }
    if seat_state.extra_skills.iter().any(|c| c == code) {
        return true;
    }
    let Some(general) = seat_state.general.as_deref() else {
        return false;
    };
    if skills_of(general)
        .iter()
        .any(|s| s.code == code && s.implemented)
    {
        return true;
    }
    // 伪帝（袁术）：视为拥有主公的主公技（仅已实现的主公技；不借觉醒技）。
    let has_weidi = seat_state.extra_skills.iter().any(|c| c == "weidi")
        || skills_of(general)
            .iter()
            .any(|s| s.code == "weidi" && s.implemented);
    if !has_weidi {
        return false;
    }
    let Some(lord) = st.lord_index() else {
        return false;
    };
    if lord == seat {
        return false;
    }
    let Some(lord_general) = st.seats[lord].general.as_deref() else {
        return false;
    };
    skills_of(lord_general)
        .iter()
        .any(|s| s.code == code && s.implemented && s.kind == SkillType::Lord)
}

/// 转换技当前面（P4.4.1 SP-1）：`false` = 阳（默认），`true` = 阴。
pub fn yj_yin(st: &SgsState, seat: usize, code: &str) -> bool {
    st.seats
        .get(seat)
        .and_then(|s| s.flags.get(&format!("yj:{code}")))
        .copied()
        .unwrap_or(false)
}

/// 转换技切换（每次发动后调用）。
pub fn toggle_yj(st: &mut SgsState, seat: usize, code: &str) {
    if let Some(s) = st.seats.get_mut(seat) {
        let key = format!("yj:{code}");
        let cur = s.flags.get(&key).copied().unwrap_or(false);
        s.flags.insert(key, !cur);
    }
}

/// 「橘」标记数（陆绩·怀橘）。
pub fn ju_count(st: &SgsState, seat: usize) -> u32 {
    st.seats
        .get(seat)
        .and_then(|s| s.mark_counts.get("ju"))
        .copied()
        .unwrap_or(0)
}

/// 获得/移除「橘」标记。
pub fn add_ju(st: &mut SgsState, seat: usize, n: u32) {
    if let Some(s) = st.seats.get_mut(seat) {
        *s.mark_counts.entry("ju".into()).or_insert(0) += n;
    }
}

pub fn remove_ju(st: &mut SgsState, seat: usize, n: u32) {
    if let Some(s) = st.seats.get_mut(seat) {
        let cur = s.mark_counts.get("ju").copied().unwrap_or(0);
        let left = cur.saturating_sub(n);
        if left == 0 {
            s.mark_counts.remove("ju");
        } else {
            s.mark_counts.insert("ju".into(), left);
        }
    }
}

/// 存活角色的势力数（庸肆 X；至少 1）。
pub fn kingdom_count(st: &SgsState) -> u32 {
    let mut seen = std::collections::BTreeSet::new();
    for i in st.alive_indices() {
        if let Some(g) = st.seats[i]
            .general
            .as_deref()
            .and_then(data::general)
            .map(|d| d.kingdom)
        {
            seen.insert(g);
        }
    }
    (seen.len() as u32).max(1)
}

/// 「权」（钟会·权计）张数：武将牌上的牌。
pub fn quan_count(st: &SgsState, seat: usize) -> usize {
    mark_cards(st, seat, "quan").len()
}

/// 「逆」（刘封·陷嗣）张数：武将牌上的牌。
pub fn ni_count(st: &SgsState, seat: usize) -> usize {
    mark_cards(st, seat, "ni").len()
}

/// 无言（徐庶，锁定）：锦囊牌造成的伤害被防止（你使用或受到）。
pub fn wuyan_blocks(st: &SgsState, source: usize, victim: usize, card: Option<&str>) -> bool {
    let is_trick = card
        .and_then(|id| st.cards.get(id))
        .and_then(|c| data::card_def(&c.def))
        .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick));
    is_trick && (has(st, source, "wuyan") || has(st, victim, "wuyan"))
}

/// 暗箭（潘璋马忠，锁定）：你使用【杀】对不在你攻击范围内的角色造成伤害时 +1。
pub fn anjian_bonus(st: &SgsState, source: usize, victim: usize) -> i32 {
    if has(st, source, "anjian") && source != victim && !st.in_attack_range(victim, source) {
        1
    } else {
        0
    }
}

/// 复原武将牌（举荐）：翻回正面；返回是否发生了翻面。
pub fn restore_general(st: &mut SgsState, seat: usize) -> bool {
    let Some(s) = st.seats.get_mut(seat) else {
        return false;
    };
    let was = s.flipped;
    s.flipped = false;
    was
}

/// 自守（刘表）：本回合你防止对其他角色造成的伤害。
pub fn zishou_blocks_damage(st: &SgsState, source: usize, victim: usize) -> bool {
    source != victim && has(st, source, "zishou") && st.counter("zishou") > 0
}

/// 精策（郭淮）：记录本回合使用的花色/牌类型（位图：花色 1/2/4/8，类型基本/锦囊/装备 1/2/4）。
pub fn note_card_used(st: &mut SgsState, seat: usize, card_id: &str) {
    if !has(st, seat, "jingce") {
        return;
    }
    let Some(inst) = st.cards.get(card_id) else {
        return;
    };
    let suit_bit = match inst.suit {
        Suit::Spade => 1u32,
        Suit::Heart => 2,
        Suit::Club => 4,
        Suit::Diamond => 8,
    };
    let type_bit = match data::card_def(&inst.def).map(|d| d.category) {
        Some(Category::Basic) => 1u32,
        Some(Category::Trick | Category::DelayedTrick) => 2,
        Some(Category::Weapon | Category::Armor | Category::Horse | Category::Treasure) => 4,
        None => 0,
    };
    let suits = st.counter("jingce_suits") | suit_bit;
    st.set_counter("jingce_suits", suits);
    let types = st.counter("jingce_types") | type_bit;
    st.set_counter("jingce_types", types);
}

/// 精策：本回合使用过的牌的类型数（基本/锦囊/装备）。
pub fn jingce_type_count(st: &SgsState) -> u32 {
    st.counter("jingce_types").count_ones()
}

/// 龙吟（关平）：一名角色于其出牌阶段内使用【杀】时询问（弃一牌使该【杀】不计次数；红【杀】摸 1）。
pub fn longyin_trigger(st: &mut SgsState, user: usize, card: &str) {
    let Some(owner) = st
        .alive_indices()
        .into_iter()
        .find(|i| has(st, *i, "longyin") && !st.seats[*i].hand.is_empty())
    else {
        return;
    };
    crate::resolve::push(
        st,
        crate::resolve::Frame::LongYin(crate::resolve_yijiang3::LongYinFrame {
            owner,
            user,
            card: card.to_string(),
        }),
    );
}

/// 纵适（简雍）：拼点结算后获得拼点牌（赢 → 点数较小者；没赢 → 自己的拼点牌；自动）。
pub fn zongshi_on_pindian(
    st: &mut SgsState,
    initiator: usize,
    target: usize,
    winner: Option<usize>,
    initiator_card: &str,
    target_card: &str,
) {
    for seat in [initiator, target] {
        if !has(st, seat, "zongshi2") {
            continue;
        }
        let own = if seat == initiator {
            initiator_card
        } else {
            target_card
        };
        let other = if seat == initiator {
            target_card
        } else {
            initiator_card
        };
        let wanted = if winner == Some(seat) {
            let ro = st.cards.get(other).map(|c| c.rank).unwrap_or(0);
            let rs = st.cards.get(own).map(|c| c.rank).unwrap_or(0);
            if ro < rs {
                other
            } else {
                own
            }
        } else {
            own
        };
        let Some(pos) = st.deck.discard.iter().position(|c| c == wanted) else {
            continue;
        };
        st.deck.discard.remove(pos);
        st.seats[seat].hand.push(wanted.to_string());
        let label = crate::turn::card_label(st, wanted);
        st.push_log(
            Some(&st.seats[seat].player_id.clone()),
            format!("纵适：获得拼点牌【{label}】"),
        );
    }
}

/// 本回合禁止 `user` 对 `target` 使用牌（拒战）。
pub fn deny_target(st: &SgsState, user: usize, target: usize) -> bool {
    st.counter(&format!("deny:{user}:{target}")) > 0
}

pub fn set_deny_target(st: &mut SgsState, user: usize, target: usize) {
    st.set_counter(&format!("deny:{user}:{target}"), 1);
}

/// 成略：本阶段与弃置牌同花色的牌无距离/次数限制。
pub fn chenglue_free(st: &SgsState, _seat: usize, card_id: &str) -> bool {
    let Some(suit) = st.cards.get(card_id).map(|c| c.suit) else {
        return false;
    };
    let key = match suit {
        Suit::Spade => "cl_spade",
        Suit::Heart => "cl_heart",
        Suit::Club => "cl_club",
        Suit::Diamond => "cl_diamond",
    };
    st.counter(key) > 0
}

/// 恃才：本回合首次使用某一类型的牌后，将其置于牌堆顶，然后摸 1 张。
///
/// 口径：装备牌不触发（`kind` 只接受 `basic` / `trick`）；返回是否已触发。
pub fn shicai_trigger(st: &mut SgsState, seat: usize, kind: &str, card_id: &str) -> bool {
    if !has(st, seat, "shicai") || st.turn != seat {
        return false;
    }
    let key = format!("shicai:{kind}");
    if st.counter(&key) > 0 {
        return false;
    }
    // 牌必须仍在处理区（使用后尚未进弃牌堆）。
    if !st.deck.processing.iter().any(|c| c == card_id) {
        return false;
    }
    st.set_counter(&key, 1);
    crate::resolve::leave_processing(st, card_id);
    st.deck.draw.push(card_id.to_string());
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 1, &mut ev);
    let label = crate::turn::card_label(st, card_id);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("恃才：将【{label}】置于牌堆顶并摸 {drawn} 张"),
    );
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
    true
}

/// 额外技能（觉醒/化身）注册表（不在武将原始技能列表中的技能）。
pub static EXTRA_SKILLS: &[SkillDef] = &[
    SkillDef {
        code: "jixi",
        name: "急袭",
        kind: SkillType::Normal,
        implemented: true,
    },
    // SP-2 觉醒授予（2026-10-30 补齐官方文本后实现）。
    SkillDef {
        code: "qingce",
        name: "清侧",
        kind: SkillType::Normal,
        implemented: true,
    },
    SkillDef {
        code: "huairou",
        name: "怀柔",
        kind: SkillType::Normal,
        implemented: true,
    },
    // 界吕蒙·勤学（觉醒获得）：观看一名其他角色手牌，可弃置其中一张红桃牌。
    SkillDef {
        code: "gongxin",
        name: "攻心",
        kind: SkillType::Normal,
        implemented: true,
    },
    // 钟会·自立（觉醒获得）：移去一张「权」，令一名角色摸 2；若其手牌数大于你，你对其造成 1 点伤害。
    SkillDef {
        code: "paiyi",
        name: "排异",
        kind: SkillType::Normal,
        implemented: true,
    },
];

/// 按 code 查技能定义（武将原始技能 + 额外技能）。
pub fn def_of(code: &str) -> Option<&'static SkillDef> {
    for general in crate::data::GENERAL_PACKS.iter().flat_map(|p| p.iter()) {
        if let Some(def) = skills_of(general.code).iter().find(|s| s.code == code) {
            return Some(def);
        }
    }
    EXTRA_SKILLS.iter().find(|s| s.code == code)
}

/// 授予额外技能（去重）。
pub fn grant_skill(st: &mut SgsState, seat: usize, code: &str) {
    if let Some(s) = st.seats.get_mut(seat) {
        if !s.extra_skills.iter().any(|c| c == code) {
            s.extra_skills.push(code.to_string());
        }
    }
}

/// 失去基础技能（破势「失去决堰」等；本局生效）。
pub fn lose_skill(st: &mut SgsState, seat: usize, code: &str) {
    if let Some(s) = st.seats.get_mut(seat) {
        s.flags.insert(format!("lost:{code}"), true);
        s.extra_skills.retain(|c| c != code);
    }
}

/// 撤销额外技能。
pub fn revoke_skill(st: &mut SgsState, seat: usize, code: &str) {
    if let Some(s) = st.seats.get_mut(seat) {
        s.extra_skills.retain(|c| c != code);
    }
}

/// 武将第一个已实现的技能（化身用）。
pub fn first_implemented_skill(general: &str) -> Option<&'static str> {
    skills_of(general)
        .iter()
        .find(|s| s.implemented)
        .map(|s| s.code)
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
            if has(st, seat, "jixi")
                && crate::data::card_def(&inst.def).is_some_and(|d| d.equip_slot.is_some())
            {
                return Some(("jixi", "急袭"));
            }
            // 禁酒（高顺，锁定）：手牌【酒】视为【杀】。
            if has(st, seat, "jinjiu") && inst.def == "jiu" {
                return Some(("jinjiu", "禁酒"));
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
            // 龙胆（界赵云）：【酒】当【桃】。
            if has(st, seat, "longdan") && inst.def == "jiu" && is_jiexian(st, seat) {
                return Some(("longdan", "龙胆"));
            }
        }
        "wuxie_keji" => {
            // 看破：黑色手牌当【无懈可击】。
            if has(st, seat, "kanpo") && black {
                return Some(("kanpo", "看破"));
            }
        }
        "jiu" => {
            // 酒池：黑桃手牌当【酒】。
            if has(st, seat, "jiuchi") && inst.suit == Suit::Spade {
                return Some(("jiuchi", "酒池"));
            }
            // 龙胆（界赵云）：【桃】当【酒】。
            if has(st, seat, "longdan") && inst.def == "tao" && is_jiexian(st, seat) {
                return Some(("longdan", "龙胆"));
            }
        }
        _ => {}
    }
    None
}

/// 谦逊（锁定技）：不能成为【顺手牵羊】/【过河拆桥】的目标。
pub fn qianxun_blocks(st: &SgsState, seat: usize) -> bool {
    // 界谦逊改为「将手牌移出游戏」，不再屏蔽【顺手牵羊】/【过河拆桥】。
    has(st, seat, "qianxun") && !is_jiexian(st, seat)
}

/// 连营：失去最后的手牌后摸 1 张（自动发动）。
pub fn lianying_on_lose_last(st: &mut SgsState, seat: usize) {
    if !st.seats.get(seat).is_some_and(|s| s.hand.is_empty()) || !has(st, seat, "lianying") {
        return;
    }
    if is_jiexian(st, seat) {
        // 界连营：栈空后询问令一名角色摸 1（X 按失去最后一张 = 1 实现）。
        st.set_counter(&format!("lianying_pending:{seat}"), 1);
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
    // 旋风（凌统）：失去装备区的牌后触发（与枭姬同窗口）。
    if has(st, seat, "xuanfeng") && st.counter(&format!("xuanfeng:{}", seat)) == 0 {
        st.set_counter(&format!("xuanfeng:{}", seat), 1);
        crate::resolve::push(
            st,
            crate::resolve::Frame::XuanFeng(crate::resolve_yijiang::XuanFengFrame {
                owner: seat,
                remaining: 2,
                chosen: Vec::new(),
            }),
        );
    }
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

/// 诈降（界黄盖，锁定）：失去 1 点体力后摸 3 张；出牌阶段内 +1【杀】次数且红杀无距离/不可抵消。
pub fn zhaxiang_on_lose_hp(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "zhaxiang") {
        return;
    }
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 3, &mut ev);
    if st.turn == seat && st.turn_phase == crate::state::TurnPhase::Play {
        st.bump_counter("zhaxiang_sha_bonus");
        st.set_counter("zhaxiang_red", 1);
    }
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("诈降：失去体力，摸 {drawn} 张"),
    );
    let over = st.seats[seat]
        .hand
        .len()
        .saturating_sub(st.config.max_hand_cards as usize);
    if over > 0 {
        crate::resolve::push_discard(st, seat, over, "hand_cap");
    }
}

/// 博图（界吕蒙）：记录本回合进入弃牌堆的牌的花色（bitmask：黑桃1/红桃2/梅花4/方片8）。
pub fn note_discard_suit(st: &mut SgsState, card: &str) {
    let Some(c) = st.cards.get(card) else {
        return;
    };
    let bit = match c.suit {
        Suit::Spade => 1,
        Suit::Heart => 2,
        Suit::Club => 4,
        Suit::Diamond => 8,
    };
    let cur = st.counter("discard_suits");
    st.set_counter("discard_suits", cur | bit);
}

/// 界护驾：其他魏势力角色于其回合外代出【闪】后，主公摸 1（每回合限一次）。
pub fn hujia_lord_draw(st: &mut SgsState, by: usize) {
    if st.turn == by {
        return;
    }
    let Some(lord) = st.lord_index() else {
        return;
    };
    if !has(st, lord, "hujia") || !is_jiexian(st, lord) {
        return;
    }
    if st.counter("hujia_lord_draw") > 0 {
        return;
    }
    st.set_counter("hujia_lord_draw", 1);
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, lord, 1, &mut ev);
    st.push_log(
        Some(&st.seats[lord].player_id.clone()),
        format!("护驾：其他魏势力角色回合外代出【闪】，摸 {drawn} 张"),
    );
}

/// 界救援：其他吴势力角色于其回合内回复体力时，可改为让主公回复 1，然后其摸 1。
/// 返回 `true` 表示已改为救援主公（调用方不再回复原角色）。
pub fn jie_jiuyuan_redirect(st: &mut SgsState, seat: usize) -> bool {
    if st.turn != seat {
        return false;
    }
    let Some(lord) = st.lord_index() else {
        return false;
    };
    if lord == seat || !has(st, lord, "jiuyuan") || !is_jiexian(st, lord) {
        return false;
    }
    if kingdom(st, seat) != "wu" || st.seats[seat].hp < st.seats[lord].hp {
        return false;
    }
    if st.seats[lord].hp >= st.seats[lord].max_hp {
        return false;
    }
    st.seats[lord].hp += 1;
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 1, &mut ev);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!(
            "救援：{} 改为令主公 {} 回复 1 点体力，摸 {drawn} 张",
            st.seats[seat].general_name, st.seats[lord].general_name
        ),
    );
    true
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
    crate::resolve_jiexian::qingjian_trigger(st, owner);
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

/// 义从（锁定）：计算与其他角色的距离 -1。
pub fn yicong_attack_bonus(st: &SgsState, seat: usize) -> i32 {
    i32::from(has(st, seat, "yicong"))
}

/// 义从（锁定）：体力值 ≤2 时，其他角色计算与你的距离 +1。
pub fn yicong_defense_bonus(st: &SgsState, seat: usize) -> i32 {
    match st.seats.get(seat) {
        Some(s) if s.hp <= 2 && has(st, seat, "yicong") => 1,
        _ => 0,
    }
}

/// 武将势力 code（`shu` / `wu` / `wei` / `qun`）。
pub fn kingdom_of(st: &SgsState, seat: usize) -> &'static str {
    kingdom(st, seat)
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
    // 界闭月：没有手牌摸 2 张，否则摸 1 张。
    let n = if is_jiexian(st, idx) && st.seats[idx].hand.is_empty() {
        2
    } else {
        1
    };
    let drawn = draw_and_cap(st, idx, n, events);
    st.push_log(
        Some(&st.seats[idx].player_id.clone()),
        format!("闭月：摸 {drawn} 张"),
    );
}

/// 使用锦囊后的钩子：集智（使用非延时锦囊时摸 1；界版可弃基本牌换手牌上限）。
pub fn on_non_delayed_trick_used(st: &mut SgsState, seat: usize, events: &mut Vec<EngineEvent>) {
    if !has(st, seat, "jizhi") {
        return;
    }
    if is_jiexian(st, seat) {
        // 界集智：摸 1；若摸到基本牌，可弃置之使本回合手牌上限 +1（自动）。
        let Some(card) = crate::turn::draw_one(st, events) else {
            return;
        };
        let basic = st
            .cards
            .get(&card)
            .and_then(|c| data::card_def(&c.def))
            .is_some_and(|d| d.category == crate::state::Category::Basic);
        let label = crate::turn::card_label(st, &card);
        if basic {
            crate::turn::discard_by(st, Some(seat), &card);
            st.bump_counter("jizhi_hand_limit");
            st.push_log(
                Some(&st.seats[seat].player_id.clone()),
                format!("集智：摸到基本牌【{label}】，弃置后本回合手牌上限 +1"),
            );
        } else {
            st.seats[seat].hand.push(card);
            st.push_log(
                Some(&st.seats[seat].player_id.clone()),
                format!("集智：摸 1 张（{label}）"),
            );
        }
        // 超出手牌上限时立即弃牌（与 draw_and_cap 一致）。
        let over = st.seats[seat]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, seat, over, "hand_cap");
        }
        return;
    }
    let drawn = draw_and_cap(st, seat, 1, events);
    st.push_log(
        Some(&st.seats[seat].player_id.clone()),
        format!("集智：摸 {drawn} 张"),
    );
}

/// 弃牌阶段上限：克己（出牌阶段未使用过杀 → 跳过弃牌）；不屈（有「创」时上限为创数）。
pub fn hand_limit(st: &SgsState, seat: usize) -> usize {
    if has(st, seat, "buqu") && !buqu_cards(st, seat).is_empty() {
        return buqu_cards(st, seat).len();
    }
    if has(st, seat, "keji") && st.counter("sha_used") == 0 {
        return usize::MAX;
    }
    let mut limit = st.hand_limit(seat);
    // 将驰（曹彰，选项一）：本回合【杀】不计入手牌上限。
    if has(st, seat, "jiangchi") && st.counter(&format!("sha_banned:{seat}")) > 0 {
        let sha_count = st.seats[seat]
            .hand
            .iter()
            .filter(|id| {
                st.cards
                    .get(*id)
                    .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
            })
            .count();
        limit += sha_count;
    }
    // 清俭（界夏侯惇）：交牌后当前回合角色手牌上限 +1。
    if has(st, seat, "qingjian") && is_jiexian(st, seat) {
        limit += st.counter("qingjian_limit") as usize;
    }
    // 界洛神：本回合以此法获得的牌不计入手牌上限。
    if has(st, seat, "luoshen") && is_jiexian(st, seat) {
        limit += st.counter("luoshen_free") as usize;
    }
    // 界英姿（锁定）：手牌上限为体力上限。
    if has(st, seat, "yingzi") && is_jiexian(st, seat) {
        limit = st
            .seats
            .get(seat)
            .map(|s| s.max_hp.max(0) as usize)
            .unwrap_or(limit);
    }
    // 界集智：本回合弃置基本牌换手牌上限 +1。
    if has(st, seat, "jizhi") && is_jiexian(st, seat) {
        limit += st.counter("jizhi_hand_limit") as usize;
    }
    // 决堰（防具栏）：本回合手牌上限 +3。
    if has(st, seat, "jueyan") {
        limit += st.counter("jueyan_hand_limit") as usize;
    }
    // 陷阵（没赢）：本回合【杀】不计入手牌上限（等价于上限 + 手牌中【杀】数）。
    if has(st, seat, "xianzhen") && st.counter("xz_lose") > 0 {
        let sha_count = st.seats[seat]
            .hand
            .iter()
            .filter(|id| {
                st.cards
                    .get(*id)
                    .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
            })
            .count();
        limit += sha_count;
    }
    // 血裔（主公技，锁定）：其他群势力角色数 ×2 手牌上限。
    if has(st, seat, "xueyi") && is_lord(st, seat) {
        let allies = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != seat && kingdom(st, *i) == "qun")
            .count();
        limit += allies * 2;
    }
    // 宗室（刘表，锁定）：全场每有一个势力，手牌上限 +1。
    if has(st, seat, "zongshi") {
        limit += kingdom_count(st) as usize;
    }
    // 权计（钟会）：每有一张「权」手牌上限 +1。
    if has(st, seat, "quanji") {
        limit += quan_count(st, seat);
    }
    // 精策（郭淮）：本回合每使用一种花色的手牌，手牌上限 +1。
    if has(st, seat, "jingce") {
        limit += st.counter("jingce_suits").count_ones() as usize;
    }
    limit
}

/// 不屈「创」（武将牌上的牌）。
pub fn buqu_cards(st: &SgsState, seat: usize) -> Vec<String> {
    st.seats
        .get(seat)
        .and_then(|s| s.marks.get("buqu"))
        .cloned()
        .unwrap_or_default()
}

/// 不屈：濒死时翻出牌堆顶一张「创」；点数不重复则回复至 1 点体力。
/// 返回 `true` 表示已脱离濒死。
pub fn buqu_save(st: &mut SgsState, victim: usize) -> bool {
    let mut events = Vec::new();
    let Some(card) = crate::turn::draw_one(st, &mut events) else {
        return false;
    };
    let rank = st.cards.get(&card).map(|c| c.rank).unwrap_or(0);
    let existing: Vec<u8> = buqu_cards(st, victim)
        .iter()
        .filter_map(|id| st.cards.get(id).map(|c| c.rank))
        .collect();
    let pid = st.seats[victim].player_id.clone();
    let label = crate::turn::card_label(st, &card);
    if existing.contains(&rank) {
        crate::turn::to_discard_card(st, &card);
        st.push_log(
            Some(&pid),
            format!("不屈：新增【{label}】点数重复，置入弃牌堆（仍濒死）"),
        );
        false
    } else {
        st.seats[victim]
            .marks
            .entry("buqu".into())
            .or_default()
            .push(card);
        st.seats[victim].hp = 1;
        st.push_log(
            Some(&pid),
            format!("不屈：新增【{label}】点数不同，回复至 1 点体力"),
        );
        true
    }
}

/// 红颜（锁定）：黑桃视为红桃（按牌的持有者结算）。
pub fn effective_suit(st: &SgsState, seat: usize, card_id: &str) -> Option<Suit> {
    let suit = st.cards.get(card_id)?.suit;
    if suit == Suit::Spade && has(st, seat, "hongyan") {
        Some(Suit::Heart)
    } else {
        Some(suit)
    }
}

/// 奇谋：本回合与其他角色的距离 -X（X = 失去的体力值）。
pub fn qimou_bonus(st: &SgsState, seat: usize) -> i32 {
    if has(st, seat, "qimou") {
        st.counter("qimou") as i32
    } else {
        0
    }
}

/// 屯田：每有一张「田」，计算与其他角色的距离 -1。
pub fn tuotian_bonus(st: &SgsState, seat: usize) -> i32 {
    st.seats
        .get(seat)
        .and_then(|s| s.marks.get("tian"))
        .map(|v| v.len() as i32)
        .unwrap_or(0)
}

/// 激昂：使用/成为【决斗】或红色【杀】目标后摸 1。
pub fn jiang_trigger(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "jiang") {
        return;
    }
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, seat, 1, &mut ev);
    if drawn > 0 {
        st.push_log(Some(&st.seats[seat].player_id.clone()), "激昂：摸 1 张");
    }
}

/// 烈弓：使用【杀】指定目标后，满足条件则目标不能使用【闪】。
pub fn liegong_blocks(st: &SgsState, attacker: usize, victim: usize) -> bool {
    if !has(st, attacker, "liegong") {
        return false;
    }
    let hand = st.seats[victim].hand.len() as i32;
    let hp = st.seats[attacker].hp.max(0);
    let range = st.attack_range(attacker);
    hand >= hp || hand <= range
}

/// 天香：受到伤害时是否有可用红桃手牌。
pub fn tianxiang_available(st: &SgsState, victim: usize) -> bool {
    has(st, victim, "tianxiang")
        && st.seats[victim]
            .hand
            .iter()
            .any(|id| effective_suit(st, victim, id) == Some(Suit::Heart))
}

/// 奋激：手牌被弃置/获得后的可发动持有者（存活）。
pub fn fenji_owner(st: &SgsState, lost_by: usize) -> Option<usize> {
    let mut candidates: Vec<usize> = vec![lost_by];
    candidates.extend(st.seats_from_left_of(lost_by));
    candidates
        .into_iter()
        .find(|i| st.seats.get(*i).is_some_and(|s| s.alive) && has(st, *i, "fenji"))
}

/// 天义（没赢）：本回合不能使用【杀】。
pub fn sha_blocked(st: &SgsState, seat: usize) -> bool {
    (has(st, seat, "tianyi") && st.counter("tianyi_lose") > 0)
        || (has(st, seat, "xianzhen") && st.counter("xz_lose") > 0)
}

/// 奇才：使用锦囊无距离限制。
pub fn trick_distance_unlimited(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qicai")
}

/// 将驰（曹彰）：本回合不能使用或打出【杀】。
pub fn sha_play_banned(st: &SgsState, seat: usize) -> bool {
    st.counter(&format!("sha_banned:{seat}")) > 0
}

/// 潜袭（马岱）：本回合不能使用/打出某种颜色的手牌（1=红 2=黑）。
pub fn qianxi_banned(st: &SgsState, seat: usize, card: &str) -> bool {
    let banned = st.counter(&format!("qianxi:{seat}"));
    if banned == 0 {
        return false;
    }
    let red = st.cards.get(card).is_some_and(|c| is_red(c.suit));
    (banned == 1 && red) || (banned == 2 && !red)
}

/// 伤逝（张春华）：手牌数小于已损失体力值时，摸至该值。
pub fn shangshi_draw(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "shangshi") {
        return;
    }
    let lost = (st.seats[seat].max_hp - st.seats[seat].hp).max(0) as usize;
    let hand = st.seats[seat].hand.len();
    if lost > hand {
        let mut ev = Vec::new();
        let drawn = crate::turn::draw_cards(st, seat, (lost - hand) as u32, &mut ev);
        if drawn > 0 {
            st.push_log(
                Some(&st.seats[seat].player_id.clone()),
                format!("伤逝：摸至 {lost} 张（摸 {drawn}）"),
            );
        }
    }
}

/// 醇醪「醇」数量（程普）。
pub fn chun_count(st: &SgsState, seat: usize) -> usize {
    st.seats
        .get(seat)
        .and_then(|s| s.marks.get("chun"))
        .map(|v| v.len())
        .unwrap_or(0)
}

/// 缠怨（蛊惑质疑惩罚）：体力值为 1 时不能使用或打出手牌；也不能质疑于吉。
pub fn chanyuan_locked(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .is_some_and(|s| s.flags.contains_key("chanyuan") && s.hp == 1)
}

/// 势力判定（供跨模块使用）。
pub fn kingdom_is(st: &SgsState, seat: usize, wanted: &str) -> bool {
    kingdom(st, seat) == wanted
}

/// 屯田：回合外失去牌后判定，非红桃置于武将牌上为「田」。
pub fn tuotian_on_lose(st: &mut SgsState, seat: usize) {
    if !has(st, seat, "tuotian") {
        return;
    }
    crate::resolve::push(
        st,
        crate::resolve::Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner: seat,
            reason: "tuotian".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: serde_json::json!({ "owner": seat }),
            saved_outcome: None,
        }),
    );
}

/// 女性角色（离间/雌雄/肉林等）。
pub fn is_female(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .and_then(|s| s.general.as_deref())
        .and_then(crate::data::general)
        .is_some_and(|g| g.gender == "female")
}

/// 肉林：涉及董卓与女性的【杀】需两张【闪】。
pub fn roulin_need(st: &SgsState, attacker: usize, target: usize) -> bool {
    (has(st, attacker, "roulin") && is_female(st, target))
        || (has(st, target, "roulin") && is_female(st, attacker))
}

/// 帷幕：不能成为黑色锦囊牌的目标。
pub fn weimu_blocks(st: &SgsState, target: usize, card_id: &str) -> bool {
    has(st, target, "weimu") && st.cards.get(card_id).is_some_and(|c| is_black(c.suit))
}

/// 巨象/祸首：南蛮入侵免疫。
pub fn nanman_immune(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "juxiang") || has(st, seat, "huoshou")
}

/// 巨象：其他角色使用的南蛮结算后，获得该牌。
pub fn juxiang_owner(st: &SgsState, user: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != user && has(st, *i, "juxiang"))
}

/// 祸首：其他角色使用的南蛮，其代替使用者成为伤害来源。
pub fn huoshou_owner(st: &SgsState, user: usize) -> Option<usize> {
    st.alive_indices()
        .into_iter()
        .find(|i| *i != user && has(st, *i, "huoshou"))
}

/// 完杀：贾诩回合内，仅濒死者本人可使用【桃】（贾诩本人不受限）。
pub fn wansha_blocks(st: &SgsState, seat: usize, victim: usize) -> bool {
    seat != victim && seat != st.turn && has(st, st.turn, "wansha")
}

/// 截辎：其他角色跳过摸牌阶段后，你摸一张。
pub fn jiezi_on_skip_draw(st: &mut SgsState, skipper: usize) {
    for i in st.alive_indices() {
        if i == skipper || !has(st, i, "jiezi") {
            continue;
        }
        let mut ev = Vec::new();
        let drawn = crate::turn::draw_cards(st, i, 1, &mut ev);
        if drawn > 0 {
            st.push_log(
                Some(&st.seats[i].player_id.clone()),
                "截辎：其他角色跳过摸牌阶段，摸 1 张",
            );
        }
        let over = st.seats[i]
            .hand
            .len()
            .saturating_sub(st.config.max_hand_cards as usize);
        if over > 0 {
            crate::resolve::push_discard(st, i, over, "hand_cap");
        }
    }
}

/// 颂威：其他魏势力角色的黑色判定牌生效后，令曹丕摸一张（自动）。
pub fn songwei_on_judge(st: &mut SgsState, owner: usize, judge_id: &str) {
    if kingdom(st, owner) != "wei" {
        return;
    }
    if !st.cards.get(judge_id).is_some_and(|c| is_black(c.suit)) {
        return;
    }
    let Some(lord) = st
        .alive_indices()
        .into_iter()
        .find(|i| has(st, *i, "songwei") && is_lord(st, *i))
    else {
        return;
    };
    if lord == owner {
        return;
    }
    let mut ev = Vec::new();
    let drawn = crate::turn::draw_cards(st, lord, 1, &mut ev);
    if drawn > 0 {
        st.push_log(
            Some(&st.seats[lord].player_id.clone()),
            "颂威：魏势力黑色判定生效，摸 1 张",
        );
    }
}

/// 黄天：可接收牌的张角主公（其他群势力角色出牌阶段限一次）。
pub fn huangtian_owner(st: &SgsState, giver: usize) -> Option<usize> {
    if kingdom(st, giver) != "qun" {
        return None;
    }
    st.alive_indices()
        .into_iter()
        .find(|i| *i != giver && has(st, *i, "huangtian") && is_lord(st, *i))
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

// ---------------------------------------------------------------------------
// P4.4.1 SP-2 辅助（任/荣/箜声、废除栏、谦节、往烈、父荫、罪论、雄乱、镇骨）
// ---------------------------------------------------------------------------

/// 武将牌上的标记牌（任/荣/箜声）。
pub fn mark_cards(st: &SgsState, seat: usize, code: &str) -> Vec<String> {
    st.seats
        .get(seat)
        .and_then(|s| s.marks.get(code))
        .cloned()
        .unwrap_or_default()
}

/// 把一张牌置于武将牌上成为标记（从手牌移出；调用方负责先从原区域取牌）。
pub fn add_mark(st: &mut SgsState, seat: usize, code: &str, card: &str) {
    if let Some(s) = st.seats.get_mut(seat) {
        s.marks
            .entry(code.to_string())
            .or_default()
            .push(card.to_string());
    }
}

/// 从武将牌上取出一张标记牌（返回是否成功）。
pub fn remove_mark(st: &mut SgsState, seat: usize, code: &str, card: &str) -> bool {
    let Some(s) = st.seats.get_mut(seat) else {
        return false;
    };
    let Some(list) = s.marks.get_mut(code) else {
        return false;
    };
    let Some(pos) = list.iter().position(|c| c == card) else {
        return false;
    };
    list.remove(pos);
    if list.is_empty() {
        s.marks.remove(code);
    }
    true
}

/// 装备栏废除（永久）：弃置该槽装备并记录（枭姬按失去装备处理）。
pub fn abolish_slot(st: &mut SgsState, seat: usize, slot: EquipSlot) {
    if st.seats[seat].abolished_slots.contains(&slot) {
        return;
    }
    st.seats[seat].abolished_slots.push(slot);
    let old = st.seats[seat].equips.insert(slot, None);
    if let Some(Some(card)) = old {
        crate::turn::to_discard_card(st, &card);
        xiaoji_on_lose_equip(st, seat);
    }
}

/// 谦节（锁定）：不能被横置 / 成为延时锦囊与拼点目标。
pub fn qianjie_blocks_target(st: &SgsState, seat: usize) -> bool {
    has(st, seat, "qianjie")
}

/// 父荫（锁定）：每回合首次成为其他角色【杀】/【决斗】目标且对方手牌 ≥ 你 → 此牌无效。
pub fn fuyin_blocks(st: &SgsState, owner: usize, user: usize) -> bool {
    has(st, owner, "fuyin")
        && st.counter("fuyin_done") == 0
        && st.seats[user].hand.len() >= st.seats[owner].hand.len()
}

/// 往烈：「本阶段不能再使用牌」。
pub fn no_card_use(st: &SgsState, seat: usize) -> bool {
    st.seats
        .get(seat)
        .is_some_and(|_| st.counter("no_card_use") > 0)
}

/// 雄乱：本回合手牌被封（不能使用/打出手牌）。
pub fn hand_sealed(st: &SgsState, seat: usize) -> bool {
    st.counter(&format!("hand_sealed:{seat}")) > 0
}

/// 决堰（坐骑栏 / 雄乱）：使用牌无距离限制。
pub fn no_distance_turn(st: &SgsState, seat: usize) -> bool {
    st.counter("jueyan_no_distance") > 0
        || st.counter(&format!("xl_seat:{seat}")) > 0
        || st.counter("jiangchi_no_distance") > 0
        || st.counter("gongqi_unlimited") > 0
}

/// 界奇才（锁定）：防止其他角色弃置你装备区里的防具/宝物牌。
pub fn qicai_blocks_discard(st: &SgsState, target: usize, user: usize, slot: EquipSlot) -> bool {
    target != user
        && has(st, target, "qicai")
        && is_jiexian(st, target)
        && matches!(slot, EquipSlot::Armor | EquipSlot::Treasure)
}

/// 雄乱目标（本回合对其使用牌无距离与次数限制）。
pub fn xiongluan_target(st: &SgsState, seat: usize) -> Option<usize> {
    let t = st.counter("xl_target");
    if t == 0 {
        None
    } else if t as usize == seat {
        None
    } else {
        Some(t as usize - 1)
    }
}

/// 每回合首次（`key` 计数器）；返回是否首次（首次时置 1）。
pub fn first_each_turn(st: &mut SgsState, key: &str) -> bool {
    let k = format!("once:{key}");
    if st.counter(&k) > 0 {
        return false;
    }
    st.set_counter(&k, 1);
    true
}
