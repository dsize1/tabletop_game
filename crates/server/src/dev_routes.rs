//! 内部 dev API（server ↔ 薄桥 / n8n）：只读 payload、事件追加与增量、投影写、权限判定。
//!
//! 与 `/internal/chat/*` 同类：**仅监听本机、无 token 鉴权**（内网链路）。
//! 保持「SQLite 单写者 = server」：桥/执行器不直连库，统一经此 API。
//!
//! 权限（ADR-0015 §2.4 第 1 项）：`DEV_WHITELIST` 由 **server** 持有并判定，
//! n8n/桥只能调本 API 拿结论，不得自判。判定结果用 200 + `{allow,reason}` 返回，
//! 便于 n8n IF 节点直接分流（不依赖 4xx 语义）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::collections::HashSet;

use crate::dev_state;
use crate::AppState;

const EVENTS_DEFAULT_LIMIT: usize = 50;
const EVENTS_MAX_LIMIT: usize = 200;

/// 路由错误载体：`(status, {"error": reason})`，便于桥转回 `错误: ...` 文本。
type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(serde_json::json!({ "error": msg.into() })))
}

/// 白名单 + 归属校验（写操作共用）。
fn require_owner(
    state: &AppState,
    task: &storage::DevTask,
    qq_id: &str,
) -> Result<(), ApiError> {
    if !is_dev(&state.dev_whitelist, qq_id) {
        return Err(err(StatusCode::FORBIDDEN, "非开发白名单"));
    }
    if task.qq_id != qq_id {
        return Err(err(StatusCode::FORBIDDEN, "非任务发起者"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub session_id: String,
    #[serde(default)]
    pub since_id: Option<i64>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct EventAppend {
    pub event_id: String,
    pub session_id: String,
    pub kind: String,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub payload: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectionUpdate {
    pub session_id: String,
    pub status: String,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub executor: Option<String>,
    #[serde(default)]
    pub executor_ref: Option<String>,
    #[serde(default)]
    pub plan_format: Option<String>,
    /// 谁写的投影（`n8n` / `bridge` / `langgraph` / `manual`）
    pub sync_source: String,
}

#[derive(Debug, Deserialize)]
pub struct PermissionCheck {
    /// `create` | `confirm` | `cancel` | `append` | `read`
    pub op: String,
    pub qq_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// 纯函数：开发白名单判定（空白名单 = 全拒）。
pub fn is_dev(whitelist: &HashSet<String>, qq_id: &str) -> bool {
    whitelist.contains(qq_id)
}

/// 纯函数：按操作做权限判定（便于单测，不依赖 AppState/Redis）。
///
/// - `create`：仅白名单；
/// - `confirm`/`cancel`/`read`：白名单 + 任务归属；
/// - `append`：白名单 + 归属 + 非终态。
pub fn permission_decision(
    op: &str,
    whitelist: &HashSet<String>,
    qq_id: &str,
    task: Option<&storage::DevTask>,
) -> Result<(), String> {
    if qq_id.trim().is_empty() {
        return Err("缺少 qq_id".to_string());
    }
    if !is_dev(whitelist, qq_id) {
        return Err("非开发白名单".to_string());
    }
    match op {
        "create" => Ok(()),
        "confirm" | "cancel" | "append" | "read" => {
            let t = task.ok_or_else(|| "任务不存在".to_string())?;
            if t.qq_id != qq_id {
                return Err("非任务发起者".to_string());
            }
            if op == "append" && matches!(t.status.as_str(), "done" | "failed" | "cancelled") {
                return Err("任务已终态".to_string());
            }
            Ok(())
        }
        other => Err(format!("未知操作: {other}")),
    }
}

/// `GET /internal/dev_task/{session_id}` → `{task, events, artifacts}`（n8n 拉 payload 用）。
pub async fn dev_task_payload(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let task = state
        .storage
        .get_dev_task(&session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "任务不存在"))?;
    let events = state
        .storage
        .list_dev_task_events(&session_id, 0, EVENTS_DEFAULT_LIMIT)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取事件失败"))?;
    let artifacts = state
        .storage
        .list_dev_artifacts(&session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取产物失败"))?;
    serde_json::to_value(serde_json::json!({
        "task": task,
        "events": events,
        "artifacts": artifacts,
    }))
    .map(Json)
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}

/// `POST /internal/dev/events` → 追加事件（`event_id` 幂等）→ `{"inserted": bool}`。
pub async fn append_event(
    State(state): State<AppState>,
    Json(body): Json<EventAppend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if body.event_id.trim().is_empty()
        || body.session_id.trim().is_empty()
        || body.kind.trim().is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let inserted = state
        .storage
        .append_dev_task_event(
            &body.event_id,
            &body.session_id,
            &body.kind,
            body.actor.as_deref(),
            body.payload.as_deref().unwrap_or("{}"),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "inserted": inserted })))
}

/// `GET /internal/dev/events?session_id=&since_id=&limit=` → 增量事件（升序）。
pub async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = q
        .limit
        .unwrap_or(EVENTS_DEFAULT_LIMIT)
        .min(EVENTS_MAX_LIMIT);
    let rows = state
        .storage
        .list_dev_task_events(&q.session_id, q.since_id.unwrap_or(0), limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    serde_json::to_value(rows)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `POST /internal/dev/projection` → 更新任务投影列（任务不存在 → 404）。
pub async fn update_projection(
    State(state): State<AppState>,
    Json(body): Json<ProjectionUpdate>,
) -> Result<StatusCode, ApiError> {
    if body.session_id.trim().is_empty()
        || body.status.trim().is_empty()
        || body.sync_source.trim().is_empty()
    {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "缺少 session_id/status/sync_source",
        ));
    }
    let exists = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .is_some();
    if !exists {
        return Err(err(StatusCode::NOT_FOUND, "任务不存在"));
    }
    state
        .storage
        .update_dev_task_projection(
            &body.session_id,
            &body.status,
            body.stage.as_deref(),
            body.executor.as_deref(),
            body.executor_ref.as_deref(),
            body.plan_format.as_deref(),
            &body.sync_source,
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写投影失败"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /internal/dev/permission` → `{allow, reason}`（权限判定只在 server）。
pub async fn check_permission(
    State(state): State<AppState>,
    Json(body): Json<PermissionCheck>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let task = match body.session_id.as_deref() {
        Some(sid) => state
            .storage
            .get_dev_task(sid)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => None,
    };
    let verdict = permission_decision(
        &body.op,
        &state.dev_whitelist,
        &body.qq_id,
        task.as_ref(),
    );
    Ok(Json(match verdict {
        Ok(()) => serde_json::json!({ "allow": true, "reason": null }),
        Err(reason) => serde_json::json!({ "allow": false, "reason": reason }),
    }))
}

#[derive(Debug, Deserialize)]
pub struct TasksQuery {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskCreate {
    pub session_id: String,
    pub group_id: String,
    pub qq_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TransitionRequest {
    pub session_id: String,
    pub to: String,
    /// 人工迁移（网关）必填并校验归属；内部里程碑迁移（执行器/桥）可省略。
    #[serde(default)]
    pub qq_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub session_id: String,
    pub qq_id: String,
}

#[derive(Debug, Deserialize)]
pub struct MetaUpdate {
    pub session_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub plan_path: Option<String>,
    #[serde(default)]
    pub plan_format: Option<String>,
}

/// `GET /internal/dev/tasks?status=` → 任务列表（按创建时间倒序）。
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(q): Query<TasksQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let rows = state
        .storage
        .list_dev_tasks(q.status.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    serde_json::to_value(rows)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `POST /internal/dev/tasks` → 创建任务（白名单校验，初始 `queued`）+ `created` 事件。
pub async fn create_task(
    State(state): State<AppState>,
    Json(body): Json<TaskCreate>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.session_id.trim().is_empty() || body.qq_id.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 session_id/qq_id"));
    }
    permission_decision("create", &state.dev_whitelist, &body.qq_id, None)
        .map_err(|e| err(StatusCode::FORBIDDEN, e))?;
    state
        .storage
        .create_dev_task(&body.session_id, &body.group_id, &body.qq_id, "queued")
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "创建任务失败"))?;
    let _ = state.storage.append_dev_task_event(
        &uuid::Uuid::new_v4().to_string(),
        &body.session_id,
        "created",
        Some(&body.qq_id),
        "{}",
    );
    let task = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "任务不存在"))?;
    serde_json::to_value(task)
        .map(Json)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}

/// `POST /internal/dev/transition` → 确定性迁移（白名单 + 归属 + 迁移表）+ 事件。
pub async fn transition_task(
    State(state): State<AppState>,
    Json(body): Json<TransitionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !dev_state::is_known(&body.to) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知状态: {}", body.to)));
    }
    let task = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "任务不存在"))?;
    if let Some(qq_id) = body.qq_id.as_deref() {
        require_owner(&state, &task, qq_id)?;
    }
    if !dev_state::is_valid_transition(&task.status, &body.to) {
        return Err(err(
            StatusCode::CONFLICT,
            format!("非法状态迁移: {} -> {}", task.status, body.to),
        ));
    }
    state
        .storage
        .set_dev_task_status(&body.session_id, &body.to)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写状态失败"))?;
    let kind = match body.to.as_str() {
        "cancelled" => "cancelled",
        "failed" => "failed",
        _ => "status_changed",
    };
    let payload = serde_json::json!({"from": task.status, "to": body.to}).to_string();
    let _ = state.storage.append_dev_task_event(
        &uuid::Uuid::new_v4().to_string(),
        &body.session_id,
        kind,
        body.qq_id.as_deref(),
        &payload,
    );
    let updated = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "任务不存在"))?;
    serde_json::to_value(updated)
        .map(Json)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}

