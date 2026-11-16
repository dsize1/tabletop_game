//! 体力结算唯一出口：伤害 / 失去体力 / 回血 / 体力上限变化 + 牌移动出口（摸/弃/获得）。
//!
//! 不变量（constitution §I2 / mechanics `damage-vs-hp-loss` / `damage-attributes`）：
//! 1. **伤害 ≠ 失去体力 ≠ 体力上限变化**：三类动作各有独立入口与事件，
//!    禁止复用 `damage` 伪造 `lose_hp`（反之亦然）；
//! 2. 伤害事件必须带**二维**（`DamageKind` 属性 × `SourceKind` 来源）——
//!    藤甲「普通杀无效」= `Slash + Normal`，火杀 = `Slash + Fire`（+1），雷杀 = `Slash + Thunder`（正常）；
//! 3. 数值链顺序：技能加成 → 藤甲 → 白银狮子**最后**（>1 改为 1）；
//! 4. 体力不得低于 0（濒死是 `resolve` 层职责，flow 只钳到 0）；
//! 5. **事件由本模块 emit**（P2）：任何体力/牌移动路径都自带事件，不靠测试手推 `g.events.push`。

use super::event::{Event, EventKind};
use super::{ArmorKind, CardId, DamageKind, DamageOutcome, DamageSpec, Game, SourceKind};

/// 防具伤害链：技能加成已并入 `amount`，这里只做防具（藤甲 → 白银狮子）。
///
/// - 藤甲：普通杀无效（含转化普杀）；南蛮/万箭无效；火 +1；雷正常；其他正常。
/// - 白银狮子：最后结算，>1 改为 1。
pub fn apply_armor(armor: ArmorKind, spec: &DamageSpec, mut amount: i32) -> DamageOutcome {
    match armor {
        ArmorKind::Tengjia => {
            if spec.is_normal_slash()
                || matches!(spec.source, SourceKind::Nanman | SourceKind::Wanjian)
            {
                return DamageOutcome::Blocked;
            }
            if spec.kind == DamageKind::Fire {
                amount += 1;
            }
        }
        ArmorKind::Baiyin => {}
        // 八卦阵（响应拦截）/仁王盾（黑色杀拦截）不在伤害链，拦截走 P5 的响应钩子。
        ArmorKind::Bagua | ArmorKind::Renwang | ArmorKind::None => {}
    }
    // 白银狮子最后（恒为 1）。
    if armor == ArmorKind::Baiyin && amount > 1 {
        amount = 1;
    }
    DamageOutcome::Dealt(amount)
}

/// 造成伤害（唯一出口）。`bonus` = 技能加成（链祸/暗箭/咆哮…）。
///
/// 返回本次产生的事件（含铁索传导）；事件同时已 emit 进 `g.events`。
pub fn damage(g: &mut Game, spec: DamageSpec, armor: ArmorKind, bonus: i32) -> Vec<Event> {
    // 造成伤害前的技能窗口（制蛮）：先压询问帧，伤害延后到 handler 里 `damage_apply`。
    if super::skills::ask_damage_before(g, spec, armor, bonus) {
        return vec![];
    }
    if super::skills::ask_damage_before_victim(g, spec, armor, bonus) {
        return vec![];
    }
    damage_apply(g, spec, armor, bonus)
}

