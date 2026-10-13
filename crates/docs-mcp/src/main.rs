//! docs-mcp：本地开发文档预览的只读 MCP 原语（stdio 暴露给 agent-gateway）。
//!
//! 职责：白名单校验后返回「全部文档根」的授权资源标识（`docs:all`），
//! 由网关签发短期 token 并拼接短链；实际的树/文件读取由 server 资源 API 完成。
//!
//! 原语：
//! - `prepare_docs_link(qq_id)`  只读：DEV_WHITELIST 校验 → `{resource, roots[]}`
//! - `list_capabilities()`       只读：本服务能力描述符
//!
//! 范围：只读浏览。**搜索功能本期未实现**
//! TODO(search): 后续如需，新增文件名搜索 / 正文 grep 原语（仍只读、仍走白名单）。

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use std::collections::HashSet;

/// 单一资源范围：一枚 token 覆盖全部配置根（省事看全部根）。
const DOCS_RESOURCE: &str = "docs:all";

/// 工具参数：调用者 openid（member_openid）。
#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct OwnerRequest {
    qq_id: String,
}

#[derive(Clone)]
struct DocsServer {
    tool_router: ToolRouter<Self>,
    whitelist: HashSet<String>,
    roots: Vec<shared::docs_config::DocsRoot>,
}

impl DocsServer {
    fn new(whitelist: HashSet<String>, roots: Vec<shared::docs_config::DocsRoot>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            whitelist,
            roots,
        }
    }
}

/// 本服务全部 MCP 能力清单（能力名/简述/使用方式/读写）。
fn capabilities() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "prepare_docs_link",
            "rw": "读",
            "description": "校验开发者白名单，返回全部文档根的授权资源（token 由网关签发）",
            "usage": "prepare_docs_link(qq_id)",
            "example": "#文档"
        },
        {
            "name": "list_capabilities",
            "rw": "读",
            "description": "列出本服务全部 MCP 能力",
            "usage": "list_capabilities()",
            "example": "（网关内部；用户用 #能力）"
        }
    ])
}

#[tool_router(router = tool_router)]
impl DocsServer {
    #[tool(
        name = "prepare_docs_link",
        description = "[读] 校验开发者白名单，返回全部文档根的授权资源 docs:all（网关据此签发 token）"
    )]
    async fn prepare_docs_link(&self, Parameters(p): Parameters<OwnerRequest>) -> String {
        if !self.whitelist.contains(p.qq_id.trim()) {
            return "错误: 无权限：调用者不在开发者白名单".to_string();
        }
        let roots: Vec<&str> = self.roots.iter().map(|r| r.name.as_str()).collect();
        serde_json::to_string(&serde_json::json!({
            "resource": DOCS_RESOURCE,
            "roots": roots,
        }))
        .unwrap_or_default()
    }

    #[tool(
        name = "list_capabilities",
        description = "[读] 列出本服务全部 MCP 能力（能力名/简述/使用方式/读写）"
    )]
    async fn list_capabilities(&self) -> String {
        capabilities().to_string()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DocsServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdio 被 MCP 协议占用，日志必须写 stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let whitelist: HashSet<String> = std::env::var("DEV_WHITELIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // roots 与 server 资源 API 共读同一份配置（config/dev-docs.json）
    let config = shared::docs_config::load();
    let server = DocsServer::new(whitelist, config.roots);
    let serving = server.serve(rmcp::transport::stdio()).await?;
    tracing::info!("docs-mcp stdio server 已启动");
    serving.waiting().await?;
    Ok(())
}
