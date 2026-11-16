//! B13 技能（界限突破·J1：8 将 / 16 技能 + 授予技 荐言/思蜀）。
//!
//! 依据 `docs/games/sanguosha/align/B13.json` 的 `agreed`（权威）与
//! `memory_palace/decisions/ADR-0044-b13-rulings.md`（化身池 / 蛊惑 / 缠怨口径）。
//!
//! 本文件按「一批一文件」推进；`SKILLS` 只声明**已实现并收口**的技能（门禁 `check:sgs3-registry`
//! 会报告 `B13 进行中：已注册 x/16`）。

use serde_json::json;

use super::super::event::{EventKind, Subject, Window};
use super::super::flow;
use super::super::gear;
use super::super::pending::PendingBuilder;
use super::super::resolve::{take_answer, Ask, Stage, Step};
use super::super::{CardId, DamageKind, DamageSpec, Game, Phase, SourceKind, Zone};
use super::common::{self, card_label, own_cards, phase_key, seat_name, turn_key};
use super::{Face, Hook, SkillDef, SkillKind, Subscription};
use crate::kernel::skills;

pub static SKILLS: &[SkillDef] = &[
    // —— 界左慈 · 化身 / 新生 ——
    SkillDef { code: "huashen", name: "化身", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[
            // 「回合开始时」/「回合结束后」：内核 `turn::{begin_turn,end_turn}` 的唯一事件点。
            Subscription { event: EventKind::TurnStart, subject: Subject::Any },
            Subscription { event: EventKind::TurnEnd, subject: Subject::Any },
        ], hooks: &[], can_active: false }] },
    SkillDef { code: "xinsheng", name: "新生", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ },
        ], hooks: &[], can_active: false }] },
    // —— 界黄忠 / 界徐庶 / 界华雄 ——
    SkillDef { code: "liegong", name: "烈弓", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaIntercept], can_active: false }] },
    SkillDef { code: "zhuhai", name: "诛害", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OtherPhase(Phase::End), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "qianxin", name: "潜心", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::By }], hooks: &[], can_active: false }] },
    SkillDef { code: "jianyan", name: "荐言", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "yaowu", name: "耀武", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::DamageAfter, subject: Subject::Self_ }], hooks: &[], can_active: false }] },
    SkillDef { code: "shizhan", name: "势斩", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "huangtian", name: "黄天", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[], can_active: false }] },
    // —— 界袁绍 / 界刘禅 ——
    SkillDef { code: "luanji", name: "乱击", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    SkillDef { code: "xueyi", name: "血裔", kind: SkillKind::Lord, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[Hook::GameStart], can_active: false }] },
    SkillDef { code: "xiangle", name: "享乐", kind: SkillKind::Locked, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::ShaIntercept], can_active: false }] },
    SkillDef { code: "fangquan", name: "放权", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnTurn, subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: true }] },
    SkillDef { code: "ruoyu", name: "若愚", kind: SkillKind::Awaken, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Prepare), subscriptions: &[Subscription { event: EventKind::PhaseStart, subject: Subject::Any }], hooks: &[], can_active: false }] },
    SkillDef { code: "sishu", name: "思蜀", kind: SkillKind::Normal, grant_only: true, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界于吉 · 蛊惑 ——
    SkillDef { code: "guhuo", name: "蛊惑", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::OwnPhase(Phase::Play), subscriptions: &[], hooks: &[], can_active: true }] },
    // —— 界张角 · 鬼道 / 雷击 ——
    SkillDef { code: "guidao", name: "鬼道", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[], hooks: &[Hook::JudgeRewrite], can_active: false }] },
    SkillDef { code: "leiji", name: "雷击", kind: SkillKind::Normal, grant_only: false, transform: false,
        faces: &[Face { window: Window::AnyTime, subscriptions: &[
            Subscription { event: EventKind::CardUsed, subject: Subject::Self_ },
            Subscription { event: EventKind::CardPlayed, subject: Subject::Self_ },
        ], hooks: &[], can_active: false }] },
];

pub fn handler(code: &str) -> Option<fn(&mut Game, &Ask) -> Result<Step, String>> {
    Some(match code {
        "huashen" => huashen,
        "xinsheng" => xinsheng,
        "guidao" => guidao,
        "leiji" => leiji,
        "guhuo" => guhuo,
        "liegong" => liegong,
        "zhuhai" => zhuhai,
        "qianxin" => qianxin,
        "jianyan" => jianyan,
        "yaowu" => yaowu,
        "shizhan" => shizhan,
        "huangtian" => huangtian,
        "luanji" => luanji,
        "xueyi" => xueyi,
        "xiangle" => xiangle,
        "fangquan" => fangquan,
        "ruoyu" => ruoyu,
        "sishu" => sishu,
        _ => return None,
    })
}

pub fn availability(code: &str) -> Option<fn(&Game, usize) -> bool> {
    Some(match code {
        "guhuo" => guhuo_available,
        "jianyan" => jianyan_available,
        "shizhan" => shizhan_available,
        "luanji" => luanji_available,
        "fangquan" => fangquan_available,
        "sishu" => sishu_available,
        _ => return None,
    })
}

fn guhuo_available(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play && guhuo_ready(g, seat)
}

fn step_of(ask: &Ask) -> &str {
    ask.data.get("step").and_then(|v| v.as_str()).unwrap_or("ask")
}

fn pool(g: &Game, seat: usize) -> Vec<String> {
    g.huashen.get(&seat).map(|h| h.pool.clone()).unwrap_or_default()
}

fn current(g: &Game, seat: usize) -> Option<String> {
    g.huashen.get(&seat).and_then(|h| h.current.clone())
}

/// 非展示的化身牌（切换/2 换 1 的候选）。
fn bench(g: &Game, seat: usize) -> Vec<String> {
    let cur = current(g, seat);
    pool(g, seat)
        .into_iter()
        .filter(|c| Some(c.clone()) != cur)
        .collect()
}

/// 授予某个化身牌的技能（授予技形式：`skills::grant`；不依赖技能实现）。
/// 已拥有的技能先撤销，再授予新技能（`Huashen.active_skill` 是唯一记录）。
fn grant_from(g: &mut Game, seat: usize, general: &str, code: &str) {
    if let Some(old) = g.huashen.get(&seat).and_then(|h| h.active_skill.clone()) {
        skills::revoke(g, seat, &old);
    }
    skills::grant(g, seat, code);
    if let Some(h) = g.huashen.get_mut(&seat) {
        h.active_skill = Some(code.to_string());
    }
    let name = skills::def(code).map(|d| d.name).unwrap_or(code);
    let _ = general;
    let _ = name;
}

fn revoke_current(g: &mut Game, seat: usize) {
    if let Some(old) = g.huashen.get(&seat).and_then(|h| h.active_skill.clone()) {
        skills::revoke(g, seat, &old);
        if let Some(h) = g.huashen.get_mut(&seat) {
            h.active_skill = None;
        }
    }
}

// ---------------------------------------------------------------------------
// 界左慈 · 化身
// ---------------------------------------------------------------------------

