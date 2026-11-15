//! 回合驱动（P1）+ 死亡 + 终局（P6 生命周期）。
//!
//! 不变量（constitution I2 / ADR-0042 缺陷 6「生命周期」）：
//! 1. **死亡是显式结算**：`Seat.alive` 只由 `settle_death` 置 false；弃牌 + 身份公开 + 胜负检查在同一结算内；
//! 2. **回合只由本模块驱动**：`start` → `Turn` 流程帧 → 阶段机（prepare→judge→draw→play→discard→end）；
//!    跳阶段/额外回合/翻面都从 `Game.skipped` / `Game.extra_turns` / `Seat.flipped` 读取，不在别处硬编码；
//! 3. 终局后 `Game.winner` 置位，回合帧不再推进。

use serde_json::{json, Value};

use super::cards::Effect;
use super::event::{Event, EventKind};
use super::judge;
use super::pending::PendingBuilder;
use super::resolve::{push_flow, take_answer, FlowKind, Stage, Step};
use super::{cards, flow, Game, Identity, Phase, Zone};

/// 阶段顺序（唯一事实源；`turn::PHASES` 是阶段机的驱动表）。
pub const PHASES: [Phase; 6] = [
    Phase::Prepare,
    Phase::Judge,
    Phase::Draw,
    Phase::Play,
    Phase::Discard,
    Phase::End,
];

/// 空对局 + `players` 个存活座位（体力 4；武将/身份由 `begin_game*` 落地）。
pub fn new_game(players: usize) -> Game {
    let mut g = Game::default();
    for i in 0..players {
        g.seats.push(super::Seat {
            id: i,
            hp: 4,
            max_hp: 4,
            alive: true,
            ..Default::default()
        });
    }
    g
}

/// 开局（无武将）：发身份（座位 0 = 主公）+ 发初始手牌。
pub fn begin_game(g: &mut Game, identities: &[Identity], hand_cards: usize) {
    begin_game_with_generals(g, identities, &[], hand_cards)
}

/// 开局（P6）：发身份 → 选将落地（势力/体力/性别/主公 +1）→ 发初始手牌 → 游戏开始钩子。
///
/// 主公**只 +1 体力上限**，初始手牌与其他人一致（4 张；`v3-parity-baseline.md` §5.1）。
pub fn begin_game_with_generals(
    g: &mut Game,
    identities: &[Identity],
    generals: &[&str],
    hand_cards: usize,
) {
    for (i, seat) in g.seats.iter_mut().enumerate() {
        seat.alive = true;
        seat.identity = Some(if i == 0 {
            Identity::Lord
        } else {
            identities.get(i - 1).copied().unwrap_or(Identity::Rebel)
        });
        if seat.max_hp <= 0 {
            seat.max_hp = 4;
        }
    }
    for (i, code) in generals.iter().enumerate() {
        if code.is_empty() {
            continue;
        }
        let _ = super::generals::apply_to_seat(g, i, code);
    }
    for seat in g.seats.iter_mut() {
        if seat.hp <= 0 {
            seat.hp = seat.max_hp;
        }
    }
    for seat in 0..g.seats.len() {
        flow::draw(g, seat, hand_cards);
    }
    g.started = true;
    g.winner = None;
    // 游戏开始钩子（怀橘 3 橘等）。**在发牌之后**，且不产生 Setup 事件（明任教训）。
    super::skills::fire_game_start(g);
}

/// 开局后进入第一个回合（主公先手），并推进到第一个「需要输入」的点。
pub fn start(g: &mut Game) -> Result<(), String> {
    if g.seats.is_empty() {
        return Err("没有座位".into());
    }
    if g.stack.iter().any(is_turn_frame) {
        return Err("对局已开始（回合帧已在栈上）".into());
    }
    let first = g.current_lord().or_else(|| g.alive_seats().first().copied()).unwrap_or(0);
    g.pending = None;
    g.answer = None;
    g.answer_skill = None;
    g.winner = None;
    begin_turn(g, first);
    push_flow(
        g,
        FlowKind::Turn,
        first,
        Stage::PhaseStep,
        json!({ "seat": first, "pi": 0, "entered": false }),
    );
    super::resolve::advance(g)
}

