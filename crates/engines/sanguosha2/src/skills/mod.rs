//! 技能注册表：**声明 + handler**，禁止在核心流程写武将名分支。
//!
//! 单一状态源：`active()`（自带 ∪ 授予）。旧引擎有 6 种「我现在有这个技能吗」的
//! 表达（has / flags / turn_counters / marks / mark_counts / GrantedSkill），钩子只查
//! 其中一种 → 「两处各写一半」。这里只允许 `active()`。

pub mod b0;
pub mod b1;
pub mod b1b;
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
pub mod mingren;
pub mod kongsheng;
pub mod liangyin;
pub mod zhenliang;

use crate::kernel::event::{EventKind, Sub, Window};
use crate::kernel::pending::PendingBuilder;
use crate::kernel::resolve::{self, Ask, Stage, Step};
use crate::kernel::{Game, Phase};

pub struct SkillDef {
    pub code: &'static str,
    pub name: &'static str,
    pub triggers: &'static [EventKind],
    pub window: Window,
    pub priority: i32,
    /// 主动技的可发动判定（与 `execute_active` **同源**）。
    pub can_active: Option<fn(&Game, usize) -> bool>,
    pub handler: fn(&mut Game, &Ask) -> Result<Step, String>,
    /// 濒死锁定技钩子（返回 true = 已处理，濒死流程重新检查体力）。
    pub dying_hook: Option<fn(&mut Game, usize) -> bool>,
    /// 翻至正面钩子（解围②）。
    pub flip_hook: Option<fn(&mut Game, usize)>,
    /// 无懈窗口可提供的额外选项（解围：装备当无懈）。
    pub wuxie_opts: Option<fn(&Game, usize) -> Vec<(String, String)>>,
    /// 无懈窗口选项的执行（返回 true = 已处理该选项）。
    pub wuxie_use: Option<fn(&mut Game, usize, &str) -> Result<bool, String>>,
    /// 「不能成为目标」拦截（谦节等）：返回 true = 该角色不能成为此牌目标。
    pub target_blocker: Option<fn(&Game, usize, usize, &str) -> bool>,
    /// 转换技：阳面窗口（Some 表示这是一面转换技）。
    pub yang_window: Option<Window>,
    /// 转换技：阴面窗口。
    pub yin_window: Option<Window>,
}