/// 实际结算伤害（技能防伤拒绝后调用；**跳过 DamageBefore 询问**，防递归）。
pub fn damage_apply(g: &mut Game, spec: DamageSpec, armor: ArmorKind, bonus: i32) -> Vec<Event> {
    // 绝情：即将造成的伤害视为失去体力（整次数值一次；保留来源用于死亡归属）。
    if super::skills::damage_to_hp_loss(g, spec.from, spec.target) {
        let ev = lose_hp_by(g, spec.target, spec.amount + bonus, "伤害转失去体力", spec.from);
        return ev;
    }
    // 「造成伤害时」的数值加成（暗箭等）：与调用方传入的 bonus 相加。
    let bonus = bonus + super::skills::damage_bonus(g, &spec);
    // 链祸（锁定的属性加成）：火焰 + 自己是传导起点 → +1，且该 +1 随传导继续。
    let chain_extra = super::skills::lianhuo_bonus(g, &spec).max(0);
    let mut events = damage_one(g, spec, armor, bonus + chain_extra);
    // 铁索连环：属性伤害传导（传导后所有横置角色重置）。
    if spec.kind != DamageKind::Normal && !events.is_empty() {
        events.extend(propagate_chain(g, spec, armor, chain_extra));
    }
    events
}

/// 单体伤害（不传导）：被技能/防具免疫则返回空。
fn damage_one(g: &mut Game, spec: DamageSpec, armor: ArmorKind, bonus: i32) -> Vec<Event> {
    // 锁定技能防伤（怀橘：移除 1 枚「橘」防止该次伤害）——技能防伤先于防具数值链。
    if super::skills::prevent_damage(g, spec.target) {
        return vec![];
    }
    let base = spec.amount + bonus;
    // 帷幕（K6b）：自己回合内受到的伤害**全部**防止，并摸「防止点数 ×2」。
    if super::skills::try_full_prevent(g, spec.target, base).is_some() {
        return vec![];
    }
    match apply_armor(armor, &spec, base) {
        DamageOutcome::Blocked => vec![],
        DamageOutcome::Dealt(n) => {
            super::skills::b15::on_damage_after(g, spec.from, spec.source == SourceKind::Slash);
            // 护甲（mechanics `armor-points`）：先扣护甲，溢出伤害扣体力；
            // 伤害事件仍成立（amount = 原伤害值），因此「受到伤害后」技能照常触发。
            let armor_points = g.seats.get(spec.target).map(|s| s.armor).unwrap_or(0);
            let mut hp_loss = n;
            if hp_loss > 0 && armor_points > 0 {
                let absorbed = armor_points.min(hp_loss);
                g.add_armor(spec.target, -absorbed);
                hp_loss -= absorbed;
            }
            let seat = match g.seats.get_mut(spec.target) {
                Some(s) => s,
                None => return vec![],
            };
            seat.hp = (seat.hp - hp_loss).max(0);
            let mut ev = Event::simple(EventKind::DamageAfter, Some(spec.target), None);
            ev.amount = Some(n);
            ev.by = spec.from;
            if let Some(card) = g.damage_card.clone() {
                ev.card = Some(card.clone());
                if let Some(from) = spec.from {
                    g.bump(&format!("turn.{from}.damage_card.{card}"));
                    g.bump(&format!("phase.{from}.damage_card.{card}"));
                }
            }
            ev.turn = g.turn;
            ev.phase = g.phase;
            ev.damage = Some(DamageSpec {
                from: spec.from,
                target: spec.target,
                amount: n,
                kind: spec.kind,
                source: spec.source,
            });
            g.push_event(ev.clone());
            // 伤害记账（唯一写点）：本回合/本阶段/全局「造成伤害值」。
            // 急攻（本阶段手牌上限 = 造成伤害值）等只读这些计数器，不各自记账。
            if let Some(from) = spec.from {
                g.bump(&format!("turn.{from}.damage_dealt"));
                g.bump(&format!("phase.{from}.damage_dealt"));
                g.bump(&format!("total.damage_dealt.{from}"));
            }
            enter_dying(g, spec.target, spec.from);
            vec![ev]
        }
    }
}