fn huashen(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "huashen") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    match step {
        // ① 选择本次做哪一项（替换展示 / 2 换 1）。
        "ask" => {
            let card_slot = common::seat_of(ask, "slot").unwrap_or(0);
            let bench = bench(g, owner);
            return match take_answer(g, "huashen") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("huashen");
                    if !bench.is_empty() {
                        b = b.option("swap", "替换展示的化身牌");
                    }
                    if !bench.is_empty() {
                        b = b.option("trade", "移去至多两张非展示化身牌，换一张新化身牌");
                    }
                    let data = json!({ "step": "ask", "slot": card_slot });
                    Ok(Step::Need(b.escape("不发动").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "swap" => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "swap_pick", "slot": card_slot }),
                )),
                Some(a) if a == "trade" => Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "trade_pick", "slot": card_slot, "picked": [] }),
                )),
                Some(o) => Err(format!("化身：未知选项 {o}")),
            };
        }
        // ② 替换展示的化身牌 → 再从该牌上选一个技能。
        "swap_pick" => {
            let slot = common::seat_of(ask, "slot").unwrap_or(0);
            let bench = bench(g, owner);
            if bench.is_empty() {
                return Ok(Step::Finish("化身：无可替换的化身牌".into()));
            }
            return match take_answer(g, "huashen") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("huashen");
                    for code in &bench {
                        let name = crate::kernel::generals::def(code)
                            .map(|d| d.name)
                            .unwrap_or(code);
                        b = b.option(format!("c:{code}"), format!("展示「{name}」"));
                    }
                    let data = json!({ "step": "swap_pick", "slot": slot });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => {
                    let code = a[2..].to_string();
                    if !bench.contains(&code) {
                        return Err("化身：该牌不在非展示化身牌中".into());
                    }
                    revoke_current(g, owner);
                    g.huashen_set_current(owner, Some(&code));
                    let choices = skills::huashen_choices(g, owner, &code);
                    match choices.len() {
                        0 => Ok(Step::Finish("化身：展示新化身（无可授予技能）".into())),
                        1 => {
                            grant_from(g, owner, &code, choices[0]);
                            Ok(Step::Finish("化身：展示并取得技能".into()))
                        }
                        _ => Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "skill_pick", "slot": slot, "general": code }),
                        )),
                    }
                }
                Some(o) => Err(format!("化身：未知选项 {o}")),
            };
        }
        // ③ 选择该化身牌上要取得的技能（多技能武将）。
        "skill_pick" => {
            let general = ask
                .data
                .get("general")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let choices = skills::huashen_choices(g, owner, &general);
            if choices.is_empty() {
                return Ok(Step::Finish("化身：无可授予技能".into()));
            }
            return match take_answer(g, "huashen") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("huashen");
                    for code in &choices {
                        let name = skills::def(code).map(|d| d.name).unwrap_or(code);
                        b = b.option(format!("s:{code}"), format!("取得「{name}」"));
                    }
                    let data = json!({ "step": "skill_pick", "general": general });
                    Ok(Step::Need(b.escape_mandatory("（必选一个技能）").data(data).build()))
                }
                Some(a) if a.starts_with("s:") => {
                    let code = a[2..].to_string();
                    if !choices.iter().any(|c| *c == code) {
                        return Err("化身：该技能不可取得".into());
                    }
                    grant_from(g, owner, &general, &code);
                    Ok(Step::Finish("化身：取得技能".into()))
                }
                Some(o) => Err(format!("化身：未知选项 {o}")),
            };
        }
        // ④ 2 换 1：移去至多两张非展示化身牌 → 获得一张新化身牌。
        "trade_pick" => {
            let picked = common::cards_of(ask, "picked");
            let bench = bench(g, owner);
            let cands: Vec<String> = bench
                .iter()
                .filter(|c| !picked.contains(c))
                .cloned()
                .collect();
            return match take_answer(g, "huashen") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("huashen");
                    for code in &cands {
                        let name = crate::kernel::generals::def(code)
                            .map(|d| d.name)
                            .unwrap_or(code);
                        b = b.option(format!("x:{code}"), format!("移去「{name}」"));
                    }
                    if !picked.is_empty() {
                        b = b.option("confirm", "确认（换一张新化身牌）");
                    }
                    let data = json!({ "step": "trade_pick", "picked": picked });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "confirm" => {
                    if picked.is_empty() {
                        return Err("化身：至少移去一张".into());
                    }
                    if let Some(h) = g.huashen.get_mut(&owner) {
                        h.pool.retain(|c| !picked.contains(c));
                    }
                    // Q9：本次换入的新牌**一定不**在刚移去的两张里（下次换取才可再随机到）。
                    let mut fresh: Vec<String> = skills::huashen_generals(g, owner)
                        .into_iter()
                        .filter(|c| !picked.contains(c))
                        .collect();
                    if let Some(code) = if fresh.is_empty() {
                        None
                    } else {
                        let i = g.rand_below(fresh.len());
                        Some(fresh.remove(i))
                    } {
                        if let Some(h) = g.huashen.get_mut(&owner) {
                            h.pool.push(code.clone());
                        }
                        let name = crate::kernel::generals::def(&code)
                            .map(|d| d.name)
                            .unwrap_or(code.as_str());
                        return Ok(Step::Finish(format!("化身：移去 {} 张，获得新化身「{name}」", picked.len())));
                    }
                    Ok(Step::Finish("化身：池空，未获得新化身".into()))
                }
                Some(a) if a.starts_with("x:") => {
                    let code = a[2..].to_string();
                    if !cands.contains(&code) {
                        return Err("化身：该牌不可移去".into());
                    }
                    if picked.len() >= 2 {
                        return Err("化身：至多移去两张".into());
                    }
                    let mut np = picked;
                    np.push(code);
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "trade_pick", "picked": np }),
                    ))
                }
                Some(o) => Err(format!("化身：未知选项 {o}")),
            };
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 界左慈 · 新生（每受到 1 点伤害：获得一张新化身牌）
// ---------------------------------------------------------------------------

