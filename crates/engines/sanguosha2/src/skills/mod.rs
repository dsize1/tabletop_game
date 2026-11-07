//! 技能注册表：**声明 + handler**，禁止在核心流程写武将名分支。
//!
//! 单一状态源：`active()`（自带 ∪ 授予）。旧引擎有 6 种「我现在有这个技能吗」的
//! 表达（has / flags / turn_counters / marks / mark_counts / GrantedSkill），钩子只查
//! 其中一种 → 「两处各写一半」。这里只允许 `active()`。

pub mod b0;
pub mod b1;
pub mod b1b;
pub mod b2;
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
        _ => {}
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

/// 翻至正面：调用所有拥有者的 flip 钩子（解围②）。
pub fn on_flip_to_face_up(g: &mut Game, seat: usize) {
    for d in SKILLS {
        if !active(g, seat, d.code) {
            continue;
        }
        if let Some(h) = d.flip_hook {
            h(g, seat);
        }
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

/// 开局钩子（怀橘获得 3 枚「橘」等）。
pub fn on_game_start(g: &mut Game) {
    for seat in 0..g.seats.len() {
        if active(g, seat, "huaiju") {
            add_ju(g, seat, 3);
        }
    }
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
