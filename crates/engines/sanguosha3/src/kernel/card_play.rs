//! 牌结算单一流程（P3 交互 + P4 完整结算 + P5 装备特效）：
//! 使用 → 选目标 → 无懈链 → 结算 → 弃牌堆/装备区/判定区。
//!
//! 不变量（mechanics `use-vs-play` / `target-list` / `response-cards` / `no-wuxie`）：
//! 1. **使用 ≠ 打出**：「使用」= 主动进入结算；「打出」= 响应提供牌（不进结算）。
//!    闪只能响应、不能主动使用；杀在响应里是「打出」，不计入使用次数。
//! 2. **目标列表**：每次使用记录 `targets: Vec<seat>`，技能只读列表，不按牌名反推目标模式。
//! 3. **响应牌**读 `CardDef.tags` / `responds_as`，不按牌名枚举。
//! 4. **无懈链**：奇偶层判定（奇数 = 锦囊被抵消）；「不能被无懈」是牌属性的独立事实。
//! 5. **效果分派只读 `CardDef.effect`**（内核零牌名）；装备特效只读 `gear::weapon_of`/`armor_of`。
//!
//! 交互形态（P3）：所有询问都走 `pending` + `resolve::submit`；
//! 只有「合法集为空」才不询问（constitution I1），因此无可用响应牌时同步结算（老调用方不受影响）。

use serde_json::{json, Map, Value};

use super::cards::{self, CardDef, Effect, ResponseKind, TargetMode};
use super::event::{Event, EventKind};
use super::flow;
use super::judge;
use super::pending::PendingBuilder;
use super::resolve::{take_answer, FlowKind, Stage, Step};
use super::{ArmorKind, CardId, DamageKind, DamageSpec, Game, Slot, SourceKind, WeaponKind, Zone};

// ---------------------------------------------------------------------------
// 低层 API（技能/测试直接调用；P4 流程帧在它们之上组合）
// ---------------------------------------------------------------------------

/// 出牌阶段可主动使用的手牌（闪/无懈/满血桃被排除）。
///
/// `defs` = 牌定义表查表（生成物；内核不按牌名字面量判定）。
pub fn legal_uses(g: &Game, seat: usize, defs: &dyn Fn(&str) -> Option<CardDef>) -> Vec<String> {
    let mut out = vec![];
    for (id, zone) in &g.cards {
        if *zone != Zone::Hand(seat) {
            continue;
        }
        // 手牌冻结（mechanics `hand-freeze`）：冻结的手牌不进入主动使用候选。
        if g.is_frozen(seat, id) {
            continue;
        }
        let Some(c) = g.card_defs.get(id) else { continue };
        let Some(d) = defs(&c.def) else { continue };
        if d.tags.shan || d.tags.wuxie {
            continue;
        }
        if d.tags.tao && g.seats[seat].hp >= g.seats[seat].max_hp {
            continue;
        }
        out.push(id.clone());
    }
    out.sort();
    out
}

/// 出牌阶段**真正可用**的手牌（含次数/距离/目标存在性过滤；UI 与校验同源）。
pub fn usable_hand(g: &Game, seat: usize) -> Vec<String> {
    let mut out = vec![];
    for id in g.hand(seat) {
        if g.is_frozen(seat, &id) {
            continue;
        }
        let Some(c) = g.card_defs.get(&id) else { continue };
        let Some(def) = cards::def(&c.def) else { continue };
        if !can_use_now(g, seat, def) {
            continue;
        }
        out.push(id);
    }
    out
}

/// 单张牌现在能否使用（唯一判定；`usable_hand` 与 `begin_use` 共同依赖）。
pub fn can_use_now(g: &Game, seat: usize, def: &CardDef) -> bool {
    if def.tags.shan || def.tags.wuxie {
        return false;
    }
    if def.tags.tao && g.seats[seat].hp >= g.seats[seat].max_hp {
        return false;
    }
    if def.tags.jiu && jiu_limit_reached(g, seat) {
        return false;
    }
    if def.tags.sha && sha_limit_reached(g, seat) {
        return false;
    }
    if let Some(slot) = def.slot {
        // 已废除的装备栏不可再装备（候选与执行同源）。
        return !g.slot_abolished(seat, slot);
    }
    match def.effect {
        Effect::ExNihilo | Effect::PeachGarden | Effect::None => true,
        Effect::Lightning => true,
        Effect::Collateral => collateral_available(g, seat),
        _ => !legal_targets(g, seat, def).is_empty(),
    }
}

/// 本阶段【杀】次数是否已用尽（连弩/技能视为无限）。
pub fn sha_limit_reached(g: &Game, seat: usize) -> bool {
    let used = g.counter(&format!("phase.sha_used.{seat}"));
    // 连弩（装备特效 = 特殊技能注入）+ 技能无限杀。
    if super::gear::weapon_of(g, seat) == WeaponKind::Zhugeliannu
        || super::skills::sha_unlimited(g, seat)
    {
        return false;
    }
    let allowed = 1 + super::skills::sha_extra(g, seat);
    used >= allowed
}

/// 本回合【酒】是否已使用。
pub fn jiu_limit_reached(g: &Game, seat: usize) -> bool {
    g.counter(&format!("phase.jiu_used.{seat}")) >= 1
}

/// 使用【桃】→ 回血（唯一出口 `flow::heal`，满血回 0）。
pub fn resolve_tao(g: &mut Game, seat: usize) -> Vec<Event> {
    flow::heal(g, seat, 1, "桃")
}

/// 无懈链：奇数层 = 锦囊被抵消（`layers` = 已打出的无懈张数）。
pub fn wuxie_cancelled(layers: usize) -> bool {
    layers % 2 == 1
}

/// 一张牌能否指定该目标（默认规则；技能覆盖走钩子）。
pub fn can_target(def: &CardDef, self_seat: usize, target: usize, alive: bool) -> bool {
    cards::valid_target(def.target_mode, self_seat, target, alive)
}

/// 牌进弃牌堆（唯一出口，`lose-vs-discard`：弃置是离开手牌/装备区进弃牌堆）。
pub fn to_discard(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), Zone::Discard);
}

/// 响应牌判定：读标签，不按牌名。
pub fn can_respond(def: &CardDef, kind: ResponseKind) -> bool {
    def.responds_as(kind)
}

/// 【杀】结算结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaResult {
    /// 目标出【闪】，抵消。
    Blocked,
    /// 命中，造成点数（含【酒】加成）。
    Hit { damage: i32 },
}

/// 使用【酒】：本回合下一张【杀】伤害 +1（出牌阶段；濒死自救走 `flow::rescue`）。
pub fn use_jiu(g: &mut Game) {
    g.sha_bonus = 1;
}

