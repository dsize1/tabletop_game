//! 道具中台素材（P1 入中台）：内容寻址存储 + 上传/缩略图/绑定/映射。
//!
//! 设计（ADR-0036）：
//! - 磁盘路径只由 `sha256 + mime 扩展名` 生成，**绝不使用客户端文件名**；
//! - server 独占写 `GAMEPROPS_ASSETS_DIR`（默认 `data/gameprops/assets`，已 gitignore）；
//! - web 以 `/game-assets` 静态挂载同一目录（文件名哈希 → immutable），浏览器不直连 server；
//! - 绑定只写业务表 `image_asset_id`，不复制行。

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path as FsPath, PathBuf};

use crate::gameprops_routes::{require_admin, ApiError};
use crate::AppState;

const MAX_FULL_BYTES: usize = 8 * 1024 * 1024;
const MAX_THUMB_BYTES: usize = 1024 * 1024;
const MAX_DIM: i64 = 4096;

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// 内容类型 → (规范化 mime, 扩展名)；只允许 webp/png/jpeg。
fn mime_ext(headers: &HeaderMap) -> Result<(&'static str, &'static str), ApiError> {
    let raw = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match raw.as_str() {
        "image/webp" => Ok(("image/webp", "webp")),
        "image/png" => Ok(("image/png", "png")),
        "image/jpeg" | "image/jpg" => Ok(("image/jpeg", "jpg")),
        _ => Err(err(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "仅支持 image/webp、image/png、image/jpeg",
        )),
    }
}

fn check_dim(w: i64, h: i64) -> Result<(), ApiError> {
    if !(1..=MAX_DIM).contains(&w) || !(1..=MAX_DIM).contains(&h) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("尺寸需在 1–{MAX_DIM} 之间"),
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 客户端原始文件名：仅审计，不参与路径；去目录、去控制字符、截断。
fn sanitize_original_name(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect::<String>();
    if base.is_empty() {
        None
    } else {
        Some(base)
    }
}

fn validate_crop(crop: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(crop) = crop.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if crop.len() > 1024 {
        return Err(err(StatusCode::BAD_REQUEST, "crop 过大"));
    }
    let v: Value =
        serde_json::from_str(crop).map_err(|_| err(StatusCode::BAD_REQUEST, "crop 必须是 JSON"))?;
    if !v.is_object() {
        return Err(err(StatusCode::BAD_REQUEST, "crop 必须是 JSON 对象"));
    }
    Ok(Some(crop.to_string()))
}

/// 相对路径 → 磁盘路径（防御 `..` 与绝对路径）。
fn resolve_path(root: &FsPath, rel: &str) -> Result<PathBuf, ApiError> {
    let rel_path = FsPath::new(rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(err(StatusCode::BAD_REQUEST, "非法素材路径"));
    }
    Ok(root.join(rel_path))
}

fn asset_url(path: &str) -> String {
    format!("/game-assets/{path}")
}

fn with_urls(mut row: Value) -> Value {
    let path = row
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let thumb = row
        .get("thumb_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(obj) = row.as_object_mut() {
        obj.insert("url".into(), json!(asset_url(&path)));
        obj.insert(
            "thumb_url".into(),
            json!(thumb
                .map(|t| asset_url(&t))
                .unwrap_or_else(|| asset_url(&path))),
        );
    }
    row
}

async fn write_file(root: &FsPath, rel: &str, bytes: &[u8]) -> Result<(), ApiError> {
    let abs = resolve_path(root, rel)?;
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("建目录失败: {e}"),
            )
        })?;
    }
    if tokio::fs::try_exists(&abs).await.unwrap_or(false) {
        return Ok(());
    }
    let tmp = abs.with_extension(format!(
        "{}.tmp-{}",
        abs.extension().and_then(|s| s.to_str()).unwrap_or("bin"),
        std::process::id()
    ));
    tokio::fs::write(&tmp, bytes).await.map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("写文件失败: {e}"),
        )
    })?;
    tokio::fs::rename(&tmp, &abs)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("落盘失败: {e}")))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub game_id: String,
    pub kind: String,
    pub width: i64,
    pub height: i64,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub crop: Option<String>,
}

/// `POST /internal/gameprops/assets/upload`：raw body 上传原图（内容寻址）。
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<UploadQuery>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !valid_slug(&q.game_id) {
        return Err(err(StatusCode::BAD_REQUEST, "非法 game_id"));
    }
    if !valid_slug(&q.kind) {
        return Err(err(StatusCode::BAD_REQUEST, "非法 kind"));
    }
    check_dim(q.width, q.height)?;
    if body.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "空文件"));
    }
    if body.len() > MAX_FULL_BYTES {
        return Err(err(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("原图超过 {}MB", MAX_FULL_BYTES / 1024 / 1024),
        ));
    }
    let (mime, ext) = mime_ext(&headers)?;
    let hash = sha256_hex(&body);
    let rel = format!("{}/{}/{}.{}", q.game_id, q.kind, hash, ext);
    write_file(&state.props_assets_dir, &rel, &body).await?;

    let mut row = Map::new();
    row.insert("game_id".into(), json!(q.game_id));
    row.insert("kind".into(), json!(q.kind));
    row.insert("path".into(), json!(rel));
    row.insert("mime".into(), json!(mime));
    row.insert("width".into(), json!(q.width));
    row.insert("height".into(), json!(q.height));
    row.insert("bytes".into(), json!(body.len() as i64));
    row.insert("sha256".into(), json!(hash));
    if let Some(name) = sanitize_original_name(q.filename.as_deref()) {
        row.insert("original_name".into(), json!(name));
    }
    if let Some(crop) = validate_crop(q.crop.as_deref())? {
        row.insert("crop_json".into(), json!(crop));
    }
    let saved = state
        .storage
        .upsert_gameprops("prop_assets", &row)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("写入素材失败: {e}")))?;
    Ok(Json(json!({ "asset": with_urls(saved) })))
}

