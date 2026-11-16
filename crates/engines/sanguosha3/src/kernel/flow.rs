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
    match apply_armor(armor, &spec, base) {
        DamageOutcome::Blocked => vec![],
        DamageOutcome::Dealt(n) => {
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
    // 不屈（锁定，`align/B0.json`）：濒死自动翻「创」判定；点数不重复 → 回复至 1，脱离濒死。
    if super::skills::active(g, seat, "buqu")
        && super::skills::enabled("buqu")
        && try_buqu(g, seat)
    {
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

/// 不屈：翻一张「创」判定。点数与该座位已有「创」不重复 → 视为「创」移出游戏并回复至 1。
///
/// 返回 `true` = 已脱离濒死（不再压濒死帧）。
fn try_buqu(g: &mut Game, seat: usize) -> bool {
    let Some(r) = super::judge::judge(g, seat, "不屈") else {
        return false;
    };
    let key = format!("buqu_rank.{seat}.{}", r.rank);
    if g.counter(&key) > 0 {
        // 重复：判定牌留在弃牌堆，仍濒死（继续求桃）。
        return false;
    }
    // 不重复：作为「创」移出游戏（不产生弃置事件）。
    g.cards.insert(r.card.clone(), super::Zone::Void);
    let mut ev = Event::card(
        EventKind::CardOutOfGame,
        Some(seat),
        r.card.clone(),
        Some("不屈·创".to_string()),
    );
    ev.by = Some(seat);
    let e = ev.snapshot(g);
    g.push_event(e);
    g.bump(&key);
    g.bump(&format!("buqu_total.{seat}"));
    if let Some(s) = g.seats.get_mut(seat) {
        s.hp = 1;
    }
    true
}

/// 回复体力（唯一出口）：钳到 `max_hp`，满血不回（但不报错）。
pub fn heal(g: &mut Game, target: usize, amount: i32, cause: &str) -> Vec<Event> {
    // 救援：吴势力角色回合内回复时，可改为令主公回复（这里按**同步重定向**实现）。
    let target = super::skills::heal_redirect(g, target).unwrap_or(target);
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
    g.cards.insert(id.to_string(), super::Zone::Discard);
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
        if g.hand(owner).is_empty() {
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

/// 获得（唯一出口）：牌进手牌；事件记「谁失去 → 谁获得」。
pub fn gain(g: &mut Game, to: usize, id: &str, by: usize, cause: &str) -> Option<Event> {
    let from_zone = g.cards.get(id)?.clone();
    let from = from_zone.seat();
    g.cards.insert(id.to_string(), super::Zone::Hand(to));
    if let Some(owner) = from {
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

/// 手牌摸至 n 张（审时「摸至 4 张」）：只摸不弃（超过 n 不处理）。
pub fn fill_hand(g: &mut Game, seat: usize, n: usize) -> Vec<CardId> {
    let cur = g.hand_size(seat);
    if cur >= n {
        return vec![];
    }
    draw(g, seat, n - cur)
}
