//! game-engine-mcp：把 EnginePlugin 映射为 MCP 统一原语，以 stdio 暴露给 agent-gateway。
//!
//! 统一原语：get_rules / get_active_game / get_state / get_player_view / state_summary /
//! get_legal_actions / create_game / start_game / execute_action。
//! 所有写入路径由 GameManager → EnginePlugin 无条件重校验（合法性 + 权限）；
//! 引擎产出的事件逐条落 `game_events`，快照 `version` 对齐最后事件 seq（ADR-0004/0008）。

use game_engine_core::{
    EngineEvent, EngineRegistry, GameManager, GameState, GameStatus, Policy, RandomPolicy,
    SeededRng,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    /// 房间快照版本（`game_state.version`）：内存态与真相源对齐用。
    ///
    /// `game_state` 是真相源，server（Web 动作宿主）会持续写入；MCP 只在建局/lobby
    /// 写入。若不比对版本，`#局面` 会一直读到开局前的旧状态（P6.3 真机问题）。
    versions: Arc<Mutex<HashMap<String, i64>>>,
}

impl EngineServer {
    fn new(manager: Arc<GameManager>, storage: Arc<storage::Storage>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            manager,
            storage,
            versions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 若 SQLite 里的快照比本进程内存态新，就重新 restore（MCP 与 server 交替写入）。
    fn sync_room(&self, room_id: &str) {
        let Some(cur) = self.manager.get_state(room_id) else {
            return;
        };
        let Ok(Some(db)) = self.storage.game_snapshot_version(&cur.game_id, room_id) else {
            return;
        };
        let tracked = self
            .versions
            .lock()
            .ok()
            .and_then(|m| m.get(room_id).copied());
        if tracked.is_some_and(|t| db <= t) {
            return;
        }
        let Ok(Some(json)) = self.storage.get_game_snapshot(&cur.game_id, room_id) else {
            return;
        };
        let Ok(state) = serde_json::from_str::<GameState>(&json) else {
            return;
        };
        self.manager.restore(state);
        if let Ok(mut m) = self.versions.lock() {
            m.insert(room_id.to_string(), db);
        }
        tracing::debug!("已按快照对齐内存态 room={room_id} version={db}");
    }

    /// 把引擎产出的事件逐条写入 `game_events`，快照 version 对齐最后 seq。
    /// 返回最后一条事件的 seq（写失败返回 -1，仅告警）。
    fn persist(&self, state: &GameState, events: &[EngineEvent]) -> i64 {
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
            Ok(seq) => {
                if let Ok(mut m) = self.versions.lock() {
                    m.insert(state.room_id.clone(), seq);
                }
                tracing::debug!("已写入 {} 条引擎事件，version={seq}", records.len());
                seq
            }
            Err(e) => {
                tracing::warn!("写 game_events 失败: {e:#}");
                -1
            }
        }
    }