pub const SKILLS: &[SkillDef] = &[
    b1::YONGSI,
    b1::QIZHI,
    b1::JINZOU,
    b1::HUAJU,
    b1::CUNMU,
    b1b::WEIDI,
    b1b::JUZHAN,
    b1b::FEIJUN,
    b1b::BINGLUE,
    b1b::YILI,
    b1b::ZHENGLUN,
    b1b::KUIZHU,
    b1b::CHEZHENG,
    b1b::LIJUN,
    b1b::JIANXIANG,
    b1b::SHENSHI,
    b1b::CHENGLUE,
    b1b::SHICAI,
    b2::ZHENGRONG,
    b2::CONGJIAN,
    b2::WANGLIE,
    b2::FUYIN,
    b2::JUEYAN,
    b2::POSHI,
    b2::XIONGLUAN,
    b2::QIANJIE,
    b2::ZHENGU,
    b2::ZUILUN,
    b2::HONGJU,
    mingren::DEF,
    b0::JUSHOU,
    b0::JIEWEI,
    b0::BUQU,
    b0::FENJI,
    b0::XINGSHANG,
    b0::FANGZHU,
    b0::SONGWEI,
    liangyin::DEF,
    kongsheng::DEF,
    zhenliang::DEF,
    // B3（马谡 / 荀攸 / 钟会 / 满宠 / 关平）
    b3::SANYAO,
    b3::ZHIMAN,
    b3::QICE,
    b3::ZHIYU,
    b3::QUANJI,
    b3::ZILI,
    b3::JUNXING,
    b3::YUCE,
    b3::LONGYIN,
    // B4（刘封 / 潘璋马忠 / 朱然 / 曹真 / 韩浩史涣 / 陈群 / 吴懿 / 周仓）
    b4::XIANSI,
    b4::DUODAO,
    b4::ANJIAN,
    b4::DANSHOU,
    b4::SIDI,
    b4::SHENDUAN,
    b4::YONGLUE,
    b4::PINDI,
    b4::FAEN,
    b4::BENXI,
    b4::ZHONGYONG,
    // B5（孙鲁班 / 朱桓 / 顾雍 / 曹叡 / 曹休 / 钟繇）
    b5::ZANHUI,
    b5::JIAOJIN,
    b5::FENLI,
    b5::PINGKOU,
    b5::SHENXING,
    b5::BINGYI,
    b5::HUITUO,
    b5::MINGJIAN,
    b5::XINGSHUAI,
    b5::QIANJU,
    b5::QINGXI,
    b5::HUOMO,
    b5::ZUODING,
    // B6（刘谌 / 张嶷 / 孙休 / 朱治 / 全琮 / 公孙渊 / 郭图逢纪）
    b6::ZHANJUE,
    b6::QINWANG,
    b6::WURONG,
    b6::SHIZHI,
    b6::YANZHU,
    b6::XINGXUE,
    b6::ZHAOFU,
    b6::ANGUO,
    b6::YAOMING,
    b6::HUAIYI,
    b6::JIGONG,
    b6::SHIFEI,
    // B7（李严 / 孙登 / 刘虞 / 岑昏 / 孙资刘放 / 黄皓 / 张让）
    b7::DULIANG,
    b7::FULIN,
    b7::KUANGBI,
    b7::ZHIGE,
    b7::ZONGZUO,
    b7::JISHE,
    b7::LIANHUO,
    b7::GUIZAO,
    b7::JIYU,
    b7::QINQING,
    b7::HUISHENG,
    b7::TAOLUAN,
    // B8（辛宪英 / 吴苋 / 徐氏 / 曹节 / 嵇康 / 秦宓 / 薛综 / 蔡邕）+ 绝响授予的四条子技能
    b8::ZHONGJIAN,
    b8::CAISHI,
    b8::FUMIAN,
    b8::DAIYAN,
    b8::WENGUA,
    b8::FUZHU,
    b8::SHOUXI,
    b8::HUIMIN,
    b8::QINGXIAN,
    b8::JUEXIANG,
    b8::JIANZHENG,
    b8::ZHUANDUI,
    b8::TIANBIAN,
    b8::FUNAN,
    b8::JIEXUN,
    b8::BIZHUAN,
    b8::TONGBO,
    b8::JIXIAN,
    b8::ROUXIAN,
    b8::LIEXIAN,
    b8::HEXIAN,
    // B9（界刘备 / 界关羽 / 界张飞 / 界赵云 / 界马超 / 界诸葛亮 / 界黄月英）
    b9::RENDE,
    b9::JIJIANG,
    b9::YIJUE,
    b9::WUSHENG,
    b9::TISHEN,
    b9::PAOXIAO,
    b9::YAJIAO,
    b9::LONGDAN,
    b9::MASHU,
    b9::TIEQI,
    b9::KONGCHENG,
    b9::GUANXING,
    b9::JIZHI,
    b9::QICAI,
    // B10（界甘宁 / 界吕蒙 / 界黄盖 / 界周瑜 / 界大乔 / 界陆逊 / 界孙权 / 界孙尚香）
    b10::QIXI,
    b10::FENWEI,
    b10::KEJI,
    b10::QINXUE,
    b10::GONGXIN,
    b10::BOTU,
    b10::KUROU,
    b10::ZHAXIANG,
    b10::YINGZI,
    b10::FANJIAN,
    b10::LIULI,
    b10::GUOSE,
    b10::QIANXUN,
    b10::LIANYING,
    b10::ZHIHENG,
    b10::JIUYUAN,
    b10::XIAOJI,
    b10::JIEYIN,
    // B11
    b11::JIANXIONG,
    b11::HUJIA,
    b11::FANKUI,
    b11::GUICAI,
    b11::GANGLIE,
    b11::QINGJIAN,
    b11::TUXI,
    b11::LUOYI,
    b11::TIANDU,
    b11::YIJI,
    b11::XUNXUN,
    b11::WANGXI,
    b11::QINGGUO,
    b11::LUOSHEN,
    // B12（界华佗 / 界吕布 / 界公孙瓒 / 界貂蝉）
    b12::JIJIU,
    b12::CHULI,
    b12::WUSHUANG,
    b12::LIYU,
    b12::YICONG,
    b12::QIAOMENG,
    b12::LIJIAN,
    b12::BIYUE,
    // B13（界黄忠 / 界徐庶 / 界华雄 / 界张角 / 界左慈 / 界于吉 / 界袁绍 / 界刘禅）
    b13::LIEGONG,
    b13::ZHUHAI,
    b13::QIANXIN,
    b13::JIANYAN,
    b13::YAOWU,
    b13::SHIZHAN,
    b13::LEIJI,
    b13::GUIDAO,
    b13::HUANGTIAN,
    b13::HUASHEN,
    b13::XINSHENG,
    b13::GUHUO,
    b13::LUANJI,
    b13::XUEYI,
    b13::XIANGLE,
    b13::FANGQUAN,
    b13::RUOYU,
    b13::SISHU,
];

/// 转换技（阳/阴两面）：视图按此清单投影 `transforms`（与 handlers 的 `g.yang/toggle_yang` 对应）。
pub const TRANSFORM_SKILLS: &[&str] = &["zhenliang", "juzhan", "shenshi", "chenglue"];

/// 按技能名查注册 code（化身：武将在 heroes.json 里只有技能名）。
pub fn code_by_name(name: &str) -> Option<&'static str> {
    SKILLS.iter().find(|d| d.name == name).map(|d| d.code)
}

pub fn def(code: &str) -> Option<&'static SkillDef> {
    SKILLS.iter().find(|s| s.code == code)
}

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    def(code).map(|d| d.handler)
}

/// **唯一**技能状态查询入口。
pub fn active(g: &Game, seat: usize, code: &str) -> bool {
    // 非锁定技失效：本回合该角色的非锁定技视为不存在（义绝·黑 / 铁骑）。
    if nonlocked_lost(g, seat) && !LOCKED_SKILLS.contains(&code) {
        return false;
    }
    let Some(s) = g.seats.get(seat) else { return false };
    if s.skills.iter().any(|x| x == code) || s.granted.iter().any(|x| x == code) {
        return true;
    }
    // 伪帝：视为拥有当前主公的主公技（仅注册表内已知的主公技；不可主动使用）。
    let has_weidi = s.skills.iter().any(|x| x == "weidi") || s.granted.iter().any(|x| x == "weidi");
    if has_weidi && is_lord_skill(code) {
        if let Some(lord) = g.seats.iter().position(|x| x.flags.contains("lord")) {
            return lord != seat && g.seats[lord].skills.iter().any(|x| x == code);
        }
    }
    false
}