/// `POST /internal/dev/confirm` → 人工确认（`awaiting_confirm→running` / `verifying→done`）。
pub async fn confirm_task(
    State(state): State<AppState>,
    Json(body): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let task = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "任务不存在"))?;
    require_owner(&state, &task, &body.qq_id)?;
    let to = dev_state::confirm_target(&task.status)
        .map_err(|e| err(StatusCode::CONFLICT, e))?;
    state
        .storage
        .set_dev_task_status(&body.session_id, to)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写状态失败"))?;
    let kind = if to == "running" { "approved" } else { "verified" };
    let _ = state.storage.append_dev_task_event(
        &uuid::Uuid::new_v4().to_string(),
        &body.session_id,
        kind,
        Some(&body.qq_id),
        "{}",
    );
    let updated = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "任务不存在"))?;
    serde_json::to_value(updated)
        .map(Json)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}

/// `POST /internal/dev/meta` → 回填标题 / 计划路径 / 计划格式（网关或执行器内部用）。
pub async fn update_meta(
    State(state): State<AppState>,
    Json(body): Json<MetaUpdate>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let exists = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .is_some();
    if !exists {
        return Err(err(StatusCode::NOT_FOUND, "任务不存在"));
    }
    if let Some(title) = body.title.as_deref() {
        state
            .storage
            .set_dev_task_title(&body.session_id, title)
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写标题失败"))?;
    }
    if let Some(path) = body.plan_path.as_deref() {
        state
            .storage
            .set_dev_task_plan_path(&body.session_id, path)
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写计划路径失败"))?;
    }
    if let Some(fmt) = body.plan_format.as_deref() {
        state
            .storage
            .set_dev_task_plan_format(&body.session_id, fmt)
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写计划格式失败"))?;
    }
    let task = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "任务不存在"))?;
    serde_json::to_value(task)
        .map(Json)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}


