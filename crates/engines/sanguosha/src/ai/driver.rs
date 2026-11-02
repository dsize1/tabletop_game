//! 机器人自动推进（P6.3）：把「机器人回合」连续推进到人类回合 / 终局。
//!
//! 宿主：**只在 server**（Web 动作通路的单写者）调用——
//! 浏览器动作成功后，server 直接把后续机器人动作跑完并落库，
//! 避免「MCP 与 server 双 `GameManager` 双写」（见 `docs/modules/game-engine.md`）。
//!
//! 与 `ai::selfplay` 的关系：
//! - 自对弈（`selfplay`）操控**全部座位**并采集训练/回归统计；
//! - 本模块只推进 `seat.is_bot` 的座位，带同样的三层兜底（回落 / 破环器 / 步数上限），
//!   两者共用 `state_digest` 与「同技能重复守卫」等 AI 侧策略。

use super::policy::SgsPolicy;
use super::selfplay::state_digest;
use crate::state::{Phase, SgsState};
use game_engine_core::{Action, EngineEvent, GameManager, GameState, GameStatus, Policy, SeededRng};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// 单次推进（一次 Web 动作之后）最多执行的机器人动作数。
pub const MAX_STEPS: usize = 200;

/// 停止原因（写日志/响应体，便于排障）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// 轮到人类（正常停点）。
    HumanTurn,
    /// 对局结束。
    Finished,
    /// 找不到待行动者。
    NoActor,
    /// 待行动者无合法动作（引擎侧异常，仅告警不阻塞）。
    NoLegalActions,
    /// 达到单次推进上限（下次请求继续）。
    MaxSteps,
    /// 状态回环且无可试动作。
    Loop,
    /// 引擎拒绝且无法回落 / 状态异常。
    EngineError,
    /// 不是三国杀对局（其它引擎不在此处推进）。
    NotSanguosha,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::HumanTurn => "human_turn",
            StopReason::Finished => "finished",
            StopReason::NoActor => "no_actor",
            StopReason::NoLegalActions => "no_legal_actions",
            StopReason::MaxSteps => "max_steps",
            StopReason::Loop => "loop",
            StopReason::EngineError => "engine_error",
            StopReason::NotSanguosha => "not_sanguosha",
        }
    }
}

/// 一次推进的结果。
#[derive(Debug, Clone, Default)]
pub struct DriveReport {
    /// 实际执行的机器人动作数（成功落库的次数）。
    pub steps: usize,
    pub finished: bool,
    /// 被引擎拒绝的动作数（已回落，不中断推进）。
    pub illegal: usize,
    /// 破环器强行改判次数。
    pub loops: usize,
    pub stop: Option<StopReason>,
    pub last_error: Option<String>,
    /// 被引擎拒绝的动作明细（已回落，供排障/台账）。
    pub errors: Vec<String>,
}

impl DriveReport {
    pub fn advanced(&self) -> bool {
        self.steps > 0
    }

    fn stop(mut self, reason: StopReason) -> Self {
        self.stop = Some(reason);
        self
    }
}

/// 从引擎状态里找「当前待行动者」：pending.actor → 选将阶段未选者 → 当前回合玩家。
fn current_actor(mgr: &GameManager, room: &str, st: &SgsState) -> Option<String> {
    if let Some(p) = &st.pending {
        return Some(p.actor.clone());
    }
    if st.phase == Phase::Setup {
        return st
            .seats
            .iter()
            .map(|s| s.player_id.clone())
            .find(|pid| !mgr.legal_actions(room, pid).unwrap_or_default().is_empty());
    }
    st.seats.get(st.turn).map(|s| s.player_id.clone())
}

fn is_bot(st: &SgsState, actor: &str) -> bool {
    st.seat(actor).map(|s| s.is_bot).unwrap_or(false)
}

/// 交互内回环检测的范围：无 pending 的重复（如「当先」额外出牌阶段）属合法流程；
/// 无懈链/五谷逐人询问天然多轮重复，由构造保证收敛。
fn loop_trackable(st: &SgsState) -> bool {
    matches!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some(kind) if !matches!(kind, "wuxie" | "wugu")
    )
}

fn fallback_action(legal: &[Action], failed: &str) -> Option<String> {
    for id in ["pass", "confirm", "end_play"] {
        if legal.iter().any(|a| a.id == id) {
            return Some(id.to_string());
        }
    }
    legal
        .iter()
        .map(|a| a.id.clone())
        .find(|id| id != failed)
}

