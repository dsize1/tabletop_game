//! Web 动作通路（P3）：对局动作的**唯一 Web 执行入口**（仅本机链路）。
//!
//! 链路：浏览器 → web（短 token 校验）→ `POST /internal/game/{game_id}/{room_id}/action` → 引擎执行 → 快照/事件落库。
//!
//! 约束（`docs/architecture.md` / 计划台账 P3）：
//! - 仅本机：server 绑定 127.0.0.1，本路由无独立鉴权（与 `/internal/*` 既有约定一致）；
//!   token 与越权校验在 web 层完成（`resource == game:{room_id}`）。
//! - **单写者**：P3–P6 期间三国杀动作只由 Web/server 提交；MCP 仅用于建局/种子局（不并发）。
//!   `game_state` 无 CAS（last-write-wins），P7 接入 Q 时必须统一到本入口。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use game_engine_core::{Action, EngineError};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::resources_routes::ensure_room;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ActionRequest {
    pub player_id: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub ok: bool,
    pub message: String,
    pub finished: bool,
    /// 事件写入后的快照 version（对齐最后一条 `game_events.seq`）。
    pub version: i64,
    /// 事件落库是否成功（失败仅告警，不回滚内存状态，与 MCP 行为一致）。
    pub persisted: bool,
    /// 本次请求内机器人自动推进的动作数（P6.3）。
    pub bot_steps: usize,
    /// 机器人推进的停止原因（`human_turn` / `finished` / `max_steps` / ...）。
    pub bot_stop: Option<String>,
    /// 机器人动作被拒后回落的明细（排障用；正常为空）。
    pub bot_errors: Vec<String>,
}