/// 中断/重开（P6）：清空结算栈与待办，保留座位/牌区事实（可重放）。
pub fn abort(g: &mut Game) {
    g.stack.clear();
    g.pending = None;
    g.deferred.clear();
    g.events.clear();
    g.answer = None;
    g.answer_skill = None;
    g.marks.clear();
    g.started = false;
}

fn is_turn_frame(f: &super::resolve::Frame) -> bool {
    matches!(&f.inner, super::resolve::FrameInner::Flow(ff) if ff.kind == FlowKind::Turn)
}

/// 跳过一个阶段（技能/延时锦囊唯一写入口）。
pub fn skip_phase(g: &mut Game, phase: Phase) {
    if !g.skipped.contains(&phase) {
        g.skipped.push(phase);
    }
}

/// 插入一个额外回合（技能唯一写入口；下次回合轮到时优先消费）。
pub fn extra_turn(g: &mut Game, seat: usize) {
    g.extra_turns.push(seat);
}

/// 回合开始：置当前回合、清回合/阶段计数、发 `TurnStart` 事件。
pub fn begin_turn(g: &mut Game, seat: usize) {
    g.turn_seq += 1;
    g.turn = seat;
    g.phase = Phase::Prepare;
    g.sha_bonus = 0;
    g.skipped.clear();
    g.clear_counters("turn.");
    g.clear_counters("phase.");
    Event::simple(EventKind::TurnStart, Some(seat), None).emit(g);
}

/// 回合结束：发 `TurnEnd` → 计算下一个回合角色（额外回合 / 翻面跳过 / 轮数 +1）。
/// 返回 `None` = 对局结束（无下一个回合）。
pub fn end_turn(g: &mut Game) -> Option<usize> {
    let seat = g.turn;
    // 回合级手牌冻结到期（往烈“本阶段不能再使用牌”按回合结束口径）。
    g.unfreeze_turn();
    Event::simple(EventKind::TurnEnd, Some(seat), None).emit(g);
    g.sha_bonus = 0;
    if g.winner.is_some() {
        return None;
    }
    // 游标：跳过翻面/死亡座位时从**被跳过的座位**继续找下一个，而不是从头再找（否则死循环）。
    let mut cursor = seat;
    loop {
        let normal = g.extra_turns.is_empty();
        let next = if normal {
            next_living(g, cursor)
        } else {
            Some(g.extra_turns.remove(0))
        };
        let Some(next) = next else { break };
        if !g.seats.get(next).map(|s| s.alive).unwrap_or(false) {
            continue;
        }
        if normal && next <= cursor {
            g.round += 1;
        }
        cursor = next;
        if g.seats[next].flipped {
            g.seats[next].flipped = false;
            Event::simple(EventKind::TurnStart, Some(next), Some("翻面跳过回合")).emit(g);
            Event::simple(EventKind::TurnEnd, Some(next), Some("翻面跳过回合")).emit(g);
            continue;
        }
        return Some(next);
    }
    None
}

/// 顺时针下一个存活座位（含绕圈）。
pub fn next_living(g: &Game, from: usize) -> Option<usize> {
    let n = g.seats.len();
    (1..=n)
        .map(|k| (from + k) % n)
        .find(|&s| g.seats.get(s).map(|s| s.alive).unwrap_or(false))
}