/// 明确的**锁定技**（义绝/铁骑的「非锁定技失效」不作用于这些）。
pub const LOCKED_SKILLS: &[&str] = &[
    "jueyan", "jishe", "lianhuo", "zongzuo", "shushen", "weimu", "wushuang", "tieqi_locked",
    "kongcheng", "mashu", "paoxiao", "qicai", "fulin", "yingzi", "zhaxiang", "yicong",
    "yaowu", "liegong", "xiangle",
];

/// 「本回合非锁定技失效」（义绝·黑 / 铁骑）。
pub fn nonlocked_lost(g: &Game, seat: usize) -> bool {
    g.counter(&format!("turn.nonlocked_lost:{seat}")) > 0
}

/// 注册表里已知的主公技（伪帝借用范围）。
fn is_lord_skill(code: &str) -> bool {
    matches!(code, "songwei" | "lijun")
}

/// 由注册表派生订阅（禁止手写「技能清单」）。
pub fn rebuild_subs(g: &mut Game) {
    let mut subs = Vec::new();
    let mut seq = 0usize;
    for seat in 0..g.seats.len() {
        for d in SKILLS {
            if !active(g, seat, d.code) {
                continue;
            }
            for kind in d.triggers {
                subs.push(Sub {
                    owner: seat,
                    skill: d.code.to_string(),
                    kind: *kind,
                    window: d.window,
                    priority: d.priority,
                    seq,
                    side: 0,
                });
                seq += 1;
                if let (Some(yw), Some(_)) = (d.yang_window, d.yin_window) {
                    subs.push(Sub {
                        owner: seat,
                        skill: d.code.to_string(),
                        kind: *kind,
                        window: yw,
                        priority: d.priority,
                        seq,
                        side: 1,
                    });
                    seq += 1;
                }
            }
        }
    }
    g.subs = subs;
}

// ---------------------------------------------------------------------------
// legal / execute 同源
// ---------------------------------------------------------------------------

pub fn legal_actions(g: &Game, seat: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in SKILLS {
        if let Some(can) = d.can_active {
            if can(g, seat) {
                out.push((format!("skill:{}", d.code), format!("发动【{}】", d.name)));
            }
        }
    }
    out.push(("end_play".to_string(), "结束出牌阶段".to_string()));
    out
}

/// 执行主动技：**再次**调用同一个 `can_active`（legal 与 execute 不会漂移）。
pub fn execute_active(g: &mut Game, seat: usize, action: &str) -> Result<String, String> {
    let code = action
        .strip_prefix("skill:")
        .ok_or_else(|| format!("未知动作：{action}"))?;
    let d = def(code).ok_or_else(|| format!("未注册技能：{code}"))?;
    let can = d.can_active.ok_or_else(|| format!("【{}】不是主动技", d.name))?;
    if !can(g, seat) {
        return Err(format!("【{}】当前不可发动", d.name));
    }
    match code {
        "zhenliang" => resolve::push_ask(g, "zhenliang", Stage::YangTarget, seat, serde_json::json!({})),
        "chenglue" => resolve::push_ask(g, "chenglue", Stage::Choose, seat, serde_json::json!({})),
        "feijun" => resolve::push_ask(g, "feijun", Stage::Choose, seat, serde_json::json!({ "step": "opt" })),
        "shenshi" => resolve::push_ask(g, "shenshi", Stage::YangTarget, seat, serde_json::json!({})),
        "zhengu" => resolve::push_ask(g, "zhengu", Stage::Choose, seat, serde_json::json!({})),
        "xiongluan" => resolve::push_ask(g, "xiongluan", Stage::Choose, seat, serde_json::json!({})),
        "jueyan" => resolve::push_ask(g, "jueyan", Stage::Choose, seat, serde_json::json!({})),
        // 陷嗣（刘封）的主动部分是**其他角色**「移去两张『逆』」，用 mode 区分触发/主动两条入口。
        "xiansi" => {
            resolve::push_ask(g, "xiansi", Stage::Choose, seat, serde_json::json!({ "mode": "use" }))
        }
        // 其余主动技（sanyao / qice / junxing …）统一从 Choose 开始，由 handler 分流阶段。
        code => resolve::push_ask(g, code, Stage::Choose, seat, serde_json::json!({})),
    }
    resolve::advance(g)?;
    Ok(g.presentation.last().map(|p| p.text()).unwrap_or_default())
}

/// 目标候选：存活且非自己。
pub fn other_alive(g: &Game, seat: usize) -> Vec<usize> {
    (0..g.seats.len())
        .filter(|i| *i != seat && g.seats[*i].hp > 0)
        .collect()
}

/// 构造 pending 的统一入口（保证出口存在，见 `PendingBuilder` 类型状态）。
pub fn builder(actor: usize, kind: &'static str) -> PendingBuilder<crate::kernel::pending::NoEscape> {
    PendingBuilder::new(actor, kind)
}

pub fn phase_is(g: &Game, seat: usize, phase: Phase) -> bool {
    g.turn == seat && g.phase == phase
}

// ---------------------------------------------------------------------------
// 内核 → 技能 的钩子（内核不认识技能名）
// ---------------------------------------------------------------------------

