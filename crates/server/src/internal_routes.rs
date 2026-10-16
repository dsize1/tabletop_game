//! 内部 API（agent-gateway → server）：对话窗口读取 + 全局人物画像读写。
//!
//! 用途：
//! - 画像蒸馏（凌晨批）读窗口 / 活跃 openid，写回画像；
//! - chat/game 组装时读画像作全局背景。
//!
//! 与 `/auth/issue` 同类：**仅监听本机、无 token 鉴权**（属内网链路）。
//! 保持「SQLite 单写者 = server」：agent-gateway 不直连库，统一经此 API。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct WindowQuery {
    pub from: String,
    pub to: String,
}

/// `GET /internal/chat/window?from=&to=` → 窗口内对话（按 openid、时间升序）。
pub async fn chat_window(
    State(state): State<AppState>,
    Query(q): Query<WindowQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let rows = state
        .storage
        .get_chat_window(&q.from, &q.to)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    serde_json::to_value(rows)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /internal/chat/active_openids?from=&to=` → 窗口内出现过的 openid（去重升序）。
pub async fn active_openids(
    State(state): State<AppState>,
    Query(q): Query<WindowQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ids = state
        .storage
        .list_active_openids(&q.from, &q.to)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    serde_json::to_value(ids)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /internal/profile/{openid}` → 画像（无 → 404）。
pub async fn get_profile(
    State(state): State<AppState>,
    Path(openid): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.storage.get_profile(&openid) {
        Ok(Some(p)) => serde_json::to_value(p)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProfileUpsert {
    pub openid: String,
    pub profile_text: String,
    /// 上次蒸馏时刻（增量游标，ISO8601）；由调用方给出
    pub distilled_at: Option<String>,
    /// 写入时刻（ISO8601）；由调用方给出
    pub updated_at: String,
}

/// `POST /internal/profile` → upsert 画像。
pub async fn upsert_profile(
    State(state): State<AppState>,
    Json(body): Json<ProfileUpsert>,
) -> Result<StatusCode, StatusCode> {
    if body.openid.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .storage
        .upsert_profile(
            &body.openid,
            &body.profile_text,
            body.distilled_at.as_deref(),
            &body.updated_at,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
