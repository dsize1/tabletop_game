//! game-engine-mcp：把 EnginePlugin 映射为 MCP 统一原语，以 stdio 暴露给 agent-gateway。
//!
//! 统一原语：get_rules / get_active_game / get_state / get_player_view / state_summary /
//! get_legal_actions / create_game / start_game / execute_action。
//! 所有写入路径由 GameManager → EnginePlugin 无条件重校验（合法性 + 权限）；
//! 引擎产出的事件逐条落 `game_events`，快照 `version` 对齐最后事件 seq（ADR-0004/0008）。

use game_engine_core::{
    EngineEvent, EngineRegistry, GameManager, GameState, GameStatus, Policy, RandomPolicy, SeededRng,
};
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
struct CreateGameRequest {
    game_id: String,
    group_id: String,
    host_id: String,
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
struct PlayerViewRequest {
    room_id: String,
    player_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct LegalActionsRequest {
    room_id: String,
    player_id: String,
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct SmallGameRuleRequest {
    small_game_id: String,
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

    /// 把引擎产出的事件逐条写入 `game_events`，快照 version 对齐最后 seq。
    fn persist(&self, state: &GameState, events: &[EngineEvent]) {
        let state_json = serde_json::to_string(state).unwrap_or_default();
        let event_ids: Vec<String> = events.iter().map(|_| shared::new_event_id()).collect();
        let payloads: Vec<String> = events
            .iter()
            .map(|e| serde_json::to_string(&e.payload).unwrap_or_else(|_| "null".into()))
            .collect();
        let records: Vec<storage::GameEventRecord<'_>> = events
            .iter()
            .enumerate()
            .map(|(i, e)| storage::GameEventRecord {
                event_id: &event_ids[i],
                event_type: &e.event_type,
                actor: e.actor.as_deref(),
                payload: &payloads[i],
            })
            .collect();
        match self
            .storage
            .append_game_events(&state.game_id, &state.room_id, &records, &state_json)
        {
            Ok(seq) => tracing::debug!("已写入 {} 条引擎事件，version={seq}", records.len()),
            Err(e) => tracing::warn!("写 game_events 失败: {e:#}"),
        }
    }
}

#[tool_router(router = tool_router)]
impl EngineServer {
    #[tool(
        name = "get_small_game_rules",
        description = "查询拉斯维加斯小游戏规则（A1–H2，大小写不敏感）"
    )]
    async fn get_small_game_rules(&self, Parameters(p): Parameters<SmallGameRuleRequest>) -> String {
        let id = p.small_game_id.trim().to_ascii_uppercase();
        match las_vegas::sg::rule_text(&id) {
            Some(t) => format!("{id}：{t}"),
            None => format!("错误: 未知小游戏 {}（可用 A1–H2）", p.small_game_id),
        }
    }

    #[tool(name = "get_rules", description = "获取指定游戏的规则说明")]
    async fn get_rules(&self, Parameters(p): Parameters<GameIdRequest>) -> String {
        match self.manager.registry().get(&p.game_id) {
            Some(e) => e.rules(),
            None => format!("错误: 未找到游戏 {}", p.game_id),
        }
    }

    #[tool(
        name = "create_game",
        description = "创建对局（进入 lobby，发起人自动入局；群内已有 Active 则拒绝）"
    )]
    async fn create_game(&self, Parameters(p): Parameters<CreateGameRequest>) -> String {
        match self.manager.create_game(&p.game_id, &p.group_id, &p.host_id) {
            Ok((state, events)) => {
                self.persist(&state, &events);
                serde_json::to_string(&state).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "start_game", description = "直接开桌（无 lobby 的简单引擎），返回对局状态")]
    async fn start_game(&self, Parameters(p): Parameters<StartGameRequest>) -> String {
        match self.manager.start_game(&p.game_id, &p.group_id, p.players) {
            Ok((state, events)) => {
                self.persist(&state, &events);
                serde_json::to_string(&state).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "get_active_game", description = "查询群内当前活跃对局（含 lobby；无则返回 null）")]
    async fn get_active_game(&self, Parameters(p): Parameters<GroupRequest>) -> String {
        match self.manager.get_active_game(&p.group_id) {
            Some(s) => serde_json::to_string(&s).unwrap_or_default(),
            None => "null".to_string(),
        }
    }

    #[tool(name = "get_state", description = "查询对局状态（公共视图，已脱敏；无则返回 null）")]
    async fn get_state(&self, Parameters(p): Parameters<StateRequest>) -> String {
        match self.manager.public_state(&p.room_id) {
            Ok(s) => serde_json::to_string(&s).unwrap_or_default(),
            Err(_) => "null".to_string(),
        }
    }

    #[tool(name = "get_player_view", description = "查询玩家个人视图（默认与公共视图一致）")]
    async fn get_player_view(&self, Parameters(p): Parameters<PlayerViewRequest>) -> String {
        match self.manager.player_view(&p.room_id, &p.player_id) {
            Ok(s) => serde_json::to_string(&s).unwrap_or_default(),
            Err(_) => "null".to_string(),
        }
    }

    #[tool(name = "get_legal_actions", description = "查询玩家当前合法动作列表")]
    async fn get_legal_actions(&self, Parameters(p): Parameters<LegalActionsRequest>) -> String {
        match self.manager.legal_actions(&p.room_id, &p.player_id) {
            Ok(actions) => serde_json::to_string(&actions).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "state_summary", description = "查询对局局面摘要（紧凑文本，给主持人）")]
    async fn state_summary(&self, Parameters(p): Parameters<StateRequest>) -> String {
        match self.manager.state_summary(&p.room_id) {
            Ok(s) => s,
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "bot_action",
        description = "让内置 AI（拉斯维加斯启发式）替指定玩家行动（测试/机器人补位）"
    )]
    async fn bot_action(&self, Parameters(p): Parameters<LegalActionsRequest>) -> String {
        let Some(state) = self.manager.get_state(&p.room_id) else {
            return "错误: 未找到对局".to_string();
        };
        let actions = match self.manager.legal_actions(&p.room_id, &p.player_id) {
            Ok(a) => a,
            Err(e) => return format!("错误: {e}"),
        };
        if actions.is_empty() {
            return "错误: 该玩家当前没有合法动作".to_string();
        }
        let mut rng = SeededRng::new(0);
        let chosen = if state.game_id == "las-vegas" {
            las_vegas::ai::HeuristicPolicy.choose(&state, &p.player_id, &actions, &mut rng)
        } else {
            RandomPolicy.choose(&state, &p.player_id, &actions, &mut rng)
        };
        let Some(action) = chosen else {
            return "错误: 无可用动作".to_string();
        };
        match self.manager.execute_action(&p.room_id, &p.player_id, &action) {
            Ok(result) => {
                self.persist(&result.state, &result.events);
                serde_json::to_string(&result).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(name = "execute_action", description = "执行动作（引擎无条件重校验合法性+权限）")]
    async fn execute_action(&self, Parameters(p): Parameters<ExecuteActionRequest>) -> String {
        match self.manager.execute_action(&p.room_id, &p.player_id, &p.action) {
            Ok(result) => {
                self.persist(&result.state, &result.events);
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
    registry.register(las_vegas::LasVegasEngine);
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
