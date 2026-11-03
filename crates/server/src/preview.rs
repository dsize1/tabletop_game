//! 工作台预览宿主（P5 基座，ADR-0033）。
//!
//! 与线上对局**完全隔离**：
//! - 每个预览会话一个独立 `GameManager`（独立房间表），不进入 `AppState.games`；
//! - 房间号强制 `preview:<uuid>`，不注册 `active_by_group`；
//! - 不写 `game_state` / `game_events` / `game_decision_log`（只在内存里留步骤时间线）；
//! - 只由道具中台/本机访问（`require_workbench`）。
//!
//! 预览输入是模板快照（可选 `SceneSpec` 场景注入），输出与 `/resources/game`
//! 相同的三分区视图契约，供 Web 预览页复用 P3 渲染。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use game_engine_core::{
    Action, EngineError, EngineRegistry, GameManager, GameState, GameStatus, Policy, RandomPolicy,
    SeededRng, SystemRng,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use storage::templates::TemplateInput;

use crate::gameprops_routes::{actor, require_workbench};
use crate::AppState;

type ApiError = (StatusCode, Json<Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

fn map_engine_error(e: EngineError) -> ApiError {
    match e {
        EngineError::NotFound(m) => err(StatusCode::NOT_FOUND, m),
        EngineError::PermissionDenied(m) => err(StatusCode::FORBIDDEN, m),
        EngineError::IllegalAction(m) => err(StatusCode::BAD_REQUEST, m),
        EngineError::Internal(m) => err(StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 一条预览动作记录（内存；不落库）。
#[derive(Debug, Clone, Serialize)]
pub struct PreviewStep {
    pub seq: u64,
    pub actor: String,
    /// `human`（工作台代操作）/ `ai`
    pub source: String,
    pub action: String,
    pub message: String,
    pub finished: bool,
    #[serde(default)]
    pub legal: Vec<Action>,
    #[serde(default)]
    pub candidates: Vec<Action>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub state_digest: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
    /// 引擎产出的事件（工作台排障/断言用；仅本机可见）。
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub at: u64,
}

/// 一个预览会话。
struct PreviewSession {
    id: String,
    room_id: String,
    game_id: String,
    mgr: GameManager,
    /// 重置基线（已应用场景的最终状态）。
    base: GameState,
    template_id: Option<i64>,
    created_at: u64,
    last_used: u64,
    steps: Vec<PreviewStep>,
    seq: u64,
}

impl PreviewSession {
    fn new(
        registry: Arc<EngineRegistry>,
        state: GameState,
        template_id: Option<i64>,
    ) -> Self {
        let id = format!("preview:{}", uuid::Uuid::new_v4());
        let mut state = state;
        state.room_id = id.clone();
        state.group_id = id.clone();
        let mgr = GameManager::with_shared_registry(registry, Box::new(SystemRng));
        mgr.restore(state.clone());
        let now = now_secs();
        Self {
            id,
            room_id: state.room_id.clone(),
            game_id: state.game_id.clone(),
            mgr,
            base: state,
            template_id,
            created_at: now,
            last_used: now,
            steps: Vec::new(),
            seq: 0,
        }
    }

    fn touch(&mut self) {
        self.last_used = now_secs();
    }

    fn reset(&mut self) {
        self.mgr = GameManager::with_shared_registry(
            self.mgr.registry_arc(),
            Box::new(SystemRng),
        );
        self.mgr.restore(self.base.clone());
        self.steps.clear();
        self.seq = 0;
        self.touch();
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn seat_summary(&self) -> Value {
        if let Some(state) = self.mgr.get_state(&self.room_id) {
            if let Ok(st) = sanguosha::SgsState::parse(&state.data) {
                let seats: Vec<Value> = st
                    .seats
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        json!({
                            "index": i,
                            "player_id": s.player_id,
                            "general": s.general,
                            "general_name": s.general_name,
                            "identity": s.identity.code(),
                            "hp": s.hp,
                            "max_hp": s.max_hp,
                            "alive": s.alive,
                            "is_bot": s.is_bot,
                        })
                    })
                    .collect();
                return json!({
                    "phase": format!("{:?}", st.phase).to_lowercase(),
                    "turn": st.turn,
                    "turn_phase": format!("{:?}", st.turn_phase).to_lowercase(),
                    "round": st.round,
                    "seats": seats,
                });
            }
        }
        json!({ "seats": [] })
    }

    /// 构造与 `/resources/game` 同形的视图（供预览页/工作台共用）。
    fn view(&self, viewer: Option<&str>, actor: Option<&str>, raw: bool) -> Value {
        let Ok(public) = self.mgr.public_state(&self.room_id) else {
            return json!({ "error": "预览会话状态丢失" });
        };
        let public_json = serde_json::to_value(&public).unwrap_or_default();
        let player_json = match viewer.filter(|v| !v.is_empty()).and_then(|v| {
            self.mgr
                .player_view(&self.room_id, v)
                .ok()
                .and_then(|s| serde_json::to_value(s).ok())
        }) {
            Some(v) => v,
            None => public_json.clone(),
        };
        let action_owner: Option<String> = actor
            .filter(|a| !a.is_empty())
            .map(str::to_string)
            .or_else(|| viewer.filter(|v| !v.is_empty()).map(str::to_string))
            .or_else(|| self.current_actor());
        let actions: Vec<Value> = action_owner
            .as_deref()
            .and_then(|a| self.mgr.legal_actions(&self.room_id, a).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|a| json!({ "id": a.id, "description": a.description }))
            .collect();
        let raw_json = if raw {
            self.mgr
                .get_state(&self.room_id)
                .and_then(|s| serde_json::to_value(s).ok())
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let finished = self
            .mgr
            .get_state(&self.room_id)
            .is_some_and(|s| s.status == GameStatus::Finished);
        let summary = self
            .mgr
            .state_summary(&self.room_id)
            .unwrap_or_else(|_| "预览会话".to_string());
        json!({
            "preview_id": self.id,
            "room_id": self.room_id,
            "game_id": self.game_id,
            "viewer": viewer,
            "openid": viewer,
            "public": public_json,
            "player": player_json,
            "actions": actions,
            "names": {},
            "finished": finished,
            "summary": summary,
            "seats": self.seat_summary(),
            "steps": self.steps,
            "raw": raw_json,
            "template_id": self.template_id,
            "created_at": self.created_at,
            "last_used": self.last_used,
        })
    }

    fn record(
        &mut self,
        actor: &str,
        source: &str,
        action: &str,
        message: String,
        finished: bool,
        legal: Vec<Action>,
        trace: Option<&sanguosha::ai::policy::DecisionTrace>,
        events: &[game_engine_core::EngineEvent],
        latency_ms: i64,
    ) {
        let seq = self.next_seq();
        let at = now_secs();
        let step = PreviewStep {
            seq,
            actor: actor.to_string(),
            source: source.to_string(),
            action: action.to_string(),
            message,
            finished,
            legal,
            candidates: trace.map(|t| t.candidates.clone()).unwrap_or_default(),
            score: trace.map(|t| t.score),
            priority: trace.map(|t| t.priority),
            reason: trace.map(|t| t.reason.clone()),
            state_digest: trace.map(|t| t.state_digest.clone()),
            latency_ms: Some(latency_ms),
            events: events
                .iter()
                .map(|e| {
                    json!({
                        "event_type": e.event_type,
                        "actor": e.actor,
                        "payload": e.payload,
                    })
                })
                .collect(),
            at,
        };
        self.steps.push(step);
        // 防御：单会话最多保留 500 步（工作台排障足够）。
        if self.steps.len() > 500 {
            let drop_n = self.steps.len() - 500;
            self.steps.drain(0..drop_n);
        }
        self.touch();
    }

    fn execute(&mut self, player: &str, action: &str, source: &str) -> Result<PreviewStep, ApiError> {
        let legal = self
            .mgr
            .legal_actions(&self.room_id, player)
            .map_err(map_engine_error)?;
        let t0 = std::time::Instant::now();
        let result = self
            .mgr
            .execute_action(&self.room_id, player, action)
            .map_err(map_engine_error)?;
        let latency = t0.elapsed().as_millis() as i64;
        let seq = self.seq + 1;
        self.record(
            player,
            source,
            action,
            result.message,
            result.finished,
            legal,
            None,
            &result.events,
            latency,
        );
        self.steps
            .iter()
            .find(|s| s.seq == seq)
            .cloned()
            .ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "记录预览步骤失败"))
    }

    fn current_actor(&self) -> Option<String> {
        let state = self.mgr.get_state(&self.room_id)?;
        let st = sanguosha::SgsState::parse(&state.data).ok()?;
        sanguosha::ai::driver::current_actor(&self.mgr, &self.room_id, &st)
    }

    /// AI 单步/连推。`max_steps == 1` 可操作任意当前行动者；`> 1` 只推进机器人。
    fn drive(&mut self, req: &DriveRequest) -> Value {
        let max_steps = req.max_steps.unwrap_or(1).clamp(1, 200);
        let policy = req.policy.as_deref().unwrap_or("heuristic");
        let single = max_steps == 1;
        let mut started = 0usize;
        let mut stop = "max_steps".to_string();
        let mut errors: Vec<String> = Vec::new();
        let mut repeat_key = String::new();
        let mut repeat_count = 0usize;

        for i in 0..max_steps {
            let Some(state) = self.mgr.get_state(&self.room_id) else {
                stop = "no_state".to_string();
                break;
            };
            if state.status == GameStatus::Finished {
                stop = "finished".to_string();
                break;
            }
            let Ok(st) = sanguosha::SgsState::parse(&state.data) else {
                stop = "engine_error".to_string();
                errors.push("状态解析失败".to_string());
                break;
            };
            let actor = if single && req.actor.as_deref().is_some_and(|a| !a.is_empty()) {
                req.actor.clone().unwrap()
            } else {
                match sanguosha::ai::driver::current_actor(&self.mgr, &self.room_id, &st) {
                    Some(a) => a,
                    None => {
                        stop = "no_actor".to_string();
                        break;
                    }
                }
            };
            if !single && !st.seat(&actor).map(|s| s.is_bot).unwrap_or(false) {
                stop = "human_turn".to_string();
                break;
            }
            let legal = match self.mgr.legal_actions(&self.room_id, &actor) {
                Ok(l) => l,
                Err(e) => {
                    stop = "engine_error".to_string();
                    errors.push(format!("legal_actions: {e}"));
                    break;
                }
            };
            if legal.is_empty() {
                stop = "no_legal_actions".to_string();
                break;
            }
            let t0 = std::time::Instant::now();
            let (chosen, trace) = if policy == "random" {
                let mut rng = SeededRng::new((self.seq + 1) | 1);
                let chosen = RandomPolicy.choose(&state, &actor, &legal, &mut rng);
                (chosen, None)
            } else {
                let trace = sanguosha::ai::policy::decide_traced(&state, &actor, &legal);
                (trace.as_ref().map(|t| t.action.clone()), trace)
            };
            let Some(chosen) = chosen else {
                stop = "no_decision".to_string();
                break;
            };
            // 简单破环器：同一 (actor, action) 连续 8 次 → 停止，避免预览卡死。
            let key = format!("{actor}|{chosen}");
            if key == repeat_key {
                repeat_count += 1;
                if repeat_count >= 8 {
                    stop = "loop".to_string();
                    errors.push(format!("{key} 连续重复 {repeat_count} 次"));
                    break;
                }
            } else {
                repeat_key = key;
                repeat_count = 1;
            }
            match self.mgr.execute_action(&self.room_id, &actor, &chosen) {
                Ok(result) => {
                    let latency = t0.elapsed().as_millis() as i64;
                    self.record(
                        &actor,
                        "ai",
                        &chosen,
                        result.message,
                        result.finished,
                        legal,
                        trace.as_ref(),
                        &result.events,
                        latency,
                    );
                    started += 1;
                    if result.finished {
                        stop = "finished".to_string();
                        break;
                    }
                }
                Err(e) => {
                    stop = "engine_error".to_string();
                    errors.push(format!("{actor} {chosen}: {e}"));
                    break;
                }
            }
            if i + 1 == max_steps {
                stop = "max_steps".to_string();
            }
        }

        let newest: Vec<&PreviewStep> = self.steps.iter().rev().take(started).collect();
        json!({
            "ok": errors.is_empty() || started > 0,
            "steps": started,
            "stop": stop,
            "errors": errors,
            "trace": newest.first().map(|s| (*s).clone()),
            "view": self.view(req.viewer.as_deref(), None, false),
        })
    }
}

/// 预览会话管理器（进程内；重启即丢）。
pub struct PreviewManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<PreviewSession>>>>,
    registry: Arc<EngineRegistry>,
    ttl_secs: u64,
    max_sessions: usize,
}

impl PreviewManager {
    pub fn new(registry: Arc<EngineRegistry>, ttl_secs: u64, max_sessions: usize) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            registry,
            ttl_secs: ttl_secs.max(60),
            max_sessions: max_sessions.clamp(1, 64),
        }
    }

    fn cleanup_locked(&self, map: &mut HashMap<String, Arc<Mutex<PreviewSession>>>) {
        let now = now_secs();
        map.retain(|_, s| {
            s.lock()
                .map(|s| now.saturating_sub(s.last_used) < self.ttl_secs)
                .unwrap_or(false)
        });
    }

    fn create(&self, state: GameState, template_id: Option<i64>) -> Result<String, String> {
        let session = PreviewSession::new(self.registry.clone(), state, template_id);
        let id = session.id.clone();
        let mut map = self
            .sessions
            .lock()
            .map_err(|_| "预览会话表锁失败".to_string())?;
        self.cleanup_locked(&mut map);
        if map.len() >= self.max_sessions {
            // 淘汰最久未使用的会话（工作台场景；避免内存无限增长）。
            if let Some(oldest) = map
                .iter()
                .filter_map(|(k, v)| v.lock().ok().map(|s| (k.clone(), s.last_used)))
                .min_by_key(|(_, t)| *t)
                .map(|(k, _)| k)
            {
                map.remove(&oldest);
            }
        }
        map.insert(id.clone(), Arc::new(Mutex::new(session)));
        Ok(id)
    }

    fn get(&self, id: &str) -> Result<Arc<Mutex<PreviewSession>>, ApiError> {
        let mut map = self
            .sessions
            .lock()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话表锁失败"))?;
        self.cleanup_locked(&mut map);
        map.get(id)
            .cloned()
            .ok_or_else(|| err(StatusCode::NOT_FOUND, "预览会话不存在或已过期"))
    }

    fn remove(&self, id: &str) -> bool {
        self.sessions
            .lock()
            .map(|mut m| m.remove(id).is_some())
            .unwrap_or(false)
    }

    fn list(&self) -> Vec<Value> {
        let mut map = match self.sessions.lock() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        self.cleanup_locked(&mut map);
        let mut out: Vec<Value> = map
            .values()
            .filter_map(|s| s.lock().ok())
            .map(|s| {
                json!({
                    "preview_id": s.id,
                    "game_id": s.game_id,
                    "template_id": s.template_id,
                    "created_at": s.created_at,
                    "last_used": s.last_used,
                    "steps": s.steps.len(),
                    "seats": s.seat_summary(),
                })
            })
            .collect();
        out.sort_by(|a, b| {
            b["last_used"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&a["last_used"].as_u64().unwrap_or(0))
        });
        out
    }
}