/// 回合流程帧 handler（`FlowKind::Turn`）。
pub fn step_turn(g: &mut Game, data: &Value) -> Result<Step, String> {
    let Some(seat) = data.get("seat").and_then(|v| v.as_u64()).map(|v| v as usize) else {
        return Ok(Step::Done);
    };
    let pi = data.get("pi").and_then(|v| v.as_u64()).unwrap_or(PHASES.len() as u64) as usize;
    let entered = data.get("entered").and_then(|v| v.as_bool()).unwrap_or(false);
    // 阶段自动动作是否已执行：`entered=false` 只发 PhaseStart，让阶段开始类技能**先询问**，
    // 询问结算完再执行自动动作（否则整论来不及跳过摸牌、遗礼来不及在出牌阶段开始前结算）。
    // 未显式声明 `auto` 时：已进入阶段即视为自动动作已执行（防旧帧数据丢字段导致重复结算）。
    let auto_done = data.get("auto").and_then(|v| v.as_bool()).unwrap_or(entered);
    if g.winner.is_some() {
        return Ok(Step::Done);
    }
    if pi >= PHASES.len() {
        return match end_turn(g) {
            None => Ok(Step::Done),
            Some(next) => {
                begin_turn(g, next);
                push_flow(
                    g,
                    FlowKind::Turn,
                    next,
                    Stage::PhaseStep,
                    json!({ "seat": next, "pi": 0, "entered": false }),
                );
                Ok(Step::Done)
            }
        };
    }
    // 当前回合角色在自己的回合内阵亡（如闪电/反伤/濒死失败）→ 立即结束该回合。
    if !g.seats.get(seat).map(|s| s.alive).unwrap_or(false) {
        if g.winner.is_some() {
            return Ok(Step::Done);
        }
        return match end_turn(g) {
            None => Ok(Step::Done),
            Some(next) => {
                begin_turn(g, next);
                push_flow(
                    g,
                    FlowKind::Turn,
                    next,
                    Stage::PhaseStep,
                    json!({ "seat": next, "pi": 0, "entered": false }),
                );
                Ok(Step::Done)
            }
        };
    }
    let phase = PHASES[pi];
    if !entered {
        // 跳阶段（乐不思蜀/兵粮寸断/技能）：不产生 PhaseStart/PhaseEnd（阶段未发生）。
        if g.skipped.contains(&phase) {
            g.skipped.retain(|p| *p != phase);
            return Ok(Step::Next(
                Stage::PhaseStep,
                json!({ "seat": seat, "pi": pi + 1, "entered": false }),
            ));
        }
        g.phase = phase;
        // 阶段级手牌冻结到期（mechanics `hand-freeze`）：阶段切换自动解冻。
        g.unfreeze_phase();
        g.clear_counters("phase.");
        let mut start_ev = Event::simple(EventKind::PhaseStart, Some(seat), None);
        start_ev.turn = g.turn;
        start_ev.phase = phase;
        g.push_event(start_ev);
        // 有阶段开始类技能 → 先询问（事件路由在 advance 内、本帧下一步之前完成），
        // 自动动作留到 `auto=false → true` 的下一次进入再执行；无技能则保持单步推进。
        if super::skills::has_phase_start_sub(g, seat, phase) {
            return Ok(Step::Next(
                Stage::PhaseStep,
                json!({ "seat": seat, "pi": pi, "entered": true, "auto": false }),
            ));
        }
        // 无阶段开始技能：直接执行自动动作（原行为，不额外增加推进步数）。
        match phase {
            Phase::Prepare => return finish_phase(g, seat, pi),
            Phase::Judge => {
                judge_phase(g, seat);
                return finish_phase(g, seat, pi);
            }
            Phase::Draw => {
                g.set_counter(&format!("phase.drawn.{seat}"), 0);
                let drawn = flow::draw_phase(g, seat);
                g.set_counter(&format!("phase.drawn.{seat}"), drawn.len() as i32);
                return finish_phase(g, seat, pi);
            }
            Phase::Play => {
                g.set_counter(&format!("phase.sha_used.{seat}"), 0);
                g.set_counter(&format!("phase.jiu_used.{seat}"), 0);
            }
            Phase::Discard | Phase::End => {}
        }
        let stage = match phase {
            Phase::Play => Stage::PlayAct,
            _ => Stage::DiscardPick,
        };
        return Ok(Step::Next(
            stage,
            json!({ "seat": seat, "pi": pi, "entered": true, "auto": true }),
        ));
    }
    if !auto_done {
        // 阶段自动动作（询问之后执行）。
        match phase {
            Phase::Prepare => return finish_phase(g, seat, pi),
            Phase::Judge => {
                judge_phase(g, seat);
                return finish_phase(g, seat, pi);
            }
            Phase::Draw => {
                g.set_counter(&format!("phase.drawn.{seat}"), 0);
                let drawn = flow::draw_phase(g, seat);
                g.set_counter(&format!("phase.drawn.{seat}"), drawn.len() as i32);
                return finish_phase(g, seat, pi);
            }
            Phase::Play => {
                g.set_counter(&format!("phase.sha_used.{seat}"), 0);
                g.set_counter(&format!("phase.jiu_used.{seat}"), 0);
                return Ok(Step::Next(
                    Stage::PlayAct,
                    json!({ "seat": seat, "pi": pi, "entered": true, "auto": true }),
                ));
            }
            Phase::Discard => {
                return Ok(Step::Next(
                    Stage::DiscardPick,
                    json!({ "seat": seat, "pi": pi, "entered": true, "auto": true }),
                ));
            }
            Phase::End => return finish_phase(g, seat, pi),
        }
    }
    match phase {
        Phase::Play => step_play(g, seat, pi),
        Phase::Discard => step_discard(g, seat, pi),
        _ => finish_phase(g, seat, pi),
    }
}

