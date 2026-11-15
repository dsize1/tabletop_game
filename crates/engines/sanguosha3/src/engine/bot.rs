//! 机器人：**打分策略**（B0–B20 批次前也能单人真机）+ **推进循环**（server `/drive`）。
//!
//! 打分只读「公共信息 + 自己的身份」：他人身份仅在主公/死亡后公开时才参与敌我判定
//! （与真人口径一致，AI 不作弊）；未知身份按中性处理。
//!
//! 口径：打分是**启发式基线**（不是 MCTS），与武将技能无关——所有分数只依赖牌/体力/目标。

use game_engine_core::{Action, EngineEvent, GameManager, GameState, GameStatus};
use serde::{Deserialize, Serialize};

use crate::kernel::cards::{self, Effect};
use crate::kernel::{Game, Identity};

use super::EngineState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    HumanTurn,
    Finished,
    MaxSteps,
    NoState,
    Error,
}

impl StopReason {
    /// 报告/日志用的稳定字符串（server `bot_stop` 字段）。
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::HumanTurn => "human_turn",
            StopReason::Finished => "finished",
            StopReason::MaxSteps => "max_steps",
            StopReason::NoState => "no_state",
            StopReason::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub actor: String,
    pub seq: i64,
    pub action: String,
    pub score: f64,
    /// 动作前的合法集（决策日志用）。
    #[serde(default)]
    pub legal: Vec<Action>,
    #[serde(default)]
    pub candidates: Vec<Action>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub state_digest: String,
    #[serde(default)]
    pub latency_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriveReport {
    pub steps: usize,
    pub finished: bool,
    pub illegal: usize,
    pub loops: usize,
    pub stop: Option<StopReason>,
    pub last_error: Option<String>,
    pub decisions: Vec<Decision>,
    pub errors: Vec<String>,
}

/// 单次决策轨迹（与 v1/v2 的 `DecisionTrace` 同形，供 MCP/决策日志复用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub action: String,
    pub legal: Vec<Action>,
    pub candidates: Vec<Action>,
    pub priority: i32,
    pub score: f64,
    pub reason: String,
    pub state_digest: String,
}