fn parse_scene_meta(meta: Option<&str>) -> Option<Value> {
    let meta = meta?;
    let value: Value = serde_json::from_str(meta).ok()?;
    value.get("scene").cloned()
}

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    #[serde(default)]
    pub game_id: Option<String>,
    #[serde(default)]
    pub template_id: Option<i64>,
    #[serde(default)]
    pub snapshot: Option<Value>,
    #[serde(default)]
    pub scene: Option<Value>,
    #[serde(default)]
    pub viewer: Option<String>,
}

/// `POST /internal/game/preview`：从模板/内联快照创建预览会话。
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRequest>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;

    let (game_id, snapshot, meta_scene) = if let Some(template_id) = req.template_id {
        let row = state
            .storage
            .get_template(template_id)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("读取模板失败: {e}")))?
            .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("模板 {template_id} 不存在")))?;
        if let Some(gid) = req.game_id.as_deref().filter(|s| !s.is_empty()) {
            if gid != row.game_id {
                return Err(err(
                    StatusCode::BAD_REQUEST,
                    format!("模板 game_id={} 与请求 {gid} 不一致", row.game_id),
                ));
            }
        }
        let snapshot: Value = serde_json::from_str(&row.snapshot)
            .map_err(|e| err(StatusCode::BAD_REQUEST, format!("模板 snapshot 非法: {e}")))?;
        (row.game_id, snapshot, parse_scene_meta(row.meta.as_deref()))
    } else {
        let snapshot = req
            .snapshot
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "需要 template_id 或 snapshot"))?;
        let game_id = req
            .game_id
            .clone()
            .or_else(|| snapshot.get("game_id").and_then(|v| v.as_str()).map(str::to_string))
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "缺少 game_id"))?;
        (game_id, snapshot, None)
    };

    let engine = state
        .games
        .registry()
        .get(&game_id)
        .cloned()
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("未知游戏 {game_id}")))?;

    let mut game_state: GameState = serde_json::from_value(snapshot)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("snapshot 不是 GameState: {e}")))?;
    if game_state.game_id != game_id {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("snapshot.game_id={} 与 {game_id} 不一致", game_state.game_id),
        ));
    }
    // 先隔离房间，再应用场景/校验（预览永远不能指向真实房间）。
    let placeholder = format!("preview:{}", uuid::Uuid::new_v4());
    game_state.room_id = placeholder.clone();
    game_state.group_id = placeholder;

    let scene = req.scene.or(meta_scene).unwrap_or(Value::Null);
    if !scene.is_null() {
        game_state = engine
            .apply_scene(&game_state, &scene)
            .map_err(map_engine_error)?;
    }
    engine
        .validate_state(&game_state)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("模板不可用: {e}")))?;

    let id = state
        .previews
        .create(game_state, req.template_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let session = state.previews.get(&id)?;
    let guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    Ok(Json(guard.view(
        req.viewer.as_deref(),
        req.viewer.as_deref(),
        true,
    )))
}