    /// 决策日志采样（与 server 同口径）：0 关，1 全量，中间按概率。
    fn should_log_decision(&self) -> bool {
        let rate = std::env::var("SGS_DECISION_LOG_RATE")
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        if rate >= 1.0 {
            return true;
        }
        if rate <= 0.0 {
            return false;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        (nanos % 10_000) as f64 / 10_000.0 < rate
    }

    /// 首个决策前写回放基线（同房间只写一份）。
    fn capture_replay_base(&self, state: &GameState) {
        if state.game_id != "sanguosha" {
            return;
        }
        let Ok(state_json) = serde_json::to_string(state) else {
            return;
        };
        let seq = self
            .storage
            .game_snapshot_version(&state.game_id, &state.room_id)
            .ok()
            .flatten()
            .unwrap_or(0);
        if let Err(e) =
            self.storage
                .ensure_replay_base(&state.game_id, &state.room_id, seq, &state_json)
        {
            tracing::warn!("写 replay base 失败（已忽略）: {e:#}");
        }
    }

    /// 写决策日志；失败只告警。
    #[allow(clippy::too_many_arguments)]
    fn log_decision(
        &self,
        game_id: &str,
        room_id: &str,
        seq: i64,
        player_id: &str,
        source: &str,
        legal: &[game_engine_core::Action],
        candidates: Option<&[game_engine_core::Action]>,
        chosen: Option<&str>,
        score: Option<f64>,
        reason: Option<&str>,
        state_digest: Option<&str>,
        latency_ms: Option<i64>,
    ) {
        if game_id != "sanguosha" || !self.should_log_decision() {
            return;
        }
        let legal_json = serde_json::to_string(legal).unwrap_or_else(|_| "[]".into());
        let candidates_json =
            candidates.map(|c| serde_json::to_string(c).unwrap_or_else(|_| "[]".into()));
        let record = storage::DecisionLogRecord {
            game_id,
            room_id,
            seq,
            player_id,
            source,
            legal_actions: &legal_json,
            candidates: candidates_json.as_deref(),
            chosen,
            confidence: score,
            latency_ms,
            reason,
            state_digest,
        };
        if let Err(e) = self.storage.append_decision_log(&record) {
            tracing::warn!("写 decision log 失败（已忽略）: {e:#}");
        }
    }
}

#[tool_router(router = tool_router)]
impl EngineServer {
    #[tool(
        name = "get_small_game_rules",
        description = "查询拉斯维加斯小游戏规则（A1–H2，大小写不敏感）"
    )]
    async fn get_small_game_rules(
        &self,
        Parameters(p): Parameters<SmallGameRuleRequest>,
    ) -> String {
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
        match self
            .manager
            .create_game(&p.game_id, &p.group_id, &p.host_id)
        {
            Ok((state, events)) => {
                self.persist(&state, &events);
                serde_json::to_string(&state).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "start_game",
        description = "直接开桌（无 lobby 的简单引擎），返回对局状态"
    )]
    async fn start_game(&self, Parameters(p): Parameters<StartGameRequest>) -> String {
        match self.manager.start_game(&p.game_id, &p.group_id, p.players) {
            Ok((state, events)) => {
                self.persist(&state, &events);
                serde_json::to_string(&state).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "get_active_game",
        description = "查询群内当前活跃对局（含 lobby；无则返回 null）"
    )]
    async fn get_active_game(&self, Parameters(p): Parameters<GroupRequest>) -> String {
        match self.manager.get_active_game(&p.group_id) {
            Some(s) => serde_json::to_string(&s).unwrap_or_default(),
            None => "null".to_string(),
        }
    }

    #[tool(
        name = "get_state",
        description = "查询对局状态（公共视图，已脱敏；无则返回 null）"
    )]
    async fn get_state(&self, Parameters(p): Parameters<StateRequest>) -> String {
        self.sync_room(&p.room_id);
        match self.manager.public_state(&p.room_id) {
            Ok(s) => serde_json::to_string(&s).unwrap_or_default(),
            Err(_) => "null".to_string(),
        }
    }

    #[tool(
        name = "get_player_view",
        description = "查询玩家个人视图（默认与公共视图一致）"
    )]
    async fn get_player_view(&self, Parameters(p): Parameters<PlayerViewRequest>) -> String {
        self.sync_room(&p.room_id);
        match self.manager.player_view(&p.room_id, &p.player_id) {
            Ok(s) => serde_json::to_string(&s).unwrap_or_default(),
            Err(_) => "null".to_string(),
        }
    }

    #[tool(name = "get_legal_actions", description = "查询玩家当前合法动作列表")]
    async fn get_legal_actions(&self, Parameters(p): Parameters<LegalActionsRequest>) -> String {
        self.sync_room(&p.room_id);
        match self.manager.legal_actions(&p.room_id, &p.player_id) {
            Ok(actions) => serde_json::to_string(&actions).unwrap_or_default(),
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "state_summary",
        description = "查询对局局面摘要（紧凑文本，给主持人）"
    )]
    async fn state_summary(&self, Parameters(p): Parameters<StateRequest>) -> String {
        self.sync_room(&p.room_id);
        match self.manager.state_summary(&p.room_id) {
            Ok(s) => s,
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "bot_action",
        description = "让内置 AI 替指定玩家行动（拉斯维加斯启发式 / 三国杀行为树，其余随机；测试与机器人补位）"
    )]
    async fn bot_action(&self, Parameters(p): Parameters<LegalActionsRequest>) -> String {
        self.sync_room(&p.room_id);
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
        self.capture_replay_base(&state);
        let started = std::time::Instant::now();
        let mut rng = SeededRng::new(0);
        // 三国杀：记录决策轨迹（P6.4）；其余引擎保持原策略。
        let mut trace = None;
        let chosen = match state.game_id.as_str() {
            "las-vegas" => {
                las_vegas::ai::HeuristicPolicy.choose(&state, &p.player_id, &actions, &mut rng)
            }
            "sanguosha" => {
                let t = sanguosha::ai::decide_traced(&state, &p.player_id, &actions);
                trace = t.clone();
                t.map(|t| t.action)
            }
            _ => RandomPolicy.choose(&state, &p.player_id, &actions, &mut rng),
        };
        let Some(action) = chosen else {
            return "错误: 无可用动作".to_string();
        };
        match self
            .manager
            .execute_action(&p.room_id, &p.player_id, &action)
        {
            Ok(result) => {
                let seq = self.persist(&result.state, &result.events);
                if let Some(t) = trace {
                    self.log_decision(
                        &result.state.game_id,
                        &p.room_id,
                        seq,
                        &p.player_id,
                        "bot",
                        &t.legal,
                        Some(&t.candidates),
                        Some(&action),
                        Some(t.score),
                        Some(&t.reason),
                        Some(&t.state_digest),
                        Some(started.elapsed().as_millis() as i64),
                    );
                }
                serde_json::to_string(&result).unwrap_or_default()
            }
            Err(e) => format!("错误: {e}"),
        }
    }

    #[tool(
        name = "execute_action",
        description = "执行动作（引擎无条件重校验合法性+权限）"
    )]
    async fn execute_action(&self, Parameters(p): Parameters<ExecuteActionRequest>) -> String {
        self.sync_room(&p.room_id);
        let before = self.manager.get_state(&p.room_id);
        if let Some(st) = &before {
            self.capture_replay_base(st);
        }
        let started = std::time::Instant::now();
        let (legal, digest) = match &before {
            Some(st) if st.game_id == "sanguosha" => {
                let legal = self
                    .manager
                    .legal_actions(&p.room_id, &p.player_id)
                    .unwrap_or_default();
                let digest = sanguosha::SgsState::parse(&st.data)
                    .ok()
                    .map(|s| sanguosha::ai::state_digest(&s));
                (legal, digest)
            }
            _ => (Vec::new(), None),
        };
        match self
            .manager
            .execute_action(&p.room_id, &p.player_id, &p.action)
        {
            Ok(result) => {
                let seq = self.persist(&result.state, &result.events);
                self.log_decision(
                    &result.state.game_id,
                    &p.room_id,
                    seq,
                    &p.player_id,
                    "human",
                    &legal,
                    None,
                    Some(&p.action),
                    None,
                    Some("human_action"),
                    digest.as_deref(),
                    Some(started.elapsed().as_millis() as i64),
                );
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
    registry.register(sanguosha::SanguoshaEngine);
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine_core::{EngineEvent, GameStatus};

    fn server_with_sgs() -> (EngineServer, Arc<GameManager>, Arc<storage::Storage>) {
        let storage = Arc::new(storage::Storage::open(":memory:").expect("storage"));
        let mut registry = EngineRegistry::new();
        registry.register(sanguosha::SanguoshaEngine);
        let manager = Arc::new(GameManager::with_rng(registry, Box::new(SeededRng::new(7))));
        let server = EngineServer::new(manager.clone(), storage.clone());
        (server, manager, storage)
    }

    /// 回归（P6.3 真机）：MCP 在 lobby 写、server 打牌写 → MCP 必须按快照版本对齐，
    /// 否则 `#局面`/`get_state` 一直读到开局前的旧状态。
    #[test]
    fn sync_room_follows_newer_snapshot() {
        let (server, manager, storage) = server_with_sgs();
        let (lobby, _) = manager
            .create_game("sanguosha", "g1", "u1")
            .expect("create");
        let room = lobby.room_id.clone();

        // 造一份「更新」的状态（加入机器人 + 开赛 → setup），并写入快照（version 1）。
        for _ in 0..4 {
            manager
                .execute_action(&room, "u1", "lobby:add_bot")
                .expect("add bot");
        }
        manager
            .execute_action(&room, "u1", "lobby:start")
            .expect("start");
        let setup = manager.get_state(&room).expect("setup state");
        let json = serde_json::to_string(&setup).unwrap();
        let ev = [storage::GameEventRecord {
            event_id: "ev-sync-1",
            event_type: "lobby_start",
            actor: Some("u1"),
            payload: "{}",
        }];
        storage
            .append_game_events("sanguosha", &room, &ev, &json)
            .expect("persist snapshot");

        // 把内存态退回旧 lobby（模拟 server 在别处推进过、MCP 落伍）。
        manager.restore(lobby.clone());
        let stale = manager.get_state(&room).expect("stale");
        assert_eq!(stale.status, GameStatus::Active);

        // 同步后应看到 setup（座位数 5、选将中）。
        server.sync_room(&room);
        let now = manager.get_state(&room).expect("synced");
        let st = sanguosha::SgsState::parse(&now.data).expect("parse");
        assert_eq!(st.phase, sanguosha::Phase::Setup, "应已同步到选将阶段");
        assert_eq!(st.seats.len(), 5);
    }

    /// 反向保护：版本没变时不做多余 restore（避免把内存里更新的选择丢掉）。
    #[test]
    fn sync_room_keeps_memory_when_version_matches() {
        let (server, manager, storage) = server_with_sgs();
        let (lobby, _) = manager
            .create_game("sanguosha", "g2", "u1")
            .expect("create");
        let room = lobby.room_id.clone();
        let json = serde_json::to_string(&lobby).unwrap();
        let ev = [storage::GameEventRecord {
            event_id: "ev-sync-2",
            event_type: "game_created",
            actor: Some("u1"),
            payload: "{}",
        }];
        storage
            .append_game_events("sanguosha", &room, &ev, &json)
            .expect("persist");
        server.sync_room(&room); // 第一次对齐，记下版本
                                 // 内存里继续加机器人（比快照更新），版本未变 → 不能被快照覆盖回去。
        manager
            .execute_action(&room, "u1", "lobby:add_bot")
            .expect("add bot");
        server.sync_room(&room);
        let now = manager.get_state(&room).expect("state");
        let st = sanguosha::SgsState::parse(&now.data).expect("parse");
        assert_eq!(st.seats.len(), 2, "版本未变时不应回滚内存态");
    }

    #[test]
    fn persist_records_version() {
        let (server, manager, _storage) = server_with_sgs();
        let (lobby, _) = manager
            .create_game("sanguosha", "g3", "u1")
            .expect("create");
        let room = lobby.room_id.clone();
        let events = vec![EngineEvent::new("ping", serde_json::json!({}))];
        let state = manager.get_state(&room).expect("state");
        server.persist(&state, &events);
        assert!(
            server
                .versions
                .lock()
                .unwrap()
                .get(&room)
                .copied()
                .unwrap_or_default()
                > 0
        );
    }
}