fn finish_phase(g: &mut Game, seat: usize, pi: usize) -> Result<Step, String> {
    let phase = PHASES[pi];
    let mut ev = Event::simple(EventKind::PhaseEnd, Some(seat), None);
    ev.turn = g.turn;
    ev.phase = phase;
    g.push_event(ev);
    Ok(Step::Next(
        Stage::PhaseStep,
        json!({ "seat": seat, "pi": pi + 1, "entered": false }),
    ))
}

/// 出牌阶段：`pending` 选牌/选技能 → 压 `Card` 流程帧 → 回来继续问，直到 `end`。
fn step_play(g: &mut Game, seat: usize, pi: usize) -> Result<Step, String> {
    if let Some(ans) = take_answer(g, "flow:play") {
        // Web 端「结束出牌」按钮提交 `end_play`（兼容旧 id `end`）。
        if ans == "end_play" || ans == "end" {
            return finish_phase(g, seat, pi);
        }
        if let Some(code) = ans.strip_prefix("skill:") {
            super::skills::start_active(g, seat, code)?;
            return Ok(Step::Next(
                Stage::PlayAct,
                json!({ "seat": seat, "pi": pi, "entered": true }),
            ));
        }
        if ans == "zhangba" {
            g.set_mark("play.mode", "zhangba1");
            g.marks.remove(&format!("play.zb.{seat}"));
        } else if ans == "pass" && g.mark("play.mode") == Some("zhangba1") {
            // 丈八选牌阶段取消：清掉中间态，回到普通出牌询问。
            g.marks.remove("play.mode");
            g.marks.remove(&format!("play.zb.{seat}"));
        } else if let Some(id) = ans.strip_prefix("zbc:") {
            let key = format!("play.zb.{seat}");
            let prev = g.mark(&key).map(|s| s.to_string());
            match prev {
                None => g.set_mark(&key, id),
                Some(first) if first == id => return Err("丈八蛇矛：不能重复选择同一张牌".into()),
                Some(first) => {
                    g.marks.remove(&key);
                    g.marks.remove("play.mode");
                    super::card_play::begin_use_zhangba(g, seat, &first, id)?;
                    return Ok(Step::Next(
                        Stage::PlayAct,
                        json!({ "seat": seat, "pi": pi, "entered": true }),
                    ));
                }
            }
        } else if let Some(id) = ans.strip_prefix("recast:") {
            super::card_play::recast(g, seat, id)?;
            return Ok(Step::Next(
                Stage::PlayAct,
                json!({ "seat": seat, "pi": pi, "entered": true }),
            ));
        } else if let Some(rest) = ans.strip_prefix("card:") {
            let (id, fire) = match rest.split_once(':') {
                Some((a, b)) => (a, b == "fire"),
                None => (rest, false),
            };
            super::card_play::begin_use(g, seat, id, fire)?;
            return Ok(Step::Next(
                Stage::PlayAct,
                json!({ "seat": seat, "pi": pi, "entered": true }),
            ));
        }
    }
    if g.mark("play.mode") == Some("zhangba1") {
        let picked = format!("play.zb.{seat}");
        let first = g.mark(&picked).map(|s| s.to_string());
        let mut b = PendingBuilder::new(seat, "zhangba_pick").skill("flow:play");
        for id in g.hand(seat) {
            if Some(&id) == first.as_ref() {
                continue;
            }
            b = b.option(format!("zbc:{id}"), format!("选择【{}】", card_name(g, &id)));
        }
        return Ok(Step::NeedAt(Stage::CardPick, b.escape("取消").build()));
    }
    match build_play_pending(g, seat) {
        Some(p) => Ok(Step::NeedAt(Stage::PlayAct, p)),
        None => finish_phase(g, seat, pi),
    }
}

