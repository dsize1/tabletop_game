//! B1 技能（风林火山·SP-1：8 将 / 18 技能）。
//!
//! 依据 `docs/games/sanguosha/align/B1.md` 的**对齐后描述（权威）**与 24 条裁定：
//! 袁术 庸肆/伪帝、王基 奇制/进趋、严颜 拒战、王平 飞军/兵略、陆绩 怀橘/遗礼/整论、
//! 孙亮 溃诛/掣政/立军、蒯越蒯良 荐降/审时、许攸 成略/恃才/寸目。
//!
//! 三条形态约束（沿用 B0 范式，B1 起固化）：
//! 1. **声明 → 行为**：窗口/订阅/钩子在 `SKILLS`，行为在本模块 handler（只返回 `Step`）；
//! 2. **转换技拆 face**（§9.1 裁定 B）：拒战/审时/成略各两面，`transform: true`，路由只读当前面；
//! 3. **数值钩子单出口**：摸牌加成/摸牌方向/整段跳过/伤害防止/牌无效都从这里委托，
//!    内核（`flow`/`card_play`）不出现技能名分支。
//!
//! 约定：跨阶段状态只走 `Game::counter` / `Game::mark`（键自带命名空间）；「橘」走 `Seat.counters`（计数标记，非牌）。

use serde_json::{json, Value};

use super::super::cards::{self, Category};
use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{DamageKind, DamageSpec, Game, Phase, SourceKind};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

/// 「橘」计数标记名（怀橘：计数标记，不是牌，不可被拆/顺/弃）。
pub const JU: &str = "ju";

/// B1 声明（18 技能）。
pub static SKILLS: &[SkillDef] = &[
    // —— 袁术 ——
    SkillDef {
        code: "yongsi",
        name: "庸肆",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            // 弃牌阶段开始时先弃 X（摸牌加成在 `draw_bonus`，摸牌阶段单出口）。
            window: Window::OwnPhase(Phase::Discard),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "weidi",
        name: "伪帝",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 王基 ——
    SkillDef {
        code: "qizhi",
        name: "奇制",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            // 回合内使用非装备牌指定目标后（`Subject::By` = 使用者是我，不含响应打出）。
            window: Window::OwnTurn,
            subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::By }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "jinzou",
        name: "进趋",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::End),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: true,
        }],
    },
    // —— 严颜（转换技：阳 = 他人回合，阴 = 自己回合）——
    SkillDef {
        code: "juzhan",
        name: "拒战",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: true,
        faces: &[
            Face {
                // 阳：他人回合内，成为其他角色【杀】的目标后。
                window: Window::OtherTurn,
                subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ }],
                hooks: &[],
                can_active: false,
            },
            Face {
                // 阴：严颜自己的回合内，使用【杀】指定目标后。
                window: Window::OwnTurn,
                subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::By }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    // —— 王平 ——
    SkillDef {
        code: "feijun",
        name: "飞军",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "binglue",
        name: "兵略",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 陆绩 ——
    SkillDef {
        code: "huaiju",
        name: "怀橘",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::GameStart, Hook::DamagePrevent],
            can_active: false,
        }],
    },
    SkillDef {
        code: "yili",
        name: "遗礼",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: true,
        }],
    },
    SkillDef {
        code: "zhenglun",
        name: "整论",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Draw),
            subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }],
            hooks: &[],
            can_active: true,
        }],
    },
    // —— 孙亮 ——
    SkillDef {
        code: "kuizhu",
        name: "溃诛",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Discard),
            subscriptions: &[Subscription { event: EventKind::PhaseEnd, subject: Subject::Any }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "chezheng",
        name: "掣政",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnPhase(Phase::Play),
            subscriptions: &[Subscription { event: EventKind::PhaseEnd, subject: Subject::Any }],
            hooks: &[Hook::CardIneffective],
            can_active: false,
        }],
    },
    SkillDef {
        code: "lijun",
        name: "立军",
        kind: SkillKind::Lord,
        grant_only: false,
        transform: false,
        faces: &[Face {
            // 主公技他人侧：其他吴势力角色在自己的出牌阶段使用【杀】后。
            window: Window::OtherPhase(Phase::Play),
            subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::Other }],
            hooks: &[],
            can_active: false,
        }],
    },
    // —— 蒯越蒯良 ——
    SkillDef {
        code: "jianxiang",
        name: "荐降",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[Subscription { event: EventKind::TargetChosen, subject: Subject::Self_ }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "shenshi",
        name: "审时",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: true,
        faces: &[
            Face {
                // 阳：回合内、出牌阶段（每阶段限一次）。
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            Face {
                // 阴：回合外，其他角色对你造成伤害后。
                window: Window::OtherTurn,
                subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }],
                hooks: &[],
                can_active: false,
            },
        ],
    },
    // —— 许攸 ——
    SkillDef {
        code: "chenglue",
        name: "成略",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: true,
        faces: &[
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
            Face {
                window: Window::OwnPhase(Phase::Play),
                subscriptions: &[],
                hooks: &[],
                can_active: true,
            },
        ],
    },
    SkillDef {
        code: "shicai",
        name: "恃才",
        kind: SkillKind::Normal,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::OwnTurn,
            subscriptions: &[Subscription { event: EventKind::CardUsed, subject: Subject::By }],
            hooks: &[],
            can_active: false,
        }],
    },
    SkillDef {
        code: "cunmu",
        name: "寸目",
        kind: SkillKind::Locked,
        grant_only: false,
        transform: false,
        faces: &[Face {
            window: Window::AnyTime,
            subscriptions: &[],
            hooks: &[Hook::DrawFromBottom],
            can_active: false,
        }],
    },
];

/// B1 handler 分发。
pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "yongsi" => yongsi,
        "weidi" => noop,
        "qizhi" => qizhi,
        "jinzou" => jinzou,
        "juzhan" => juzhan,
        "feijun" => feijun,
        "binglue" => binglue,
        "huaiju" => huaiju,
        "yili" => yili,
        "zhenglun" => zhenglun,
        "kuizhu" => kuizhu,
        "chezheng" => chezheng,
        "lijun" => lijun,
        "jianxiang" => jianxiang,
        "shenshi" => shenshi,
        "chenglue" => chenglue,
        "shicai" => shicai,
        "cunmu" => noop,
        _ => return None,
    })
}

/// 主动技可用性判定（`skills::active_actions` 唯一读取）：每阶段/每回合次数 + 前置条件。
pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "jinzou" => can_jinzou,
        "feijun" => can_feijun,
        "yili" => can_yili,
        "zhenglun" => can_zhenglun,
        "shenshi" => can_shenshi,
        "chenglue" => can_chenglue,
        _ => return None,
    })
}

fn can_jinzou(g: &Game, seat: usize) -> bool {
    g.turn == seat
        && g.phase == Phase::End
        && g.counter(&turn_key(seat, "qizhi")) > 0
        && g.counter(&phase_key(seat, "jinzou")) == 0
}

fn can_feijun(g: &Game, seat: usize) -> bool {
    if g.turn != seat || g.phase != Phase::Play || g.counter(&turn_key(seat, "feijun")) > 0 {
        return false;
    }
    if own_cards(g, seat).is_empty() {
        return false;
    }
    alive_others(g, seat).into_iter().any(|t| {
        g.hand_size(t) > g.hand_size(seat) || equip_count(g, t) > equip_count(g, seat)
    })
}