/// 使用【杀】结算（同步版，技能/测试用）：`shan` = 目标是否打出【闪】。
/// 【酒】加成在使用时消费（无论命中与否，「酒」已花费）。
pub fn resolve_sha(g: &mut Game, from: usize, target: usize, shan: bool, armor: ArmorKind) -> ShaResult {
    let bonus = g.sha_bonus;
    g.sha_bonus = 0;
    if shan {
        return ShaResult::Blocked;
    }
    let spec = DamageSpec {
        from: Some(from),
        target,
        amount: 1 + bonus,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    flow::damage(g, spec, armor, 0);
    ShaResult::Hit { damage: 1 + bonus }
}

/// 无中生有：摸 2 张。
pub fn resolve_wuzhong(g: &mut Game, seat: usize) -> Vec<CardId> {
    flow::draw(g, seat, 2)
}

/// 过河拆桥：弃目标一张牌（`card` 必须是目标手牌/装备区/判定区的牌）。
pub fn resolve_guohe(g: &mut Game, target: usize, card: &str) -> bool {
    let ok = matches!(
        g.cards.get(card),
        Some(Zone::Hand(t)) | Some(Zone::Equip(t, _)) | Some(Zone::Judge(t)) if *t == target
    );
    if ok {
        remove_card(g, target, card, "过河拆桥");
    }
    ok
}

/// 延时锦囊进目标判定区（同名不可重复放）。
pub fn place_delayed(g: &mut Game, target: usize, card: &str, def_code: &str) -> bool {
    // 判定区被废除（雄乱）：延时锦囊不可进入。
    if g.seats.get(target).map(|s| s.judge_abolished).unwrap_or(false) {
        return false;
    }
    let stack = g.judge_stacks.entry(target).or_default();
    if stack.iter().any(|id| g.card_defs.get(id).map(|c| c.def == def_code).unwrap_or(false)) {
        return false;
    }
    g.cards.insert(card.to_string(), Zone::Judge(target));
    stack.push(card.to_string());
    let mut ev = Event::card(
        EventKind::JudgeZoneEntered,
        Some(target),
        card.to_string(),
        Some("延时锦囊".to_string()),
    );
    ev.by = Some(target);
    let e = ev.snapshot(g);
    g.push_event(e);
    true
}

/// 判定区 LIFO：后置先判。返回下一个要判定的牌（栈顶），并移出判定区。
pub fn judge_zone_lifo(g: &mut Game, seat: usize) -> Option<String> {
    let id = g.judge_stacks.get_mut(&seat)?.pop()?;
    g.cards.insert(id.clone(), Zone::Discard);
    Some(id)
}

/// 锦囊结算（无懈包装）：`layers` = 无懈张数，奇数 = 锦囊被抵消。
pub fn trick_cancelled_by_wuxie(layers: usize) -> bool {
    wuxie_cancelled(layers)
}

// ---------------------------------------------------------------------------
// P3：使用入口（出牌阶段 → 压 Card 流程帧）
// ---------------------------------------------------------------------------

/// 开始使用一张手牌（唯一入口）：发使用/失去牌事件，牌进处理区，压结算流程帧。
pub fn begin_use(g: &mut Game, actor: usize, card: &str, fire: bool) -> Result<(), String> {
    if !matches!(g.cards.get(card), Some(Zone::Hand(s)) if *s == actor) {
        return Err(format!("{card} 不在座位 {actor} 的手牌"));
    }
    if g.is_frozen(actor, card) {
        return Err("该手牌已被冻结（本回合/本阶段不能使用）".into());
    }
    let def = def_of(g, card)?;
    if def.tags.sha && sha_limit_reached(g, actor) {
        return Err("本阶段【杀】次数已用尽".into());
    }
    if def.tags.jiu && jiu_limit_reached(g, actor) {
        return Err("本回合【酒】已使用".into());
    }
    use_start(g, actor, card)?;
    super::resolve::push_flow(
        g,
        FlowKind::Card,
        actor,
        Stage::CardTarget,
        json!({ "card": card, "actor": actor, "fire": fire, "started": true, "sub": "target" }),
    );
    Ok(())
}

/// 指定目标使用一张牌（**技能驱动/内部使用**：跳过目标合法性，只做结算）。
///
/// 用途：掣政①「攻击范围不含你的角色对你使用牌无效」这类**规则允许但越距**的使用（技能强制/测试）。
/// 玩家主动出牌必须走 `begin_use`（受 `legal_targets` 约束）。
pub fn use_on_targets(g: &mut Game, actor: usize, card: &str, targets: &[usize]) -> Result<(), String> {
    use_start(g, actor, card)?;
    super::resolve::push_flow(
        g,
        FlowKind::Card,
        actor,
        Stage::CardTarget,
        json!({
            "card": card,
            "actor": actor,
            "fire": false,
            "started": true,
            "sub": "target",
            "targets": targets,
        }),
    );
    Ok(())
}

/// 【铁索连环】重铸（出牌阶段唯一入口）：弃置此牌 → 摸一张牌（不计入「使用」）。
pub fn recast(g: &mut Game, actor: usize, card: &str) -> Result<(), String> {
    if !matches!(g.cards.get(card), Some(Zone::Hand(s)) if *s == actor) {
        return Err(format!("{card} 不在座位 {actor} 的手牌"));
    }
    if g.is_frozen(actor, card) {
        return Err("该手牌已被冻结（本回合/本阶段不能使用）".into());
    }
    let def = def_of(g, card)?;
    if !def.tags.lianhuan {
        return Err("只有【铁索连环】可以重铸".into());
    }
    flow::discard(g, actor, card, "重铸");
    flow::draw(g, actor, 1);
    Ok(())
}

/// 丈八蛇矛：两张手牌当【杀】（虚拟牌实例；守恒由「实例 + 区域」同时登记保证）。
pub fn begin_use_zhangba(g: &mut Game, actor: usize, c1: &str, c2: &str) -> Result<(), String> {
    for c in [c1, c2] {
        if !matches!(g.cards.get(c), Some(Zone::Hand(s)) if *s == actor) {
            return Err(format!("{c} 不在座位 {actor} 的手牌"));
        }
    }
    if sha_limit_reached(g, actor) {
        return Err("本阶段【杀】次数已用尽".into());
    }
    let suit = g.card_defs.get(c1).map(|c| c.suit).unwrap_or(super::Suit::Spade);
    let rank = g.card_defs.get(c1).map(|c| c.rank).unwrap_or(7);
    for c in [c1, c2] {
        g.cards.insert(c.to_string(), Zone::Discard);
        let mut lost = Event::card(EventKind::CardLost, Some(actor), c.to_string(), Some("丈八蛇矛".to_string()));
        lost.by = Some(actor);
        lost.from = Some(actor);
        let e = lost.snapshot(g);
        g.push_event(e);
    }
    let vid = format!("v:z{}", g.next_frame_id);
    g.card_defs.insert(
        vid.clone(),
        super::Card {
            id: vid.clone(),
            def: "sha".into(),
            suit,
            rank,
        },
    );
    g.cards.insert(vid.clone(), Zone::Processing);
    g.bump(&format!("turn.{actor}.cards_used"));
    g.bump(&format!("phase.sha_used.{actor}"));
    let mut used = Event::card(EventKind::CardUsed, Some(actor), vid.clone(), Some("丈八蛇矛".to_string()));
    used.by = Some(actor);
    used.from = Some(actor);
    let e = used.snapshot(g);
    g.push_event(e);
    super::resolve::push_flow(
        g,
        FlowKind::Card,
        actor,
        Stage::CardTarget,
        json!({ "card": vid, "actor": actor, "fire": false, "started": true, "sub": "target" }),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 流程帧入口（`resolve::advance` 的 Flow 分支）
// ---------------------------------------------------------------------------

/// 流程帧 handler。`Card` 走 P3/P4 状态机；`Turn` 走回合驱动；`Dying` 走濒死救援。
pub fn step_flow(
    g: &mut Game,
    kind: FlowKind,
    data: &Value,
    actor: usize,
) -> Result<Step, String> {
    match kind {
        FlowKind::Card => step_card(g, actor, data),
        FlowKind::Turn => super::turn::step_turn(g, data),
        FlowKind::Dying => step_dying(g, data),
        FlowKind::Pindian => super::pindian::step_pindian(g, data),
        FlowKind::Guhuo => super::guhuo::step_guhuo(g, data),
        _ => Ok(Step::Done),
    }
}

fn def_of(g: &Game, card: &str) -> Result<CardDef, String> {
    g.card_defs
        .get(card)
        .and_then(|c| cards::def(&c.def))
        .copied()
        .ok_or_else(|| format!("牌 {card} 无定义"))
}

fn uid(d: &Map<String, Value>, k: &str) -> usize {
    d.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as usize
}

fn flag(d: &Map<String, Value>, k: &str) -> bool {
    d.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn sval(d: &Map<String, Value>, k: &str) -> Option<String> {
    d.get(k).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn next(stage: Stage, d: Map<String, Value>) -> Step {
    Step::Next(stage, Value::Object(d))
}

fn targets_of(d: &Map<String, Value>) -> Vec<usize> {
    d.get("targets")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|v| v as usize)).collect())
        .unwrap_or_default()
}

fn step_card(g: &mut Game, actor: usize, data: &Value) -> Result<Step, String> {
    let Some(card) = data.get("card").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
        return Err("流程帧缺 card".into());
    };
    let mut d = match data {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    // 首次进入：发「使用牌 / 失去手牌」事件 + 牌移入处理区（P2：结算路径自动产事件）。
    if d.get("started").is_none() {
        use_start(g, actor, &card)?;
        d.insert("started".into(), json!(true));
    }
    match d.get("sub").and_then(|v| v.as_str()).unwrap_or("target") {
        "target" => step_target(g, actor, d),
        "wuxie" => step_wuxie(g, actor, d),
        _ => step_effect(g, actor, d),
    }
}

/// 使用起步（唯一）：手牌 → 处理区，发 `CardLost` + `CardUsed`，记使用计数。
fn use_start(g: &mut Game, actor: usize, card: &str) -> Result<(), String> {
    let def = def_of(g, card)?;
    if let Some(Zone::Hand(s)) = g.cards.get(card).cloned() {
        let mut lost = Event::card(
            EventKind::CardLost,
            Some(s),
            card.to_string(),
            Some("使用".to_string()),
        );
        lost.by = Some(actor);
        lost.from = Some(s);
        let e = lost.snapshot(g);
        g.push_event(e);
    }
    g.cards.insert(card.to_string(), Zone::Processing);
    g.bump(&format!("turn.{actor}.cards_used"));
    if def.tags.sha {
        g.bump(&format!("phase.sha_used.{actor}"));
    }
    if def.tags.jiu {
        g.bump(&format!("phase.jiu_used.{actor}"));
    }
    let mut used = Event::card(
        EventKind::CardUsed,
        Some(actor),
        card.to_string(),
        Some("使用".to_string()),
    );
    used.by = Some(actor);
    let e = used.snapshot(g);
    g.push_event(e);
    // 使用前技能窗口（往烈：首张无距离 / 不能响应 + 手牌冻结）：压帧到牌结算帧之上。
    super::skills::fire_use_pre(g, actor, card);
    Ok(())
}

// ---------------------------------------------------------------------------
// 目标选择（P3：选目标 / 多选 + 确认 / 取消）
// ---------------------------------------------------------------------------

/// 合法目标集合（唯一来源）：存活 + 目标模式 + 「不能成为目标」+ 距离/存在性。
pub fn legal_targets(g: &Game, actor: usize, def: &CardDef) -> Vec<usize> {
    legal_targets_with(g, actor, None, def)
}

/// 合法目标集合（带**本次使用**的 `UseOpts`）：`no_distance` / `range_override` 只作用于本次。
pub fn legal_targets_with(
    g: &Game,
    actor: usize,
    card: Option<&str>,
    def: &CardDef,
) -> Vec<usize> {
    let opts = card.map(|c| g.use_opts(c)).unwrap_or_default();
    let mut out = vec![];
    for s in 0..g.seats.len() {
        if !g.seats[s].alive {
            continue;
        }
        if !cards::valid_target(def.target_mode, actor, s, true) {
            continue;
        }
        if def.target_mode != TargetMode::AllAlive
            && (super::skills::deny_target(g, actor, s)
                || super::skills::deny_target_card(g, actor, s, def.code))
        {
            continue;
        }
        match def.effect {
            Effect::Sha => {
                // 本次无距离限制（往烈首张/陷嗣/慎断）：只跳过距离，不改其它目标规则。
                let reach = opts
                    .range_override
                    .map(|r| r as usize)
                    .unwrap_or_else(|| super::gear::attack_range(g, actor) as usize);
                if !opts.no_distance
                    && !super::skills::ignore_distance(g, actor)
                    && super::gear::distance(g, actor, s) > reach
                {
                    continue;
                }
            }
            Effect::Snatch => {
                if super::gear::distance(g, actor, s) > 1 {
                    continue;
                }
                if target_cards(g, s).is_empty() {
                    continue;
                }
            }
            Effect::Dismantle => {
                if target_cards(g, s).is_empty() {
                    continue;
                }
            }
            Effect::Indulgence | Effect::SupplyShortage => {
                if has_delayed(g, s, def.code) {
                    continue;
                }
                // 兵粮寸断：只能对距离 1 以内的角色使用（乐不思蜀无距离限制）。
                if def.effect == Effect::SupplyShortage
                    && super::gear::distance(g, actor, s) > 1
                {
                    continue;
                }
            }
            _ => {}
        }
        out.push(s);
    }
    out
}

fn has_delayed(g: &Game, seat: usize, code: &str) -> bool {
    g.judge_stacks
        .get(&seat)
        .map(|stack| {
            stack
                .iter()
                .any(|id| g.card_defs.get(id).map(|c| c.def == code).unwrap_or(false))
        })
        .unwrap_or(false)
}

fn target_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| match z {
            Zone::Hand(s) | Zone::Equip(s, _) | Zone::Judge(s) => *s == seat,
            _ => false,
        })
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn collateral_available(g: &Game, actor: usize) -> bool {
    let has_weapon = (0..g.seats.len()).any(|s| {
        s != actor && g.seats[s].alive && super::gear::equip_in(g, s, Slot::Weapon).is_some()
    });
    has_weapon && g.alive_seats().len() > 2
}

/// 目标数量上下界（`Effect::Collateral` 两目标 / 铁索 1–2 / 方天最后手牌 1–3 / 技能额外目标）。
fn target_bounds(g: &Game, actor: usize, def: &CardDef, card: Option<&str>) -> (usize, usize) {
    let opts = card.map(|c| g.use_opts(c)).unwrap_or_default();
    let extra = opts.extra_targets.max(0) as usize;
    let trim = opts.trim_targets.max(0) as usize;
    let base = match def.effect {
        Effect::Collateral => (2, 2),
        Effect::IronChain => (1, 2),
        Effect::Sha
            if super::gear::weapon_of(g, actor) == WeaponKind::Fangtian && g.hand_size(actor) == 0 =>
        {
            (1, 3)
        }
        _ => match def.target_mode {
            TargetMode::None | TargetMode::SelfOnly => (0, 0),
            // 群体锦囊：裁剪后仍必须至少选 1 个（少选 N 个，不选不能使用）。
            TargetMode::AllAlive => {
                let total = other_alive_from(g, actor).len();
                let keep = total.saturating_sub(trim).max(1);
                (keep, keep)
            }
            TargetMode::Single | TargetMode::Chosen => (1, 1),
        },
    };
    (base.0, base.1 + extra)
}

fn step_target(g: &mut Game, actor: usize, mut d: Map<String, Value>) -> Result<Step, String> {
    let card = sval(&d, "card").ok_or("流程帧缺 card")?;
    let def = def_of(g, &card)?;
    // 兼容旧同步路径：显式给 target / targets 时不询问。
    if d.get("targets").is_none() {
        if let Some(t) = d.get("target").and_then(|v| v.as_u64()) {
            d.insert("targets".into(), json!([t as usize]));
        }
    }
    if let Some(list) = d.get("targets").and_then(|v| v.as_array()).cloned() {
        emit_targets(g, actor, &card, &list);
        d.insert("sub".into(), json!("wuxie"));
        return Ok(next(Stage::WuxieAsk, d));
    }
    // 自动目标（无选择）
    let opts = g.use_opts(&card);
    let auto: Option<Vec<usize>> = match def.target_mode {
        TargetMode::None | TargetMode::SelfOnly => Some(vec![actor]),
        // 群体锦囊少选（mechanics `aoe-target-trim`）：交给玩家选，不自动全选。
        TargetMode::AllAlive if opts.trim_targets <= 0 => Some(other_alive_from(g, actor)),
        _ => None,
    };
    if let Some(list) = auto {
        d.insert("targets".into(), json!(list));
        emit_targets(g, actor, &card, &d["targets"].as_array().cloned().unwrap_or_default());
        d.insert("sub".into(), json!("wuxie"));
        return Ok(next(Stage::WuxieAsk, d));
    }
    let (min, max) = target_bounds(g, actor, &def, Some(&card));
    let mut picked: Vec<usize> = d
        .get("picked")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|v| v as usize)).collect())
        .unwrap_or_default();
    let mut confirmed = false;
    if let Some(ans) = take_answer(g, "flow:card") {
        if ans == "cancel" {
            return Ok(cancel_use(g, actor, &card));
        } else if ans == "ok" {
            confirmed = true;
        } else if let Some(t) = ans.strip_prefix("t:") {
            let t: usize = t.parse().map_err(|_| "非法目标".to_string())?;
            if !legal_pick(g, actor, &card, &def, &picked, t) {
                return Err(format!("非法目标：{t}"));
            }
            picked.push(t);
        }
    }
    if picked.len() >= max || (confirmed && picked.len() >= min) {
        d.insert("targets".into(), json!(picked));
        emit_targets(g, actor, &card, &d["targets"].as_array().cloned().unwrap_or_default());
        d.insert("sub".into(), json!("wuxie"));
        return Ok(next(Stage::WuxieAsk, d));
    }
    d.insert("picked".into(), json!(picked));
    let mut b = PendingBuilder::new(actor, "sgs3_target").skill("flow:card");
    for t in legal_targets_with(g, actor, Some(&card), &def) {
        if picked.contains(&t) || !legal_pick(g, actor, &card, &def, &picked, t) {
            continue;
        }
        b = b.option(
            format!("t:{t}"),
            format!(
                "选择 {} 号位（{}）",
                t + 1,
                g.seats.get(t).map(|s| s.name.clone()).unwrap_or_default()
            ),
        );
    }
    if picked.len() >= min {
        b = b.option("ok", "确定目标");
    }
    Ok(Step::NeedWith(
        Stage::CardTarget,
        Value::Object(d),
        b.escape_with("cancel", "取消使用").build(),
    ))
}

