//! game-engine-mcp：把 EnginePlugin 映射为 MCP 统一原语，以 stdio 暴露给 agent-gateway。
//!
//! 统一原语：get_rules / start_game / get_active_game / get_state / get_legal_actions / execute_action。
//! 所有写入路径由 GameManager → EnginePlugin 无条件重校验（合法性 + 权限）。

use game_engine_core::{EngineRegistry, GameManager, GameState, GameStatus};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use std::sync::Arc;

// —— 工具参数类型（MCP tool 入参）——
#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct GameIdRequest {
    game_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct GroupRequest {
    group_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct StartGameRequest {
    game_id: String,
    group_id: String,
    players: Vec<String>,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct StateRequest {
    room_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct LegalActionsRequest {
    room_id: String,
    player_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct ExecuteActionRequest {
    room_id: String,
    player_id: String,
    action: String,
}

#[derive(Clone)]
struct EngineServer {
    tool_router: ToolRouter<Self>,
    manager: Arc<GameManager>,
    storage: Arc<storage::Storage>,
}

impl EngineServer {
    fn new(manager: Arc<GameManager>, storage: Arc<storage::Storage>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            manager,
            storage,
        }
    }
}

#[tool_router(router = tool_router)]
impl EngineServer {
    #[tool(name = "get_rules", description = "获取指定游戏的规则说明")]
    async fn get_rules(&self, Parameters(p): Parameters<GameIdRequest>) -> String {
        match self.manager.registry().get(&p.game_id) {
            Some(e) => e.rules(),
            None => format!("错误: 未找到游戏 {}", p.game_id),
        }
    }

    #[tool(name = "start_game", description = "开桌（确定性），返回对局状态")]
    async fn start_game(&self, Parameters(p): Parameters<StartGameRequest>) -> String {
        match self.manager.start_game(&p.game_id, &p.group_id, p.players) {
            Ok(state) => {
                let state_json = serde_json::to_string(&state).unwrap_or_default();
                let payload = serde_json::json!({ "game_id": p.game_id, "group_id": p.group_id }).to_string();
                if let Err(e) = self.storage.record_game_transition(
                    &shared::new_event_id(),
                    &state.game_id,
                    &state.room_id,
                    "game_started",
                    None,
                    &payload,
                    &state_json,
                    1,
                ) {
                    tracing::warn!("写 game_events 失败: {e:#}");
                }
                state_json
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "get_active_game", description = "查询群内当前活跃对局（无则返回 null）")]
    async fn get_active_game(&self, Parameters(p): Parameters<GroupRequest>) -> String {
        match self.manager.get_active_game(&p.group_id) {
            Some(s) => serde_json::to_string(&s).unwrap_or_default(),
            None => "null".to_string(),
        }
    }

    #[tool(name = "get_state", description = "查询对局状态（无则返回 null）")]
    async fn get_state(&self, Parameters(p): Parameters<StateRequest>) -> String {
        match self.manager.get_state(&p.room_id) {
            Some(s) => serde_json::to_string(&s).unwrap_or_default(),
            None => "null".to_string(),
        }
    }

    #[tool(name = "get_legal_actions", description = "查询玩家当前合法动作列表")]
    async fn get_legal_actions(&self, Parameters(p): Parameters<LegalActionsRequest>) -> String {
        match self.manager.legal_actions(&p.room_id, &p.player_id) {
            Ok(actions) => serde_json::to_string(&actions).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "execute_action", description = "执行动作（引擎无条件重校验合法性+权限）")]
    async fn execute_action(&self, Parameters(p): Parameters<ExecuteActionRequest>) -> String {
        match self.manager.execute_action(&p.room_id, &p.player_id, &p.action) {
            Ok(result) => {
                let state_json = serde_json::to_string(&result.state).unwrap_or_default();
                let payload = serde_json::json!({ "action": p.action, "message": result.message }).to_string();
                if let Err(e) = self.storage.record_game_transition(
                    &shared::new_event_id(),
                    &result.state.game_id,
                    &result.state.room_id,
                    "action_executed",
                    Some(&p.player_id),
                    &payload,
                    &state_json,
                    2,
                ) {
                    tracing::warn!("写 game_events 失败: {e:#}");
                }
                serde_json::to_string(&result).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EngineServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdio 被 MCP 协议占用，日志必须写 stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let sqlite_path =
        std::env::var("SQLITE_PATH").unwrap_or_else(|_| "data/tabletop.db".to_string());
    let storage = Arc::new(storage::Storage::open(&sqlite_path)?);

    let mut registry = EngineRegistry::new();
    registry.register(rock_paper_scissors::RpsEngine);
    let manager = Arc::new(GameManager::new(registry));

    // 从 SQLite 恢复活跃对局（引擎重启后内存丢失）
    match storage.load_game_snapshots() {
        Ok(snapshots) => {
            for s in snapshots {
                match serde_json::from_str::<GameState>(&s) {
                    Ok(state) => {
                        let is_active = state.status == GameStatus::Active;
                        manager.restore(state.clone());
                        if is_active {
                            tracing::info!("恢复活跃对局 room={}", state.room_id);
                        }
                    }
                    Err(e) => tracing::warn!("恢复对局快照失败: {e:#}"),
                }
            }
        }
        Err(e) => tracing::warn!("加载对局快照失败: {e:#}"),
    }

    let server = EngineServer::new(manager, storage);
    let serving = server.serve(rmcp::transport::stdio()).await?;
    tracing::info!("game-engine-mcp stdio server 已启动");
    serving.waiting().await?;
    Ok(())
}