fn xinsheng(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "xinsheng") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    let left = if step == "more" {
        ask.data.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as usize
    } else {
        let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        amount
    };
    if left == 0 {
        return Ok(Step::Finish("新生：结算完成".into()));
    }
    let mut fresh = skills::huashen_generals(g, owner);
    if fresh.is_empty() {
        return Ok(Step::Finish("新生：池空，不触发".into()));
    }
    match take_answer(g, "xinsheng") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("xinsheng");
            b = b.option("use", format!("获得一张新化身牌（剩 {left}）"));
            let data = json!({ "step": "ask", "left": left });
            Ok(Step::Need(b.escape("不发动").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("新生：放弃剩余".into())),
        Some(a) if a == "use" => {
            let i = g.rand_below(fresh.len());
            let code = fresh.remove(i);
            if let Some(h) = g.huashen.get_mut(&owner) {
                h.pool.push(code.clone());
            }
            let name = crate::kernel::generals::def(&code)
                .map(|d| d.name)
                .unwrap_or(code.as_str());
            if left > 1 {
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "more", "left": left - 1 }),
                ));
            }
            Ok(Step::Finish(format!("新生：获得新化身「{name}」")))
        }
        Some(o) => Err(format!("新生：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界张角 · 鬼道（判定牌生效前：用**黑色**牌代替；黑桃 2~9 摸一张）
// ---------------------------------------------------------------------------

/// 鬼道可用替代牌：黑色牌（手牌 + 装备区）。
pub fn guidao_cards(g: &Game, seat: usize) -> Vec<CardId> {
    own_cards(g, seat)
        .into_iter()
        .filter(|id| g.card_defs.get(id).map(|c| !c.suit.is_red()).unwrap_or(false))
        .collect()
}

fn guidao(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "guidao") {
        return Ok(Step::Done);
    }
    let judge_seat = common::seat_of(ask, "judge_seat");
    let Some(judge_card) = ask.data.get("card").and_then(|v| v.as_str()).map(str::to_string) else {
        return Ok(Step::Done);
    };
    let cands = guidao_cards(g, owner);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    let data = json!({ "step": "ask", "judge_seat": judge_seat, "card": judge_card });
    match take_answer(g, "guidao") {
        None => {
            let mut b = PendingBuilder::new(owner, "pick_own").skill("guidao");
            for id in &cands {
                b = b.option(
                    format!("card:{id}"),
                    format!("用黑色【{}】代替判定牌", card_label(g, id)),
                );
            }
            Ok(Step::Need(b.escape("不改判").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("card:") => {
            let id = a[5..].to_string();
            if !cands.contains(&id) {
                return Err("鬼道：只能用黑色手牌/装备区的牌改判".into());
            }
            if matches!(g.cards.get(&id), Some(Zone::Equip(..))) {
                flow::leave_equip(g, owner, &id, "鬼道");
            }
            let (suit, rank) = g
                .card_defs
                .get(&id)
                .map(|c| (c.suit, c.rank))
                .ok_or("鬼道：承载牌无定义")?;
            g.cards.insert(id.clone(), Zone::Processing);
            g.judge_rewrite = Some(id.clone());
            // 黑桃 2~9 → 立即摸一张。
            if suit == super::super::Suit::Spade && (2..=9).contains(&rank) {
                flow::draw(g, owner, 1);
            }
            // 改的是自己的判定 → 判定生效后可发动界雷击的「判定后」效果。
            if judge_seat == Some(owner) {
                g.bump(&turn_key(owner, "guidao_leiji"));
            }
            Ok(Step::Finish(format!("鬼道：用黑色【{}】改判", card_label(g, &id))))
        }
        Some(o) => Err(format!("鬼道：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界张角 · 雷击（使用/打出【闪】或使用【闪电】→ 判定后结算）
// ---------------------------------------------------------------------------

fn suit_of(ask: &Ask) -> String {
    ask.data
        .get("suit")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// 判定生效后统一钩子（`quick_judge` 唯一调用点）：鬼道改判自己的判定 → 询问雷击。
pub fn on_judge_effective(g: &mut Game, judge_seat: usize) -> bool {
    if g.counter(&turn_key(judge_seat, "guidao_leiji")) <= 0 {
        return false;
    }
    if !skills::active(g, judge_seat, "leiji") || !skills::enabled("leiji") {
        return false;
    }
    g.set_counter(&turn_key(judge_seat, "guidao_leiji"), 0);
    let (suit, rank) = g
        .judge_result
        .as_ref()
        .map(|r| (format!("{:?}", r.suit), r.rank))
        .unwrap_or_default();
    super::super::resolve::push_ask(
        g,
        "leiji",
        Stage::Choose,
        judge_seat,
        json!({ "step": "after", "suit": suit, "rank": rank }),
    );
    true
}

fn leiji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "leiji") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "target" {
        let amount = ask.data.get("amount").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let cands: Vec<usize> = (0..g.seats.len())
            .filter(|&s| g.seats[s].alive)
            .collect();
        return match take_answer(g, "leiji") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("leiji");
                for t in &cands {
                    b = b.option(
                        format!("t:{t}"),
                        format!("对 {} 造成 {amount} 点雷电伤害", seat_name(g, *t)),
                    );
                }
                let data = json!({ "step": "target", "amount": amount });
                Ok(Step::Need(b.escape("不造成伤害").data(data).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Finish("雷击：不造成伤害".into())),
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "雷击：非法目标")?;
                if !cands.contains(&t) {
                    return Err("雷击：目标非法".into());
                }
                let spec = DamageSpec {
                    from: Some(owner),
                    target: t,
                    amount,
                    kind: DamageKind::Thunder,
                    source: SourceKind::Skill,
                };
                let armor = gear::armor_of(g, t);
                let _ = flow::damage(g, spec, armor, 0);
                Ok(Step::Finish("雷击：造成雷电伤害".into()))
            }
            Some(o) => Err(format!("雷击：未知选项 {o}")),
        };
    }
    // `result` = quick_judge 续接（结果已注入 data）；`resolve` = 鬼道联动的判定后效果。
    if step == "resolve" || step == "result" {
        let suit = suit_of(ask);
        return match suit.as_str() {
            "Spade" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "target", "amount": 2 }),
            )),
            "Club" => {
                flow::heal(g, owner, 1, "雷击");
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "amount": 1 }),
                ))
            }
            _ => Ok(Step::Finish("雷击：判定花色无效果".into())),
        };
    }
    // ask（事件触发）/ after（鬼道联动）：询问是否发动。
    match take_answer(g, "leiji") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("leiji");
            b = b.option("use", "发动雷击（判定）");
            let mut data = json!({ "step": step });
            if let Some(v) = ask.data.get("suit") {
                data["suit"] = v.clone();
            }
            if let Some(v) = ask.data.get("rank") {
                data["rank"] = v.clone();
            }
            Ok(Step::Need(b.escape("不发动").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Finish("雷击：不发动".into())),
        Some(a) if a == "use" => {
            if step == "after" {
                let suit = suit_of(ask);
                return Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "resolve", "suit": suit }),
                ));
            }
            super::super::quick_judge::start(
                g,
                owner,
                "雷击",
                Some(("leiji", owner, json!({ "step": "resolve" }))),
            );
            Ok(Step::Done)
        }
        Some(o) => Err(format!("雷击：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界于吉 · 蛊惑（hidden-declaration）
//
// Q5 裁定：可声明【无懈可击】；影响链 = 出牌阶段 / 【杀】【闪】响应 / 濒死【桃】/【酒】自救 / 无懈窗口。
// 蛊惑作为【无懈可击】时**不再开无懈窗口**（由 `card_play::step_wuxie` 的 `alt` 分支直接收口）。
// ---------------------------------------------------------------------------

fn guhuo_ready(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "guhuo")
        && skills::enabled("guhuo")
        && g.counter(&turn_key(seat, "guhuo_used")) == 0
        && !g.hand(seat).is_empty()
}

/// 出牌阶段可声明的牌定义：基本牌 / 普通锦囊，排除延时锦囊、【无懈可击】（无主动使用）与【闪】。
fn declare_defs(g: &Game, seat: usize) -> Vec<&'static super::super::cards::CardDef> {
    let mut out = Vec::new();
    for d in crate::cards_generated::DEFS.iter() {
        if d.category != super::super::cards::Category::Basic
            && d.category != super::super::cards::Category::Trick
        {
            continue;
        }
        if d.tags.delayed || d.tags.wuxie || d.tags.shan {
            continue;
        }
        // legal ≡ execute：没有合法目标的牌不出现在声明列表里。
        let needs_target = !matches!(
            d.target_mode,
            super::super::cards::TargetMode::None | super::super::cards::TargetMode::SelfOnly
        );
        if needs_target && super::super::card_play::legal_targets(g, seat, d).is_empty() {
            continue;
        }
        out.push(d);
    }
    out
}

/// 响应窗口的声明牌（按需要的响应种类）。
fn declared_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "sha" => Some("sha"),
        "shan" => Some("shan"),
        "wuxie" => Some("wuxie_keji"),
        _ => None,
    }
}

/// 蛊惑在响应窗口的替代（杀/闪/无懈）。
pub fn response_alternatives(
    g: &Game,
    seat: usize,
    kind: super::super::cards::ResponseKind,
    _ctx: &str,
) -> Vec<(&'static str, String)> {
    use super::super::cards::ResponseKind;
    let k = match kind {
        ResponseKind::Sha => "sha",
        ResponseKind::Shan => "shan",
        ResponseKind::Wuxie => "wuxie",
        _ => return Vec::new(),
    };
    if !guhuo_ready(g, seat) {
        return Vec::new();
    }
    let name = match k {
        "sha" => "【杀】",
        "shan" => "【闪】",
        _ => "【无懈可击】",
    };
    vec![("guhuo", format!("蛊惑：扣置一张手牌当{name}"))]
}

/// 濒死救援的蛊惑声明（他人=【桃】；自己=【酒】）。
pub fn dying_declared(asker: usize, target: usize) -> &'static str {
    if asker == target {
        "jiu"
    } else {
        "tao"
    }
}

fn carrier_pending(g: &Game, actor: usize, skill: &'static str, data: serde_json::Value) -> Step {
    let mut b = PendingBuilder::new(actor, "pick_own").skill(skill);
    for id in g.hand(actor) {
        b = b.option(format!("c:{id}"), format!("扣置【{}】", card_label(g, &id)));
    }
    Step::Need(b.escape("取消").data(data).build())
}