#[derive(Debug, Deserialize)]
pub struct ValidateSceneRequest {
    pub snapshot: Value,
    #[serde(default)]
    pub scene: Option<Value>,
}

/// `POST /internal/game/preview/scene`：只校验+应用场景，不创建会话（模板保存前用）。
pub async fn validate_scene(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ValidateSceneRequest>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let mut game_state: GameState = serde_json::from_value(req.snapshot)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("snapshot 不是 GameState: {e}")))?;
    let engine = state
        .games
        .registry()
        .get(&game_state.game_id)
        .cloned()
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("未知游戏 {}", game_state.game_id)))?;
    if let Some(scene) = req.scene.filter(|v| !v.is_null()) {
        game_state = engine
            .apply_scene(&game_state, &scene)
            .map_err(map_engine_error)?;
    }
    engine
        .validate_state(&game_state)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("模板不可用: {e}")))?;
    Ok(Json(json!({
        "ok": true,
        "snapshot": game_state,
        "seat_count": game_state.players.len(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct GetQuery {
    #[serde(default)]
    pub viewer: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub raw: Option<u8>,
}

/// `GET /internal/game/preview`：列出当前进程内的预览会话。
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    Ok(Json(json!({ "sessions": state.previews.list() })))
}

/// `GET /internal/game/preview/{id}`：视图 + 合法动作（可选 raw）。
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<GetQuery>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let session = state.previews.get(&id)?;
    let guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    Ok(Json(guard.view(
        q.viewer.as_deref(),
        q.actor.as_deref(),
        q.raw.unwrap_or(0) == 1,
    )))
}

#[derive(Debug, Deserialize)]
pub struct ActionRequest {
    pub player_id: String,
    pub action: String,
}

/// `POST /internal/game/preview/{id}/action`：代任意座位执行（工作台语义）。
pub async fn action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ActionRequest>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    if req.player_id.trim().is_empty() || req.action.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 player_id 或 action"));
    }
    let session = state.previews.get(&id)?;
    let mut guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    let step = guard.execute(&req.player_id, req.action.trim(), "human")?;
    let viewer = Some(req.player_id.as_str());
    Ok(Json(json!({
        "ok": true,
        "step": step,
        "view": guard.view(viewer, viewer, false),
    })))
}

#[derive(Debug, Deserialize)]
pub struct DriveRequest {
    #[serde(default)]
    pub max_steps: Option<usize>,
    /// `heuristic`（默认）/ `random`。
    #[serde(default)]
    pub policy: Option<String>,
    /// 单步时指定行动者（不填 = 当前行动者）。
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub viewer: Option<String>,
}

/// `POST /internal/game/preview/{id}/drive`：AI 单步/连推（不落库）。
pub async fn drive(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<DriveRequest>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let session = state.previews.get(&id)?;
    let mut guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    Ok(Json(guard.drive(&req)))
}

/// `POST /internal/game/preview/{id}/reset`：回到模板/场景基线。
pub async fn reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let session = state.previews.get(&id)?;
    let mut guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    guard.reset();
    Ok(Json(json!({
        "ok": true,
        "preview_id": guard.id,
        "view": guard.view(None, None, false),
    })))
}

