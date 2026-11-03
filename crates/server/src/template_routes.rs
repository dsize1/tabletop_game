//! 工作台模板 API（`/internal/gameprops/templates`，P5 基座）。
//!
//! 只经 server 读写 SQLite（单写者）；准入与道具中台一致（`require_workbench`）。
//! 模板 = 引擎完整 `GameState` 快照 + 可选 `meta.scene`（场景注入）。

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use game_engine_core::GameState;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::gameprops_routes::{actor, require_workbench, ApiError};
use crate::AppState;

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

fn snapshot_text(v: &Value) -> Result<String, ApiError> {
    match v {
        Value::String(s) => {
            serde_json::from_str::<Value>(s).map_err(|e| {
                err(
                    StatusCode::BAD_REQUEST,
                    format!("snapshot 字符串不是合法 JSON: {e}"),
                )
            })?;
            Ok(s.clone())
        }
        _ => serde_json::to_string(v)
            .map_err(|e| err(StatusCode::BAD_REQUEST, format!("snapshot 序列化失败: {e}"))),
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn parse_game_state(snapshot: &str) -> Result<GameState, ApiError> {
    serde_json::from_str(snapshot)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("snapshot 不是 GameState: {e}")))
}

/// 校验快照可被引擎用于预览；返回解析后的 GameState。
fn validate_snapshot(
    state: &AppState,
    game_id: &str,
    snapshot: &str,
) -> Result<GameState, ApiError> {
    let game_state = parse_game_state(snapshot)?;
    if game_state.game_id != game_id {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("snapshot.game_id={} 与 {game_id} 不一致", game_state.game_id),
        ));
    }
    let engine = state
        .games
        .registry()
        .get(game_id)
        .cloned()
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("未知游戏 {game_id}")))?;
    engine
        .validate_state(&game_state)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("模板不可用: {e}")))?;
    Ok(game_state)
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// `GET /internal/gameprops/templates`
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let game_id = q.game_id.as_deref().filter(|s| !s.is_empty());
    let rows = state
        .storage
        .list_templates(game_id, q.limit.unwrap_or(100), q.offset.unwrap_or(0))
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询模板失败: {e}")))?;
    let total = state
        .storage
        .count_templates(game_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("统计模板失败: {e}")))?;
    // 列表不返回完整 snapshot（可能很大）；详情接口再取。
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "game_id": r.game_id,
                "name": r.name,
                "source": r.source,
                "meta": r.meta,
                "created_by": r.created_by,
                "created_at": r.created_at,
                "updated_at": r.updated_at,
            })
        })
        .collect();
    Ok(Json(json!({ "rows": rows, "total": total })))
}