fn guhuo(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "guhuo") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    match step {
        // —— 响应窗口（杀/闪/无懈）——
        "alt" => {
            if !guhuo_ready(g, owner) {
                return Ok(Step::Done);
            }
            let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let Some(declared) = declared_of_kind(kind) else {
                return Ok(Step::Done);
            };
            let mut extra = json!({ "step": "alt", "kind": kind, "declared": declared });
            if let Some(c) = ask.data.get("card") {
                extra["card"] = c.clone();
            }
            match take_answer(g, "guhuo") {
                None => Ok(carrier_pending(g, owner, "guhuo", extra)),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => {
                    let mut data = extra.clone();
                    data["step"] = json!("alt_go");
                    data["c"] = json!(a[2..].to_string());
                    Ok(Step::Next(Stage::Choose, data))
                }
                Some(o) => Err(format!("蛊惑：未知选项 {o}")),
            }
        }
        "alt_go" => {
            let carrier = common::card_of(ask, "c").ok_or("蛊惑：缺扣置牌")?;
            let declared = ask
                .data
                .get("declared")
                .and_then(|v| v.as_str())
                .unwrap_or("sha")
                .to_string();
            let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
            g.bump(&turn_key(owner, "guhuo_used"));
            super::super::guhuo::start(g, owner, &carrier, &declared, "蛊惑")?;
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "alt_wait", "kind": kind }),
            ))
        }
        "alt_wait" => {
            let truth = g.guhuo_result.take().map(|r| r.truth).unwrap_or(false);
            if truth {
                let kind = ask.data.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                // 无懈窗口的结果通道用 `g.seats.len()`（与 step_wuxie 的 alt 分支同口径）。
                let key = if kind == "wuxie" { g.seats.len() } else { owner };
                g.set_mark(&format!("response.sub.{key}"), "1");
            }
            Ok(Step::Finish(if truth {
                "蛊惑：为真".into()
            } else {
                "蛊惑：为假，此牌作废".into()
            }))
        }
        // —— 濒死救援 ——
        "dying_pick" => {
            if !guhuo_ready(g, owner) {
                return Ok(Step::Done);
            }
            let target = common::seat_of(ask, "target").unwrap_or(owner);
            let declared = dying_declared(owner, target);
            let data = json!({ "step": "dying_pick", "target": target, "declared": declared });
            match take_answer(g, "guhuo") {
                None => Ok(carrier_pending(g, owner, "guhuo", data)),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => {
                    let mut d2 = data.clone();
                    d2["step"] = json!("dying_go");
                    d2["c"] = json!(a[2..].to_string());
                    Ok(Step::Next(Stage::Choose, d2))
                }
                Some(o) => Err(format!("蛊惑：未知选项 {o}")),
            }
        }
        "dying_go" => {
            let carrier = common::card_of(ask, "c").ok_or("蛊惑：缺扣置牌")?;
            let declared = ask
                .data
                .get("declared")
                .and_then(|v| v.as_str())
                .unwrap_or("tao")
                .to_string();
            g.bump(&turn_key(owner, "guhuo_used"));
            super::super::guhuo::start(g, owner, &carrier, &declared, "蛊惑")?;
            Ok(Step::Next(Stage::Choose, json!({ "step": "dying_wait" })))
        }
        "dying_wait" => {
            let truth = g.guhuo_result.take().map(|r| r.truth).unwrap_or(false);
            if truth {
                g.set_mark(&format!("dying.guhuo.{owner}"), "ok");
            }
            Ok(Step::Finish("蛊惑：濒死救援结算".into()))
        }
        // —— 出牌阶段主动使用 ——
        "choose" => {
            if !guhuo_ready(g, owner) {
                return Ok(Step::Done);
            }
            match take_answer(g, "guhuo") {
                None => Ok(carrier_pending(g, owner, "guhuo", json!({ "step": "choose" }))),
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("c:") => Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "declare_pick", "c": a[2..].to_string() }),
                )),
                Some(o) => Err(format!("蛊惑：未知选项 {o}")),
            }
        }
        "declare_pick" => {
            let carrier = common::card_of(ask, "c").ok_or("蛊惑：缺扣置牌")?;
            if !matches!(g.cards.get(&carrier), Some(Zone::Hand(s)) if *s == owner) {
                return Err("蛊惑：扣置牌必须是自己手牌".into());
            }
            let defs = declare_defs(g, owner);
            if defs.is_empty() {
                return Ok(Step::Finish("蛊惑：无合法声明".into()));
            }
            match take_answer(g, "guhuo") {
                None => {
                    let mut b = PendingBuilder::new(owner, "choose").skill("guhuo");
                    for d in &defs {
                        b = b.option(format!("d:{}", d.code), format!("声明为【{}】", d.name));
                    }
                    let data = json!({ "step": "declare_pick", "c": carrier });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a.starts_with("d:") => {
                    let declared = a[2..].to_string();
                    if !defs.iter().any(|d| d.code == declared) {
                        return Err("蛊惑：该声明不可用".into());
                    }
                    let def = super::super::cards::def(&declared).ok_or("蛊惑：未知牌名")?;
                    let need = !matches!(
                        def.target_mode,
                        super::super::cards::TargetMode::None
                            | super::super::cards::TargetMode::SelfOnly
                    );
                    if !need {
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "use_go", "c": carrier, "declared": declared, "targets": [] }),
                        ));
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "target_pick", "c": carrier, "declared": declared, "picked": [] }),
                    ))
                }
                Some(o) => Err(format!("蛊惑：未知选项 {o}")),
            }
        }
        "target_pick" => {
            let carrier = common::card_of(ask, "c").unwrap_or_default();
            let declared = ask
                .data
                .get("declared")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let picked = common::seats_of(ask, "picked");
            let def = super::super::cards::def(&declared).ok_or("蛊惑：未知牌名")?;
            let legal: Vec<usize> = super::super::card_play::legal_targets(g, owner, def)
                .into_iter()
                .filter(|t| !picked.contains(t))
                .collect();
            let (lo, hi) = match def.effect {
                super::super::cards::Effect::Collateral => (2usize, 2usize),
                super::super::cards::Effect::IronChain => (1, 2),
                _ => match def.target_mode {
                    super::super::cards::TargetMode::AllAlive => {
                        let all: Vec<usize> = super::super::card_play::legal_targets(g, owner, def);
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "use_go", "c": carrier, "declared": declared, "targets": all }),
                        ));
                    }
                    _ => (1, 1),
                },
            };
            match take_answer(g, "guhuo") {
                None => {
                    let mut b = PendingBuilder::new(owner, "target").skill("guhuo");
                    for t in &legal {
                        b = b.option(format!("t:{t}"), format!("目标 {}", seat_name(g, *t)));
                    }
                    if picked.len() >= lo {
                        b = b.option("confirm", "确认目标");
                    }
                    let data = json!({ "step": "target_pick", "c": carrier, "declared": declared, "picked": picked });
                    Ok(Step::Need(b.escape("取消").data(data).build()))
                }
                Some(a) if a == "pass" => Ok(Step::Done),
                Some(a) if a == "confirm" => {
                    if picked.len() < lo {
                        return Err("蛊惑：目标数量不足".into());
                    }
                    Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "use_go", "c": carrier, "declared": declared, "targets": picked }),
                    ))
                }
                Some(a) if a.starts_with("t:") => {
                    let t: usize = a[2..].parse().map_err(|_| "蛊惑：非法目标")?;
                    if !legal.contains(&t) {
                        return Err("蛊惑：目标非法".into());
                    }
                    let mut np = picked;
                    np.push(t);
                    if np.len() >= hi {
                        return Ok(Step::Next(
                            Stage::Choose,
                            json!({ "step": "use_go", "c": carrier, "declared": declared, "targets": np }),
                        ));
                    }
                    Ok(Step::Next(
                        Stage::Target,
                        json!({ "step": "target_pick", "c": carrier, "declared": declared, "picked": np }),
                    ))
                }
                Some(o) => Err(format!("蛊惑：未知选项 {o}")),
            }
        }
        "use_go" => {
            let carrier = common::card_of(ask, "c").ok_or("蛊惑：缺扣置牌")?;
            let declared = ask
                .data
                .get("declared")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let targets = common::seats_of(ask, "targets");
            if !matches!(g.cards.get(&carrier), Some(Zone::Hand(s)) if *s == owner) {
                return Err("蛊惑：扣置牌不在手牌".into());
            }
            g.bump(&turn_key(owner, "guhuo_used"));
            super::super::guhuo::start(g, owner, &carrier, &declared, "蛊惑")?;
            Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "use_wait", "c": carrier, "declared": declared, "targets": targets }),
            ))
        }
        "use_wait" => {
            let carrier = common::card_of(ask, "c").ok_or("蛊惑：缺扣置牌")?;
            let declared = ask
                .data
                .get("declared")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let targets = common::seats_of(ask, "targets");
            let truth = g.guhuo_result.take().map(|r| r.truth).unwrap_or(false);
            if !truth {
                return Ok(Step::Finish("蛊惑：为假，此牌作废".into()));
            }
            super::super::card_play::use_as(g, owner, &declared, &[carrier], &targets, false)?;
            Ok(Step::Finish("蛊惑：为真，按声明牌结算".into()))
        }
        _ => Ok(Step::Done),
    }
}

