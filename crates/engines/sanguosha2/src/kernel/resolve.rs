//! 结算栈 + 推进器。
//!
//! 不变量（plan §2.1）：**技能不得就地压/弹帧**。技能 handler 只返回 `Step`，
//! 由 `advance` 统一改栈 → 「父帧 pop 把子帧弹掉」类缺陷在结构上不可能发生。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::deferred::{Deferred, Payload};
use super::event::{EventKind, Window};
use super::pending::Pending;
use super::{CardId, Game, Phase, Zone};
use crate::presentation::{POutcome, Presentation};
use crate::skills;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// 装备窗口：选择是否发动 / 发动哪一项。
    GearChoose,
    /// 装备窗口：逐张选牌（贯石斧/寒冰剑/雌雄）。
    GearPick,
    /// 结算前钩子（谮毁）。
    PreUse,
    /// 通用：是/否/选项询问。
    Choose,
    /// 选择目标或候选牌。
    Target,
    /// 良姻·弃置：发动者弃 1。
    DiscardOwner,
    /// 良姻·弃置：目标弃 1。
    DiscardTarget,
    /// 良姻·回血判定。
    Heal,
    /// 箜声·准备阶段置牌。
    PreparePick,
    /// 箜声·结束阶段取回/归还装备。
    EndReturn,
    /// 箜声·使用剩余装备。
    UseEquip,
    /// 贞良（阴）·选择摸牌目标。
    YinTarget,
    /// 贞良（阳）·选择目标。
    YangTarget,
    /// 贞良（阳）·弃同色牌。
    YangCard,
    /// 通用：从自己的手牌选一张（不弃置，交由技能处理；御策展示 / 权计置「权」）。
    PickOwn,
    /// 通用：由非发动者选择一项（御策来源二选一 / 峻刑目标二选一）。
    PickOther,
    /// 通用：发动者二选一（品第：摸 / 弃）。
    Mode,
    /// 濒死流程（内核）。
    DyingAsk,
    /// 无懈窗口（内核）。
    WuxieAsk,
    /// 牌结算：等待目标/使用者给出响应牌（闪/杀/无懈替代）。
    CardAsk,
    /// 牌结算：从某角色的牌区选择一张牌（过河拆桥 / 顺手牵羊）。
    CardPick,
    /// 牌结算：牌特有的额外选择（火攻弃同花色 / 五谷选牌 / 借刀出杀或交武器）。
    CardOpt,
    /// 技能声明的牌需要选目标（滔乱：按声明牌名的目标规则）。
    CardTarget,
    /// 拼点：发起方暗选拼点牌。
    PindianInit,
    /// 拼点：目标方暗选拼点牌。
    PindianResp,
    /// 判定阶段（内核）：逐张结算判定区延时锦囊。
    JudgeResolve,
    /// 技能判定子流程（亮牌 → 改判 → 生效 → 天妒）。
    QuickJudge,
    /// 翻面结算完（解围②等子流程返回后继续）。
    AfterFlip,
    /// 濒死结算完（奋激：脱困后才摸牌）。
    AfterDying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowKind {
    /// 装备窗口（青龙偃月刀 / 贯石斧 / 寒冰剑 / 雌雄双股剑 / 麒麟弓）。
    Gear,
    /// 结算前钩子（谮毁：代用 / 额外目标）。
    PreUse,
    Dying,
    Wuxie,
    /// 牌结算（使用→目标→响应→结算→弃牌/装备/判定区）。
    Card,
    /// 拼点子流程（双方暗选 → 亮牌比点数 → 双方拼点牌进弃牌堆）。
    Pindian,
    /// 判定阶段：延时锦囊 LIFO 结算。
    Judge,
    /// 技能判定子流程。
    QuickJudge,
    /// 弃牌阶段：逐张弃置直到手牌上限。
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFrame {
    pub kind: FlowKind,
    pub skill: String,
    pub actor: usize,
    pub stage: Stage,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub skill: String,
    pub stage: Stage,
    pub actor: usize,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Ask(Ask),
    /// 内核流程帧（濒死 / 无懈）：handler 在 `kernel::flow`，与技能无关。
    Flow(FlowFrame),
    /// 伤害后钩子（压在濒死之下，救起后仍会执行）。
    DamageAfter { victim: usize, source: Option<usize> },
}

#[derive(Debug)]
pub enum Step {
    Next(Stage, Value),
    Done,
    Need(Pending),
    /// 切换阶段并立刻询问（`Need` 不会改阶段，容易导致「答完又走同一段」）。
    NeedAt(Stage, Pending),
    Finish(String),
}

pub fn push_ask(
    g: &mut Game,
    skill: impl Into<String>,
    stage: Stage,
    actor: usize,
    data: Value,
) {
    g.stack.push(Frame::Ask(Ask {
        skill: skill.into(),
        stage,
        actor,
        data,
    }));
}

/// 推进到「需要输入」或「无事可做」。
pub fn advance(g: &mut Game) -> Result<(), String> {
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 2000 {
            return Err("结算推进次数异常（疑似死循环）".into());
        }
        if g.pending.is_some() || g.finished {
            return Ok(());
        }
        if !g.stack.is_empty() {
            // 记录当前帧索引：handler 可能压入子帧（如翻面触发解围），
            // Step::Next/Done 必须作用在**发起帧**上，而不是栈顶。
            let idx = g.stack.len() - 1;
            let frame = g.stack[idx].clone();
            // 回答归属：**事件触发的技能帧**不得消费上一个待办的回答（会读成自己的选项）。
            //
            // 背景（2026-11-10 自对弈抓到）：技能 A 的 pending 被提交后，A 重入时因前置
            // 条件失效直接 `Step::Done`（没消费回答），事件随后触发技能 B（龙吟），B 把
            // `opt1` / `choose:*` 当成自己的选项 → `未知选项`。
            // 只拦「事件触发 + 非发起技能」这一种情况：牌结算/濒死/接力等流程帧仍按原语义
            // 跨辅助函数消费回答（`read_answer` 依赖它）。
            let (frame_skill, is_ask) = match &frame {
                Frame::Ask(a) => (a.skill.clone(), true),
                Frame::Flow(f) => (f.skill.clone(), false),
                _ => (String::new(), false),
            };
            if g.answer.is_some() {
                // 技能帧（Ask）：回答只能由**发起该待办的技能帧**消费；
                // 内核流程帧（Flow：牌结算/濒死/无懈…）保持原有跨辅助函数的消费语义。
                let own = g.answer_skill.as_deref() == Some(frame_skill.as_str());
                let fallback = g.answer_skill.is_none() && g.answer_budget > 0;
                if is_ask && !own && !fallback {
                    g.answer = None;
                }
            }
            g.answer_budget = 0;
            let step = match frame {
                Frame::Ask(a) => {
                    let h = skills::handler(&a.skill)
                        .ok_or_else(|| format!("未知技能处理器：{}", a.skill))?;
                    h(g, &a)?
                }
                Frame::Flow(f) => match f.kind {
                    FlowKind::PreUse => super::card_play::step_pre_use(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Gear => super::gear::step_gear(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Dying => super::flow::step_dying(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Wuxie => super::flow::step_wuxie(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Card => super::card_play::step_card(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Judge => super::card_play::step_judge(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Discard => crate::turn::step_discard(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::QuickJudge => super::card_play::step_quick_judge(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                    FlowKind::Pindian => super::pindian::step_pindian(g, &Ask {
                        skill: f.skill.clone(),
                        stage: f.stage,
                        actor: f.actor,
                        data: f.data.clone(),
                    })?,
                },
                Frame::DamageAfter { victim, source } => {
                    g.stack.pop();
                    let ev = super::event::Event::simple(
                        EventKind::DamageAfter,
                        source,
                        Some(&format!("victim={victim}")),
                    );
                    g.emit(ev);
                    Step::Done
                }
            };
            match step {
                Step::Next(stage, data) => {
                    match &mut g.stack[idx] {
                        Frame::Ask(ask) => {
                            ask.stage = stage;
                            ask.data = data;
                        }
                        Frame::Flow(f) => {
                            f.stage = stage;
                            f.data = data;
                        }
                        _ => return Err("栈顶不可替换阶段".into()),
                    }
                    continue;
                }
                Step::Done => {
                    g.stack.remove(idx);
                    continue;
                }
                Step::Need(p) => {
                    // 同一技能重入后重新提问 = 上一个回答未被消费 → 丢弃，避免污染。
                    g.answer = None;
                    g.answer_skill = Some(frame_skill.clone());
                    g.pending = Some(p);
                    return Ok(());
                }
                Step::NeedAt(stage, p) => {
                    g.answer = None;
                    g.answer_skill = Some(frame_skill.clone());
                    match &mut g.stack[idx] {
                        Frame::Ask(ask) => ask.stage = stage,
                        Frame::Flow(f) => f.stage = stage,
                        _ => return Err("栈顶不可替换阶段".into()),
                    }
                    g.pending = Some(p);
                    return Ok(());
                }
                Step::Finish(msg) => {
                    g.stack.remove(idx);
                    g.push_log(msg);
                    continue;
                }
            }
        }
        // 事件先于延迟队列：事件是「刚发生的事实」，延迟队列是「事实之后要补的询问」。
        // 否则死亡清理会跑在行殇之前（B0 实测）。
        if !g.events.is_empty() {
            // 镇骨的「第二个时刻」：被选角色回合结束时结算（框架层，不认识技能细节）
            let ended = g.events[0]
                .kind
                == crate::kernel::event::EventKind::PhaseEnd
                && g.events[0].cause.as_deref() == Some("End");
            if ended {
                if let Some(seat) = g.events[0].seat {
                    crate::skills::on_turn_end_for_scheduled(g, seat);
                }
            }
            route_one_event(g);
            continue;
        }
        // 单一延迟队列：按显式顺序表取最早的一项。
        if let Some(idx) = next_deferred_index(g) {
            let d = g.deferred.remove(idx);
            run_deferred(g, d)?;
            continue;
        }
        // 连营：手牌归零后（栈空、事件清空）询问至多 X 名角色各摸 1。
        if let Some((seat, x)) = g.lianying_pending.take() {
            if crate::skills::start_lianying(g, seat, x) {
                continue;
            }
        }
        // 结束阶段触发已结算完：完成回合切换并进入新回合首阶段（M12 / ADR-0039）。
        if g.pending.is_none() && g.stack.is_empty() {
            if let Some((seat, phase)) = g.switch_to.take() {
                complete_turn_switch(g, seat);
                return enter_phase(g, seat, phase, false);
            }
        }
        break;
    }
    // 不变量：结算栈空且无待办时，`answer` 必须已被消费（否则会污染下一个动作的技能）。
    //
    // 历史缺陷（2026-11-10 自对弈抓到）：技能在「重入时前置条件已失效」的分支返回
    // `Step::Done` 而没有 `g.answer.take()`，未消费的回答被序列化进快照，
    // 下一个动作的技能（奋威）把它当成自己的选项 → `未知选项 choose:kuizhu:draw`。
    if g.pending.is_none() && g.stack.is_empty() {
        g.answer = None;
        g.answer_skill = None;
        g.answer_budget = 0;
    }
    Ok(())
}

fn next_deferred_index(g: &Game) -> Option<usize> {
    g.deferred
        .iter()
        .enumerate()
        .min_by_key(|(i, d)| (d.order_index(), *i))
        .map(|(i, _)| i)
}

fn run_deferred(g: &mut Game, d: Deferred) -> Result<(), String> {
    match d.payload {
        Payload::Trigger {
            owner,
            skill,
            event,
            seat,
            card,
            cause,
            amount,
            turn,
            phase,
            by,
            from,
        } => {
            // handler 一律从 `data` 读「发生时刻」：`turn` / `phase`。
            let mut data = json!({
                "cause": cause,
                "event": event,
                "seat": seat,
                "turn": turn,
                "phase": format!("{phase:?}"),
                "by": by,
                "from": from,
            });
            if let Some(card) = card {
                data["card"] = json!(card);
            }
            if let Some(amount) = amount {
                data["amount"] = json!(amount);
            }
            push_ask(g, skill, Stage::Choose, owner, data);
            Ok(())
        }
        Payload::Ask {
            skill,
            stage,
            actor,
            data,
        } => {
            if skill == "__death_cleanup" {
                let victim = data["victim"].as_u64().unwrap_or(0) as usize;
                super::flow::cleanup_dead(g, victim)?;
                return Ok(());
            }
            push_ask(g, skill, stage, actor, data);
            Ok(())
        }
        Payload::Land { card, to } => {
            // 帧暂存牌落盘：唯一出口。
            if let Some(frame) = g.zone_of(&card).cloned() {
                if frame.is_frame() {
                    let mut tmp = json!({});
                    let _ = &mut tmp;
                    // 通过 from_void/gain 之外的低层落盘需要 kernel 内部 API：
                    // 这里用专用 helper（Land 是框架层，不属于任何技能）。
                    g.land_card(&card, to)?;
                }
            }
            Ok(())
        }
        Payload::JudgePhase { seat } => {
            super::card_play::start_judge(g, seat);
            Ok(())
        }
    }
}

fn route_one_event(g: &mut Game) {
    if g.events.is_empty() {
        return;
    }
    let ev = g.events.remove(0);
    // 开局/发牌等系统事件（Setup）不触发技能：发牌不是「获得」，组牌堆不是「移出游戏」。
    if ev.lifecycle == super::event::Lifecycle::Setup {
        return;
    }
    let mut matched: Vec<super::event::Sub> = g
        .subs
        .iter()
        .filter(|s| s.accepts(&ev))
        .cloned()
        .collect();
    // 「主体窗口」：客观判定（如奋激：受影响的角色必须在回合外）。
    // 由技能声明，不写进 handler；判定同样用事件快照，避免回合切换后的误读。
    matched.retain(|s| {
        let Some(def) = crate::skills::def(&s.skill) else {
            return true;
        };
        // 主公技统一闸门（伪帝借用由 may_use_lord_skill 处理）。
        if def.kind == crate::skills::SkillKind::Lord
            && !crate::skills::lord_gate_ok(g, s.owner, &s.skill)
        {
            return false;
        }
        let Some(sw) = def.subject_window else {
            return true;
        };
        let subject = ev.seat.unwrap_or(s.owner);
        sw.allows(ev.turn, ev.phase, subject)
    });
    super::event::sort_subs(&mut matched, ev.turn, g.seats.len());
    for s in matched {
        g.deferred.push(Deferred {
            kind: super::deferred::DeferredKind::SkillTrigger,
            payload: Payload::Trigger {
                owner: s.owner,
                skill: s.skill,
                event: format!("{:?}|side={}", ev.kind, s.side),
                seat: ev.seat,
                card: ev.card.clone(),
                cause: ev.cause.clone(),
                amount: ev.amount,
                turn: ev.turn,
                phase: ev.phase,
                by: ev.by,
                from: ev.from,
            },
        });
    }
}

/// 提交一个动作（当前 pending 的回答）。返回执行后的结果文案。
pub fn submit(g: &mut Game, actor: usize, action: &str) -> Result<String, String> {
    let Some(p) = g.pending.clone() else {
        return Err("当前没有待办".into());
    };
    if p.actor() != actor {
        return Err("当前等待其他玩家响应".into());
    }
    if !p.has_option(action) {
        return Err(format!("非法选项：{action}"));
    }
    // 强制选择类待办：出口只是类型系统占位 → 自动改选第一个实质选项（不能放弃）。
    let mut action = action.to_string();
    if p.escape_hidden() && action == p.escape_id() {
        let Some(first) = p.options().iter().find(|o| o.id != p.escape_id()) else {
            return Err("该待办必须选择，但没有可选项".into());
        };
        action = first.id.clone();
    }
    let action = action.as_str();
    g.begin_action_as(actor);
    g.pending = None;
    g.answer = Some(action.to_string());
    g.answer_budget = 1;
    // 「未发动」只用于真正的放弃出口（`pass` / `cancel`）：觉醒技等把真实收益当出口时不算放弃。
    if p.escape_id() == action && matches!(action, "pass" | "cancel") {
        let sk = p.skill().unwrap_or("pending").to_string();
        g.presentation.push(
            Presentation::new(sk.clone(), POutcome::Declined { skill: sk }).fx("pending_cancel"),
        );
    }
    let log_before = g.log.len();
    advance(g)?;
    // 有些「放弃/选择」不产生业务日志（如 `pass`）：补一条动作记录，
    // 保证 Web 侧栏「出牌历史」能即时反映每一步交互（与 v1 的 message 口径一致）。
    if g.log.len() == log_before {
        let name = g.seats.get(actor).map(|s| s.name.clone()).unwrap_or_default();
        let label = if action == "pass" {
            "放弃".to_string()
        } else {
            action.to_string()
        };
        g.push_log(format!("{name} 选择：{label}"));
    }
    Ok(g
        .log
        .last()
        .map(|e| e.text.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| format!("已执行：{action}")))
}

/// 压入「结算前钩子」帧（谮毁：代用 / 额外目标）。
pub fn start_pre_use(
    g: &mut Game,
    user: usize,
    card: Option<&CardId>,
    name: &str,
    targets: Vec<usize>,
) {
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::PreUse,
        skill: "__preuse".to_string(),
        actor: user,
        stage: Stage::PreUse,
        data: json!({
            "user": user,
            "card": card.map(|c| c.to_string()),
            "name": name,
            "targets": targets,
            "asked": false,
        }),
    }));
}

/// 回合/阶段切换（测试与引擎共用）：清一次性计数器 + 发事件。
pub fn begin_phase(g: &mut Game, seat: usize, phase: Phase) -> Result<(), String> {
    if g.turn != seat {
        // 回合切换：先让「回合结束/结束阶段」的触发结算完，再清计数并换人（M12 / ADR-0039）。
        // 期间若产生 pending，`advance` 会停下；`switch_to` 在队列清空后由 `advance` 完成。
        crate::skills::on_turn_end(g, g.turn);
        let old_turn = g.turn;
        let old_phase = g.phase;
        g.emit(super::event::Event::simple(
            EventKind::PhaseEnd,
            Some(old_turn),
            Some(&format!("{old_phase:?}")),
        ));
        // 2026-11-11 回滚：延迟切回合（switch_to）在自对弈中导致卡死/回环（见 engine-self-audit M12）。
        // 暂按「先切回合、PhaseEnd 已带事件快照」的 A 口径运行；B 待用「计数快照」方案重做。
        complete_turn_switch(g, seat);
        return enter_phase(g, seat, phase, false);
    }
    enter_phase(g, seat, phase, true)
}

/// 完成回合切换（在所有结束阶段触发结算完之后调用）。
fn complete_turn_switch(g: &mut Game, seat: usize) {
    for i in 0..g.seats.len() {
        g.unfreeze_hand(i);
        // 义绝·黑的手牌封印只持续到本回合结束。
        g.seats[i].flags.remove("hand_sealed");
    }
    // 谦逊：回合结束时把移出游戏的手牌归还。
    crate::skills::return_void_cards(g, g.turn_seq);
    g.clear_prefix("turn.");
    // 本回合的「已使用牌 / 不计次数标记」随回合结束清空（派生计数 sha_used 依赖它）。
    g.used_this_turn.clear();
    g.uncounted.clear();
    // 明鉴：交牌时置 1（待生效）→ 其下个回合开始时置 2（本回合生效）→ 再下一回合清零。
    let key = format!("mingjian:{seat}");
    let cur = g.counters.get(&key).copied().unwrap_or(0);
    if cur >= 1 {
        g.counters.insert(key, if cur == 1 { 2 } else { 0 });
    }
    g.turn = seat;
    g.turn_seq += 1;
    g.emit(super::event::Event::simple(
        EventKind::TurnStart,
        Some(seat),
        None,
    ));
}

/// 进入某阶段（不含回合切换的计数清理）。`emit_phase_end` = 是否补发旧阶段的 PhaseEnd。
pub(crate) fn enter_phase(
    g: &mut Game,
    seat: usize,
    phase: Phase,
    emit_phase_end: bool,
) -> Result<(), String> {
    let mut phase = phase;
    // 判定区效果（乐不思蜀）：跳过出牌阶段（唯一消费点；判定阶段写入 `turn.skip_play`）。
    if phase == Phase::Play && g.counter("turn.skip_play") > 0 {
        g.counters.insert("turn.skip_play".to_string(), 0);
        g.push_log(format!("{} 跳过出牌阶段", g.seats[seat].name));
        phase = Phase::Discard;
    }
    // 阶段开始前的技能询问（巧变）：可暂停切换（返回后由驱动器重试同一目标阶段）。
    if phase != g.phase && crate::skills::before_phase(g, seat, phase) {
        advance(g)?;
        return Ok(());
    }
    if g.phase != phase {
        // 阶段结束事件（供「阶段结束时」技能；cause = 旧阶段名）。
        if emit_phase_end {
            let old_phase = g.phase;
            g.emit(super::event::Event::simple(
                EventKind::PhaseEnd,
                Some(seat),
                Some(&format!("{old_phase:?}")),
            ));
        }
        g.phase = phase;
        g.clear_prefix("phase.");
        g.phase_discard_cards.clear();
        // 阶段级手牌冻结到期（司敌：只在被封锁角色的出牌阶段内有效）。
        for i in 0..g.seats.len() {
            g.unfreeze_phase(i);
        }
        g.emit(super::event::Event::simple(
            EventKind::PhaseStart,
            Some(seat),
            Some(&format!("{phase:?}")),
        ));
        // 判定阶段：排一个「延时锦囊 LIFO 结算」（在阶段开始技能触发之后）。
        if phase == Phase::Judge
            && g.counter("turn.skip_judge") == 0
            && g.judge_zone(seat)
                .iter()
                .any(|id| g.def_of(id).tags.delayed)
        {
            g.deferred.push(super::deferred::Deferred {
                kind: super::deferred::DeferredKind::JudgePhase,
                payload: super::deferred::Payload::JudgePhase { seat },
            });
        }
    }
    advance(g)
}

/// 供 kernel 内部落盘使用（Land 延迟项）。
pub fn land(g: &mut Game, card: &CardId, to: Zone) -> Result<(), String> {
    g.land_card(card, to)
}

/// 便捷：把窗口断言写进测试。
pub fn window_allows(w: Window, turn: usize, phase: Phase, owner: usize) -> bool {
    w.allows(turn, phase, owner)
}
