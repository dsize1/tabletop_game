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
use super::gear;
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
    /// 承载牌可以来自他人手牌（主公技接力：代出的【杀】视为主公使用，牌不转手）。
    pub foreign_carrier: bool,
}

impl UseOpts {
    /// 普通使用（手牌 + 自己出牌阶段）。
    pub fn normal() -> Self {
        Self {
            strict: true,
            damage_source: None,
            no_wuxie: false,
            foreign_carrier: false,
        }
    }

    /// 技能驱动的「视为使用」（不检查回合/阶段/手牌；目标由技能自行保证）。
    pub fn skill() -> Self {
        Self {
            strict: false,
            damage_source: None,
            no_wuxie: false,
            foreign_carrier: false,
        }
    }

    /// 出牌阶段的转化牌：严格校验（次数/距离），但伤害来源可覆盖。
    pub fn carrier() -> Self {
        Self {
            strict: true,
            damage_source: None,
            no_wuxie: false,
            foreign_carrier: false,
        }
    }

    /// 主公技接力（激将/护驾）：严格校验（次数/范围按主公），承载牌可来自他人手牌。
    pub fn relay() -> Self {
        Self {
            strict: true,
            damage_source: None,
            no_wuxie: false,
            foreign_carrier: true,
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
    // 【无懈可击】只能在响应窗口使用；技能驱动的声明（矫诏）允许作为虚拟牌使用。
    if def.tags.wuxie && opts.strict {
        return Err("【无懈可击】只能用于响应".into());
    }
    // 巧说没赢（界简雍）：本回合不能使用锦囊牌（含延时锦囊）。
    if def.kind == CardKind::Trick && crate::skills::trick_blocked(g, seat) {
        return Err("巧说没赢：本回合不能使用锦囊牌".into());
    }
    if (def.kind == CardKind::Equip || def.tags.delayed) && carrier.is_none() {
        return Err("该牌需要实体牌承载".into());
    }
    // 装备栏已废除（决堰/破势/雄乱）：legal 与 execute 同源 —— 不再枚举「装不上」的牌。
    if def.kind == CardKind::Equip {
        if let Some(slot) = def.slot {
            if g.is_slot_abolished(seat, slot) {
                return Err(format!("{} 栏已废除，不能装备", slot.label()));
            }
        }
    }
    if opts.strict {
        let Some(card) = carrier else {
            return Err("普通使用需要实体牌".into());
        };
        match g.zone_of(card) {
            Some(Zone::Hand(s)) if *s == seat => {}
            // 主公技接力：代出者的手牌直接作为承载牌（不转手主公）；次数/范围仍按主公算。
            Some(Zone::Hand(_)) if opts.foreign_carrier => {}
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
    // 「本回合不能对除其以外的角色使用牌」（惴恐）：按目标列表判定，对自己使用不受限。
    // 用户 2026-11-10 裁定：**技能视为使用不受封锁**（`opts.strict == false` 的 use_virtual/use_as 通道）。
    if opts.strict
        && crate::skills::others_use_blocked(g, seat)
        && planned.iter().any(|t| *t != seat)
    {
        return Err("本回合不能对其他角色使用牌".into());
    }
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
/// 技能视角的合法目标集（激将主动形态：代出后由主公选目标）。
///
/// 与 `legal_uses` 同一口径：候选来自 `target_sets_for`，再用 `can_target` + `plan_use`
/// 逐组复核（strict=true + `foreign_carrier`），保证 legal ≡ execute。
/// 牌名/牌定义由技能侧给出（内核不按牌名分支）。
pub fn target_sets_for_skill(
    g: &Game,
    seat: usize,
    name: &str,
    def: &cards::CardDef,
    carrier: Option<&CardId>,
) -> Vec<Vec<usize>> {
    // 尚未有承载牌时（发起前判断「是否可用」）用非严格校验；有承载牌时用接力口径
    // （严格：次数/攻击范围按使用者，承载牌允许来自他人手牌）。
    let opts = if carrier.is_some() {
        UseOpts::relay()
    } else {
        UseOpts::skill()
    };
    target_sets_for(g, seat, def)
        .into_iter()
        .filter(|ts| ts.iter().all(|t| skills::can_target(g, seat, name, *t, carrier)))
        .filter(|ts| plan_use(g, seat, carrier, name, ts, opts).is_ok())
        .collect()
}

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
        // 目标候选集与转化牌共用同一入口（避免 legal 与 execute 不一致）；
        // 再按 `can_target`（帷幕/谦节等「不能成为目标」）逐目标过滤。
        let mut target_sets: Vec<Vec<usize>> = target_sets_for(g, seat, &def)
            .into_iter()
            .filter(|ts| {
                ts.iter()
                    .all(|t| skills::can_target(g, seat, &name, *t, Some(&id)))
            })
            .collect();
        // 多目标【杀】（方天画戟 / 福绵）：枚举 2..=1+extra 的组合（legality 仍由 plan_use 复核）。
        if def.tags.sha {
            let cands: Vec<usize> = target_sets
                .iter()
                .flat_map(|ts| ts.iter().copied())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if cands.len() >= 2 {
                let extra_room = if g.weapon(seat) == crate::cards::WeaponKind::Fangtian
                    && g.hand(seat).len() == 1
                {
                    2
                } else {
                    skills::extra_targets(g, seat, Some(&id), &name)
                };
                let max_targets = 1 + extra_room;
                let mut combos: Vec<Vec<usize>> = Vec::new();
                fn pick(
                    cands: &[usize],
                    start: usize,
                    need: usize,
                    cur: &mut Vec<usize>,
                    out: &mut Vec<Vec<usize>>,
                ) {
                    if need == 0 {
                        out.push(cur.clone());
                        return;
                    }
                    for i in start..cands.len() {
                        cur.push(cands[i]);
                        pick(cands, i + 1, need - 1, cur, out);
                        cur.pop();
                    }
                }
                for n in 2..=max_targets.min(cands.len()) {
                    let mut cur = Vec::new();
                    pick(&cands, 0, n, &mut cur, &mut combos);
                }
                target_sets.extend(combos);
            }
        }
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
                    format!(
                        "使用【{}】{}",
                        g.card_label(&id),
                        target_suffix(g, &targets)
                    ),
                ));
            }
        }
        // 巧说（-1）：群体锦囊少指定一个目标。
        if matches!(def.tags.target_mode, Some(cards::TargetMode::AllAlive))
            && crate::skills::can_trim_targets(g, seat)
        {
            for t in alive.iter().copied() {
                out.push((
                    format!("trim:{id}:{t}"),
                    format!(
                        "【{}】少指定一个目标（不指定 {}）",
                        g.card_label(&id),
                        g.seats[t].name
                    ),
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
        for targets in target_sets_for(g, seat, &def)
            .into_iter()
            .filter(|ts| {
                ts.iter()
                    .all(|t| skills::can_target(g, seat, conv.to, *t, Some(&conv.from)))
            })
        {
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
                    format!(
                        "【{}】当【{}】{}",
                        g.card_label(&conv.from),
                        conv.to,
                        target_suffix(g, &targets)
                    ),
                ));
            }
        }
    }
    out
}