/// 推进机器人回合，直到人类回合 / 终局 / 上限。
///
/// `persist` 由调用方提供（server 传 `game_events` + 快照落库；测试传空实现），
/// 保证「动作 → 事件 → 快照」仍是同一条链路、同一写者。
pub fn drive_bots(
    mgr: &GameManager,
    room: &str,
    max_steps: usize,
    mut persist: impl FnMut(&GameState, &[EngineEvent]),
) -> DriveReport {
    let mut report = DriveReport::default();
    let mut rng = SeededRng::new(0);
    let mut recent: VecDeque<String> = VecDeque::new();
    let mut tried: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut repeat_key = String::new();
    let mut repeat_count = 0usize;

    for _ in 0..max_steps {
        let Some(state) = mgr.get_state(room) else {
            return report
                .stop(StopReason::NoActor)
                .with_error("状态丢失（房间不在内存且无快照）");
        };
        if state.game_id != "sanguosha" {
            return report.stop(StopReason::NotSanguosha);
        }
        if state.status == GameStatus::Finished {
            report.finished = true;
            return report.stop(StopReason::Finished);
        }
        let Ok(st) = SgsState::parse(&state.data) else {
            return report.stop(StopReason::EngineError).with_error("状态解析失败");
        };
        let Some(actor) = current_actor(mgr, room, &st) else {
            return report.stop(StopReason::NoActor);
        };
        if !is_bot(&st, &actor) {
            return report.stop(StopReason::HumanTurn);
        }
        let legal = match mgr.legal_actions(room, &actor) {
            Ok(l) => l,
            Err(e) => {
                return report
                    .stop(StopReason::EngineError)
                    .with_error(format!("legal_actions 失败: {e}"));
            }
        };
        if legal.is_empty() {
            return report
                .stop(StopReason::NoLegalActions)
                .with_error(format!(
                    "机器人 {actor} 无合法动作（phase={:?}）",
                    st.phase
                ));
        }
        let Some(mut action) = SgsPolicy.choose(&state, &actor, &legal, &mut rng) else {
            return report
                .stop(StopReason::EngineError)
                .with_error("策略未给出动作".to_string());
        };

        // 破环器：同一交互状态下换一个未试过的动作（与自对弈同口径）。
        let trackable = loop_trackable(&st);
        let digest = state_digest(&st);
        if trackable && recent.contains(&digest) {
            let seen = tried.entry(digest.clone()).or_default();
            match legal.iter().find(|a| !seen.contains(&a.id) && a.id != action) {
                Some(a) => {
                    seen.insert(a.id.clone());
                    report.loops += 1;
                    action = a.id.clone();
                }
                None => {
                    return report
                        .stop(StopReason::Loop)
                        .with_error(format!("状态回环且无可试动作（{actor}）"));
                }
            }
        } else if trackable {
            recent.push_back(digest);
            if recent.len() > 24 {
                recent.pop_front();
            }
        }

        // 同一 (actor, action) 连续 8 次 → 视作无进展。
        let key = format!("{actor}|{action}");
        if key == repeat_key {
            repeat_count += 1;
            if repeat_count >= 8 {
                return report
                    .stop(StopReason::Loop)
                    .with_error(format!("{key} 连续重复 {repeat_count} 次"));
            }
        } else {
            repeat_key = key;
            repeat_count = 1;
        }

        match mgr.execute_action(room, &actor, &action) {
            Ok(res) => {
                persist(&res.state, &res.events);
                report.steps += 1;
                if res.finished {
                    report.finished = true;
                    return report.stop(StopReason::Finished);
                }
            }
            Err(e) => {
                // 兜底：引擎拒绝时退到 pass/confirm/end_play/其它动作，不让一局卡死。
                report.illegal += 1;
                let engine_error = format!("{e}");
                report.errors.push(format!("{action} 被拒绝：{engine_error}"));
                let Some(fb) = fallback_action(&legal, &action) else {
                    return report
                        .stop(StopReason::EngineError)
                        .with_error(format!("{action} 被拒绝：{engine_error}"));
                };
                match mgr.execute_action(room, &actor, &fb) {
                    Ok(res) => {
                        persist(&res.state, &res.events);
                        report.steps += 1;
                        if res.finished {
                            report.finished = true;
                            return report.stop(StopReason::Finished);
                        }
                    }
                    Err(e2) => {
                        return report.stop(StopReason::EngineError).with_error(format!(
                            "{action} 被拒绝（{engine_error}）；回落 {fb} 也失败：{e2}"
                        ));
                    }
                }
            }
        }
    }

    report.stop(StopReason::MaxSteps)
}

