//! 健康检查：后台自检循环 + 探测函数。
//!
//! - 后台循环每 10s 探测 Redis / SQLite，结果写 Redis 缓存 `health:status`（TTL 30s）。
//! - `/health` 与 agent-gateway 只读缓存（<1ms），不进消息热路径。

use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use std::time::Duration;

/// 后台自检循环（由 main 以 tokio::spawn 启动）。
pub async fn health_loop(redis_url: String, sqlite_path: String) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        let report = probe(&redis_url, &sqlite_path).await;
        write_cache(&redis_url, &report).await;
    }
}

/// 现场探测（按需 / 缓存 miss 时调用）。
pub async fn probe(redis_url: &str, sqlite_path: &str) -> Value {
    let redis_ok = redis_ping(redis_url).await;
    let sqlite_ok = storage::check(sqlite_path);
    json!({
        "status": if redis_ok && sqlite_ok { "ok" } else { "degraded" },
        "checks": { "redis": redis_ok, "sqlite": sqlite_ok },
        "probed_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

/// 读缓存状态；未命中则现场探测并写缓存。
pub async fn get_status(redis_url: &str, sqlite_path: &str) -> Value {
    if let Some(mut conn) = redis_conn(redis_url).await {
        let cached: redis::RedisResult<String> = redis::cmd("GET")
            .arg("health:status")
            .query_async(&mut conn)
            .await;
        if let Ok(s) = cached {
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                return v;
            }
        }
    }
    let report = probe(redis_url, sqlite_path).await;
    write_cache(redis_url, &report).await;
    report
}

async fn redis_conn(url: &str) -> Option<ConnectionManager> {
    let client = redis::Client::open(url).ok()?;
    ConnectionManager::new(client).await.ok()
}

async fn redis_ping(url: &str) -> bool {
    match redis_conn(url).await {
        Some(mut conn) => {
            let r: redis::RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;
            r.map(|s| s == "PONG").unwrap_or(false)
        }
        None => false,
    }
}

async fn write_cache(redis_url: &str, report: &Value) {
    if let Some(mut conn) = redis_conn(redis_url).await {
        let _: redis::RedisResult<()> = redis::cmd("SET")
            .arg("health:status")
            .arg(report.to_string())
            .arg("EX")
            .arg(30)
            .query_async(&mut conn)
            .await;
    }
}
