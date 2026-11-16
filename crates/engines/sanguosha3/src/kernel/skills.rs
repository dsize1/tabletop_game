//! 技能注册表 + **声明式 SkillDef** + 单一状态源（`active`）。
//!
//! 这是 M4 的核心（§6.3 技能模型从「handler」到「声明」）。关键差异 vs v2：
//! 1. **face 化**：转换技/主动+响应类拆多 face，每个 face 独立声明窗口（治「拒战阴面误写成他人回合」）；
//! 2. **subscription 带 Subject**：事件归属是声明不是守卫（M105 教训，治屯田/燕语在「他人失去牌」误触发）；
//! 3. **授予技一等公民**（O2=A）：`grant_only` + 所有技能（含怀柔/清侧/排异/攻心）都进注册表；
//! 4. **单一状态源**（O4=A）：`active()` = 自带 ∪ granted ∪ 临时（含伪帝借用），禁止 flags/counters
//!    各自表达「拥有技能」。
//!
//! 声明按批次分文件（`b0`/`b1`/…），本文件只放**类型 + 单一查询 + 路由 + 出口**。

use serde_json::{json, Value};

use super::event::{Event, EventKind, Subject, Window};
use super::resolve::{Ask, Stage, Step};
use super::{Game, Phase};

pub mod b0;
pub mod b1;
pub mod b2;
pub mod b3;
pub mod b4;
pub mod b5;
pub mod b6;
pub mod b7;
pub mod b8;
pub mod b9;
pub mod b10;
pub mod b11;
pub mod b12;
pub mod b13;
pub mod b14;
pub mod b15;
pub mod common;

/// 技能类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillKind {
    Normal,
    Locked,
    Limited,
    Awaken,
    Lord,
    /// 装备特效（O3=A：武器/防具 = 特殊技能）。
    Equip,
}

/// 钩子（归一 v2 的 30+ 个 `skills::xxx()` 散装函数）。技能只声明它挂了哪些钩子，
/// 内核/牌结算按钩子类型回调，不 match 技能名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    /// 游戏开始时（怀橘：开局 3 橘）。
    GameStart,
    /// 濒死钩子（不屈：濒死翻创判定）。
    Dying,
    /// 翻面钩子（解围②：翻回正面移动装备）。
    Flip,
    /// 无懈替代（解围①/看破：装备区牌当无懈）。
    WuxieOpt,
    /// 转化牌（武圣/龙胆/急救/丈八…）。
    Conversion,
    /// 距离修正（马术：-1）。
    Distance(i32),
    /// 【杀】次数覆盖（咆哮：无限）。
    ShaLimit,
    /// 手牌上限修正（血裔：+N）。
    HandLimit(i32),
    /// 摸牌方向（寸目：从牌堆底摸）。
    DrawFromBottom,
    /// 伤害防止（怀橘：自动移橘防伤）。
    DamagePrevent,
    /// 牌对你无效（掣政①：攻击范围不含你的角色对你使用牌无效）。
    CardIneffective,
    /// 【杀】指定目标之后（谏征/专对：目标列表重写 / 不能响应）。
    ShaTargeted,
    /// 手牌变动（奋激：回合外 手牌离开进弃牌堆 / 手牌增加）。
    HandChanged,
    /// 虚拟装备（八阵：无防具时视为装备【八卦阵】）：槽位 + 视为的牌定义 code。
    VirtualEquip(super::Slot, &'static str),
    /// 【杀】结算前介入（享乐/铁骑类目标锁定技）：响应窗口之前询问。
    ShaIntercept,
    /// 濒死求援顺序（兴衰：左手侧 → 自己最后）。
    DyingLeftFirst,
    /// 使用牌之前（往烈：首张无距离 / 不能响应 + 手牌冻结）。
    UsePre,
    /// 造成伤害时、伤害结算之前（制蛮：可防止并取牌）。
    DamageBefore,
    /// 濒死进入时（兴衰：其他魏势力逐个支援）。
    DyingEnter,
    /// 判定改判（鬼才=任意牌 / 鬼道=黑色牌）：候选由技能自己判定，询问按技能 code 分发。
    JudgeRewrite,
}

/// 订阅（声明式）：事件 + 主体。handler 首行按 `subject` 校验（不再靠人记）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    pub event: EventKind,
    pub subject: Subject,
}

impl Subscription {
    /// 订阅是否匹配该事件（事件类型 + 主体，相对 `owner`）。窗口由 `Face.window` 判定。
    pub fn matches(&self, ev: &Event, owner: usize) -> bool {
        self.event == ev.kind && self.subject.matches(ev, owner)
    }
}

/// 技能面：独立窗口 + 订阅 + 钩子。默认单 face；转换技/主动+响应类拆多 face。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    pub window: Window,
    pub subscriptions: &'static [Subscription],
    pub hooks: &'static [Hook],
    /// 是否有主动入口（出牌阶段可 `skill:<code>` 发动）。M4 后续把 bool 升级为条件函数。
    pub can_active: bool,
}

/// 技能定义（声明式；行为 handler 由 `SkillDef` 的 code 在 `skills::handler` 里分发，M4 分批接入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDef {
    pub code: &'static str,
    pub name: &'static str,
    pub kind: SkillKind,
    pub faces: &'static [Face],
    /// 授予技（一等公民）：不能被选将自带，只能被授予（攻心/怀柔/清侧/排异/弦技…）。
    pub grant_only: bool,
    /// **转换技**：只在「当前面」上订阅/可发动，每次发动后 `flip_face` 换面（§9.1 拆 face 清单）。
    pub transform: bool,
}

/// 全量注册表（按批次拼接）。**唯一事实源**：禁止在别处再列技能名清单。
pub fn all() -> Vec<&'static SkillDef> {
    b0::SKILLS
        .iter()
        .chain(b1::SKILLS.iter())
        .chain(b2::SKILLS.iter())
        .chain(b3::SKILLS.iter())
        .chain(b4::SKILLS.iter())
        .chain(b5::SKILLS.iter())
        .chain(b6::SKILLS.iter())
        .chain(b7::SKILLS.iter())
        .chain(b8::SKILLS.iter())
        .chain(b9::SKILLS.iter())
        .chain(b10::SKILLS.iter())
        .chain(b11::SKILLS.iter())
        .chain(b12::SKILLS.iter())
        .chain(b13::SKILLS.iter())
        .chain(b14::SKILLS.iter())
        .chain(b15::SKILLS.iter())
        .collect()
}