/// 濒死锁定技：按注册顺序问第一个能处理的（不屈）。
pub fn dying_hooks(g: &mut Game, seat: usize) -> bool {
    for d in SKILLS {
        if !active(g, seat, d.code) {
            continue;
        }
        if let Some(h) = d.dying_hook {
            if h(g, seat) {
                return true;
            }
        }
    }
    false
}

/// 翻至正面：调用所有拥有者的 flip 钩子（解围②）+ 法恩（任意角色翻至正面 → 陈群可令其摸 1）。
pub fn on_flip_to_face_up(g: &mut Game, seat: usize) {
    for d in SKILLS {
        if !active(g, seat, d.code) {
            continue;
        }
        if let Some(h) = d.flip_hook {
            h(g, seat);
        }
    }
    ask_faen(g, seat);
}

/// 法恩：受益者（翻至正面 / 进入连环的角色）→ 拥有者可令其摸 1（受益者可以是别人）。
pub fn ask_faen(g: &mut Game, beneficiary: usize) {
    if g.seats.get(beneficiary).map(|s| s.hp <= 0).unwrap_or(true) {
        return;
    }
    let owners: Vec<usize> = (0..g.seats.len())
        .filter(|i| active(g, *i, "faen"))
        .collect();
    for owner in owners {
        resolve::push_ask(
            g,
            "faen",
            Stage::Choose,
            owner,
            serde_json::json!({ "beneficiary": beneficiary }),
        );
    }
}

/// 无懈窗口的额外选项（解围：装备区牌当【无懈可击】）。
pub fn wuxie_use(g: &mut Game, seat: usize, action: &str) -> Result<bool, String> {
    for d in SKILLS {
        if !active(g, seat, d.code) {
            continue;
        }
        if let Some(f) = d.wuxie_use {
            if f(g, seat, action)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 目标合法性：该角色能否成为此牌的目标（谦节等 target_blocker）。
pub fn can_target(g: &Game, source: usize, card: &str, target: usize) -> bool {
    for d in SKILLS {
        if !active(g, target, d.code) {
            continue;
        }
        if let Some(f) = d.target_blocker {
            if f(g, target, source, card) {
                return false;
            }
        }
    }
    true
}

/// 拼点目标拦截（谦节）。
pub fn blocks_pindian(g: &Game, seat: usize) -> bool {
    active(g, seat, "qianjie")
}
pub fn wuxie_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in SKILLS {
        if !active(g, seat, d.code) {
            continue;
        }
        if let Some(f) = d.wuxie_opts {
            out.extend(f(g, seat));
        }
    }
    out
}

/// 统一伤害出口（转调内核流程）。
pub fn damage(g: &mut Game, source: usize, victim: usize, amount: i32) {
    crate::kernel::flow::damage(g, Some(source), victim, amount);
}

// ---------------------------------------------------------------------------
// 内核 → 技能 的数值/开关钩子（内核不认识技能名；技能名只出现在本模块）
// ---------------------------------------------------------------------------

/// 存活角色的不同势力数（庸肆的 X）。
pub fn kingdom_count(g: &Game) -> usize {
    let mut set = std::collections::BTreeSet::new();
    for s in &g.seats {
        if s.hp > 0 {
            set.insert(s.kingdom.clone());
        }
    }
    set.len()
}

/// 「橘」数量（怀橘）。
pub fn ju(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get(&"ju".to_string()).copied())
        .unwrap_or(0)
}

pub fn add_ju(g: &mut Game, seat: usize, n: u32) {
    *g.seats[seat].counters.entry("ju".to_string()).or_insert(0) += n;
}

pub fn remove_ju(g: &mut Game, seat: usize, n: u32) -> bool {
    let c = g.seats[seat].counters.entry("ju".to_string()).or_insert(0);
    if *c < n {
        return false;
    }
    *c -= n;
    true
}

/// 摸牌阶段加成（庸肆 + 怀橘）。
pub fn draw_bonus(g: &Game, seat: usize) -> usize {
    let mut n = 0;
    if active(g, seat, "yongsi") {
        n += kingdom_count(g);
    }
    if active(g, seat, "huaiju") && ju(g, seat) > 0 {
        n += 1;
    }
    // 英姿（界周瑜，锁定）：摸牌阶段多摸 1 张。
    if active(g, seat, "yingzi") {
        n += 1;
    }
    // 福绵①（吴苋）：准备阶段选择后，本回合摸牌阶段多摸 N 张。
    if active(g, seat, "fumian") {
        n += b8::fumian_draw_bonus(g);
    }
    n
}

/// 是否从牌堆底摸（寸目）。
pub fn draw_from_bottom(g: &Game, seat: usize) -> bool {
    active(g, seat, "cunmu")
}

/// 弃牌阶段额外弃牌数（庸肆）。
pub fn discard_extra(g: &Game, seat: usize) -> usize {
    if active(g, seat, "yongsi") {
        kingdom_count(g)
    } else {
        0
    }
}

/// 伤害防止（怀橘）：返回 true = 本次伤害被防止。
pub fn damage_prevent(g: &mut Game, victim: usize) -> bool {
    if active(g, victim, "huaiju") && ju(g, victim) > 0 {
        remove_ju(g, victim, 1);
        g.presentation.push(
            crate::presentation::Presentation::new(
                "huaiju",
                crate::presentation::POutcome::Activated { skill: "怀橘".to_string() },
            )
            .target(victim)
            .fx("huaiju_prevent"),
        );
        g.push_log(format!("怀橘：移除 1 枚「橘」防止伤害（剩余 {}）", ju(g, victim)));
        return true;
    }
    false
}

/// 「造成伤害时」钩子链：返回 true = 已压帧并拦下本次伤害（由技能决定是否补结算）。
///
/// 顺序：制蛮（来源侧）→ 骄矜（受害者侧）→ 倾袭（来源侧）；各钩子内部用 `hook_suppressed`
/// 自抑制，保证「改完数值后仍继续走剩余钩子」而不递归。
pub fn damage_before(
    g: &mut Game,
    source: usize,
    victim: usize,
    amount: i32,
    card: Option<&crate::kernel::CardId>,
) -> bool {
    if b3::zhiman_before(g, source, victim, amount) {
        return true;
    }
    if b5::jiaojin_before(g, Some(source), victim, amount, card) {
        return true;
    }
    if b7::huisheng_before(g, Some(source), victim, amount, card) {
        return true;
    }
    b5::qingxi_before(g, Some(source), victim, amount, card)
}

/// 带伤害属性/传导起点标记的「造成伤害时」钩子链（链祸等按属性判定）。
pub fn damage_before_typed(
    g: &mut Game,
    source: usize,
    victim: usize,
    amount: i32,
    card: Option<&crate::kernel::CardId>,
    dtype: crate::cards::DamageKind,
    chain_origin: bool,
) -> bool {
    if damage_before(g, source, victim, amount, card) {
        return true;
    }
    // 链祸是数值加成（无询问），不需要拦下伤害。
    let _ = (dtype, chain_origin);
    false
}

/// 伤害数值加成（带属性/传导起点）：暗箭 + 链祸。
pub fn damage_bonus_typed(
    g: &Game,
    source: usize,
    victim: usize,
    card: Option<&crate::kernel::CardId>,
    dtype: crate::cards::DamageKind,
    chain_origin: bool,
) -> i32 {
    let b9_extra = b9::paoxiao_bonus(g, source, card)
        + b9::yijue_heart_bonus(g, source, victim, card)
        + b13::liegong_bonus(g, source, victim, card);
    if b9_extra > 0 {
        return b4::anjian_bonus(g, source, victim, card) + b9_extra;
    }
    damage_bonus(g, source, victim, card) + b7::lianhuo_bonus(g, victim, dtype, chain_origin)
}

/// 无来源伤害的属性加成（链祸可被无来源火伤触发）。
pub fn damage_bonus_sourceless(
    g: &Game,
    victim: usize,
    dtype: crate::cards::DamageKind,
    chain_origin: bool,
) -> i32 {
    b7::lianhuo_bonus(g, victim, dtype, chain_origin)
}

/// 无来源伤害的「造成伤害时」钩子（骄矜需要男性来源，通常不触发）。
pub fn damage_before_sourceless(
    g: &mut Game,
    victim: usize,
    amount: i32,
    card: Option<&crate::kernel::CardId>,
) -> bool {
    b5::jiaojin_before(g, None, victim, amount, card)
}

/// 「需要使用/打出【闪】」时的替代选项（饰非；使用与打出共用）。
pub fn dodge_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    b6::shifei_options(g, seat)
}