// ---------------------------------------------------------------------------
// 界黄忠 · 烈弓
// ---------------------------------------------------------------------------

fn liegong_ready(g: &Game, seat: usize) -> bool {
    skills::active(g, seat, "liegong") && skills::enabled("liegong")
}

/// 使用【杀】时：距离 ≤ 此【杀】点数即视为可达（逐目标，不改写全局攻击范围）。
pub fn on_use_start(g: &mut Game, actor: usize, card: &str) {
    if !liegong_ready(g, actor) {
        return;
    }
    let Some(def) = common::def_of(g, card) else { return };
    if !def.tags.sha {
        return;
    }
    let rank = g.card_defs.get(card).map(|c| c.rank).unwrap_or(1) as i32;
    super::super::card_play::declare_range_override(g, card, rank);
}

/// 【杀】伤害 +1（逐目标：`turn.<seat>.liegong.t.<target>`）。
pub fn damage_bonus(g: &Game, spec: &DamageSpec) -> i32 {
    let Some(from) = spec.from else { return 0 };
    if spec.source != SourceKind::Slash {
        return 0;
    }
    if g.counter(&turn_key(from, &format!("liegong.t.{}", spec.target))) > 0 {
        1
    } else {
        0
    }
}

fn liegong(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !liegong_ready(g, owner) {
        return Ok(Step::Done);
    }
    let Some(by) = common::seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let data = json!({ "step": "sha_intercept", "by": by, "target": owner, "card": card });
    match take_answer(g, "liegong") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("liegong");
            b = b.option("use", "发动烈弓");
            Ok(Step::Need(b.escape("不发动").data(data).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => {
            role_grant_guard(g, owner);
            // 手牌数：目标 ≤ 自己 → 不能使用【闪】。
            let deny = g.hand_size(by) <= g.hand_size(owner);
            // 体力：自己 ≤ 目标 → 此【杀】对其伤害 +1。
            let bonus = g.seats.get(owner).map(|s| s.hp).unwrap_or(0)
                <= g.seats.get(by).map(|s| s.hp).unwrap_or(0);
            if bonus {
                g.bump(&turn_key(owner, &format!("liegong.t.{by}")));
            }
            let result = if deny {
                super::super::ShaIntercept::Unresponsive
            } else {
                super::super::ShaIntercept::Continue
            };
            super::super::card_play::declare_intercept(g, &card, result);
            Ok(Step::Finish("烈弓：结算".into()))
        }
    }
}

fn role_grant_guard(_g: &Game, _seat: usize) {}

// ---------------------------------------------------------------------------
// 界徐庶 · 诛害（他人结束阶段、其本回合造成过伤害 → 对其使用手牌【杀】，无距离）
// ---------------------------------------------------------------------------

fn zhuhai(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "zhuhai") || !skills::enabled("zhuhai") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    let victim = g.turn;
    if victim == owner || !g.seats.get(victim).map(|s| s.alive).unwrap_or(false) {
        return Ok(Step::Done);
    }
    if g.counter(&turn_key(victim, "damage_by")) <= 0 {
        return Ok(Step::Done);
    }
    let shas: Vec<CardId> = g
        .hand(owner)
        .into_iter()
        .filter(|id| common::def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
        .collect();
    if shas.is_empty() {
        return Ok(Step::Done);
    }
    if step == "pick" {
        return match take_answer(g, "zhuhai") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("zhuhai");
                for id in &shas {
                    b = b.option(format!("c:{id}"), format!("使用【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape("取消")
                        .data(json!({ "step": "pick", "victim": victim }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !shas.contains(&id) {
                    return Err("诛害：只能使用手牌中的【杀】".into());
                }
                super::super::card_play::use_on_targets(g, owner, &id, &[victim])?;
                Ok(Step::Finish("诛害：对其使用【杀】".into()))
            }
            Some(o) => Err(format!("诛害：未知选项 {o}")),
        };
    }
    match take_answer(g, "zhuhai") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("zhuhai");
            b = b.option("use", format!("对其使用一张【杀】（{}）", seat_name(g, victim)));
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask", "victim": victim }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a == "use" => {
            if shas.len() == 1 {
                super::super::card_play::use_on_targets(g, owner, &shas[0], &[victim])?;
                return Ok(Step::Finish("诛害：对其使用【杀】".into()));
            }
            Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "victim": victim }),
            ))
        }
        Some(_) => Ok(Step::Next(
            Stage::PickOwn,
            json!({ "step": "pick", "victim": victim }),
        )),
    }
}

// ---------------------------------------------------------------------------
// 界徐庶 · 潜心（觉醒）+ 授予技 荐言
// ---------------------------------------------------------------------------

fn qianxin(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "qianxin") || !skills::enabled("qianxin") {
        return Ok(Step::Done);
    }
    let by = common::seat_of(ask, "by");
    if by != Some(owner) {
        return Ok(Step::Done);
    }
    let wounded = g.seats.get(owner).map(|s| s.hp < s.max_hp).unwrap_or(false);
    if !wounded || g.counter(&format!("awaken.{owner}.qianxin")) > 0 {
        return Ok(Step::Done);
    }
    g.set_counter(&format!("awaken.{owner}.qianxin"), 1);
    flow::lose_max_hp(g, owner, 1, "潜心");
    skills::grant(g, owner, "jianyan");
    Ok(Step::Finish("潜心：觉醒（获得【荐言】）".into()))
}

fn jianyan_available(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && skills::active(g, seat, "jianyan")
        && g.counter(&phase_key(seat, "jianyan")) == 0
        && common::alive_seats(g)
            .into_iter()
            .any(|s| g.seats[s].male)
}

