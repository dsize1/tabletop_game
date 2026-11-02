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

/// 把快照按需恢复到内存（server 是 Web 动作宿主；快照是真相源）。
///
/// - 内存里已有该房间 → 直接返回；
/// - 否则从 `game_state` 读快照并 `restore`（支持 MCP 先建局、server 后接管）；
/// - 快照不存在或 `game_id` 不匹配 → 404。

/// 是否需要从 SQLite 重新载入内存态（纯函数，便于单测）。
///
/// 规则：内存里没有 → 要载入；DB 没有版本 → 保持现状（不存在/异常）；否则
/// 「DB 版本 > 本进程已知版本（或从未记录）」→ 重新载入。
pub(crate) fn needs_reload(in_memory: bool, db_version: Option<i64>, tracked: Option<i64>) -> bool {
    if !in_memory {
        return true;
    }
    match db_version {
        None => false,
        Some(db) => tracked.map_or(true, |t| db > t),
    }
}

pub(crate) fn ensure_room(
    state: &AppState,
    game_id: &str,
    room_id: &str,
) -> Result<(), StatusCode> {
    // 快照版本：DB 是真相源。MCP 仍会写 lobby（`#加机器人`/`#武将包`/`#开始`），
    // 因此「内存里有」不等于「内存是新的」——版本落后就必须重新 restore，
    // 否则页面会停在旧阶段（P6.3 真机：`#开始` 后仍卡在 lobby/选将）。
    let db_version = state
        .storage
        .game_snapshot_version(game_id, room_id)
        .ok()
        .flatten();
    let tracked = state
        .room_versions
        .lock()
        .ok()
        .and_then(|m| m.get(room_id).copied());
    if !needs_reload(state.games.get_state(room_id).is_some(), db_version, tracked) {
        return Ok(());
    }

    let snapshot = state
        .storage
        .get_game_snapshot(game_id, room_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let snapshot: game_engine_core::GameState =
        serde_json::from_str(&snapshot).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if snapshot.game_id != game_id {
        return Err(StatusCode::NOT_FOUND);
    }
    state.games.restore(snapshot);
    if let Some(v) = db_version {
        if let Ok(mut m) = state.room_versions.lock() {
            m.insert(room_id.to_string(), v);
        }
    }
    Ok(())
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

    if game_state.game_id != game_id {
        return Err(StatusCode::NOT_FOUND);
    }

    // 群昵称：agent-gateway 在每次 @消息 时写入 Redis hash `names:{group_id}`。
    let names = load_names(&state.redis_url, &game_state.group_id).await;

    // 引擎投影（P3）：按 game_id 分派到引擎 `public_state` / `player_view`。
    // 注意：这里**不是**权限层（P8），而是引擎视图契约——
    // 未注册的引擎不能把内部状态原样下发（会泄漏手牌/身份/牌堆顺序）。
    ensure_room(&state, &game_id, &room_id)?;
    let public = state
        .games
        .public_state(&room_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let player = state
        .games
        .player_view(&room_id, &rec.openid)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (public, player) = (
        serde_json::to_value(public).unwrap_or_default(),
        serde_json::to_value(player).unwrap_or_default(),
    );
    // 请求者本人的合法动作（引擎按权限过滤：非其回合/非其 pending 时为空）。
    let actions = state
        .games
        .legal_actions(&room_id, &rec.openid)
        .map(|list| {
            list.into_iter()
                .map(|a| serde_json::json!({ "id": a.id, "description": a.description }))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "game_id": game_id,
        "room_id": room_id,
        "openid": rec.openid,
        "public": public,
        "player": player,
        "actions": actions,
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

#[cfg(test)]
mod tests {
    use super::needs_reload;

    #[test]
    fn needs_reload_rules() {
        // 内存里没有 → 必须载入
        assert!(needs_reload(false, Some(7), None));
        assert!(needs_reload(false, None, None));
        // 内存里有、DB 版本更新（MCP 写了 lobby）→ 重新载入
        assert!(needs_reload(true, Some(8), Some(7)));
        // 本进程从未记录过版本 → 保守重载一次
        assert!(needs_reload(true, Some(7), None));
        // 版本没变 / 自己刚写过 → 不动
        assert!(!needs_reload(true, Some(7), Some(7)));
        assert!(!needs_reload(true, Some(6), Some(7)));
        // DB 无版本记录 → 不动
        assert!(!needs_reload(true, None, Some(7)));
    }
}