/// 铁索传导：其他横置角色各受同属性同点数伤害，然后全部解除横置。
fn propagate_chain(g: &mut Game, spec: DamageSpec, _armor: ArmorKind, extra: i32) -> Vec<Event> {
    let chained: Vec<usize> = g
        .seats
        .iter()
        .filter(|s| s.alive && s.chained && s.id != spec.target)
        .map(|s| s.id)
        .collect();
    let victim_chained = g.seats.get(spec.target).map(|s| s.chained).unwrap_or(false);
    if chained.is_empty() && !victim_chained {
        return vec![];
    }
    let mut events = vec![];
    // 传导前先解除所有横置（避免递归传导）。
    let unchain: Vec<usize> = g
        .seats
        .iter_mut()
        .filter(|s| s.chained)
        .map(|s| {
            s.chained = false;
            s.id
        })
        .collect();
    for id in unchain {
        let ev = Event::simple(EventKind::ChainedLeft, Some(id), Some("属性伤害传导"));
        g.push_event(ev.clone());
        events.push(ev);
    }
    for t in chained {
        let armor = super::gear::armor_of(g, t);
        let sub = DamageSpec { target: t, ..spec };
        events.extend(damage_one(g, sub, armor, extra));
    }
    events
}

/// 进入连环状态（唯一入口）：已横置则无操作；产生 `ChainedEnter`（法恩等订阅）。
pub fn chain_enter(g: &mut Game, seat: usize, cause: &str) -> Vec<Event> {
    let Some(s) = g.seats.get_mut(seat) else {
        return vec![];
    };
    if s.chained {
        return vec![];
    }
    s.chained = true;
    let mut ev = Event::simple(EventKind::ChainedEnter, Some(seat), Some(cause));
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    vec![ev]
}

/// 失去（流失）体力：**不是伤害**，不产生 `DamageAfter`（mechanics `damage-vs-hp-loss`）。
pub fn lose_hp(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    seat.hp = (seat.hp - amount).max(0);
    let mut ev = Event::simple(EventKind::HpLost, Some(target), Some(cause));
    ev.amount = Some(amount);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    enter_dying(g, target, None);
    vec![ev]
}

/// **失去体力（带来源）**：绝情/恩怨等「伤害转失去体力」保留来源用于死亡归属。
pub fn lose_hp_by(
    g: &mut Game,
    target: usize,
    amount: i32,
    cause: &str,
    by: Option<usize>,
) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    seat.hp = (seat.hp - amount).max(0);
    let mut ev = Event::simple(EventKind::HpLost, Some(target), Some(cause));
    ev.amount = Some(amount);
    ev.by = by;
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    enter_dying(g, target, by);
    vec![ev]
}

/// **濒死入口（唯一）**：hp ≤ 0 且存活 → 压入通用 `FlowKind::Dying` 帧。
///
/// 同一座位同时只允许一个濒死帧（重复伤害/多段结算不得叠帧）。
pub fn enter_dying(g: &mut Game, seat: usize, by: Option<usize>) {
    let dying = g
        .seats
        .get(seat)
        .map(|s| s.alive && s.hp <= 0)
        .unwrap_or(false);
    if !dying {
        return;
    }
    let already = g.stack.iter().any(|f| {
        matches!(
            &f.inner,
            super::resolve::FrameInner::Flow(ff)
                if ff.kind == super::resolve::FlowKind::Dying
                    && ff.data.get("target").and_then(|v| v.as_u64()) == Some(seat as u64)
        )
    });
    if already {
        return;
    }
    // 濒死前钩子（`Hook::Dying`，唯一来源：不屈）：泛化接线，内核不出现技能名。
    //
    // 判定走 `quick_judge`（含鬼才改判 / 天妒窗口），结果由持有者 handler 在 `step="result"` 收口：
    // 未脱离濒死时由 handler 再压濒死帧（濒死求桃顺序不变）。
    let has_draw_pile =
        !g.deck.is_empty() || g.cards.values().any(|z| matches!(z, super::Zone::Discard));
    if has_draw_pile {
        if let Some(code) = super::skills::dying_hook_owner(g, seat) {
            super::quick_judge::start(
                g,
                seat,
                "濒死前钩子",
                Some((code, seat, serde_json::json!({ "by": by }))),
            );
            return;
        }
    }
    enter_dying_bypass_buqu(g, seat, by);
}