/// 执行「需要【闪】」的替代选项（返回 true = 已处理）。
pub fn dodge_use(g: &mut Game, seat: usize, action: &str) -> Result<bool, String> {
    b6::shifei_use(g, seat, action)
}

/// 矢志：该牌是否已不能当【闪】使用/打出（替代用法）。
pub fn shan_blocked_by_shizhi(g: &Game, seat: usize, id: &crate::kernel::CardId) -> bool {
    b6::shizhi_blocks_shan(g, seat, id)
}

/// 战绝：该【决斗】造成伤害后的摸牌（由【决斗】结算引擎调用）。
pub fn zhanjue_on_damage(g: &mut Game, owner: usize, victim: usize) {
    b6::zhanjue_on_damage(g, owner, victim);
}

/// 兴衰是否可用（濒死流程用）。
pub fn xingshuai_available(g: &Game, seat: usize) -> bool {
    b5::xingshuai_available(g, seat)
}

/// 濒死流程：用户选择「发动兴衰」后压入逐个询问。
pub fn start_xingshuai(g: &mut Game, victim: usize) {
    b5::start_xingshuai(g, victim);
}

/// 脱离濒死后结算「支援者受到 1 点伤害」。
pub fn settle_after_dying(g: &mut Game, victim: usize) {
    b5::settle_after_dying(g, victim);
}

/// 移出游戏的牌打归属标记（「逆」等：既要按技能取回，又不能被常规选牌命中）。
pub fn tag_void(g: &mut Game, card: &crate::kernel::CardId, seat: usize, code: &str) {
    g.void_tags.insert(card.clone(), (seat, code.to_string()));
}

/// 「逆」数量（陷嗣）。
pub fn ni(g: &Game, seat: usize) -> u32 {
    g.void_tags
        .values()
        .filter(|(s, code)| *s == seat && code == "ni")
        .count() as u32
}

/// 把指定归属的移出游戏牌移去（进弃牌堆），返回实际移去的牌。
pub fn take_void_marked(
    g: &mut Game,
    seat: usize,
    code: &str,
    n: usize,
) -> Result<Vec<crate::kernel::CardId>, String> {
    let cards: Vec<crate::kernel::CardId> = g
        .ids_in(|z| *z == crate::kernel::Zone::Void)
        .into_iter()
        .filter(|id| {
            g.void_tags
                .get(id)
                .is_some_and(|(s, c)| *s == seat && c == code)
        })
        .take(n)
        .collect();
    for id in &cards {
        g.void_tags.remove(id);
        g.to_discard(id, code)?;
    }
    Ok(cards)
}