fn card_name(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| "?".into())
}

fn build_play_pending(g: &Game, seat: usize) -> Option<super::pending::Pending> {
    let mut b = PendingBuilder::new(seat, "play_act").skill("flow:play");
    let mut any = false;
    for id in super::card_play::usable_hand(g, seat) {
        let Some(c) = g.card_defs.get(&id) else { continue };
        let Some(def) = cards::def(&c.def) else { continue };
        b = b.option(format!("card:{id}"), format!("使用【{}】", def.name));
        any = true;
        // 朱雀羽扇：普通【杀】可当【火杀】使用（转化牌按生效定义定属性）。
        if def.tags.sha
            && def.damage_kind == super::DamageKind::Normal
            && super::gear::weapon_of(g, seat) == super::WeaponKind::Zhuque
        {
            b = b.option(format!("card:{id}:fire"), "使用【火杀】（朱雀羽扇）");
        }
    }
    for code in super::skills::active_actions(g, seat) {
        let name = super::skills::def(code).map(|d| d.name).unwrap_or(code);
        b = b.option(format!("skill:{code}"), format!("发动【{name}】"));
        any = true;
    }
    // 铁索连环重铸（弃此牌摸一张；不是「使用」，故单独入口）。
    for id in g.hand(seat) {
        if g.card_defs
            .get(&id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.tags.lianhuan)
            .unwrap_or(false)
        {
            b = b.option(format!("recast:{id}"), format!("重铸【{}】", card_name(g, &id)));
        }
    }
    // 丈八蛇矛：两张手牌当【杀】（装备特效 = 特殊技能注入）。
    if super::gear::weapon_of(g, seat) == super::WeaponKind::Zhangba && g.hand_size(seat) >= 2 {
        b = b.option("zhangba", "丈八蛇矛：两张手牌当【杀】");
        any = true;
    }
    let _ = any;
    // 询问完整性（I1）：只有出口也必须询问（`must_ask`）。
    Some(b.escape_with("end_play", "结束出牌阶段").build())
}