fn can_yili(g: &Game, seat: usize) -> bool {
    g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "yili")) == 0
        && !alive_others(g, seat).is_empty()
}

fn can_zhenglun(g: &Game, seat: usize) -> bool {
    g.turn == seat && g.phase == Phase::Draw && ju(g, seat) == 0
}

fn can_shenshi(g: &Game, seat: usize) -> bool {
    if g.turn != seat || g.phase != Phase::Play || g.counter(&phase_key(seat, "shenshi")) > 0 {
        return false;
    }
    if skills::face_of(g, seat, "shenshi") != 0 || own_cards(g, seat).is_empty() {
        return false;
    }
    !alive_others(g, seat).is_empty()
}

fn can_chenglue(g: &Game, seat: usize) -> bool {
    g.turn == seat
        && g.phase == Phase::Play
        && g.counter(&phase_key(seat, "chenglue")) == 0
        && !g.hand(seat).is_empty()
}

/// 主动技的「发动/放弃」二选一 pending（所有主动技统一形态）。
fn choose_use(actor: usize, skill: &'static str, label: &str, data: Value) -> Step {
    Step::Need(
        PendingBuilder::new(actor, "choose")
            .skill(skill)
            .option("use", label)
            .escape("不发动")
            .data(data)
            .build(),
    )
}


/// 触发类可选技能的统一「发动/放弃」询问（首帧；`Step::Need` 不改 data）。
fn trigger_ask(actor: usize, skill: &'static str, label: &str, data: Value) -> Step {
    Step::Need(
        PendingBuilder::new(actor, "choose")
            .skill(skill)
            .option("use", label)
            .escape("不发动")
            .data(data)
            .build(),
    )
}

/// 强制选牌询问（**禁止**用 `cands.first()` 兜底代选，constitution I1）。
fn must_pick(
    g: &Game,
    actor: usize,
    skill: &'static str,
    verb: &str,
    cands: &[String],
    data: Value,
) -> Step {
    let mut b = PendingBuilder::new(actor, "pick_other").skill(skill);
    for id in cands {
        b = b.option(format!("card:{id}"), format!("{verb}【{}】", card_label(g, id)));
    }
    Step::Need(b.escape_mandatory("（必选一张）").data(data).build())
}

// ---------------------------------------------------------------------------
// 数值/回合钩子的实现（`skills::draw_bonus` 等唯一出口的落点）
// ---------------------------------------------------------------------------

/// 「橘」数量（Seat 计数标记）。
pub fn ju(g: &Game, seat: usize) -> i32 {
    g.seats.get(seat).map(|s| s.counter(JU)).unwrap_or(0)
}

/// 增减「橘」（钳到 ≥ 0）。
pub fn add_ju(g: &mut Game, seat: usize, n: i32) -> i32 {
    match g.seats.get_mut(seat) {
        Some(s) => s.add_counter(JU, n),
        None => 0,
    }
}

/// 移除 n 枚「橘」；不足则不动作并返回 false。
pub fn remove_ju(g: &mut Game, seat: usize, n: i32) -> bool {
    if ju(g, seat) < n {
        return false;
    }
    add_ju(g, seat, -n);
    true
}

/// 摸牌阶段加成：庸肆 +X（X = 存活不同势力数）+ 怀橘 +1（有橘）。M6 摸牌阶段单出口调用。
pub fn draw_bonus(g: &Game, seat: usize) -> usize {
    let mut n = 0;
    if skills::active(g, seat, "yongsi") {
        n += g.kingdom_count();
    }
    if skills::active(g, seat, "huaiju") && ju(g, seat) > 0 {
        n += 1;
    }
    n
}

/// 寸目：所有摸牌改为从牌堆底摸（`flow::draw` 唯一调用点）。
pub fn draw_from_bottom(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "cunmu")
}

/// 整论：本回合摸牌阶段被整段跳过（`turn.*` 计数，回合切换由 M6 回合驱动清理）。
pub fn skip_draw(g: &Game, seat: usize) -> bool {
    g.counter(&format!("turn.{seat}.skip_draw")) > 0
}

/// 怀橘：受到伤害时自动移除 1 枚「橘」并**防止该次伤害**（锁定，不询问）。
pub fn prevent_damage(g: &mut Game, victim: usize) -> bool {
    if !skills::active(g, victim, "huaiju") || ju(g, victim) <= 0 {
        return false;
    }
    add_ju(g, victim, -1);
    true
}

/// 掣政①：你的出牌阶段内，攻击范围不含你的其他角色对你使用牌**无效**。
pub fn card_ineffective(g: &Game, user: usize, target: usize) -> bool {
    if !skills::active(g, target, "chezheng") || g.turn != target || g.phase != Phase::Play {
        return false;
    }
    !in_attack_range(g, user, target)
}

/// 攻击范围是否覆盖（`gear` 单一距离入口）。
pub fn in_attack_range(g: &Game, from: usize, to: usize) -> bool {
    gear::attack_range(g, from) as usize >= gear::distance(g, from, to)
}

/// 拒战「不能再对你使用牌」：本回合 `user` 被禁止对 `target` 使用牌。
pub fn deny_target(g: &Game, user: usize, target: usize) -> bool {
    g.counter(&format!("deny.{}.{user}.{target}", g.turn)) > 0
}

fn forbid_target(g: &mut Game, user: usize, target: usize) {
    g.bump(&format!("deny.{}.{user}.{target}", g.turn));
}

/// 成略：「与弃置牌花色相同的牌无距离与次数限制」。花色按弃置牌记录（可多花色，**本阶段**有效）。
pub fn chenglue_free(g: &Game, seat: usize, suit: super::super::Suit) -> bool {
    let want = format!("{}:{:?}", g.turn, g.phase);
    g.mark(&format!("free.{seat}.{suit:?}")) == Some(want.as_str())
}

/// 兵略：本局是否已对 `target` 结算过飞军（永久，每目标一次）。
pub fn binglue_used(g: &Game, actor: usize, target: usize) -> bool {
    g.counter(&format!("binglue.{actor}.{target}")) > 0
}

/// 立军带来的【杀】次数加成（本回合）。
pub fn sha_extra(g: &Game, seat: usize) -> i32 {
    g.counter(&format!("turn.{seat}.sha_limit"))
}

// ---------------------------------------------------------------------------
// 内部小工具
// ---------------------------------------------------------------------------

fn noop(_g: &mut Game, _a: &Ask) -> Result<Step, String> {
    Ok(Step::Done)
}

fn turn_key(seat: usize, key: &str) -> String {
    format!("turn.{seat}.{key}")
}

fn phase_key(seat: usize, key: &str) -> String {
    format!("phase.{seat}.{key}")
}

fn hook_of(ask: &Ask) -> &str {
    ask.data.get("hook").and_then(|v| v.as_str()).unwrap_or("")
}

fn seat_of(ask: &Ask, key: &str) -> Option<usize> {
    ask.data.get(key).and_then(|v| v.as_u64()).map(|v| v as usize)
}

