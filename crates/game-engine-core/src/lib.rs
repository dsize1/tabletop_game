//! 桌游引擎核心框架
//!
//! EnginePlugin trait、引擎注册表、对局状态管理（本次内存存储，P3 换 SQLite）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 动作描述（暴露给决策模型选择）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub description: String,
}

/// 对局状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub game_id: String,
    pub room_id: String,
    pub group_id: String,
    pub status: GameStatus,
    pub players: Vec<String>,
    /// 引擎私有状态（JSON，引擎自行解析）。
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    Active,
    Finished,
}

/// 引擎错误。
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("非法动作: {0}")]
    IllegalAction(String),
    #[error("无权限: {0}")]
    PermissionDenied(String),
    #[error("未找到: {0}")]
    NotFound(String),
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 动作执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub state: GameState,
    /// 给玩家/主持人的结果文案。
    pub message: String,
    pub finished: bool,
}

/// 引擎插件 trait：纯逻辑，无状态存储（状态由 GameManager 管理）。
pub trait EnginePlugin: Send + Sync {
    fn game_id(&self) -> &'static str;
    fn rules(&self) -> String;
    /// 开桌：创建初始状态。
    fn new_game(&self, room_id: &str, group_id: &str, players: Vec<String>) -> GameState;
    /// 玩家合法动作。
    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action>;
    /// 执行动作：无条件重校验合法性 + 权限，返回新状态与结果。
    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
    ) -> Result<ActionResult, EngineError>;
}

/// 引擎注册表。
#[derive(Default)]
pub struct EngineRegistry {
    engines: HashMap<String, Arc<dyn EnginePlugin>>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<E: EnginePlugin + 'static>(&mut self, engine: E) {
        let id = engine.game_id().to_string();
        self.engines.insert(id, Arc::new(engine));
    }

    pub fn get(&self, game_id: &str) -> Option<&Arc<dyn EnginePlugin>> {
        self.engines.get(game_id)
    }

    pub fn game_ids(&self) -> Vec<String> {
        self.engines.keys().cloned().collect()
    }
}

/// 对局管理器（内存存储，本次）。
pub struct GameManager {
    registry: Arc<EngineRegistry>,
    rooms: RwLock<HashMap<String, GameState>>,
    active_by_group: RwLock<HashMap<String, String>>,
}

impl GameManager {
    pub fn new(registry: EngineRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
            rooms: RwLock::new(HashMap::new()),
            active_by_group: RwLock::new(HashMap::new()),
        }
    }

    pub fn registry(&self) -> &Arc<EngineRegistry> {
        &self.registry
    }

    /// 开桌。
    pub fn start_game(
        &self,
        game_id: &str,
        group_id: &str,
        players: Vec<String>,
    ) -> Result<GameState, EngineError> {
        let engine = self
            .registry
            .get(game_id)
            .ok_or_else(|| EngineError::NotFound(format!("游戏 {game_id}")))?;

        // 群内已有活跃对局：清理残留后覆盖开新局（猜拳一轮即终局，残留视为悬挂）
        if let Some(old_room) = self.active_by_group.write().unwrap().remove(group_id) {
            self.rooms.write().unwrap().remove(&old_room);
        }

        let room_id = shared::new_event_id();
        let state = engine.new_game(&room_id, group_id, players);

        self.rooms
            .write()
            .unwrap()
            .insert(room_id.clone(), state.clone());
        self.active_by_group
            .write()
            .unwrap()
            .insert(group_id.to_string(), room_id);

        Ok(state)
    }

    /// 从快照恢复一个对局（引擎重启后调用）。
    pub fn restore(&self, state: GameState) {
        if state.status == GameStatus::Active {
            self.active_by_group
                .write()
                .unwrap()
                .insert(state.group_id.clone(), state.room_id.clone());
        }
        self.rooms
            .write()
            .unwrap()
            .insert(state.room_id.clone(), state);
    }

    pub fn get_active_game(&self, group_id: &str) -> Option<GameState> {
        let room_id = self.active_by_group.read().unwrap().get(group_id)?.clone();
        self.rooms.read().unwrap().get(&room_id).cloned()
    }

    pub fn get_state(&self, room_id: &str) -> Option<GameState> {
        self.rooms.read().unwrap().get(room_id).cloned()
    }

    pub fn legal_actions(
        &self,
        room_id: &str,
        player_id: &str,
    ) -> Result<Vec<Action>, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self
            .registry
            .get(&state.game_id)
            .ok_or_else(|| EngineError::NotFound(format!("游戏 {}", state.game_id)))?;
        Ok(engine.legal_actions(&state, player_id))
    }

    pub fn execute_action(
        &self,
        room_id: &str,
        player_id: &str,
        action: &str,
    ) -> Result<ActionResult, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self
            .registry
            .get(&state.game_id)
            .ok_or_else(|| EngineError::NotFound(format!("游戏 {}", state.game_id)))?;

        let result = engine.execute_action(&state, player_id, action)?;

        // 保存新状态；终局则解除群活跃对局登记
        self.rooms
            .write()
            .unwrap()
            .insert(room_id.to_string(), result.state.clone());
        if result.finished {
            self.active_by_group
                .write()
                .unwrap()
                .remove(&state.group_id);
        }
        Ok(result)
    }
}
