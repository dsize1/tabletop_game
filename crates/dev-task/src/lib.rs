//! 开发任务引擎核心：确定性状态机 + 白名单校验 + SQLite 持久化。
//!
//! 与 game-engine 同构：权限/状态迁移由本引擎无条件校验，不信任任何上游。
//! 区别：dev 任务的「决策模型」是人（固定前缀命令，置信度 100%），
//! 因此 confirm/cancel 仅由网关收到人工命令后调用；子会话只做内容，不改状态。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// 开发任务状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevTaskStatus {
    /// 已受理，排队中
    Queued,
    /// 子会话规划中（含多轮澄清）
    Planning,
    /// 计划已生成，待人工确认
    AwaitingConfirm,
    /// 执行中
    Running,
    /// 待人工验收
    Verifying,
    /// 已完成（验收通过）
    Done,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

impl DevTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Planning => "planning",
            Self::AwaitingConfirm => "awaiting_confirm",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "planning" => Some(Self::Planning),
            "awaiting_confirm" => Some(Self::AwaitingConfirm),
            "running" => Some(Self::Running),
            "verifying" => Some(Self::Verifying),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// 任务引擎错误。
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("无权限: {0}")]
    PermissionDenied(String),
    #[error("任务不存在: {0}")]
    NotFound(String),
    #[error("非法状态迁移: {0} -> {1}")]
    IllegalTransition(String, String),
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 确定性迁移表：Forward 迁移白名单 + 任意非终态可进入 Failed/Cancelled。
fn is_valid_transition(from: DevTaskStatus, to: DevTaskStatus) -> bool {
    use DevTaskStatus::*;
    if from.is_terminal() {
        return false;
    }
    if matches!(to, Failed | Cancelled) {
        return true;
    }
    matches!(
        (from, to),
        (Queued, Planning)
            | (Planning, AwaitingConfirm)
            | (AwaitingConfirm, Running)
            | (Running, Verifying)
            | (Running, Done)
            | (Verifying, Running)
            | (Verifying, Done)
    )
}

/// 任务管理器：状态机 + 白名单校验，落盘 SQLite。
pub struct TaskManager {
    storage: Arc<storage::Storage>,
    whitelist: HashSet<String>,
}

impl TaskManager {
    pub fn new(storage: Arc<storage::Storage>, whitelist: HashSet<String>) -> Self {
        Self { storage, whitelist }
    }

    /// 白名单校验（空白名单 = 全部拒绝）。
    fn is_dev(&self, qq_id: &str) -> bool {
        self.whitelist.contains(qq_id)
    }

    /// 创建任务（白名单校验），初始状态 Queued。
    pub fn create_task(
        &self,
        session_id: &str,
        group_id: &str,
        qq_id: &str,
    ) -> Result<storage::DevTask, TaskError> {
        if !self.is_dev(qq_id) {
            return Err(TaskError::PermissionDenied(qq_id.to_string()));
        }
        self.storage
            .create_dev_task(session_id, group_id, qq_id, DevTaskStatus::Queued.as_str())
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        self.get(session_id)
    }

    /// 人工确认（白名单 + 任务归属）。
    /// - AwaitingConfirm → Running（确认计划，开始执行）
    /// - Verifying → Done（验收通过）
    pub fn confirm_task(&self, session_id: &str, qq_id: &str) -> Result<storage::DevTask, TaskError> {
        self.check_owner(session_id, qq_id)?;
        let task = self.get(session_id)?;
        let to = match DevTaskStatus::from_str(&task.status) {
            Some(DevTaskStatus::AwaitingConfirm) => DevTaskStatus::Running,
            Some(DevTaskStatus::Verifying) => DevTaskStatus::Done,
            _ => {
                return Err(TaskError::IllegalTransition(
                    task.status,
                    "confirmed".to_string(),
                ))
            }
        };
        self.transition(session_id, to)
    }

    /// 人工取消（白名单 + 任务归属），非终态 → Cancelled。
    pub fn cancel_task(&self, session_id: &str, qq_id: &str) -> Result<storage::DevTask, TaskError> {
        self.check_owner(session_id, qq_id)?;
        self.transition(session_id, DevTaskStatus::Cancelled)
    }

    /// 白名单 + 归属 + 非终态校验（网关的「补充正文 / 重签链接」等动作共用）。
    pub fn check_active_owner(
        &self,
        session_id: &str,
        qq_id: &str,
    ) -> Result<storage::DevTask, TaskError> {
        self.check_owner(session_id, qq_id)?;
        let task = self.get(session_id)?;
        let status = DevTaskStatus::from_str(&task.status)
            .ok_or_else(|| TaskError::Internal(format!("未知状态: {}", task.status)))?;
        if status.is_terminal() {
            return Err(TaskError::IllegalTransition(task.status, "active".to_string()));
        }
        Ok(task)
    }