/// **已收口可挂载**的技能（逐条放开；未列出的技能即使有 handler 也不进入对局）。
///
/// B0 对齐进度：据守 ✅（本切片）→ 解围/不屈/奋激/行殇/放逐/颂威 按 `align/B0.json` 断言逐条补齐后追加；
/// B1 批次同理（收口后才追加，避免「形状绿、语义错」的技能进真机）。
pub const ENABLED_SKILLS: &[&str] = &[
    // B0 批次（按 `align/B0.json` 逐条收口）
    "jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei",
    // B1 批次（18 技能；真实路由 + 真实待办 + pass 出口，2026-11-15 收口）
    "yongsi", "weidi", "qizhi", "jinzou", "juzhan", "feijun", "binglue", "huaiju", "yili",
    "zhenglun", "kuizhu", "chezheng", "lijun", "jianxiang", "shenshi", "chenglue", "shicai",
    "cunmu",
    // B2 批次（15 技能 + 授予技 清侧/怀柔 + 临时技 集智，2026-11-15 收口）
    "mingren", "zhenliang", "zhengu", "wanglie", "zuilun", "fuyin", "liangyin", "kongsheng",
    "zhengrong", "hongju", "qianjie", "jueyan", "poshi", "xiongluan", "congjian", "huairou",
    "qingce", "jizhi",
    // B3 批次（9 技能 + 授予技 排异，2026-11-15 收口）
    "sanyao", "zhiman", "qice", "zhiyu", "quanji", "zili", "junxing", "yuce", "longyin",
    "paiyi",
    // B4 批次（8 将 / 11 技能，2026-11-15 收口）
    "xiansi", "duodao", "anjian", "danshou", "sidi", "shenduan", "yonglue", "pindi", "faen",
    "benxi", "zhongyong",
    // B5 批次（6 将 / 13 技能，2026-11-15 收口）
    "zanhui", "jiaojin", "fenli", "pingkou", "shenxing", "bingyi", "huituo", "mingjian",
    "xingshuai", "qianju", "qingxi", "huomo", "zuoding",
    // B6 批次（7 将 / 12 技能，2026-11-16 收口）
    "zhanjue", "qinwang", "wurong", "shizhi", "yanzhu", "xingxue", "zhaofu", "anguo",
    "yaoming", "huaiyi", "jigong", "shifei",
    // B7 批次（7 将 / 12 技能，2026-11-16 收口）
    "duliang", "fulin", "kuangbi", "zhige", "zongzuo", "jishe", "lianhuo", "guizao",
    "jiyu", "qinqing", "huisheng", "taoluan",
    // B8 批次（8 将 / 17 技能，2026-11-16 收口）
    "zhongjian", "caishi", "fumian", "daiyan", "wengua", "fuzhu", "shouxi", "huimin",
    "qingxian", "juexiang", "hexian", "rouxian", "liexian", "jixian", "jianzheng",
    "zhuandui", "tianbian", "funan", "jiexun", "bizhuan", "tongbo",
    // B9 批次（界标·蜀：7 将 / 14 技能，2026-11-16 收口）
    "rende", "jijiang", "yijue", "wusheng", "tishen", "paoxiao", "yajiao", "longdan",
    "mashu", "tieqi", "kongcheng", "guanxing", "qicai",
    // B10 批次（界标·吴：8 将 / 17 技能，2026-11-16 收口）
    "qixi", "fenwei", "keji", "qinxue", "botu", "kurou", "zhaxiang", "yingzi", "fanjian",
    "liuli", "guose", "qianxun", "lianying", "zhiheng", "jiuyuan", "xiaoji", "jieyin",
    // 勤学授予（grant_only；B10 收口缺口 ④）
    "gongxin",
    // B11 批次（界标·魏：8 将 / 14 技能，judge-rewrite + 摸牌阶段替代）
    "jianxiong", "hujia", "fankui", "guicai", "ganglie", "qingjian", "tuxi", "luoyi", "tiandu",
    "yiji", "xunxun", "wangxi", "qingguo", "luoshen",
    // B12 批次（界标·群：4 将 / 8 技能）
    "jijiu", "chuli", "wushuang", "liyu", "yicong", "qiaomeng", "lijian", "biyue",
    // B13 批次（J1：进行中，见 check:sgs3-registry 的 ⏳ 计数）
    "huashen", "xinsheng", "guidao", "leiji", "guhuo",
    "liegong", "zhuhai", "qianxin", "jianyan", "yaowu", "shizhan", "huangtian",
    "luanji", "xueyi", "xiangle", "fangquan", "ruoyu", "sishu",
    // B14 批次（J2：8 将 / 20 技能；马术复用 B9）
    "zhiba", "jiang", "hunzi", "shensu", "shebian", "bazhen", "cangzhuo", "huoji", "kanpo",
    "lianhuan", "niepan", "kuanggu", "qimou", "hongyan", "tianxiang", "piaoling", "yinghun",
    "wulie", "jiandu",
    // B15 批次（J3：进行中，见 check:sgs3-registry 的 ⏳ 计数）
    "roulin", "bengkuai", "jiuchi", "baonue", "tiaoxin", "zhiji",
    "tianyi", "hanzhan", "tuotian", "zaoxian", "jixi", "juxiang", "liereng", "changbiao",
    "weimu", "luanwu", "wansha", "haoshi", "dimeng", "qiaobian",
];

/// 手牌变动类型（奋激口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandChange {
    /// 手牌离开并进入弃牌堆。
    Discarded,
    /// 从其他区域/角色获得。
    Gained,
    /// 摸牌（摸牌阶段不计）。
    Drawn,
}

/// **翻面（唯一入口）**：取反；`背面 → 正面` 时触发 `Hook::Flip`（解围②等）。
pub fn flip_seat(g: &mut Game, seat: usize) {
    let was_flipped = g.seats.get(seat).map(|s| s.flipped).unwrap_or(false);
    if let Some(s) = g.seats.get_mut(seat) {
        s.flipped = !was_flipped;
    }
    if was_flipped {
        let mut ev = super::event::Event::simple(
            super::event::EventKind::Flipped,
            Some(seat),
            Some("face_up"),
        );
        ev.turn = g.turn;
        ev.phase = g.phase;
        g.push_event(ev);
        fire_hook(g, seat, Hook::Flip);
    } else {
        let mut ev = super::event::Event::simple(
            super::event::EventKind::Flipped,
            Some(seat),
            Some("face_down"),
        );
        ev.turn = g.turn;
        ev.phase = g.phase;
        g.push_event(ev);
    }
}

/// 触发某座位的某个钩子（**已收口**技能才触发；压技能帧，不直接执行）。
pub fn fire_hook(g: &mut Game, seat: usize, hook: Hook) {
    let mut to_push: Vec<&'static str> = Vec::new();
    for skill in all() {
        if !enabled(skill.code) || !active(g, seat, skill.code) {
            continue;
        }
        for face in skill.faces {
            if face.hooks.contains(&hook) && face.window.allows(g.turn, g.phase, seat) {
                to_push.push(skill.code);
                break;
            }
        }
    }
    for code in to_push {
        super::resolve::push_ask(
            g,
            code,
            Stage::Choose,
            seat,
            serde_json::json!({ "hook": format!("{hook:?}"), "step": "ask" }),
        );
    }
}

/// 手牌上限**覆盖**（不屈：「创」存在时 = 创数，取代体力值）。`None` = 用默认口径。
pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    // 赋值型（急攻：本回合上限 = 造成的伤害值）。
    if let Some(n) = b6::hand_limit_override(g, seat) {
        return Some(n);
    }
    if let Some(n) = b10::hand_limit_override(g, seat) {
        return Some(n);
    }
    if let Some(n) = b14::hand_limit_override(g, seat) {
        return Some(n);
    }
    if active(g, seat, "buqu") && enabled("buqu") {
        let n = g.counter(&format!("buqu_total.{seat}"));
        if n > 0 {
            return Some(n as usize);
        }
    }
    None
}

/// 手牌变动钩子（奋激）：满足「回合外 + 手牌离开进弃牌堆 / 手牌增加」时，
/// 从变动者开始按座位顺序找**第一个**拥有奋激的存活角色并询问。
pub fn on_hand_changed(g: &mut Game, seat: usize, change: HandChange) {
    if !enabled("fenji") || !g.started {
        return;
    }
    // 防递归：奋激自己造成的摸牌不再次触发。
    let suppress = format!("fenji.suppress.{seat}");
    if g.counter(&suppress) > 0 {
        return;
    }
    // 摸牌阶段摸牌不计。
    if change == HandChange::Drawn && g.phase == Phase::Draw {
        return;
    }
    if !g.seats.get(seat).map(|s| s.alive).unwrap_or(false) {
        return;
    }
    // 窗口：动作发生时不是该角色的回合（回合外）。
    if g.turn == seat {
        return;
    }
    let n = g.seats.len();
    let holder = (0..n)
        .map(|k| (seat + k) % n)
        .find(|s| g.seats[*s].alive && active(g, *s, "fenji"));
    let Some(holder) = holder else { return };
    super::resolve::push_ask(
        g,
        "fenji",
        Stage::Choose,
        holder,
        serde_json::json!({ "step": "ask", "victim": seat }),
    );
}