fn card_of(ask: &Ask, key: &str) -> Option<String> {
    ask.data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn seats_of(ask: &Ask, key: &str) -> Vec<usize> {
    ask.data
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect())
        .unwrap_or_default()
}


/// 自己可控的牌（手牌 + 装备区，按牌 id 排序 = 确定性）。
fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut ids = g.hand(seat);
    for slot in [
        super::super::Slot::Weapon,
        super::super::Slot::Armor,
        super::super::Slot::HorseOffense,
        super::super::Slot::HorseDefense,
        super::super::Slot::Treasure,
    ] {
        if let Some(id) = gear::equip_in(g, seat, slot) {
            ids.push(id.clone());
        }
    }
    ids.sort();
    ids
}

fn equip_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, super::super::Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn equip_count(g: &Game, seat: usize) -> usize {
    equip_cards(g, seat).len()
}

/// 目标身上「可被技能拿走/弃置」的牌：手牌 + 装备区 + 判定区（奇制 Q1 口径）。
fn target_any_cards(g: &Game, target: usize) -> Vec<String> {
    let mut ids = own_cards(g, target);
    for id in g.cards.iter().filter(|(_, z)| matches!(z, super::super::Zone::Judge(s) if *s == target)) {
        ids.push(id.0.clone());
    }
    ids.sort();
    ids
}

/// 目标身上「获得其一张牌」的合法区：**仅手牌与装备区**（拒战 Q2 裁定 B）。
fn target_own_cards(g: &Game, target: usize) -> Vec<String> {
    own_cards(g, target)
}

fn pick_from(ask: &Ask, key: &str, cands: &[String]) -> Option<String> {
    card_of(ask, key)
        .filter(|c| cands.iter().any(|x| x == c))
        .or_else(|| cands.first().cloned())
}

fn alive_others(g: &Game, seat: usize) -> Vec<usize> {
    (0..g.seats.len()).filter(|&i| i != seat && g.seats[i].alive).collect()
}

/// 牌面名（pending 选项文案；查不到定义时回退牌 id）。
fn card_label(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| id.to_string())
}

// ---------------------------------------------------------------------------
// 袁术 · 庸肆（锁定）：弃牌阶段先弃 X（X = 存活不同势力数），计入本阶段弃牌数
// ---------------------------------------------------------------------------

fn yongsi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let k = phase_key(actor, "yongsi");
    if g.counter(&k) > 0 {
        return Ok(Step::Done);
    }
    g.bump(&k);
    let x = g.kingdom_count();
    if x == 0 {
        return Ok(Step::Done);
    }
    // 弃牌阶段开始时先弃 X 张（不足则弃光）；走 `flow::discard` → 计入 phase.discarded.Discard。
    let hand = g.hand(actor);
    let mut n = 0;
    for id in hand.into_iter().take(x) {
        if flow::discard(g, actor, &id, "庸肆").is_some() {
            n += 1;
        }
    }
    Ok(Step::Finish(format!("庸肆：弃牌阶段先弃 {n} 张（X={x}）")))
}

// ---------------------------------------------------------------------------
// 王基 · 奇制：回合内使用非装备牌指定目标后 → 弃目标一张牌 + 其摸 1；不限次
// ---------------------------------------------------------------------------