/// 濒死帧压入（**跳过不屈判定**）：不屈判定失败后由 `buqu` handler 调用；
/// 其余进入钩子（兴衰）仍然生效（`mechanics dying-aftermath`）。
pub fn enter_dying_bypass_buqu(g: &mut Game, seat: usize, by: Option<usize>) {
    let dying = g
        .seats
        .get(seat)
        .map(|s| s.alive && s.hp <= 0)
        .unwrap_or(false);
    if !dying {
        return;
    }
    let already = g.stack.iter().any(|f| {
        matches!(
            &f.inner,
            super::resolve::FrameInner::Flow(ff)
                if ff.kind == super::resolve::FlowKind::Dying
                    && ff.data.get("target").and_then(|v| v.as_u64()) == Some(seat as u64)
        )
    });
    if already {
        return;
    }
    // 兴衰（主公技+限定技）：先由其他魏势力逐个支援，再收口濒死。
    if super::skills::fire_dying_enter(g, seat) {
        return;
    }
    super::resolve::push_flow(
        g,
        super::resolve::FlowKind::Dying,
        seat,
        super::resolve::Stage::DyingAsk,
        serde_json::json!({ "target": seat, "by": by, "idx": 0 }),
    );
}

/// 回复体力（唯一出口）：钳到 `max_hp`，满血不回（但不报错）。
pub fn heal(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    // 救援：吴势力角色回合内回复时**询问**是否改为令主公回复（询问帧由技能压入，
    // 本次回复延后到技能结算里执行）。
    if super::skills::heal_ask(g, target, amount, cause) {
        return vec![];
    }
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    let healed = (seat.max_hp - seat.hp).min(amount).max(0);
    seat.hp = (seat.hp + healed).min(seat.max_hp);
    let mut ev = Event::simple(EventKind::Healed, Some(target), Some(cause));
    ev.amount = Some(healed);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    vec![ev]
}

/// **回复到指定体力值（单一出口）**：不屈/涅槃/若愚/伏枥/仁心等「回复至 X」。
///
/// 与 `heal` 的区别：不触发救援（救援只针对「回复」量），直接把 hp 设为 `to`（钳到上限）；
/// 只发 `Healed` 事件（amount = 实际回复量）。
pub fn restore_hp(g: &mut Game, target: usize, to: i32, cause: &str) -> Vec<Event> {
    let Some(seat) = g.seats.get_mut(target) else {
        return vec![];
    };
    let to = to.min(seat.max_hp);
    if to <= seat.hp {
        return vec![];
    }
    let amount = to - seat.hp;
    seat.hp = to;
    let mut ev = Event::simple(EventKind::Healed, Some(target), Some(cause));
    ev.amount = Some(amount);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    vec![ev]
}

/// **增加体力上限（单一出口）**：宗祚/若愚等；不自动回复体力（调用方按需 restore/heal）。
pub fn gain_max_hp(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let Some(seat) = g.seats.get_mut(target) else {
        return vec![];
    };
    seat.max_hp += amount;
    let mut ev = Event::simple(EventKind::MaxHpChanged, Some(target), Some(cause));
    ev.amount = Some(amount);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    vec![ev]
}

/// 体力上限变化（第三类动作，非伤害非失去体力）：减上限时 hp 一并钳到新上限。
pub fn lose_max_hp(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    let seat = match g.seats.get_mut(target) {
        Some(s) => s,
        None => return vec![],
    };
    seat.max_hp = (seat.max_hp - amount).max(0);
    seat.hp = seat.hp.min(seat.max_hp);
    let mut ev = Event::simple(EventKind::MaxHpChanged, Some(target), Some(cause));
    ev.amount = Some(amount);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    vec![ev]
}