/// 技能是否已收口可挂载（选将挂载与 AI/视图共用）。
pub fn enabled(code: &str) -> bool {
    ENABLED_SKILLS.contains(&code)
}

/// 技能**显示名**（升级技改名：复难 → 复难plus；未升级时同注册名）。
pub fn display_name(g: &Game, seat: usize, code: &str) -> String {
    if let Some(name) = b8::display_name(g, seat, code) {
        return name.to_string();
    }
    def(code).map(|d| d.name.to_string()).unwrap_or_else(|| code.to_string())
}

/// 技能中文名 → code（只查**已注册**技能；未接入批次的武将技能名返回 None）。
pub fn code_by_name(name: &str) -> Option<&'static str> {
    all().into_iter().find(|s| s.name == name).map(|s| s.code)
}

/// 查技能定义。
pub fn def(code: &str) -> Option<&'static SkillDef> {
    all().into_iter().find(|s| s.code == code)
}

/// 自带 ∪ granted（**不含**伪帝借用）。
fn owned(g: &Game, seat: usize, code: &str) -> bool {
    let Some(s) = g.seats.get(seat) else {
        return false;
    };
    s.skills.iter().any(|c| c == code) || s.granted.iter().any(|c| c == code)
}

/// **单一状态源**（O4=A / mechanics `skill-status`）：自带 ∪ granted ∪ 借用。
///
/// 借用 = 伪帝（B1）：「视为拥有当前主公的主公技」，只参与联动判定，不能主动使用（见 `can_active`）。
pub fn active(g: &Game, seat: usize, code: &str) -> bool {
    // 缠怨（Q6 裁定）：体力值为 1 时**所有技能**失效（含锁定技/装备特效）。
    // 独立闸门（持续整局），不复用回合制 `suppress.*`。
    if g.counter(&format!("chanyuan.{seat}")) > 0
        && g.seats.get(seat).map(|s| s.hp == 1).unwrap_or(false)
    {
        return false;
    }
    // 完杀（K7b）：某人濒死结算中，除贾诩与濒死者外的角色**非锁定技**失效。
    if b15::suppressed_in_dying(g, seat) {
        if let Some(d) = def(code) {
            if !matches!(d.kind, SkillKind::Locked | SkillKind::Equip) {
                return false;
            }
        }
    }
    // 义绝/铁骑：「本回合非锁定技失效」——锁定技与装备特效不受影响。
    if g.counter(&format!("suppress.{seat}.{}", g.turn)) > 0 {
        if let Some(d) = def(code) {
            if !matches!(d.kind, SkillKind::Locked | SkillKind::Equip) {
                return false;
            }
        }
    }
    if owned(g, seat, code) {
        return true;
    }
    let Some(d) = def(code) else {
        return false;
    };
    if d.kind != SkillKind::Lord || !owned(g, seat, "weidi") {
        return false;
    }
    // 只借「当前主公实际拥有」的主公技（B1 Q3 裁定 A）。
    g.current_lord()
        .map(|lord| owned(g, lord, code))
        .unwrap_or(false)
}

/// 能否**主动使用**（出牌阶段 skill 入口）：借来的技能不可主动使用（B1 伪帝裁定）。
pub fn can_active(g: &Game, seat: usize, code: &str) -> bool {
    if !owned(g, seat, code) {
        return false;
    }
    def(code)
        .map(|d| d.faces.iter().any(|f| f.can_active))
        .unwrap_or(false)
}

/// 授予技能（唯一入口；授予技一等公民，grant 后 `active()` 立即可查到）。
pub fn grant(g: &mut Game, seat: usize, code: &str) {
    let Some(s) = g.seats.get_mut(seat) else {
        return;
    };
    if !s.granted.iter().any(|c| c == code) {
        s.granted.push(code.to_string());
    }
}

/// 转换技当前面（0 起；默认 0）。
pub fn face_of(g: &Game, seat: usize, code: &str) -> usize {
    g.seats
        .get(seat)
        .and_then(|s| s.faces.get(code).copied())
        .unwrap_or(0) as usize
}

/// 转换技换面（只有 `transform = true` 的技能才应调用；按 face 数取模）。
pub fn flip_face(g: &mut Game, seat: usize, code: &str) {
    let Some(d) = def(code) else { return };
    if !d.transform || d.faces.len() < 2 {
        return;
    }
    let next = (face_of(g, seat, code) + 1) % d.faces.len();
    if let Some(s) = g.seats.get_mut(seat) {
        s.faces.insert(code.to_string(), next as u8);
    }
}

/// 技能 handler 分发（`resolve::advance` 用；按 code 找分批 handler）。
pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    b0::handler(code)
        .or_else(|| b1::handler(code))
        .or_else(|| b2::handler(code))
        .or_else(|| b3::handler(code))
        .or_else(|| b4::handler(code))
        .or_else(|| b5::handler(code))
        .or_else(|| b6::handler(code))
        .or_else(|| b7::handler(code))
        .or_else(|| b8::handler(code))
        .or_else(|| b9::handler(code))
        .or_else(|| b10::handler(code))
        .or_else(|| b11::handler(code))
        .or_else(|| b12::handler(code))
        .or_else(|| b13::handler(code))
        .or_else(|| b14::handler(code))
        .or_else(|| b15::handler(code))
}

/// 主动技的**可用性判定**（声明之外的前置：手牌/目标/每阶段次数等）。
/// 未提供判定的技能视为「不可主动使用」（安全默认：不向 UI 提供无效入口）。
pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    b0::availability(code)
        .or_else(|| b1::availability(code))
        .or_else(|| b2::availability(code))
        .or_else(|| b3::availability(code))
        .or_else(|| b4::availability(code))
        .or_else(|| b5::availability(code))
        .or_else(|| b6::availability(code))
        .or_else(|| b7::availability(code))
        .or_else(|| b8::availability(code))
        .or_else(|| b9::availability(code))
        .or_else(|| b10::availability(code))
        .or_else(|| b11::availability(code))
        .or_else(|| b12::availability(code))
        .or_else(|| b13::availability(code))
        .or_else(|| b14::availability(code))
        .or_else(|| b15::availability(code))
}

/// **主动入口合法集**（唯一来源）：自带技能 ∩ 当前面 can_active ∩ 窗口允许 ∩ 可用性判定
/// ∩ 限定技未使用（mechanics `limit-skill`）。
///
/// 禁止各处拼「技能名清单」；UI 的 `skill:<code>` 选项与 `use_active` 校验都读这里。
pub fn active_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    all()
        .into_iter()
        .filter(|d| active_action_allowed(g, seat, d))
        .map(|d| d.code)
        .collect()
}

/// 单个技能定义是否可主动发动（纯函数，供门禁/用例直接测）。
///
/// 限定技（`SkillKind::Limited`）每局只能发动一次：已使用 → 不再出现在合法集/选项里。
pub fn active_action_allowed(g: &Game, seat: usize, def: &SkillDef) -> bool {
    if !owned(g, seat, def.code) {
        return false;
    }
    if !limit_gate(def.kind, limited_used(g, seat, def.code)) {
        return false;
    }
    if !def
        .faces
        .iter()
        .any(|f| f.can_active && f.window.allows(g.turn, g.phase, seat))
    {
        return false;
    }
    availability(def.code).map(|f| f(g, seat)).unwrap_or(false)
}

/// 限定技门禁（mechanics `limit-skill`）：已使用的限定技不再出现在合法集/选项里。
pub fn limit_gate(kind: SkillKind, used: bool) -> bool {
    !(kind == SkillKind::Limited && used)
}

/// 限定技是否已使用（每局一次；存座位计数器，可回放）。
pub fn limited_used(g: &Game, seat: usize, code: &str) -> bool {
    g.seats
        .get(seat)
        .map(|s| s.counter(&format!("skill.limited.{code}")) > 0)
        .unwrap_or(false)
}