#[derive(Debug, Deserialize)]
pub struct NotifyRequest {
    pub session_id: String,
    pub text: String,
    #[serde(default)]
    pub kind: Option<String>,
}

/// `POST /internal/dev/notify` → 追加 `notify_requested` 事件（出站由网关统一发送）。
pub async fn notify(
    State(state): State<AppState>,
    Json(body): Json<NotifyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.session_id.trim().is_empty() || body.text.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 session_id/text"));
    }
    let payload = serde_json::json!({"text": body.text, "kind": body.kind}).to_string();
    let event_id = uuid::Uuid::new_v4().to_string();
    let inserted = state
        .storage
        .append_dev_task_event(&event_id, &body.session_id, "notify_requested", Some("n8n"), &payload)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写事件失败"))?;
    Ok(Json(serde_json::json!({"event_id": event_id, "inserted": inserted})))
}

// ---- dev_runs（执行 run；单活跃由 DB 唯一索引兼底）----

#[derive(Debug, Deserialize)]
pub struct RunStart {
    pub session_id: String,
    pub stage: String,
    pub executor: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunStatusUpdate {
    pub status: String,
    #[serde(default)]
    pub executor_ref: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

/// `POST /internal/dev/runs` → 启一个 run（同任务已有活跃 run → 409）。
pub async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<RunStart>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.session_id.trim().is_empty()
        || body.stage.trim().is_empty()
        || body.executor.trim().is_empty()
    {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 session_id/stage/executor"));
    }
    let exists = state
        .storage
        .get_dev_task(&body.session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取任务失败"))?
        .is_some();
    if !exists {
        return Err(err(StatusCode::NOT_FOUND, "任务不存在"));
    }
    let run_id = format!(
        "{}:{}:{}",
        body.session_id,
        body.stage,
        uuid::Uuid::new_v4().simple()
    );
    state
        .storage
        .insert_dev_run(
            &run_id,
            &body.session_id,
            &body.stage,
            &body.executor,
            body.branch.as_deref(),
            body.commit_sha.as_deref(),
            body.idempotency_key.as_deref(),
        )
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                err(StatusCode::CONFLICT, "同任务已有活跃 run")
            } else {
                err(StatusCode::INTERNAL_SERVER_ERROR, "创建 run 失败")
            }
        })?;
    let _ = state.storage.append_dev_task_event(
        &uuid::Uuid::new_v4().to_string(),
        &body.session_id,
        "run_started",
        Some(&body.executor),
        &serde_json::json!({"run_id": run_id, "stage": body.stage}).to_string(),
    );
    Ok(Json(serde_json::json!({"run_id": run_id, "status": "queued"})))
}