/// `DELETE /internal/game/preview/{id}`：销毁预览会话。
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let existed = state.previews.remove(&id);
    Ok(Json(json!({ "ok": true, "existed": existed })))
}

/// `GET /internal/game/preview/meta`：沙盒表单数据（默认 149 去重武将 + 标准身份配比）。
pub async fn sandbox_meta(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    Ok(Json(sanguosha::sandbox::sandbox_meta()))
}

#[derive(Debug, Deserialize)]
pub struct SandboxRequest {
    pub players: usize,
    /// 随机种子（缺省自动随机；同 seed 可复现）。
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub seats: Vec<sanguosha::sandbox::SandboxSeat>,
    /// 是否同时保存为模板（默认 false）。
    #[serde(default)]
    pub save_template: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /internal/game/preview/sandbox`：按表单创建「已开局」预览会话。
///
/// 只写 `game_state_templates`（可选）；预览状态仅内存，不写对局真相源。
pub async fn sandbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SandboxRequest>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let seed = req.seed.unwrap_or_else(rand::random);
    let spec = sanguosha::sandbox::SandboxSpec {
        players: req.players,
        seats: req.seats,
    };
    let mut rng = SeededRng::new(seed);
    let placeholder = format!("preview:{}", uuid::Uuid::new_v4());
    let (game_state, _events) =
        sanguosha::sandbox::new_sandbox_game(&placeholder, &placeholder, &spec, &mut rng)
            .map_err(map_engine_error)?;
    let viewer = game_state.players.first().cloned().unwrap_or_default();

    let template_id = if req.save_template.unwrap_or(false) {
        let snapshot = serde_json::to_string(&game_state)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("序列化失败: {e}")))?;
        let name = req
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("创建对局-{}人-{}", spec.players, seed));
        let meta = json!({
            "sandbox": {
                "players": spec.players,
                "seed": seed,
                "seats": spec.seats,
            }
        })
        .to_string();
        let created_by = actor(&headers);
        let row = state
            .storage
            .create_template(&TemplateInput {
                game_id: "sanguosha",
                name: &name,
                source: "manual",
                snapshot: &snapshot,
                meta: Some(&meta),
                created_by: Some(created_by.as_str()),
            })
            .map_err(|e| err(StatusCode::BAD_REQUEST, format!("保存模板失败: {e}")))?;
        Some(row.id)
    } else {
        None
    };

    let id = state
        .previews
        .create(game_state, template_id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let session = state.previews.get(&id)?;
    let guard = session
        .lock()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "预览会话锁失败"))?;
    let mut view = guard.view(Some(&viewer), Some(&viewer), true);
    if let Some(obj) = view.as_object_mut() {
        obj.insert("seed".into(), json!(seed));
    }
    Ok(Json(view))
}

/// 只读决策日志（工作台「决策日志」TAB；player_id 默认哈希）。
#[derive(Debug, Deserialize)]
pub struct DecisionLogQuery {
    #[serde(default = "default_game_id")]
    pub game_id: String,
    pub room_id: String,
    #[serde(default)]
    pub since_seq: Option<i64>,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn default_game_id() -> String {
    "sanguosha".to_string()
}

fn hash_player(id: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(id.as_bytes());
    digest[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// `GET /internal/gameprops/decision-logs`：决策日志只读查询（工作台用）。
pub async fn decision_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DecisionLogQuery>,
) -> Result<Json<Value>, ApiError> {
    require_workbench(&state, &headers)?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let rows = state
        .storage
        .list_decision_logs(&q.game_id, &q.room_id, q.since_seq.unwrap_or(0), limit)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询决策日志失败: {e}")))?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "seq": r.seq,
                "player_id": hash_player(&r.player_id),
                "source": r.source,
                "chosen": r.chosen,
                "confidence": r.confidence,
                "latency_ms": r.latency_ms,
                "reason": r.reason,
                "state_digest": r.state_digest,
                "created_at": r.created_at,
                "legal_actions": r.legal_actions,
                "candidates": r.candidates,
            })
        })
        .collect();
    Ok(Json(json!({ "rows": out })))
}