fn qizhi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    if target == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let cands = target_any_cards(g, target);
    if cands.is_empty() {
        return Ok(Step::Done); // 目标无牌：不提供发动
    }
    if step == "ask" {
        return match take_answer(g, "qizhi") {
            None => Ok(trigger_ask(
                actor,
                "qizhi",
                "奇制：弃其一张牌，然后令其摸一张牌",
                json!({ "step": "ask", "target": target }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "pick", "target": target }),
            )),
            Some(o) => Err(format!("奇制：未知选项 {o}")),
        };
    }
    match take_answer(g, "qizhi") {
        None => Ok(must_pick(
            g,
            actor,
            "qizhi",
            "弃置",
            &cands,
            json!({ "step": "pick", "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("奇制：只能弃置目标区域里的一张牌".into());
            }
            flow::discard(g, actor, &id, "奇制");
            flow::draw(g, target, 1);
            g.bump(&turn_key(actor, "qizhi"));
            Ok(Step::Finish("奇制：弃其一张牌，令其摸一张牌".into()))
        }
        Some(o) => Err(format!("奇制：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 王基 · 进趋：结束阶段摸 2 后弃至 X（X = 本回合奇制次数）；X=0 不允许发动
// ---------------------------------------------------------------------------

fn jinzou(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    // B1 Q6 裁定 B：奇制 0 次时不允许发动（不给入口）。
    if g.counter(&turn_key(actor, "qizhi")) <= 0 || g.counter(&phase_key(actor, "jinzou")) > 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "jinzou") {
        None => Ok(choose_use(
            actor,
            "jinzou",
            "进趋：摸两张牌，然后将手牌弃至 X 张",
            json!({ "step": "choose" }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            let x = g.counter(&turn_key(actor, "qizhi"));
            g.bump(&phase_key(actor, "jinzou"));
            flow::draw(g, actor, 2);
            let hand = g.hand(actor);
            let excess = hand.len().saturating_sub(x as usize);
            for id in hand.into_iter().take(excess) {
                flow::discard(g, actor, &id, "进趋");
            }
            Ok(Step::Finish(format!("进趋：摸 2 张后弃至 {x} 张")))
        }
        Some(o) => Err(format!("进趋：未知选项 {o}")),
    }
}
// ---------------------------------------------------------------------------
// 严颜 · 拒战（转换技）
// ---------------------------------------------------------------------------

fn juzhan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let face = ask.data.get("face").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if face == 0 {
        // 阳：他人回合内成为【杀】目标后 → 各摸 1，然后其本回合不能再对你使用牌。
        let Some(src) = seat_of(ask, "by") else {
            return Ok(Step::Done);
        };
        if src == actor || !g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        return match take_answer(g, "juzhan") {
            None => Ok(trigger_ask(
                actor,
                "juzhan",
                "拒战（阳）：与其各摸一张，其本回合不能再对你使用牌",
                json!({ "step": "ask", "face": 0, "by": src }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                flow::draw(g, actor, 1);
                flow::draw(g, src, 1);
                forbid_target(g, src, actor);
                skills::flip_face(g, actor, "juzhan");
                Ok(Step::Finish(
                    "拒战（阳）：与来源各摸一张，其本回合不能再对你使用牌".into(),
                ))
            }
            Some(o) => Err(format!("拒战：未知选项 {o}")),
        };
    }
    // 阴：自己回合内使用【杀】指定目标后 → 获得其一张牌（仅手牌/装备），本回合不能再对其使用牌。
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    if target == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    let cands = target_own_cards(g, target);
    if cands.is_empty() {
        return Ok(Step::Done); // 无牌可获得：不提供发动
    }
    if step == "ask" {
        return match take_answer(g, "juzhan") {
            None => Ok(trigger_ask(
                actor,
                "juzhan",
                "拒战（阴）：获得其一张牌，本回合不能再对其使用牌",
                json!({ "step": "ask", "face": 1, "target": target }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::PickOther,
                json!({ "step": "pick", "face": 1, "target": target }),
            )),
            Some(o) => Err(format!("拒战：未知选项 {o}")),
        };
    }
    match take_answer(g, "juzhan") {
        None => Ok(must_pick(
            g,
            actor,
            "juzhan",
            "获得",
            &cands,
            json!({ "step": "pick", "face": 1, "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("拒战（阴）：只能获得其手牌或装备区的一张牌".into());
            }
            flow::gain(g, actor, &id, actor, "拒战");
            forbid_target(g, actor, target);
            skills::flip_face(g, actor, "juzhan");
            Ok(Step::Finish(
                "拒战（阴）：获得其一张牌，本回合不能再对其使用牌".into(),
            ))
        }
        Some(o) => Err(format!("拒战：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 王平 · 飞军 / 兵略
// ---------------------------------------------------------------------------

fn feijun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    let opt = ask.data.get("opt").and_then(|v| v.as_u64()).unwrap_or(1);
    match step {
        "choose" => {
            if g.counter(&turn_key(actor, "feijun")) > 0 || own_cards(g, actor).is_empty() {
                return Ok(Step::Done); // 出牌阶段限一次 / 无牌可弃
            }
            match take_answer(g, "feijun") {
                None => {
                    let others = alive_others(g, actor);
                    let can1 = others.iter().any(|&t| g.hand_size(t) > g.hand_size(actor));
                    let can2 = others.iter().any(|&t| equip_count(g, t) > equip_count(g, actor));
                    if !can1 && !can2 {
                        return Ok(Step::Done); // 两选项都无合法目标：不提供发动
                    }
                    let mut b = PendingBuilder::new(actor, "choose").skill("feijun");
                    if can1 {
                        b = b.option("use:1", "飞军①：令手牌数大于你的角色交给你一张牌");
                    }
                    if can2 {
                        b = b.option("use:2", "飞军②：令装备数大于你的角色弃一件装备");
                    }
                    Ok(Step::Need(
                        b.escape("不发动飞军").data(json!({ "step": "choose" })).build(),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("use:") => {
                    let opt: u64 = a[4..].parse().map_err(|_| "飞军：未知选项")?;
                    Ok(Step::Next(Stage::PickOwn, json!({ "step": "cost", "opt": opt })))
                }
                Some(o) => Err(format!("飞军：未知选项 {o}")),
            }
        }
        "cost" => match take_answer(g, "feijun") {
            None => {
                let own = own_cards(g, actor);
                let mut b = PendingBuilder::new(actor, "pick_own").skill("feijun");
                for id in &own {
                    b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape("不发动飞军").data(json!({ "step": "cost", "opt": opt })).build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("card:") => {
                let id = a[5..].to_string();
                if !own_cards(g, actor).contains(&id) {
                    return Err("飞军：须弃置自己一张牌".into());
                }
                flow::discard(g, actor, &id, "飞军");
                Ok(Step::Next(Stage::Target, json!({ "step": "target", "opt": opt })))
            }
            Some(o) => Err(format!("飞军：未知选项 {o}")),
        },
        "target" => match take_answer(g, "feijun") {
            None => {
                let cands: Vec<usize> = alive_others(g, actor)
                    .into_iter()
                    .filter(|&t| match opt {
                        1 => g.hand_size(t) > g.hand_size(actor),
                        _ => equip_count(g, t) > equip_count(g, actor),
                    })
                    .collect();
                if cands.is_empty() {
                    return Ok(Step::Finish("飞军：无合法目标".into()));
                }
                let mut b = PendingBuilder::new(actor, "target").skill("feijun");
                for t in &cands {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target", "opt": opt }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "飞军：非法目标")?;
                if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) || t == actor {
                    return Err("飞军：目标非法".into());
                }
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "pick", "opt": opt, "target": t }),
                ))
            }
            Some(o) => Err(format!("飞军：未知选项 {o}")),
        },
        "pick" => {
            let target = seat_of(ask, "target").ok_or("飞军：缺少目标")?;
            match take_answer(g, "feijun") {
                None => {
                    // ①目标**自选**一张手牌（pending 归属 = 目标）；②由王平选装备。
                    let (cands, asker) = if opt == 1 {
                        (g.hand(target), target)
                    } else {
                        (equip_cards(g, target), actor)
                    };
                    if cands.is_empty() {
                        return feijun_finish(g, actor, opt, target, None);
                    }
                    let mut b = PendingBuilder::new(asker, "pick_other").skill("feijun");
                    for id in &cands {
                        b = b.option(format!("card:{id}"), format!("选择【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "pick", "opt": opt, "target": target }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    feijun_finish(g, actor, opt, target, Some(id))
                }
                Some(o) => Err(format!("飞军：未知选项 {o}")),
            }
        }
        other => Err(format!("飞军：未知阶段 {other}")),
    }
}

fn feijun_finish(
    g: &mut Game,
    actor: usize,
    opt: u64,
    target: usize,
    pick: Option<String>,
) -> Result<Step, String> {
    if let Some(id) = pick {
        if opt == 1 {
            flow::gain(g, actor, &id, target, "飞军");
        } else {
            flow::discard(g, actor, &id, "飞军");
        }
    }
    g.bump(&turn_key(actor, "feijun"));
    binglue_on_feijun(g, actor, target);
    Ok(Step::Finish(
        if opt == 1 { "飞军：得其一牌" } else { "飞军：弃其一装备" }.into(),
    ))
}
/// 兵略（锁定）：首次对某角色发动飞军 → 摸 2（本局永久，每目标一次）。
fn binglue_on_feijun(g: &mut Game, actor: usize, target: usize) {
    if !skills::active(g, actor, "binglue") || binglue_used(g, actor, target) {
        return;
    }
    g.bump(&format!("binglue.{actor}.{target}"));
    flow::draw(g, actor, 2);
}

fn binglue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    binglue_on_feijun(g, ask.actor, target);
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 陆绩 · 怀橘 / 遗礼 / 整论
// ---------------------------------------------------------------------------

fn huaiju(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    if hook_of(ask) == "game_start" {
        add_ju(g, ask.actor, 3); // 游戏开始时获得 3 枚「橘」（计数标记）
    }
    Ok(Step::Done)
}

fn yili(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    // 濒死救回后的补发（延迟队列 `SkillTrigger` 在濒死流程结束后派发）。
    if hook_of(ask) == "grant_if_rescued" {
        let Some(target) = seat_of(ask, "target") else {
            return Ok(Step::Done);
        };
        if !g.seats.get(actor).map(|s| s.alive && s.hp > 0).unwrap_or(false) {
            return Ok(Step::Done); // 未救回：不发放
        }
        add_ju(g, target, 1);
        return Ok(Step::Finish(format!(
            "遗礼：救回后令其获得 1 枚「橘」（现有 {}）",
            ju(g, target)
        )));
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    if g.counter(&phase_key(actor, "yili")) > 0 {
        return Ok(Step::Done); // 每阶段一次
    }
    match step {
        "choose" => match take_answer(g, "yili") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("yili");
                b = b.option("cost:hp", "代价：失去 1 点体力");
                if ju(g, actor) > 0 {
                    b = b.option("cost:ju", "代价：移除 1 枚「橘」");
                }
                Ok(Step::Need(
                    b.escape("不发动遗礼").data(json!({ "step": "choose" })).build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("cost:") => {
                let cost = a[5..].to_string();
                if cost != "hp" && cost != "ju" {
                    return Err(format!("遗礼：未知代价 {cost}"));
                }
                if cost == "ju" && ju(g, actor) <= 0 {
                    return Err("遗礼：没有「橘」可移除".into());
                }
                Ok(Step::Next(Stage::Target, json!({ "step": "target", "cost": cost })))
            }
            Some(o) => Err(format!("遗礼：未知选项 {o}")),
        },
        "target" => {
            let cost = ask.data.get("cost").and_then(|v| v.as_str()).unwrap_or("hp").to_string();
            match take_answer(g, "yili") {
                None => {
                    let cands = alive_others(g, actor);
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("yili");
                    for t in &cands {
                        b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选目标）")
                            .data(json!({ "step": "target", "cost": cost }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("seat:") => {
                    let target: usize = a[5..].parse().map_err(|_| "遗礼：非法目标")?;
                    if target == actor || !g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
                        return Err("遗礼：目标须是其他存活角色".into());
                    }
                    yili_finish(g, actor, &cost, target)
                }
                Some(o) => Err(format!("遗礼：未知选项 {o}")),
            }
        }
        other => Err(format!("遗礼：未知阶段 {other}")),
    }
}

fn yili_finish(g: &mut Game, actor: usize, cost: &str, target: usize) -> Result<Step, String> {
    g.bump(&phase_key(actor, "yili"));
    if cost == "ju" {
        if !remove_ju(g, actor, 1) {
            return Err("遗礼：没有「橘」可移除".into());
        }
        add_ju(g, target, 1);
        return Ok(Step::Finish(format!(
            "遗礼：移除 1 枚「橘」，令其获得 1 枚（现有 {}）",
            ju(g, target)
        )));
    }
    flow::lose_hp(g, actor, 1, "遗礼");
    if flow::is_dying(&g.seats[actor]) {
        // B1 Q14 裁定：先完成濒死并救回，之后才令目标获得「橘」——
        // 挂延迟队列（`SkillTrigger` 排在濒死流程之后派发），救回后才发放。
        g.deferred.push(super::super::deferred::Deferred::ask(
            super::super::deferred::DeferredKind::SkillTrigger,
            "yili",
            Stage::Choose,
            actor,
            json!({ "hook": "grant_if_rescued", "target": target }),
        ));
        return Ok(Step::Finish("遗礼：失去体力致濒死，待救回后发放「橘」".into()));
    }
    add_ju(g, target, 1);
    Ok(Step::Finish(format!(
        "遗礼：令其获得 1 枚「橘」（现有 {}）",
        ju(g, target)
    )))
}
fn zhenglun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if ju(g, actor) > 0 {
        return Ok(Step::Done); // 有橘时不询问
    }
    match take_answer(g, "zhenglun") {
        None => Ok(choose_use(
            actor,
            "zhenglun",
            "整论：跳过摸牌阶段并获得 1 枚「橘」",
            json!({ "step": "choose" }),
        )),
        Some(a) if a == "pass" => Ok(Step::Done), // 放弃：正常摸牌
        Some(a) if a == "use" => {
            // 跳过**整段**摸牌阶段（所有加成都不结算），获得 1 枚「橘」（Q15 裁定 A）。
            g.set_counter(&turn_key(actor, "skip_draw"), 1);
            add_ju(g, actor, 1);
            Ok(Step::Finish("整论：跳过摸牌阶段，获得 1 枚「橘」".into()))
        }
        Some(o) => Err(format!("整论：未知选项 {o}")),
    }
}
// ---------------------------------------------------------------------------
// 孙亮 · 溃诛 / 掣政 / 立军
// ---------------------------------------------------------------------------

fn kuizhu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    // X = **本阶段**（弃牌阶段）弃置的牌数（Q1/Q16 裁定：只算弃牌阶段）。
    let x = g.counter(&format!("phase.discarded.{:?}", Phase::Discard));
    if x <= 0 {
        return Ok(Step::Done);
    }
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("mode");
    if step == "mode" {
        // ② 的组合必须在本阶段就枚举好：无满足组合则不提供该选项。
        let combos = kuizhu_combos(g, x);
        return match take_answer(g, "kuizhu") {
            None => {
                let mut b = PendingBuilder::new(actor, "choose").skill("kuizhu");
                b = b.option(
                    format!("use:1"),
                    format!("溃诛①：令至多 {x} 名角色各摸一张"),
                );
                if !combos.is_empty() {
                    b = b.option("use:2", "溃诛②：对体力值之和等于 X 的任意名角色各造成 1 点伤害");
                }
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "mode", "x": x }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use:1" => Ok(Step::Next(
                Stage::MultiSelect,
                json!({ "step": "pick1", "x": x, "picked": [] }),
            )),
            Some(a) if a == "use:2" => Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "pick2", "x": x }),
            )),
            Some(o) => Err(format!("溃诛：未知选项 {o}")),
        };
    }
    if step == "pick1" {
        let picked: Vec<usize> = ask
            .data
            .get("picked")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
            .unwrap_or_default();
        let x = ask.data.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        return match take_answer(g, "kuizhu") {
            None => {
                let mut b = PendingBuilder::new(actor, "multi_select").skill("kuizhu");
                for t in (0..g.seats.len()).filter(|&t| g.seats[t].alive && !picked.contains(&t)) {
                    if picked.len() >= x {
                        break;
                    }
                    b = b.option(format!("t:{t}"), format!("令 #{} 摸一张", t + 1));
                }
                if !picked.is_empty() {
                    b = b.option("ok", "确定");
                }
                Ok(Step::Need(
                    b.escape_with("cancel", "放弃")
                        .data(json!({ "step": "pick1", "x": x, "picked": picked }))
                        .build(),
                ))
            }
            Some(a) if a == "cancel" => Ok(Step::Done),
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "溃诛：非法目标")?;
                if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) || picked.contains(&t) {
                    return Err("溃诛：目标非法".into());
                }
                let mut next = picked.clone();
                next.push(t);
                Ok(Step::Next(
                    Stage::MultiSelect,
                    json!({ "step": "pick1", "x": x, "picked": next }),
                ))
            }
            Some(a) if a == "ok" => {
                for &t in &picked {
                    flow::draw(g, t, 1);
                }
                Ok(Step::Finish(format!("溃诛：令 {} 名角色各摸一张", picked.len())))
            }
            Some(o) => Err(format!("溃诛：未知选项 {o}")),
        };
    }
    // pick2：系统枚举「体力值之和 = X」的组合供选。
    let combos = kuizhu_combos(g, x);
    if combos.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "kuizhu") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("kuizhu");
            for (i, combo) in combos.iter().enumerate() {
                let names: Vec<String> = combo.iter().map(|t| format!("#{}", t + 1)).collect();
                b = b.option(format!("combo:{i}"), format!("对 {} 各造成 1 点伤害", names.join("、")));
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "pick2", "x": x }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("combo:") => {
            let i: usize = a[6..].parse().map_err(|_| "溃诛：非法组合")?;
            let combo = combos.get(i).ok_or("溃诛：非法组合")?.clone();
            for t in combo {
                let spec = DamageSpec {
                    from: Some(actor),
                    target: t,
                    amount: 1,
                    kind: DamageKind::Normal,
                    source: SourceKind::Skill,
                };
                let armor = gear::armor_of(g, t);
                flow::damage(g, spec, armor, 0);
            }
            Ok(Step::Finish("溃诛：对所选角色各造成 1 点伤害".into()))
        }
        Some(o) => Err(format!("溃诛：未知选项 {o}")),
    }
}

/// 溃诛②：枚举存活角色中「体力值之和 = X」的组合（≤8 人，上限 64 个供选）。
fn kuizhu_combos(g: &Game, x: i32) -> Vec<Vec<usize>> {
    let alive: Vec<usize> = (0..g.seats.len()).filter(|&t| g.seats[t].alive).collect();
    let mut out = Vec::new();
    if alive.len() > 8 {
        return out;
    }
    for mask in 1u32..(1 << alive.len()) {
        let mut picked = Vec::new();
        let mut sum = 0;
        for (i, &t) in alive.iter().enumerate() {
            if mask & (1 << i) != 0 {
                picked.push(t);
                sum += g.seats[t].hp;
            }
        }
        if sum == x {
            out.push(picked);
            if out.len() >= 64 {
                break;
            }
        }
    }
    out
}

fn chezheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    // ② 出牌阶段结束时：本阶段使用牌数（含装备与响应打出，Q17 裁定 B）< 攻击范围不含你的角色数。
    let used = g.counter(&turn_key(actor, "cards_used"));
    let outside: Vec<usize> = alive_others(g, actor)
        .into_iter()
        .filter(|&i| !in_attack_range(g, i, actor))
        .collect();
    if outside.is_empty() || used >= outside.len() as i32 {
        return Ok(Step::Done);
    }
    if step == "ask" {
        return match take_answer(g, "chezheng") {
            None => Ok(trigger_ask(
                actor,
                "chezheng",
                "掣政：弃置一名攻击范围不含你的角色一张牌",
                json!({ "step": "ask" }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(Stage::Target, json!({ "step": "target" }))),
            Some(o) => Err(format!("掣政：未知选项 {o}")),
        };
    }
    if step == "target" {
        return match take_answer(g, "chezheng") {
            None => {
                let mut b = PendingBuilder::new(actor, "target").skill("chezheng");
                for t in &outside {
                    b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选目标）")
                        .data(json!({ "step": "target" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("seat:") => {
                let t: usize = a[5..].parse().map_err(|_| "掣政：非法目标")?;
                if !outside.contains(&t) {
                    return Err("掣政：目标须是攻击范围不含你的角色".into());
                }
                Ok(Step::Next(
                    Stage::PickOther,
                    json!({ "step": "pick", "target": t }),
                ))
            }
            Some(o) => Err(format!("掣政：未知选项 {o}")),
        };
    }
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    let cands = target_any_cards(g, target);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "chezheng") {
        None => Ok(must_pick(
            g,
            actor,
            "chezheng",
            "弃置",
            &cands,
            json!({ "step": "pick", "target": target }),
        )),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("掣政：只能弃置目标区域里的一张牌".into());
            }
            flow::discard(g, actor, &id, "掣政");
            Ok(Step::Finish("掣政：弃其一张牌".into()))
        }
        Some(o) => Err(format!("掣政：未知选项 {o}")),
    }
}

fn lijun(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let lord = ask.actor;
    if !skills::active(g, lord, "lijun") {
        return Ok(Step::Done);
    }
    let Some(user) = seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    if user == lord || !g.seats.get(user).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    // 其他**吴势力**角色在其出牌阶段使用【杀】后。
    if g.seats[user].kingdom != "wu" {
        return Ok(Step::Done);
    }
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let is_sha = g
        .card_defs
        .get(&card)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.tags.sha)
        .unwrap_or(false);
    if !is_sha {
        return Ok(Step::Done);
    }
    if g.counter(&turn_key(user, "lijun")) > 0 {
        return Ok(Step::Done); // 每回合限一次
    }
    let step = ask
        .data
        .get("step")
        .and_then(|v| v.as_str())
        .unwrap_or("give");
    // 两个「可以」各自独立：是否交牌（吴角色）→ 是否令其摸 1 且杀次数 +1（孙亮）。
    if step == "give" {
        // 询问者 = 使用【杀】的吴角色（不是孙亮）。
        return match take_answer(g, "lijun") {
            None => {
                let mut b = PendingBuilder::new(user, "choose").skill("lijun");
                b = b.option("use", "立军：将此【杀】交给孙亮");
                Ok(Step::Need(
                    b.escape("不交")
                        .data(json!({ "step": "give", "by": user, "card": card }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "bonus", "by": user, "card": card }),
            )),
            Some(o) => Err(format!("立军：未知选项 {o}")),
        };
    }
    match take_answer(g, "lijun") {
        None => {
            let mut b = PendingBuilder::new(lord, "choose").skill("lijun");
            b = b.option("use", "立军：令其摸一张牌且本回合【杀】次数 +1");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "bonus", "by": user, "card": card }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" || a == "use" => {
            g.bump(&turn_key(user, "lijun"));
            flow::gain(g, lord, &card, user, "立军");
            let mut msg = String::from("立军：获得该【杀】");
            if a == "use" {
                flow::draw(g, user, 1);
                g.bump(&turn_key(user, "sha_limit"));
                msg.push_str("，令其摸一张且本回合【杀】次数 +1");
            }
            Ok(Step::Finish(msg))
        }
        Some(o) => Err(format!("立军：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 蒯越蒯良 · 荐降 / 审时（转换技）
// ---------------------------------------------------------------------------

fn jianxiang(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    if seat_of(ask, "by") == Some(actor) {
        return Ok(Step::Done); // 须是「其他角色使用牌」
    }
    // 手牌数最少的一名角色（并列时由蒯越蒯良选择，Q19 裁定 B）。
    let alive: Vec<usize> = (0..g.seats.len()).filter(|&i| g.seats[i].alive).collect();
    let min = alive.iter().map(|&i| g.hand_size(i)).min().unwrap_or(0);
    let cands: Vec<usize> = alive.into_iter().filter(|&i| g.hand_size(i) == min).collect();
    let Some(first) = cands.first().copied() else {
        return Ok(Step::Done);
    };
    if step == "ask" {
        return match take_answer(g, "jianxiang") {
            None => Ok(trigger_ask(
                actor,
                "jianxiang",
                "荐降：令手牌数最少的角色摸一张牌",
                json!({ "step": "ask" }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                if cands.len() == 1 {
                    flow::draw(g, first, 1);
                    Ok(Step::Finish("荐降：令手牌最少者摸一张".into()))
                } else {
                    Ok(Step::Next(Stage::Target, json!({ "step": "target" })))
                }
            }
            Some(o) => Err(format!("荐降：未知选项 {o}")),
        };
    }
    match take_answer(g, "jianxiang") {
        None => {
            let mut b = PendingBuilder::new(actor, "target").skill("jianxiang");
            for t in &cands {
                b = b.option(format!("seat:{t}"), format!("令 #{} 摸一张", t + 1));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选目标）")
                    .data(json!({ "step": "target" }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("seat:") => {
            let t: usize = a[5..].parse().map_err(|_| "荐降：非法目标")?;
            if !cands.contains(&t) {
                return Err("荐降：目标须为手牌数最少的角色".into());
            }
            flow::draw(g, t, 1);
            Ok(Step::Finish("荐降：令手牌最少者摸一张".into()))
        }
        Some(o) => Err(format!("荐降：未知选项 {o}")),
    }
}

fn shenshi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    match hook_of(ask) {
        "end" => return Ok(shenshi_end_check(g, ask)),
        "after_damage" => return shenshi_after_damage(g, ask),
        _ => {}
    }
    let face = ask.data.get("face").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if face == 1 {
        // 阴：其他角色对你造成伤害后（回合外）→ 观看其手牌并交给其一张牌。
        if g.turn == actor {
            return Ok(Step::Done); // 窗口 = 回合外
        }
        // 伤害事件的 `target` = 伤害来源（event_payload 口径）；`src` 为中间态键。
        let Some(src) = seat_of(ask, "target").or_else(|| seat_of(ask, "src")) else {
            return Ok(Step::Done);
        };
        if src == actor || !g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
            return Ok(Step::Done);
        }
        let own = own_cards(g, actor);
        if own.is_empty() {
            return Ok(Step::Done);
        }
        let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
        if step == "ask" {
            return match take_answer(g, "shenshi") {
                None => Ok(trigger_ask(
                    actor,
                    "shenshi",
                    "审时（阴）：交给其一张牌（结束阶段校验）",
                    json!({ "step": "ask", "face": 1, "target": src, "src": src }),
                )),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "give", "face": 1, "target": src, "src": src }),
                )),
                Some(o) => Err(format!("审时：未知选项 {o}")),
            };
        }
        return match take_answer(g, "shenshi") {
            None => {
                let mut b = PendingBuilder::new(actor, "pick_own").skill("shenshi");
                for id in &own {
                    b = b.option(format!("card:{id}"), format!("交给【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一张）")
                        .data(json!({ "step": "give", "face": 1, "target": src, "src": src }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("card:") => {
                let give = a[5..].to_string();
                if !own.contains(&give) {
                    return Err("审时（阴）：只能交出自己的手牌或装备牌".into());
                }
                flow::gain(g, src, &give, actor, "审时");
                // 当前回合结束阶段校验：由 `DeferredKind::TurnEndCheck` 在结束阶段派发（单一延迟队列）。
                g.deferred.push(super::super::deferred::Deferred {
                    kind: super::super::deferred::DeferredKind::TurnEndCheck,
                    payload: super::super::deferred::Payload::Ask {
                        skill: "shenshi".into(),
                        stage: Stage::Choose,
                        actor,
                        data: json!({ "hook": "end", "turn": g.turn, "src": src, "card": give }),
                    },
                });
                skills::flip_face(g, actor, "shenshi");
                Ok(Step::Finish("审时（阴）：已交给其一张牌并转换面".into()))
            }
            Some(o) => Err(format!("审时：未知选项 {o}")),
        }
    }
    // 阳：出牌阶段限一次，交给手牌最多的其他角色一张牌，并对其造成 1 点伤害。
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    match step {
        "choose" => {
            if g.phase != Phase::Play
                || g.counter(&phase_key(actor, "shenshi")) > 0
                || own_cards(g, actor).is_empty()
            {
                return Ok(Step::Done);
            }
            match take_answer(g, "shenshi") {
                None => Ok(choose_use(
                    actor,
                    "shenshi",
                    "审时（阳）：交给手牌最多的其他角色一张牌，并对其造成 1 点伤害",
                    json!({ "step": "choose", "face": 0 }),
                )),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "face": 0 }),
                )),
                Some(o) => Err(format!("审时：未知选项 {o}")),
            }
        }
        "target" => {
            let others = alive_others(g, actor);
            let max = others.iter().map(|&i| g.hand_size(i)).max().unwrap_or(0);
            let cands: Vec<usize> = others.into_iter().filter(|&i| g.hand_size(i) == max).collect();
            match take_answer(g, "shenshi") {
                None => {
                    if cands.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "target").skill("shenshi");
                    for t in &cands {
                        b = b.option(format!("seat:{t}"), format!("目标：#{}", t + 1));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选目标）")
                            .data(json!({ "step": "target", "face": 0 }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("seat:") => {
                    let t: usize = a[5..].parse().map_err(|_| "审时：非法目标")?;
                    if !cands.contains(&t) {
                        return Err("审时（阳）：目标须为手牌最多的其他角色".into());
                    }
                    Ok(Step::Next(
                        Stage::PickOwn,
                        json!({ "step": "give", "face": 0, "target": t }),
                    ))
                }
                Some(o) => Err(format!("审时：未知选项 {o}")),
            }
        }
        "give" => {
            let target = seat_of(ask, "target").ok_or("审时：缺少目标")?;
            let own = own_cards(g, actor);
            match take_answer(g, "shenshi") {
                None => {
                    if own.is_empty() {
                        return Ok(Step::Done);
                    }
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("shenshi");
                    for id in &own {
                        b = b.option(format!("card:{id}"), format!("交给【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_mandatory("（必选一张）")
                            .data(json!({ "step": "give", "face": 0, "target": target }))
                            .build(),
                    ))
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    shenshi_yang_finish(g, actor, target, &id)
                }
                Some(o) => Err(format!("审时：未知选项 {o}")),
            }
        }
        other => Err(format!("审时：未知阶段 {other}")),
    }
}

fn shenshi_yang_finish(
    g: &mut Game,
    actor: usize,
    target: usize,
    card: &str,
) -> Result<Step, String> {
    g.bump(&phase_key(actor, "shenshi"));
    flow::gain(g, target, card, actor, "审时");
    let spec = DamageSpec {
        from: Some(actor),
        target,
        amount: 1,
        kind: DamageKind::Normal,
        source: SourceKind::Skill,
    };
    let armor = gear::armor_of(g, target);
    flow::damage(g, spec, armor, 0);
    // 「若其因此死亡」→ 死亡结算（濒死流程/结算）之后再由延迟项询问「摸至 4 张」。
    g.deferred.push(super::super::deferred::Deferred::ask(
        super::super::deferred::DeferredKind::DeathCleanup,
        "shenshi",
        Stage::Choose,
        actor,
        json!({ "hook": "after_damage", "target": target }),
    ));
    skills::flip_face(g, actor, "shenshi");
    Ok(Step::Finish("审时（阳）：已交牌并造成 1 点伤害".into()))
}

/// 审时（阳）的死亡后续：目标确实死亡时，令一名角色将手牌摸至 4 张（可放弃）。
fn shenshi_after_damage(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let Some(target) = seat_of(ask, "target") else {
        return Ok(Step::Done);
    };
    if g.seats.get(target).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done); // 未死亡：不提供
    }
    match take_answer(g, "shenshi") {
        None => {
            let mut b = PendingBuilder::new(actor, "choose").skill("shenshi");
            for t in (0..g.seats.len()).filter(|&t| g.seats[t].alive) {
                b = b.option(format!("fill:{t}"), format!("令 #{} 将手牌摸至 4 张", t + 1));
            }
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "hook": "after_damage", "target": target }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("fill:") => {
            let t: usize = a[5..].parse().map_err(|_| "审时：非法目标")?;
            if !g.seats.get(t).map(|s| s.alive).unwrap_or(false) {
                return Err("审时：目标须存活".into());
            }
            flow::fill_hand(g, t, 4);
            Ok(Step::Finish("审时（阳）：令其手牌摸至 4 张".into()))
        }
        Some(o) => Err(format!("审时：未知选项 {o}")),
    }
}

/// 审时（阴）的**结束阶段校验**：当前回合结束阶段，若其未失去该牌 → 将自己的手牌摸至 4 张。
/// `DeferredKind::TurnEndCheck` 只在当前回合结束阶段派发（未到则保留在队列）。
pub fn shenshi_end_check(g: &mut Game, ask: &Ask) -> Step {
    let seat = ask.actor;
    let Some(turn) = ask.data.get("turn").and_then(|v| v.as_u64()).map(|v| v as usize) else {
        return Step::Done;
    };
    let Some(src) = ask.data.get("src").and_then(|v| v.as_u64()).map(|v| v as usize) else {
        return Step::Done;
    };
    let Some(card) = ask.data.get("card").and_then(|v| v.as_str()) else {
        return Step::Done;
    };
    if g.phase != Phase::End || g.turn != turn {
        return Step::Done;
    }
    let still_holds = matches!(g.cards.get(card), Some(super::super::Zone::Hand(s)) if *s == src);
    if still_holds {
        flow::fill_hand(g, seat, 4);
        return Step::Finish("审时（阴）：其未失去此牌，你摸至 4 张".into());
    }
    Step::Done
}
fn chenglue(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    if g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    let face = ask
        .data
        .get("face")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| skills::face_of(g, actor, "chenglue") as u64) as usize;
    let discard_n = if face == 0 { 2 } else { 1 };
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("choose");
    match step {
        "choose" => {
            if g.counter(&phase_key(actor, "chenglue")) > 0 || g.hand(actor).is_empty() {
                return Ok(Step::Done);
            }
            match take_answer(g, "chenglue") {
                None => {
                    let label = if face == 0 {
                        "成略（阳）：摸 1 张并弃 2 张手牌"
                    } else {
                        "成略（阴）：摸 2 张并弃 1 张手牌"
                    };
                    Ok(choose_use(
                        actor,
                        "chenglue",
                        label,
                        json!({ "step": "choose", "face": face }),
                    ))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "use" => Ok(Step::Next(
                    Stage::PickOwn,
                    json!({ "step": "discard", "face": face, "picked": [] }),
                )),
                Some(o) => Err(format!("成略：未知选项 {o}")),
            }
        }
        "discard" => {
            let mut picked: Vec<String> = ask
                .data
                .get("picked")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let hand = g.hand(actor);
            match take_answer(g, "chenglue") {
                None => {
                    let mut b = PendingBuilder::new(actor, "pick_own").skill("chenglue");
                    for id in hand.iter().filter(|id| !picked.contains(id)) {
                        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_label(g, id)));
                    }
                    Ok(Step::Need(
                        b.escape_with("confirm", "完成弃牌")
                            .data(json!({ "step": "discard", "face": face, "picked": picked }))
                            .build(),
                    ))
                }
                Some(a) if a == "confirm" => {
                    if picked.len() != discard_n {
                        return Err(format!("成略：须弃置 {discard_n} 张手牌"));
                    }
                    chenglue_finish(g, actor, face, &picked)
                }
                Some(a) if a.starts_with("card:") => {
                    let id = a[5..].to_string();
                    if !hand.contains(&id) {
                        return Err("成略：只能弃置手牌".into());
                    }
                    if picked.contains(&id) {
                        return Err("成略：该牌已选".into());
                    }
                    picked.push(id);
                    if picked.len() == discard_n {
                        chenglue_finish(g, actor, face, &picked)
                    } else {
                        Ok(Step::Next(
                            Stage::PickOwn,
                            json!({ "step": "discard", "face": face, "picked": picked }),
                        ))
                    }
                }
                Some(o) => Err(format!("成略：未知选项 {o}")),
            }
        }
        other => Err(format!("成略：未知阶段 {other}")),
    }
}

fn chenglue_finish(
    g: &mut Game,
    actor: usize,
    face: usize,
    picked: &[String],
) -> Result<Step, String> {
    let draw_n = if face == 0 { 1 } else { 2 };
    g.bump(&phase_key(actor, "chenglue"));
    flow::draw(g, actor, draw_n);
    for id in picked {
        if !matches!(g.cards.get(id), Some(super::super::Zone::Hand(s)) if *s == actor) {
            return Err("成略：只能弃置手牌".into());
        }
        let suit = g.card_defs.get(id).map(|c| c.suit);
        flow::discard(g, actor, id, "成略");
        // 「本阶段使用与弃置牌花色相同的牌无距离和次数限制」（可多个花色）。
        if let Some(s) = suit {
            g.set_mark(&format!("free.{actor}.{s:?}"), format!("{}:{:?}", g.turn, g.phase));
        }
    }
    skills::flip_face(g, actor, "chenglue");
    Ok(Step::Finish(
        if face == 0 {
            "成略（阳）：摸 1 弃 2"
        } else {
            "成略（阴）：摸 2 弃 1"
        }
        .into(),
    ))
}
fn shicai(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let actor = ask.actor;
    let step = ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask");
    let Some(card) = card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let Some(def) = g.card_defs.get(&card).and_then(|c| cards::def(&c.def)) else {
        return Ok(Step::Done);
    };
    // 每回合分别对基本牌、锦囊牌两类各限一次；装备牌不触发。
    let kind = match def.category {
        Category::Basic => "basic",
        Category::Trick => "trick",
        _ => return Ok(Step::Done),
    };
    let used_key = turn_key(actor, &format!("shicai.{kind}"));
    if g.counter(&used_key) > 0 {
        return Ok(Step::Done);
    }
    if step == "ask" {
        return match take_answer(g, "shicai") {
            None => Ok(trigger_ask(
                actor,
                "shicai",
                "恃才：将此牌置于牌堆顶，然后摸一张牌",
                json!({ "step": "ask", "card": card, "kind": kind }),
            )),
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a == "use" => {
                flow::to_deck_top(g, &card);
                flow::draw(g, actor, 1);
                g.bump(&used_key);
                Ok(Step::Finish("恃才：置顶后摸一张".into()))
            }
            Some(o) => Err(format!("恃才：未知选项 {o}")),
        };
    }
    Err("恃才：未知阶段".into())
}

// ---------------------------------------------------------------------------
// 供测试直接构造 handler 上下文（绕过 Ask 结构体展开）
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub fn ask(skill: &str, actor: usize, data: Value) -> Ask {
    Ask {
        skill: skill.to_string(),
        stage: crate::kernel::resolve::Stage::Choose,
        actor,
        data,
    }
}

#[allow(dead_code)]
pub fn data(pairs: &[(&str, Value)]) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

/// 空数据（无参 handler）。
#[allow(dead_code)]
pub fn empty() -> Value {
    json!({})
}