/// `POST /internal/dev/runs/{run_id}/status` → 推进 run 状态。
pub async fn set_run_status(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(body): Json<RunStatusUpdate>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !matches!(
        body.status.as_str(),
        "queued" | "running" | "succeeded" | "failed" | "cancelled"
    ) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知 run 状态: {}", body.status)));
    }
    state
        .storage
        .set_dev_run_status(
            &run_id,
            &body.status,
            body.executor_ref.as_deref(),
            body.last_error.as_deref(),
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "写 run 失败"))?;
    Ok(Json(serde_json::json!({"run_id": run_id, "status": body.status})))
}

/// `GET /internal/dev/runs/{session_id}/active` → 活跃 run（无 → 404）。
pub async fn active_run(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let run = state
        .storage
        .get_active_dev_run(&session_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取 run 失败"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "无活跃 run"))?;
    serde_json::to_value(run)
        .map(Json)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "序列化失败"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn wl(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn task(qq_id: &str, status: &str) -> storage::DevTask {
        storage::DevTask {
            session_id: "s1".into(),
            group_id: "g1".into(),
            qq_id: qq_id.into(),
            title: None,
            plan_path: None,
            status: status.into(),
            created_at: "1".into(),
            updated_at: "1".into(),
            stage: None,
            executor: None,
            executor_ref: None,
            contract_version: None,
            plan_format: None,
            last_synced_at: None,
            sync_source: None,
        }
    }

    #[test]
    fn empty_whitelist_denies_all() {
        assert!(!is_dev(&wl(&[]), "u1"));
        assert!(permission_decision("create", &wl(&[]), "u1", None).is_err());
    }

    #[test]
    fn create_only_needs_whitelist() {
        let w = wl(&["dev1"]);
        assert!(permission_decision("create", &w, "dev1", None).is_ok());
        assert!(permission_decision("create", &w, "outsider", None).is_err());
        assert!(permission_decision("create", &w, "  ", None).is_err());
    }

    #[test]
    fn owner_ops_require_task_and_ownership() {
        let w = wl(&["dev1"]);
        // 无任务
        assert!(permission_decision("confirm", &w, "dev1", None).is_err());
        // 非归属者
        let t = task("dev1", "awaiting_confirm");
        assert!(permission_decision("confirm", &w, "other", Some(&t)).is_err());
        // 归属者
        assert!(permission_decision("confirm", &w, "dev1", Some(&t)).is_ok());
        assert!(permission_decision("read", &w, "dev1", Some(&t)).is_ok());
        assert!(permission_decision("cancel", &w, "dev1", Some(&t)).is_ok());
        // 非白名单即便归属也拒
        assert!(permission_decision("confirm", &wl(&[]), "dev1", Some(&t)).is_err());
    }

    #[test]
    fn append_rejects_terminal_tasks() {
        let w = wl(&["dev1"]);
        for st in ["done", "failed", "cancelled"] {
            let t = task("dev1", st);
            assert!(
                permission_decision("append", &w, "dev1", Some(&t)).is_err(),
                "{st} 应拒绝追加"
            );
        }
        let running = task("dev1", "running");
        assert!(permission_decision("append", &w, "dev1", Some(&running)).is_ok());
    }

    #[test]
    fn unknown_op_is_rejected() {
        let w = wl(&["dev1"]);
        assert!(permission_decision("deploy", &w, "dev1", Some(&task("dev1", "running"))).is_err());
    }
}