    /// 内容里程碑迁移（网关驱动，确定性迁移表校验）。
    pub fn transition(
        &self,
        session_id: &str,
        to: DevTaskStatus,
    ) -> Result<storage::DevTask, TaskError> {
        let task = self.get(session_id)?;
        let from = DevTaskStatus::from_str(&task.status)
            .ok_or_else(|| TaskError::Internal(format!("未知状态: {}", task.status)))?;
        if !is_valid_transition(from, to) {
            return Err(TaskError::IllegalTransition(
                from.as_str().to_string(),
                to.as_str().to_string(),
            ));
        }
        self.storage
            .set_dev_task_status(session_id, to.as_str())
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        self.get(session_id)
    }

    /// 回填标题（子会话计划生成后由网关调用）。
    pub fn set_title(&self, session_id: &str, title: &str) -> Result<storage::DevTask, TaskError> {
        self.storage
            .set_dev_task_title(session_id, title)
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        self.get(session_id)
    }

    /// 记录计划文档路径。
    pub fn set_plan_path(
        &self,
        session_id: &str,
        plan_path: &str,
    ) -> Result<storage::DevTask, TaskError> {
        self.storage
            .set_dev_task_plan_path(session_id, plan_path)
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        self.get(session_id)
    }

    /// 白名单 + 任务归属双重校验（confirm/cancel 共用）。
    fn check_owner(&self, session_id: &str, qq_id: &str) -> Result<(), TaskError> {
        if !self.is_dev(qq_id) {
            return Err(TaskError::PermissionDenied(qq_id.to_string()));
        }
        let task = self.get(session_id)?;
        if task.qq_id != qq_id {
            return Err(TaskError::PermissionDenied(format!("非任务发起者: {qq_id}")));
        }
        Ok(())
    }

    pub fn get(&self, session_id: &str) -> Result<storage::DevTask, TaskError> {
        self.storage
            .get_dev_task(session_id)
            .map_err(|e| TaskError::Internal(e.to_string()))?
            .ok_or_else(|| TaskError::NotFound(session_id.to_string()))
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<storage::DevTask>, TaskError> {
        self.storage
            .list_dev_tasks(status_filter)
            .map_err(|e| TaskError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(whitelist: &[&str]) -> TaskManager {
        let storage = Arc::new(storage::Storage::open(":memory:").unwrap());
        let set: HashSet<String> = whitelist.iter().map(|s| s.to_string()).collect();
        TaskManager::new(storage, set)
    }

    #[test]
    fn create_requires_whitelist() {
        let m = manager(&["dev1"]);
        assert!(m.create_task("s1", "g1", "dev1").is_ok());
        assert!(m.create_task("s2", "g1", "outsider").is_err());
        // 空白名单 = 全部拒绝
        let m0 = manager(&[]);
        assert!(m0.create_task("s3", "g1", "dev1").is_err());
    }

    #[test]
    fn full_lifecycle() {
        let m = manager(&["dev1"]);
        let t = m.create_task("s1", "g1", "dev1").unwrap();
        assert_eq!(t.status, "queued");
        assert_eq!(m.transition("s1", DevTaskStatus::Planning).unwrap().status, "planning");
        assert_eq!(
            m.transition("s1", DevTaskStatus::AwaitingConfirm).unwrap().status,
            "awaiting_confirm"
        );
        // 确认计划 → running
        assert_eq!(m.confirm_task("s1", "dev1").unwrap().status, "running");
        // 非归属者确认被拒
        assert!(m.confirm_task("s1", "outsider").is_err());
        // 执行完 → verifying → 验收通过 → done
        assert_eq!(m.transition("s1", DevTaskStatus::Verifying).unwrap().status, "verifying");
        assert_eq!(m.confirm_task("s1", "dev1").unwrap().status, "done");
        // 终态不可再迁移
        assert!(m.transition("s1", DevTaskStatus::Running).is_err());
    }

    #[test]
    fn illegal_transition() {
        let m = manager(&["dev1"]);
        m.create_task("s1", "g1", "dev1").unwrap();
        assert!(m.transition("s1", DevTaskStatus::Running).is_err());
        assert!(m.transition("s1", DevTaskStatus::AwaitingConfirm).is_err());
    }

    #[test]
    fn cancel_and_title() {
        let m = manager(&["dev1"]);
        m.create_task("s1", "g1", "dev1").unwrap();
        m.set_title("s1", "做个新功能").unwrap();
        assert_eq!(m.get("s1").unwrap().title.as_deref(), Some("做个新功能"));
        assert_eq!(m.cancel_task("s1", "dev1").unwrap().status, "cancelled");
        assert!(m.transition("s1", DevTaskStatus::Planning).is_err());
    }

    #[test]
    fn check_active_owner_rules() {
        let m = manager(&["dev1"]);
        m.create_task("s1", "g1", "dev1").unwrap();
        // 非白名单 / 非归属者被拒
        assert!(m.check_active_owner("s1", "outsider").is_err());
        assert!(m.check_active_owner("s1", "other").is_err());
        // 非终态可访问
        assert_eq!(m.check_active_owner("s1", "dev1").unwrap().status, "queued");
        // 终态被拒
        m.cancel_task("s1", "dev1").unwrap();
        assert!(m.check_active_owner("s1", "dev1").is_err());
    }
}