/// 弃牌阶段：弃到手牌上限（`hand_limit`），强制选择（不接受出口）。
fn step_discard(g: &mut Game, seat: usize, pi: usize) -> Result<Step, String> {
    if let Some(ans) = take_answer(g, "flow:discard") {
        if let Some(id) = ans.strip_prefix("card:") {
            if g.hand(seat).iter().any(|c| c == id) {
                flow::discard(g, seat, id, "弃牌阶段");
            } else {
                return Err(format!("弃牌阶段：{id} 不在手牌"));
            }
        }
    }
    let limit = g.hand_limit(seat);
    if g.hand_size(seat) <= limit {
        return finish_phase(g, seat, pi);
    }
    let mut b = PendingBuilder::new(seat, "discard_phase").skill("flow:discard");
    for id in g.hand(seat) {
        b = b.option(format!("card:{id}"), format!("弃置【{}】", card_name(g, &id)));
    }
    Ok(Step::NeedAt(Stage::DiscardPick, b.escape_mandatory("弃牌").build()))
}

/// 判定阶段：判定区 LIFO（后置先判），逐个结算延时锦囊。
pub fn judge_phase(g: &mut Game, seat: usize) {
    while let Some(id) = g
        .judge_stacks
        .get(&seat)
        .and_then(|s| s.last().cloned())
    {
        if let Some(stack) = g.judge_stacks.get_mut(&seat) {
            stack.pop();
        }
        g.cards.insert(id.clone(), Zone::Processing);
        let mut left = Event::card(EventKind::JudgeZoneLeft, Some(seat), id.clone(), Some("判定阶段".to_string()));
        left.by = Some(seat);
        left.turn = g.turn;
        left.phase = g.phase;
        g.push_event(left);
        let effect = g
            .card_defs
            .get(&id)
            .and_then(|c| cards::def(&c.def))
            .map(|d| d.effect);
        match effect {
            Some(Effect::Indulgence) => {
                // 判定效果反转（mechanics `judge-reversal`）：只改分支，不改牌面花色。
                let reversed = g.judge_reversed(seat, "lebu_sishu");
                let heart = judge::judge(g, seat, "乐不思蜀")
                    .map(|r| r.is_heart())
                    .unwrap_or(false);
                let skipped = if reversed { heart } else { !heart };
                super::card_play::to_discard(g, &id);
                if skipped {
                    skip_phase(g, Phase::Play);
                }
            }
            Some(Effect::SupplyShortage) => {
                let skipped = !judge::judge(g, seat, "兵粮寸断")
                    .map(|r| r.is_club())
                    .unwrap_or(false);
                super::card_play::to_discard(g, &id);
                if skipped {
                    skip_phase(g, Phase::Draw);
                }
            }
            Some(Effect::Lightning) => {
                let hit = judge::judge(g, seat, "闪电")
                    .map(|r| r.is_spade() && r.rank_in(2, 9))
                    .unwrap_or(false);
                if hit {
                    super::card_play::to_discard(g, &id);
                    let spec = super::DamageSpec {
                        from: None,
                        target: seat,
                        amount: 3,
                        kind: super::DamageKind::Thunder,
                        source: super::SourceKind::Other,
                    };
                    let _ = flow::damage(g, spec, super::gear::armor_of(g, seat), 0);
                } else if let Some(next) = next_living(g, seat) {
                    // 未生效：移到下家判定区（同名去重；下家已有闪电则弃置）。
                    let code = g.card_defs.get(&id).map(|c| c.def.clone()).unwrap_or_default();
                    if !super::card_play::place_delayed(g, next, &id, &code) {
                        super::card_play::to_discard(g, &id);
                    }
                } else {
                    super::card_play::to_discard(g, &id);
                }
            }
            _ => super::card_play::to_discard(g, &id),
        }
        if g.winner.is_some() {
            return;
        }
    }
}

/// 死亡确认结算（无击杀者上下文；保留给「非伤害死亡」与测试）。
pub fn die(g: &mut Game, seat: usize) -> Vec<Event> {
    settle_death(g, seat, None)
}