fn jianyan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    let step = step_of(ask);
    // 可用性只在**首次进入**校验（后续步骤已消耗本阶段次数）。
    if (step == "ask" || step == "choose" || step == "declare") && !jianyan_available(g, owner) {
        return Ok(Step::Done);
    }
    if step == "ask" || step == "choose" {
        return Ok(Step::Next(Stage::Choose, json!({ "step": "declare" })));
    }
    if step == "declare" {
        return match take_answer(g, "jianyan") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("jianyan");
                for (id, label) in [
                    ("basic", "声明：基本牌"),
                    ("trick", "声明：锦囊牌"),
                    ("equip", "声明：装备牌"),
                    ("red", "声明：红色牌"),
                    ("black", "声明：黑色牌"),
                ] {
                    b = b.option(id, label);
                }
                Ok(Step::Need(b.escape("不发动").data(json!({ "step": "declare" })).build()))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) => {
                if !["basic", "trick", "equip", "red", "black"].contains(&a.as_str()) {
                    return Err(format!("荐言：未知声明 {a}"));
                }
                // 亮出牌堆中第一张符合条件的牌。
                let mut found = None;
                let mut scanned = 0;
                let len = g.deck.len();
                while scanned < len {
                    let idx = g.deck.len() - 1 - scanned;
                    let id = g.deck[idx].clone();
                    let ok = g.card_defs.get(&id).map(|c| {
                        let d = super::super::cards::def(&c.def);
                        match a.as_str() {
                            "red" => c.suit.is_red(),
                            "black" => !c.suit.is_red(),
                            "basic" => d.map(|x| x.category == super::super::cards::Category::Basic).unwrap_or(false),
                            "trick" => d.map(|x| x.category == super::super::cards::Category::Trick).unwrap_or(false),
                            _ => d.map(|x| x.category == super::super::cards::Category::Equip).unwrap_or(false),
                        }
                    }).unwrap_or(false);
                    if ok {
                        found = Some(idx);
                        break;
                    }
                    scanned += 1;
                }
                let Some(idx) = found else {
                    return Ok(Step::Finish("荐言：牌堆中无符合条件的牌".into()));
                };
                let id = g.deck.remove(idx);
                g.cards.insert(id.clone(), Zone::Processing);
                g.reveal(&[id.clone()], "荐言");
                g.bump(&phase_key(owner, "jianyan"));
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "give", "card": id }),
                ))
            }
        };
    }
    // give：交给一名男性角色。
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let cands: Vec<usize> = common::alive_seats(g)
        .into_iter()
        .filter(|s| g.seats[*s].male)
        .collect();
    if cands.is_empty() {
        return Ok(Step::Finish("荐言：无男性角色".into()));
    }
    match take_answer(g, "jianyan") {
        None => {
            let mut b = PendingBuilder::new(owner, "target").skill("jianyan");
            for t in &cands {
                b = b.option(format!("t:{t}"), format!("交给 {}", seat_name(g, *t)));
            }
            Ok(Step::Need(
                b.escape_mandatory("（必选一名男性角色）")
                    .data(json!({ "step": "give", "card": card }))
                    .build(),
            ))
        }
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "荐言：非法目标")?;
            if !cands.contains(&t) {
                return Err("荐言：只能交给男性角色".into());
            }
            flow::gain(g, t, &card, owner, "荐言");
            Ok(Step::Finish("荐言：交给男性角色".into()))
        }
        Some(o) => Err(format!("荐言：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界华雄 · 耀武（锁定：红牌伤害来源摸 1；非红则自己摸 1）
// ---------------------------------------------------------------------------

fn yaowu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "yaowu") || !skills::enabled("yaowu") {
        return Ok(Step::Done);
    }
    let by = common::seat_of(ask, "by");
    let red = common::card_of(ask, "card")
        .and_then(|c| g.card_defs.get(&c).map(|x| x.suit.is_red()))
        .unwrap_or(false);
    if red {
        if let Some(src) = by {
            if src != owner && g.seats.get(src).map(|s| s.alive).unwrap_or(false) {
                flow::draw(g, src, 1);
                return Ok(Step::Finish("耀武：伤害来源摸一张".into()));
            }
        }
        return Ok(Step::Finish("耀武：红牌但无有效来源".into()));
    }
    flow::draw(g, owner, 1);
    Ok(Step::Finish("耀武：非红/虚拟牌，自己摸一张".into()))
}

// ---------------------------------------------------------------------------
// 界华雄 · 势斩（出牌阶段限两次：令其视为对你使用【决斗】）
// ---------------------------------------------------------------------------

fn shizhan_available(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && skills::active(g, seat, "shizhan")
        && g.counter(&phase_key(seat, "shizhan")) < 2
        && !common::alive_others(g, seat).is_empty()
}

