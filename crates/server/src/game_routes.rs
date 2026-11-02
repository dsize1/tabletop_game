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
use game_engine_core::EngineError;
use serde::{Deserialize, Serialize};

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
    let report = sanguosha::ai::drive_bots(
        &state.games,
        room_id,
        sanguosha::ai::DRIVE_MAX_STEPS,
        |game_state, events| {
            if let Err(e) = persist(state, game_state, events) {
                tracing::warn!("机器人动作落库失败: {e:#}");
            }
        },
    );
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