/// 标记限定技已使用（唯一写入口；发动时调用）。
pub fn mark_limited_used(g: &mut Game, seat: usize, code: &str) {
    if let Some(s) = g.seats.get_mut(seat) {
        s.add_counter(&format!("skill.limited.{code}"), 1);
    }
}

/// 是否有【杀】结算前介入技能（`Hook::ShaIntercept`，mechanics `sha-intercept`）。
pub fn has_sha_intercept(g: &Game, seat: usize) -> bool {
    all().into_iter().any(|s| {
        enabled(s.code)
            && active(g, seat, s.code)
            && s.faces
                .iter()
                .any(|f| f.hooks.iter().any(|h| *h == Hook::ShaIntercept))
    })
}

/// 发起【杀】结算前介入询问（每个持有者一帧；handler 用 `card_play::declare_intercept` 写结果）。
pub fn fire_sha_intercept(g: &mut Game, by: usize, target: usize, card: &str) {
    let mut owners: Vec<&'static str> = Vec::new();
    for s in all() {
        if !enabled(s.code) || !active(g, target, s.code) {
            continue;
        }
        if s.faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| *h == Hook::ShaIntercept))
        {
            owners.push(s.code);
        }
    }
    for code in owners {
        super::resolve::push_ask(
            g,
            code,
            Stage::Choose,
            target,
            json!({ "step": "sha_intercept", "by": by, "target": target, "card": card }),
        );
    }
}

/// 虚拟装备（mechanics `virtual-equip`）：真实同类装备优先；虚拟装备不占槽、不参与守恒。
///
/// 传入 defs 便于用例构造（生产入口 `virtual_equip` 用全量注册表）。
pub fn virtual_equip_in(
    defs: &[&SkillDef],
    g: &Game,
    seat: usize,
    slot: super::Slot,
) -> Option<&'static str> {
    // 真实同类装备优先（有真实装备时虚拟装备失效）。
    if super::gear::equip_in(g, seat, slot).is_some() {
        return None;
    }
    for d in defs {
        let provides = d
            .faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| matches!(h, Hook::VirtualEquip(s, _) if *s == slot)));
        if !provides {
            continue;
        }
        if !g
            .seats
            .get(seat)
            .map(|s| s.skills.iter().any(|c| c == d.code) || s.granted.iter().any(|c| c == d.code))
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(f) = availability(d.code) {
            if !f(g, seat) {
                continue;
            }
        }
        for face in d.faces {
            for h in face.hooks {
                if let Hook::VirtualEquip(s, code) = h {
                    if *s == slot {
                        return Some(code);
                    }
                }
            }
        }
    }
    None
}

/// 生产入口：当前座位的虚拟装备（真实槽位为空时才该被调用）。
pub fn virtual_equip(g: &Game, seat: usize, slot: super::Slot) -> Option<&'static str> {
    virtual_equip_in(&all(), g, seat, slot)
}

/// 该座位在该阶段是否有「阶段开始」订阅（需要**先询问再执行**阶段自动动作）。
///
/// 用途（turn 驱动）：无技能时不额外增加推进步数；有阶段开始技能时先路由询问，
/// 询问结算完再执行摸牌/判定等自动动作（整论/遗礼类）。
pub fn has_phase_start_sub(g: &Game, seat: usize, phase: Phase) -> bool {
    let kind = EventKind::PhaseStart;
    all().into_iter().any(|s| {
        if !enabled(s.code) || !active(g, seat, s.code) {
            return false;
        }
        let face_now = face_of(g, seat, s.code);
        s.faces.iter().enumerate().any(|(idx, f)| {
            if s.transform && idx != face_now {
                return false;
            }
            f.window.allows(g.turn, phase, seat)
                && f.subscriptions.iter().any(|sub| sub.event == kind)
        })
    })
}

/// 濒死求援是否左手侧优先（兴衰；`Hook::DyingLeftFirst`）。
pub fn dying_left_first(g: &Game, target: usize) -> bool {
    all().into_iter().any(|s| {
        enabled(s.code)
            && active(g, target, s.code)
            && s.faces
                .iter()
                .any(|f| f.hooks.iter().any(|h| *h == Hook::DyingLeftFirst))
    })
}

/// 发动主动技（唯一入口）：校验合法 → 压技能帧（`Stage::Choose`）→ 推进到第一个 pending。
pub fn use_active(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    start_active(g, seat, code)?;
    super::resolve::advance(g)
}

/// 只压帧不推进（内核回合流程帧内使用；**禁止**在流程帧 handler 里递归 `advance`）。
pub fn start_active(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if !active_actions(g, seat).iter().any(|c| *c == code) {
        return Err(format!("技能 {code} 当前不可主动使用"));
    }
    let face = face_of(g, seat, code);
    super::resolve::push_ask(
        g,
        code,
        Stage::Choose,
        seat,
        serde_json::json!({ "active": true, "face": face, "step": "choose" }),
    );
    Ok(())
}

/// 事件快照 → handler 上下文（`data`）。**只读快照**，不读 live 状态。
pub fn event_payload(ev: &Event, face: usize) -> Value {
    let mut m = serde_json::Map::new();
    // 事件类型（声明式订阅之外，handler 需要区分事件种类时读它；不写技能名/牌名）。
    m.insert("event".into(), json!(format!("{:?}", ev.kind)));
    if let Some(s) = ev.seat {
        m.insert("victim".into(), json!(s));
    }
    // `target`：伤害事件的「目标」按 B0 口径取施加者（`by`），其余取事件主体。
    let target = match ev.kind {
        EventKind::DamageAfter => ev.by,
        _ => ev.seat,
    };
    if let Some(t) = target {
        m.insert("target".into(), json!(t));
    }
    if let Some(c) = &ev.card {
        m.insert("card".into(), json!(c));
    }
    if let Some(c) = &ev.cause {
        m.insert("cause".into(), json!(c));
    }
    if let Some(a) = ev.amount {
        m.insert("amount".into(), json!(a));
    }
    if let Some(b) = ev.by {
        m.insert("by".into(), json!(b));
    }
    if let Some(f) = ev.from {
        m.insert("from".into(), json!(f));
    }
    m.insert("turn".into(), json!(ev.turn));
    m.insert("phase".into(), json!(format!("{:?}", ev.phase)));
    m.insert("face".into(), json!(face));
    Value::Object(m)
}