/// `GET /internal/gameprops/templates/{id}`
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let row = state
        .storage
        .get_template(id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("读取模板失败: {e}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "模板不存在"))?;
    let snapshot: Value = serde_json::from_str(&row.snapshot).unwrap_or(Value::Null);
    let meta: Value = row
        .meta
        .as_deref()
        .and_then(|m| serde_json::from_str(m).ok())
        .unwrap_or(Value::Null);
    Ok(Json(json!({
        "id": row.id,
        "game_id": row.game_id,
        "name": row.name,
        "source": row.source,
        "snapshot": snapshot,
        "meta": meta,
        "created_by": row.created_by,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub game_id: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    pub snapshot: Value,
    #[serde(default)]
    pub meta: Option<Value>,
}

/// `POST /internal/gameprops/templates`
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let snapshot = snapshot_text(&body.snapshot)?;
    validate_snapshot(&state, &body.game_id, &snapshot)?;
    let meta = match &body.meta {
        Some(Value::Null) | None => None,
        Some(v) => Some(serde_json::to_string(v).unwrap_or_default()),
    };
    let source = body.source.as_deref().unwrap_or("manual");
    let created_by = actor(&headers);
    let row = state
        .storage
        .create_template(&storage::templates::TemplateInput {
            game_id: &body.game_id,
            name: &body.name,
            source,
            snapshot: &snapshot,
            meta: meta.as_deref(),
            created_by: Some(created_by.as_str()),
        })
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("创建模板失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub snapshot: Option<Value>,
    /// `null` = 清空 meta；缺省 = 保持不变。
    #[serde(default)]
    pub meta: Option<Value>,
}

/// `PUT /internal/gameprops/templates/{id}`
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let snapshot = match &body.snapshot {
        Some(v) => {
            let text = snapshot_text(v)?;
            let current = state
                .storage
                .get_template(id)
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("读取模板失败: {e}")))?
                .ok_or_else(|| err(StatusCode::NOT_FOUND, "模板不存在"))?;
            validate_snapshot(&state, &current.game_id, &text)?;
            Some(text)
        }
        None => None,
    };
    let (meta_set, meta_value): (bool, Option<String>) = match &body.meta {
        None => (false, None),
        Some(Value::Null) => (true, None),
        Some(v) => (true, Some(serde_json::to_string(v).unwrap_or_default())),
    };
    let meta_arg = if meta_set {
        Some(meta_value.as_deref())
    } else {
        None
    };
    let row = state
        .storage
        .update_template(id, body.name.as_deref(), snapshot.as_deref(), meta_arg)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("更新模板失败: {e}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "模板不存在"))?;
    Ok(Json(json!({ "row": row })))
}

#[derive(Debug, Deserialize)]
pub struct DuplicateBody {
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /internal/gameprops/templates/{id}/duplicate`
pub async fn duplicate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<DuplicateBody>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let row = state
        .storage
        .duplicate_template(id, body.name.as_deref())
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("复制模板失败: {e}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "模板不存在"))?;
    Ok(Json(json!({ "row": row })))
}

/// `DELETE /internal/gameprops/templates/{id}`
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let existed = state
        .storage
        .delete_template(id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("删除模板失败: {e}")))?;
    Ok(Json(json!({ "ok": true, "existed": existed })))
}

#[derive(Debug, Deserialize)]
pub struct FromRoomBody {
    pub room_id: String,
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scene: Option<Value>,
}

/// `POST /internal/gameprops/templates/from-room`：从当前对局快照抓取模板。
pub async fn from_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FromRoomBody>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let room = body.room_id.trim();
    if room.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 room_id"));
    }
    // 优先内存态（最新），没有则读 SQLite 快照（真相源）。
    let mut game_state = state.games.get_state(room);
    if game_state.is_none() {
        let snapshot = state
            .storage
            .get_game_snapshot(body.game_id.as_deref().unwrap_or("sanguosha"), room)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("读取对局失败: {e}")))?;
        if let Some(snapshot) = snapshot {
            game_state = serde_json::from_str(&snapshot).ok();
        }
    }
    let game_state = game_state.ok_or_else(|| err(StatusCode::NOT_FOUND, "对局不存在"))?;
    let snapshot = serde_json::to_string(&game_state)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("序列化失败: {e}")))?;
    let meta = body
        .scene
        .as_ref()
        .filter(|v| !v.is_null())
        .map(|v| json!({ "scene": v }).to_string());
    let name = body
        .name
        .clone()
        .unwrap_or_else(|| format!("对局 {}（{}）", short_id(room), game_state.game_id));
    let created_by = actor(&headers);
    let row = state
        .storage
        .create_template(&storage::templates::TemplateInput {
            game_id: &game_state.game_id,
            name: &name,
            source: &format!("room:{room}"),
            snapshot: &snapshot,
            meta: meta.as_deref(),
            created_by: Some(created_by.as_str()),
        })
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("创建模板失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

#[derive(Debug, Deserialize)]
pub struct FromReplayBody {
    pub room_id: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /internal/gameprops/templates/from-replay`：从回放基线抓取模板。
pub async fn from_replay(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FromReplayBody>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let (game_id, seq, snapshot) = state
        .storage
        .get_replay_base(body.room_id.trim())
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("读取回放基线失败: {e}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "该房间没有回放基线"))?;
    validate_snapshot(&state, &game_id, &snapshot)?;
    let name = body
        .name
        .clone()
        .unwrap_or_else(|| format!("回放 {}@{}", short_id(&body.room_id), seq));
    let created_by = actor(&headers);
    let row = state
        .storage
        .create_template(&storage::templates::TemplateInput {
            game_id: &game_id,
            name: &name,
            source: &format!("replay:{}@{seq}", body.room_id),
            snapshot: &snapshot,
            meta: None,
            created_by: Some(created_by.as_str()),
        })
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("创建模板失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}