/// 追加选择是否合法（借刀：先选持武器者，再选另一个目标）。
fn legal_pick(g: &Game, actor: usize, card: &str, def: &CardDef, picked: &[usize], t: usize) -> bool {
    if picked.contains(&t) || t == actor || !g.seats[t].alive {
        return false;
    }
    if def.effect == Effect::Collateral {
        if picked.is_empty() {
            return super::gear::equip_in(g, t, Slot::Weapon).is_some();
        }
        return t != picked[0];
    }
    legal_targets_with(g, actor, Some(card), def).contains(&t)
}

fn other_alive_from(g: &Game, seat: usize) -> Vec<usize> {
    let n = g.seats.len();
    (1..n)
        .map(|k| (seat + k) % n)
        .filter(|&s| g.seats[s].alive)
        .collect()
}

fn emit_targets(g: &mut Game, actor: usize, card: &str, targets: &[Value]) {
    let seats: Vec<usize> = targets
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as usize))
        .collect();
    g.use_targets.insert(card.to_string(), seats);
    for t in targets {
        let Some(t) = t.as_u64() else { continue };
        let mut ev = Event::card(
            EventKind::TargetChosen,
            Some(t as usize),
            card.to_string(),
            Some("目标".to_string()),
        );
        ev.by = Some(actor);
        let e = ev.snapshot(g);
        g.push_event(e);
    }
}

/// 取消使用：牌回手牌，使用计数回退（未完成的「使用」不算一次使用）。
/// 虚拟牌（丈八蛇矛 `v:`）不回手牌，落到 `Void`（避免合成牌混进手牌/牌堆）。
fn cancel_use(g: &mut Game, actor: usize, card: &str) -> Step {
    g.clear_use_opts(card);
    if card.starts_with("v:") {
        // 取消转化使用：承载牌回手牌（未发生使用），虚拟牌落 Void。
        if let Some(origins) = g.virtual_origins.remove(card) {
            for id in origins {
                g.cards.insert(id, Zone::Hand(actor));
            }
        }
        g.cards.insert(card.to_string(), Zone::Void);
    } else {
        g.cards.insert(card.to_string(), Zone::Hand(actor));
    }
    let key = format!("turn.{actor}.cards_used");
    let v = g.counter(&key);
    g.set_counter(&key, (v - 1).max(0));
    if let Ok(def) = def_of(g, card) {
        if def.tags.sha {
            let k = format!("phase.sha_used.{actor}");
            let v = g.counter(&k);
            g.set_counter(&k, (v - 1).max(0));
        }
        if def.tags.jiu {
            let k = format!("phase.jiu_used.{actor}");
            let v = g.counter(&k);
            g.set_counter(&k, (v - 1).max(0));
        }
    }
    Step::Done
}

// ---------------------------------------------------------------------------
// 无懈链（P4）
// ---------------------------------------------------------------------------

fn step_wuxie(g: &mut Game, actor: usize, mut d: Map<String, Value>) -> Result<Step, String> {
    let card = sval(&d, "card").ok_or("流程帧缺 card")?;
    let def = def_of(g, &card)?;
    // 基本牌/装备/无懈本身不进无懈窗口；“本次不能被无懈”（no-wuxie）只关无懈窗口。
    if !def.effect.is_trick() || def.tags.wuxie || flag(&d, "no_wuxie") || g.use_opts(&card).no_wuxie {
        d.insert("sub".into(), json!("effect"));
        d.insert("esub".into(), json!("start"));
        return Ok(next(Stage::CardResolve, d));
    }
    let layers = uid(&d, "layers");
    let idx = uid(&d, "widx");
    let order: Vec<usize> = {
        let n = g.seats.len();
        (0..n)
            .map(|k| (actor + k) % n)
            .filter(|&s| g.seats[s].alive)
            .collect()
    };
    if idx >= order.len() {
        d.insert("cancelled".into(), json!(wuxie_cancelled(layers)));
        d.insert("sub".into(), json!("effect"));
        d.insert("esub".into(), json!("start"));
        return Ok(next(Stage::CardResolve, d));
    }
    let asker = order[idx];
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("w:") {
            let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == asker)
                && def_of(g, id).map(|c| c.tags.wuxie).unwrap_or(false);
            if !ok {
                return Err("无懈可击：只能打出手牌中的【无懈可击】".into());
            }
            played(g, asker, id, "无懈可击")?;
            d.insert("layers".into(), json!(layers + 1));
            d.insert("widx".into(), json!(0));
            return Ok(next(Stage::WuxieAsk, d));
        } else if let Some(id) = ans.strip_prefix("equip:") {
            // 解围①：装备区牌当【无懈可击】。
            let ok = matches!(g.cards.get(id), Some(Zone::Equip(s, _)) if *s == asker)
                && super::skills::active(g, asker, "jiewei")
                && super::skills::enabled("jiewei");
            if !ok {
                return Err("解围：只能以自己装备区的牌当【无懈可击】".into());
            }
            play_from_equip(g, asker, id, "解围·无懈可击")?;
            d.insert("layers".into(), json!(layers + 1));
            d.insert("widx".into(), json!(0));
            return Ok(next(Stage::WuxieAsk, d));
        } else if ans == "pass" {
            d.insert("widx".into(), json!(idx + 1));
            return Ok(next(Stage::WuxieAsk, d));
        }
    }
    let mut b = PendingBuilder::new(asker, "wuxie").skill("flow:card");
    let mut any = false;
    for id in g.hand(asker) {
        if def_of(g, &id).map(|c| c.tags.wuxie).unwrap_or(false) && !g.is_frozen(asker, &id) {
            b = b.option(format!("w:{id}"), "打出【无懈可击】");
            any = true;
        }
    }
    // 解围①：装备区牌当【无懈可击】。
    if super::skills::active(g, asker, "jiewei") && super::skills::enabled("jiewei") {
        for id in super::gear::equips_of(g, asker) {
            b = b.option(format!("equip:{id}"), "解围：以装备牌当【无懈可击】");
            any = true;
        }
    }
    if !any {
        d.insert("widx".into(), json!(idx + 1));
        return Ok(next(Stage::WuxieAsk, d));
    }
    let _ = any;
    Ok(Step::NeedAt(Stage::WuxieAsk, b.escape("不无懈").build()))
}

// ---------------------------------------------------------------------------
// 效果结算（P4 / P5）
// ---------------------------------------------------------------------------