/// 事件路由：把事件快照按 SKILLS 的订阅（窗口 + subject）路由给技能帧。
/// 先收集匹配（避免借用冲突），再统一压帧。
/// 开局/发牌等 `Lifecycle::Setup` 事件**不路由**（明任开局置任的教训）。
pub fn route_event(g: &mut Game, ev: &Event) {
    if ev.lifecycle == super::event::Lifecycle::Setup {
        return;
    }
    let mut to_push: Vec<(&'static str, usize, Value)> = Vec::new();
    for seat in 0..g.seats.len() {
        for skill in all() {
            if !active(g, seat, skill.code) {
                continue;
            }
            let face_now = face_of(g, seat, skill.code);
            let mut fired = false;
            for (idx, face) in skill.faces.iter().enumerate() {
                // 转换技：只有当前面订阅/可发动。
                if skill.transform && idx != face_now {
                    continue;
                }
                if !face.window.allows(ev.turn, ev.phase, seat) {
                    continue;
                }
                for sub in face.subscriptions {
                    if sub.matches(ev, seat) {
                        to_push.push((skill.code, seat, event_payload(ev, idx)));
                        fired = true;
                        break; // 一次事件一个技能最多触发一次
                    }
                }
                if fired {
                    break;
                }
            }
        }
    }
    for (code, seat, data) in to_push {
        super::resolve::push_ask(g, code, Stage::Choose, seat, data);
    }
}

/// 游戏开始钩子（`Hook::GameStart`）：开局一次性数值/标记初始化（怀橘 3 橘…）。
///
/// 在 `turn::begin_game` 末尾调用；handler 只允许同步结算（不得产生 pending）。
pub fn fire_game_start(g: &mut Game) {
    let mut fires: Vec<(&'static str, usize, usize)> = Vec::new();
    for seat in 0..g.seats.len() {
        for skill in all() {
            if !owned(g, seat, skill.code) {
                continue;
            }
            for (idx, face) in skill.faces.iter().enumerate() {
                if face.hooks.contains(&Hook::GameStart) {
                    fires.push((skill.code, seat, idx));
                }
            }
        }
    }
    for (code, seat, idx) in fires {
        if let Some(h) = handler(code) {
            let ask = Ask {
                skill: code.to_string(),
                stage: Stage::Choose,
                actor: seat,
                data: json!({ "hook": "game_start", "face": idx }),
            };
            let _ = h(g, &ask);
        }
    }
}

// ---------------------------------------------------------------------------
// 数值/回合钩子的**唯一出口**（内核不按技能名判定；这里委托给批次实现）
// ---------------------------------------------------------------------------

/// 摸牌阶段摸牌数加成（庸肆 +X / 怀橘 +1…）。`flow::draw_phase` 唯一调用点。
pub fn draw_bonus(g: &mut Game, seat: usize) -> usize {
    b1::draw_bonus(g, seat)
        + b7::draw_bonus(g, seat)
        + b8::draw_bonus(g, seat)
        + b10::draw_bonus(g, seat)
}

/// 摸牌方向：true = 从牌堆底摸（寸目）。`flow::draw` 唯一调用点。
pub fn draw_from_bottom(g: &Game, seat: usize) -> bool {
    b1::draw_from_bottom(g, seat)
}

/// 本回合摸牌阶段是否被整段跳过（整论）。`flow::draw_phase` 唯一调用点。
pub fn skip_draw(g: &Game, seat: usize) -> bool {
    b1::skip_draw(g, seat) || b11::skip_draw(g, seat)
}

/// 伤害防止（怀橘，锁定自动）。`flow::damage` 唯一调用点；true = 本次伤害被防止。
pub fn prevent_damage(g: &mut Game, victim: usize) -> bool {
    b1::prevent_damage(g, victim) || b14::prevent_damage(g, victim)
}

/// 弃牌阶段手牌计数（藏拙：未使用锦囊时锦囊不计入）。
pub fn hand_size_for_discard(g: &Game, seat: usize) -> usize {
    b14::hand_size_for_discard(g, seat)
}

/// 看破：转化的【无懈可击】不能被响应。
pub fn wuxie_no_chain(g: &Game, seat: usize, card: &str) -> bool {
    b14::wuxie_no_chain(g, seat, card)
}

/// 连环：重铸算「使用牌」。
pub fn on_recast(g: &mut Game, seat: usize, card: &str) {
    b14::on_recast(g, seat, card)
}

/// 红颜：使用/打出时虚拟牌的花色（`card_play::push_virtual_use` 唯一调用点）。
pub fn suit_for_use(g: &Game, seat: usize, physical: super::Suit) -> super::Suit {
    b14::suit_for_use(g, seat, physical)
}

/// 红颜：判定牌花色（`judge::reveal/effective` 唯一调用点）。
pub fn suit_for_judge(g: &Game, seat: usize, physical: super::Suit) -> super::Suit {
    b14::suit_for_judge(g, seat, physical)
}

/// 某张牌在归属者语义下的有效花色（火攻等「他人读牌」唯一调用点）。
pub fn suit_of_card(g: &Game, card: &str) -> Option<super::Suit> {
    b14::suit_of_card(g, card)
}

/// 实例级「不能成为目标」（帷幕：黑色锦囊）。
pub fn deny_target_instance(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&str>,
    def: &super::cards::CardDef,
) -> bool {
    b15::deny_target_instance(g, user, target, card, def)
}

/// 群体牌目标排除（巨象）。
pub fn aoe_excluded(g: &Game, user: usize, target: usize, def_code: &str) -> bool {
    b15::aoe_excluded(g, user, target, def_code)
}

/// 结算结束后钩子（巨象：获得他人使用的【南蛮】）。
pub fn on_card_finished(g: &mut Game, card: &str, user: usize) {
    b15::on_card_finished(g, card, user);
}

/// 回合内全防伤（帷幕）：返回被防止的点数；`flow::damage_one` 唯一调用点。
pub fn try_full_prevent(g: &mut Game, target: usize, amount: i32) -> Option<i32> {
    b15::try_full_prevent(g, target, amount)
}

/// 濒死求桃限制（完杀）。
pub fn dying_allow_tao(g: &Game, asker: usize, target: usize) -> bool {
    b15::dying_allow_tao(g, asker, target)
}

/// 天义没赢 → 本回合不能使用【杀】。
pub fn sha_use_blocked(g: &Game, seat: usize) -> bool {
    b15::sha_use_blocked(g, seat)
}

/// 濒死窗口的技能选项（涅槃）。
pub fn dying_skill_options(g: &Game, asker: usize, target: usize) -> Vec<(&'static str, String)> {
    b14::dying_options(g, asker, target)
}

/// 手牌上限修正（`Hook::HandLimit`，唯一调用点 `Game::hand_limit`）。
pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    let mut total = b2::hand_limit_bonus(g, seat)
        + b3::hand_limit_bonus(g, seat)
        + b5::hand_limit_bonus(g, seat)
        + b7::hand_limit_bonus(g, seat)
        + b8::hand_limit_bonus(g, seat)
        + b11::hand_limit_bonus(g, seat)
        + b13::hand_limit_bonus(g, seat);
    for skill in all() {
        if !active(g, seat, skill.code) {
            continue;
        }
        for face in skill.faces {
            for h in face.hooks {
                if let Hook::HandLimit(n) = h {
                    total += *n;
                }
            }
        }
    }
    total
}

/// 距离修正（`Hook::Distance`，唯一调用点 `gear::distance`）。
/// 动态距离修正（千驹：每损失 1 点体力 -1）。
pub fn distance_bonus(g: &Game, seat: usize) -> i32 {
    b5::distance_bonus(g, seat) + b14::distance_bonus(g, seat) + b15::distance_bonus(g, seat)
}

/// 「他人 → 该座位」距离修正（义从：体力 ≤2 时 +1；`gear::distance` 唯一调用点）。
pub fn distance_to_bonus(g: &Game, seat: usize) -> i32 {
    b12::distance_to_bonus(g, seat)
}

pub fn distance_modifier(g: &Game, seat: usize) -> i32 {
    let mut total = 0;
    for skill in all() {
        if !active(g, seat, skill.code) {
            continue;
        }
        for face in skill.faces {
            for h in face.hooks {
                if let Hook::Distance(n) = h {
                    total += *n;
                }
            }
        }
    }
    total
}

/// 【杀】次数是否无限（`Hook::ShaLimit`，连弩/喀嗇）。`card_play::sha_used_limit` 唯一调用点。
pub fn sha_unlimited(g: &Game, seat: usize) -> bool {
    if b2::sha_unlimited(g, seat) {
        return true;
    }
    all().into_iter().any(|skill| {
        active(g, seat, skill.code)
            && skill
                .faces
                .iter()
                .any(|f| f.hooks.iter().any(|h| *h == Hook::ShaLimit))
    })
}

/// 响应张数（无双：`opponent` 有锁定技 → 需连续 2 张；`card_play` 唯一调用点）。
pub fn response_need(
    g: &Game,
    responder: usize,
    opponent: usize,
    kind: super::cards::ResponseKind,
) -> u32 {
    // 无双（覆盖为 2）与肉林（+1，与无双相加 → 3）分别为「下限/加成」。
    let base = b12::response_need(g, responder, opponent, kind).max(1);
    if kind != super::cards::ResponseKind::Shan {
        return base;
    }
    base + b15::response_bonus(g, responder, opponent)
}

/// 酒池：【酒】无次数限制（`card_play::jiu_limit_reached` 唯一调用点）。
pub fn jiu_unlimited(g: &Game, seat: usize) -> bool {
    b15::jiu_unlimited(g, seat)
}

/// 濒死救援的技能转化候选（急救：回合外红色手牌/装备牌当【桃】）。
pub fn dying_alternatives(g: &Game, asker: usize) -> Vec<super::CardId> {
    b12::jijiu_options(g, asker)
}

/// 濒死救援的技能转化结算（承载牌落盘；回复由调用方执行）。
pub fn use_dying_alternative(g: &mut Game, asker: usize, id: &str) -> Result<(), String> {
    b12::use_jijiu(g, asker, id)
}

/// 牌对目标是否**无效**（掣政①）。`card_play` 唯一调用点。
pub fn card_ineffective(g: &Game, user: usize, target: usize) -> bool {
    b1::card_ineffective(g, user, target)
}

/// 【杀】次数加成（`ShaLimit` 之外：咆哮/成略等 +N）。`card_play::sha_limit` 唯一调用点。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    b1::sha_extra(g, seat)
        + b2::sha_extra(g, seat)
        + b5::sha_extra(g, seat)
        + b10::sha_extra(g, seat)
        + b14::sha_extra(g, seat)
        + b15::sha_extra(g, seat)
}