fn shizhan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !shizhan_available(g, owner) {
        return Ok(Step::Done);
    }
    let cands = common::alive_others(g, owner);
    match take_answer(g, "shizhan") {
        None => {
            let mut b = PendingBuilder::new(owner, "target").skill("shizhan");
            for t in &cands {
                b = b.option(format!("t:{t}"), format!("令 {} 视为对你使用【决斗】", seat_name(g, *t)));
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "势斩：非法目标")?;
            if !cands.contains(&t) {
                return Err("势斩：目标非法".into());
            }
            g.bump(&phase_key(owner, "shizhan"));
            super::super::card_play::use_virtual(g, t, "juedou", &[owner], false)?;
            Ok(Step::Finish("势斩：视为使用【决斗】".into()))
        }
        Some(o) => Err(format!("势斩：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界张角 · 黄天（主公技；其他群势力角色出牌阶段可交给张角【闪】或黑桃手牌）
// ---------------------------------------------------------------------------

fn huangtian_lord(g: &Game) -> Option<usize> {
    g.seats.iter().position(|s| {
        s.alive
            && s.identity == Some(super::super::Identity::Lord)
            && skills::active(g, s.id, "huangtian")
            && skills::enabled("huangtian")
    })
}

fn huangtian_cards(g: &Game, seat: usize) -> Vec<CardId> {
    g.hand(seat)
        .into_iter()
        .filter(|id| {
            let shan = common::def_of(g, id).map(|d| d.tags.shan).unwrap_or(false);
            let spade = g.card_defs.get(id).map(|c| c.suit == super::super::Suit::Spade).unwrap_or(false);
            shan || spade
        })
        .collect()
}

pub fn other_side_actions(g: &Game, seat: usize) -> Vec<&'static str> {
    let Some(lord) = huangtian_lord(g) else { return vec![] };
    if seat == lord || g.turn != seat || g.phase != Phase::Play {
        return vec![];
    }
    if g.seats.get(seat).map(|s| s.kingdom.as_str()) != Some("qun") {
        return vec![];
    }
    if g.counter(&phase_key(seat, "huangtian")) > 0 || huangtian_cards(g, seat).is_empty() {
        return vec![];
    }
    vec!["huangtian"]
}

pub fn start_other(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    if code != "huangtian" || !other_side_actions(g, seat).contains(&"huangtian") {
        return Err("黄天：当前不可用".into());
    }
    super::super::resolve::push_ask(
        g,
        "huangtian",
        Stage::Choose,
        seat,
        json!({ "step": "give" }),
    );
    Ok(())
}

fn huangtian(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let giver = ask.actor;
    let Some(lord) = huangtian_lord(g) else {
        return Ok(Step::Done);
    };
    let cands = huangtian_cards(g, giver);
    if cands.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "huangtian") {
        None => {
            let mut b = PendingBuilder::new(giver, "pick_own").skill("huangtian");
            for id in &cands {
                b = b.option(format!("c:{id}"), format!("交给主公【{}】", card_label(g, id)));
            }
            Ok(Step::Need(b.escape("不交").data(json!({ "step": "give" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("c:") => {
            let id = a[2..].to_string();
            if !cands.contains(&id) {
                return Err("黄天：只能交【闪】或黑桃手牌".into());
            }
            g.bump(&phase_key(giver, "huangtian"));
            flow::gain(g, lord, &id, giver, "黄天");
            Ok(Step::Finish("黄天：交给主公".into()))
        }
        Some(o) => Err(format!("黄天：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界袁绍 · 乱击（两张同花色手牌当【万箭齐发】；可少选一个目标）
// ---------------------------------------------------------------------------

fn luanji_available(g: &Game, seat: usize) -> bool {
    if g.phase != Phase::Play || !skills::active(g, seat, "luanji") {
        return false;
    }
    // 需要两张同花色手牌。
    let hand = g.hand(seat);
    for i in 0..hand.len() {
        for j in (i + 1)..hand.len() {
            let si = g.card_defs.get(&hand[i]).map(|c| c.suit);
            let sj = g.card_defs.get(&hand[j]).map(|c| c.suit);
            if si.is_some() && si == sj {
                return true;
            }
        }
    }
    false
}

fn luanji(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "luanji") || !skills::enabled("luanji") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    if step == "pick2" {
        let first = common::card_of(ask, "first").ok_or("乱击：缺第一张牌")?;
        let suit = g.card_defs.get(&first).map(|c| c.suit);
        let cands: Vec<CardId> = g
            .hand(owner)
            .into_iter()
            .filter(|id| *id != first)
            .filter(|id| g.card_defs.get(id).map(|c| Some(c.suit) == suit).unwrap_or(false))
            .collect();
        if cands.is_empty() {
            return Ok(Step::Finish("乱击：无同花色第二张".into()));
        }
        return match take_answer(g, "luanji") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("luanji");
                for id in &cands {
                    b = b.option(format!("c:{id}"), format!("第二张【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape("取消")
                        .data(json!({ "step": "pick2", "first": first }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(a) if a.starts_with("c:") => {
                let second = a[2..].to_string();
                if !cands.contains(&second) {
                    return Err("乱击：第二张必须同花色".into());
                }
                let others: Vec<usize> = common::alive_others(g, owner);
                if others.len() <= 1 {
                    return Ok(Step::Next(
                        Stage::Choose,
                        json!({ "step": "use", "first": first, "second": second, "targets": others }),
                    ));
                }
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "trim", "first": first, "second": second }),
                ))
            }
            Some(o) => Err(format!("乱击：未知选项 {o}")),
        };
    }
    if step == "trim" {
        let first = common::card_of(ask, "first").ok_or("乱击：缺第一张牌")?;
        let second = common::card_of(ask, "second").ok_or("乱击：缺第二张牌")?;
        let others: Vec<usize> = common::alive_others(g, owner);
        return match take_answer(g, "luanji") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("luanji");
                b = b.option("trim", "少选一个目标");
                let data = json!({ "step": "trim", "first": first, "second": second });
                Ok(Step::Need(
                    b.escape("不少选")
                        .data(data)
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Next(
                Stage::Choose,
                json!({ "step": "use", "first": first, "second": second, "targets": others }),
            )),
            Some(a) if a == "trim" => Ok(Step::Next(
                Stage::Target,
                json!({ "step": "trim_pick", "first": first, "second": second }),
            )),
            Some(o) => Err(format!("乱击：未知选项 {o}")),
        };
    }
    if step == "trim_pick" {
        let first = common::card_of(ask, "first").ok_or("乱击：缺第一张牌")?;
        let second = common::card_of(ask, "second").ok_or("乱击：缺第二张牌")?;
        let others: Vec<usize> = common::alive_others(g, owner);
        return match take_answer(g, "luanji") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("luanji");
                for t in &others {
                    b = b.option(format!("t:{t}"), format!("少选 {}", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一个目标）")
                        .data(json!({ "step": "trim_pick", "first": first, "second": second }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "乱击：非法目标")?;
                if !others.contains(&t) {
                    return Err("乱击：目标非法".into());
                }
                let targets: Vec<usize> = others.into_iter().filter(|x| *x != t).collect();
                Ok(Step::Next(
                    Stage::Choose,
                    json!({ "step": "use", "first": first, "second": second, "targets": targets }),
                ))
            }
            Some(o) => Err(format!("乱击：未知选项 {o}")),
        };
    }
    if step == "use" {
        let first = common::card_of(ask, "first").ok_or("乱击：缺第一张牌")?;
        let second = common::card_of(ask, "second").ok_or("乱击：缺第二张牌")?;
        let targets = common::seats_of(ask, "targets");
        super::super::card_play::use_as(g, owner, "wanjian_qifa", &[first, second], &targets, false)?;
        return Ok(Step::Finish("乱击：视为使用【万箭齐发】".into()));
    }
    // 首步：选第一张牌。
    let hand = g.hand(owner);
    if hand.is_empty() {
        return Ok(Step::Done);
    }
    match take_answer(g, "luanji") {
        None => {
            let mut b = PendingBuilder::new(owner, "pick_own").skill("luanji");
            for id in &hand {
                b = b.option(format!("c:{id}"), format!("第一张【{}】", card_label(g, id)));
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "pick1" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("c:") => {
            let id = a[2..].to_string();
            if !hand.contains(&id) {
                return Err("乱击：该牌不在手牌".into());
            }
            Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick2", "first": id }),
            ))
        }
        Some(o) => Err(format!("乱击：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界袁绍 · 血裔（主公技：「裔」= 群势力数×2；出牌阶段开始可移 1 摸 1；上限 +裔数）
// ---------------------------------------------------------------------------

fn xueyi_start(ask: &Ask) -> bool {
    ask.data.get("event").and_then(|v| v.as_str()) == Some("PhaseStart")
}

fn xueyi(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "xueyi") || !skills::enabled("xueyi") {
        return Ok(Step::Done);
    }
    // 开局：按全场群势力角色数（含自己）×2 获得「裔」。
    if ask.data.get("event").is_none() {
        let n = g
            .seats
            .iter()
            .filter(|s| s.alive && s.kingdom == "qun")
            .count();
        g.set_counter(&format!("yi.{owner}"), (n * 2) as i32);
        return Ok(Step::Finish(format!("血裔：获得 {} 枚「裔」", n * 2)));
    }
    if !xueyi_start(ask) || g.phase != Phase::Play {
        return Ok(Step::Done);
    }
    if g.counter(&format!("yi.{owner}")) <= 0 || g.counter(&phase_key(owner, "xueyi")) > 0 {
        return Ok(Step::Done);
    }
    match take_answer(g, "xueyi") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("xueyi");
            b = b.option("use", "移去 1 枚「裔」，摸一张牌");
            Ok(Step::Need(
                b.escape("不发动")
                    .data(json!({ "step": "ask" }))
                    .build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => {
            g.set_counter(&format!("yi.{owner}"), g.counter(&format!("yi.{owner}")) - 1);
            g.bump(&phase_key(owner, "xueyi"));
            flow::draw(g, owner, 1);
            Ok(Step::Finish("血裔：移 1 枚「裔」摸一张".into()))
        }
    }
}

pub fn hand_limit_bonus(g: &Game, seat: usize) -> i32 {
    if !skills::active(g, seat, "xueyi") || !skills::enabled("xueyi") {
        return 0;
    }
    g.counter(&format!("yi.{seat}")).max(0)
}

// ---------------------------------------------------------------------------
// 界刘禅 · 享乐（锁定：成为【杀】目标后，使用者须弃一张基本牌，否则【杀】无效）
// ---------------------------------------------------------------------------

fn xiangle(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let target = ask.actor;
    if !skills::active(g, target, "xiangle") || !skills::enabled("xiangle") {
        return Ok(Step::Done);
    }
    let Some(by) = common::seat_of(ask, "by") else {
        return Ok(Step::Done);
    };
    let Some(card) = common::card_of(ask, "card") else {
        return Ok(Step::Done);
    };
    let basics: Vec<CardId> = g
        .hand(by)
        .into_iter()
        .filter(|id| {
            common::def_of(g, id)
                .map(|d| d.category == super::super::cards::Category::Basic)
                .unwrap_or(false)
        })
        .collect();
    if basics.is_empty() {
        // 没有基本牌 → 直接无效（不产生 pending）。
        super::super::card_play::declare_intercept(g, &card, super::super::ShaIntercept::Immune);
        return Ok(Step::Finish("享乐：使用者无基本牌，此【杀】无效".into()));
    }
    let data = json!({ "step": "sha_intercept", "by": by, "card": card });
    match take_answer(g, "xiangle") {
        None => {
            let mut b = PendingBuilder::new(by, "pick_own").skill("xiangle");
            for id in &basics {
                b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
            }
            Ok(Step::Need(
                b.escape("不弃（此【杀】对你无效）")
                    .data(data)
                    .build(),
            ))
        }
        Some(a) if a == "pass" => {
            super::super::card_play::declare_intercept(g, &card, super::super::ShaIntercept::Immune);
            Ok(Step::Finish("享乐：未弃牌，此【杀】无效".into()))
        }
        Some(a) if a.starts_with("c:") => {
            let id = a[2..].to_string();
            if !basics.contains(&id) {
                return Err("享乐：只能弃置一张基本牌".into());
            }
            flow::discard(g, by, &id, "享乐");
            super::super::card_play::declare_intercept(g, &card, super::super::ShaIntercept::Continue);
            Ok(Step::Finish("享乐：弃置基本牌，【杀】正常结算".into()))
        }
        Some(o) => Err(format!("享乐：未知选项 {o}")),
    }
}

// ---------------------------------------------------------------------------
// 界刘禅 · 放权（跳过出牌阶段；弃牌阶段开始弃一张手牌令他人额外回合）
// ---------------------------------------------------------------------------

fn fangquan_available(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && skills::active(g, seat, "fangquan")
        && g.counter(&turn_key(seat, "fangquan")) == 0
}

fn fangquan(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "fangquan") || !skills::enabled("fangquan") {
        return Ok(Step::Done);
    }
    let step = step_of(ask);
    // ② 弃牌阶段开始（`PhaseStart`）：弃一张手牌 → 选一名其他角色（其额外回合）。
    if step == "ask" || ask.data.get("event").is_some() {
        if g.phase != Phase::Discard || g.counter(&turn_key(owner, "fangquan")) <= 0 {
            return Ok(Step::Done);
        }
        if g.hand(owner).is_empty() {
            return Ok(Step::Finish("放权：无手牌，不能授予额外回合".into()));
        }
        return match take_answer(g, "fangquan") {
            None => {
                let mut b = PendingBuilder::new(owner, "choose").skill("fangquan");
                b = b.option("use", "弃一张手牌，令一名其他角色执行额外回合");
                Ok(Step::Need(
                    b.escape("不发动")
                        .data(json!({ "step": "pick", "phase": "discard" }))
                        .build(),
                ))
            }
            Some(a) if a == "pass" => Ok(Step::Done),
            Some(_) => Ok(Step::Next(
                Stage::PickOwn,
                json!({ "step": "pick", "phase": "discard" }),
            )),
        };
    }
    if step == "pick" {
        let hand = g.hand(owner);
        return match take_answer(g, "fangquan") {
            None => {
                let mut b = PendingBuilder::new(owner, "pick_own").skill("fangquan");
                for id in &hand {
                    b = b.option(format!("c:{id}"), format!("弃置【{}】", card_label(g, id)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必弃一张）")
                        .data(json!({ "step": "pick", "phase": "discard" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("c:") => {
                let id = a[2..].to_string();
                if !hand.contains(&id) {
                    return Err("放权：该牌不在手牌".into());
                }
                flow::discard(g, owner, &id, "放权");
                Ok(Step::Next(
                    Stage::Target,
                    json!({ "step": "target", "phase": "discard" }),
                ))
            }
            Some(o) => Err(format!("放权：未知选项 {o}")),
        };
    }
    if step == "target" {
        let cands = common::alive_others(g, owner);
        if cands.is_empty() {
            return Ok(Step::Finish("放权：无其他角色".into()));
        }
        return match take_answer(g, "fangquan") {
            None => {
                let mut b = PendingBuilder::new(owner, "target").skill("fangquan");
                for t in &cands {
                    b = b.option(format!("t:{t}"), format!("令 {} 执行额外回合", seat_name(g, *t)));
                }
                Ok(Step::Need(
                    b.escape_mandatory("（必选一名其他角色）")
                        .data(json!({ "step": "target", "phase": "discard" }))
                        .build(),
                ))
            }
            Some(a) if a.starts_with("t:") => {
                let t: usize = a[2..].parse().map_err(|_| "放权：非法目标")?;
                if !cands.contains(&t) {
                    return Err("放权：目标非法".into());
                }
                super::super::turn::extra_turn(g, t);
                Ok(Step::Finish("放权：其执行一个额外回合".into()))
            }
            Some(o) => Err(format!("放权：未知选项 {o}")),
        };
    }
    // ① 出牌阶段主动：跳过出牌阶段，并登记弃牌阶段的放权窗口。
    match take_answer(g, "fangquan") {
        None => {
            let mut b = PendingBuilder::new(owner, "choose").skill("fangquan");
            b = b.option("use", "跳过出牌阶段（弃牌阶段开始可令他人额外回合）");
            Ok(Step::Need(
                b.escape("不发动").data(json!({ "step": "active" })).build(),
            ))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(_) => {
            g.bump(&turn_key(owner, "fangquan"));
            super::super::turn::skip_current_play(g, owner);
            Ok(Step::Finish("放权：跳过出牌阶段".into()))
        }
    }
}

// ---------------------------------------------------------------------------
// 界刘禅 · 若愚（主公技+觉醒）+ 授予技 思蜀
// ---------------------------------------------------------------------------

fn ruoyu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !skills::active(g, owner, "ruoyu") || !skills::enabled("ruoyu") {
        return Ok(Step::Done);
    }
    if g.seats.get(owner).and_then(|s| s.identity) != Some(super::super::Identity::Lord) {
        return Ok(Step::Done);
    }
    if g.counter(&format!("awaken.{owner}.ruoyu")) > 0 {
        return Ok(Step::Done);
    }
    let hp = g.seats.get(owner).map(|s| s.hp).unwrap_or(0);
    let strictly_min = g
        .seats
        .iter()
        .filter(|s| s.alive && s.id != owner)
        .all(|s| hp < s.hp);
    if !strictly_min {
        return Ok(Step::Done);
    }
    g.set_counter(&format!("awaken.{owner}.ruoyu"), 1);
    if let Some(s) = g.seats.get_mut(owner) {
        s.max_hp += 1;
        if s.hp < 3 {
            s.hp = 3;
        }
    }
    let mut ev = super::super::event::Event::simple(
        EventKind::MaxHpChanged,
        Some(owner),
        Some("若愚"),
    );
    ev.amount = Some(1);
    let e = ev.snapshot(g);
    g.push_event(e);
    skills::grant(g, owner, "jijiang");
    skills::grant(g, owner, "sishu");
    Ok(Step::Finish("若愚：觉醒（体力上限+1，获得【激将】【思蜀】）".into()))
}

fn sishu_available(g: &Game, seat: usize) -> bool {
    g.phase == Phase::Play
        && skills::active(g, seat, "sishu")
        && g.counter(&turn_key(seat, "sishu")) == 0
        && !g.alive_seats().is_empty()
}

fn sishu(g: &mut Game, ask: &Ask) -> Result<Step, String> {
    let owner = ask.actor;
    if !sishu_available(g, owner) {
        return Ok(Step::Done);
    }
    let cands: Vec<usize> = g.alive_seats();
    match take_answer(g, "sishu") {
        None => {
            let mut b = PendingBuilder::new(owner, "target").skill("sishu");
            for t in &cands {
                let state = if g.judge_reversed(*t, "lebu_sishu") {
                    "恢复常规"
                } else {
                    "判定反转"
                };
                b = b.option(format!("t:{t}"), format!("{}（{state}）", seat_name(g, *t)));
            }
            Ok(Step::Need(b.escape("不发动").data(json!({ "step": "target" })).build()))
        }
        Some(a) if a == "pass" => Ok(Step::Done),
        Some(a) if a.starts_with("t:") => {
            let t: usize = a[2..].parse().map_err(|_| "思蜀：非法目标")?;
            if !cands.contains(&t) {
                return Err("思蜀：目标非法".into());
            }
            g.bump(&turn_key(owner, "sishu"));
            if g.judge_reversed(t, "lebu_sishu") {
                // 已反转 → 恢复常规。
                g.judge_reverse.retain(|_, v| v != "lebu_sishu");
                Ok(Step::Finish(format!("思蜀：{} 恢复常规", seat_name(g, t))))
            } else {
                // 未反转 → 反转（切换时旧目标自动清除，`reverse_judge` 单值语义）。
                g.reverse_judge(t, "lebu_sishu");
                Ok(Step::Finish(format!("思蜀：{} 的【乐不思蜀】判定反转", seat_name(g, t))))
            }
        }
        Some(o) => Err(format!("思蜀：未知选项 {o}")),
    }
}

#[allow(dead_code)]
fn _unused(_: &Game, _: Phase, _: &str) {
    let _ = seat_name;
}
