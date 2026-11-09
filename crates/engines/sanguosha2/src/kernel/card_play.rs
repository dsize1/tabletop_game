//! 卡牌结算内核（M1/M2）：**使用 → 目标 → 响应 → 结算 → 弃置/装备/判定区** 单一流程。
//!
//! 设计约束（`memory_palace/plan/next-engine-replacement.md` §M1 / 纪律）：
//! 1. 牌结算**只能**经 `flow::damage*` / `flow::lose_hp` / `flow::heal` 改体力；本文件不得出现 `hp +=` / `hp -=`。
//! 2. 牌名/技能名**不得**进入判定：分派只读 `cards::def()` 的属性（`kind` / `tags` / `effect` /
//!    `damage_kind` / `armor` / `weapon` / `target_mode`），响应读 `ResponseKind`。
//! 3. 新增状态必须进「牌区不变量」（守恒 160 / 无帧遗留）。
//! 4. 入口分三类（M2 起技能全部走这里，`Game::use_card` 只保留为「事件模拟」兼容）：
//!    - `resolve_use`：普通使用（手牌 + 自己出牌阶段）；
//!    - `start_virtual`：纯虚拟牌（无承载牌，如「视为使用【杀】」）；
//!    - `start_carrier`：转化牌（实体承载 + 生效牌名，如奇策/战绝/滔乱/慎断）。
//!
//! 流程结构：一张牌 = 一个 `Frame::Flow(FlowKind::Card)`；多目标/多阶段用 `data` 里的
//! `cursor` / `asked` / `step` 推进，**不**由技能压弹栈。无懈链复用 `flow::start_wuxie`，
//! 其结果经 `Game::wuxie_result`（子流程唯一结果通道）回传父流程。

use serde_json::{json, Value};

use super::event::{Event, EventKind};
use super::resolve::{Ask, FlowFrame, FlowKind, Frame, Stage, Step};
use super::{CardId, Game, Phase, Slot, Suit, Zone};
use crate::cards::{self, ArmorKind, CardKind, Effect, ResponseKind};
use crate::kernel::flow;
use crate::presentation::{POutcome, Presentation};
use crate::skills;

// ---------------------------------------------------------------------------
// 入口：legal 与 execute 同源
// ---------------------------------------------------------------------------

/// 使用选项（技能转化牌与普通使用的唯一差异集中在这里）。
#[derive(Debug, Clone, Copy, Default)]
pub struct UseOpts {
    /// 严格校验：回合/阶段/手牌/冻结/【杀】次数/攻击范围（普通使用与出牌阶段转化牌）。
    pub strict: bool,
    /// 伤害来源覆盖（勤王：代出的【杀】伤害来源视为刘谌）。
    pub damage_source: Option<usize>,
    /// 不能被【无懈可击】抵消（机制 `no-wuxie`；如离间「不能被无懈可击抵消」）。
    /// 只关无懈窗口，不影响该牌的其它响应（【决斗】仍需出【杀】）。
    pub no_wuxie: bool,
}

impl UseOpts {
    /// 普通使用（手牌 + 自己出牌阶段）。
    pub fn normal() -> Self {
        Self {
            strict: true,
            damage_source: None,
            no_wuxie: false,
        }
    }

    /// 技能驱动的「视为使用」（不检查回合/阶段/手牌；目标由技能自行保证）。
    pub fn skill() -> Self {
        Self {
            strict: false,
            damage_source: None,
            no_wuxie: false,
        }
    }

    /// 出牌阶段的转化牌：严格校验（次数/距离），但伤害来源可覆盖。
    pub fn carrier() -> Self {
        Self {
            strict: true,
            damage_source: None,
            no_wuxie: false,
        }
    }

    pub fn with_damage_source(mut self, seat: usize) -> Self {
        self.damage_source = Some(seat);
        self
    }

    pub fn with_no_wuxie(mut self) -> Self {
        self.no_wuxie = true;
        self
    }
}

fn plan_use(
    g: &Game,
    seat: usize,
    carrier: Option<&CardId>,
    name: &str,
    targets: &[usize],
    opts: UseOpts,
) -> Result<(Vec<usize>, cards::CardDef), String> {
    if g.finished {
        return Err("对局已结束".into());
    }
    let def = cards::def(name);
    if !def.known {
        return Err(format!("未知牌名：{name}"));
    }
    if def.tags.shan {
        return Err("【闪】只能用于响应".into());
    }
    if def.tags.wuxie {
        return Err("【无懈可击】只能用于响应".into());
    }
    if (def.kind == CardKind::Equip || def.tags.delayed) && carrier.is_none() {
        return Err("该牌需要实体牌承载".into());
    }
    if opts.strict {
        let Some(card) = carrier else {
            return Err("普通使用需要实体牌".into());
        };
        match g.zone_of(card) {
            Some(Zone::Hand(s)) if *s == seat => {}
            _ => return Err("只能使用自己的手牌".into()),
        }
        if !g.can_use_card(seat, card) {
            return Err("该牌已被冻结，本回合不能使用".into());
        }
        if g.turn != seat {
            return Err("不是你的回合".into());
        }
        if g.phase != Phase::Play {
            return Err("只能在自己的出牌阶段使用".into());
        }
    } else if let Some(card) = carrier {
        // 技能转化牌：手牌（可打出）或「弃牌堆里的虚拟承载牌」（慎断）。
        match g.zone_of(card) {
            Some(Zone::Hand(s)) if *s == seat => {
                if !g.can_use_card(seat, card) {
                    return Err("该牌已被冻结，本回合不能使用".into());
                }
            }
            // 弃牌堆里的承载牌（慎断）：技能负责合法性，这里只要求牌存在且未失效。
            Some(Zone::Discard) => {}
            // 牌堆里的承载牌（伏诛「使用牌堆中的【杀】」，mechanics deck-carrier）：
            // 合法性（张数上限/目标）由技能负责，内核只按真实牌结算。
            Some(Zone::Draw) => {}
            // 自己装备区的牌作为承载牌（界奇袭：黑色装备当过河拆桥）。
            Some(Zone::Equip(s, _)) if *s == seat => {}
            _ => return Err("转化牌不在可用区域".into()),
        }
    }
    let planned = planned_targets(g, seat, &def, name, targets, opts.strict, carrier)?;
    // 「本回合不能对自己使用牌」（才识②）：含装备牌（用户 2026-11-09 裁定）。
    if skills::self_use_blocked(g, seat)
        && (def.kind == CardKind::Equip || planned.contains(&seat))
    {
        return Err("本回合不能对自己使用牌".into());
    }
    Ok((planned, def))
}

