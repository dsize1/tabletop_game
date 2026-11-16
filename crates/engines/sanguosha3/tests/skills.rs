//! M4 技能层门禁：声明式 SkillDef + face + Subject 声明 + active 单一查询 + 授予技一等公民。

use sanguosha3::kernel::event::{EventKind, Subject};
use sanguosha3::kernel::skills::{self, Hook, SkillKind};
use sanguosha3::kernel::{Game, Seat};

fn seat(id: usize, skills: Vec<&str>) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        granted: vec![],
        skills: skills.into_iter().map(String::from).collect(),
        identity: None,
        flipped: false, ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 注册表：B0 7 技能 + B1 18 技能
// ---------------------------------------------------------------------------

#[test]
fn registry_has_all_batches() {
    let codes: Vec<&str> = skills::all().iter().map(|s| s.code).collect();
    for c in ["jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei"] {
        assert!(codes.contains(&c), "B0 技能 {c} 应注册");
    }
    for c in [
        "yongsi", "weidi", "qizhi", "jinzou", "juzhan", "feijun", "binglue", "huaiju", "yili",
        "zhenglun", "kuizhu", "chezheng", "lijun", "jianxiang", "shenshi", "chenglue", "shicai",
        "cunmu",
    ] {
        assert!(codes.contains(&c), "B1 技能 {c} 应注册");
    }
    for c in [
        "mingren", "zhenliang", "zhengu", "wanglie", "zuilun", "fuyin", "liangyin", "kongsheng",
        "zhengrong", "hongju", "qianjie", "jueyan", "poshi", "xiongluan", "congjian", "huairou",
        "qingce", "jizhi",
    ] {
        assert!(codes.contains(&c), "B2 技能 {c} 应注册");
    }
    for c in [
        "sanyao", "zhiman", "qice", "zhiyu", "quanji", "zili", "junxing", "yuce", "longyin",
        "paiyi",
    ] {
        assert!(codes.contains(&c), "B3 技能 {c} 应注册");
    }
    for c in [
        "xiansi", "duodao", "anjian", "danshou", "sidi", "shenduan", "yonglue", "pindi", "faen",
        "benxi", "zhongyong",
    ] {
        assert!(codes.contains(&c), "B4 技能 {c} 应注册");
    }
    for c in [
        "zanhui", "jiaojin", "fenli", "pingkou", "shenxing", "bingyi", "huituo", "mingjian",
        "xingshuai", "qianju", "qingxi", "huomo", "zuoding",
    ] {
        assert!(codes.contains(&c), "B5 技能 {c} 应注册");
    }
    for c in [
        "zhanjue", "qinwang", "wurong", "shizhi", "yanzhu", "xingxue", "zhaofu", "anguo",
        "yaoming", "huaiyi", "jigong", "shifei",
    ] {
        assert!(codes.contains(&c), "B6 技能 {c} 应注册");
    }
    for c in [
        "duliang", "fulin", "kuangbi", "zhige", "zongzuo", "jishe", "lianhuo", "guizao",
        "jiyu", "qinqing", "huisheng", "taoluan",
    ] {
        assert!(codes.contains(&c), "B7 技能 {c} 应注册");
    }
    for c in [
        "zhongjian", "caishi", "fumian", "daiyan", "wengua", "fuzhu", "shouxi", "huimin",
        "qingxian", "juexiang", "hexian", "rouxian", "liexian", "jixian", "jianzheng",
        "zhuandui", "tianbian", "funan", "jiexun", "bizhuan", "tongbo",
    ] {
        assert!(codes.contains(&c), "B8 技能 {c} 应注册");
    }
    for c in [
        "rende", "jijiang", "yijue", "wusheng", "tishen", "paoxiao", "yajiao", "longdan",
        "mashu", "tieqi", "kongcheng", "guanxing", "qicai",
    ] {
        assert!(codes.contains(&c), "B9 技能 {c} 应注册");
    }
    for c in [
        "qixi", "fenwei", "keji", "qinxue", "botu", "kurou", "zhaxiang", "yingzi", "fanjian",
        "liuli", "guose", "qianxun", "lianying", "zhiheng", "jiuyuan", "xiaoji", "jieyin",
        "gongxin",
    ] {
        assert!(codes.contains(&c), "B10 技能 {c} 应注册");
    }
    for c in [
        "jianxiong", "hujia", "fankui", "guicai", "ganglie", "qingjian", "tuxi", "luoyi", "tiandu",
        "yiji", "xunxun", "wangxi", "qingguo", "luoshen",
    ] {
        assert!(codes.contains(&c), "B11 技能 {c} 应注册");
    }
    for c in ["jijiu", "chuli", "wushuang", "liyu", "yicong", "qiaomeng", "lijian", "biyue"] {
        assert!(codes.contains(&c), "B12 技能 {c} 应注册");
    }
    for c in [
        "huashen", "xinsheng", "guidao", "leiji", "guhuo", "liegong", "zhuhai", "qianxin",
        "jianyan", "yaowu", "shizhan", "huangtian", "luanji", "xueyi", "xiangle", "fangquan",
        "ruoyu", "sishu",
        "zhiba", "jiang", "hunzi", "shensu", "shebian", "bazhen", "cangzhuo", "huoji", "kanpo",
        "lianhuan", "niepan", "kuanggu", "qimou", "hongyan", "tianxiang", "piaoling", "yinghun",
        "wulie", "jiandu",
        "roulin", "bengkuai", "jiuchi", "baonue", "tiaoxin", "zhiji",
        "tianyi", "hanzhan", "tuotian", "zaoxian", "jixi", "juxiang", "liereng", "changbiao",
        "weimu", "luanwu", "wansha", "haoshi", "dimeng", "qiaobian",
    ] {
        assert!(codes.contains(&c), "B13 技能 {c} 应注册");
    }
    for c in [
        "huoshou", "zaizhi", "duanliang", "jiezi", "duanchang", "beige", "quhu", "jieming",
        "qiangxi", "ninge", "shuangxiong", "zhijian", "guzheng", "pojun",
    ] {
        assert!(codes.contains(&c), "B16 技能 {c} 应注册");
    }
    for c in [
        "xuanfeng", "zhichi", "mingce", "jiangchi", "zongshi_jie", "qiaoshuo", "zongxuan",
        "zhiyan", "xianzhen", "jinjiu", "xuanhuo", "enyuan", "lihuo", "chunlao",
    ] {
        assert!(codes.contains(&c), "B17 技能 {c} 应注册");
    }
    for c in [
        "zhenlie", "miji", "jueqing", "shangshi", "jianmie", "zishou", "zongshi", "xianzhou",
        "qieting", "chengxiang", "renxin", "luoying", "jiushi", "ganlu", "buyi", "dangxian", "fuli",
    ] {
        assert!(codes.contains(&c), "B18 技能 {c} 应注册");
    }
    for c in [
        "mieji", "fencheng", "juece", "jingce", "zhuikong", "qiuyuan", "gongqi", "jiefan",
        "fuhen", "qianxi", "jiazhao", "danxin", "shibei", "jianying",
    ] {
        assert!(codes.contains(&c), "B19 技能 {c} 应注册");
    }
    assert_eq!(
        skills::all().len(),
        291,
        "B0–B18 277 + B19 14 新 + 马术复用（已在 B9 计入）"
    );
    assert!(skills::check_unique_codes().is_ok(), "技能 code 不得重复");
}