fn map_engine_error(err: &EngineError) -> StatusCode {
    match err {
        EngineError::NotFound(_) => StatusCode::NOT_FOUND,
        EngineError::PermissionDenied(_) => StatusCode::FORBIDDEN,
        EngineError::IllegalAction(_) => StatusCode::BAD_REQUEST,
        EngineError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 执行一个动作：引擎无条件重校验 → 写 `game_events` + 快照。
pub async fn execute_action(
    State(state): State<AppState>,
    Path((game_id, room_id)): Path<(String, String)>,
    Json(req): Json<ActionRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    if req.player_id.trim().is_empty() || req.action.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    ensure_room(&state, &game_id, &room_id)?;
    capture_replay_base(&state, &game_id, &room_id);

    // 人类决策轨迹：动作前的合法集/状态摘要（P6.4）。
    let started = Instant::now();
    let (legal, digest) = decision_context(&state, &game_id, &room_id, &req.player_id);
    let result = state
        .games
        .execute_action(&room_id, &req.player_id, &req.action)
        .map_err(|e| map_engine_error(&e))?;

    let persisted = persist(&state, &result.state, &result.events);
    let (version, persisted_ok) = match &persisted {
        Ok(seq) => (*seq, true),
        Err(e) => {
            tracing::warn!("写 game_events 失败: {e:#}");
            (-1, false)
        }
    };
    log_decision(
        &state,
        DecisionLogInput {
            game_id: &game_id,
            room_id: &room_id,
            seq: version,
            player_id: &req.player_id,
            source: "human",
            legal: &legal,
            candidates: None,
            chosen: Some(&req.action),
            score: None,
            reason: Some("human_action"),
            state_digest: digest.as_deref(),
            latency_ms: Some(started.elapsed().as_millis() as i64),
        },
    );

    // P6.3：机器人自动推进（本请求内把机器人回合跑完；人类回合即停）。
    // 只在 server 推进（单写者），并复用 AI 的兜底/破环器（`sanguosha::ai::drive_bots`）。
    let drive = drive_bots(&state, &room_id);

    Ok(Json(ActionResponse {
        ok: true,
        message: result.message,
        finished: result.finished || drive.finished,
        version,
        persisted: persisted_ok,
        bot_steps: drive.steps,
        bot_stop: drive.stop.map(|s| s.as_str().to_string()),
        bot_errors: drive.errors,
    }))
}

#[derive(Debug, Serialize)]
pub struct DriveResponse {
    pub ok: bool,
    pub bot_steps: usize,
    pub bot_stop: Option<String>,
    pub finished: bool,
    pub illegal: usize,
    /// 被引擎拒绝的动作明细（已回落；排障用）。
    pub bot_errors: Vec<String>,
}

/// `POST /internal/game/{game_id}/{room_id}/drive`：手动触发一次机器人推进。
///
/// 用途（P6.3）：
/// - 并发保护下**跳过**了推进（另一请求正在推进）时的兜底；
/// - server 重启/快照恢复后，如果停在机器人回合，可用它续推；
/// - e2e/脚本可确定性地把机器人回合跑完（不依赖"恰好有人提交动作"）。
pub async fn drive(
    State(state): State<AppState>,
    Path((game_id, room_id)): Path<(String, String)>,
) -> Result<Json<DriveResponse>, StatusCode> {
    ensure_room(&state, &game_id, &room_id)?;
    let report = drive_bots(&state, &room_id);
    Ok(Json(DriveResponse {
        ok: true,
        bot_steps: report.steps,
        bot_stop: report.stop.map(|s| s.as_str().to_string()),
        finished: report.finished,
        illegal: report.illegal,
        bot_errors: report.errors,
    }))
}

/// 决策日志输入（server 内部；字段与 `storage::DecisionLogRecord` 对齐）。
struct DecisionLogInput<'a> {
    game_id: &'a str,
    room_id: &'a str,
    seq: i64,
    player_id: &'a str,
    source: &'a str,
    legal: &'a [Action],
    candidates: Option<&'a [Action]>,
    chosen: Option<&'a str>,
    score: Option<f64>,
    reason: Option<&'a str>,
    state_digest: Option<&'a str>,
    latency_ms: Option<i64>,
}

/// `SGS_DECISION_LOG_RATE`：0 关闭，1 全量，中间按概率采样。默认 1。
fn decision_log_rate() -> f64 {
    std::env::var("SGS_DECISION_LOG_RATE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

fn should_log_decision() -> bool {
    let rate = decision_log_rate();
    if rate >= 1.0 {
        return true;
    }
    if rate <= 0.0 {
        return false;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 10_000) as f64 / 10_000.0 < rate
}

fn json_actions(actions: &[Action]) -> String {
    serde_json::to_string(actions).unwrap_or_else(|_| "[]".to_string())
}

/// 写一条决策日志；失败只告警，绝不阻塞对局。
fn log_decision(state: &AppState, d: DecisionLogInput<'_>) {
    if d.game_id != "sanguosha" || !should_log_decision() {
        return;
    }
    let legal = json_actions(d.legal);
    let candidates = d.candidates.map(json_actions);
    let record = storage::DecisionLogRecord {
        game_id: d.game_id,
        room_id: d.room_id,
        seq: d.seq,
        player_id: d.player_id,
        source: d.source,
        legal_actions: &legal,
        candidates: candidates.as_deref(),
        chosen: d.chosen,
        confidence: d.score,
        latency_ms: d.latency_ms,
        reason: d.reason,
        state_digest: d.state_digest,
    };
    if let Err(e) = state.storage.append_decision_log(&record) {
        tracing::warn!("写 decision log 失败（已忽略）: {e:#}");
    }
}

/// 动作前的合法集 + 状态摘要（人类动作日志用）；非三国杀返回空。
fn decision_context(
    state: &AppState,
    game_id: &str,
    room_id: &str,
    player_id: &str,
) -> (Vec<Action>, Option<String>) {
    if game_id != "sanguosha" {
        return (Vec::new(), None);
    }
    let legal = state
        .games
        .legal_actions(room_id, player_id)
        .unwrap_or_default();
    let digest = state.games.get_state(room_id).and_then(|s| {
        sanguosha::SgsState::parse(&s.data)
            .ok()
            .map(|st| sanguosha::ai::state_digest(&st))
    });
    (legal, digest)
}

/// 首个决策前写回放基线（同房间只写一份）。
fn capture_replay_base(state: &AppState, game_id: &str, room_id: &str) {
    if game_id != "sanguosha" {
        return;
    }
    let Some(gs) = state.games.get_state(room_id) else {
        return;
    };
    let Ok(state_json) = serde_json::to_string(&gs) else {
        return;
    };
    let seq = state
        .storage
        .game_snapshot_version(game_id, room_id)
        .ok()
        .flatten()
        .unwrap_or(0);
    if let Err(e) = state
        .storage
        .ensure_replay_base(game_id, room_id, seq, &state_json)
    {
        tracing::warn!("写 replay base 失败（已忽略）: {e:#}");
    }
}

/// 后台续推轮数：单请求到 `DRIVE_MAX_STEPS` 后仍在机器人回合时，继续在后台推进；
/// 遇到人类回合/终局自然停止（P6.3 真机：玩家阵亡后剩余全 AI 会停在「bot:3 行动中…」）。
/// 15 × 200 = 3000 步/局（与计划台账的 3000 步告警口径一致）。
const DRIVE_CHAIN_ROUNDS: usize = 15;

/// 把机器人回合推进到人类回合/终局（最多 `DRIVE_MAX_STEPS` 步）。
///
/// - 只推进 `seat.is_bot` 的座位；人类回合立刻停（正常停点）；
/// - 每步动作都按同一链路落 `game_events` + 快照（与人类动作一致）；
/// - 拿到房间锁才推进；已有推进在跑则跳过（并发保护，页面轮询兜底）；
/// - 到达单请求上限且仍在机器人回合 → 后台链式续推（否则全 AI 局会永久停在 bot 回合）。
fn drive_bots(state: &AppState, room_id: &str) -> sanguosha::ai::DriveReport {
    let report = drive_bots_once(state, room_id);
    if report.stop == Some(sanguosha::ai::StopReason::MaxSteps) && !report.finished {
        let state = state.clone();
        let room_id = room_id.to_string();
        tokio::task::spawn_blocking(move || {
            for _ in 0..DRIVE_CHAIN_ROUNDS {
                let next = drive_bots_once(&state, &room_id);
                if next.finished || next.stop != Some(sanguosha::ai::StopReason::MaxSteps) {
                    break;
                }
            }
        });
    }
    report
}

fn drive_bots_once(state: &AppState, room_id: &str) -> sanguosha::ai::DriveReport {
    let Some(_guard) = state.bot_drive.try_acquire(room_id) else {
        tracing::debug!("房间 {room_id} 已有机器人推进在跑，跳过本次");
        return sanguosha::ai::DriveReport::default();
    };
    // 机器人回合开始前写回放基线（若尚无）。
    if let Some(gs) = state.games.get_state(room_id) {
        capture_replay_base(state, &gs.game_id, room_id);
    }
    let report = sanguosha::ai::drive_bots(
        &state.games,
        room_id,
        sanguosha::ai::DRIVE_MAX_STEPS,
        |game_state, events| match persist(state, game_state, events) {
            Ok(seq) => seq,
            Err(e) => {
                tracing::warn!("机器人动作落库失败: {e:#}");
                -1
            }
        },
    );
    // 机器人决策日志（与成功动作一一对应；seq 已在 driver 内对齐）。
    for d in &report.decisions {
        log_decision(
            state,
            DecisionLogInput {
                game_id: "sanguosha",
                room_id,
                seq: d.seq,
                player_id: &d.actor,
                source: "bot",
                legal: &d.legal,
                candidates: Some(&d.candidates),
                chosen: Some(&d.chosen),
                score: Some(d.score),
                reason: Some(&d.reason),
                state_digest: Some(&d.state_digest),
                latency_ms: Some(d.latency_ms),
            },
        );
    }
    if report.steps > 0 || report.last_error.is_some() {
        tracing::info!(
            "机器人推进 room={room_id} steps={} stop={:?} illegal={} loops={} finished={}",
            report.steps,
            report.stop,
            report.illegal,
            report.loops,
            report.finished
        );
    }
    for e in &report.errors {
        tracing::warn!("机器人动作被拒（已回落）: {e}");
    }
    if let Some(err) = &report.last_error {
        tracing::warn!("机器人推进中止: {err}");
    }
    report
}

/// 把引擎产出的事件逐条写入 `game_events`，快照 version 对齐最后 seq（与 MCP 侧一致）。
fn persist(
    state: &AppState,
    game_state: &game_engine_core::GameState,
    events: &[game_engine_core::EngineEvent],
) -> anyhow::Result<i64> {
    let state_json = serde_json::to_string(game_state)?;
    let event_ids: Vec<String> = events.iter().map(|_| shared::new_event_id()).collect();
    let payloads: Vec<String> = events
        .iter()
        .map(|e| serde_json::to_string(&e.payload).unwrap_or_else(|_| "null".into()))
        .collect();
    let records: Vec<storage::GameEventRecord<'_>> = events
        .iter()
        .enumerate()
        .map(|(i, e)| storage::GameEventRecord {
            event_id: &event_ids[i],
            event_type: &e.event_type,
            actor: e.actor.as_deref(),
            payload: &payloads[i],
        })
        .collect();
    let seq = state.storage.append_game_events(
        &game_state.game_id,
        &game_state.room_id,
        &records,
        &state_json,
    )?;
    // 记录本进程已知版本：避免下一次 `ensure_room` 把自己的写入当成「落后」而多读一次快照。
    if let Ok(mut m) = state.room_versions.lock() {
        m.insert(game_state.room_id.clone(), seq);
    }
    tracing::debug!("已写入 {} 条引擎事件，version={seq}", records.len());
    Ok(seq)
}
