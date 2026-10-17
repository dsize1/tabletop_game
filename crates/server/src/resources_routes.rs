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

/// game 资源：返回对局的**公共视图** + **玩家个人视图**（引擎脱敏）。
/// 安全约束：
/// - token 必填且有效（401）；
/// - token 资源范围必须为 `game:{room_id}`（越权 → 403），room_id 由资源范围解析；
/// - 对局快照不存在 → 404。
pub async fn game_resource(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    Query(q): Query<TokenQuery>,
) -> Result<Response, StatusCode> {
    let token = q.token.ok_or(StatusCode::UNAUTHORIZED)?;
    let rec = state.auth.verify(&token).await.map_err(|e| match e {
        auth::AuthError::Invalid => StatusCode::UNAUTHORIZED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    let room_id = rec
        .resource
        .strip_prefix("game:")
        .ok_or(StatusCode::FORBIDDEN)?
        .to_string();

    let snapshot = state
        .storage
        .get_game_snapshot(&game_id, &room_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let game_state: game_engine_core::GameState =
        serde_json::from_str(&snapshot).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 群昵称：agent-gateway 在每次 @消息 时写入 Redis hash `names:{group_id}`。
    let names = load_names(&state.redis_url, &game_state.group_id).await;

    // 引擎脱敏（目前仅 las-vegas；其它引擎直接返回原状态）。
    use game_engine_core::EnginePlugin;
    let engine = las_vegas::LasVegasEngine;
    let engine: &dyn EnginePlugin = &engine;
    let (public, player) = if game_state.game_id == engine.game_id() {
        (
            serde_json::to_value(engine.public_state(&game_state)).unwrap_or_default(),
            serde_json::to_value(engine.player_view(&game_state, &rec.openid)).unwrap_or_default(),
        )
    } else {
        let v = serde_json::to_value(&game_state).unwrap_or_default();
        (v.clone(), v)
    };

    Ok(Json(serde_json::json!({
        "game_id": game_id,
        "room_id": room_id,
        "openid": rec.openid,
        "public": public,
        "player": player,
        "names": names,
    }))
    .into_response())
}

/// 读取群昵称映射（openid → nickname）；Redis 不可用/无数据时返回空。
async fn load_names(
    redis_url: &str,
    group_id: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    let Ok(client) = redis::Client::open(redis_url) else {
        return out;
    };
    let Ok(mut conn) = redis::aio::ConnectionManager::new(client).await else {
        return out;
    };
    let key = format!("names:{group_id}");
    let map: redis::RedisResult<std::collections::HashMap<String, String>> =
        redis::cmd("HGETALL").arg(key).query_async(&mut conn).await;
    if let Ok(m) = map {
        for (k, v) in m {
            out.insert(k, serde_json::Value::String(v));
        }
    }
    out
}