// ---------------------------------------------------------------------------
// B8 跨批机制钩子（内核只读这些函数，不认识具体技能）
// ---------------------------------------------------------------------------

/// 手牌上限修正（才识 +1 / 忠鉴 -1 / 辟撰「书」数）。
pub fn hand_limit_delta(g: &Game, seat: usize) -> i32 {
    b13::hand_limit_delta(g, seat)
        + b8::hand_limit_delta(g, seat)
        + g.counter("turn.jizhi_hand_limit") as i32
        + g.counter(&format!("turn.qingjian_limit:{seat}")) as i32
        + g.counter(&format!("turn.luoshen_free:{seat}")) as i32
}

/// 「本回合不能对自己使用牌」（才识②）：含装备牌。
pub fn self_use_blocked(g: &Game, seat: usize) -> bool {
    b8::self_use_blocked(g, seat)
}

/// 目标数修正（福绵②）：返回可**额外**指定的目标数（0 = 不修正）。
pub fn extra_targets(g: &Game, seat: usize, card: Option<&crate::kernel::CardId>) -> usize {
    b8::extra_targets(g, seat, card)
}

/// 消费「本回合多指定目标」的一次性效果（福绵②）。
pub fn consume_extra_targets(g: &mut Game, seat: usize) {
    b8::consume_extra_targets(g, seat);
}

/// 响应牌打出后的统一接入点（复难：「其他角色响应你使用的牌」）。
pub fn on_response_played(
    g: &mut Game,
    used: Option<&crate::kernel::CardId>,
    responder: usize,
    response: &crate::kernel::CardId,
) {
    b8::on_response_played(g, used, responder, response);
}

/// 判定改判窗口（鬼才）。
pub fn start_judge_rewrite(g: &mut Game, owner: usize, card: &crate::kernel::CardId) -> bool {
    match judge_rewrite_holders(g, owner).first() {
        Some(seat) => start_rewrite_for(g, owner, *seat, card),
        None => false,
    }
}

/// 天妒：判定生效后询问是否获得判定牌。
pub fn start_tiandu(g: &mut Game, owner: usize, card: &crate::kernel::CardId) -> bool {
    b11::start_tiandu(g, owner, card)
}

/// 拼点选牌的附加选项（天辩：牌堆顶的牌）。
pub fn pindian_options(g: &Game, seat: usize) -> Vec<(String, String)> {
    b8::pindian_options(g, seat)
}

/// 拼点选牌附加选项的执行（返回取到的牌 id）。
pub fn pindian_take(
    g: &mut Game,
    seat: usize,
    option: &str,
) -> Result<Option<crate::kernel::CardId>, String> {
    b8::pindian_take(g, seat, option)
}

/// 拼点牌的有效点数（天辩：红桃视为 K）。
pub fn pindian_rank(g: &Game, seat: usize, card: &crate::kernel::CardId) -> u8 {
    b8::pindian_rank(g, seat, card)
}

/// 「【杀】指定目标后 / 成为【杀】目标后」可介入的技能（按注册顺序）。
pub fn sha_intercept_skills(
    g: &Game,
    user: usize,
    target: usize,
    card: Option<&crate::kernel::CardId>,
) -> Vec<(usize, &'static str)> {
    b8::sha_intercept_skills(g, user, target, card)
}

/// 该介入技能是否「每张牌只问一次」（否则每个目标各问一次）。
pub fn sha_intercept_per_card(code: &str) -> bool {
    b8::sha_intercept_per_card(code)
}

/// 「随机使用一张装备牌」（清弦/四弦）：牌堆顶向下第一张装备，无则遍历弃牌堆。
pub fn take_equip_from_piles(g: &mut Game, cause: &str) -> Option<crate::kernel::CardId> {
    b8::take_equip_from_piles(g, cause)
}

// ---------------------------------------------------------------------------
// B9 钩子（转化牌 / 主公技接力 / 数值开关）
// ---------------------------------------------------------------------------

/// 被动转化技（武圣/龙胆）可用的一次性视图。
pub struct Conversion {
    pub skill: &'static str,
    /// 承载牌。
    pub from: crate::kernel::CardId,
    /// 生效牌名（如 "杀"）。
    pub to: &'static str,
    /// 可主动使用。
    pub usable: bool,
    /// 可作为响应牌打出。
    pub playable: bool,
}

pub fn conversions(g: &Game, seat: usize) -> Vec<Conversion> {
    let mut out = b9::conversions(g, seat);
    out.extend(b10::conversions(g, seat));
    out.extend(b11::conversions(g, seat));
    out.extend(b12::conversions(g, seat));
    out
}

/// 濒死救援可用的转化【桃】（急救）：`legal` 与 `execute` 共用同一份候选。
pub fn dying_conversions(g: &Game, seat: usize) -> Vec<Conversion> {
    conversions(g, seat)
        .into_iter()
        .filter(|c| c.to == "桃" && c.playable)
        .collect()
}

/// 响应窗口的额外选项（主公技接力「激将」）。
pub fn relay_options(g: &Game, seat: usize, kind: crate::cards::ResponseKind) -> Vec<(String, String)> {
    let mut out = b9::relay_options(g, seat, kind);
    out.extend(b11::relay_options(g, seat, kind));
    out
}