/// 濒死判定（constitution I2）：hp ≤ 0 且仍存活 = 濒死（等待救援，**不是死亡**）。
pub fn is_dying(seat: &super::Seat) -> bool {
    seat.alive && seat.hp <= 0
}

/// 濒死救援（唯一入口，走 `heal` 语义）：桃/酒从 hp ≤ 0 回血；hp > 0 即脱离濒死。
pub fn rescue(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    heal(g, target, amount, cause)
}

/// **濒死结算后的延后结算**（mechanics `dying-aftermath`）：脱困（或死亡）后才派发。
///
/// 兴衰：支援者先救人，脱困后才逐个受到 1 点伤害（仍是伤害事件，来源可为无）。
/// 用户裁定（2026-11-15）：即使濒死者最终死亡，支援者**仍受伤**。
pub fn settle_after_dying(g: &mut Game) -> Vec<Event> {
    let entries = g.drain_after_dying();
    let mut events = vec![];
    for e in entries {
        if g.seats.get(e.seat).map(|s| s.alive).unwrap_or(false) {
            let spec = DamageSpec {
                from: None,
                target: e.seat,
                amount: e.amount,
                kind: DamageKind::Normal,
                source: SourceKind::Other,
            };
            let armor = super::gear::armor_of(g, e.seat);
            events.extend(damage(g, spec, armor, 0));
        }
    }
    events
}

/// 洗牌（伏诛结算后）：只重排牌堆顺序（确定性 Fisher-Yates；不改变区域归属）。
pub fn shuffle_deck(g: &mut Game) {
    for i in (1..g.deck.len()).rev() {
        let j = g.rand_below(i + 1);
        g.deck.swap(i, j);
    }
}