/// 不能成为目标（掣政②/空城/帷幕…）。`card_play::legal_targets` 唯一调用点。
pub fn deny_target(g: &Game, user: usize, target: usize) -> bool {
    b1::deny_target(g, user, target) || b8::deny_target(g, user, target)
}

/// 距离/范围是否被技能覆盖（决堰·坐骑 / 雄乱）：本次使用跳过距离校验。
/// 造成伤害时的数值加成（暗箭：不在其攻击范围内 +1）。
pub fn damage_bonus(g: &Game, spec: &super::DamageSpec) -> i32 {
    b4::damage_bonus(g, spec) + b11::damage_bonus(g, spec) + b13::damage_bonus(g, spec)
}

/// 他人侧主动技（陷嗣：其他角色移去两张「逆」出【杀】）。
pub fn other_side_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    let mut v = b4::other_side_actions(g, seat);
    v.extend(b8::other_side_actions(g, seat));
    v.extend(b13::other_side_actions(g, seat));
    v.extend(b14::other_side_actions(g, seat));
    v
}

/// 发动他人侧技能（唯一入口：回合帧的 `skill:<code>` 在自有技能不可用时回退到这里）。
pub fn start_other(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if b14::other_side_actions(g, seat).contains(&code) {
        return b14::start_other(g, seat, code);
    }
    if b13::other_side_actions(g, seat).contains(&code) {
        return b13::start_other(g, seat, code);
    }
    if b8::other_side_actions(g, seat).contains(&code) {
        return b8::start_other(g, seat, code);
    }
    b4::start_other(g, seat, code)
}

pub fn ignore_distance(g: &Game, seat: usize) -> bool {
    b2::ignore_distance(g, seat)
}

/// 「对方不能响应」（雄乱）：本次使用对该目标整段跳过响应。
pub fn no_response_for(g: &Game, user: usize, target: usize) -> bool {
    b2::no_response_for(g, user, target)
}

/// 目标拦截（谦节：延时锦囊 / 铁索连环）：按牌定义判定，legal 与 execute 同源。
pub fn deny_target_card(g: &Game, user: usize, target: usize, code: &str) -> bool {
    b2::deny_target_card(g, user, target, code)
        || b8::deny_target_card(g, user, target, code)
        || b9::deny_target_card(g, user, target, code)
}

/// 拼点目标拦截（谦节）。
pub fn deny_pindian(g: &Game, target: usize) -> bool {
    b2::deny_pindian(g, target)
}

/// 「此牌对目标无效」（父荫）：按牌逐目标判定。
pub fn card_ineffective_for(g: &mut Game, user: usize, target: usize, card: &str) -> bool {
    b2::card_ineffective_for(g, user, target, card) || b8::card_ineffective_for(g, user, target, card)
}

/// 造成伤害前询问（`Hook::DamageBefore`）：把技能帧压到伤害之前，由 handler 决定防伤或继续。
///
/// 返回 `true` = 已压询问帧（伤害延后到 handler 里 `flow::damage_apply`）。
pub fn ask_damage_before(
    g: &mut Game,
    spec: super::DamageSpec,
    _armor: super::ArmorKind,
    _bonus: i32,
) -> bool {
    let Some(from) = spec.from else {
        return false;
    };
    if !b3::should_ask_damage_before(g, from, spec.target) {
        return false;
    }
    let mut codes: Vec<&'static str> = Vec::new();
    for s in all() {
        if !enabled(s.code) || !active(g, from, s.code) {
            continue;
        }
        if s.faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| *h == Hook::DamageBefore))
        {
            codes.push(s.code);
        }
    }
    if codes.is_empty() {
        return false;
    }
    let kind = match spec.kind {
        super::DamageKind::Fire => "fire",
        super::DamageKind::Thunder => "thunder",
        _ => "normal",
    };
    let source = match spec.source {
        super::SourceKind::Slash => "slash",
        super::SourceKind::Nanman => "nanman",
        super::SourceKind::Wanjian => "wanjian",
        super::SourceKind::Duel => "duel",
        super::SourceKind::Skill => "skill",
        _ => "other",
    };
    for code in codes {
        super::resolve::push_ask(
            g,
            code,
            Stage::Choose,
            from,
            json!({
                "step": "ask",
                "target": spec.target,
                "amount": spec.amount,
                "kind": kind,
                "source": source,
            }),
        );
    }
    true
}

/// 受害者侧「造成伤害时」询问（骄矜：弃装备牌令伤害 -1）。
pub fn ask_damage_before_victim(
    g: &mut Game,
    spec: super::DamageSpec,
    _armor: super::ArmorKind,
    _bonus: i32,
) -> bool {
    let Some(from) = spec.from else {
        return false;
    };
    if !b5::should_ask_damage_before_victim(g, from, spec.target)
        && !b7::should_ask_damage_before_victim(g, from, spec.target)
        && !b14::should_ask_damage_before_victim(g, from, spec.target)
    {
        return false;
    }
    let mut codes: Vec<&'static str> = Vec::new();
    for s in all() {
        if !enabled(s.code) || !active(g, spec.target, s.code) {
            continue;
        }
        if s.faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| *h == Hook::DamageBefore))
        {
            codes.push(s.code);
        }
    }
    if codes.is_empty() {
        return false;
    }
    for code in codes {
        super::resolve::push_ask(
            g,
            code,
            Stage::Choose,
            spec.target,
            json!({
                "step": "ask",
                "target": spec.target,
                "from": from,
                "amount": spec.amount,
            }),
        );
    }
    true
}

/// 濒死进入时询问（兴衰）：已压询问帧则返回 true（濒死帧由 handler 收口）。
pub fn fire_dying_enter(g: &mut Game, seat: usize) -> bool {
    if !b5::should_fire_dying_enter(g, seat) {
        return false;
    }
    let mut pushed = false;
    for s in all() {
        if !enabled(s.code) || !active(g, seat, s.code) {
            continue;
        }
        if s.faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| *h == Hook::DyingEnter))
        {
            super::resolve::push_ask(
                g,
                s.code,
                Stage::Choose,
                seat,
                json!({ "step": "ask", "target": seat, "cursor": 0, "helped": 0 }),
            );
            pushed = true;
        }
    }
    pushed
}

/// 角色死亡后的技能钩子（宗祚：某势力最后一名角色死亡 → 减体力上限）。
///
/// 唯一调用点：`turn::finish_death`（死亡清理开始前）。handler 只允许同步结算（不得产生 pending）。
pub fn on_death(g: &mut Game, dead: usize) {
    b7::on_death(g, dead);
    b8::on_death(g, dead);
}

