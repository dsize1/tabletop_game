//! dev-task-mcp：把开发任务引擎映射为 MCP 统一原语，以 stdio 暴露给 agent-gateway。
//!
//! 只读：list_tasks / get_task / get_progress / list_capabilities
//! 写入（受校验）：create_task / confirm_task / cancel_task / transition_task / update_task_meta
//! 校验链（与 game-engine 同构）：白名单 + 任务归属 + 确定性状态迁移表。

use dev_task::{DevTaskStatus, TaskManager};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use std::sync::Arc;

// —— 工具参数类型（MCP tool 入参）——
#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct SessionRequest {
    session_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct ListTasksRequest {
    status: Option<String>,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct CreateTaskRequest {
    session_id: String,
    group_id: String,
    qq_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct OwnerRequest {
    session_id: String,
    qq_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct TransitionRequest {
    session_id: String,
    status: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct UpdateMetaRequest {
    session_id: String,
    title: Option<String>,
    plan_path: Option<String>,
}

#[derive(Clone)]
struct DevTaskServer {
    tool_router: ToolRouter<Self>,
    manager: Arc<TaskManager>,
}

impl DevTaskServer {
    fn new(manager: Arc<TaskManager>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            manager,
        }
    }
}

/// 统计计划文档某分区（待办实现/验证事项）的复选框完成度。
fn count_plan_items(plan_path: Option<&str>, section: &str) -> (usize, usize) {
    let Some(path) = plan_path else { return (0, 0) };
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    let mut in_section = false;
    let mut done = 0usize;
    let mut total = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            in_section = rest == section;
            continue;
        }
        if !in_section || !trimmed.starts_with("- [") {
            continue;
        }
        total += 1;
        if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
            done += 1;
        }
    }
    (done, total)
}

/// 本服务全部 MCP 能力清单（能力名/简述/使用方式/读写），供 list_capabilities 展示。
fn capabilities() -> serde_json::Value {
    serde_json::json!([
        {"name": "list_tasks", "rw": "读", "description": "列出所有开发任务（可按状态过滤）", "usage": "list_tasks(status?)"},
        {"name": "get_task", "rw": "读", "description": "查询单个任务详情", "usage": "get_task(session_id)"},
        {"name": "get_progress", "rw": "读", "description": "查询任务进度（阶段 + 待办/验证完成度）", "usage": "get_progress(session_id)"},
        {"name": "list_capabilities", "rw": "读", "description": "列出本服务全部 MCP 能力", "usage": "list_capabilities()"},
        {"name": "create_task", "rw": "写", "description": "创建开发任务（白名单校验）", "usage": "create_task(session_id, group_id, qq_id)"},
        {"name": "confirm_task", "rw": "写", "description": "人工确认计划并开始执行（白名单+归属校验）", "usage": "confirm_task(session_id, qq_id)"},
        {"name": "cancel_task", "rw": "写", "description": "人工取消任务（白名单+归属校验）", "usage": "cancel_task(session_id, qq_id)"},
        {"name": "transition_task", "rw": "写", "description": "任务状态迁移（内容里程碑，确定性迁移表校验）", "usage": "transition_task(session_id, status)"},
        {"name": "update_task_meta", "rw": "写", "description": "回填任务标题/计划路径", "usage": "update_task_meta(session_id, title?, plan_path?)"}
    ])
}

#[tool_router(router = tool_router)]
impl DevTaskServer {
    #[tool(name = "list_tasks", description = "[读] 列出所有开发任务（可按状态过滤），返回 session_id/status/title/group_id/qq_id")]
    async fn list_tasks(&self, Parameters(p): Parameters<ListTasksRequest>) -> String {
        match self.manager.list(p.status.as_deref()) {
            Ok(tasks) => serde_json::to_string(&tasks).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "get_task", description = "[读] 查询单个开发任务详情")]
    async fn get_task(&self, Parameters(p): Parameters<SessionRequest>) -> String {
        match self.manager.get(&p.session_id) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "get_progress", description = "[读] 查询任务进度（阶段 + 待办/验证完成度）")]
    async fn get_progress(&self, Parameters(p): Parameters<SessionRequest>) -> String {
        match self.manager.get(&p.session_id) {
            Ok(t) => {
                let (todo_done, todo_total) = count_plan_items(t.plan_path.as_deref(), "待办实现");
                let (verify_done, verify_total) = count_plan_items(t.plan_path.as_deref(), "验证事项");
                serde_json::to_string(&serde_json::json!({
                    "session_id": t.session_id,
                    "status": t.status,
                    "title": t.title,
                    "plan_path": t.plan_path,
                    "todo": {"done": todo_done, "total": todo_total},
                    "verify": {"done": verify_done, "total": verify_total},
                }))
                .unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "list_capabilities", description = "[读] 列出本服务全部 MCP 能力（能力名/简述/使用方式/读写）")]
    async fn list_capabilities(&self) -> String {
        capabilities().to_string()
    }

    #[tool(name = "create_task", description = "[写] 创建开发任务（白名单校验），初始状态 queued")]
    async fn create_task(&self, Parameters(p): Parameters<CreateTaskRequest>) -> String {
        match self.manager.create_task(&p.session_id, &p.group_id, &p.qq_id) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "confirm_task", description = "[写] 人工确认计划并开始执行（白名单+任务归属校验）")]
    async fn confirm_task(&self, Parameters(p): Parameters<OwnerRequest>) -> String {
        match self.manager.confirm_task(&p.session_id, &p.qq_id) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "cancel_task", description = "[写] 人工取消任务（白名单+任务归属校验）")]
    async fn cancel_task(&self, Parameters(p): Parameters<OwnerRequest>) -> String {
        match self.manager.cancel_task(&p.session_id, &p.qq_id) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "transition_task", description = "[写] 任务状态迁移（内容里程碑，确定性迁移表校验）")]
    async fn transition_task(&self, Parameters(p): Parameters<TransitionRequest>) -> String {
        let Some(status) = DevTaskStatus::from_str(&p.status) else {
            return format!("错误: 未知状态 {}", p.status);
        };
        match self.manager.transition(&p.session_id, status) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "update_task_meta", description = "[写] 回填任务标题/计划路径（子会话计划生成后由网关调用）")]
    async fn update_task_meta(&self, Parameters(p): Parameters<UpdateMetaRequest>) -> String {
        if let Some(title) = &p.title {
            if let Err(e) = self.manager.set_title(&p.session_id, title) {
                return format!("错误: {e}");
            }
        }
        if let Some(plan_path) = &p.plan_path {
            if let Err(e) = self.manager.set_plan_path(&p.session_id, plan_path) {
                return format!("错误: {e}");
            }
        }
        match self.manager.get(&p.session_id) {
            Ok(t) => serde_json::to_string(&t).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DevTaskServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdio 被 MCP 协议占用，日志必须写 stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let sqlite_path =
        std::env::var("SQLITE_PATH").unwrap_or_else(|_| "data/tabletop.db".to_string());
    let storage = Arc::new(storage::Storage::open(&sqlite_path)?);

    let whitelist: std::collections::HashSet<String> = std::env::var("DEV_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let manager = Arc::new(TaskManager::new(storage, whitelist));
    let server = DevTaskServer::new(manager);
    let serving = server.serve(rmcp::transport::stdio()).await?;
    tracing::info!("dev-task-mcp stdio server 已启动");
    serving.waiting().await?;
    Ok(())
}