/// 打分决策 + 轨迹（MCP `bot_action` / 真机排障用）。
pub fn decide_traced(state: &GameState, player_id: &str, legal: &[Action]) -> Option<Trace> {
    let es = EngineState::from_state(state).ok()?;
    let seat = es.kernel.seat_of(player_id)?;
    let action = choose(&es.kernel, seat)?;
    let kind = es
        .kernel
        .pending
        .as_ref()
        .map(|p| p.kind().to_string())
        .unwrap_or_default();
    let score = score_option(&es.kernel, seat, &kind, &action) as f64;
    Some(Trace {
        action: action.clone(),
        legal: legal.to_vec(),
        candidates: legal.to_vec(),
        priority: 0,
        score,
        reason: format!("heuristic:{kind}"),
        state_digest: state_digest_of(state).unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// 敌我判定（只读公开信息）
// ---------------------------------------------------------------------------

fn identity_public(g: &Game, seat: usize) -> bool {
    g.seats
        .get(seat)
        .map(|s| s.identity == Some(Identity::Lord) || !s.alive)
        .unwrap_or(false)
}

/// +1 敌对 / -1 友方 / 0 未知（只读公开身份：主公与死亡角色）。
pub fn relation(g: &Game, me: usize, other: usize) -> i32 {
    if me == other {
        return 0;
    }
    let my = match g.seats.get(me).and_then(|s| s.identity) {
        Some(i) => i,
        None => return 0,
    };
    let their = match g.seats.get(other).and_then(|s| s.identity) {
        Some(i) if identity_public(g, other) => i,
        _ => return 0,
    };
    use Identity::*;
    let enemy = match my {
        Lord | Loyalist => matches!(their, Rebel | Spy),
        Rebel => matches!(their, Lord | Loyalist | Spy),
        Spy => !matches!(their, Spy),
    };
    if enemy {
        1
    } else {
        -1
    }
}

fn is_enemy(g: &Game, me: usize, other: usize) -> bool {
    relation(g, me, other) > 0
}

// ---------------------------------------------------------------------------
// 打分
// ---------------------------------------------------------------------------

fn def_of(g: &Game, id: &str) -> Option<cards::CardDef> {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .copied()
}

/// 牌的基础价值（选牌/弃牌/获得共用）。
pub fn card_value(g: &Game, id: &str) -> i32 {
    let Some(def) = def_of(g, id) else { return 0 };
    match def.effect {
        Effect::Tao => 50,
        Effect::ExNihilo => 46,
        Effect::Sha => 32,
        Effect::Shan => 32,
        Effect::Jiu => 22,
        Effect::Wuxie => 26,
        Effect::Dismantle | Effect::Snatch => 34,
        Effect::Duel | Effect::FireAttack => 30,
        Effect::SavageAssault | Effect::ArcheryAttack => 28,
        Effect::PeachGarden | Effect::Harvest => 24,
        Effect::Indulgence | Effect::SupplyShortage => 34,
        Effect::Lightning => 10,
        Effect::IronChain | Effect::Collateral => 18,
        Effect::None => {
            if def.slot.is_some() {
                30
            } else {
                12
            }
        }
    }
}

/// 目标的基础价值（越低血越优先；敌我按公开身份）。
fn target_score(g: &Game, me: usize, other: usize) -> i32 {
    let Some(s) = g.seats.get(other) else { return -999 };
    if !s.alive {
        return -999;
    }
    let mut score = 0;
    score += relation(g, me, other) * 25;
    score -= s.hp * 4;
    let _ = s;
    if g.hand_size(other) == 0 {
        score += 3;
    }
    score
}

/// 【杀】/伤害锦囊的最佳目标分。
fn best_target_score(g: &Game, me: usize, def: &cards::CardDef) -> i32 {
    crate::kernel::card_play::legal_targets(g, me, def)
        .into_iter()
        .map(|t| target_score(g, me, t))
        .max()
        .unwrap_or(-50)
}

/// 是否还有【杀】可用（酒的价值判定）。
fn has_slash(g: &Game, me: usize) -> bool {
    g.hand(me)
        .iter()
        .any(|id| def_of(g, id).map(|d| d.tags.sha).unwrap_or(false))
}

/// 出牌阶段「使用某张牌」的分数。
fn card_use_score(g: &Game, me: usize, id: &str, fire: bool) -> i32 {
    let Some(def) = def_of(g, id) else { return 0 };
    let mut score = match def.effect {
        Effect::Sha => {
            let base = 30 + best_target_score(g, me, &def);
            // 酒杀/火杀（对藤甲）价值略高。
            base + if fire { 6 } else { 0 }
        }
        Effect::Tao => {
            let s = &g.seats[me];
            // 留桃救命：只在体力见底（≤1）时主动回血。
            if s.hp <= 1 {
                40
            } else {
                0
            }
        }
        Effect::Jiu => {
            if has_slash(g, me) {
                24
            } else {
                0
            }
        }
        Effect::Shan | Effect::Wuxie => 0,
        Effect::ExNihilo => 36,
        Effect::PeachGarden => {
            // 自己/友方缺血越多越值钱，敌人回血打折。
            let mut v = 0;
            for s in 0..g.seats.len() {
                if !g.seats[s].alive {
                    continue;
                }
                let heal = (g.seats[s].max_hp - g.seats[s].hp).clamp(0, 1);
                let rel = if s == me { -1 } else { relation(g, me, s) };
                v += heal * if rel < 0 { 10 } else { -12 };
            }
            14 + v
        }
        Effect::Dismantle | Effect::Snatch => {
            let best = crate::kernel::card_play::legal_targets(g, me, &def)
                .into_iter()
                .map(|t| {
                    let cards = g
                        .cards
                        .values()
                        .filter(|z| {
                            z.seat() == Some(t)
                        })
                        .count() as i32;
                    relation(g, me, t) * 20 + cards
                })
                .max()
                .unwrap_or(-20);
            20 + best
        }
        Effect::Duel => 26 + best_target_score(g, me, &def) / 2,
        Effect::SavageAssault | Effect::ArcheryAttack => {
            let enemies = (0..g.seats.len())
                .filter(|s| *s != me && g.seats[*s].alive && is_enemy(g, me, *s))
                .count() as i32;
            let allies = (0..g.seats.len())
                .filter(|s| *s != me && g.seats[*s].alive && relation(g, me, *s) < 0)
                .count() as i32;
            12 + enemies * 6 - allies * 5
        }
        Effect::Harvest => 14,
        Effect::FireAttack => 18 + best_target_score(g, me, &def) / 3,
        Effect::Collateral => 16,
        Effect::IronChain => 8,
        Effect::Indulgence | Effect::SupplyShortage => {
            22 + crate::kernel::card_play::legal_targets(g, me, &def)
                .into_iter()
                .map(|t| target_score(g, me, t))
                .max()
                .unwrap_or(-30)
        }
        Effect::Lightning => 2,
        Effect::None => {
            if def.slot.is_some() {
                // 已有同槽装备时优先不换（避免白亏牌）。
                18
            } else {
                6
            }
        }
    };
    if def.tags.sha {
        // 杀在敌方空城时更值；这里只做轻微修正，避免与目标分重复。
        score += 0;
    }
    score
}

fn pick_score(g: &Game, me: usize, id: &str) -> i32 {
    // `c:`（拆/顺/五谷/寒冰）取走：价值越高越好
    let value = id.split_once(':').map(|(_, c)| c).unwrap_or("");
    let _ = me;
    card_value(g, value)
}

/// 选项打分（`kind` = pending.kind）。
pub fn score_option(g: &Game, me: usize, kind: &str, id: &str) -> i32 {
    match id {
        "end" => 14,
        "ok" => 6,
        // 目标选择阶段「取消」= 白扔一次使用，绝不该选；其他场景允许放弃。
        "cancel" => {
            if kind == "card_target" {
                -1000
            } else {
                0
            }
        }
        "pass" => 0,
        _ if id.starts_with("card:") => {
            let rest = &id["card:".len()..];
            let (card, fire) = match rest.split_once(':') {
                Some((c, f)) => (c, f == "fire"),
                None => (rest, false),
            };
            match kind {
                // 濒死救援：救自己/队友，不救敌人（救敌人等于自杀）。
                "dying_ask" => {
                    let target = g.pending.as_ref()
                        .and_then(|p| p.data().get("target"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                        .unwrap_or(me);
                    let rel = if target == me { -1 } else { relation(g, me, target) };
                    match rel {
                        -1 => 100,
                        0 => 60,
                        _ => 0,
                    }
                }
                _ => card_use_score(g, me, card, fire),
            }
        }
        _ if id.starts_with("t:") => {
            10 + id["t:".len()..]
                .parse::<usize>()
                .map(|t| target_score(g, me, t))
                .unwrap_or(0)
        }
        _ if id.starts_with("s:") => match kind {
            "shan_ask" | "collateral_shan" => 46,
            "duel_ask" => 34,
            "fire_attack_discard" => -card_value(g, &id[2..]) / 4,
            _ => 40, // 借刀/青龙（使用杀）
        },
        _ if id.starts_with("p:") => 42,
        _ if id.starts_with("w:") => 24,
        _ if id.starts_with("again:") => 44,
        _ if id.starts_with("c:") => pick_score(g, me, id),
        _ if id.starts_with("q:") => 8,
        _ if id.starts_with("r:") => -card_value(g, &id[2..]) / 3,
        _ if id.starts_with("discard:") => {
            // 弃牌代价：越不值钱的牌越先弃。
            20 - card_value(g, &id["discard:".len()..])
        }
        _ if id.starts_with("zbc:") => card_value(g, &id["zbc:".len()..]),
        _ if id.starts_with("skill:") => 20,
        "zhangba" => 22,
        "guanshi" => 18,
        "bagua" => 26,
        "hanbing" => 12,
        "draw" => 20,
        _ => 5,
    }
}

/// 选择当前待办的动作（打分最高；同分取更靠后的选项以避免「总是 pass」）。
pub fn choose(g: &Game, me: usize) -> Option<String> {
    let p = g.pending.as_ref()?;
    if p.actor() != me {
        return None;
    }
    let mut best: Option<(i32, String)> = None;
    for o in p.options() {
        if p.escape_hidden() && o.id == p.escape_id() {
            continue;
        }
        let score = score_option(g, me, p.kind(), &o.id);
        match &best {
            Some((s, _)) if *s >= score => {}
            _ => best = Some((score, o.id.clone())),
        }
    }
    best.map(|(_, id)| id)
}

// ---------------------------------------------------------------------------
// 推进（server `/drive`）
// ---------------------------------------------------------------------------

/// 局面摘要（决策日志 `state_digest` 用）。
pub fn state_digest_of(state: &GameState) -> Option<String> {
    let es = EngineState::from_state(state).ok()?;
    let g = &es.kernel;
    let hps: Vec<i32> = g.seats.iter().map(|s| s.hp).collect();
    let hands: Vec<usize> = (0..g.seats.len()).map(|s| g.hand_size(s)).collect();
    Some(format!(
        "turn={} phase={:?} round={} hps={hps:?} hands={hands:?} pending={}",
        g.turn,
        g.phase,
        g.round,
        g.pending
            .as_ref()
            .map(|p| p.kind().to_string())
            .unwrap_or_else(|| "-".to_string()),
    ))
}

/// 把所有机器人座位推进到人类回合/终局（单调：每次执行只走一步）。
pub fn drive_bots(
    manager: &GameManager,
    room_id: &str,
    max_steps: usize,
    mut persist: impl FnMut(&GameState, &[EngineEvent]) -> i64,
) -> DriveReport {
    let mut report = DriveReport::default();
    for _ in 0..max_steps {
        let Some(state) = manager.get_state(room_id) else {
            report.stop = Some(StopReason::NoState);
            break;
        };
        let Ok(es) = EngineState::from_state(&state) else {
            report.stop = Some(StopReason::NoState);
            break;
        };
        if es.finished() {
            report.finished = true;
            report.stop = Some(StopReason::Finished);
            break;
        }
        let Some(p) = es.kernel.pending.as_ref() else {
            report.stop = Some(StopReason::HumanTurn);
            break;
        };
        let actor = p.actor();
        let kind = p.kind().to_string();
        let Some(seat) = es.kernel.seats.get(actor) else {
            report.stop = Some(StopReason::NoState);
            break;
        };
        if !seat.bot {
            report.stop = Some(StopReason::HumanTurn);
            break;
        }
        let player_id = seat.name.clone();
        let Some(action) = choose(&es.kernel, actor) else {
            report.stop = Some(StopReason::HumanTurn);
            break;
        };
        let score = score_option(&es.kernel, actor, &kind, &action) as f64;
        let digest = state_digest_of(&state).unwrap_or_default();
        let legal = manager.legal_actions(room_id, &player_id).unwrap_or_default();
        let started = std::time::Instant::now();
        match manager.execute_action(room_id, &player_id, &action) {
            Ok(res) => {
                let seq = persist(&res.state, &res.events);
                report.steps += 1;
                report.decisions.push(Decision {
                    actor: player_id,
                    seq,
                    action: action.clone(),
                    score,
                    legal,
                    candidates: Vec::new(),
                    reason: format!("heuristic:{kind}"),
                    state_digest: digest,
                    latency_ms: started.elapsed().as_millis() as i64,
                });
                if res.finished || res.state.status == GameStatus::Finished {
                    report.finished = true;
                    report.stop = Some(StopReason::Finished);
                    break;
                }
            }
            Err(e) => {
                report.illegal += 1;
                report.last_error = Some(format!("{e}"));
                report.errors.push(format!("{e}"));
                report.stop = Some(StopReason::Error);
                break;
            }
        }
    }
    if report.stop.is_none() {
        report.stop = Some(StopReason::MaxSteps);
    }
    report
}

/// 供 server 复用：机器人座位数（全 AI 局判定）。
pub fn bot_seats(state: &GameState) -> usize {
    EngineState::from_state(state)
        .map(|es| es.kernel.seats.iter().filter(|s| s.bot).count())
        .unwrap_or(0)
}