fn step_effect(g: &mut Game, actor: usize, mut d: Map<String, Value>) -> Result<Step, String> {
    let card = sval(&d, "card").ok_or("流程帧缺 card")?;
    let def = def_of(g, &card)?;
    if flag(&d, "cancelled") {
        finish_card(g, &card);
        return Ok(Step::Done);
    }
    let mut esub = sval(&d, "esub").unwrap_or_else(|| "start".into());
    if esub == "start" {
        d.insert("ti".into(), json!(0));
        d.insert("esub".into(), json!("ask"));
        d.insert("picked".into(), json!(0));
        esub = "ask".into();
    }
    let targets = targets_of(&d);
    let ti = uid(&d, "ti");
    let target = targets.get(ti).copied();
    match def.effect {
        Effect::Sha => step_sha(g, actor, &card, &def, target, &mut d, &esub),
        Effect::Duel => step_duel(g, actor, &def, target, &mut d, &esub),
        Effect::SavageAssault | Effect::ArcheryAttack => {
            step_aoe(g, actor, &def, target, &mut d, &esub)
        }
        Effect::Dismantle | Effect::Snatch => step_steal(g, actor, &def, target, &mut d, &esub),
        Effect::ExNihilo => {
            flow::draw(g, actor, 2);
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::PeachGarden => {
            if !g.endgame_no_heal() {
                for s in g.alive_seats() {
                    flow::heal(g, s, 1, "桃园结义");
                }
            }
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::Harvest => step_harvest(g, actor, &card, &mut d, &esub),
        Effect::FireAttack => step_fire_attack(g, actor, &def, target, &mut d, &esub),
        Effect::Collateral => step_collateral(g, actor, &def, &mut d, &esub),
        Effect::IronChain => {
            for t in targets {
                toggle_chain(g, t, "铁索连环");
            }
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::Indulgence | Effect::SupplyShortage | Effect::Lightning => {
            let mut placed = false;
            for t in targets {
                if place_delayed(g, t, &card, def.code) {
                    placed = true;
                }
            }
            if !placed {
                finish_card(g, &card);
            }
            Ok(Step::Done)
        }
        Effect::Shan | Effect::Wuxie | Effect::None if def.slot.is_none() => {
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::Tao => {
            // 终局房规（ADR-0040）：第 100 轮后【桃】回血无效；濒死救援走 `flow::rescue`，不受限。
            if !g.endgame_no_heal() {
                flow::heal(g, actor, 1, "桃");
            }
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::Jiu => {
            g.sha_bonus = 1;
            finish_card(g, &card);
            Ok(Step::Done)
        }
        Effect::None => {
            // 装备牌：顶掉同槽旧装备 → 离开装备区钩子（白银狮子回体力）→ ArmorChanged 事件。
            let named_ev = if def.slot.is_some() {
                Some(equip_card(g, actor, &card, &def)?)
            } else {
                None
            };
            let _ = named_ev;
            finish_card(g, &card);
            Ok(Step::Done)
        }
        _ => {
            finish_card(g, &card);
            Ok(Step::Done)
        }
    }
}

pub(crate) fn equip_card(g: &mut Game, actor: usize, card: &str, def: &CardDef) -> Result<Event, String> {
    if let Some(slot) = def.slot {
        if g.slot_abolished(actor, slot) {
            return Err("该装备栏已被废除，不可再装备".into());
        }
    }
    let old = super::gear::equip(g, actor, card);
    if let Some(old_id) = old {
        let mut lost = Event::card(
            EventKind::CardLost,
            Some(actor),
            old_id.clone(),
            Some("装备被替换".to_string()),
        );
        lost.by = Some(actor);
        lost.from = Some(actor);
        let e = lost.snapshot(g);
        g.push_event(e);
        let mut discarded = Event::card(
            EventKind::CardDiscarded,
            Some(actor),
            old_id.clone(),
            Some("装备被替换".to_string()),
        );
        discarded.by = Some(actor);
        discarded.from = Some(actor);
        let e = discarded.snapshot(g);
        g.push_event(e);
        flow::leave_equip(g, actor, &old_id, "装备被替换");
    }
    if def.slot == Some(Slot::Armor) {
        let mut ev = Event::simple(EventKind::ArmorChanged, Some(actor), Some("装备防具"));
        ev.card = Some(card.to_string());
        let e = ev.snapshot(g);
        g.push_event(e.clone());
        return Ok(e);
    }
    let mut ev = Event::simple(EventKind::CardIntoGame, Some(actor), Some("装备"));
    ev.card = Some(card.to_string());
    let e = ev.snapshot(g);
    g.push_event(e.clone());
    Ok(e)
}

fn toggle_chain(g: &mut Game, seat: usize, cause: &str) {
    let Some(s) = g.seats.get_mut(seat) else { return };
    s.chained = !s.chained;
    let kind = if s.chained {
        EventKind::ChainedEnter
    } else {
        EventKind::ChainedLeft
    };
    let ev = Event::simple(kind, Some(seat), Some(cause));
    let chained = s.chained;
    let e = ev.snapshot(g);
    let _ = chained;
    g.push_event(e);
}

/// 【杀】结算：仁王盾 → 闪/八卦 → 命中（酒/古锭/藤甲/青釭/铁索传导）→ 麒麟弓。
fn step_sha(
    g: &mut Game,
    actor: usize,
    card: &str,
    def: &CardDef,
    target: Option<usize>,
    d: &mut Map<String, Value>,
    esub: &str,
) -> Result<Step, String> {
    let Some(target) = target else { return Ok(finish_and_done(g, card)) };
    let ti = uid(d, "ti");
    let advance_target = |d: &mut Map<String, Value>| {
        d.insert("ti".into(), json!(ti + 1));
        d.insert("esub".into(), json!("ask"));
        d.insert("picked".into(), json!(0));
        d.insert("bagua_used".into(), json!(false));
        next(Stage::CardResolve, d.clone())
    };
    if !g.seats[target].alive
        || super::skills::card_ineffective(g, actor, target)
        || super::skills::card_ineffective_for(g, actor, target, card)
    {
        return Ok(advance_target(d));
    }
    if super::skills::no_response_for(g, actor, target) {
        g.use_opts_mut(card).no_response = true;
    }
    let ignore_armor = super::gear::ignores_armor(g, actor) || g.use_opts(card).ignore_armor;
    let armor = if ignore_armor {
        ArmorKind::None
    } else {
        super::gear::armor_of(g, target)
    };
    match esub {
        "ask" => {
            // 【杀】结算前介入（mechanics `sha-intercept`）：目标锁定技在响应窗口之前结算。
            if !g.use_opts(card).intercept_checked {
                if super::skills::has_sha_intercept(g, target) {
                    g.use_opts_mut(card).intercept_checked = true;
                    super::skills::fire_sha_intercept(g, actor, target, card);
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
                g.use_opts_mut(card).intercept_checked = true;
            }
            match g.use_opts(card).intercept {
                Some(super::ShaIntercept::Immune) => {
                    // 此【杀】对此目标无效：不询问【闪】，不产生伤害。
                    return Ok(advance_target(d));
                }
                Some(super::ShaIntercept::Unresponsive) => {
                    d.insert("esub".into(), json!("harm"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
                Some(super::ShaIntercept::Replace(t2)) => {
                    if g.seats.get(t2).map(|s| s.alive).unwrap_or(false) && t2 != target {
                        let mut list = targets_of(d);
                        if let Some(slot) = list.get_mut(ti) {
                            *slot = t2;
                        }
                        let vals: Vec<Value> = list.iter().map(|v| json!(v)).collect();
                        d.insert("targets".into(), json!(vals));
                        d.insert("esub".into(), json!("ask"));
                        return Ok(next(Stage::CardResolve, d.clone()));
                    }
                }
                _ => {}
            }
            // 不能被响应（往烈/雄乱）：不等【闪】直接结算伤害。
            if g.use_opts(card).no_response {
                d.insert("esub".into(), json!("harm"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            // 多张响应（mechanics `multi-response`）：需连续 N 张【闪】才算抵消。
            let need = g.use_opts(card).multi_response.max(1);
            let got = d.get("resp_got").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            // 仁王盾：黑色【杀】无效（不需要出闪）。
            if armor == ArmorKind::Renwang {
                let black = g
                    .card_defs
                    .get(card)
                    .map(|c| !c.suit.is_red())
                    .unwrap_or(false);
                if black {
                    return Ok(advance_target(d));
                }
            }
            let bagua = armor == ArmorKind::Bagua && !flag(d, "bagua_used");
            if let Some(ans) = take_answer(g, "flow:card") {
                if ans == "bagua" {
                    let red = judge::judge(g, target, "八卦阵")
                        .map(|r| r.is_red())
                        .unwrap_or(false);
                    d.insert("bagua_used".into(), json!(true));
                    d.insert("esub".into(), json!(if red { "blocked" } else { "ask" }));
                    return Ok(next(Stage::CardResolve, d.clone()));
                } else if let Some(id) = ans.strip_prefix("s:") {
                    let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == target)
                        && def_of(g, id).map(|c| c.tags.shan).unwrap_or(false)
                        && !g.is_frozen(target, id);
                    if !ok {
                        return Err("【杀】：只能打出【闪】".into());
                    }
                    played(g, target, id, "闪")?;
                    if got + 1 < need {
                        d.insert("resp_got".into(), json!(got + 1));
                        d.insert("esub".into(), json!("ask"));
                        return Ok(next(Stage::CardResolve, d.clone()));
                    }
                    d.insert("esub".into(), json!("blocked"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                } else if ans == "pass" {
                    d.insert("esub".into(), json!("harm"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
            }
            let shan: Vec<String> = g
                .hand(target)
                .into_iter()
                .filter(|id| def_of(g, id).map(|c| c.tags.shan).unwrap_or(false))
                .filter(|id| !g.is_frozen(target, id))
                .collect();
            if shan.is_empty() && !bagua {
                d.insert("esub".into(), json!("harm"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            let mut b = PendingBuilder::new(target, "shan_ask").skill("flow:card");
            for id in shan {
                b = b.option(format!("s:{id}"), "打出【闪】");
            }
            if bagua {
                b = b.option("bagua", "发动【八卦阵】判定");
            }
            Ok(Step::NeedAt(Stage::CardAsk, b.escape("不闪避").build()))
        }
        "blocked" => {
            // 被闪抵消：青龙偃月刀（再杀）/ 贯石斧（弃两张牌强命中）。
            let weapon = super::gear::weapon_of(g, actor);
            if let Some(ans) = take_answer(g, "flow:card") {
                if let Some(id) = ans.strip_prefix("again:") {
                    let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == actor)
                        && def_of(g, id).map(|c| c.tags.sha).unwrap_or(false);
                    if !ok {
                        return Err("青龙偃月刀：只能使用手牌中的【杀】".into());
                    }
                    played(g, actor, id, "青龙偃月刀")?;
                    d.insert("bagua_used".into(), json!(true));
                    d.insert("esub".into(), json!("ask"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                } else if ans == "guanshi" {
                    d.insert("esub".into(), json!("guanshi"));
                    d.insert("picked".into(), json!(0));
                    return Ok(next(Stage::CardResolve, d.clone()));
                } else if ans == "pass" {
                    return Ok(advance_target(d));
                }
            }
            let mut b = PendingBuilder::new(actor, "sha_blocked").skill("flow:card");
            let mut any = false;
            if weapon == WeaponKind::Qinglong {
                for id in g.hand(actor) {
                    if def_of(g, &id).map(|c| c.tags.sha).unwrap_or(false) {
                        b = b.option(format!("again:{id}"), "青龙偃月刀：再使用一张【杀】");
                        any = true;
                    }
                }
            }
            if weapon == WeaponKind::Guanshi && own_cards(g, actor).len() >= 2 {
                b = b.option("guanshi", "贯石斧：弃两张牌令【杀】命中");
                any = true;
            }
            if !any {
                return Ok(advance_target(d));
            }
            Ok(Step::NeedAt(Stage::CardAsk, b.escape("放弃").build()))
        }
        "guanshi" => {
            let picked = uid(d, "picked");
            if picked >= 2 {
                d.insert("picked".into(), json!(0));
                d.insert("esub".into(), json!("harm"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            if let Some(ans) = take_answer(g, "flow:card") {
                if let Some(id) = ans.strip_prefix("discard:") {
                    if !own_cards(g, actor).iter().any(|c| c == id) {
                        return Err("贯石斧：只能弃置自己的手牌/装备".into());
                    }
                    flow::discard(g, actor, id, "贯石斧");
                    d.insert("picked".into(), json!(picked + 1));
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
            }
            let mut b = PendingBuilder::new(actor, "guanshi_pick").skill("flow:card");
            for id in own_cards(g, actor) {
                b = b.option(format!("discard:{id}"), format!("弃置【{}】", card_name(g, &id)));
            }
            Ok(Step::NeedAt(
                Stage::CardPick,
                b.escape_mandatory("弃牌").build(),
            ))
        }
        "cixiong" => {
            let opposite = g.seats[actor].male != g.seats[target].male;
            if !opposite {
                d.insert("esub".into(), json!("harm"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            if let Some(ans) = take_answer(g, "flow:card") {
                if ans == "draw" {
                    flow::draw(g, actor, 1);
                    d.insert("esub".into(), json!("harm"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                } else if let Some(id) = ans.strip_prefix("discard:") {
                    let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == target);
                    if !ok {
                        return Err("雌雄双股剑：只能弃置目标手牌".into());
                    }
                    flow::discard(g, target, id, "雌雄双股剑");
                    d.insert("esub".into(), json!("harm"));
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
            }
            let mut b = PendingBuilder::new(target, "cixiong").skill("flow:card");
            for id in g.hand(target) {
                b = b.option(format!("discard:{id}"), format!("弃置【{}】", card_name(g, &id)));
            }
            b = b.option("draw", "令使用者摸一张牌");
            Ok(Step::NeedAt(Stage::CardOpt, b.escape_mandatory("选择一项").build()))
        }
        "hanbing" => {
            let picked = uid(d, "picked");
            if picked >= 2 || target_cards(g, target).is_empty() {
                d.insert("esub".into(), json!("next"));
                return Ok(advance_target(d));
            }
            if let Some(ans) = take_answer(g, "flow:card") {
                if let Some(id) = ans.strip_prefix("c:") {
                    remove_card(g, target, id, "寒冰剑");
                    d.insert("picked".into(), json!(picked + 1));
                    return Ok(next(Stage::CardResolve, d.clone()));
                }
            }
            let mut b = PendingBuilder::new(actor, "hanbing_pick").skill("flow:card");
            for id in target_cards(g, target) {
                b = b.option(format!("c:{id}"), format!("弃置【{}】", card_name(g, &id)));
            }
            Ok(Step::NeedAt(Stage::CardPick, b.escape_mandatory("弃牌").build()))
        }
        "qilin" => {
            if let Some(ans) = take_answer(g, "flow:card") {
                if let Some(id) = ans.strip_prefix("q:") {
                    if super::gear::horses_of(g, target).contains(&id.to_string()) {
                        remove_card(g, target, id, "麒麟弓");
                    }
                }
            }
            Ok(advance_target(d))
        }
        "harm" => {
            // 雌雄双股剑：指定异性目标后，目标二选一（弃一张手牌 / 令使用者摸一张牌）。
            if super::gear::weapon_of(g, actor) == WeaponKind::Cixiong
                && g.seats[actor].male != g.seats[target].male
                && !flag(d, "cixiong_done")
            {
                d.insert("cixiong_done".into(), json!(true));
                d.insert("esub".into(), json!("cixiong"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            // 寒冰剑：可选「防止伤害，改为弃其两张牌」。
            if super::gear::weapon_of(g, actor) == WeaponKind::Hanbing {
                if let Some(ans) = take_answer(g, "flow:card") {
                    if ans == "hanbing" {
                        d.insert("esub".into(), json!("hanbing"));
                        d.insert("picked".into(), json!(0));
                        return Ok(next(Stage::CardResolve, d.clone()));
                    }
                    if ans == "pass" {
                        d.insert("esub".into(), json!("hit"));
                        return Ok(next(Stage::CardResolve, d.clone()));
                    }
                }
                let mut b = PendingBuilder::new(actor, "hanbing_opt").skill("flow:card");
                b = b.option("hanbing", "寒冰剑：防止此伤害，改为弃其两张牌");
                return Ok(Step::NeedAt(Stage::CardOpt, b.escape("继续造成伤害").build()));
            }
            d.insert("esub".into(), json!("hit"));
            Ok(next(Stage::CardResolve, d.clone()))
        }
        _ => {
            // "hit"（及未知）→ 实际伤害
            let jiu = g.sha_bonus;
            g.sha_bonus = 0;
            let guding = super::gear::weapon_of(g, actor) == WeaponKind::Gudingdao
                && g.hand_size(target) == 0;
            let amount = 1 + jiu + if guding { 1 } else { 0 };
            let kind = if flag(d, "fire") {
                DamageKind::Fire
            } else {
                def.damage_kind
            };
            let armor = if ignore_armor {
                ArmorKind::None
            } else {
                super::gear::armor_of(g, target)
            };
            let spec = DamageSpec {
                from: Some(actor),
                target,
                amount,
                kind,
                source: SourceKind::Slash,
            };
            flow::damage(g, spec, armor, 0);
            // 麒麟弓：造成伤害后可弃置其一张坐骑牌。
            if super::gear::weapon_of(g, actor) == WeaponKind::Qilin
                && !super::gear::horses_of(g, target).is_empty()
            {
                if let Some(ans) = take_answer(g, "flow:card") {
                    if let Some(id) = ans.strip_prefix("q:") {
                        if super::gear::horses_of(g, target).contains(&id.to_string()) {
                            remove_card(g, target, id, "麒麟弓");
                        }
                        return Ok(advance_target(d));
                    }
                    if ans == "pass" {
                        return Ok(advance_target(d));
                    }
                }
                let mut b = PendingBuilder::new(actor, "qilin_opt").skill("flow:card");
                for id in super::gear::horses_of(g, target) {
                    b = b.option(format!("q:{id}"), format!("弃置【{}】", card_name(g, &id)));
                }
                return Ok(Step::NeedAt(Stage::CardOpt, b.escape("跳过").build()));
            }
            Ok(advance_target(d))
        }
    }
}

/// 决斗：目标先出【杀】→ 交替，先不出者受到对方 1 点伤害。
fn step_duel(
    g: &mut Game,
    actor: usize,
    _def: &CardDef,
    target: Option<usize>,
    d: &mut Map<String, Value>,
    _esub: &str,
) -> Result<Step, String> {
    let Some(target) = target else { return finish_done(g, d) };
    let card = sval(d, "card").unwrap_or_default();
    let need = g.use_opts(&card).multi_response.max(1);
    let mut cur = d.get("duel_cur").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(target);
    let other = d.get("duel_other").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(actor);
    if !g.seats[cur].alive {
        cur = other;
    }
    if g.use_opts(&card).no_response
        || super::skills::no_response_for(g, actor, cur)
    {
        let spec = DamageSpec {
            from: Some(other),
            target: cur,
            amount: 1,
            kind: DamageKind::Normal,
            source: SourceKind::Duel,
        };
        flow::damage(g, spec, super::gear::armor_of(g, cur), 0);
        return finish_done(g, d);
    }
    let got = d.get("resp_got").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("s:") {
            let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == cur)
                && def_of(g, id).map(|c| c.tags.sha).unwrap_or(false)
                && !g.is_frozen(cur, id);
            if !ok {
                return Err("决斗：只能打出【杀】".into());
            }
            played(g, cur, id, "决斗")?;
            if got + 1 < need {
                d.insert("resp_got".into(), json!(got + 1));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            d.insert("resp_got".into(), json!(0));
            d.insert("duel_cur".into(), json!(other));
            d.insert("duel_other".into(), json!(cur));
            return Ok(next(Stage::CardResolve, d.clone()));
        } else if ans == "pass" {
            let spec = DamageSpec {
                from: Some(other),
                target: cur,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Duel,
            };
            flow::damage(g, spec, super::gear::armor_of(g, cur), 0);
            return finish_done(g, d);
        }
    }
    let shas: Vec<String> = g
        .hand(cur)
        .into_iter()
        .filter(|id| def_of(g, id).map(|c| c.tags.sha).unwrap_or(false))
        .filter(|id| !g.is_frozen(cur, id))
        .collect();
    if shas.is_empty() {
        let spec = DamageSpec {
            from: Some(other),
            target: cur,
            amount: 1,
            kind: DamageKind::Normal,
            source: SourceKind::Duel,
        };
        flow::damage(g, spec, super::gear::armor_of(g, cur), 0);
        return finish_done(g, d);
    }
    let mut b = PendingBuilder::new(cur, "duel_ask").skill("flow:card");
    for id in shas {
        b = b.option(format!("s:{id}"), "打出【杀】");
    }
    Ok(Step::NeedAt(Stage::CardAsk, b.escape("不出杀").build()))
}

/// 南蛮入侵 / 万箭齐发：逐目标要求响应牌（杀/闪），不出则受伤。
fn step_aoe(
    g: &mut Game,
    actor: usize,
    def: &CardDef,
    target: Option<usize>,
    d: &mut Map<String, Value>,
    _esub: &str,
) -> Result<Step, String> {
    let Some(target) = target else { return finish_done(g, d) };
    let card = sval(d, "card").unwrap_or_default();
    let need = g.use_opts(&card).multi_response.max(1);
    let ti = uid(d, "ti");
    let next_target = |d: &mut Map<String, Value>| {
        d.insert("ti".into(), json!(ti + 1));
        d.insert("resp_got".into(), json!(0));
        next(Stage::CardResolve, d.clone())
    };
    let expect = def.respond;
    let source = if def.effect == Effect::SavageAssault {
        SourceKind::Nanman
    } else {
        SourceKind::Wanjian
    };
    let damage = |g: &mut Game, d: &mut Map<String, Value>| -> Result<Step, String> {
        let spec = DamageSpec {
            from: Some(actor),
            target,
            amount: 1,
            kind: DamageKind::Normal,
            source,
        };
        flow::damage(g, spec, super::gear::armor_of(g, target), 0);
        Ok(next_target(d))
    };
    if !g.seats[target].alive || super::skills::card_ineffective(g, actor, target) {
        return Ok(next_target(d));
    }
    // 不能被响应（往烈 / 雄乱）：群体锦囊目标直接受伤。
    if g.use_opts(&card).no_response || super::skills::no_response_for(g, actor, target) {
        return damage(g, d);
    }
    let got = d.get("resp_got").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("p:") {
            let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == target)
                && expect
                    .map(|k| def_of(g, id).map(|c| c.responds_as(k)).unwrap_or(false))
                    .unwrap_or(false)
                && !g.is_frozen(target, id);
            if !ok {
                return Err("响应牌不符合要求".into());
            }
            played(g, target, id, "响应")?;
            if got + 1 < need {
                d.insert("resp_got".into(), json!(got + 1));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
            return Ok(next_target(d));
        } else if ans == "pass" {
            return damage(g, d);
        }
    }
    let cards: Vec<String> = match expect {
        Some(k) => g
            .hand(target)
            .into_iter()
            .filter(|id| def_of(g, id).map(|c| c.responds_as(k)).unwrap_or(false))
            .filter(|id| !g.is_frozen(target, id))
            .collect(),
        None => vec![],
    };
    if cards.is_empty() {
        return damage(g, d);
    }
    let mut b = PendingBuilder::new(target, "aoe_ask").skill("flow:card");
    let label = match expect {
        Some(ResponseKind::Shan) => "打出【闪】",
        Some(ResponseKind::Sha) => "打出【杀】",
        _ => "打出响应牌",
    };
    for id in cards {
        b = b.option(format!("p:{id}"), label);
    }
    Ok(Step::NeedAt(Stage::CardAsk, b.escape("不响应").build()))
}

/// 过河拆桥 / 顺手牵羊：选目标一张牌 → 弃置 / 获得。
fn step_steal(
    g: &mut Game,
    actor: usize,
    def: &CardDef,
    target: Option<usize>,
    d: &mut Map<String, Value>,
    _esub: &str,
) -> Result<Step, String> {
    let card = sval(d, "card").unwrap_or_default();
    let Some(target) = target else { return Ok(finish_and_done(g, &card)) };
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("c:") {
            let ok = matches!(
                g.cards.get(id),
                Some(Zone::Hand(s)) | Some(Zone::Equip(s, _)) | Some(Zone::Judge(s)) if *s == target
            );
            if !ok {
                return Err("只能选择目标区域内的牌".into());
            }
            match def.effect {
                Effect::Dismantle => remove_card(g, target, id, "过河拆桥"),
                _ => {
                    flow::gain(g, actor, id, actor, "顺手牵羊");
                    // 判定区被拿走：从 LIFO 栈剔除。
                    if let Some(stack) = g.judge_stacks.get_mut(&target) {
                        stack.retain(|c| c != id);
                    }
                }
            }
            return Ok(finish_and_done(g, &card));
        } else if ans == "pass" {
            return Ok(finish_and_done(g, &card));
        }
    }
    let cards = target_cards(g, target);
    if cards.is_empty() {
        return Ok(finish_and_done(g, &card));
    }
    let mut b = PendingBuilder::new(actor, "sgs3_pick").skill("flow:card");
    for id in cards {
        b = b.option(format!("c:{id}"), format!("选择【{}】", card_name(g, &id)));
    }
    Ok(Step::NeedAt(Stage::CardPick, b.escape("不选择").build()))
}

/// 五谷丰登：亮出 N 张（N = 存活人数），依次选一张。
fn step_harvest(
    g: &mut Game,
    actor: usize,
    card: &str,
    d: &mut Map<String, Value>,
    _esub: &str,
) -> Result<Step, String> {
    if d.get("revealed").is_none() {
        let n = g.alive_seats().len();
        let mut revealed = vec![];
        for _ in 0..n {
            let Some(id) = g.deck.pop() else { break };
            g.cards.insert(id.clone(), Zone::Processing);
            let mut ev = Event::card(
                EventKind::CardRevealed,
                Some(actor),
                id.clone(),
                Some("五谷丰登".to_string()),
            );
            ev.by = Some(actor);
            let e = ev.snapshot(g);
            g.push_event(e);
            revealed.push(id);
        }
        d.insert("revealed".into(), json!(revealed));
        d.insert("hidx".into(), json!(0));
    }
    let order = vec![actor]
        .into_iter()
        .chain(other_alive_from(g, actor))
        .collect::<Vec<usize>>();
    let hidx = uid(d, "hidx");
    let mut revealed: Vec<String> = d
        .get("revealed")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if hidx >= order.len() || revealed.is_empty() {
        for id in revealed {
            discard_processed(g, &id, "五谷丰登");
        }
        return Ok(finish_and_done(g, card));
    }
    let picker = order[hidx];
    // 只有「收到本层的选择」才推进到下一个选牌者；否则继续问当前 picker。
    let mut answered = false;
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("c:") {
            if !revealed.iter().any(|c| c == id) {
                return Err("五谷丰登：请选择亮出的牌".into());
            }
            flow::gain(g, picker, id, picker, "五谷丰登");
            revealed.retain(|c| c != id);
            answered = true;
        }
    }
    let next_idx = if answered { hidx + 1 } else { hidx };
    if revealed.is_empty() || next_idx >= order.len() {
        for id in revealed {
            discard_processed(g, &id, "五谷丰登");
        }
        d.insert("revealed".into(), json!([]));
        d.insert("hidx".into(), json!(order.len()));
        return Ok(finish_and_done(g, card));
    }
    d.insert("revealed".into(), json!(revealed.clone()));
    d.insert("hidx".into(), json!(next_idx));
    let mut b = PendingBuilder::new(order[next_idx], "sgs3_harvest").skill("flow:card");
    for id in &revealed {
        b = b.option(format!("c:{id}"), format!("取走【{}】", card_name(g, id)));
    }
    Ok(Step::NeedWith(
        Stage::CardPick,
        Value::Object(d.clone()),
        b.escape_mandatory("选一张").build(),
    ))
}

/// 火攻：目标展示一张手牌 → 使用者可弃同花色手牌造成 1 点火焰伤害。
fn step_fire_attack(
    g: &mut Game,
    actor: usize,
    _def: &CardDef,
    target: Option<usize>,
    d: &mut Map<String, Value>,
    esub: &str,
) -> Result<Step, String> {
    let card = sval(d, "card").unwrap_or_default();
    let Some(target) = target else { return Ok(finish_and_done(g, &card)) };
    if esub != "discard" {
        if let Some(ans) = take_answer(g, "flow:card") {
            if let Some(id) = ans.strip_prefix("r:") {
                let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == target);
                if !ok {
                    return Err("火攻：目标只能展示自己的手牌".into());
                }
                d.insert("shown".into(), json!(id));
                d.insert("esub".into(), json!("discard"));
                return Ok(next(Stage::CardResolve, d.clone()));
            }
        }
        let hand = g.hand(target);
        if hand.is_empty() {
            return Ok(finish_and_done(g, &card));
        }
        let mut b = PendingBuilder::new(target, "fire_attack_reveal").skill("flow:card");
        for id in hand {
            b = b.option(format!("r:{id}"), format!("展示【{}】", card_name(g, &id)));
        }
        return Ok(Step::NeedAt(
            Stage::CardPick,
            b.escape_mandatory("展示一张手牌").build(),
        ));
    }
    // esub == "discard"
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("s:") {
            let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == actor);
            if !ok {
                return Err("火攻：只能弃置自己的手牌".into());
            }
            flow::discard(g, actor, id, "火攻");
            let spec = DamageSpec {
                from: Some(actor),
                target,
                amount: 1,
                kind: DamageKind::Fire,
                source: SourceKind::Other,
            };
            flow::damage(g, spec, super::gear::armor_of(g, target), 0);
            return Ok(finish_and_done(g, &card));
        } else if ans == "pass" {
            return Ok(finish_and_done(g, &card));
        }
    }
    let shown_suit = d
        .get("shown")
        .and_then(|v| v.as_str())
        .and_then(|id| g.card_defs.get(id))
        .map(|c| c.suit);
    let cards: Vec<String> = g
        .hand(actor)
        .into_iter()
        .filter(|id| {
            g.card_defs
                .get(id)
                .map(|c| Some(c.suit) == shown_suit)
                .unwrap_or(false)
        })
        .collect();
    if cards.is_empty() {
        return Ok(finish_and_done(g, &card));
    }
    let mut b = PendingBuilder::new(actor, "fire_attack_discard").skill("flow:card");
    for id in cards {
        b = b.option(format!("s:{id}"), format!("弃置【{}】造成火焰伤害", card_name(g, &id)));
    }
    Ok(Step::NeedAt(Stage::CardPick, b.escape("不弃牌").build()))
}

/// 借刀杀人：目标 A 对 B 使用【杀】，否则把武器交给使用者。
fn step_collateral(
    g: &mut Game,
    actor: usize,
    _def: &CardDef,
    d: &mut Map<String, Value>,
    esub: &str,
) -> Result<Step, String> {
    let card = sval(d, "card").unwrap_or_default();
    let targets = targets_of(d);
    if targets.len() < 2 {
        return Ok(finish_and_done(g, &card));
    }
    let (a, b) = (targets[0], targets[1]);
    if esub != "hit" {
        if let Some(ans) = take_answer(g, "flow:card") {
            if let Some(id) = ans.strip_prefix("s:") {
                let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == a)
                    && def_of(g, id).map(|c| c.tags.sha).unwrap_or(false);
                if !ok {
                    return Err("借刀杀人：只能使用手牌中的【杀】".into());
                }
                played(g, a, id, "借刀杀人")?;
                d.insert("esub".into(), json!("hit"));
                d.insert("ti".into(), json!(0));
                return Ok(next(Stage::CardResolve, d.clone()));
            } else if ans == "pass" {
                // 不出杀 → 把武器交给使用者（不是交给自己）。
                if let Some(w) = super::gear::equip_in(g, a, Slot::Weapon).cloned() {
                    flow::gain(g, actor, &w, a, "借刀杀人");
                }
                return Ok(finish_and_done(g, &card));
            }
        }
        let shas: Vec<String> = g
            .hand(a)
            .into_iter()
            .filter(|id| def_of(g, id).map(|c| c.tags.sha).unwrap_or(false))
            .collect();
        if shas.is_empty() {
            if let Some(w) = super::gear::equip_in(g, a, Slot::Weapon).cloned() {
                flow::gain(g, a, &w, a, "借刀杀人");
            }
            return Ok(finish_and_done(g, &card));
        }
        let mut p = PendingBuilder::new(a, "collateral_sha").skill("flow:card");
        for id in shas {
            p = p.option(format!("s:{id}"), format!("对 {} 号位使用【杀】", b + 1));
        }
        return Ok(Step::NeedAt(Stage::CardAsk, p.escape("不出杀（交出武器）").build()));
    }
    // esub == "hit"：B 出【闪】或受伤（来源 = A）。
    if let Some(ans) = take_answer(g, "flow:card") {
        if let Some(id) = ans.strip_prefix("s:") {
            let ok = matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == b)
                && def_of(g, id).map(|c| c.tags.shan).unwrap_or(false);
            if !ok {
                return Err("借刀杀人：目标只能打出【闪】".into());
            }
            played(g, b, id, "闪")?;
            return Ok(finish_and_done(g, &card));
        } else if ans == "pass" {
            let spec = DamageSpec {
                from: Some(a),
                target: b,
                amount: 1,
                kind: DamageKind::Normal,
                source: SourceKind::Slash,
            };
            flow::damage(g, spec, super::gear::armor_of(g, b), 0);
            return Ok(finish_and_done(g, &card));
        }
    }
    let shan: Vec<String> = g
        .hand(b)
        .into_iter()
        .filter(|id| def_of(g, id).map(|c| c.tags.shan).unwrap_or(false))
        .collect();
    if shan.is_empty() {
        let spec = DamageSpec {
            from: Some(a),
            target: b,
            amount: 1,
            kind: DamageKind::Normal,
            source: SourceKind::Slash,
        };
        flow::damage(g, spec, super::gear::armor_of(g, b), 0);
        return Ok(finish_and_done(g, &card));
    }
    let mut p = PendingBuilder::new(b, "collateral_shan").skill("flow:card");
    for id in shan {
        p = p.option(format!("s:{id}"), "打出【闪】");
    }
    Ok(Step::NeedAt(Stage::CardAsk, p.escape("不闪避").build()))
}

// ---------------------------------------------------------------------------
// 濒死救援流程（通用，所有「致濒死」渠道共用）
// ---------------------------------------------------------------------------

fn step_dying(g: &mut Game, data: &Value) -> Result<Step, String> {
    let target = data.get("target").and_then(|v| v.as_u64()).ok_or("濒死帧缺 target")? as usize;
    let killer = data.get("by").and_then(|v| v.as_u64()).map(|v| v as usize);
    let idx = data.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    // 已脱险（被桃/酒/技能救回）：先执行「濒死结算结束后」的延后结算，再收口。
    if !flow::is_dying(&g.seats[target]) {
        flow::settle_after_dying(g);
        return Ok(Step::Done);
    }
    let order = dying_order(g, target);
    if idx >= order.len() {
        let evs = super::turn::settle_death(g, target, killer);
        let _ = evs;
        // 即使濒死者死亡，求桃支援者的代价仍要结算（用户 2026-11-15 裁定）。
        flow::settle_after_dying(g);
        return Ok(Step::Done);
    }
    let asker = order[idx];
    if let Some(ans) = take_answer(g, "dying") {
        if let Some(card) = ans.strip_prefix("card:") {
            let in_hand = matches!(g.cards.get(card), Some(Zone::Hand(s)) if *s == asker);
            let def = def_of(g, card).ok();
            let is_tao = def.map(|d| d.tags.tao).unwrap_or(false);
            let is_jiu = def.map(|d| d.tags.jiu).unwrap_or(false) && asker == target;
            if !in_hand || !(is_tao || is_jiu) {
                return Err("濒死救援：只能使用【桃】（濒死者本人可用【酒】）".into());
            }
            flow::discard(g, asker, card, "濒死救援");
            flow::rescue(g, target, 1, if is_tao { "桃" } else { "酒" });
            if !flow::is_dying(&g.seats[target]) {
                return Ok(Step::Done);
            }
        } else if ans != "pass" {
            return Err(format!("濒死救援：未知选项 {ans}"));
        }
        return Ok(Step::Next(
            Stage::DyingAsk,
            json!({ "target": target, "by": killer, "idx": idx + 1 }),
        ));
    }
    let mut b = PendingBuilder::new(asker, "dying").skill("dying");
    let mut any = false;
    for id in g.hand(asker) {
        if g.is_frozen(asker, &id) {
            continue;
        }
        let def = match def_of(g, &id) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if def.tags.tao {
            b = b.option(format!("card:{id}"), "使用【桃】");
            any = true;
        } else if def.tags.jiu && asker == target {
            b = b.option(format!("card:{id}"), "使用【酒】自救");
            any = true;
        }
    }
    let _ = any;
    let p = b
        .escape("不出桃")
        .data(json!({ "target": target, "idx": idx }))
        .build();
    Ok(Step::Need(p))
}

/// 濒死求援顺序（mechanics `dying-aftermath`）：默认为自己 → 下家；
/// 兴衰式技能的求桃顺序改为**左手侧 → 自己最后**（用户 2026-11-06 裁定）。
pub fn dying_order_with(g: &Game, target: usize, left_first: bool) -> Vec<usize> {
    let n = g.seats.len();
    let step = |k: usize| (target + k) % n;
    let mut out = Vec::new();
    if left_first {
        for k in 1..n {
            let s = step(k);
            if s != target && g.seats[s].alive && g.seats[s].hp > 0 {
                out.push(s);
            }
        }
        out.push(target);
    } else {
        for k in 0..n {
            let s = step(k);
            if g.seats[s].alive && (g.seats[s].hp > 0 || s == target) {
                out.push(s);
            }
        }
    }
    out
}

/// 濒死求援顺序单入口（读技能钩子）。
pub fn dying_order(g: &Game, target: usize) -> Vec<usize> {
    dying_order_with(g, target, super::skills::dying_left_first(g, target))
}

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

fn card_name(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| "?".into())
}

/// 自己的手牌 + 装备区牌（贯石斧/寒冰剑等弃置候选）。
fn own_cards(g: &Game, seat: usize) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Hand(s) | Zone::Equip(s, _) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

/// 把目标区域的一张牌移出（弃置/获得/弃置判定区牌）：走事件 + 判定区栈维护。
pub fn remove_card(g: &mut Game, owner: usize, id: &str, cause: &str) {
    let zone = g.cards.get(id).cloned();
    match zone {
        Some(Zone::Hand(_)) | Some(Zone::Equip(..)) => {
            flow::discard(g, owner, id, cause);
        }
        Some(Zone::Judge(_)) => {
            to_discard(g, id);
            if let Some(stack) = g.judge_stacks.get_mut(&owner) {
                stack.retain(|c| c != id);
            }
            let mut ev = Event::card(
                EventKind::JudgeZoneLeft,
                Some(owner),
                id.to_string(),
                Some(cause.to_string()),
            );
            ev.by = Some(owner);
            let e = ev.snapshot(g);
            g.push_event(e);
        }
        _ => to_discard(g, id),
    }
}

/// 「打出」：手牌 → 弃牌堆 + `CardPlayed` 事件（不计入使用次数，但计入掣政「使用过的牌数」）。
fn played(g: &mut Game, seat: usize, id: &str, cause: &str) -> Result<(), String> {
    if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == seat) {
        return Err(format!("{id} 不在座位 {seat} 的手牌"));
    }
    // 冻结 = 不能主动使用/打出（mechanics `hand-freeze`；单入口防御）。
    if g.is_frozen(seat, id) {
        return Err(format!("{id} 已被冻结，不能打出"));
    }
    g.cards.insert(id.to_string(), Zone::Discard);
    g.bump(&format!("turn.{seat}.cards_used"));
    let mut lost = Event::card(EventKind::CardLost, Some(seat), id.to_string(), Some(cause.to_string()));
    lost.by = Some(seat);
    lost.from = Some(seat);
    let e = lost.snapshot(g);
    g.push_event(e);
    let mut ev = Event::card(EventKind::CardPlayed, Some(seat), id.to_string(), Some(cause.to_string()));
    ev.by = Some(seat);
    ev.from = Some(seat);
    let e = ev.snapshot(g);
    g.push_event(e);
    Ok(())
}

/// 处理区的牌进弃牌堆并 emit `CardDiscarded`（五谷剩余牌等「非弃置来源」的公开落盘）。
fn discard_processed(g: &mut Game, id: &str, cause: &str) {
    to_discard(g, id);
    let ev = Event::card(
        EventKind::CardDiscarded,
        None,
        id.to_string(),
        Some(cause.to_string()),
    );
    let e = ev.snapshot(g);
    g.push_event(e);
}

/// 废除判定区（雄乱）：判定区所有牌置入弃牌堆（离开判定区 != 弃置），并标记废除。
pub fn abolish_judge_zone(g: &mut Game, seat: usize) {
    let stack = g.judge_stacks.remove(&seat).unwrap_or_default();
    for id in stack {
        to_discard(g, &id);
        let mut ev = Event::card(
            EventKind::JudgeZoneLeft,
            Some(seat),
            id,
            Some("判定区废除".to_string()),
        );
        ev.by = Some(seat);
        let e = ev.snapshot(g);
        g.push_event(e);
    }
    g.abolish_judge_zone(seat);
}

/// 装备区牌当响应牌打出（解围①：装备牌当【无懈可击】）。
fn play_from_equip(g: &mut Game, seat: usize, id: &str, cause: &str) -> Result<(), String> {
    if !matches!(g.cards.get(id), Some(Zone::Equip(s, _)) if *s == seat) {
        return Err(format!("{id} 不在座位 {seat} 的装备区"));
    }
    g.cards.insert(id.to_string(), Zone::Discard);
    let mut lost = Event::card(EventKind::CardLost, Some(seat), id.to_string(), Some(cause.to_string()));
    lost.by = Some(seat);
    lost.from = Some(seat);
    let e = lost.snapshot(g);
    g.push_event(e);
    flow::leave_equip(g, seat, id, cause);
    let mut ev = Event::card(EventKind::CardPlayed, Some(seat), id.to_string(), Some(cause.to_string()));
    ev.by = Some(seat);
    ev.from = Some(seat);
    let e = ev.snapshot(g);
    g.push_event(e);
    Ok(())
}

/// 从手牌**使用装备牌**（技能驱动入口：据守④「弃置的装备牌改为使用」等）。
pub fn equip_from_hand(g: &mut Game, actor: usize, card: &str) -> Result<(), String> {
    let def = def_of(g, card)?;
    if def.slot.is_none() {
        return Err("不是装备牌".into());
    }
    if let Some(slot) = def.slot {
        if g.slot_abolished(actor, slot) {
            return Err("该装备栏已被废除，不可再装备".into());
        }
    }
    equip_card(g, actor, card, &def)?;
    finish_card(g, card);
    Ok(())
}

/// 结算收尾：牌从处理区落盘（虚拟牌进 `Void`；装备/判定区已落位则不动）。
fn finish_card(g: &mut Game, card: &str) {
    // 本次使用事实只作用于本次（禁止跨牌残留）。
    g.clear_use_opts(card);
    if card.starts_with("v:") {
        g.cards.insert(card.to_string(), Zone::Void);
        // 转化牌：承载牌（origin）随结算完成整组进弃牌堆（不得提前逐张弃置）。
        if let Some(origins) = g.virtual_origins.remove(card) {
            for id in origins {
                g.cards.insert(id.clone(), Zone::Discard);
                let ev = Event::card(
                    EventKind::CardDiscarded,
                    None,
                    id,
                    Some("转化牌结算完成".to_string()),
                );
                let e = ev.snapshot(g);
                g.push_event(e);
            }
        }
        // 虚拟牌实例保留（区域 = Void）：守恒要求「实例 ↔ 区域」同时存在。
        return;
    }
    if matches!(g.cards.get(card), Some(Zone::Processing) | Some(Zone::Frame(_))) {
        g.cards.insert(card.to_string(), Zone::Discard);
    }
}

fn finish_and_done(g: &mut Game, card: &str) -> Step {
    finish_card(g, card);
    Step::Done
}

fn finish_done(g: &mut Game, d: &Map<String, Value>) -> Result<Step, String> {
    let card = sval(d, "card").unwrap_or_default();
    Ok(finish_and_done(g, &card))
}


// ---------------------------------------------------------------------------
// 虚拟牌 / 转化牌 / 牌堆承载使用（mechanics `virtual-card` / `deck-carrier`）
// ---------------------------------------------------------------------------

/// 视为使用一张牌（**无承载牌**）：勇略/司敌式「视为使用一张【杀】」。
///
/// 走**同一套真实结算**（目标响应/防具/属性/伤害），只是没有物理承载牌。
pub fn use_virtual(
    g: &mut Game,
    actor: usize,
    def_code: &str,
    targets: &[usize],
    fire: bool,
) -> Result<(), String> {
    push_virtual_use(g, actor, def_code, Vec::new(), targets, fire)
}

/// 转化使用（**带承载牌 origin**）：奇策/活墨/陷嗣式。
///
/// 物理牌区域不变（只改生效定义）；承载牌在结算完成后整组进弃牌堆（不得提前逐张弃置）。
pub fn use_as(
    g: &mut Game,
    actor: usize,
    def_code: &str,
    origin: &[CardId],
    targets: &[usize],
    fire: bool,
) -> Result<(), String> {
    if origin.is_empty() {
        return Err("转化牌必须声明承载牌（origin）".into());
    }
    for id in origin {
        if !matches!(g.cards.get(id), Some(Zone::Hand(s)) if *s == actor) {
            return Err(format!("承载牌 {id} 不在座位 {actor} 的手牌"));
        }
        if g.is_frozen(actor, id) {
            return Err(format!("承载牌 {id} 已被冻结"));
        }
    }
    // 承载牌离开手牌（进处理区；结算完成落弃牌堆）：区域仍唯一归属。
    for id in origin {
        g.cards.insert(id.clone(), Zone::Processing);
        let mut lost = Event::card(
            EventKind::CardLost,
            Some(actor),
            id.clone(),
            Some("转化使用".to_string()),
        );
        lost.by = Some(actor);
        lost.from = Some(actor);
        let e = lost.snapshot(g);
        g.push_event(e);
    }
    push_virtual_use(g, actor, def_code, origin.to_vec(), targets, fire)
}

fn push_virtual_use(
    g: &mut Game,
    actor: usize,
    def_code: &str,
    origin: Vec<CardId>,
    targets: &[usize],
    fire: bool,
) -> Result<(), String> {
    let def = cards::def(def_code).ok_or_else(|| format!("虚拟牌定义不存在：{def_code}"))?;
    let (suit, rank) = origin
        .first()
        .and_then(|id| g.card_defs.get(id))
        .map(|c| (c.suit, c.rank))
        .unwrap_or((super::Suit::Spade, 1));
    let vid = format!("v:m{}", g.next_frame_id);
    g.next_frame_id += 1;
    g.card_defs.insert(
        vid.clone(),
        super::Card {
            id: vid.clone(),
            def: def_code.to_string(),
            suit,
            rank,
        },
    );
    g.cards.insert(vid.clone(), Zone::Processing);
    if !origin.is_empty() {
        g.virtual_origins.insert(vid.clone(), origin);
    }
    g.bump(&format!("turn.{actor}.cards_used"));
    if def.tags.sha {
        g.bump(&format!("phase.sha_used.{actor}"));
    }
    if def.tags.jiu {
        g.bump(&format!("phase.jiu_used.{actor}"));
    }
    let mut used = Event::card(
        EventKind::CardUsed,
        Some(actor),
        vid.clone(),
        Some(if def_code.is_empty() { "视为使用".to_string() } else { "转化使用".to_string() }),
    );
    used.by = Some(actor);
    let e = used.snapshot(g);
    g.push_event(e);
    super::skills::fire_use_pre(g, actor, &vid);
    super::resolve::push_flow(
        g,
        FlowKind::Card,
        actor,
        Stage::CardTarget,
        json!({
            "card": vid,
            "actor": actor,
            "fire": fire,
            "started": true,
            "sub": "target",
            "targets": targets,
        }),
    );
    Ok(())
}

/// 以牌堆/弃牌堆中的一张牌为**承载牌**、按另一张牌的定义使用（慎断：黑色基本牌当【兵粮寸断】）。
pub fn use_from_pile_as(
    g: &mut Game,
    actor: usize,
    carrier: &str,
    def_code: &str,
    targets: &[usize],
) -> Result<(), String> {
    let zone = g
        .cards
        .get(carrier)
        .cloned()
        .ok_or_else(|| format!("牌 {carrier} 不存在"))?;
    if !matches!(zone, Zone::Draw | Zone::Discard) {
        return Err("承载牌必须来自牌堆/弃牌堆".into());
    }
    if cards::def(def_code).is_none() {
        return Err(format!("未知牌定义：{def_code}"));
    }
    g.deck.retain(|c| c != carrier);
    g.cards.insert(carrier.to_string(), Zone::Processing);
    push_virtual_use(g, actor, def_code, vec![carrier.to_string()], targets, false)
}

/// 使用牌堆/弃牌堆中的一张牌（mechanics `deck-carrier`）。
///
/// 承载牌必须是公共可验证的牌区牌（Draw/Discard）；走真实结算，结算后进弃牌堆。
pub fn use_from_pile(
    g: &mut Game,
    actor: usize,
    card: &str,
    targets: &[usize],
) -> Result<(), String> {
    let zone = g
        .cards
        .get(card)
        .cloned()
        .ok_or_else(|| format!("牌 {card} 不存在"))?;
    if !matches!(zone, Zone::Draw | Zone::Discard) {
        return Err("只能用牌堆/弃牌堆中的牌作为承载牌".into());
    }
    if let Zone::Draw = zone {
        g.deck.retain(|c| c != card);
    }
    g.cards.insert(card.to_string(), Zone::Processing);
    let mut used = Event::card(
        EventKind::CardUsed,
        Some(actor),
        card.to_string(),
        Some("承载使用".to_string()),
    );
    used.by = Some(actor);
    let e = used.snapshot(g);
    g.push_event(e);
    g.bump(&format!("turn.{actor}.cards_used"));
    if let Ok(def) = def_of(g, card) {
        if def.tags.sha {
            g.bump(&format!("phase.sha_used.{actor}"));
        }
    }
    super::skills::fire_use_pre(g, actor, card);
    super::resolve::push_flow(
        g,
        FlowKind::Card,
        actor,
        Stage::CardTarget,
        json!({
            "card": card,
            "actor": actor,
            "fire": false,
            "started": true,
            "sub": "target",
            "targets": targets,
        }),
    );
    Ok(())
}

/// 声明「本次不能被【无懈可击】抵消」（技能入口；只作用于本次使用）。
pub fn declare_no_wuxie(g: &mut Game, card: &str) {
    g.use_opts_mut(card).no_wuxie = true;
}

/// 声明「本次不能被响应」（闪/杀响应整段跳过）。
pub fn declare_no_response(g: &mut Game, card: &str) {
    g.use_opts_mut(card).no_response = true;
}

/// 声明「本次无距离限制」。
pub fn declare_no_distance(g: &mut Game, card: &str) {
    g.use_opts_mut(card).no_distance = true;
}

/// 声明目标数 +N（必须同时是合法目标）。
pub fn declare_extra_targets(g: &mut Game, card: &str, n: i32) {
    g.use_opts_mut(card).extra_targets = n.max(0);
}

/// 声明群体锦囊少选 N 个目标（在无懈/响应之前完成）。
pub fn declare_trim_targets(g: &mut Game, card: &str, n: i32) {
    g.use_opts_mut(card).trim_targets = n.max(0);
}

/// 声明本次距离口径覆盖（烈弓：距离 ≤ 此牌点数即视为可达）。
pub fn declare_range_override(g: &mut Game, card: &str, range: i32) {
    g.use_opts_mut(card).range_override = Some(range.max(0));
}

/// 声明响应计数 N（无双：需连续 N 张同类响应）。
pub fn declare_multi_response(g: &mut Game, card: &str, n: u32) {
    let v = g.use_opts_mut(card);
    v.multi_response = n.max(1);
}

/// 【杀】不计入本阶段使用次数（龙吟）：已计过的回退，并打上本次标记。
///
/// `owner` = 该【杀】的使用者（次数计数在 `phase.sha_used.<owner>`）。
pub fn declare_sha_not_counted(g: &mut Game, owner: usize, card: &str) {
    if g.use_opts(card).sha_not_counted {
        return;
    }
    g.use_opts_mut(card).sha_not_counted = true;
    let k = format!("phase.sha_used.{owner}");
    let v = g.counter(&k);
    g.set_counter(&k, (v - 1).max(0));
}

/// 声明【杀】结算前介入结果（目标锁定技；`None` = 不介入）。
pub fn declare_intercept(g: &mut Game, card: &str, result: super::ShaIntercept) {
    let v = g.use_opts_mut(card);
    v.intercept = Some(result);
    v.intercept_checked = true;
}
