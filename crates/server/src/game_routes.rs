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

    Ok(Json(ActionResponse {
        ok: true,
        message: result.message,
        finished: result.finished,
        version,
        persisted: persisted_ok,
    }))
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
    tracing::debug!("已写入 {} 条引擎事件，version={seq}", records.len());
    Ok(seq)
}