/// 回合开始钩子（绝响：「直到其下回合开始」的花色保护到期）。
/// 唯一调用点：`turn::begin_turn`。
pub fn on_turn_start(g: &mut Game, seat: usize) {
    b8::on_turn_start(g, seat);
}

/// 使用牌起步时的同步声明（福绵：红色牌可多指定 N 个目标 / 无距离）。
/// 唯一调用点：`card_play::use_start` / `push_virtual_use`（**在目标选择之前**）。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    b8::on_use_start(g, actor, card);
    b9::on_use_start(g, actor, card);
    b10::on_use_start(g, actor, card);
    b13::on_use_start(g, actor, card);
    b15::on_use_start(g, actor, card);
}

/// 克己：本回合出牌阶段未使用【杀】→ 跳过弃牌阶段（`turn::step_turn` 唯一调用点）。
pub fn skip_discard(g: &Game, seat: usize) -> bool {
    b10::skip_discard(g, seat)
}

/// 诈降：红色【杀】不能被**真实打出**的【闪】抵消（八卦/技能视为闪仍可）。
pub fn no_real_shan(g: &Game, user: usize, card: &str) -> bool {
    b10::no_real_shan(g, user, card)
}

/// 失去全部手牌后的技能窗口（连营）：`x` = 本次失去的手牌数（部分场景按 1 计，见 docs）。
pub fn on_hand_empty(g: &mut Game, seat: usize, x: usize) {
    b10::on_hand_empty(g, seat, x);
}

/// 装备区失去牌（枭姬）：`flow::leave_equip` 唯一调用点。
pub fn on_equip_lost(g: &mut Game, owner: usize, id: &str) {
    b10::on_equip_lost(g, owner, id);
}

/// 回合结束前的技能窗口（博图额外回合 / 谦逊归还手牌）。
pub fn on_turn_end(g: &mut Game, seat: usize) {
    b10::on_turn_end(g, seat);
}

/// 多目标牌结算前（奋威）：返回是否压入询问帧。
pub fn fire_multi_target(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    b10::fire_multi_target(g, user, targets, card)
}

/// 取走「此牌对哪些目标无效」结果（奋威；读完清空）。
pub fn take_multi_invalid(g: &mut Game, card: &str) -> Option<Vec<usize>> {
    b10::take_multi_invalid(g, card)
}

/// 回复体力前的「救援」询问（返回 true = 已压询问帧，本次回复延后由技能结算）。
pub fn heal_ask(g: &mut Game, target: usize, amount: i32, cause: &str) -> bool {
    b10::heal_ask(g, target, amount, cause)
}

/// 延时锦囊判定生效时（谦逊）：`turn::judge_phase` 唯一调用点。
pub fn on_delayed_judge(g: &mut Game, seat: usize, card: &str) {
    b10::on_delayed_judge(g, seat, card);
}

/// 【杀】造成伤害时的技能加成（义绝红桃杀 / 咆哮抵消后下一杀）。
pub fn sha_damage_bonus(g: &mut Game, actor: usize, target: usize, card: &str) -> i32 {
    b9::sha_damage_bonus(g, actor, target, card)
}

/// 【杀】被抵消后（八卦/技能视为闪也算）的技能回调（咆哮）。
pub fn on_sha_dodged(g: &mut Game, actor: usize) {
    b9::on_sha_dodged(g, actor);
}

/// 代出【杀】的接力完成（激将：代出者回合外可令主公摸一张）。
pub fn on_relay_sha(g: &mut Game, actor: usize, source: usize) {
    b9::on_relay_sha(g, actor, source);
}

/// 锦囊无距离限制（奇才）。
pub fn trick_no_distance(g: &Game, actor: usize) -> bool {
    b9::trick_no_distance(g, actor)
}

/// 防止弃置（奇才：他人弃置你的防具/宝物）。
pub fn prevent_discard(g: &Game, by: usize, owner: usize, id: &str) -> bool {
    b9::prevent_discard(g, by, owner, id)
}

/// 【杀】指定目标之后的技能询问（谏征/专对）：返回是否压入了询问帧。
pub fn fire_sha_targeted(g: &mut Game, user: usize, targets: &[usize], card: &str) -> bool {
    b8::fire_sha_targeted(g, user, targets, card)
}

/// 取走「目标列表重写」结果（读完清空；`None` = 未改写）。
pub fn take_sha_targets(g: &mut Game, card: &str) -> Option<Vec<usize>> {
    b8::take_sha_targets(g, card)
}

/// 拼点：是否提供「用牌堆顶的牌拼点」选项（天辩）。
pub fn pindian_deck_option(g: &Game, seat: usize) -> bool {
    b8::pindian_deck_option(g, seat)
}

/// 拼点：有效点数修正（天辩红桃视为 K）。
pub fn pindian_rank(g: &Game, seat: usize, card: &str) -> u8 {
    b8::pindian_rank(g, seat, card)
}

/// 链祸：火焰伤害的连环传导加成（`flow::damage_apply` 唯一调用点）。
pub fn lianhuo_bonus(g: &Game, spec: &super::DamageSpec) -> i32 {
    b7::lianhuo_bonus(g, spec)
}

/// 撤销技能（觉醒失去技能 / 临时授予到期；只动 owned，不碰 granted 的其它来源）。
pub fn revoke(g: &mut Game, seat: usize, code: &str) {
    if let Some(s) = g.seats.get_mut(seat) {
        s.skills.retain(|c| c != code);
        s.granted.retain(|c| c != code);
    }
}

/// 使用牌之前询问（`Hook::UsePre`）：把技能帧压到牌结算帧之上，回答后牌继续结算。
pub fn fire_use_pre(g: &mut Game, actor: usize, card: &str) {
    let mut codes: Vec<&'static str> = Vec::new();
    for s in all() {
        if !enabled(s.code) || !active(g, actor, s.code) {
            continue;
        }
        if s.faces
            .iter()
            .any(|f| f.hooks.iter().any(|h| *h == Hook::UsePre))
        {
            codes.push(s.code);
        }
    }
    for code in codes {
        super::resolve::push_ask(
            g,
            code,
            Stage::Choose,
            actor,
            json!({ "step": "use_pre", "card": card }),
        );
    }
}

// ---------------------------------------------------------------------------
// 牌面替代（转化技）与响应/使用单入口（武圣/龙胆/矢志…）
// ---------------------------------------------------------------------------

/// 一张牌在某语义下的「生效定义」（`As(code)` / 明确不可用 `Deny` / 无意见 `NoOpinion`）。
///
/// `Deny` 用于**替代型**技能（矢志：体力 1 时【闪】不能再当【闪】）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAs {
    As(&'static str),
    Deny,
    NoOpinion,
}

/// 出牌阶段「将某张手牌当另一张牌使用」的替代定义（矢志/武圣/奇袭/国色…）。
pub fn use_alias(g: &Game, seat: usize, id: &str) -> Option<&'static str> {
    if card_locked(g, seat, id) {
        return None;
    }
    b6::use_alias(g, seat, id)
        .or_else(|| b9::use_alias(g, seat, id))
        .or_else(|| b10::use_alias(g, seat, id))
        .or_else(|| b14::use_alias(g, seat, id))
        .or_else(|| b15::use_alias(g, seat, id))
}

/// 响应语义下的生效定义：先问替代型技能（矢志/武圣/龙胆），再看牌自身标签。
pub fn response_as(
    g: &Game,
    seat: usize,
    id: &str,
    kind: super::cards::ResponseKind,
) -> Option<&'static str> {
    for opinion in [
        b6::response_as(g, seat, id, kind),
        b9::response_as(g, seat, id, kind),
        b11::response_as(g, seat, id, kind),
        b14::response_as(g, seat, id, kind),
    ] {
        match opinion {
            CardAs::As(code) => return Some(code),
            CardAs::Deny => return None,
            CardAs::NoOpinion => {}
        }
    }
    if card_locked(g, seat, id) {
        return None;
    }
    let def = g.card_defs.get(id).and_then(|c| super::cards::def(&c.def))?;
    if def.responds_as(kind) {
        Some(def.code)
    } else {
        None
    }
}

