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
];

pub fn def(code: &str) -> Option<&'static SkillDef> {
    SKILLS.iter().find(|s| s.code == code)
}

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    def(code).map(|d| d.handler)
}

/// **唯一**技能状态查询入口。
pub fn active(g: &Game, seat: usize, code: &str) -> bool {
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