/// 能否使用（给 `Game::can_resolve_use` / 未来 AI 枚举用）。
/// 枚举「本阶段可主动使用的牌」的具体动作（`use:{id}` / `use:{id}:{t1,t2}`）。
///
/// legal 与 execute 同源：每个候选都用 `plan_use` 复核。
pub fn legal_uses(g: &Game, seat: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let alive: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .collect();
    for id in g.hand(seat) {
        if !g.can_use_card(seat, &id) {
            continue;
        }
        let name = g.name_of(&id);
        let def = cards::def(&name);
        if def.tags.shan || def.tags.wuxie {
            continue;
        }
        if def.tags.lianhuan {
            out.push((format!("recast:{id}"), format!("重铸【{}】", g.card_label(&id))));
        }
        let target_sets: Vec<Vec<usize>> = match def.effect {
            Effect::Sha
            | Effect::Duel
            | Effect::Dismantle
            | Effect::Snatch
            | Effect::FireAttack
            | Effect::Indulgence
            | Effect::SupplyShortage => alive
                .iter()
                .filter(|t| **t != seat)
                .map(|t| vec![*t])
                .collect(),
            Effect::Collateral => {
                let mut sets = Vec::new();
                for a in alive.iter().filter(|t| **t != seat) {
                    if g.equip_in(*a, Slot::Weapon).is_none() {
                        continue;
                    }
                    for b in alive.iter().filter(|t| *t != a) {
                        sets.push(vec![*a, *b]);
                    }
                }
                sets
            }
            Effect::IronChain => {
                let mut sets: Vec<Vec<usize>> = alive.iter().map(|t| vec![*t]).collect();
                for (i, a) in alive.iter().enumerate() {
                    for b in alive.iter().skip(i + 1) {
                        sets.push(vec![*a, *b]);
                    }
                }
                sets
            }
            _ => vec![Vec::new()],
        };
        for targets in target_sets {
            if plan_use(g, seat, Some(&id), &name, &targets, UseOpts::normal()).is_ok() {
                let suffix = if targets.is_empty() {
                    String::new()
                } else {
                    format!(
                        ":{}",
                        targets
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                out.push((
                    format!("use:{id}{suffix}"),
                    format!("使用【{}】", g.card_label(&id)),
                ));
            }
        }
    }
    // 被动转化技（武圣/龙胆）：`convert:<skill>:<承载牌>:<生效牌名>[:目标]`
    for conv in skills::conversions(g, seat) {
        if !conv.usable {
            continue;
        }
        let def = cards::def(conv.to);
        for targets in target_sets_for(g, seat, &def) {
            if plan_use(g, seat, Some(&conv.from), conv.to, &targets, UseOpts::skill()).is_ok() {
                let suffix = if targets.is_empty() {
                    String::new()
                } else {
                    format!(
                        ":{}",
                        targets
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                out.push((
                    format!("convert:{}:{}:{}{suffix}", conv.skill, conv.from, conv.to),
                    format!("【{}】当【{}】", g.card_label(&conv.from), conv.to),
                ));
            }
        }
    }
    out
}

/// 某个牌定义的目标候选集（legal 枚举与转化牌共用）。
fn target_sets_for(g: &Game, seat: usize, def: &cards::CardDef) -> Vec<Vec<usize>> {
    let alive: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .collect();
    match def.effect {
        Effect::Sha
        | Effect::Duel
        | Effect::Dismantle
        | Effect::Snatch
        | Effect::FireAttack
        | Effect::Indulgence
        | Effect::SupplyShortage => alive
            .iter()
            .filter(|t| **t != seat)
            .map(|t| vec![*t])
            .collect(),
        Effect::Collateral => {
            let mut sets = Vec::new();
            for a in alive.iter().filter(|t| **t != seat) {
                if g.equip_in(*a, Slot::Weapon).is_none() {
                    continue;
                }
                for b in alive.iter().filter(|t| *t != a) {
                    sets.push(vec![*a, *b]);
                }
            }
            sets
        }
        Effect::IronChain => {
            let mut sets: Vec<Vec<usize>> = alive.iter().map(|t| vec![*t]).collect();
            for (i, a) in alive.iter().enumerate() {
                for b in alive.iter().skip(i + 1) {
                    sets.push(vec![*a, *b]);
                }
            }
            sets
        }
        _ => vec![Vec::new()],
    }
}

pub fn can_use(g: &Game, seat: usize, card: &CardId, targets: &[usize]) -> Result<(), String> {
    plan_use(
        g,
        seat,
        Some(card),
        &g.name_of(card),
        targets,
        UseOpts::normal(),
    )
    .map(|_| ())
}

/// 目标规则：读 `effect` / `target_mode`，**不**比较牌名。
fn planned_targets(
    g: &Game,
    seat: usize,
    def: &cards::CardDef,
    name: &str,
    given: &[usize],
    strict: bool,
    carrier: Option<&CardId>,
) -> Result<Vec<usize>, String> {
    // 目标数修正（福绵②）：只有红色基本牌/红色普通锦囊且是「自己使用」时生效。
    let extra = if g.turn == seat && g.phase == Phase::Play {
        skills::extra_targets(g, seat, carrier.filter(|c| g.is_red(c)))
    } else {
        0
    };
    let alive = |i: usize| g.seats.get(i).is_some_and(|s| s.hp > 0);
    // 注意：`seats_from_left_of` 末尾包含自己，这里要排除（目标列表不得重复）。
    let left_alive: Vec<usize> = g
        .seats_from_left_of(seat)
        .into_iter()
        .filter(|i| *i != seat && alive(*i))
        .collect();
    let many_others = |given: &[usize], extra: usize, what: &str| -> Result<Vec<usize>, String> {
        if given.is_empty() || given.len() > 1 + extra {
            return Err(format!("{what}目标数不合法"));
        }
        let mut out: Vec<usize> = Vec::new();
        for t in given {
            if *t == seat {
                return Err(format!("{what}不能以自己为目标"));
            }
            if !alive(*t) {
                return Err("目标已死亡".into());
            }
            if out.contains(t) {
                return Err("目标不能重复".into());
            }
            if !skills::can_target(g, seat, name, *t) {
                return Err("目标不能成为此牌目标".into());
            }
            out.push(*t);
        }
        Ok(out)
    };

    if def.kind == CardKind::Equip {
        return Ok(Vec::new());
    }
    match def.effect {
        Effect::Tao => {
            if g.seats[seat].hp >= g.seats[seat].max_hp {
                return Err("体力已满，不能使用【桃】".into());
            }
            if !given.is_empty() && (given.len() != 1 || given[0] != seat) {
                return Err("【桃】只能对自己使用".into());
            }
            Ok(vec![seat])
        }
        Effect::Jiu => {
            if g.counter(&format!("turn.jiu_used:{seat}")) > 0 {
                return Err("本回合已使用过【酒】".into());
            }
            if !given.is_empty() && (given.len() != 1 || given[0] != seat) {
                return Err("【酒】只能对自己使用".into());
            }
            Ok(vec![seat])
        }
        Effect::ExNihilo | Effect::Lightning => {
            if !given.is_empty() && (given.len() != 1 || given[0] != seat) {
                return Err("该牌只能对自己使用".into());
            }
            Ok(vec![seat])
        }
        Effect::PeachGarden | Effect::Harvest => {
            let mut v = vec![seat];
            v.extend(left_alive);
            Ok(v)
        }
        Effect::SavageAssault | Effect::ArcheryAttack => {
            let mut v = left_alive;
            // 乱击（aoe-target-trim）：少选一个目标。
            if let Some(t) = skills::aoe_trim_target(g, seat) {
                v.retain(|x| *x != t);
            }
            Ok(v)
        }
        Effect::Sha => {
            if given.len() != 1 && given.len() > 1 + extra {
                return Err("【杀】目标数不合法".into());
            }
            let many = many_others(given, extra, "【杀】")?;
            if strict {
                let Some(card) = carrier else {
                    return Err("【杀】需要实体牌承载".into());
                };
                // 多指定目标时「无距离限制」（福绵②）；界武圣的方片【杀】也无距离。
                let no_dist = extra > 0 || crate::skills::sha_no_distance(g, seat, card);
                if !no_dist && !g.can_reach(seat, many[0], card) {
                    return Err("目标不在攻击范围内".into());
                }
                if g.sha_used(seat) >= g.sha_limit(seat) {
                    return Err("本阶段【杀】使用次数已用完".into());
                }
            }
            Ok(many)
        }
        Effect::Duel | Effect::Dismantle | Effect::Snatch | Effect::FireAttack => {
            let many = many_others(given, extra, "该牌")?;
            for t in &many {
                if matches!(def.effect, Effect::Dismantle | Effect::Snatch) && !has_cards(g, *t) {
                    return Err("目标没有可操作的牌".into());
                }
            }
            // 多指定目标时「无距离限制」；否则按牌自身距离规则。
            if def.effect == Effect::Snatch
                && extra == 0
                && !crate::skills::trick_no_distance(g, seat)
                && g.distance(seat, many[0]) > 1
            {
                return Err("【顺手牵羊】只能对距离 1 的角色使用".into());
            }
            Ok(many)
        }
        Effect::Indulgence | Effect::SupplyShortage => {
            // 延时锦囊不参与「多指定目标」：一张物理牌只能进一个判定区。
            let many = many_others(given, 0, "该牌")?;
            for t in &many {
                if same_effect_in_judge(g, *t, def.effect) {
                    return Err("目标判定区已有同名延时锦囊".into());
                }
            }
            Ok(many)
        }
        Effect::Collateral => {
            if given.len() != 2 {
                return Err("【借刀杀人】需要指定 2 个目标".into());
            }
            let (a, b) = (given[0], given[1]);
            if a == b || a == seat {
                return Err("【借刀杀人】目标不合法".into());
            }
            if !alive(a) || !alive(b) {
                return Err("目标已死亡".into());
            }
            if g.equip_in(a, Slot::Weapon).is_none() {
                return Err("第一名目标需装备武器".into());
            }
            Ok(vec![a, b])
        }
        Effect::IronChain => {
            if given.is_empty() || given.len() > 2 {
                return Err("【铁索连环】需指定 1–2 个目标".into());
            }
            let mut seen = Vec::new();
            for t in given {
                if !alive(*t) {
                    return Err("目标已死亡".into());
                }
                if seen.contains(t) {
                    return Err("目标不能重复".into());
                }
                seen.push(*t);
            }
            Ok(given.to_vec())
        }
        _ => Err("该牌不能主动使用".into()),
    }
}

fn has_cards(g: &Game, seat: usize) -> bool {
    !g.hand(seat).is_empty()
        || !g.judge_zone(seat).is_empty()
        || [
            Slot::Weapon,
            Slot::Armor,
            Slot::HorseOffense,
            Slot::HorseDefense,
            Slot::Treasure,
        ]
        .iter()
        .any(|s| g.equip_in(seat, *s).is_some())
}

fn same_effect_in_judge(g: &Game, seat: usize, effect: Effect) -> bool {
    g.judge_zone(seat)
        .iter()
        .any(|id| g.def_of(id).effect == effect)
}

// ---------------------------------------------------------------------------
// 使用入口
// ---------------------------------------------------------------------------

/// 普通使用入口：使用一张手牌 → 推进到底（直到需要输入或结算完成）。
pub fn resolve_use(
    g: &mut Game,
    seat: usize,
    card: &CardId,
    targets: Vec<usize>,
) -> Result<(), String> {
    if g.pending.is_some() || !g.stack.is_empty() {
        return Err("请先完成当前结算".into());
    }
    let name = g.name_of(card);
    start_use(g, seat, Some(card), &name, targets, UseOpts::normal())?;
    super::resolve::advance(g)
}

/// 技能入口：纯虚拟牌（无承载牌，如「视为使用【杀】」）。
pub fn start_virtual(
    g: &mut Game,
    seat: usize,
    name: &str,
    targets: Vec<usize>,
) -> Result<(), String> {
    start_virtual_with(g, seat, name, targets, UseOpts::skill())
}

/// 纯虚拟牌 + 使用选项（离间的「不能被无懈可击抵消」）。
pub fn start_virtual_with(
    g: &mut Game,
    seat: usize,
    name: &str,
    targets: Vec<usize>,
    opts: UseOpts,
) -> Result<(), String> {
    start_use(g, seat, None, name, targets, opts)
}

/// 技能入口：转化牌（实体承载 + 生效牌名）。
///
/// 调用前若已 `set_virtual`（含 origins，如奇策/战绝），保留其 origins；否则默认单一承载牌。
pub fn start_carrier(
    g: &mut Game,
    seat: usize,
    carrier: &CardId,
    name: &str,
    targets: Vec<usize>,
    opts: UseOpts,
) -> Result<(), String> {
    // 先干跑校验（失败时不得留下虚拟定义/origins 残状态）。
    plan_use(g, seat, Some(carrier), name, &targets, opts)?;
    let origins = g
        .origins
        .get(carrier)
        .cloned()
        .unwrap_or_else(|| vec![carrier.clone()]);
    g.set_virtual(carrier, name, origins);
    start_use(g, seat, Some(carrier), name, targets, opts)
}

/// 唯一使用记账 + 压帧入口（**不**推进：由外层 `advance` 驱动）。
pub fn start_use(
    g: &mut Game,
    seat: usize,
    carrier: Option<&CardId>,
    name: &str,
    targets: Vec<usize>,
    opts: UseOpts,
) -> Result<(), String> {
    let (planned, def) = plan_use(g, seat, carrier, name, &targets, opts)?;

    // 使用记账（与 `Game::use_card` 同口径：事件 + 本回合使用数）。
    g.bump("turn.cards_used");
    if let Some(card) = carrier {
        if g.seats[seat].flags.contains("xiongluan_active") {
            g.mark_unresponsive(card);
        }
        g.events.push(Event::card(
            EventKind::CardUsed,
            Some(seat),
            card.clone(),
            None,
        ));
        g.used_this_turn.push((seat, card.clone()));
    } else {
        g.events.push(Event::simple(
            EventKind::CardUsed,
            Some(seat),
            Some(name),
        ));
        // 纯虚拟【杀】也占用本阶段次数（`sha_used` 派生时计入）。
        if def.tags.sha {
            g.bump(&format!("turn.sha_virtual:{seat}"));
        }
    }
    g.begin_action();
    // 福绵②：本回合「可以多指定目标」的机会用掉一次（即便只指定了 1 个目标）。
    if crate::skills::extra_targets(g, seat, carrier) > 0 {
        crate::skills::consume_extra_targets(g, seat);
    }
    if !planned.is_empty() {
        let targets_str = planned
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        match carrier {
            Some(card) => g.events.push(Event::card(
                EventKind::TargetChosen,
                Some(seat),
                card.clone(),
                Some(targets_str),
            )),
            None => g.events.push(Event::simple(
                EventKind::TargetChosen,
                Some(seat),
                Some(&targets_str),
            )),
        }
    }
    if let Some(card) = carrier {
        g.to_processing(card)?;
    }
    g.push_log(format!(
        "{} 使用【{}】{}",
        g.seats[seat].name,
        name,
        if planned.is_empty() {
            String::new()
        } else {
            format!(
                "（目标：{}）",
                planned
                    .iter()
                    .map(|t| g.seats[*t].name.clone())
                    .collect::<Vec<_>>()
                    .join("、")
            )
        }
    ));
    let mut pres = Presentation::new("card", POutcome::Played { label: name.to_string() })
        .actor(seat)
        .targets(planned.clone())
        .fx("card_use");
    if let Some(card) = carrier {
        pres = pres.cards(vec![card.clone()]);
    }
    g.presentation.push(pres);

    let data = json!({
        "card": carrier,
        "name": name,
        "targets": planned,
        "cursor": 0,
        "asked": false,
        "wuxie_started": false,
        "wuxie_done": false,
        "step": "start",
        "responder": planned.first().copied(),
        "opponent": seat,
        "damage_source": opts.damage_source,
        "no_wuxie": opts.no_wuxie,
    });
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::Card,
        skill: "__card".to_string(),
        actor: seat,
        stage: Stage::CardOpt,
        data,
    }));
    // 效果表必须覆盖所有可主动使用的牌（装备除外）。
    debug_assert!(def.effect != Effect::None || def.kind == CardKind::Equip);
    Ok(())
}

/// 重铸（铁索连环）：弃置该牌并摸 1，**不计入**「使用牌」。
pub fn resolve_recast(g: &mut Game, seat: usize, card: &CardId) -> Result<(), String> {
    if g.zone_of(card) != Some(&Zone::Hand(seat)) {
        return Err("只能重铸自己的手牌".into());
    }
    if !g.def_of(card).tags.lianhuan {
        return Err("只有【铁索连环】可以重铸".into());
    }
    if !g.can_use_card(seat, card) {
        return Err("该牌已被冻结".into());
    }
    if g.turn != seat || g.phase != Phase::Play {
        return Err("重铸只能在自己的出牌阶段进行".into());
    }
    g.to_discard(card, "recast")?;
    g.draw(seat, 1);
    g.push_log(format!("{} 重铸【{}】", g.seats[seat].name, g.name_of(card)));
    Ok(())
}

/// 【酒】的统一效果（普通使用与活墨共用）：本回合下一张【杀】伤害 +1，每回合一次。
pub fn apply_jiu(g: &mut Game, seat: usize) {
    *g.counters
        .entry(format!("turn.jiu_used:{seat}"))
        .or_insert(0) += 1;
    *g.counters.entry("turn.jiu_bonus".to_string()).or_insert(0) += 1;
    g.push_log(format!(
        "{} 使用【酒】：本回合下一张【杀】伤害 +1",
        g.seats[seat].name
    ));
}

// ---------------------------------------------------------------------------
// 推进器
// ---------------------------------------------------------------------------

pub fn step_card(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let card: Option<CardId> = a.data["card"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if let Some(c) = &card {
        if !g.cards.contains_key(c) {
            return Err(format!("牌结算：未知牌 {c}"));
        }
    }
    let name = a.data["name"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| card.as_ref().map(|c| g.name_of(c)))
        .ok_or_else(|| "牌结算：缺少牌名".to_string())?;
    let source = a.actor;
    let def = cards::def(&name);
    let card_ref = card.as_ref();
    let mut data = a.data.clone();
    let stage = a.stage;

    // 奋威：任何「目标列表为多人」的牌，指定目标后可令任意个目标无效。
    if !data["fenwei_done"].as_bool().unwrap_or(false) {
        data["fenwei_done"] = json!(true);
        let planned = target_list(&data);
        if planned.len() >= 2 && crate::skills::start_fenwei(g, &planned, source) {
            return Ok(Step::Next(stage, data));
        }
    }
    // 谦逊：普通锦囊仅以持有者为唯一目标时，可将其所有手牌移出游戏。
    if !data["qianxun_done"].as_bool().unwrap_or(false) {
        data["qianxun_done"] = json!(true);
        let planned = target_list(&data);
        if crate::skills::start_qianxun(g, source, &planned, &name) {
            return Ok(Step::Next(stage, data));
        }
    }
    // 无懈链：普通锦囊（无懈本身与延时锦囊不问）。
    if def.kind == CardKind::Trick && !def.tags.wuxie && !def.tags.delayed {
        let unresponsive = card_ref.map(|c| g.is_unresponsive(c)).unwrap_or(false);
        // 机制 `no-wuxie`：只关无懈窗口，不影响【决斗】等的其它响应。
        let no_wuxie = data["no_wuxie"].as_bool().unwrap_or(false);
        if !data["wuxie_started"].as_bool().unwrap_or(false) {
            data["wuxie_started"] = json!(true);
            if unresponsive || no_wuxie {
                // 往烈/雄乱/离间：此牌不能被无懈。
                data["wuxie_done"] = json!(true);
            } else {
                flow::start_wuxie(g, source, card_ref.cloned().unwrap_or_else(|| name.clone()));
                return Ok(Step::Next(stage, data));
            }
        }
        if !data["wuxie_done"].as_bool().unwrap_or(false) {
            let negated = g.wuxie_result.take().unwrap_or(false);
            data["wuxie_done"] = json!(true);
            if negated {
                g.push_log(format!("【{name}】被【无懈可击】抵消"));
                finish(g, card_ref, "wuxie_negated")?;
                return Ok(Step::Done);
            }
        }
    }

    match def.kind {
        CardKind::Equip => {
            let slot = def
                .slot
                .ok_or_else(|| "装备牌缺少装备槽位".to_string())?;
            let Some(card) = card_ref else {
                return Err("装备牌需要实体牌".into());
            };
            equip(g, source, card, slot)?;
            Ok(Step::Done)
        }
        CardKind::Basic => match def.effect {
            Effect::Sha => sha_effect(g, source, card_ref, &def, &mut data),
            Effect::Tao => {
                flow::heal(g, source, 1, "tao");
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
            Effect::Jiu => {
                apply_jiu(g, source);
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
            _ => {
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
        },
        CardKind::Trick => match def.effect {
            Effect::Duel => duel(g, source, card_ref, &def, &mut data),
            Effect::Dismantle | Effect::Snatch => {
                pick_card(g, source, card_ref, &def, &mut data)
            }
            Effect::ExNihilo => {
                let n = g.draw(source, 2).len();
                g.push_log(format!("{} 摸 {n} 张牌", g.seats[source].name));
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
            Effect::SavageAssault | Effect::ArcheryAttack => {
                aoe(g, source, card_ref, &def, &mut data)
            }
            Effect::PeachGarden => {
                for t in target_list(&data) {
                    flow::heal(g, t, 1, "taoyuan");
                }
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
            Effect::Harvest => harvest(g, source, card_ref, &mut data),
            Effect::FireAttack => fire_attack(g, source, card_ref, &def, &mut data),
            Effect::Collateral => collateral(g, source, card_ref, &mut data),
            Effect::IronChain => {
                for t in target_list(&data) {
                    if g.seats[t].chained {
                        flow::chain_leave(g, t);
                        g.push_log(format!("{} 解除连环状态", g.seats[t].name));
                    } else {
                        flow::chain_enter(g, t);
                    }
                }
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
            Effect::Indulgence | Effect::SupplyShortage | Effect::Lightning => {
                let t = target_list(&data)
                    .first()
                    .copied()
                    .ok_or_else(|| "延时锦囊缺少目标".to_string())?;
                let Some(card) = card_ref else {
                    return Err("延时锦囊需要实体牌".into());
                };
                g.to_judge(card, t)?;
                g.push_log(format!(
                    "【{name}】置于 {} 的判定区",
                    g.seats[t].name
                ));
                Ok(Step::Done)
            }
            _ => {
                finish(g, card_ref, "use")?;
                Ok(Step::Done)
            }
        },
    }
}

/// 「【杀】指定目标后 / 成为【杀】目标后」的介入结果（技能写、牌结算读后清空）。
///
/// 这三个时机（守玺 / 谏征 / 专对）在**结算之前**发生，因此不能走 `TargetChosen`
/// 事件（事件在牌结算完成后才路由，见 `next-engine-replacement.md` §9.2 谮毁遗留）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShaIntercept {
    /// 继续正常结算（技能未改变任何东西）。
    Continue,
    /// 该目标不再结算（此【杀】对其无效）。
    Immune(usize),
    /// 该目标不能响应（伤害照常结算）。
    Unresponsive(usize),
    /// 目标列表被替换（谏征：原目标全部取消，秦宓成为唯一目标）。
    Replace(Vec<usize>),
}

// ---------------------------------------------------------------------------
// 基本牌 / 杀的响应循环
// ---------------------------------------------------------------------------

fn sha_effect(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    data: &mut Value,
) -> Result<Step, String> {
    let targets = target_list(data);
    if let Some(step) = sha_loop(g, source, card, def, &targets, data)? {
        return Ok(step);
    }
    finish(g, card, "use")?;
    Ok(Step::Done)
}

/// 伤害来源（勤王：代出的【杀】伤害来源视为刘谌）。
fn damage_source_of(data: &Value, user: usize) -> usize {
    data["damage_source"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(user)
}

fn deal_damage(
    g: &mut Game,
    source: usize,
    victim: usize,
    amount: i32,
    def: &cards::CardDef,
    card: Option<&CardId>,
) {
    flow::damage_def(g, Some(source), victim, amount, def, card, false);
}

/// 【杀】的多目标结算循环（也被借刀杀人的「视为出杀」复用）。
/// 返回 `Some(Step)` = 等待响应；`None` = 全部结算完成。
fn sha_loop(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    targets: &[usize],
    data: &mut Value,
) -> Result<Option<Step>, String> {
    let mut cursor = num(data, "cursor");
    let mut targets = targets.to_vec();
    let damage_source = damage_source_of(data, source);
    let unresponsive = card.map(|c| g.is_unresponsive(c)).unwrap_or(false);
    while cursor < targets.len() {
        let t = targets[cursor];
        if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) || g.fenwei_void.contains(&t) {
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        // ① 读取介入技能的结果（技能帧在上方执行完毕后回到本帧）。
        if data["intercept_wait"].as_bool().unwrap_or(false) {
            let r = g
                .sha_intercept
                .take()
                .unwrap_or(ShaIntercept::Continue);
            data["intercept_wait"] = json!(false);
            match r {
                ShaIntercept::Continue => {}
                ShaIntercept::Immune(_) => {
                    g.push_log(format!("【杀】对 {} 无效", g.seats[t].name));
                    cursor += 1;
                    data["cursor"] = json!(cursor);
                    continue;
                }
                ShaIntercept::Unresponsive(_) => {
                    let bonus = jiu_take_bonus(g);
                    deal_damage(g, damage_source, t, 1 + bonus, def, card);
                    skills::on_sha_finished(g, source, Some(t), card, None, true);
                    cursor += 1;
                    data["cursor"] = json!(cursor);
                    continue;
                }
                ShaIntercept::Replace(list) => {
                    let mut list = list;
                    list.retain(|x| g.seats.get(*x).is_some_and(|s| s.hp > 0));
                    data["targets"] = json!(list);
                    data["asked"] = json!(false);
                    data["need"] = json!(0);
                    data["got"] = json!(0);
                    data["cursor"] = json!(0);
                    targets = list;
                    cursor = 0;
                    continue;
                }
            }
        }
        // ② 询问介入技能（按注册顺序，逐个；per-card / per-target 两种去重口径）。
        for (owner, code) in skills::sha_intercept_skills(g, source, t, card) {
            let key = if skills::sha_intercept_per_card(code) {
                "__card".to_string()
            } else {
                t.to_string()
            };
            let already = data["intercept_asked"]
                .get(code)
                .and_then(|v| v.as_str())
                .map(|v| v == key)
                .unwrap_or(false);
            if already {
                continue;
            }
            data["intercept_asked"][code] = json!(key);
            data["intercept_wait"] = json!(true);
            data["intercept_target"] = json!(t);
            crate::kernel::resolve::push_ask(
                g,
                code,
                Stage::Choose,
                owner,
                json!({ "intercept": true, "user": source, "target": t, "card": card }),
            );
            data["cursor"] = json!(cursor);
            return Ok(Some(Step::Next(Stage::CardOpt, data.clone())));
        }
        if data["asked"].as_bool().unwrap_or(false) {
            let responded = apply_response(g, t, ResponseKind::Shan, card)?;
            data["asked"] = json!(false);
            if responded {
                let got = num(data, "got") + 1;
                data["got"] = json!(got);
                let need = num(data, "need").max(1);
                if got >= need {
                    data["need"] = json!(0);
                    data["got"] = json!(0);
                    // 界咆哮：【杀】被抵消后，本回合下一次【杀】伤害 +1。
                    if crate::skills::active(g, damage_source, "paoxiao") {
                        g.counters.insert("turn.paoxiao_bonus".to_string(), 1);
                        g.push_log("咆哮：【杀】被抵消，本回合下一次【杀】伤害 +1".to_string());
                    }
                    skills::on_sha_finished(g, source, Some(t), card, None, false);
                    cursor += 1;
                    data["cursor"] = json!(cursor);
                    continue;
                }
                // multi-response：还需同一目标继续打出（落到下方重新询问）。
            } else {
                data["need"] = json!(0);
                data["got"] = json!(0);
                let bonus = jiu_take_bonus(g);
                deal_damage(g, damage_source, t, 1 + bonus, def, card);
                skills::on_sha_finished(g, source, Some(t), card, None, true);
                cursor += 1;
                data["cursor"] = json!(cursor);
                continue;
            }
        }
        // 不能被响应的牌（往烈/雄乱）：直接结算伤害。
        if !unresponsive {
            if let Some(p) = response_pending(g, t, ResponseKind::Shan) {
                if num(data, "need") == 0 {
                    let need = skills::response_need(g, t, source, ResponseKind::Shan, def.effect);
                    data["need"] = json!(need);
                    data["got"] = json!(0);
                    if need > 1 {
                        g.push_log(format!(
                            "无双：{} 需连续打出 {need} 张【闪】",
                            g.seats[t].name
                        ));
                    }
                }
                data["asked"] = json!(true);
                return need_at(g, Stage::CardAsk, data.clone(), p).map(Some);
            }
        }
        let bonus = jiu_take_bonus(g);
        deal_damage(g, damage_source, t, 1 + bonus, def, card);
        skills::on_sha_finished(g, source, Some(t), card, None, true);
        cursor += 1;
        data["cursor"] = json!(cursor);
    }
    Ok(None)
}

/// 群体锦囊（南蛮/万箭）：按行动顺序逐个要求响应。
fn aoe(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    data: &mut Value,
) -> Result<Step, String> {
    let kind = def
        .tags
        .respond
        .ok_or_else(|| "群体锦囊缺少响应牌属性".to_string())?;
    let targets = target_list(data);
    let mut cursor = num(data, "cursor");
    let mut targets = targets.to_vec();
    let damage_source = damage_source_of(data, source);
    let unresponsive = card.map(|c| g.is_unresponsive(c)).unwrap_or(false);
    while cursor < targets.len() {
        let t = targets[cursor];
        if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) || g.fenwei_void.contains(&t) {
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        if data["relay_wait"].as_bool().unwrap_or(false) {
            data["relay_wait"] = json!(false);
            let relayed = g.relay_played.take().is_some();
            g.relay_none = false;
            if relayed {
                cursor += 1;
                data["cursor"] = json!(cursor);
                continue;
            }
        }
        if data["asked"].as_bool().unwrap_or(false) {
            let responded = apply_response(g, t, kind, card)?;
            data["asked"] = json!(false);
            if !responded {
                deal_damage(g, damage_source, t, 1, def, card);
            }
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        if !unresponsive {
            if crate::skills::relay_before(g, t, kind, card) {
                data["relay_wait"] = json!(true);
                data["cursor"] = json!(cursor);
                return Ok(Step::Next(Stage::CardOpt, data.clone()));
            }
            if let Some(p) = response_pending(g, t, kind) {
                data["asked"] = json!(true);
                return need_at(g, Stage::CardAsk, data.clone(), p);
            }
        }
        deal_damage(g, damage_source, t, 1, def, card);
        cursor += 1;
        data["cursor"] = json!(cursor);
    }
    finish(g, card, "use")?;
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 决斗
// ---------------------------------------------------------------------------

fn duel(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    data: &mut Value,
) -> Result<Step, String> {
    let targets = target_list(data);
    let tcursor = num(data, "tcursor").min(targets.len().saturating_sub(1));
    let initial = *targets.get(tcursor).ok_or_else(|| "【决斗】缺少目标".to_string())?;
    data["source_seat"] = json!(source);
    let responder = data["responder"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(initial);
    let opponent = if responder == source { initial } else { source };
    let damage_source = damage_source_of(data, source);
    let unresponsive = card.map(|c| g.is_unresponsive(c)).unwrap_or(false);

    if g.seats.get(responder).map(|s| s.hp <= 0).unwrap_or(true) {
        return advance_target(g, card, data);
    }

    if data["relay_wait"].as_bool().unwrap_or(false) {
        data["relay_wait"] = json!(false);
        let relayed = g.relay_played.take().is_some();
        g.relay_none = false;
        if relayed {
            data["responder"] = json!(opponent);
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        }
    }
    if data["asked"].as_bool().unwrap_or(false) {
        let responded = apply_response(g, responder, ResponseKind::Sha, card)?;
        data["asked"] = json!(false);
        if responded {
            let got = num(data, "got") + 1;
            data["got"] = json!(got);
            let need = num(data, "need").max(1);
            if got >= need {
                // 轮换：由对方继续出【杀】。
                data["responder"] = json!(opponent);
                data["need"] = json!(0);
                data["got"] = json!(0);
            }
            // multi-response：未凑满张数时同一方继续（`need` 保留）。
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        }
        let source_of_damage = if damage_source == source {
            opponent
        } else {
            damage_source
        };
        data["need"] = json!(0);
        data["got"] = json!(0);
        deal_damage(g, source_of_damage, responder, 1, def, card);
        skills::zhanjue_on_damage(g, source, responder);
        return advance_target(g, card, data);
    }

    if !unresponsive {
        if crate::skills::relay_before(g, responder, ResponseKind::Sha, card) {
            data["responder"] = json!(responder);
            data["relay_wait"] = json!(true);
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        }
        if let Some(p) = response_pending(g, responder, ResponseKind::Sha) {
            data["responder"] = json!(responder);
            data["asked"] = json!(true);
            if num(data, "need") == 0 {
                let need = crate::skills::response_need(
                    g,
                    responder,
                    opponent,
                    ResponseKind::Sha,
                    def.effect,
                );
                data["need"] = json!(need);
                data["got"] = json!(0);
                if need > 1 {
                    g.push_log(format!(
                        "无双：{} 需连续打出 {need} 张【杀】",
                        g.seats[responder].name
                    ));
                }
            }
            return need_at(g, Stage::CardAsk, data.clone(), p);
        }
    }
    let source_of_damage = if damage_source == source {
        opponent
    } else {
        damage_source
    };
    deal_damage(g, source_of_damage, responder, 1, def, card);
    skills::zhanjue_on_damage(g, source, responder);
    advance_target(g, card, data)
}

/// 多目标单目标效果：当前目标结算完 → 还有下一个目标则重置询问状态继续，否则结束。
///
/// 用于福绵②（mechanics `target-extra`）：一张红色普通锦囊/【杀】可以多指定目标。
fn advance_target(
    g: &mut Game,
    card: Option<&CardId>,
    data: &mut Value,
) -> Result<Step, String> {
    let targets = target_list(data);
    let mut tc = num(data, "tcursor") + 1;
    while tc < targets.len() && g.seats.get(targets[tc]).map(|s| s.hp <= 0).unwrap_or(true) {
        tc += 1;
    }
    data["tcursor"] = json!(tc);
    if tc < targets.len() {
        data["asked"] = json!(false);
        data["need"] = json!(0);
        data["got"] = json!(0);
        data["step"] = json!("start");
        data["responder"] = json!(targets[tc]);
        data["opponent"] = data["source_seat"].clone();
        data["shown"] = json!(null);
        Ok(Step::Next(Stage::CardOpt, data.clone()))
    } else {
        finish(g, card, "use")?;
        Ok(Step::Done)
    }
}

// ---------------------------------------------------------------------------
// 过河拆桥 / 顺手牵羊
// ---------------------------------------------------------------------------

fn pick_card(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    data: &mut Value,
) -> Result<Step, String> {
    let targets = target_list(data);
    let tcursor = num(data, "tcursor").min(targets.len().saturating_sub(1));
    let target = *targets.get(tcursor).ok_or_else(|| "缺少目标".to_string())?;

    // 候选：判定区 / 装备区（可见）+ 手牌（不可见，随机取第一张）。
    let mut options: Vec<(String, String)> = Vec::new();
    for id in g.judge_zone(target) {
        options.push((
            format!("pick:{id}"),
            format!("判定区【{}】", g.card_label(&id)),
        ));
    }
    for slot in [
        Slot::Weapon,
        Slot::Armor,
        Slot::HorseOffense,
        Slot::HorseDefense,
        Slot::Treasure,
    ] {
        if let Some(id) = g.equip_in(target, slot) {
            options.push((
                format!("pick:{id}"),
                format!("{}【{}】", slot.label(), g.card_label(&id)),
            ));
        }
    }
    if !g.hand(target).is_empty() {
        options.push(("pick:hand".to_string(), "随机一张手牌".to_string()));
    }
    if options.is_empty() {
        return advance_target(g, card, data);
    }

    if data["asked"].as_bool().unwrap_or(false) {
        let ans = read_answer(g)?;
        data["asked"] = json!(false);
        if ans == "pass" {
            return advance_target(g, card, data);
        }
        if !options.iter().any(|(id, _)| *id == ans) {
            return Err(format!("非法选牌：{ans}"));
        }
        let picked = if ans == "pick:hand" {
            g.hand(target).first().cloned()
        } else {
            ans.strip_prefix("pick:").map(|s| s.to_string())
        };
        let picked = picked.ok_or_else(|| format!("未知选项：{ans}"))?;
        if def.effect == Effect::Dismantle
            && crate::skills::prevent_equip_discard(g, target, &picked, Some(source))
        {
            g.push_log(format!(
                "奇才：防止 {} 弃置其防具/宝物【{}】",
                g.seats[target].name,
                g.card_label(&picked)
            ));
            return advance_target(g, card, data);
        }
        if def.effect == Effect::Snatch {
            g.gain(&picked, source)?;
            g.push_log(format!(
                "{} 获得 {} 的一张牌【{}】",
                g.seats[source].name,
                g.seats[target].name,
                g.card_label(&picked)
            ));
        } else {
            g.to_discard(&picked, "dismantle")?;
            g.push_log(format!(
                "{} 弃置 {} 的一张牌【{}】",
                g.seats[source].name,
                g.seats[target].name,
                g.card_label(&picked)
            ));
        }
        return advance_target(g, card, data);
    }

    let mut b = skills::builder(source, "card_pick").skill("__card");
    for (id, label) in &options {
        b = b.option(id.clone(), label.clone());
    }
    data["asked"] = json!(true);
    need_at(
        g,
        Stage::CardPick,
        data.clone(),
        b.escape("取消（不选牌）").build(),
    )
}

// ---------------------------------------------------------------------------
// 五谷丰登
// ---------------------------------------------------------------------------

fn harvest(
    g: &mut Game,
    _source: usize,
    card: Option<&CardId>,
    data: &mut Value,
) -> Result<Step, String> {
    if data["pool"].is_null() {
        let pickers = target_list(data);
        let n = pickers.len();
        let ids: Vec<CardId> = g.ids_in(|z| *z == Zone::Draw).into_iter().take(n).collect();
        for id in &ids {
            g.to_frame(id)?;
            g.reveal(id, None, "harvest");
        }
        data["pool"] = json!(ids);
        data["pickers"] = json!(pickers);
        data["cursor"] = json!(0);
    }
    let pickers: Vec<usize> = data["pickers"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    let mut cursor = num(data, "cursor");
    let mut pool: Vec<CardId> = data["pool"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    while cursor < pickers.len() && !pool.is_empty() {
        let actor = pickers[cursor];
        if data["asked"].as_bool().unwrap_or(false) {
            let ans = read_answer(g)?;
            data["asked"] = json!(false);
            if ans != "pass" {
                let id = ans
                    .strip_prefix("pick:")
                    .ok_or_else(|| format!("五谷：未知选项 {ans}"))?
                    .to_string();
                if !pool.contains(&id) {
                    return Err("五谷：该牌不在展示区".into());
                }
                g.gain(&id, actor)?;
                g.conceal(&id, "harvest");
                g.push_log(format!(
                    "{} 获得【{}】",
                    g.seats[actor].name,
                    g.card_label(&id)
                ));
                pool.retain(|c| *c != id);
                data["pool"] = json!(pool);
            }
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        let mut b = skills::builder(actor, "harvest").skill("__card");
        for id in &pool {
            b = b.option(
                format!("pick:{id}"),
                format!("获得【{}】", g.card_label(id)),
            );
        }
        data["asked"] = json!(true);
        return need_at(
            g,
            Stage::CardOpt,
            data.clone(),
            b.escape("放弃（不取）").build(),
        );
    }
    // 剩余牌进弃牌堆。
    for id in pool {
        g.conceal(&id, "harvest");
        g.to_discard(&id, "harvest")?;
    }
    finish(g, card, "use")?;
    Ok(Step::Done)
}

// ---------------------------------------------------------------------------
// 火攻
// ---------------------------------------------------------------------------

fn fire_attack(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    def: &cards::CardDef,
    data: &mut Value,
) -> Result<Step, String> {
    let fw_targets = target_list(data);
    let tcursor = num(data, "tcursor").min(fw_targets.len().saturating_sub(1));
    let target = *fw_targets
        .get(tcursor)
        .ok_or_else(|| "【火攻】缺少目标".to_string())?;
    let step = data["step"].as_str().unwrap_or("start").to_string();

    if step != "discard" {
        if data["asked"].as_bool().unwrap_or(false) {
            let ans = read_answer(g)?;
            data["asked"] = json!(false);
            if ans == "pass" {
                return advance_target(g, card, data);
            }
            let id = ans
                .strip_prefix("reveal:")
                .ok_or_else(|| format!("火攻：未知选项 {ans}"))?
                .to_string();
            if !g.hand(target).contains(&id) {
                return Err("火攻：展示的牌不在其手牌".into());
            }
            g.reveal(&id, Some(target), "fire_attack");
            data["shown"] = json!(id);
            data["step"] = json!("discard");
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        }
        let hand = g.hand(target);
        if hand.is_empty() {
            return advance_target(g, card, data);
        }
        let mut b = skills::builder(target, "fire_attack").skill("__card");
        for id in &hand {
            b = b.option(
                format!("reveal:{id}"),
                format!("展示【{}】", g.card_label(id)),
            );
        }
        data["asked"] = json!(true);
        return need_at(
            g,
            Stage::CardOpt,
            data.clone(),
            b.escape("放弃展示（火攻无效）").build(),
        );
    }

    let shown = data["shown"].as_str().unwrap_or_default().to_string();
    if data["asked"].as_bool().unwrap_or(false) {
        let ans = read_answer(g)?;
        data["asked"] = json!(false);
        if ans != "pass" {
            let id = ans
                .strip_prefix("discard:")
                .ok_or_else(|| format!("火攻：未知选项 {ans}"))?
                .to_string();
            if !g.hand(source).contains(&id) {
                return Err("火攻：弃置的牌不在手牌".into());
            }
            if g.cards.get(&id).map(|c| c.suit) != g.cards.get(&shown).map(|c| c.suit) {
                return Err("火攻：弃置的牌需与展示牌同花色".into());
            }
            g.to_discard(&id, "fire_attack")?;
            deal_damage(g, damage_source_of(data, source), target, 1, def, card);
        }
        g.conceal(&shown, "fire_attack");
        return advance_target(g, card, data);
    }

    let suit = g.cards.get(&shown).map(|c| c.suit);
    let ids: Vec<CardId> = g
        .hand(source)
        .into_iter()
        .filter(|id| g.cards.get(id).map(|c| Some(c.suit) == suit).unwrap_or(false))
        .collect();
    if ids.is_empty() {
        g.conceal(&shown, "fire_attack");
        return advance_target(g, card, data);
    }
    let mut b = skills::builder(source, "fire_attack").skill("__card");
    for id in &ids {
        b = b.option(
            format!("discard:{id}"),
            format!("弃置【{}】造成 1 点火焰伤害", g.card_label(id)),
        );
    }
    data["asked"] = json!(true);
    need_at(
        g,
        Stage::CardOpt,
        data.clone(),
        b.escape("不弃置（火攻无效）").build(),
    )
}

// ---------------------------------------------------------------------------
// 借刀杀人
// ---------------------------------------------------------------------------

fn collateral(
    g: &mut Game,
    source: usize,
    card: Option<&CardId>,
    data: &mut Value,
) -> Result<Step, String> {
    let targets = target_list(data);
    let a = *targets.first().ok_or_else(|| "缺少目标 A".to_string())?;
    let b = *targets.get(1).ok_or_else(|| "缺少目标 B".to_string())?;

    // 子流程：A 已出【杀】→ 走标准【杀】结算。
    if data["step"].as_str() == Some("sha") {
        let sha_card: CardId = data["sha_card"]
            .as_str()
            .ok_or_else(|| "借刀：缺少【杀】".to_string())?
            .to_string();
        let sha_def = g.def_of(&sha_card);
        if let Some(step) = sha_loop(g, a, Some(&sha_card), &sha_def, &[b], data)? {
            return Ok(step);
        }
        finish(g, Some(&sha_card), "collateral_sha")?;
        finish(g, card, "use")?;
        return Ok(Step::Done);
    }

    if data["asked"].as_bool().unwrap_or(false) {
        let ans = read_answer(g)?;
        data["asked"] = json!(false);
        if ans == "pass" {
            if let Some(weapon) = g.equip_in(a, Slot::Weapon) {
                g.gain(&weapon, source)?;
                g.push_log(format!(
                    "{} 将武器【{}】交给 {}",
                    g.seats[a].name,
                    g.card_label(&weapon),
                    g.seats[source].name
                ));
            }
            finish(g, card, "use")?;
            return Ok(Step::Done);
        }
        let id = ans
            .strip_prefix("respond:")
            .ok_or_else(|| format!("借刀：未知选项 {ans}"))?
            .to_string();
        if !g.hand(a).contains(&id) {
            return Err("借刀：该牌不在其手牌".into());
        }
        if !cards::responds_as(&g.name_of(&id), ResponseKind::Sha) {
            return Err("借刀：只能使用【杀】".into());
        }
        if !g.can_reach(a, b, &id) {
            return Err("借刀：目标不在攻击范围内".into());
        }
        g.to_processing(&id)?;
        data["step"] = json!("sha");
        data["sha_card"] = json!(id);
        data["cursor"] = json!(0);
        data["asked"] = json!(false);
        return Ok(Step::Next(Stage::CardOpt, data.clone()));
    }

    let mut p = skills::builder(a, "collateral").skill("__card");
    for id in g.hand(a) {
        if !cards::responds_as(&g.name_of(&id), ResponseKind::Sha) {
            continue;
        }
        if !g.can_reach(a, b, &id) {
            continue;
        }
        p = p.option(
            format!("respond:{id}"),
            format!(
                "对 {} 使用【{}】",
                g.seats[b].name,
                g.card_label(&id)
            ),
        );
    }
    data["asked"] = json!(true);
    need_at(
        g,
        Stage::CardOpt,
        data.clone(),
        p.escape("不打出【杀】（交出武器）").build(),
    )
}

// ---------------------------------------------------------------------------
// 响应（闪 / 杀 / 无懈的「牌」）
// ---------------------------------------------------------------------------

/// 该角色当前可用的响应选项（唯一来源：pending 与实际执行共用）。
fn response_options(g: &Game, target: usize, kind: ResponseKind) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in g.hand(target) {
        if !g.can_use_card(target, &id) {
            continue;
        }
        if kind == ResponseKind::Shan && skills::shan_blocked_by_shizhi(g, target, &id) {
            continue;
        }
        if cards::responds_as(&g.name_of(&id), kind) {
            out.push((format!("respond:{id}"), format!("打出【{}】", g.card_label(&id))));
        }
    }
    if kind == ResponseKind::Shan {
        if g.armor(target) == ArmorKind::Bagua {
            out.push(("armor:bagua".to_string(), "发动【八卦阵】判定".to_string()));
        }
        for (id, label) in skills::dodge_options(g, target) {
            out.push((id, label));
        }
    }
    // 被动转化技（龙胆：杀→闪 / 闪→杀）
    for conv in skills::conversions(g, target) {
        if !conv.playable || !cards::responds_as(conv.to, kind) {
            continue;
        }
        out.push((
            format!("vrespond:{}:{}", conv.skill, conv.from),
            format!("【{}】当【{}】打出", g.card_label(&conv.from), conv.to),
        ));
    }
    // 主公技接力（激将：蜀势力替主公打出【杀】）
    for (id, label) in skills::relay_options(g, target, kind) {
        out.push((id, label));
    }
    out
}

fn response_label(kind: ResponseKind) -> &'static str {
    match kind {
        ResponseKind::Sha => "杀",
        ResponseKind::Shan => "闪",
        ResponseKind::Wuxie => "无懈可击",
    }
}

fn response_pending(
    g: &mut Game,
    target: usize,
    kind: ResponseKind,
) -> Option<super::pending::Pending> {
    let mut options = response_options(g, target, kind);
    if kind == ResponseKind::Shan && crate::skills::block_real_shan(g, target) {
        options.retain(|(id, _)| !id.starts_with("respond:"));
    }
    if options.is_empty() {
        return None;
    }
    let mut b = skills::builder(target, "card_response").skill("__card");
    for (id, label) in options {
        b = b.option(id, label);
    }
    g.response_context = Some(kind);
    let label = response_label(kind);
    Some(b.escape(format!("不出【{label}】")).build())
}

/// 处理一次响应回答，返回 `true` = 已响应（抵消/免疫）。
fn apply_response(
    g: &mut Game,
    actor: usize,
    kind: ResponseKind,
    used: Option<&CardId>,
) -> Result<bool, String> {
    let ans = read_answer(g)?;
    g.response_context = None;
    if ans == "pass" {
        return Ok(false);
    }
    if let Some(id) = ans.strip_prefix("respond:") {
        let id = id.to_string();
        if !g.hand(actor).contains(&id) {
            return Err("响应：该牌不在手牌".into());
        }
        if !cards::responds_as(&g.name_of(&id), kind) {
            return Err("响应：该牌不能作为此环节的响应牌".into());
        }
        if !g.can_use_card(actor, &id) {
            return Err("响应：该牌已被冻结".into());
        }
        g.to_discard(&id, "respond")?;
        let label = g.name_of(&id);
        g.events.push(Event::card(
            EventKind::CardPlayed,
            Some(actor),
            id.clone(),
            Some("response".to_string()),
        ));
        g.presentation.push(
            Presentation::new("respond", POutcome::Played { label })
                .actor(actor)
                .cards(vec![id.clone()])
                .fx("respond"),
        );
        g.push_log(format!("{} 打出【{}】响应", g.seats[actor].name, g.card_label(&id)));
        // 「其他角色响应你使用的牌」统一接入点（复难）。
        crate::skills::on_response_played(g, used, actor, &id);
        return Ok(true);
    }
    if ans == "armor:bagua" {
        if g.armor(actor) != ArmorKind::Bagua {
            return Err("八卦阵：未装备【八卦阵】".into());
        }
        let Some(judge) = flow::judge_card(g, actor) else {
            return Ok(false);
        };
        let red = g.is_red(&judge);
        let label = g.card_label(&judge);
        flow::judge_to_discard(g, &judge)?;
        g.presentation.push(
            Presentation::new(
                "bagua",
                if red {
                    POutcome::Activated {
                        skill: "八卦阵".to_string(),
                    }
                } else {
                    POutcome::Declined {
                        skill: "八卦阵".to_string(),
                    }
                },
            )
            .actor(actor)
            .cards(vec![judge])
            .fx("bagua"),
        );
        g.push_log(format!(
            "八卦阵：判定【{label}】→ {}",
            if red { "视为打出【闪】" } else { "判定失败" }
        ));
        return Ok(red);
    }
    if let Some(rest) = ans.strip_prefix("vrespond:") {
        // 被动转化技打出：`vrespond:<skill>:<承载牌>`
        let mut parts = rest.splitn(2, ':');
        let _skill = parts.next().unwrap_or_default();
        let carrier = parts.next().unwrap_or_default().to_string();
        let from_equip = matches!(g.zone_of(&carrier), Some(Zone::Equip(s, _)) if *s == actor);
        if !g.hand(actor).contains(&carrier) && !from_equip {
            return Err("响应：该牌不在手牌/装备区".into());
        }
        if !g.can_use_card(actor, &carrier) {
            return Err("响应：该牌已被冻结".into());
        }
        let label = g.name_of(&carrier);
        g.to_discard(&carrier, "respond")?;
        g.events.push(Event::card(
            EventKind::CardPlayed,
            Some(actor),
            carrier.clone(),
            Some("response".to_string()),
        ));
        g.presentation.push(
            Presentation::new("respond", POutcome::Played { label })
                .actor(actor)
                .cards(vec![carrier.clone()])
                .fx("respond"),
        );
        g.push_log(format!(
            "{} 以【{}】转化打出响应",
            g.seats[actor].name,
            g.card_label(&carrier)
        ));
        crate::skills::on_response_played(g, used, actor, &carrier);
        return Ok(true);
    }
    if ans == "jijiang" {
        // 主公技接力：压入激将帧（响应流程在下一轮读取 `relay_played` / `relay_none`）。
        crate::skills::start_relay(g, actor, kind, used);
        return Ok(false);
    }
    if kind == ResponseKind::Shan && skills::dodge_use(g, actor, &ans)? {
        return Ok(true);
    }
    Err(format!("未知响应选项：{ans}"))
}

// ---------------------------------------------------------------------------
// 判定阶段：延时锦囊 LIFO
// ---------------------------------------------------------------------------

/// 技能判定子流程：亮判定牌 → 鬼才改判 → 生效 → 天妒（判定牌在 Frame 区）。
pub fn start_quick_judge(g: &mut Game, owner: usize, reason: &str) {
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::QuickJudge,
        skill: reason.to_string(),
        actor: owner,
        stage: Stage::QuickJudge,
        data: json!({ "owner": owner, "step": "reveal" }),
    }));
}

pub fn step_quick_judge(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
    let mut data = a.data.clone();
    let step = data["step"].as_str().unwrap_or("reveal").to_string();
    if step == "reveal" {
        let Some(card) = flow::judge_card(g, owner) else {
            g.judge_result = None;
            return Ok(Step::Done);
        };
        data["card"] = json!(card);
        data["step"] = json!("rewrite");
        return Ok(Step::Next(Stage::QuickJudge, data));
    }
    if step == "rewrite" {
        // 应用上一次改判（原判定牌进弃牌堆）。
        if let Some(new_card) = g.judge_rewrite.take() {
            let old = data["card"].as_str().unwrap_or_default().to_string();
            if !old.is_empty() && old != new_card && g.cards.contains_key(&old) {
                g.to_discard(&old, "judge_rewrite")?;
            }
            data["card"] = json!(new_card);
        }
        // 链式询问：判定者左手侧起、判定者最后（mechanics trigger-order）。
        let card = data["card"].as_str().unwrap_or_default().to_string();
        let order = crate::skills::judge_rewrite_holders(g, owner);
        let mut cursor = data["rewrite_cursor"].as_u64().unwrap_or(0) as usize;
        while cursor < order.len() {
            let seat = order[cursor];
            cursor += 1;
            data["rewrite_cursor"] = json!(cursor);
            if crate::skills::start_rewrite_for(g, owner, seat, &card) {
                return Ok(Step::Next(Stage::QuickJudge, data));
            }
        }
        data["step"] = json!("effective");
        return Ok(Step::Next(Stage::QuickJudge, data));
    }
    let card = data["card"].as_str().unwrap_or_default().to_string();
    flow::judge_effective(g, owner, &card);
    g.judge_result = Some(card.clone());
    if !data["tiandu_asked"].as_bool().unwrap_or(false) {
        data["tiandu_asked"] = json!(true);
        if crate::skills::start_tiandu(g, owner, &card) {
            return Ok(Step::Next(Stage::QuickJudge, data));
        }
    }
    if matches!(g.zone_of(&card), Some(Zone::Frame(_)) | Some(Zone::Draw) | Some(Zone::Processing)) {
        g.to_discard(&card, "judge")?;
    }
    Ok(Step::Done)
}

pub fn start_judge(g: &mut Game, seat: usize) {
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::Judge,
        skill: "__judge".to_string(),
        actor: seat,
        stage: Stage::JudgeResolve,
        data: json!({ "seat": seat }),
    }));
}

pub fn step_judge(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.data["seat"].as_u64().unwrap_or(0) as usize;
    let mut data = a.data.clone();
    if a.data["card"].is_null() {
        let next = g
            .judge_zone_lifo(seat)
            .into_iter()
            .find(|id| g.def_of(id).tags.delayed);
        let Some(id) = next else {
            return Ok(Step::Done);
        };
        data = json!({ "seat": seat, "card": id });
    }
    let card: CardId = data["card"].as_str().unwrap_or_default().to_string();

    // 1) 亮出判定牌。
    if data["judge_card"].is_null() {
        let Some(judge) = flow::judge_card(g, seat) else {
            g.to_discard(&card, "judge_no_card")?;
            return Ok(Step::Next(Stage::JudgeResolve, json!({ "seat": seat })));
        };
        data["judge_card"] = json!(judge);
        return Ok(Step::Next(Stage::JudgeResolve, data));
    }

    // 2) 无懈窗口（延时锦囊在其生效前可被无懈）。
    if !data["wuxie_done"].as_bool().unwrap_or(false) {
        data["wuxie_done"] = json!(true);
        if g.is_unresponsive(&card) {
            g.wuxie_result = Some(false);
        } else {
            flow::start_wuxie(g, seat, card.clone());
            return Ok(Step::Next(Stage::JudgeResolve, data));
        }
    }
    let judge: CardId = data["judge_card"].as_str().unwrap_or_default().to_string();
    if !data["wuxie_read"].as_bool().unwrap_or(false) {
        data["wuxie_read"] = json!(true);
        if g.wuxie_result.take().unwrap_or(false) {
            g.push_log(format!("【{}】被【无懈可击】抵消", g.name_of(&card)));
            flow::judge_to_discard(g, &judge)?;
            if matches!(g.zone_of(&card), Some(Zone::Judge(_))) {
                g.to_discard(&card, "wuxie_negated")?;
            }
            return Ok(Step::Next(Stage::JudgeResolve, json!({ "seat": seat })));
        }
    }

    // 2.5) 改判窗口（判定牌生效前；与技能判定同口径，链式询问）。
    if !data["rewrite_done"].as_bool().unwrap_or(false) {
        let mut judge = judge;
        if let Some(new_card) = g.judge_rewrite.take() {
            if !judge.is_empty() && judge != new_card && g.cards.contains_key(&judge) {
                g.to_discard(&judge, "judge_rewrite")?;
            }
            judge = new_card;
        }
        let order = crate::skills::judge_rewrite_holders(g, seat);
        let mut cursor = data["rewrite_cursor"].as_u64().unwrap_or(0) as usize;
        while cursor < order.len() {
            let who = order[cursor];
            cursor += 1;
            data["rewrite_cursor"] = json!(cursor);
            if crate::skills::start_rewrite_for(g, seat, who, &judge) {
                return Ok(Step::Next(Stage::JudgeResolve, data));
            }
        }
        data["rewrite_done"] = json!(true);
        data["judge_card"] = json!(judge);
        return Ok(Step::Next(Stage::JudgeResolve, data));
    }

    // 3) 生效。
    flow::judge_effective(g, seat, &judge);
    let def = g.def_of(&card);
    let suit = g.cards.get(&judge).map(|c| c.suit);
    let rank = g.cards.get(&judge).map(|c| c.rank).unwrap_or(0);
    let mut move_to_next = false;
    match def.effect {
        Effect::Indulgence => {
            // 思蜀（judge-reversal）：该角色本局【乐不思蜀】判定效果反转（红桃 ↔ 非红桃）。
            let reversed = crate::skills::sishu_reversed(g, seat);
            let skip = if reversed {
                suit == Some(Suit::Heart)
            } else {
                suit != Some(Suit::Heart)
            };
            if skip {
                g.counters.insert("turn.skip_play".to_string(), 1);
                g.push_log(format!(
                    "{} 判定{} → 本回合跳过出牌阶段",
                    g.seats[seat].name,
                    if reversed {
                        "红桃（思蜀反转）"
                    } else {
                        "非红桃"
                    }
                ));
            }
        }
        Effect::SupplyShortage => {
            if suit != Some(Suit::Club) {
                g.counters.insert("turn.skip_draw".to_string(), 1);
                g.push_log(format!("{} 判定非梅花 → 跳过摸牌阶段", g.seats[seat].name));
            }
        }
        Effect::Lightning => {
            let hit = suit == Some(Suit::Spade) && (2..=9).contains(&rank);
            if hit {
                flow::damage_typed(
                    g,
                    None,
                    seat,
                    3,
                    cards::DamageKind::Thunder,
                    Some(&card),
                    false,
                );
            } else {
                let next = g.seats_from_left_of(seat).into_iter().find(|i| {
                    g.seats[*i].hp > 0
                        && !g.seats[*i].judge_abolished
                        && !same_effect_in_judge(g, *i, Effect::Lightning)
                });
                if let Some(next) = next {
                    g.move_judge_to(&card, next)?;
                    move_to_next = true;
                    g.push_log(format!(
                        "【闪电】判定后移至 {} 的判定区",
                        g.seats[next].name
                    ));
                }
            }
        }
        _ => {}
    }
    flow::judge_to_discard(g, &judge)?;
    if !move_to_next && matches!(g.zone_of(&card), Some(Zone::Judge(_))) {
        g.to_discard(&card, "judge")?;
        g.clear_virtual(&card);
    }
    Ok(Step::Next(
        Stage::JudgeResolve,
        json!({ "seat": seat }),
    ))
}

// ---------------------------------------------------------------------------
// 装备 / 收尾
// ---------------------------------------------------------------------------

fn equip(g: &mut Game, seat: usize, card: &CardId, slot: Slot) -> Result<(), String> {
    if g.equip_in(seat, slot).is_some() {
        g.discard_equip_slot(seat, slot, "equip_replace")?;
    }
    g.to_equip(card, seat, slot)?;
    g.push_log(format!(
        "{} 装备【{}】（{}）",
        g.seats[seat].name,
        g.name_of(card),
        slot.label()
    ));
    Ok(())
}

/// 取走【酒】加成（每张【杀】最多消耗 1 层）。
fn jiu_take_bonus(g: &mut Game) -> i32 {
    if g.counter("turn.jiu_bonus") > 0 {
        g.counters.insert("turn.jiu_bonus".to_string(), 0);
        1
    } else {
        0
    }
}

/// 牌结算收尾：从处理区进弃牌堆（装备/延时锦囊在效果里已落位）。
///
/// 转化牌（奇策/战绝/滔乱/慎断）：承载牌按落位规则处理；其余 origins（如奇策的全部手牌）
/// 一并进弃牌堆，并清除虚拟定义。
fn finish(g: &mut Game, card: Option<&CardId>, cause: &str) -> Result<(), String> {
    // 乱击的「少选一个目标」只对本次使用生效。
    g.counters.remove("turn.aoe_trim");
    g.fenwei_void.clear();
    g.response_context = None;
    let Some(card) = card else {
        return Ok(());
    };
    if !matches!(
        g.zone_of(card),
        Some(Zone::Discard) | Some(Zone::Judge(_)) | Some(Zone::Equip(_, _))
    ) {
        g.to_discard(card, cause)?;
    }
    let origins = g.origins.remove(card).unwrap_or_default();
    for id in origins {
        if id == *card {
            continue;
        }
        if matches!(g.zone_of(&id), Some(Zone::Hand(_)) | Some(Zone::Processing)) {
            g.to_discard(&id, "virtual_origin")?;
        }
    }
    // 转化延时锦囊（慎断）：牌进判定区后仍需按声明牌名结算，虚拟定义保留到判定结束。
    let in_judge = matches!(g.zone_of(card), Some(Zone::Judge(_)));
    if !in_judge && g.virtual_defs.contains_key(card) {
        g.clear_virtual(card);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// data / 输入辅助
// ---------------------------------------------------------------------------

fn num(data: &Value, key: &str) -> usize {
    data[key].as_u64().unwrap_or(0) as usize
}

fn target_list(data: &Value) -> Vec<usize> {
    data["targets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

/// 持久化当前帧的 `data` / `stage` 后返回 `Step::Need`。
///
/// 为什么不用 `Step::NeedAt`：它只改 stage、**不保存 data**（技能侧靠「首次无 answer + Next」推进），
/// 而牌结算需要在同一次询问里累积 `cursor` / `pool` 等进度。
fn need_at(
    g: &mut Game,
    stage: Stage,
    data: Value,
    p: super::pending::Pending,
) -> Result<Step, String> {
    match g.stack.last_mut() {
        Some(Frame::Flow(f)) => {
            f.stage = stage;
            f.data = data;
        }
        Some(Frame::Ask(ask)) => {
            ask.stage = stage;
            ask.data = data;
        }
        _ => return Err("牌结算：当前结算帧不可保存进度".into()),
    }
    Ok(Step::Need(p))
}

fn read_answer(g: &mut Game) -> Result<String, String> {
    g.answer
        .take()
        .ok_or_else(|| "牌结算：等待输入但缺少回答".to_string())
}
