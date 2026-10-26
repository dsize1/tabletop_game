//! 道具中台内部 API（P1）：/internal/gameprops/*。
//!
//! 单写者：只经 server 写 SQLite；SPA 不直连库。
//! 鉴权：`PROPS_ADMIN_TOKEN` 存在时要求 `X-Props-Admin-Token`；未设置时仅限本机
//! （server 只监听 127.0.0.1）。完整的游戏权鉴在 P8。

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

type ApiError = (StatusCode, Json<Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if let Some(expected) = state.props_admin_token.as_deref().filter(|s| !s.is_empty()) {
        let got = headers
            .get("x-props-admin-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if got != expected {
            return Err(err(StatusCode::UNAUTHORIZED, "缺少或错误的道具中台 token"));
        }
    }
    Ok(())
}

fn actor(headers: &HeaderMap) -> String {
    headers
        .get("x-props-actor")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("admin")
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub table: String,
    pub entity_code: String,
    pub to_status: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// `GET /internal/gameprops/summary`
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    state
        .storage
        .gameprops_summary()
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("汇总失败: {e}")))
}

/// `GET /internal/gameprops/{table}?status=&q=&limit=&offset=`
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(table): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let rows = state
        .storage
        .list_gameprops(
            &table,
            q.status.as_deref(),
            q.q.as_deref(),
            q.limit.unwrap_or(100),
            q.offset.unwrap_or(0),
        )
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?;
    Ok(Json(json!({ "table": table, "rows": rows })))
}

/// `POST /internal/gameprops/{table}`：upsert 一行（JSON 对象）。
pub async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(table): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let obj = body
        .as_object()
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "请求体必须是 JSON 对象"))?;
    let row = state
        .storage
        .upsert_gameprops(&table, obj)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("写入失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

/// `POST /internal/gameprops/status`：生命周期流转 + 审核日志。
pub async fn set_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<StatusBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    const STATUSES: &[&str] = &["draft", "testing", "review", "live", "offline"];
    if !STATUSES.contains(&body.to_status.as_str()) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("非法状态 {}", body.to_status),
        ));
    }
    let row = state
        .storage
        .set_gameprops_status(
            &body.table,
            &body.entity_code,
            &body.to_status,
            &actor(&headers),
            body.note.as_deref(),
        )
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("状态流转失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

/// `POST /internal/gameprops/seed`：初始化身份/配比/包（幂等）。
pub async fn seed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let result = state.storage.seed_sgs_defaults().map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("初始化失败: {e}"),
        )
    })?;
    Ok(Json(json!({ "seeded": result })))
}

/// `POST /internal/gameprops/import/heroes`：读取 heroes.json 并幂等导入。
pub async fn import_heroes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let path = state.props_data_dir.join("heroes.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取 {} 失败: {e}", path.display()),
        )
    })?;
    let payload: Value = serde_json::from_str(&text).map_err(|e| {
        err(
            StatusCode::BAD_REQUEST,
            format!("heroes.json 解析失败: {e}"),
        )
    })?;
    let result = state
        .storage
        .import_sgs_heroes(&payload)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("导入失败: {e}")))?;
    Ok(Json(
        json!({ "imported": result, "source": path.display().to_string() }),
    ))
}

/// 兼容：`GET /internal/gameprops/{table}/{code}` 单行查询（便于关系页）。
pub async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((table, code)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let rows = state
        .storage
        .list_gameprops(&table, None, Some(&code), 1, 0)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?;
    let row = rows.into_iter().next().unwrap_or(Value::Null);
    Ok(Json(json!({ "row": row })))
}
