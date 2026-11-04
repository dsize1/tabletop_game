//! server：Rust 单进程多模块入口
//!
//! 组合 bot-adapter / game-engine-core / auth，
//! 提供 Axum HTTP API（健康检查、资源查询），并消费 Redis Streams。
//!
//! 注：出站推送不经 Rust 写 `qq.push`（原 `crates/push` 已于 2026-10-20 删除），
//! 生产者是 agent-gateway（Node），此处只做 `qq.push → group_messages(out)` 归档。

mod auth_routes;
mod bot_drive;
mod dev_routes;
mod dev_state;
mod docs_routes;
mod game_routes;
mod gameprops_assets;
mod gameprops_routes;
mod health;
mod internal_routes;
mod metrics;
mod preview;
mod resilient;
mod resources_routes;
mod template_routes;
mod web_auth_routes;

use axum::extract::State;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    redis_url: String,
    sqlite_path: String,
    auth: auth::AuthService,
    storage: Arc<storage::Storage>,
    plan_dir: String,
    docs: shared::docs_config::DocsConfig,
    /// 开发白名单（ADR-0015：判定只在 server；空白名单 = 全拒）
    dev_whitelist: Arc<HashSet<String>>,
    /// 中台白名单（ADR-0031：默认回落 DEV_WHITELIST；空白名单 = 全拒）
    props_whitelist: Arc<HashSet<String>>,
    /// 测试玩家白名单：仅可访问中台「工作台」菜单（ADR-0035）
    test_player_whitelist: Arc<HashSet<String>>,
    /// Web 跨端登录会话（短效 token + 状态 + 验证码，ADR-0031）
    weblogin: auth::weblogin::WebLoginService,
    /// 道具中台管理员 token（未设置 = 本机开发模式放行）
    props_admin_token: Option<String>,
    /// 道具数据目录（heroes.json / cards.json）
    props_data_dir: std::path::PathBuf,
    /// 道具素材目录（内容寻址文件；web 以 /game-assets 静态挂载同一目录）
    props_assets_dir: std::path::PathBuf,
    /// 对局执行宿主（P3：Web 动作由 server 执行；单写者约定见计划台账）
    games: Arc<game_engine_core::GameManager>,
    /// 机器人推进的房间锁（P6.3：按房间串行，避免并发双推进）
    bot_drive: Arc<bot_drive::BotDrive>,
    /// 观测指标（P8；内存计数，`/internal/metrics`）
    metrics: Arc<metrics::Metrics>,
    /// 工作台预览会话（P5 基座）：独立内存态，不写真相源（ADR-0033）。
    previews: Arc<preview::PreviewManager>,
    /// 房间快照版本（`game_state.version`）：内存态与真相源对齐用。
    ///
    /// 背景：MCP（agent-gateway）仍会写 lobby 动作（`#加机器人` / `#武将包` / `#开始`），
    /// 而 server 只在「房间不在内存」时 restore → 内存态会落后于 SQLite，
    /// 表现为页面停在旧阶段（P6.3 真机：`#开始` 后网页仍卡在选将）。
    /// 这里记「本进程最近一次已知版本」，请求进来发现 DB 更新就重新 restore。
    room_versions: Arc<std::sync::Mutex<HashMap<String, i64>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let sqlite_path =
        std::env::var("SQLITE_PATH").unwrap_or_else(|_| "data/tabletop.db".to_string());
    let plan_dir = std::env::var("PLAN_DIR").unwrap_or_else(|_| "memory_palace/plan".to_string());
    let auth_ttl = std::env::var("AUTH_TOKEN_TTL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(auth::DEFAULT_TTL_SECS);

    let storage = Arc::new(storage::Storage::open(&sqlite_path)?);

    // 对局引擎注册表：server 作为 Web 动作的执行宿主（P3，单写者）。
    // 快照是真相源：房间不在内存时按需从 SQLite 恢复（见 resources_routes::ensure_room）。
    let mut registry = game_engine_core::EngineRegistry::new();
    registry.register(las_vegas::LasVegasEngine);
    registry.register(sanguosha::SanguoshaEngine);
    let games = Arc::new(game_engine_core::GameManager::new(registry));
    let preview_registry = games.registry_arc();

    // 开发白名单（从 MCP env 迁到 server env；空白名单 = 全拒）
    let dev_whitelist: HashSet<String> = std::env::var("DEV_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // 测试玩家白名单：仅 workbench 权限（为空 = 无测试玩家，ADR-0035）
    let test_player_whitelist: HashSet<String> = std::env::var("TEST_PLAYER_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // 中台白名单：未设置时回落 DEV_WHITELIST（与开发链路同一名单，ADR-0031）
    let props_whitelist: HashSet<String> = match std::env::var("PROPS_ADMIN_WHITELIST") {
        Ok(v) if !v.trim().is_empty() => v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => dev_whitelist.clone(),
    };

    // 后台消费循环：**必须自愈**（Redis 重启/加载中会瞬时失败）。
    // 历史故障（P6.3 真机）：server 启动早于 Redis 就绪 → bot-adapter 直接退出 →
    // 群消息只进 `qq.raw` 无人消费，表现为「机器人没反应」，只能重启 server。
    let adapter_url = redis_url.clone();
    let adapter_storage = storage.clone();
    resilient::spawn_resilient("bot-adapter", move || {
        let url = adapter_url.clone();
        let st = adapter_storage.clone();
        async move { bot_adapter::run(&url, st).await }
    });

    // 后台出站归档循环（qq.push → group_messages out）
    let push_url = redis_url.clone();
    let push_storage = storage.clone();
    resilient::spawn_resilient("push-archiver", move || {
        let url = push_url.clone();
        let st = push_storage.clone();
        async move { bot_adapter::run_push_archiver(&url, st).await }
    });

    // 后台对话归档循环（chat.log → chat_history；画像蒸馏数据源，ADR-0007）
    let chatlog_url = redis_url.clone();
    let chatlog_storage = storage.clone();
    resilient::spawn_resilient("chatlog-archiver", move || {
        let url = chatlog_url.clone();
        let st = chatlog_storage.clone();
        async move { bot_adapter::run_chat_log_archiver(&url, st).await }
    });

    // 后台健康自检循环
    let hl_redis = redis_url.clone();
    let hl_sqlite = sqlite_path.clone();
    tokio::spawn(async move {
        health::health_loop(hl_redis, hl_sqlite).await;
    });

    let state = AppState {
        redis_url: redis_url.clone(),
        sqlite_path: sqlite_path.clone(),
        auth: auth::AuthService::new(&redis_url, auth_ttl).await?,
        storage: storage.clone(),
        plan_dir,
        docs: shared::docs_config::load(),
        dev_whitelist: Arc::new(dev_whitelist),
        props_whitelist: Arc::new(props_whitelist),
        test_player_whitelist: Arc::new(test_player_whitelist),
        weblogin: auth::weblogin::WebLoginService::new(&redis_url).await?,
        props_admin_token: std::env::var("PROPS_ADMIN_TOKEN")
            .ok()
            .filter(|s| !s.is_empty()),
        props_data_dir: std::path::PathBuf::from(
            std::env::var("GAMEPROPS_DATA_DIR")
                .unwrap_or_else(|_| "docs/games/sanguosha/data".to_string()),
        ),
        props_assets_dir: {
            let dir = std::path::PathBuf::from(
                std::env::var("GAMEPROPS_ASSETS_DIR")
                    .unwrap_or_else(|_| "data/gameprops/assets".to_string()),
            );
            std::fs::create_dir_all(&dir).ok();
            dir
        },
        games,
        previews: Arc::new(preview::PreviewManager::new(
            preview_registry,
            std::env::var("SGS_PREVIEW_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2 * 3600),
            std::env::var("SGS_PREVIEW_MAX_SESSIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8),
        )),
        bot_drive: Arc::new(bot_drive::BotDrive::new()),
        metrics: Arc::new(metrics::Metrics::new()),
        room_versions: Arc::new(std::sync::Mutex::new(HashMap::new())),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/internal/metrics", get(metrics_snapshot))
        .route("/auth/web/status", get(web_auth_routes::status))
        .route("/auth/web/register", post(web_auth_routes::register))
        .route("/auth/web/login", post(web_auth_routes::login))
        .route("/auth/web/confirm", get(web_auth_routes::confirm))
        .route("/auth/web/whoami", get(web_auth_routes::whoami))
        .route("/auth/web/qq/start", get(web_auth_routes::qq_start))
        .route("/auth/web/qq/callback", get(web_auth_routes::qq_callback))
        .route("/internal/auth/web/link", post(web_auth_routes::link))
        .route("/internal/auth/web/verify", post(web_auth_routes::verify))
        .route("/auth/issue", post(auth_routes::issue))
        .route("/auth/verify", post(auth_routes::verify))
        .route(
            "/resources/dev_task/{session_id}/plan",
            get(resources_routes::dev_task_plan),
        )
        .route(
            "/resources/game/{game_id}",
            get(resources_routes::game_resource),
        )
        // Web 动作通路（P3，仅本机；单写者由 web/server 承担）
        .route(
            "/internal/game/{game_id}/{room_id}/action",
            post(game_routes::execute_action),
        )
        // P6.3：手动触发机器人推进（本机；并发跳过/重启续推的兜底）
        .route(
            "/internal/game/{game_id}/{room_id}/drive",
            post(game_routes::drive),
        )
        // 工作台预览（P5 基座，ADR-0033）：独立内存态，不写真相源
        .route("/internal/game/preview", get(preview::list).post(preview::create))
        .route(
            "/internal/game/preview/scene",
            post(preview::validate_scene),
        )
        .route("/internal/game/preview/meta", get(preview::sandbox_meta))
        .route("/internal/game/preview/sandbox", post(preview::sandbox))
        .route(
            "/internal/game/preview/{id}",
            get(preview::get).delete(preview::delete),
        )
        .route(
            "/internal/game/preview/{id}/action",
            post(preview::action),
        )
        .route(
            "/internal/game/preview/{id}/drive",
            post(preview::drive),
        )
        .route(
            "/internal/game/preview/{id}/reset",
            post(preview::reset),
        )
        // 工作台决策日志只读查询（player_id 哈希；与导出脚本口径一致）
        .route(
            "/internal/gameprops/decision-logs",
            get(preview::decision_logs),
        )
        .route("/resources/docs/tree", get(docs_routes::docs_tree))
        .route("/resources/docs/file", get(docs_routes::docs_file))
        .route("/internal/chat/window", get(internal_routes::chat_window))
        .route(
            "/internal/chat/active_openids",
            get(internal_routes::active_openids),
        )
        .route(
            "/internal/profile/{openid}",
            get(internal_routes::get_profile),
        )
        .route("/internal/profile", post(internal_routes::upsert_profile))
        // dev 线内部 API（ADR-0015 §2.4）：唯写者仍是 server
        .route(
            "/internal/dev_task/{session_id}",
            get(dev_routes::dev_task_payload),
        )
        .route("/internal/dev/events", post(dev_routes::append_event))
        .route("/internal/dev/events", get(dev_routes::list_events))
        .route("/internal/dev/notify", post(dev_routes::notify))
        .route("/internal/dev/tasks", get(dev_routes::list_tasks))
        .route("/internal/dev/tasks", post(dev_routes::create_task))
        .route(
            "/internal/dev/transition",
            post(dev_routes::transition_task),
        )
        .route("/internal/dev/confirm", post(dev_routes::confirm_task))
        .route("/internal/dev/runs", post(dev_routes::start_run))
        .route(
            "/internal/dev/runs/{run_id}/status",
            post(dev_routes::set_run_status),
        )
        .route(
            "/internal/dev/runs/{session_id}/active",
            get(dev_routes::active_run),
        )
        .route("/internal/dev/meta", post(dev_routes::update_meta))
        .route(
            "/internal/dev/projection",
            post(dev_routes::update_projection),
        )
        .route(
            "/internal/dev/permission",
            post(dev_routes::check_permission),
        )
        // 道具中台（P1）：单写者仍是 server
        .route(
            "/internal/gameprops/summary",
            get(gameprops_routes::summary),
        )
        .route("/internal/gameprops/seed", post(gameprops_routes::seed))
        .route(
            "/internal/gameprops/import/heroes",
            post(gameprops_routes::import_heroes),
        )
        .route(
            "/internal/gameprops/import/engine",
            post(gameprops_routes::import_engine),
        )
        .route(
            "/internal/gameprops/status",
            post(gameprops_routes::set_status),
        )
        .route("/internal/gameprops/session", get(gameprops_routes::session))
        // 素材（P1 入中台）：内容寻址上传/缩略图/绑定/映射（ADR-0036）
        .merge(gameprops_assets::router())
        // 工作台模板（P5 基座）：显式路由优先于 `{table}` 动态路由
        .route(
            "/internal/gameprops/templates",
            get(template_routes::list).post(template_routes::create),
        )
        .route(
            "/internal/gameprops/templates/from-room",
            post(template_routes::from_room),
        )
        .route(
            "/internal/gameprops/templates/from-replay",
            post(template_routes::from_replay),
        )
        .route(
            "/internal/gameprops/templates/{id}",
            get(template_routes::get)
                .put(template_routes::update)
                .delete(template_routes::delete),
        )
        .route(
            "/internal/gameprops/templates/{id}/duplicate",
            post(template_routes::duplicate),
        )
        .route(
            "/internal/gameprops/{table}",
            get(gameprops_routes::list).post(gameprops_routes::upsert),
        )
        .route(
            "/internal/gameprops/{table}/{code}",
            get(gameprops_routes::get_one),
        )
        .with_state(state);

    let port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("server 监听 http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

/// 健康检查（读缓存，miss 时现场探测）。
async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(health::get_status(&state.redis_url, &state.sqlite_path).await)
}

/// 观测指标快照（本机；非公网）。
async fn metrics_snapshot(State(state): State<AppState>) -> Json<Value> {
    Json(state.metrics.snapshot())
}