#[derive(Debug, Deserialize)]
pub struct ThumbQuery {
    pub width: i64,
    pub height: i64,
}

/// `POST /internal/gameprops/assets/{id}/thumb`：raw body 上传缩略图。
pub async fn upload_thumb(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<ThumbQuery>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    check_dim(q.width, q.height)?;
    if body.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "空文件"));
    }
    if body.len() > MAX_THUMB_BYTES {
        return Err(err(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("缩略图超过 {}KB", MAX_THUMB_BYTES / 1024),
        ));
    }
    let asset = state
        .storage
        .get_prop_asset(id)
        .map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("读取素材失败: {e}"),
            )
        })?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("素材 {id} 不存在")))?;
    let game_id = asset
        .get("game_id")
        .and_then(Value::as_str)
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "素材缺少 game_id"))?;
    let kind = asset
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "素材缺少 kind"))?;
    let base_sha = asset
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("asset-{id}"));
    let (_, ext) = mime_ext(&headers)?;
    let rel = format!("{game_id}/{kind}/{base_sha}.t.{ext}");
    write_file(&state.props_assets_dir, &rel, &body).await?;
    let saved = state
        .storage
        .set_prop_asset_thumb(id, &rel, q.width, q.height)
        .map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("写入缩略图失败: {e}"),
            )
        })?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("素材 {id} 不存在")))?;
    Ok(Json(json!({ "asset": with_urls(saved) })))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// `GET /internal/gameprops/assets`：素材列表（kind 筛选 + 搜索）。
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let game_id = q.game_id.as_deref().unwrap_or("sanguosha");
    if !valid_slug(game_id) {
        return Err(err(StatusCode::BAD_REQUEST, "非法 game_id"));
    }
    let rows = state
        .storage
        .list_prop_assets(
            game_id,
            q.kind.as_deref(),
            q.q.as_deref(),
            q.limit.unwrap_or(200),
            q.offset.unwrap_or(0),
        )
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?;
    Ok(Json(json!({
        "game_id": game_id,
        "rows": rows.into_iter().map(with_urls).collect::<Vec<_>>(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct MapQuery {
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

/// `GET /internal/gameprops/asset-map`：业务 code → 素材 URL（web SSR 用）。
pub async fn asset_map(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<MapQuery>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let game_id = q.game_id.as_deref().unwrap_or("sanguosha");
    if !valid_slug(game_id) {
        return Err(err(StatusCode::BAD_REQUEST, "非法 game_id"));
    }
    let kind = q.kind.as_deref().unwrap_or("general");
    let rows = match kind {
        "general" => state
            .storage
            .general_asset_map(game_id)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?,
        _ => Vec::new(),
    };
    let mut map = Map::new();
    for row in rows {
        let Some(code) = row.get("code").and_then(Value::as_str) else {
            continue;
        };
        let Some(path) = row.get("path").and_then(Value::as_str) else {
            continue;
        };
        let thumb = row
            .get("thumb_path")
            .and_then(Value::as_str)
            .unwrap_or(path);
        map.insert(
            code.to_string(),
            json!({
                "url": asset_url(path),
                "thumb": asset_url(thumb),
                "width": row.get("width").cloned().unwrap_or(Value::Null),
                "height": row.get("height").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    Ok(Json(
        json!({ "game_id": game_id, "kind": kind, "assets": map }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct BindBody {
    pub table: String,
    pub code: Option<String>,
    pub id: Option<i64>,
    #[serde(default)]
    pub asset_id: Option<i64>,
}

/// `POST /internal/gameprops/bind-asset`：绑定/解绑业务行与素材。
pub async fn bind(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<BindBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let key = if body.table == "sgs_cards" {
        body.id
            .map(|v| v.to_string())
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "sgs_cards 需要 id"))?
    } else {
        body.code
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "缺少 code"))?
    };
    let row = state
        .storage
        .bind_prop_asset(&body.table, &key, body.asset_id)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("绑定失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

/// `DELETE /internal/gameprops/assets/{id}`：删除素材（被引用时拒绝）。
pub async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let refs = state.storage.count_prop_asset_refs(id).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("检查引用失败: {e}"),
        )
    })?;
    if refs > 0 {
        return Err(err(
            StatusCode::CONFLICT,
            format!("素材仍被 {refs} 条记录引用，先解绑再删除"),
        ));
    }
    let row = state
        .storage
        .delete_prop_asset(id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("删除失败: {e}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("素材 {id} 不存在")))?;
    for key in ["path", "thumb_path"] {
        if let Some(rel) = row.get(key).and_then(Value::as_str) {
            if let Ok(abs) = resolve_path(&state.props_assets_dir, rel) {
                let _ = tokio::fs::remove_file(abs).await;
            }
        }
    }
    Ok(Json(json!({ "deleted": true, "id": id })))
}

/// 素材路由（挂在 `/internal/gameprops` 之外，避免与 `{table}` 动态段混淆）。
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/internal/gameprops/assets/upload",
            post(upload).layer(DefaultBodyLimit::max(MAX_FULL_BYTES + 64 * 1024)),
        )
        .route(
            "/internal/gameprops/assets/{id}/thumb",
            post(upload_thumb).layer(DefaultBodyLimit::max(MAX_THUMB_BYTES + 64 * 1024)),
        )
        .route("/internal/gameprops/assets", get(list))
        .route("/internal/gameprops/asset-map", get(asset_map))
        .route("/internal/gameprops/bind-asset", post(bind))
        .route("/internal/gameprops/assets/{id}", delete(remove))
}
