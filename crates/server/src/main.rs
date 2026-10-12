//! server：Rust 单进程多模块入口
//!
//! 组合 bot-adapter / game-engine-core / auth / push，
//! 提供 Axum HTTP API（健康检查、资源查询），并消费 Redis Streams。

mod auth_routes;
mod health;
mod resources_routes;

use axum::extract::State;
use axum::{routing::{get, post}, Json, Router};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    redis_url: String,
    sqlite_path: String,
    auth: auth::AuthService,
    storage: Arc<storage::Storage>,
    plan_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let sqlite_path =
        std::env::var("SQLITE_PATH").unwrap_or_else(|_| "data/tabletop.db".to_string());
    let plan_dir =
        std::env::var("PLAN_DIR").unwrap_or_else(|_| "memory_palace/plan".to_string());
    let auth_ttl = std::env::var("AUTH_TOKEN_TTL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(auth::DEFAULT_TTL_SECS);

    let storage = Arc::new(storage::Storage::open(&sqlite_path)?);

    // 后台启动 bot-adapter 消费循环（qq.raw → internal）
    let adapter_url = redis_url.clone();
    let adapter_storage = storage.clone();
    tokio::spawn(async move {
        if let Err(e) = bot_adapter::run(&adapter_url, adapter_storage).await {
            tracing::error!("bot-adapter 循环退出: {e:#}");
        }
    });

    // 后台出站归档循环（qq.push → group_messages out）
    let push_url = redis_url.clone();
    let push_storage = storage.clone();
    tokio::spawn(async move {
        if let Err(e) = bot_adapter::run_push_archiver(&push_url, push_storage).await {
            tracing::error!("push 归档循环退出: {e:#}");
        }
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
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/issue", post(auth_routes::issue))
        .route("/auth/verify", post(auth_routes::verify))
        .route(
            "/resources/dev_task/{session_id}/plan",
            get(resources_routes::dev_task_plan),
        )
        .route("/resources/game/{game_id}", get(resources_routes::game_resource))
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