/// 是否可以在该响应窗口发起主公技接力（激将：替主公打出【杀】）。
pub fn relay_before(
    g: &mut Game,
    responder: usize,
    kind: crate::cards::ResponseKind,
    used: Option<&crate::kernel::CardId>,
) -> bool {
    b9::relay_before(g, responder, kind, used)
}

/// 发起主公技接力（激将：替主公打出【杀】）。
pub fn start_relay(
    g: &mut Game,
    lord: usize,
    kind: crate::cards::ResponseKind,
    used: Option<&crate::kernel::CardId>,
) {
    if kind == crate::cards::ResponseKind::Shan {
        b11::start_relay(g, lord, kind, used);
    } else {
        b9::start_relay(g, lord, kind, used);
    }
}

/// 【杀】次数覆盖（咆哮）。
pub fn sha_limit_override(g: &Game, seat: usize) -> Option<usize> {
    b9::sha_limit_override(g, seat)
}

/// 距离修正（马术 -1 / 义从 -1）。
pub fn distance_delta(g: &Game, seat: usize) -> i32 {
    b9::distance_delta(g, seat) + b12::distance_delta(g, seat)
}

/// 【杀】的可达性覆盖（烈弓：距离 ≤ 此【杀】点数）。
pub fn range_override_reach(
    g: &Game,
    from: usize,
    to: usize,
    card: &crate::kernel::CardId,
) -> bool {
    b13::range_override_reach(g, from, to, card)
}

/// 「他人 → 我」的距离修正（义从：体力 ≤2 时 +1）。
pub fn distance_to_delta(g: &Game, seat: usize) -> i32 {
    b12::distance_to_delta(g, seat)
}

/// 改判候选顺序（判定者左手侧起、判定者最后）：持有鬼才/鬼道且仍有可用牌的座位。
pub fn judge_rewrite_holders(g: &Game, owner: usize) -> Vec<usize> {
    let mut order = g.seats_from_left_of(owner);
    order.push(owner);
    order
        .into_iter()
        .filter(|i| {
            g.seats.get(*i).is_some_and(|s| s.hp > 0)
                && !b13::rewrite_cards(g, *i).is_empty()
        })
        .collect()
}

/// 让指定座位进入改判询问（返回 true = 已压入询问帧）。
pub fn start_rewrite_for(
    g: &mut Game,
    judge_owner: usize,
    seat: usize,
    card: &crate::kernel::CardId,
) -> bool {
    if b13::active_guidao(g, seat) {
        b13::start_guidao(g, seat, judge_owner);
        return true;
    }
    if b11::active_guicai(g, seat) {
        b11::start_guicai(g, seat);
        return true;
    }
    let _ = card;
    false
}

/// 蛊惑是否可用（每回合限一次 + 有手牌）。
pub fn guhuo_available(g: &Game, seat: usize) -> bool {
    b13::guhuo_available(g, seat)
}

/// 响应窗口进入蛊惑（延后结果经 `Game.guhuo_result` 回传）。
pub fn start_guhuo_response(
    g: &mut Game,
    seat: usize,
    kind: crate::cards::ResponseKind,
) -> Result<(), String> {
    b13::start_guhuo_response(g, seat, kind)
}

/// 思蜀：该角色的【乐不思蜀】判定效果是否被反转。
pub fn sishu_reversed(g: &Game, seat: usize) -> bool {
    b13::sishu_reversed(g, seat)
}

/// 群体锦囊少选目标（乱击）：返回本次要裁掉的目标。
pub fn aoe_trim_target(g: &Game, _seat: usize) -> Option<usize> {
    g.counter("turn.aoe_trim")
        .checked_sub(1)
        .map(|n| n as usize)
}

/// 同一响应窗口需连续打出的响应牌张数（multi-response；无双）。
pub fn response_need(
    g: &Game,
    responder: usize,
    opponent: usize,
    kind: crate::cards::ResponseKind,
    effect: crate::cards::Effect,
) -> usize {
    b12::response_need(g, responder, opponent, kind, effect)
}

/// 【杀】无距离限制（界武圣·方片【杀】）。
pub fn sha_no_distance(g: &Game, seat: usize, card: &crate::kernel::CardId) -> bool {
    b9::sha_no_distance(g, seat, card)
}

/// 锦囊无距离限制（奇才）。
pub fn trick_no_distance(g: &Game, seat: usize) -> bool {
    b9::trick_no_distance(g, seat)
}

/// 防止弃置（奇才：其他角色弃置你装备区的防具/宝物）。
pub fn prevent_equip_discard(
    g: &Game,
    owner: usize,
    card: &crate::kernel::CardId,
    by: Option<usize>,
) -> bool {
    b9::prevent_equip_discard(g, owner, card, by)
}

// ---------------------------------------------------------------------------
// B10 钩子（跳过阶段 / 赋值型上限 / 次数 / 响应过滤 / 结算前询问）
// ---------------------------------------------------------------------------

pub fn skip_discard_phase(g: &Game, seat: usize) -> bool {
    b10::skip_discard_phase(g, seat)
}

pub fn hand_limit_override(g: &Game, seat: usize) -> Option<usize> {
    b10::hand_limit_override(g, seat)
}

pub fn sha_limit_bonus(g: &Game, seat: usize) -> usize {
    b10::sha_limit_bonus(g, seat)
}