/// 展示用牌名（视图 `pending.data.card`）：帧数据 → 实体牌 → 兜底「牌」。
fn pending_card_name(g: &Game, data: &Value, card: Option<&CardId>) -> String {
    data["name"]
        .as_str()
        .map(str::to_string)
        .or_else(|| card.map(|c| g.name_of(c)))
        .unwrap_or_else(|| "牌".to_string())
}

/// 选项标签里的目标后缀：` → 张飞、机器人4`（无目标时为空串）。
///
/// 为什么必须带目标：端侧（Web 浮层 `选择目标 / 用法`、QQ 清单）直接用 `description` 渲染按钮，
/// 同一张【杀】对 N 个目标就有 N 个**完全相同**的 `使用【杀】` → 玩家无法区分（真机反馈）。
fn target_suffix(g: &Game, targets: &[usize]) -> String {
    if targets.is_empty() {
        return String::new();
    }
    let names: Vec<&str> = targets
        .iter()
        .filter_map(|t| g.seats.get(*t).map(|s| s.name.as_str()))
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(" → {}", names.join("、"))
    }
}

/// 某个牌定义的目标候选集（legal 枚举与转化牌共用）。
fn target_sets_for(g: &Game, seat: usize, def: &cards::CardDef) -> Vec<Vec<usize>> {
    let alive: Vec<usize> = (0..g.seats.len())
        .filter(|i| g.seats[*i].hp > 0)
        .filter(|i| !(def.tags.delayed && g.seats[*i].judge_abolished))
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
    let mut extra = if g.turn == seat && g.phase == Phase::Play {
        skills::extra_targets(g, seat, carrier.filter(|c| g.is_red(c)), name)
    } else {
        0
    };
    // 结算前钩子追加的目标（谮毁「成为额外目标」）：放宽目标数上限。
    extra += g
        .use_pre
        .as_ref()
        .map(|p| p.extra_targets.len())
        .unwrap_or(0);
    // 方天画戟：【杀】为使用者最后一张手牌时，可额外指定至多两个目标（共 3 个）。
    if def.tags.sha
        && g.weapon(seat) == crate::cards::WeaponKind::Fangtian
        && carrier.is_some_and(|c| g.hand(seat).len() == 1 && &g.hand(seat)[0] == c)
    {
        extra += 2;
    }
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
            if !skills::can_target(g, seat, name, *t, carrier) {
                return Err("目标不能成为此牌目标".into());
            }
            // 判定区已废除（雄乱）：延时锦囊不能指定该角色。
            if def.tags.delayed && g.seats[*t].judge_abolished {
                return Err("目标判定区已废除".into());
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
            // 禁酒（界高顺）：高顺自己不能把【酒】当【酒】；其他角色不能在其回合使用【酒】。
            if crate::skills::jiu_blocked(g, seat) {
                return Err("禁酒：本回合不能使用【酒】".into());
            }
            if g.counter(&format!("turn.jiu_used:{seat}")) > 0
                && !crate::skills::jiu_unlimited(g, seat)
            {
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
            // 巧说（少指定一个目标）：群体锦囊可裁掉一个目标。
            if let Some(t) = skills::aoe_trim_target(g, seat) {
                v.retain(|x| *x != t);
            }
            Ok(v)
        }
        Effect::SavageAssault | Effect::ArcheryAttack => {
            let mut v = left_alive;
            // 乱击（aoe-target-trim）：少选一个目标。
            if let Some(t) = skills::aoe_trim_target(g, seat) {
                v.retain(|x| *x != t);
            }
            // 巨象：不能成为【南蛮入侵】的目标（target_blocker）。
            v.retain(|t| skills::can_target(g, seat, name, *t, carrier));
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
                // 多指定目标时「无距离限制」（福绵②/天义）；界武圣的方片【杀】也无距离；
                // 酒诗（背面朝上时的落英牌）/ 当先（获得的无距离【杀】）同样无距离。
                let no_dist = skills::extra_targets_no_distance(g, seat, carrier, name)
                    || crate::skills::sha_no_distance(g, seat, card)
                    || crate::skills::card_no_distance(g, seat, Some(card));
                if !no_dist && !g.can_reach(seat, many[0], card) {
                    return Err("目标不在攻击范围内".into());
                }
                if !crate::skills::sha_limit_exempt(g, seat, &many)
                    && !crate::skills::sha_unlimited_card(g, seat, carrier)
                    && g.sha_used(seat) >= g.sha_limit(seat)
                {
                    return Err("本阶段【杀】使用次数已用完".into());
                }
                for t in &many {
                    if crate::skills::sha_blocked_for(g, seat, *t) {
                        return Err("陷阵没赢：本回合不能对其使用【杀】".into());
                    }
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
                && !crate::skills::card_no_distance(g, seat, carrier)
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
            // 【兵粮寸断】距离 1（断粮：本回合未造成伤害 → 无距离；奇才同理）。
            if def.effect == Effect::SupplyShortage
                && !skills::supply_shortage_no_distance(g, seat)
                && !skills::card_no_distance(g, seat, carrier)
                && g.distance(seat, many[0]) > 1
            {
                return Err("【兵粮寸断】只能对距离 1 的角色使用".into());
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
            if !skills::can_target(g, seat, name, a, carrier)
                || !skills::can_target(g, seat, name, b, carrier)
            {
                return Err("目标不能成为此牌目标".into());
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

/// 终局回血无效（房规，2026-11-11 裁定）：`round > 100` 后【桃】/【桃园结义】仍可
/// 正常使用、结算、被无懈，但**不再回复体力**。
///
/// 返回 `true` 表示本次已判为「无效」并已记日志，调用方直接收尾。
fn endgame_void_heal(g: &mut Game, source: usize, card_name: &str) -> bool {
    if !g.endgame_no_heal() {
        return false;
    }
    g.push_log(format!(
        "终局（第 {} 轮，已过 {} 轮）：{} 使用【{}】回血无效",
        g.round,
        crate::kernel::ENDGAME_ROUND,
        g.seats[source].name,
        card_name
    ));
    true
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
    // 结算前钩子（谮毁）：先询问「代用 / 额外目标」，再开始结算。
    if crate::skills::pre_use_owner(g, seat, Some(card), &name, &targets).is_some() {
        super::resolve::start_pre_use(g, seat, Some(card), &name, targets);
        return super::resolve::advance(g);
    }
    start_use(g, seat, Some(card), &name, targets, UseOpts::normal())?;
    super::resolve::advance(g)
}

/// 结算前钩子的步进（`FlowKind::PreUse`）：先让技能询问，再真正开始结算。
pub fn step_pre_use(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let user = a.data["user"].as_u64().unwrap_or(0) as usize;
    let card: Option<CardId> = a.data["card"].as_str().map(str::to_string);
    let name = a.data["name"].as_str().unwrap_or_default().to_string();
    let targets: Vec<usize> = a.data["targets"]
        .as_array()
        .map(|v| v.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect())
        .unwrap_or_default();
    let asked = a.data["asked"].as_bool().unwrap_or(false);
    if !asked {
        // 技能压入自己的询问帧（在当前帧之上）。
        if let Some(code) = crate::skills::pre_use_owner(g, user, card.as_ref(), &name, &targets) {
            crate::skills::start_pre_use_ask(g, code, user, card.as_ref(), &name, &targets);
        }
        let mut data = a.data.clone();
        data["asked"] = json!(true);
        return Ok(Step::Next(Stage::PreUse, data));
    }
    // 技能已给出结果：代用 / 额外目标。
    // 注意：`use_pre` 在 `start_use` 期间**保留**（`planned_targets` 用它放宽目标数上限），
    // 结算帧压入后再清空。
    let pre = g.use_pre.clone().unwrap_or_default();
    let switch_user = pre.switch_user;
    let mut final_targets = targets.clone();
    for t in pre.extra_targets {
        if !final_targets.contains(&t) {
            final_targets.push(t);
        }
    }
    let opts = UseOpts {
        strict: true,
        damage_source: switch_user,
        no_wuxie: false,
        foreign_carrier: false,
    };
    // 使用者身份：代用时记账在**原使用者**（牌在其实体区域），伤害来源改为协助者。
    start_use(g, user, card.as_ref(), &name, final_targets, opts)?;
    g.use_pre = None;
    Ok(Step::Done)
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
    // 「本轮未有角色使用过」记账（矫诏）：所有角色的使用都记，随轮次清空。
    g.bump(&format!("round.used:{name}"));
    if let Some(card) = carrier {
        if g.seats[seat].flags.contains("xiongluan_active") {
            g.mark_unresponsive(card);
        }
        g.emit(Event::card(
            EventKind::CardUsed,
            Some(seat),
            card.clone(),
            None,
        ));
        g.used_this_turn.push((seat, card.clone()));
    } else {
        g.emit(Event::simple(
            EventKind::CardUsed,
            Some(seat),
            Some(name),
        ));
        // 纯虚拟【杀】也占用本阶段次数（`sha_used` 派生时计入）。
        if def.tags.sha {
            g.bump(&format!("turn.sha_virtual:{seat}"));
        }
    }
    g.begin_action_as(seat);
    // 福绵②：本回合「可以多指定目标」的机会用掉一次（即便只指定了 1 个目标）。
    if crate::skills::extra_targets(g, seat, carrier, name) > 0 {
        crate::skills::consume_extra_targets(g, seat);
    }
    // 一张牌被使用：巧说消耗「巧」/ 自守统计伤害牌 / 酒诗标记落英牌不可响应。
    crate::skills::on_card_use(g, seat, carrier, name);
    if !planned.is_empty() {
        crate::skills::on_targets_chosen(g, seat, &planned);
    }
    if !planned.is_empty() {
        let targets_str = planned
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",");
        match carrier {
            Some(card) => g.emit(Event::card(
                EventKind::TargetChosen,
                Some(seat),
                card.clone(),
                Some(targets_str),
            )),
            None => g.emit(Event::simple(
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

    // 「成为普通锦囊目标后」拦截（贞烈）：逐目标询问；可令此牌对该目标无效。
    if def.kind == CardKind::Trick && !def.tags.wuxie && !def.tags.delayed {
        loop {
            let targets = target_list(&data);
            let cursor = data["ticursor"].as_u64().unwrap_or(0) as usize;
            if cursor >= targets.len() {
                break;
            }
            if data["ti_wait"].as_bool().unwrap_or(false) {
                data["ti_wait"] = json!(false);
                let (target, cancelled) = g
                    .trick_intercept
                    .take()
                    .unwrap_or((targets[cursor], false));
                if cancelled {
                    let mut list = target_list(&data);
                    list.retain(|x| *x != target);
                    data["targets"] = json!(list);
                    g.push_log(format!(
                        "贞烈：此锦囊对 {} 无效",
                        g.seats[target].name
                    ));
                    data["ticursor"] = json!(cursor.min(list.len()));
                    continue;
                }
                data["ticursor"] = json!(cursor + 1);
                continue;
            }
            if let Some((owner, code)) =
                crate::skills::trick_intercept_owner(g, source, &name, targets[cursor], card_ref)
            {
                data["ti_wait"] = json!(true);
                data["ticursor"] = json!(cursor);
                let mut payload = json!({
                    "trick_intercept": true,
                    "user": source,
                    "target": targets[cursor],
                    "name": name,
                    "card": card,
                });
                payload["target"] = json!(targets[cursor]);
                crate::kernel::resolve::push_ask(g, code, Stage::Choose, owner, payload);
                return Ok(Step::Next(stage, data));
            }
            data["ticursor"] = json!(cursor + 1);
        }
        let mut list = target_list(&data);
        // 结算中追加目标（求援：伤害锦囊的「成为额外目标」）。
        if !g.extra_targets_mid.is_empty() {
            for t in std::mem::take(&mut g.extra_targets_mid) {
                if !list.contains(&t) && g.seats.get(t).is_some_and(|s| s.hp > 0) {
                    list.push(t);
                }
            }
            data["targets"] = json!(list);
        }
        let abort = list.is_empty() || (def.effect == Effect::Collateral && list.len() < 2);
        if abort {
            finish(g, card_ref, "target_void")?;
            return Ok(Step::Done);
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
                if endgame_void_heal(g, source, "桃") {
                    finish(g, card_ref, "use")?;
                    return Ok(Step::Done);
                }
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
                if endgame_void_heal(g, source, "桃园结义") {
                    finish(g, card_ref, "use")?;
                    return Ok(Step::Done);
                }
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
    // 一张【杀】结算结束（疠火「此牌结算后」等）。
    if let Some(card) = card {
        crate::skills::on_card_finished(g, source, card);
    }
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
        // 「装备窗口」回来后的收尾：`GearResult` 决定「重新响应 / 跳过伤害 / 继续」。
        if data["gear_wait"].as_bool().unwrap_or(false) {
            data["gear_wait"] = json!(false);
            let phase = data["gear_phase"].as_str().unwrap_or("").to_string();
            data["gear_phase"] = json!("");
            let result = g.gear_result.take().unwrap_or(gear::GearResult::Declined);
            let gt = num(data, "gear_target");
            match phase.as_str() {
                // 雌雄：选完继续正常询问【闪】。
                "cixiong" => {}
                // 青龙：目标需重新响应；贯石/放弃 → 本目标结算完毕。
                "dodge" => match result {
                    gear::GearResult::Retry => {
                        data["asked"] = json!(false);
                        data["need"] = json!(0);
                        data["got"] = json!(0);
                        continue;
                    }
                    // 贯石斧：弃 2 张后强行造成伤害。
                    gear::GearResult::Accepted => {
                        let step =
                            sha_hit(g, source, damage_source, gt, def, card, data, "guanshi")?;
                        if let Some(step) = step {
                            return Ok(Some(step));
                        }
                        cursor = num(data, "cursor");
                        continue;
                    }
                    _ => {
                        skills::on_sha_finished(g, source, Some(gt), card, None, false);
                        cursor += 1;
                        data["cursor"] = json!(cursor);
                        continue;
                    }
                },
                // 寒冰：伤害被防止；否则照常造成伤害（直接进入命中结算）。
                "pre_damage" => {
                    if result == gear::GearResult::Prevented {
                        skills::on_sha_finished(g, source, Some(gt), card, None, false);
                        cursor += 1;
                        data["cursor"] = json!(cursor);
                        continue;
                    }
                    let step = sha_hit(g, source, damage_source, gt, def, card, data, phase.as_str())?;
                    if let Some(step) = step {
                        return Ok(Some(step));
                    }
                    continue;
                }
                // 麒麟：弃坐骑后本目标结束。
                _ => {
                    skills::on_sha_finished(g, source, Some(gt), card, None, false);
                    cursor += 1;
                    data["cursor"] = json!(cursor);
                    continue;
                }
            }
        }
        if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) || g.fenwei_void.contains(&t) {
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        // ① 读取介入技能的结果（技能帧在上方执行完毕后回到本帧）。
        if data["intercept_wait"].as_bool().unwrap_or(false) {
            let r = g.sha_intercept.take().unwrap_or(ShaIntercept::Continue);
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
                    let step = sha_hit(g, source, damage_source, t, def, card, data, "intercept")?;
                    if let Some(step) = step {
                        return Ok(Some(step));
                    }
                    cursor = num(data, "cursor");
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
        // 结算中追加目标（求援）：追加到列表末尾，不重置进度。
        if !g.extra_targets_mid.is_empty() {
            for t2 in std::mem::take(&mut g.extra_targets_mid) {
                if !targets.contains(&t2) && g.seats.get(t2).is_some_and(|s| s.hp > 0) {
                    targets.push(t2);
                }
            }
            data["targets"] = json!(targets);
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
        // ③ 雌雄双股剑：指定目标后（每个目标一次）。
        if !data["cixiong_done"]
            .get(t.to_string())
            .is_some_and(|v| v.as_bool() == Some(true))
            && gear::has_window(g, gear::GearTag::TargetChosen, source, t)
        {
            data["cixiong_done"][t.to_string()] = json!(true);
            data["gear_wait"] = json!(true);
            data["gear_phase"] = json!("cixiong");
            data["gear_target"] = json!(t);
            data["cursor"] = json!(cursor);
            gear::start(g, gear::GearTag::TargetChosen, source, t, card);
            return Ok(Some(Step::Next(Stage::CardOpt, data.clone())));
        }
        // I1：并发窗口的过期回答 → 丢弃并**重新询问**（不得代判「未响应」）。
        if std::mem::take(&mut g.response_stale) {
            data["asked"] = json!(false);
            data["need"] = json!(0);
            data["got"] = json!(0);
        }
        if data["asked"].as_bool().unwrap_or(false) {
            // 蛊惑延后结果优先；未就绪则保持 asked，等待流程结束重入。
            let outcome = match g.guhuo_result.take() {
                Some(r) => Some(r.is_some()),
                None => apply_response(g, t, ResponseKind::Shan, card)?,
            };
            let Some(responded) = outcome else {
                return Ok(Some(Step::Next(Stage::CardAsk, data.clone())));
            };
            data["asked"] = json!(false);
            if responded {
                let got = num(data, "got") + 1;
                data["got"] = json!(got);
                let need = num(data, "need").max(1);
                if got >= need {
                    data["need"] = json!(0);
                    data["got"] = json!(0);
                    // 界咆哮：【杀】被抵消后，本回合下一次【杀】伤害 +1。
                    if crate::skills::paoxiao_after_dodge_bonus(g, damage_source) {
                        g.counters.insert("turn.paoxiao_bonus".to_string(), 1);
                        g.push_log("咆哮：【杀】被抵消，本回合下一次【杀】伤害 +1".to_string());
                    }
                    // 青龙偃月刀 / 贯石斧：被【闪】抵消后的追加窗口（每个目标一次）。
                    if !data["dodge_gear_done"]
                        .get(t.to_string())
                        .is_some_and(|v| v.as_bool() == Some(true))
                        && gear::has_window(g, gear::GearTag::AfterDodge, source, t)
                    {
                        data["dodge_gear_done"][t.to_string()] = json!(true);
                        data["gear_wait"] = json!(true);
                        data["gear_phase"] = json!("dodge");
                        data["gear_target"] = json!(t);
                        data["cursor"] = json!(cursor);
                        gear::start(g, gear::GearTag::AfterDodge, source, t, card);
                        return Ok(Some(Step::Next(Stage::CardOpt, data.clone())));
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
                let step = sha_hit(g, source, damage_source, t, def, card, data, "response")?;
                if let Some(step) = step {
                    return Ok(Some(step));
                }
                cursor = num(data, "cursor");
                continue;
            }
        }
        // 不能被响应的牌（往烈/雄乱）：直接结算伤害。
        if !unresponsive {
            if let Some(p) = response_pending(g, t, ResponseKind::Shan, Some(source), card) {
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
        let step = sha_hit(g, source, damage_source, t, def, card, data, "no_response")?;
        if let Some(step) = step {
            return Ok(Some(step));
        }
        cursor = num(data, "cursor");
    }
    Ok(None)
}

/// 一次【杀】命中结算：寒冰剑（伤害前）/ 麒麟弓（伤害后）窗口、古锭刀 +1、朱雀羽扇火属性。
///
/// 返回 `Some(Step)` = 已压入装备窗口等待；`None` = 当前目标已结算完（`cursor` 已 +1）。
#[allow(clippy::too_many_arguments)]
fn sha_hit(
    g: &mut Game,
    source: usize,
    damage_source: usize,
    t: usize,
    def: &cards::CardDef,
    card: Option<&CardId>,
    data: &mut Value,
    cause: &str,
) -> Result<Option<Step>, String> {
    // 寒冰剑：造成伤害前（每个目标一次；从窗口回来时 cause == "pre_damage"）。
    if cause != "pre_damage"
        && !data["damage_gear_done"]
            .get(t.to_string())
            .is_some_and(|v| v.as_bool() == Some(true))
        && gear::has_window(g, gear::GearTag::BeforeDamage, source, t)
    {
        data["damage_gear_done"][t.to_string()] = json!(true);
        data["gear_wait"] = json!(true);
        data["gear_phase"] = json!("pre_damage");
        data["gear_target"] = json!(t);
        gear::start(g, gear::GearTag::BeforeDamage, source, t, card);
        return Ok(Some(Step::Next(Stage::CardOpt, data.clone())));
    }
    let mut amount = 1 + jiu_take_bonus(g);
    if amount > 1 {
        g.counters
            .insert(format!("turn.jiu_boost:{t}"), (amount - 1) as u32);
    }
    // 古锭刀：目标无手牌时伤害 +1。
    if g.weapon(source) == crate::cards::WeaponKind::Gudingdao && g.hand(t).is_empty() {
        amount += 1;
        g.push_log("古锭刀：目标无手牌，伤害 +1".to_string());
    }
    // 朱雀羽扇：普通【杀】改为火属性伤害。
    let mut hit_def = *def;
    if g.weapon(source) == crate::cards::WeaponKind::Zhuque
        && hit_def.effect == Effect::Sha
        && hit_def.damage_kind == crate::cards::DamageKind::Normal
    {
        hit_def.damage_kind = crate::cards::DamageKind::Fire;
    }
    deal_damage(g, damage_source, t, amount, &hit_def, card);
    // 麒麟弓：造成伤害后（每个目标一次）。
    if !data["qilin_done"]
        .get(t.to_string())
        .is_some_and(|v| v.as_bool() == Some(true))
        && gear::has_window(g, gear::GearTag::AfterDamage, source, t)
    {
        data["qilin_done"][t.to_string()] = json!(true);
        data["gear_wait"] = json!(true);
        data["gear_phase"] = json!("post_damage");
        data["gear_target"] = json!(t);
        gear::start(g, gear::GearTag::AfterDamage, source, t, card);
        return Ok(Some(Step::Next(Stage::CardOpt, data.clone())));
    }
    skills::on_sha_finished(g, source, Some(t), card, None, true);
    let cursor = num(data, "cursor") + 1;
    data["cursor"] = json!(cursor);
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
    let targets = targets.to_vec();
    let mut damage_source = damage_source_of(data, source);
    // 祸首：其他角色使用【南蛮入侵】时伤害来源改为祸首持有者。
    if let Some(owner) = crate::skills::aoe_damage_source(g, damage_source, card) {
        damage_source = owner;
    }
    let unresponsive = card.map(|c| g.is_unresponsive(c)).unwrap_or(false);
    while cursor < targets.len() {
        let t = targets[cursor];
        if g.seats.get(t).map(|s| s.hp <= 0).unwrap_or(true) || g.fenwei_void.contains(&t) {
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        // I1：并发窗口的过期回答 → 丢弃并重新询问。
        if std::mem::take(&mut g.response_stale) {
            data["asked"] = json!(false);
            data["need"] = json!(0);
            data["got"] = json!(0);
        }
        if data["asked"].as_bool().unwrap_or(false) {
            let outcome = match g.guhuo_result.take() {
                Some(r) => Some(r.is_some()),
                None => apply_response(g, t, kind, card)?,
            };
            let Some(responded) = outcome else {
                return Ok(Step::Next(Stage::CardOpt, data.clone()));
            };
            data["asked"] = json!(false);
            if !responded {
                deal_damage(g, damage_source, t, 1, def, card);
            }
            cursor += 1;
            data["cursor"] = json!(cursor);
            continue;
        }
        if !unresponsive {
            if let Some(p) =
                response_pending(g, t, kind, Some(damage_source), card)
            {
                data["asked"] = json!(true);
                return need_at(g, Stage::CardAsk, data.clone(), p);
            }
        }
        deal_damage(g, damage_source, t, 1, def, card);
        cursor += 1;
        data["cursor"] = json!(cursor);
    }
    finish(g, card, "use")?;
    // 牌结算结束后（巨象：其他角色的【南蛮入侵】结算结束后获得之）。
    if let Some(card) = card {
        crate::skills::on_card_finished(g, source, card);
    }
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

    // I1：并发窗口的过期回答 → 丢弃并重新询问。
    if std::mem::take(&mut g.response_stale) {
        data["asked"] = json!(false);
        data["need"] = json!(0);
        data["got"] = json!(0);
    }
    if data["asked"].as_bool().unwrap_or(false) {
        let outcome = match g.guhuo_result.take() {
            Some(r) => Some(r.is_some()),
            None => apply_response(g, responder, ResponseKind::Sha, card)?,
        };
        let Some(responded) = outcome else {
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        };
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
        if let Some(p) =
            response_pending(g, responder, ResponseKind::Sha, Some(opponent), card)
        {
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
    if !g.must_ask(target, !options.is_empty()) {
        return advance_target(g, card, data);
    }

    if data["asked"].as_bool().unwrap_or(false) {
        // I1：回答缺失（被嵌套流程消费）→ 重新询问，不得代玩家放弃。
        let Some(ans) = read_answer(g)? else {
            data["asked"] = json!(false);
            return Ok(Step::Next(Stage::CardPick, data.clone()));
        };
        data["asked"] = json!(false);
        if ans == "pass" {
            return advance_target(g, card, data);
        }
        if !options.iter().any(|(id, _)| *id == ans) {
            return Err(format!("非法选牌：{ans}"));
        }
        let picked = if ans == "pick:hand" {
            // 目标手牌不可见，应由**随机**一张决定（v1 `next_index(n)` 语义；
            // 取第一张会变成可预测的固定位置，属于引擎代选，见 I1）。
            let n = g.hand(target).len();
            if n == 0 {
                None
            } else {
                let idx = g.rand_below(n);
                g.hand(target).get(idx).cloned()
            }
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
        b.data(json!({
            "card": pending_card_name(g, data, card),
            "target": g.seats[target].name,
            "stage": "single",
        }))
        // 拆/顺**必须**选一张（不能取消整张牌）：出口只作类型占位，端侧不展示。
        .escape_mandatory("必须选择一张牌")
        .build(),
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
            // I1：回答缺失（被嵌套流程消费）→ 重新询问，不得代玩家放弃。
            let Some(ans) = read_answer(g)? else {
                data["asked"] = json!(false);
                return Ok(Step::Next(Stage::CardOpt, data.clone()));
            };
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
        // 投影所需数据必须进 pending：`special_zones.wugu` 读的是 `pending.data`（不是帧数据）。
        // 2026-11-11 真机：缺这段 → 五谷特殊区没有牌可点，对局卡住。
        return need_at(
            g,
            Stage::CardOpt,
            data.clone(),
            b.data(json!({ "pool": pool, "pickers": pickers, "cursor": cursor }))
                // 规则：每名目标角色**依次获得其中一张**（不能拒绝）→ 出口只作类型占位。
                .escape_mandatory("必须取一张")
                .build(),
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

    // 界火计：目标随机展示手牌，弃同**颜色**的牌（手牌或装备区）。
    let color_mode = crate::skills::fire_attack_color_mode(g, source);
    if step != "discard" {
        if color_mode && !data["asked"].as_bool().unwrap_or(false) {
            let hand = g.hand(target);
            if hand.is_empty() {
                return advance_target(g, card, data);
            }
            let idx = g.rand_below(hand.len());
            let id = hand[idx].clone();
            g.reveal(&id, Some(target), "fire_attack");
            data["shown"] = json!(id);
            data["step"] = json!("discard");
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        }
        if data["asked"].as_bool().unwrap_or(false) {
            // I1：回答缺失（被嵌套流程消费）→ 重新询问，不得代玩家放弃。
            let Some(ans) = read_answer(g)? else {
                data["asked"] = json!(false);
                return Ok(Step::Next(Stage::CardOpt, data.clone()));
            };
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
        // 展示手牌是**强制步骤**（目标只能选展示哪一张，不能拒绝）：出口用 `escape_mandatory`
        // （端侧不展示，提交出口 = 自动选第一张）。2026-11-11 真机：旧实现给了「放弃展示」，
        // 目标可以取消整张【火攻】（v1 无此问题）。
        return need_at(
            g,
            Stage::CardOpt,
            data.clone(),
            b.data(json!({
                "card": pending_card_name(g, data, card),
                "target": g.seats[target].name,
                "stage": "reveal",
            }))
            .escape_mandatory("必须展示一张手牌")
            .build(),
        );
    }

    let shown = data["shown"].as_str().unwrap_or_default().to_string();
    if data["asked"].as_bool().unwrap_or(false) {
        // I1：回答缺失（被嵌套流程消费）→ 重新询问，不得代玩家放弃。
        let Some(ans) = read_answer(g)? else {
            data["asked"] = json!(false);
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        };
        data["asked"] = json!(false);
        if ans != "pass" {
            let id = ans
                .strip_prefix("discard:")
                .ok_or_else(|| format!("火攻：未知选项 {ans}"))?
                .to_string();
            // legal 与 execute **同源**：`color_mode`（界火计）的候选含装备区牌，
            // 这里必须一并放行（否则 AI 会拿到 legal 里的 `discard:<装备>` 却被拒：
            // 2026-11-11 自对弈 seed=1 game 4 抓到 illegal）。
            let in_hand = g.hand(source).contains(&id);
            let in_equip = color_mode
                && g
                    .ids_in(|z| matches!(z, crate::kernel::Zone::Equip(s, _) if *s == source))
                    .contains(&id);
            if !(in_hand || in_equip) {
                return Err("火攻：弃置的牌不在手牌/装备区".into());
            }
            let same = if color_mode {
                let base = crate::skills::effective_suit(g, source, &shown).map(|s| s.is_red());
                let cur = crate::skills::effective_suit(g, source, &id).map(|s| s.is_red());
                base.is_some() && base == cur
            } else {
                crate::skills::effective_suit(g, source, &id)
                    == crate::skills::effective_suit(g, source, &shown)
            };
            if !same {
                return Err("火攻：弃置的牌需与展示牌同花色".into());
            }
            g.to_discard(&id, "fire_attack")?;
            deal_damage(g, damage_source_of(data, source), target, 1, def, card);
        }
        g.conceal(&shown, "fire_attack");
        return advance_target(g, card, data);
    }

    let ids: Vec<CardId> = if color_mode {
        let base = crate::skills::effective_suit(g, source, &shown).map(|s| s.is_red());
        let mut pool = g.hand(source);
        pool.extend(g.ids_in(|z| matches!(z, crate::kernel::Zone::Equip(s, _) if *s == source)));
        pool.into_iter()
            .filter(|id| {
                base.is_some()
                    && crate::skills::effective_suit(g, source, id).map(|s| s.is_red()) == base
            })
            .collect()
    } else {
        let suit = crate::skills::effective_suit(g, source, &shown);
        g.hand(source)
            .into_iter()
            .filter(|id| crate::skills::effective_suit(g, source, id) == suit)
            .collect()
    };
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
        b.data(json!({
            "card": pending_card_name(g, data, card),
            "target": g.seats[target].name,
            "stage": "discard",
        }))
        .escape("不弃置（火攻无效）")
        .build(),
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
        // I1：回答缺失（被嵌套流程消费）→ 重新询问，不得代玩家放弃。
        let Some(ans) = read_answer(g)? else {
            data["asked"] = json!(false);
            return Ok(Step::Next(Stage::CardOpt, data.clone()));
        };
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
        p.data(json!({
            "step": "sha",
            "target": g.seats[b].name,
            "card": pending_card_name(g, data, card),
        }))
        .escape("不打出【杀】（交出武器）")
        .build(),
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
        // 父魂：本张牌只能用同色手牌响应。
        if kind == ResponseKind::Shan {
            if let Some(red) = g.response_color_lock {
                if g.is_red(&id) != red {
                    continue;
                }
            }
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
    // 主公技接力（激将/护驾）：响应窗口里的**唯一入口**（选中后进入子流程）。
    for (id, label) in skills::relay_options(g, target, kind) {
        out.push((id, label));
    }
    // 两张手牌当【杀】打出（响应窗口）：父魂（技能）或丈八蛇矛（装备）。
    if kind == ResponseKind::Sha && two_card_sha_available(g, target) {
        for (c1, c2) in skills::two_card_sha_pairs(g, target) {
            out.push((
                format!("pair:{c1},{c2}"),
                format!(
                    "将【{}】【{}】当【杀】打出",
                    g.card_label(&c1),
                    g.card_label(&c2)
                ),
            ));
        }
    }
    // 蛊惑（于吉）：扣置一张手牌当本次需要的牌打出（延后结算：质疑链后再回传结果）。
    if (kind == ResponseKind::Shan || kind == ResponseKind::Sha)
        && skills::guhuo_available(g, target)
    {
        let name = if kind == ResponseKind::Shan { "闪" } else { "杀" };
        out.push((
            "guhuo".to_string(),
            format!("蛊惑：扣置一张手牌当【{name}】打出"),
        ));
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
    source: Option<usize>,
    card: Option<&CardId>,
) -> Option<super::pending::Pending> {
    g.response_source = source;
    let mut options = response_options(g, target, kind);
    // 规则层面「不能响应」（诈降红【杀】/ 毅重②）：这不是「没牌可打」，
    // 而是合法集为空 —— 与 v1 的 `no_shan` 同口径，直接不询问。
    let mut forbidden = false;
    if kind == ResponseKind::Shan && crate::skills::block_real_shan(g, target) {
        options.retain(|(id, _)| !id.starts_with("respond:"));
        forbidden = true;
    }
    // 毅重②（及其他「无法响应」类）：按（响应者, 使用者, 此牌）逐张判定。
    if kind == ResponseKind::Shan && crate::skills::shan_blocked_for(g, target, source, card) {
        options.retain(|(id, _)| !id.starts_with("respond:") && !id.starts_with("armor:"));
        forbidden = true;
    }
    if options.is_empty() && forbidden {
        return None;
    }
    // I1 询问完整性：只有「合法动作集为空」（无牌可打）才不询问；有手牌必须显式询问。
    if !g.must_ask(target, !options.is_empty()) {
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

/// 「两张手牌当【杀】打出」可用：父魂（技能）或丈八蛇矛（武器）。
///
/// v1 对齐：v1 的 `respond:<c1>,<c2>` 同时支持丈八蛇矛与父魂；v2 迁移时只做了父魂。
fn two_card_sha_available(g: &Game, seat: usize) -> bool {
    skills::two_card_sha(g, seat) || g.weapon(seat) == crate::cards::WeaponKind::Zhangba
}

/// 处理一次响应回答，返回 `true` = 已响应（抵消/免疫）。
fn apply_response(
    g: &mut Game,
    actor: usize,
    kind: ResponseKind,
    used: Option<&CardId>,
) -> Result<Option<bool>, String> {
    // 主公技接力子流程的结果（唯一通道，单次消费）：必须在读回答之前检查——
    // 子流程结束时本次响应窗口并没有新的 `answer`。
    if let Some(r) = g.relay_result.take() {
        match r {
            crate::kernel::RelayOutcome::Played { .. } => return Ok(Some(true)),
            crate::kernel::RelayOutcome::Declined => {
                // 全部拒绝 → 回到主公本人的响应窗口（不是「未响应」）。
                g.response_stale = true;
                return Ok(None);
            }
        }
    }
    // I1：回答缺失（被嵌套流程消费）→ 标记后由调用方重新询问；不得代玩家放弃。
    let Some(ans) = read_answer(g)? else {
        g.response_stale = true;
        return Ok(None);
    };
    // 蛊惑：扣置声明 + 质疑链，结果延后回传（内核对「响应是否成立」只认结果通道）。
    if ans == "guhuo" {
        crate::skills::start_guhuo_response(g, actor, kind)?;
        return Ok(None);
    }
    g.response_context = None;
    if ans == "pass" {
        return Ok(Some(false));
    }
    if let Some(id) = ans.strip_prefix("respond:") {
        let id = id.to_string();
        if !g.hand(actor).contains(&id) {
            // 并发响应窗口：回答可能来自已关闭的旧窗口（牌已被消费/移走）。
            // I1：不得把过期回答当作「未响应」代玩家放弃 → 标记后由调用方**重新询问**。
            g.push_log(format!("响应窗口已变化：{id} 不在手牌，重新询问"));
            g.response_stale = true;
            return Ok(None);
        }
        if !cards::responds_as(&g.name_of(&id), kind) {
            return Err("响应：该牌不能作为此环节的响应牌".into());
        }
        if !g.can_use_card(actor, &id) {
            return Err("响应：该牌已被冻结".into());
        }
        // 父魂：本张牌只能用同色手牌响应（legal 与 execute 同源）。
        if kind == ResponseKind::Shan {
            if let Some(red) = g.response_color_lock {
                if g.is_red(&id) != red {
                    return Err("响应：父魂要求打出同色手牌".into());
                }
            }
            // 毅重②：手牌数 ≤ 你的角色无法响应你的黑色【杀】。
            if crate::skills::shan_blocked_for(g, actor, g.response_source, used) {
                return Err("响应：此【杀】不能被响应".into());
            }
        }
        // 打出记账（矫诏：本轮使用与打出都算）。
        g.bump(&format!("round.used:{}", g.name_of(&id)));
        g.to_discard(&id, "respond")?;
        let label = g.name_of(&id);
        g.emit(Event::card(
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
        return Ok(Some(true));
    }
    if ans == "armor:bagua" {
        if g.armor(actor) != ArmorKind::Bagua {
            return Err("八卦阵：未装备【八卦阵】".into());
        }
        let Some(judge) = flow::judge_card(g, actor) else {
            return Ok(Some(false));
        };
        // 花色读取单入口（红颜：小乔的黑桃判定牌视为红桃）。
        let red = crate::skills::effective_suit(g, actor, &judge)
            .map(|s| s.is_red())
            .unwrap_or(false);
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
        return Ok(Some(red));
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
        // 打出记账（转化牌按生效牌名）。
        g.bump(&format!("round.used:{label}"));
        g.emit(Event::card(
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
        return Ok(Some(true));
    }
    if let Some(pair) = ans.strip_prefix("pair:") {
        // 父魂：两张手牌当【杀】打出。
        if kind != ResponseKind::Sha {
            return Err("两张手牌当【杀】只能用于响应【杀】".into());
        }
        let (c1, c2) = pair
            .split_once(',')
            .ok_or_else(|| "响应：需要两张手牌".to_string())?;
        let (c1, c2) = (c1.to_string(), c2.to_string());
        if !two_card_sha_available(g, actor) {
            return Err("响应：没有可将两张手牌当【杀】的技能/装备".into());
        }
        if !g.hand(actor).contains(&c1) || !g.hand(actor).contains(&c2) || c1 == c2 {
            return Err("响应：两张牌不合法".into());
        }
        if !g.can_use_card(actor, &c1) || !g.can_use_card(actor, &c2) {
            return Err("响应：该牌已被冻结".into());
        }
        g.to_discard(&c1, "respond")?;
        g.to_discard(&c2, "respond")?;
        g.bump("round.used:杀");
        let label = format!("{}、{}", g.card_label(&c1), g.card_label(&c2));
        g.push_log(format!(
            "{} 将【{}】当【杀】打出响应",
            g.seats[actor].name, label
        ));
        crate::skills::on_response_played(g, used, actor, &c1);
        return Ok(Some(true));
    }
    if ans == "jijiang" {
        // 主公技接力：压入子流程，等它把结果写进 `relay_result` 后由本函数消费。
        crate::skills::start_relay(g, actor, kind, used);
        return Ok(None);
    }
    if kind == ResponseKind::Shan && skills::dodge_use(g, actor, &ans)? {
        return Ok(Some(true));
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
    // 花色读取单入口（红颜：黑桃 → 红桃）。
    let suit = crate::skills::effective_suit(g, seat, &judge);
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
    // 界酒池（用户裁定 Q1=B）：同回合连喝多杯**叠加**，一次取走全部层数。
    let n = g.counter("turn.jiu_bonus") as i32;
    if n > 0 {
        g.counters.insert("turn.jiu_bonus".to_string(), 0);
    }
    n
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
    // 父魂的同色响应限制只对本次使用的牌生效。
    g.response_color_lock = None;
    g.response_source = None;
    let Some(card) = card else {
        return Ok(());
    };
    if !matches!(
        g.zone_of(card),
        Some(Zone::Discard)
            | Some(Zone::Judge(_))
            | Some(Zone::Equip(_, _))
            // 鞬出（基本牌分支）：此【杀】被目标获得，仍留在其手牌。
            | Some(Zone::Hand(_))
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
    // 只能写回**本牌结算帧**（Card/Judge）。handler 可能已经在栈顶压了子帧
    // （例如伤害触发濒死 Dying 帧）——写 `stack.last_mut()` 会把牌结算的 data
    // 覆盖到濒死帧上，导致濒死读到别人的 victim（僵尸座位 / 不结算死亡）。
    let Some(Frame::Flow(f)) = g
        .stack
        .iter_mut()
        .rev()
        .find(|f| matches!(f, Frame::Flow(ff) if matches!(ff.kind, FlowKind::Card | FlowKind::Judge)))
    else {
        return Err("牌结算：找不到发起帧（Card/Judge）".into());
    };
    f.stage = stage;
    f.data = data;
    Ok(Step::Need(p))
}

/// 读取当前回答。`None` = 尚未作答（嵌套流程已把它消费/窗口已变化）。
///
/// I1 询问完整性：**不得**把「没有回答」当成「放弃」——调用方必须标记
/// `response_stale` 并重新询问（`Some("pass")` 只能来自玩家的显式放弃）。
fn read_answer(g: &mut Game) -> Result<Option<String>, String> {
    Ok(g.answer.take())
}
