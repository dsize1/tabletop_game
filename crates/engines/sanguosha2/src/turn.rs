//! 回合 / 阶段驱动器（M3）。
//!
//! 职责：把「一个阶段结束 → 下一个阶段」的推进从动作层里移出来，保证：
//! - 只有在 `pending` 为空且结算栈为空时才推进（不会把询问吞掉）；
//! - 判定阶段由 `begin_phase` 排入 `JudgePhase` 延迟项（单一延迟队列）；
//! - 弃牌阶段用内核流程帧（`FlowKind::Discard`）询问弃哪些牌；
//! - 胜负只在驱动器里判定（`check_finished`），技能/牌结算不各自判断。
//!
//! 与 `EnginePlugin` 的关系：`new_game` 调 `begin_game`，`execute_action` 每次动作后调 `tick`。
//! 用例/单测可以只调 `resolve_use` / `submit`，不经过驱动器（保持 M1/M2 用例语义）。

use serde_json::json;

use crate::kernel::event::{Event, EventKind};
use crate::kernel::resolve::{self, Ask, FlowFrame, Frame, Stage, Step};
use crate::kernel::{Game, Phase};
use crate::skills;

/// 开局：发 `TurnStart` 并进入第一回合的准备阶段。
pub fn begin_game(g: &mut Game) -> Result<(), String> {
    // 让 `begin_phase` 认为阶段发生变化，从而发出 PhaseStart。
    g.phase = Phase::End;
    g.turn = 0;
    g.round = 1;
    g.events
        .push(Event::simple(EventKind::TurnStart, Some(0), None));
    resolve::begin_phase(g, 0, Phase::Prepare)?;
    tick(g)
}

/// 推进到「需要输入」或「等待出牌动作」为止。
///
/// 幂等：可以在每个动作后调用。
pub fn tick(g: &mut Game) -> Result<(), String> {
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 512 {
            return Err("回合推进次数异常（疑似死循环）".into());
        }
        if g.finished {
            return Ok(());
        }
        if g.pending.is_some() || !g.stack.is_empty() {
            return Ok(());
        }
        check_finished(g);
        if g.finished {
            return Ok(());
        }
        let seat = g.turn;
        match g.phase {
            Phase::Prepare => {
                resolve::begin_phase(g, seat, Phase::Judge)?;
            }
            Phase::Judge => {
                // 神速①：跳过判定阶段（同时取消已排入延迟队列的判定）。
                if g.counter("turn.skip_judge") > 0 {
                    g.counters.insert("turn.skip_judge".to_string(), 0);
                    g.deferred.retain(|d| {
                        !matches!(
                            &d.payload,
                            crate::kernel::deferred::Payload::JudgePhase { seat: s } if *s == seat
                        )
                    });
                    g.push_log(format!("{} 跳过判定阶段", g.seats[seat].name));
                }
                resolve::begin_phase(g, seat, Phase::Draw)?;
            }
            Phase::Draw => {
                g.draw_phase(seat);
                resolve::begin_phase(g, seat, Phase::Play)?;
            }
            Phase::Play => return Ok(()),
            Phase::Discard => {
                if crate::skills::skip_discard_phase(g, seat) {
                    g.push_log(format!("{} 跳过弃牌阶段", g.seats[seat].name));
                    g.events.push(Event::simple(
                        EventKind::PhaseEnd,
                        Some(seat),
                        Some("skip_discard_keji"),
                    ));
                    resolve::begin_phase(g, seat, Phase::End)?;
                    continue;
                }
                if over_hand_limit(g, seat) {
                    start_discard(g, seat);
                    resolve::advance(g)?;
                    return Ok(());
                }
                resolve::begin_phase(g, seat, Phase::End)?;
            }
            Phase::End => {
                // 博图：本回合弃牌含四花色 → 追加一个额外回合。
                crate::skills::botu_maybe(g, seat);
                if g.extra_turn == Some(seat) && !g.extra_turn_active {
                    g.extra_turn = None;
                    g.extra_turn_active = true;
                    g.push_log(format!("博图：{} 执行额外回合", g.seats[seat].name));
                    resolve::begin_phase(g, seat, Phase::Prepare)?;
                    continue;
                }
                g.extra_turn_active = false;
                let Some(next) = next_alive(g, seat) else {
                    g.finished = true;
                    return Ok(());
                };
                if next <= seat {
                    g.round += 1;
                }
                resolve::begin_phase(g, next, Phase::Prepare)?;
            }
        }
    }
}

fn over_hand_limit(g: &Game, seat: usize) -> bool {
    // 藏拙等：部分牌不计入手牌数（不是提高上限）。
    let exempt = crate::skills::hand_limit_exempt(g, seat);
    g.hand(seat).len().saturating_sub(exempt) > g.hand_limit(seat)
}

fn next_alive(g: &Game, from: usize) -> Option<usize> {
    let n = g.seats.len();
    (1..=n)
        .map(|i| (from + i) % n)
        .find(|i| g.seats[*i].hp > 0)
}

/// 胜负判定（只有存在主公身份时才生效；单测夹具没有主公 → 不判定）。
pub fn check_finished(g: &mut Game) {
    if g.finished {
        return;
    }
    let Some(lord) = g.seats.iter().position(|s| s.identity == "lord") else {
        return;
    };
    let lord_alive = g.seats[lord].hp > 0;
    let rebels_alive = g
        .seats
        .iter()
        .any(|s| s.identity == "rebel" && s.hp > 0);
    if !lord_alive {
        g.finished = true;
        g.winner = Some("rebel".to_string());
        g.push_log("主公阵亡：反贼获胜");
    } else if !rebels_alive {
        g.finished = true;
        g.winner = Some("lord".to_string());
        g.push_log("反贼全灭：主公获胜");
    }
}

// ---------------------------------------------------------------------------
// 弃牌阶段（内核流程帧，pending 必须带出口）
// ---------------------------------------------------------------------------

pub fn start_discard(g: &mut Game, seat: usize) {
    g.stack.push(Frame::Flow(FlowFrame {
        kind: crate::kernel::resolve::FlowKind::Discard,
        skill: "__discard".to_string(),
        actor: seat,
        stage: Stage::CardOpt,
        data: json!({ "seat": seat }),
    }));
}

pub fn step_discard(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let seat = a.actor;
    if !over_hand_limit(g, seat) {
        return Ok(Step::Done);
    }
    let limit = g.hand_limit(seat);
    let answer = g.answer.take();
    if let Some(ans) = answer {
        if ans == "confirm" {
            return Err(format!("还需弃置 {} 张牌", g.hand(seat).len() - limit));
        }
        let id = ans
            .strip_prefix("discard:")
            .ok_or_else(|| format!("弃牌阶段：未知选项 {ans}"))?
            .to_string();
        if !g.hand(seat).contains(&id) {
            return Err("弃牌阶段：该牌不在手牌".into());
        }
        g.to_discard(&id, "discard_phase")?;
        if !over_hand_limit(g, seat) {
            return Ok(Step::Done);
        }
    }
    let mut b = skills::builder(seat, "discard_phase").skill("__discard");
    for id in g.hand(seat) {
        b = b.option(
            format!("discard:{id}"),
            format!("弃置【{}】", g.card_label(&id)),
        );
    }
    Ok(Step::Need(b.escape("结束弃牌阶段").build()))
}