pub fn block_real_shan(g: &Game, seat: usize) -> bool {
    b10::block_real_shan(g, seat)
}

pub fn botu_maybe(g: &mut Game, seat: usize) {
    b10::botu_maybe(g, seat)
}

pub fn start_fenwei(g: &mut Game, targets: &[usize], user: usize) -> bool {
    b10::start_fenwei(g, targets, user)
}

pub fn start_qianxun(g: &mut Game, user: usize, targets: &[usize], name: &str) -> bool {
    b10::start_qianxun(g, user, targets, name)
}

pub fn start_lianying(g: &mut Game, seat: usize, x: usize) -> bool {
    b10::start_lianying(g, seat, x)
}

pub fn on_equip_lost(g: &mut Game, seat: usize) {
    b10::xiaoji_on_lose_equip(g, seat);
}

pub fn start_jiuyuan(g: &mut Game, seat: usize, n: i32, cause: &str) -> bool {
    b10::start_jiuyuan(g, seat, n, cause)
}

pub fn return_void_cards(g: &mut Game, turn_seq: u32) {
    b10::return_void_cards(g, turn_seq);
}

/// 伤害数值加成钩子（暗箭；`flow::damage_now` 在扣血前调用）。
pub fn damage_bonus(
    g: &Game,
    source: usize,
    victim: usize,
    card: Option<&crate::kernel::CardId>,
) -> i32 {
    b4::anjian_bonus(g, source, victim, card)
}

/// 【杀】结算完毕的统一接入点（忠勇「使用【杀】后」/ 勇略「未造成伤害则摸 1」/ 奔袭「伤害后摸 1」）。
/// v2 的【杀】结算引擎（未来）与用例都从这里进入。
pub fn on_sha_finished(
    g: &mut Game,
    user: usize,
    target: Option<usize>,
    card: Option<&crate::kernel::CardId>,
    respond: Option<&crate::kernel::CardId>,
    damaged: bool,
) {
    b13::on_sha_finished(g, user, target);
    // 奔袭：造成伤害后摸一张（强化项 4）
    if damaged && g.counter("turn.benxi_draw") > 0 {
        g.counters.insert("turn.benxi_draw".to_string(), 0);
        let n = g.draw(user, 1).len();
        g.push_log(format!("奔袭：造成伤害后摸 {n} 张"));
    }
    // 勇略：未造成伤害（读伤害事件，裁定 Q1=A）→ 摸 1
    if let Some(t) = target {
        let key = format!("turn.yonglue_pending:{t}");
        if g.counter(&key) > 0 {
            g.counters.insert(key, 0);
            let owner = g
                .counter("turn.yonglue_owner")
                .checked_sub(1)
                .map(|n| n as usize);
            if let Some(owner) = owner {
                if !damaged && g.seats[owner].hp > 0 {
                    let n = g.draw(owner, 1).len();
                    g.push_log(format!("勇略：此【杀】未造成伤害，摸 {n} 张"));
                }
            }
        }
    }
    // 忠勇：使用【杀】后（可交牌）
    b4::zhongyong_after_sha(g, user, target, card, respond);
}

/// 「权」数量（钟会·权计；移出游戏的牌，用手牌上限 +1 的计数表达）。
/// 授予技能（绝响：随机获得「和弦/柔弦/烈弦/激弦」之一）。
pub fn grant_skill(g: &mut Game, seat: usize, code: &str) {
    if !g.seats[seat].granted.iter().any(|x| x == code) {
        g.seats[seat].granted.push(code.to_string());
    }
    rebuild_subs(g);
}

pub fn quan(g: &Game, seat: usize) -> u32 {
    g.seats
        .get(seat)
        .and_then(|s| s.counters.get(&"quan".to_string()).copied())
        .unwrap_or(0)
}

pub fn add_quan(g: &mut Game, seat: usize, n: u32) {
    *g.seats[seat].counters.entry("quan".to_string()).or_insert(0) += n;
}

/// 记录勇略拥有者（`on_sha_finished` 未造成伤害时给他摸 1）。
pub fn set_yonglue_owner(g: &mut Game, seat: usize) {
    g.counters
        .insert("turn.yonglue_owner".to_string(), seat as u32 + 1);
}

/// 开局钩子（怀橘获得 3 枚「橘」等）。
pub fn on_game_start(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if active(g, seat, "huaiju") {
            add_ju(g, seat, 3);
        }
    }
    // 宗祚：开局按势力数增加体力上限并回复 1 点。
    b7::init_zongzuo(g);
    // 血裔：开局按群势力角色数获得「裔」。
    b13::init_xueyi(g);
    // 化身：开局获得 3 张化身牌并展示第一张。
    b13::init_huashen(g);
    // 明任：开局摸 1 张并必选一张作为「任」（压入 Ask 帧，由调用方 advance 驱动）
    for seat in 0..g.seats.len() {
        if active(g, seat, "mingren") {
            mingren::start_init(g, seat);
        }
    }
}

/// 回合结束钩子：把「到期」的延迟结算交给对应技能（目前只有镇骨）。
pub fn on_turn_end_for_scheduled(g: &mut Game, ended: usize) {
    let owners: Vec<usize> = g
        .scheduled
        .iter()
        .filter(|(_, t)| *t == ended)
        .map(|(o, _)| *o)
        .collect();
    for owner in owners {
        b2::zhengu_end(g, owner, ended);
    }
}