impl DriveReport {
    fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.last_error = Some(msg.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::FULL_GENERALS;
    use game_engine_core::EngineRegistry;

    fn manager(seed: u64) -> GameManager {
        let mut reg = EngineRegistry::new();
        reg.register(crate::SanguoshaEngine);
        GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
    }

    /// 1 真人（u1）+ 4 机器人：开赛、选将（机器人由引擎自动补选）。
    fn human_plus_bots(mgr: &GameManager) -> String {
        let (st, _) = mgr.create_game("sanguosha", "drive-group", "u1").unwrap();
        let room = st.room_id.clone();
        for _ in 0..4 {
            mgr.execute_action(&room, "u1", "lobby:add_bot").unwrap();
        }
        let _ = mgr.execute_action(&room, "u1", &format!("lobby:set_general_packs:{FULL_GENERALS}"));
        mgr.execute_action(&room, "u1", "lobby:start").unwrap();
        // 真人选将（机器人由 `auto_pick_bots` 自动补选）。
        let acts = mgr.legal_actions(&room, "u1").unwrap_or_default();
        if let Some(a) = acts.iter().find(|a| a.id.starts_with("setup:choose_general:")) {
            mgr.execute_action(&room, "u1", &a.id).unwrap();
        }
        room
    }

    #[test]
    fn drive_stops_at_human_turn() {
        let mgr = manager(9001);
        let room = human_plus_bots(&mgr);
        let report = drive_bots(&mgr, &room, MAX_STEPS, |_, _| {});
        assert!(
            report.advanced() || report.finished,
            "应推进机器人（report={report:?}）"
        );
        assert!(
            matches!(report.stop, Some(StopReason::HumanTurn) | Some(StopReason::Finished)),
            "应停在人类回合或终局：{report:?}"
        );
        assert_eq!(report.illegal, 0, "不应出现非法动作：{report:?}");
        // 停在人类回合时，当前待行动者必须是真人。
        if report.stop == Some(StopReason::HumanTurn) {
            let st = SgsState::parse(&mgr.get_state(&room).unwrap().data).unwrap();
            let actor = current_actor(&mgr, &room, &st).expect("actor");
            assert!(!is_bot(&st, &actor), "停在人类回合，实际 actor={actor}");
        }
    }

    #[test]
    fn drive_is_noop_for_unknown_room() {
        let mgr = manager(9002);
        let report = drive_bots(&mgr, "no-such-room", MAX_STEPS, |_, _| {});
        assert_eq!(report.steps, 0);
        assert_eq!(report.stop, Some(StopReason::NoActor));
    }

    /// 真人在每个回合都「结束出牌阶段」，机器人应把整局推进到终局。
    #[test]
    fn drive_can_finish_game_with_human_passing() {
        let mgr = manager(9003);
        let room = human_plus_bots(&mgr);
        for _ in 0..400 {
            let report = drive_bots(&mgr, &room, MAX_STEPS, |_, _| {});
            assert_eq!(
                report.illegal, 0,
                "非法动作：{report:?} errors={:?}",
                report.errors
            );
            if report.finished {
                return;
            }
            assert_eq!(
                report.stop,
                Some(StopReason::HumanTurn),
                "未终局时应停在人类回合：{report:?}"
            );
            let state = mgr.get_state(&room).expect("state");
            let st = SgsState::parse(&state.data).expect("parse");
            let actor = current_actor(&mgr, &room, &st).expect("actor");
            let legal = mgr.legal_actions(&room, &actor).unwrap_or_default();
            if legal.is_empty() {
                panic!("人类无合法动作（phase={:?}）", st.phase);
            }
            // 「能出就出」简化：优先结束出牌 / 放弃响应 / 确认。
            let pick = ["end_play", "pass", "confirm"]
                .iter()
                .find(|id| legal.iter().any(|a| a.id == **id))
                .map(|id| (*id).to_string())
                .unwrap_or_else(|| legal[0].id.clone());
            mgr.execute_action(&room, &actor, &pick).expect("human action");
        }
        panic!("400 轮仍未终局");
    }
}