#[test]
fn songwei_is_lord_kind() {
    assert_eq!(skills::def("songwei").unwrap().kind, SkillKind::Lord);
}

#[test]
fn buqu_is_locked() {
    assert_eq!(skills::def("buqu").unwrap().kind, SkillKind::Locked);
}

// ---------------------------------------------------------------------------
// Subject 声明（M105 教训的类型级对策）
// ---------------------------------------------------------------------------

#[test]
fn fenji_is_hook_declared_not_event_subscribed() {
    // 奋激（B0 收口）：改为**手牌变动钩子**驱动（回合外 + 进弃牌堆/手牌增加，排除摸牌阶段），
    // 不再声明 CardDiscarded/CardGained 订阅——避免「事件 + 钩子」双触发。
    let fenji = skills::def("fenji").unwrap();
    assert!(fenji.faces[0].subscriptions.is_empty());
    assert!(fenji.faces[0].hooks.contains(&Hook::HandChanged));
}

#[test]
fn fangzhu_declares_self_subject() {
    // 放逐：受到伤害后 → subject = Self（自己的受伤事件）。
    let fangzhu = skills::def("fangzhu").unwrap();
    assert_eq!(fangzhu.faces[0].subscriptions[0].subject, Subject::Self_);
    assert_eq!(fangzhu.faces[0].subscriptions[0].event, EventKind::DamageAfter);
}

// ---------------------------------------------------------------------------
// 单一状态源：active = 自带 ∪ granted
// ---------------------------------------------------------------------------

#[test]
fn active_is_union_of_own_and_granted() {
    let mut g = Game {
        seats: vec![seat(0, vec!["jushou"])],
        ..Default::default()
    };
    assert!(skills::active(&g, 0, "jushou"), "自带技能 active");
    assert!(!skills::active(&g, 0, "gongxin"), "未授予的技不 active");

    // 授予攻心（一等公民）
    skills::grant(&mut g, 0, "gongxin");
    assert!(skills::active(&g, 0, "gongxin"), "授予后 active");
}

#[test]
fn grant_is_idempotent() {
    let mut g = Game { seats: vec![seat(0, vec![])], ..Default::default() };
    skills::grant(&mut g, 0, "huairou");
    skills::grant(&mut g, 0, "huairou");
    assert_eq!(g.seats[0].granted.iter().filter(|c| *c == "huairou").count(), 1, "重复授予去重");
}

#[test]
fn active_does_not_leak_between_seats() {
    let mut g = Game {
        seats: vec![seat(0, vec!["jushou"]), seat(1, vec![])],
        ..Default::default()
    };
    assert!(skills::active(&g, 0, "jushou"));
    assert!(!skills::active(&g, 1, "jushou"), "座位 1 没有据守");
    skills::grant(&mut g, 1, "gongxin");
    assert!(skills::active(&g, 1, "gongxin"));
    assert!(!skills::active(&g, 0, "gongxin"));
}

// ---------------------------------------------------------------------------
// face 化：多 face 支持（转换技骨架）
// ---------------------------------------------------------------------------

#[test]
fn skilldef_supports_multiple_faces() {
    // 构造一个双 face 技能（模拟转换技：阳=OwnTurn，阴=OtherTurn），验证声明式结构。
    let two_face = skills::SkillDef {
        code: "test_transform",
        name: "测试转换技",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: true,
        faces: &[
            skills::Face {
                window: sanguosha3::kernel::event::Window::OwnTurn,
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            skills::Face {
                window: sanguosha3::kernel::event::Window::OtherTurn,
                subscriptions: &[],
                hooks: &[],
                can_active: false,
            },
        ],
    };
    assert_eq!(two_face.faces.len(), 2);
    assert_eq!(two_face.faces[0].window, sanguosha3::kernel::event::Window::OwnTurn);
    assert_eq!(two_face.faces[1].window, sanguosha3::kernel::event::Window::OtherTurn);
}