/// 摸牌：从牌堆顶抽 n 张进手牌（牌堆不足时摸到几张某）。
///
/// 弃牌堆洗回牌堆（牌堆耗尽时唯一入口；**不是弃置/获得**，不产生牌移动事件）。
pub fn reshuffle(g: &mut Game) {
    let mut ids: Vec<CardId> = g
        .cards
        .iter()
        .filter(|(_, z)| **z == super::Zone::Discard)
        .map(|(id, _)| id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    ids.sort();
    // Fisher-Yates（确定性）。
    for i in (1..ids.len()).rev() {
        let j = g.rand_below(i + 1);
        ids.swap(i, j);
    }
    for id in &ids {
        g.cards.insert(id.clone(), super::Zone::Draw);
    }
    g.deck = ids;
}

/// **摸牌方向单出口**：寸目（锁定）改从牌堆底摸；所有摸牌（摸牌阶段/技能摸牌）都走这里。
pub fn draw(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    // 牌堆耗尽：弃牌堆洗回牌堆（内核随机由 `Game.rng` 决定，可回放）。
    if g.deck.is_empty() {
        reshuffle(g);
    }
    let bottom = super::skills::draw_from_bottom(g, seat);
    let mut drawn = vec![];
    for _ in 0..n {
        let id = if bottom {
            if g.deck.is_empty() {
                None
            } else {
                Some(g.deck.remove(0))
            }
        } else {
            g.deck.pop()
        };
        let Some(id) = id else { break };
        g.cards.insert(id.clone(), super::Zone::Hand(seat));
        let mut ev = Event::card(EventKind::CardDrawn, Some(seat), id.clone(), Some("摸牌".to_string()));
        ev.by = Some(seat);
        ev.turn = g.turn;
        ev.phase = g.phase;
        g.push_event(ev);
        drawn.push(id);
        super::skills::on_hand_changed(g, seat, super::skills::HandChange::Drawn);
    }
    drawn
}

/// 从牌堆**底**摸牌（问卦：置顶后双方各从牌堆底摸一张；不改变寸目等其它规则）。
pub fn draw_bottom(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    if g.deck.is_empty() {
        reshuffle(g);
    }
    let mut drawn = vec![];
    for _ in 0..n {
        if g.deck.is_empty() {
            break;
        }
        let id = g.deck.remove(0);
        g.cards.insert(id.clone(), super::Zone::Hand(seat));
        let mut ev = Event::card(EventKind::CardDrawn, Some(seat), id.clone(), Some("摸牌（牌堆底）".to_string()));
        ev.by = Some(seat);
        ev.turn = g.turn;
        ev.phase = g.phase;
        g.push_event(ev);
        drawn.push(id);
        super::skills::on_hand_changed(g, seat, super::skills::HandChange::Drawn);
    }
    drawn
}

/// **摸牌阶段单出口**：基础 2 张 + 技能加成（庸肆 +X / 怀橘 +1）；整论可整段跳过。
pub fn draw_phase(g: &mut Game, seat: usize) -> Vec<CardId> {
    if super::skills::skip_draw(g, seat) {
        return vec![];
    }
    let n = 2 + super::skills::draw_bonus(g, seat);
    draw(g, seat, n)
}

/// 牌离开装备区（P5）：白银狮子 → 原持有者回 1 体力（离开装备区的唯一钩子）。
pub fn leave_equip(g: &mut Game, owner: usize, id: &str, cause: &str) -> Vec<Event> {
    // K13：失去装备区牌批次（旋风）。
    if !g
        .equip_lost_batch
        .iter()
        .any(|(o, c)| *o == owner && c == id)
    {
        g.equip_lost_batch.push((owner, id.to_string()));
    }
    // 枭姬：失去装备区的牌（唯一钩子；含弃置/替换/交出/转化使用）。
    super::skills::on_equip_lost(g, owner, id);
    let is_baiyin = g
        .card_defs
        .get(id)
        .and_then(|c| super::cards::def(&c.def))
        .map(|d| d.armor == ArmorKind::Baiyin)
        .unwrap_or(false);
    if is_baiyin {
        heal(g, owner, 1, cause)
    } else {
        vec![]
    }
}

/// 记录「失去牌」到本动作批次（K13；旋风/恩怨等）。`was_equip` = 离开装备区。
pub fn note_lost(g: &mut Game, owner: usize, id: &str, was_equip: bool) {
    // 牌级标记只对手牌有效：一旦离开手牌即清除（酒诗·落英牌）。
    g.clear_card_mark(id);
    g.lost_batch.entry(owner).or_default().push(id.to_string());
    if was_equip {
        g.equip_lost_batch.push((owner, id.to_string()));
    }
}

/// **移出游戏直到回合结束（K18；破军）**：进 `Zone::Void` + `CardOutOfGame`，
/// 记入 `void_return`；`turn::end_turn` 统一归还。
pub fn exile_until_turn_end(
    g: &mut Game,
    owner: usize,
    id: &str,
    cause: &str,
) -> Result<(), String> {
    let zone = g
        .cards
        .get(id)
        .cloned()
        .ok_or_else(|| format!("牌 {id} 不存在"))?;
    if !matches!(zone, super::Zone::Hand(s) | super::Zone::Equip(s, _) if s == owner) {
        return Err("只能移出该角色的手牌/装备区牌".into());
    }
    note_lost(g, owner, id, matches!(zone, super::Zone::Equip(..)));
    g.cards.insert(id.to_string(), super::Zone::Void);
    let mut ev = Event::card(
        EventKind::CardOutOfGame,
        Some(owner),
        id.to_string(),
        Some(cause.to_string()),
    );
    ev.by = Some(owner);
    ev.from = Some(owner);
    let ev = ev.snapshot(g);
    g.push_event(ev);
    g.void_return.push((id.to_string(), owner, g.turn_seq));
    Ok(())
}

/// **回合末归还**（K18）：归还所有 `due <= 当前 turn_seq` 的暂离牌（唯一出口）。
pub fn return_due_void(g: &mut Game) -> Vec<Event> {
    let due: Vec<(super::CardId, usize, u64)> = g
        .void_return
        .iter()
        .filter(|(_, _, t)| *t <= g.turn_seq)
        .cloned()
        .collect();
    g.void_return.retain(|(_, _, t)| *t > g.turn_seq);
    let mut events = vec![];
    for (id, owner, _) in due {
        g.cards.insert(id.clone(), super::Zone::Hand(owner));
        let mut ev = Event::card(
            EventKind::CardIntoGame,
            Some(owner),
            id.clone(),
            Some("回合末归还".to_string()),
        );
        ev.by = Some(owner);
        ev.from = Some(owner);
        let ev = ev.snapshot(g);
        g.push_event(ev.clone());
        events.push(ev);
    }
    events
}

/// **进入弃牌堆事实（唯一出口，K14）**：牌已在 `Zone::Discard` 后调用。
///
/// 与 `CardDiscarded`（弃置）区分：
/// - 本事件覆盖任意方式（弃置/使用落盘/判定/死亡清理/转化牌结算）；
/// - 判定区移出**不发**本事件（只发 `JudgeZoneLeft`）；
/// - 死亡弃牌由 `turn::death_cleanup` 调用，cause = `死亡`。
///
/// 同时记 `turn.<turn>.red_to_discard`（再起：本回合红色实体牌进弃牌堆张数；花色按物理牌）。
pub fn record_to_discard(
    g: &mut Game,
    id: &str,
    cause: &str,
    by: Option<usize>,
) -> Event {
    let owner = g.cards.get(id).and_then(|z| z.seat());
    if let Some(c) = g.card_defs.get(id) {
        if c.suit.is_red() {
            g.bump(&format!("turn.{}.red_to_discard", g.turn));
        }
    }
    let mut ev = Event::card(
        EventKind::CardToDiscard,
        owner,
        id.to_string(),
        Some(cause.to_string()),
    );
    ev.by = by;
    ev.from = owner;
    let ev = ev.snapshot(g);
    g.push_event(ev.clone());
    ev
}

/// 弃置（唯一出口，`lose-vs-discard`）：只能从手牌/装备区弃置（判定区离开**不算弃置**）。
///
/// 记账：进弃牌堆 + 本阶段弃牌计数（`phase.discarded.<Phase>`，溃诛 X 的唯一来源）+ 事件（供订阅路由）。
pub fn discard(g: &mut Game, by: usize, id: &str, cause: &str) -> Option<Event> {
    let zone = g.cards.get(id)?.clone();
    let owner = zone.seat()?;
    if !matches!(zone, super::Zone::Hand(_) | super::Zone::Equip(..)) {
        return None;
    }
    // 奇才：防止**其他角色**弃置你的防具/宝物（「获得」不受影响）。
    if super::skills::prevent_discard(g, by, owner, id) {
        return None;
    }
    let was_equip = matches!(zone, super::Zone::Equip(..));
    g.cards.insert(id.to_string(), super::Zone::Discard);
    note_lost(g, owner, id, was_equip);
    g.discard_batch.entry(by).or_default().push(id.to_string());
    record_to_discard(g, id, cause, Some(by));
    let key = format!("phase.discarded.{:?}", g.phase);
    g.bump(&key);
    // 失去牌事件（M105 教训：主体=失去者，施动者=弃置者）。
    let mut lost = Event::card(EventKind::CardLost, Some(owner), id.to_string(), Some(cause.to_string()));
    lost.by = Some(by);
    lost.from = Some(owner);
    lost.turn = g.turn;
    lost.phase = g.phase;
    g.push_event(lost);
    if matches!(zone, super::Zone::Equip(..)) {
        leave_equip(g, owner, id, cause);
    }
    let mut ev = Event::card(EventKind::CardDiscarded, Some(owner), id.to_string(), Some(cause.to_string()));
    ev.by = Some(by);
    ev.from = Some(owner);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    if matches!(zone, super::Zone::Hand(_)) {
        super::skills::on_hand_changed(g, owner, super::skills::HandChange::Discarded);
        // 弃牌阶段按「本次弃置张数」在 `turn::step_discard` 统一结算（避免按张重复触发）。
        if g.phase != super::Phase::Discard && g.hand(owner).is_empty() {
            super::skills::on_hand_empty(g, owner, 1);
        }
    }
    // 本回合置入弃牌堆的牌花色（博图：四花色齐 → 额外回合）。
    if let Some(suit) = g.card_defs.get(id).map(|c| format!("{:?}", c.suit)) {
        let key = format!("turn.{}.discard_suits", g.turn);
        let mut suits: Vec<String> = g
            .mark(&key)
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
            .unwrap_or_default();
        if !suits.contains(&suit) {
            suits.push(suit);
            g.set_mark(&key, suits.join(","));
        }
    }
    Some(ev)
}

/// **手牌交换（唯一出口）**：双方同时失去并重新获得手牌（缔盟）。
///
/// 事件成对：先快照双方手牌，再各自 `gain` 到对方（`gain` 内已 emit `CardLost`+`CardGained`）。
pub fn swap_hands(g: &mut Game, a: usize, b: usize) {
    if a == b {
        return;
    }
    let ha = g.hand(a);
    let hb = g.hand(b);
    for id in &hb {
        gain(g, a, id, b, "缔盟");
    }
    for id in &ha {
        gain(g, b, id, a, "缔盟");
    }
}

/// 获得（唯一出口）：牌进手牌；事件记「谁失去 → 谁获得」。
pub fn gain(g: &mut Game, to: usize, id: &str, by: usize, cause: &str) -> Option<Event> {
    let from_zone = g.cards.get(id)?.clone();
    let from = from_zone.seat();
    g.cards.insert(id.to_string(), super::Zone::Hand(to));
    if let Some(owner) = from {
        note_lost(g, owner, id, matches!(from_zone, super::Zone::Equip(..)));
        if owner != to {
            g.gain_batch.push((to, owner, id.to_string()));
        }
        let mut lost = Event::card(EventKind::CardLost, Some(owner), id.to_string(), Some(cause.to_string()));
        lost.by = Some(by);
        lost.from = Some(owner);
        lost.turn = g.turn;
        lost.phase = g.phase;
        g.push_event(lost);
        if matches!(from_zone, super::Zone::Equip(..)) {
            leave_equip(g, owner, id, cause);
        }
    }
    let mut ev = Event::card(EventKind::CardGained, Some(to), id.to_string(), Some(cause.to_string()));
    ev.by = Some(by);
    ev.from = from;
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev.clone());
    // 本回合「获得的牌」张数（腹鳞：弃牌阶段不计入手牌上限）。
    // 摸牌不经过这里（`CardDrawn`），因此天然排除摸牌阶段的摸牌。
    g.bump(&format!("turn.{to}.gained_cards"));
    super::skills::on_hand_changed(g, to, super::skills::HandChange::Gained);
    Some(ev)
}

/// 置于牌堆顶（恃才）：进 Draw 区并压到牌堆顶。
pub fn to_deck_top(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), super::Zone::Draw);
    g.deck.push(id.to_string());
}

/// 置于牌堆底（恂恂）：进 Draw 区并压到牌堆底。
pub fn to_deck_bottom(g: &mut Game, id: &str) {
    g.cards.insert(id.to_string(), super::Zone::Draw);
    g.deck.insert(0, id.to_string());
}

/// 手牌摸至 n 张（审时「摸至 4 张」）：只摸不弃（超过 n 不处理）。
pub fn fill_hand(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    let cur = g.hand_size(seat);
    if cur >= n {
        return vec![];
    }
    draw(g, seat, n - cur)
}