/// 化身可授予技能（mechanics `general-pool` / 2026-11-16 用户裁定 Q1/Q2）：
/// **已注册**（`all()`，不要求已收口）且**排除** 锁定技 / 觉醒技 / 主公技 / 授予技。
///
/// 授予走 `skills::grant` 同一入口（授予技形式），因此不依赖各技能的具体实现。
pub fn huashen_skills_of(general_code: &str) -> Vec<&'static str> {
    let Some(gd) = super::generals::def(general_code) else {
        return vec![];
    };
    let mut out: Vec<&'static str> = Vec::new();
    for name in gd.skills {
        let Some(code) = code_by_name(name) else { continue };
        let Some(d) = def(code) else { continue };
        if d.grant_only || matches!(d.kind, SkillKind::Locked | SkillKind::Awaken | SkillKind::Lord) {
            continue;
        }
        if !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

/// 某座位此刻**可化身**的武将池（Q2 裁定）：
/// `Status::Live` ∩ 非「同场存活武将」∩ 非自己 ∩ 非已持有化身牌 ∩ 至少一个可授予技能。
pub fn huashen_generals(g: &super::Game, seat: usize) -> Vec<String> {
    // 「同场已存在」= 存活座位正在使用的武将（含自己）。
    let on_field: Vec<String> = g
        .seats
        .iter()
        .filter(|s| s.alive && !s.general.is_empty())
        .map(|s| s.general.clone())
        .collect();
    let owned: Vec<String> = g
        .huashen
        .get(&seat)
        .map(|h| {
            let mut v = h.pool.clone();
            if let Some(c) = &h.current {
                v.push(c.clone());
            }
            v
        })
        .unwrap_or_default();
    let mut out: Vec<String> = crate::generals_generated::GENERALS
        .iter()
        .filter(|gd| gd.status == crate::generals_generated::Status::Live)
        .filter(|gd| !on_field.iter().any(|c| c == gd.code))
        .filter(|gd| !owned.iter().any(|c| c == gd.code))
        .filter(|gd| !huashen_skills_of(gd.code).is_empty())
        // Q8（2026-11-16）：该武将的可授予技能若已被同场存活角色全部拥有 → 不可入选。
        .filter(|gd| !huashen_choices(g, seat, gd.code).is_empty())
        .map(|gd| gd.code.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// 展示某个化身牌时**可选**的技能（Q2 裁定）：排除同场存活角色已拥有的技能
/// （`Seat.skills` ∪ `granted`），以及该座位已取得的化身技能。
pub fn huashen_choices(g: &super::Game, seat: usize, general_code: &str) -> Vec<&'static str> {
    let mut taken: Vec<&str> = Vec::new();
    for (i, s) in g.seats.iter().enumerate() {
        if i == seat || !s.alive {
            continue;
        }
        taken.extend(s.skills.iter().map(|c| c.as_str()));
        taken.extend(s.granted.iter().map(|c| c.as_str()));
    }
    if let Some(h) = g.huashen.get(&seat) {
        if let Some(c) = h.active_skill.as_deref() {
            taken.push(c);
        }
    }
    huashen_skills_of(general_code)
        .into_iter()
        .filter(|c| !taken.contains(c))
        .collect()
}

/// 可改判的座位 + 技能 code（mechanics `judge-rewrite`；判定者左手侧起、判定者最后）。
///
/// **按技能声明**（`Hook::JudgeRewrite`）而非写死技能名：新增改判技只需声明钩子 + 提供候选。
pub fn judge_rewrite_holders(g: &Game, judge_seat: usize) -> Vec<(usize, &'static str)> {
    let n = g.seats.len();
    let mut out = Vec::new();
    for k in 1..=n {
        let s = (judge_seat + k) % n;
        if !g.seats.get(s).map(|x| x.alive).unwrap_or(false) {
            continue;
        }
        for skill in all() {
            if !enabled(skill.code) || !active(g, s, skill.code) {
                continue;
            }
            if skill
                .faces
                .iter()
                .any(|f| f.hooks.iter().any(|h| *h == Hook::JudgeRewrite))
            {
                out.push((s, skill.code));
            }
        }
    }
    out
}

/// 某改判技能可用的替代牌（手牌 + 装备区；具体过滤由技能决定）。
pub fn judge_rewrite_cards_for(g: &Game, seat: usize, code: &str) -> Vec<super::CardId> {
    match code {
        "guicai" => b11::judge_rewrite_cards(g, seat),
        "guidao" => b13::guidao_cards(g, seat),
        _ => Vec::new(),
    }
}

/// 判定生效后的统一钩子（雷击：鬼道改判自己的判定后仍可发动判定后效果）。
pub fn on_judge_effective(g: &mut Game, judge_seat: usize) -> bool {
    b13::on_judge_effective(g, judge_seat)
}

/// 天妒：判定牌生效后是否可获得。
pub fn judge_gain_available(g: &Game, seat: usize) -> bool {
    b11::judge_gain_available(g, seat)
}

/// 花色/牌封锁（讥谀：本回合不能使用与之相同花色的牌）——使用/打出/响应统一拦截。
pub fn card_locked(g: &Game, seat: usize, id: &str) -> bool {
    b7::card_locked(g, seat, id)
}

/// 响应替代入口（饰非【闪】/激将【杀】/滔乱三场景）：技能 code + 选项文案。
///
/// `ctx` 是当前响应上下文（`sha`/`duel`/`nanman`/`wanjian`/`collateral`/`wuxie`），
/// 由牌结算流程给出；技能按（kind, ctx）声明自己覆盖哪些场景（滔乱只覆盖
/// 「无懈链/万箭/南蛮」，不覆盖普通【杀】的【闪】）。
pub fn response_alternatives(
    g: &Game,
    seat: usize,
    kind: super::cards::ResponseKind,
    ctx: &str,
) -> Vec<(&'static str, String)> {
    let mut out = b6::response_alternatives(g, seat, kind, ctx);
    out.extend(b9::response_alternatives(g, seat, kind, ctx));
    out.extend(b7::response_alternatives(g, seat, kind, ctx));
    out.extend(b11::response_alternatives(g, seat, kind, ctx));
    out.extend(b13::response_alternatives(g, seat, kind, ctx));
    out
}

/// 「视为在攻击范围内」（诏缚）——`gear::within_attack_range` 的唯一技能来源。
pub fn in_attack_range(g: &Game, from: usize, to: usize) -> bool {
    b6::in_attack_range(g, from, to)
}

/// 决斗伤害后的技能回调（战绝：与受伤者各摸一张）。
pub fn on_duel_damage(g: &mut Game, card: &str, from: Option<usize>, victim: usize) {
    b6::on_duel_damage(g, card, from, victim);
}

/// 门禁自检：注册表不得出现重复 code。
pub fn check_unique_codes() -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for s in all() {
        if !seen.insert(s.code) {
            return Err(format!("技能 code 重复注册：{}", s.code));
        }
    }
    Ok(())
}

/// 门禁自检：转换技必须**每个 face 各自绑定窗口**（§9.1 拆 face 的通用要求）。
pub fn check_transform_faces() -> Result<(), String> {
    for s in all() {
        if s.transform && s.faces.len() < 2 {
            return Err(format!("转换技 {} 必须拆 ≥2 face", s.code));
        }
    }
    Ok(())
}

/// 阶段名（事件快照用；避免各 handler 各自 `format!("{:?}")`）。
pub fn phase_name(p: Phase) -> String {
    format!("{p:?}")
}
