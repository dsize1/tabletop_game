//! 开发任务状态机（确定性迁移表）。
//!
//! 过渡期（ADR-0015 §2.4 L2）：状态权威由 `server` 持有，语义与
//! 与历史 dev-task crate（2026-10-20 已删除）**逐条对齐**（同状态名、同迁移表、同 confirm 规则）。
//! B7 切 L3 后，本模块降级为投影校验（或随 L3 一并移除）。
//!
//! 约定：校验失败返回人类可读原因（由路由转 400/404/409），不在这里做 IO。

/// 任务状态（与 `dev_tasks.status` 取值一致）。
pub const STATUSES: [&str; 8] = [
    "queued",
    "planning",
    "awaiting_confirm",
    "running",
    "verifying",
    "done",
    "failed",
    "cancelled",
];

pub fn is_terminal(status: &str) -> bool {
    matches!(status, "done" | "failed" | "cancelled")
}

pub fn is_known(status: &str) -> bool {
    STATUSES.contains(&status)
}

/// 确定性迁移表（与 dev-task crate 完全一致）。
pub fn is_valid_transition(from: &str, to: &str) -> bool {
    if !is_known(from) || !is_known(to) {
        return false;
    }
    if is_terminal(from) {
        return false;
    }
    if matches!(to, "failed" | "cancelled") {
        return true;
    }
    matches!(
        (from, to),
        ("queued", "planning")
            | ("planning", "awaiting_confirm")
            | ("awaiting_confirm", "running")
            | ("running", "verifying")
            | ("running", "done")
            | ("verifying", "running")
            | ("verifying", "done")
    )
}

/// 人工确认的目标状态：待确认计划 → 执行；待验收 → 完成；其余状态非法。
pub fn confirm_target(from: &str) -> Result<&'static str, String> {
    match from {
        "awaiting_confirm" => Ok("running"),
        "verifying" => Ok("done"),
        other => Err(format!("非法状态迁移: {other} -> confirmed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_are_dead_ends() {
        for st in ["done", "failed", "cancelled"] {
            for to in STATUSES {
                assert!(!is_valid_transition(st, to), "{st} -> {to} 应被拒绝");
            }
        }
    }

    #[test]
    fn any_active_state_can_fail_or_cancel() {
        for st in ["queued", "planning", "awaiting_confirm", "running", "verifying"] {
            assert!(is_valid_transition(st, "failed"));
            assert!(is_valid_transition(st, "cancelled"));
        }
    }

    #[test]
    fn forward_table_matches_dev_task_crate() {
        let allowed = [
            ("queued", "planning"),
            ("planning", "awaiting_confirm"),
            ("awaiting_confirm", "running"),
            ("running", "verifying"),
            ("running", "done"),
            ("verifying", "running"),
            ("verifying", "done"),
        ];
        for (from, to) in allowed {
            assert!(is_valid_transition(from, to), "{from} -> {to} 应允许");
        }
        // 反向 / 跳步 / 自环
        let rejected = [
            ("planning", "queued"),
            ("queued", "running"),
            ("queued", "awaiting_confirm"),
            ("awaiting_confirm", "verifying"),
            ("running", "queued"),
            ("verifying", "awaiting_confirm"),
            ("done", "running"),
            ("running", "running"),
            ("unknown", "planning"),
        ];
        for (from, to) in rejected {
            assert!(!is_valid_transition(from, to), "{from} -> {to} 应拒绝");
        }
    }

    #[test]
    fn confirm_target_semantics() {
        assert_eq!(confirm_target("awaiting_confirm").unwrap(), "running");
        assert_eq!(confirm_target("verifying").unwrap(), "done");
        for st in ["queued", "planning", "running", "done", "failed", "cancelled"] {
            assert!(confirm_target(st).is_err(), "{st} 不应可确认");
        }
    }
}
