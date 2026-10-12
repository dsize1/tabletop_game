//! 资源查询 API：dev_task 计划文档 / game 资源（占位）。
//!
//! - `GET /resources/dev_task/{session_id}/plan?token=xxx`：校验 token 后返回 markdown。
//! - `GET /resources/game/{game_id}`：引擎未实现，返回「暂未开放」占位。

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

/// 资源范围标识：dev_task:{sessionId}（与 agent-gateway 签发时一致）。
fn dev_task_resource(session_id: &str) -> String {
    format!("dev_task:{session_id}")
}

/// 计划文档 markdown。安全约束：
/// - sessionId 严格 uuid 校验（白名单，防路径穿越）；
/// - token 必填且有效（伪造/过期 → 401）；
/// - token 资源范围必须等于 `dev_task:{sessionId}`（越权 → 403）。
pub async fn dev_task_plan(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(q): Query<TokenQuery>,
) -> Result<Response, StatusCode> {
    if uuid::Uuid::parse_str(&session_id).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let token = q.token.ok_or(StatusCode::UNAUTHORIZED)?;
    let rec = state.auth.verify(&token).await.map_err(|e| match e {
        auth::AuthError::Invalid => StatusCode::UNAUTHORIZED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    if rec.resource != dev_task_resource(&session_id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let content = read_plan_markdown(&state, &session_id).await?;
    Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], content).into_response())
}

async fn read_plan_markdown(state: &AppState, session_id: &str) -> Result<String, StatusCode> {
    // 优先读 dev_tasks.plan_path（SQLite 真相源）
    if let Ok(Some(task)) = state.storage.get_dev_task(session_id) {
        if let Some(plan_path) = task.plan_path {
            if let Ok(content) = tokio::fs::read_to_string(&plan_path).await {
                return Ok(content);
            }
        }
    }
    // 兜底：PLAN_DIR/{session_id}.md
    let fallback = std::path::PathBuf::from(&state.plan_dir).join(format!("{session_id}.md"));
    tokio::fs::read_to_string(&fallback)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// game 资源占位（引擎未实现）。
pub async fn game_resource(Path(game_id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "not_open",
        "game_id": game_id,
        "message": "暂未开放",
    }))
}