/// **通用死亡结算（唯一出口）**：弃所有牌 → 奖惩 → 胜负（constitution I2 / `death-penalty`）。
///
/// 事件一律由本函数 emit 进 `g.events`（P2：不是测试手推）。
pub fn settle_death(g: &mut Game, seat: usize, killer: Option<usize>) -> Vec<Event> {
    let Some(s) = g.seats.get_mut(seat) else {
        return vec![];
    };
    if !s.alive {
        return vec![];
    }
    s.alive = false;
    // 行殇（`align/B0.json` QQ1=B）：其他角色死亡时、**清理前**询问获得其手牌+装备（不含判定区）。
    // 有持有者 → 延后清理（死亡事件/奖惩/胜负在 `finish_death` 里收口）。
    if let Some(holder) = (0..g.seats.len()).find(|&i| {
        i != seat
            && g.seats[i].alive
            && super::skills::active(g, i, "xingshang")
            && super::skills::enabled("xingshang")
    }) {
        let has_cards = g.cards.iter().any(|(_, z)| {
            z.seat() == Some(seat) && matches!(z, Zone::Hand(_) | Zone::Equip(..))
        });
        if has_cards {
            super::resolve::push_ask(
                g,
                "xingshang",
                super::resolve::Stage::Choose,
                holder,
                serde_json::json!({ "step": "ask", "victim": seat, "killer": killer }),
            );
            return vec![];
        }
    }
    finish_death(g, seat, killer)
}

/// 死亡清理收口（弃所有牌 → Death 事件 → 奖惩 → 胜负）。
///
/// 由 `settle_death`（无行殇持有者时）或 `xingshang` 技能结束时调用（唯一收口）。
pub fn finish_death(g: &mut Game, seat: usize, killer: Option<usize>) -> Vec<Event> {
    let victim_identity = g.seats.get(seat).and_then(|s| s.identity);
    let mut events = vec![];
    // 弃所有牌：手牌/装备/判定区（判定区栈一并清空）。
    let owned: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| {
            z.seat() == Some(seat) && matches!(z, Zone::Hand(_) | Zone::Equip(..) | Zone::Judge(_))
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in owned {
        g.cards.insert(id.clone(), Zone::Discard);
        let mut ev = Event::card(
            EventKind::CardDiscarded,
            Some(seat),
            id.clone(),
            Some("死亡弃牌".to_string()),
        );
        ev.by = Some(seat);
        ev.from = Some(seat);
        events.push(ev.snapshot(g).emit(g));
    }
    g.judge_stacks.remove(&seat);
    let mut death = Event::simple(EventKind::Death, Some(seat), killer.map(|_| "伤害"));
    death.by = killer;
    events.push(death.snapshot(g).emit(g));
    // 奖惩：只在击杀者存活且不是自己时结算。
    if let Some(k) = killer.filter(|&k| k != seat && g.seats.get(k).map(|s| s.alive).unwrap_or(false)) {
        match victim_identity {
            // 杀死反贼：摸 3 张。
            Some(Identity::Rebel) => {
                flow::draw(g, k, 3);
            }
            // 主公杀死忠臣：弃置所有手牌与装备区牌。
            Some(Identity::Loyalist) if g.seats[k].identity == Some(Identity::Lord) => {
                let cards: Vec<String> = g
                    .cards
                    .iter()
                    .filter(|(_, z)| {
                        z.seat() == Some(k) && matches!(z, Zone::Hand(_) | Zone::Equip(..))
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in cards {
                    flow::discard(g, k, &id, "主公杀忠臣");
                }
            }
            _ => {}
        }
    }
    // 胜负检查（单一入口）。
    g.winner = super::identity::check_winner(&g.seats);
    events
}

/// 检查胜负并置位（每死亡/结算后调用）。
pub fn check_finished(g: &mut Game) -> Option<super::Winner> {
    if g.winner.is_none() {
        g.winner = super::identity::check_winner(&g.seats);
    }
    g.winner
}
