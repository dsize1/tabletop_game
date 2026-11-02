//! server 侧观测指标（P8）：内存原子计数，`GET /internal/metrics` 只读快照。
//!
//! 不引入 Prometheus 依赖；如需长期留存，可由外部按需抓取后写库/时序库。
//! 只统计 server 单写者链路（Web 动作 + 机器人推进 + 决策日志），MCP 进程内动作不计。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Debug)]
pub struct Metrics {
    pub actions_total: AtomicU64,
    pub actions_denied: AtomicU64,
    pub games_started: AtomicU64,
    pub games_finished: AtomicU64,
    pub bot_steps: AtomicU64,
    pub drives: AtomicU64,
    pub decision_logs: AtomicU64,
    pub decision_log_failures: AtomicU64,
    started: Instant,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            actions_total: AtomicU64::new(0),
            actions_denied: AtomicU64::new(0),
            games_started: AtomicU64::new(0),
            games_finished: AtomicU64::new(0),
            bot_steps: AtomicU64::new(0),
            drives: AtomicU64::new(0),
            decision_logs: AtomicU64::new(0),
            decision_log_failures: AtomicU64::new(0),
            started: Instant::now(),
        }
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add(counter: &AtomicU64, n: u64) {
        counter.fetch_add(n, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "uptime_seconds": self.started.elapsed().as_secs(),
            "actions_total": self.actions_total.load(Ordering::Relaxed),
            "actions_denied": self.actions_denied.load(Ordering::Relaxed),
            "games_started": self.games_started.load(Ordering::Relaxed),
            "games_finished": self.games_finished.load(Ordering::Relaxed),
            "bot_steps": self.bot_steps.load(Ordering::Relaxed),
            "drives": self.drives.load(Ordering::Relaxed),
            "decision_logs": self.decision_logs.load(Ordering::Relaxed),
            "decision_log_failures": self.decision_log_failures.load(Ordering::Relaxed),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_exposes_counters() {
        let m = Metrics::new();
        Metrics::inc(&m.actions_total);
        Metrics::add(&m.bot_steps, 3);
        let v = m.snapshot();
        assert_eq!(v["actions_total"], 1);
        assert_eq!(v["bot_steps"], 3);
        assert_eq!(v["actions_denied"], 0);
        assert!(v["uptime_seconds"].as_u64().is_some());
    }
}
