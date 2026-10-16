//! 桌游引擎核心框架
//!
//! EnginePlugin trait、引擎注册表、对局状态管理（本次内存存储，P3 换 SQLite）。
//!
//! 关键能力（ADR-0008）：
//! - 引擎产出一串 `EngineEvent`（事件溯源由引擎自述；随机结果入事件）
//! - `RandomSource` 可注入（`SeededRng` 测试确定性 / `SystemRng` 默认）
//! - `public_state` / `player_view` 提供脱敏视图
//! - `GameManager::create_game` 创建 lobby（群内已有 Active 则拒绝）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// 动作描述（暴露给决策模型选择）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub description: String,
}

/// 引擎产出的事件（写入 `game_events`，append-only）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub event_type: String,
    pub actor: Option<String>,
    pub payload: serde_json::Value,
}

impl EngineEvent {
    pub fn new(event_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.into(),
            actor: None,
            payload,
        }
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }
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

/// 局面摘要送入主持人 LLM 的默认字符上限；超出由 `GameManager::state_summary` 截断。
pub const SUMMARY_MAX_CHARS: usize = 300;

/// 按「字符」截断（UTF-8 安全），超限追加省略号。
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
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
    /// 本次动作（含自动连锁）产出的事件序列。
    #[serde(default)]
    pub events: Vec<EngineEvent>,
}

// ---------------------------------------------------------------------------
// 可注入随机源
// ---------------------------------------------------------------------------

/// 对象安全的随机源：只暴露 `next_u64`，其余由自由函数实现。
pub trait RandomSource: Send + Sync {
    fn next_u64(&mut self) -> u64;
}

/// 确定性随机源（xorshift64*），供测试与复盘重放。
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
        }
    }
}

impl RandomSource for SeededRng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// 系统随机源（默认）。
pub struct SystemRng;

impl RandomSource for SystemRng {
    fn next_u64(&mut self) -> u64 {
        rand::random()
    }
}

/// 掷一个六面骰（1..=6）。
pub fn roll_die(rng: &mut dyn RandomSource) -> u8 {
    (rng.next_u64() % 6) as u8 + 1
}

/// 原地洗牌（Fisher-Yates，基于 `next_u64`）。
pub fn shuffle<T>(rng: &mut dyn RandomSource, items: &mut [T]) {
    let n = items.len();
    if n < 2 {
        return;
    }
    for i in (1..n).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

/// 从 `0..n` 均匀取一个下标（n=0 返回 0）。
pub fn pick_index(rng: &mut dyn RandomSource, n: usize) -> usize {
    if n == 0 {
        0
    } else {
        (rng.next_u64() % n as u64) as usize
    }
}

/// 引擎插件 trait：纯逻辑，无状态存储（状态由 GameManager 管理）。
pub trait EnginePlugin: Send + Sync {
    fn game_id(&self) -> &'static str;
    fn rules(&self) -> String;

    /// 开桌/建局：创建初始状态并产出开局事件。
    ///
    /// lobby 型引擎以 `players[0]` 为房主/发起人（由 `GameManager::create_game` 保证）。
    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>);

    /// 玩家合法动作。
    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action>;

    /// 执行动作：无条件重校验合法性 + 权限，返回新状态、结果文案与事件序列。
    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError>;

    /// 公共视图（脱敏后返回给 `get_state`）；默认直通。
    fn public_state(&self, state: &GameState) -> GameState {
        state.clone()
    }

    /// 玩家个人视图；默认与公共视图一致。
    fn player_view(&self, state: &GameState, _viewer: &str) -> GameState {
        self.public_state(state)
    }

    /// 局面摘要：给主持人 LLM 的**紧凑文本视图**（不要 JSON）。
    ///
    /// 默认实现只给「游戏 + 状态 + 玩家人数」；引擎应覆盖为更贴切的摘要
    /// （谁在等谁、关键资源/分数等）。由 `GameManager::state_summary` 统一截断到
    /// `SUMMARY_MAX_CHARS`。
    fn summarize(&self, state: &GameState) -> String {
        let status = match state.status {
            GameStatus::Active => "进行中",
            GameStatus::Finished => "已结束",
        };
        format!("{}（{}）：玩家 {} 人", self.game_id(), status, state.players.len())
    }
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
    rng: Mutex<Box<dyn RandomSource>>,
}

impl GameManager {
    pub fn new(registry: EngineRegistry) -> Self {
        Self::with_rng(registry, Box::new(SystemRng))
    }

    /// 注入随机源（测试用固定序列）。
    pub fn with_rng(registry: EngineRegistry, rng: Box<dyn RandomSource>) -> Self {
        Self {
            registry: Arc::new(registry),
            rooms: RwLock::new(HashMap::new()),
            active_by_group: RwLock::new(HashMap::new()),
            rng: Mutex::new(rng),
        }
    }

    pub fn registry(&self) -> &Arc<EngineRegistry> {
        &self.registry
    }

    fn engine_for(&self, game_id: &str) -> Result<Arc<dyn EnginePlugin>, EngineError> {
        self.registry
            .get(game_id)
            .cloned()
            .ok_or_else(|| EngineError::NotFound(format!("游戏 {game_id}")))
    }

    fn register_room(&self, state: &GameState) {
        self.rooms
            .write()
            .unwrap()
            .insert(state.room_id.clone(), state.clone());
        if state.status == GameStatus::Active {
            self.active_by_group
                .write()
                .unwrap()
                .insert(state.group_id.clone(), state.room_id.clone());
        }
    }

    /// 创建对局（lobby）：群内已有 Active（含 lobby）则拒绝。
    pub fn create_game(
        &self,
        game_id: &str,
        group_id: &str,
        host_id: &str,
    ) -> Result<(GameState, Vec<EngineEvent>), EngineError> {
        if self
            .active_by_group
            .read()
            .unwrap()
            .contains_key(group_id)
        {
            return Err(EngineError::IllegalAction(
                "该群已有进行中的对局".to_string(),
            ));
        }
        let engine = self.engine_for(game_id)?;
        let room_id = shared::new_event_id();
        let (state, events) = {
            let mut rng = self.rng.lock().unwrap();
            engine.new_game(&room_id, group_id, vec![host_id.to_string()], &mut **rng)
        };
        self.register_room(&state);
        Ok((state, events))
    }

    /// 直接开桌（无 lobby 的简单引擎，如 rps）；群内已有 Active 则拒绝。
    pub fn start_game(
        &self,
        game_id: &str,
        group_id: &str,
        players: Vec<String>,
    ) -> Result<(GameState, Vec<EngineEvent>), EngineError> {
        if self
            .active_by_group
            .read()
            .unwrap()
            .contains_key(group_id)
        {
            return Err(EngineError::IllegalAction(
                "该群已有进行中的对局".to_string(),
            ));
        }
        let engine = self.engine_for(game_id)?;
        let room_id = shared::new_event_id();
        let (state, events) = {
            let mut rng = self.rng.lock().unwrap();
            engine.new_game(&room_id, group_id, players, &mut **rng)
        };
        self.register_room(&state);
        Ok((state, events))
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

    /// 完整状态（引擎内部/公共视图的源）。
    pub fn get_state(&self, room_id: &str) -> Option<GameState> {
        self.rooms.read().unwrap().get(room_id).cloned()
    }

    /// 公共视图（脱敏）。
    pub fn public_state(&self, room_id: &str) -> Result<GameState, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self.engine_for(&state.game_id)?;
        Ok(engine.public_state(&state))
    }

    /// 玩家个人视图。
    pub fn player_view(&self, room_id: &str, player_id: &str) -> Result<GameState, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self.engine_for(&state.game_id)?;
        Ok(engine.player_view(&state, player_id))
    }

    pub fn legal_actions(
        &self,
        room_id: &str,
        player_id: &str,
    ) -> Result<Vec<Action>, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self.engine_for(&state.game_id)?;
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
        let engine = self.engine_for(&state.game_id)?;

        let result = {
            let mut rng = self.rng.lock().unwrap();
            engine.execute_action(&state, player_id, action, &mut **rng)?
        };

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

    /// 局面摘要（给主持人）：引擎自定义 `summarize`，此处统一截断到 `SUMMARY_MAX_CHARS`。
    pub fn state_summary(&self, room_id: &str) -> Result<String, EngineError> {
        let state = self
            .get_state(room_id)
            .ok_or_else(|| EngineError::NotFound(format!("对局 {room_id}")))?;
        let engine = self.engine_for(&state.game_id)?;
        Ok(truncate_chars(&engine.summarize(&state), SUMMARY_MAX_CHARS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct LongEngine;

    impl EnginePlugin for LongEngine {
        fn game_id(&self) -> &'static str {
            "long"
        }
        fn rules(&self) -> String {
            String::new()
        }
        fn new_game(
            &self,
            room_id: &str,
            group_id: &str,
            players: Vec<String>,
            _rng: &mut dyn RandomSource,
        ) -> (GameState, Vec<EngineEvent>) {
            (
                GameState {
                    game_id: "long".into(),
                    room_id: room_id.into(),
                    group_id: group_id.into(),
                    status: GameStatus::Active,
                    players,
                    data: json!({}),
                },
                vec![EngineEvent::new("game_created", json!({}))],
            )
        }
        fn legal_actions(&self, _s: &GameState, _p: &str) -> Vec<Action> {
            vec![]
        }
        fn execute_action(
            &self,
            _s: &GameState,
            _p: &str,
            _a: &str,
            _rng: &mut dyn RandomSource,
        ) -> Result<ActionResult, EngineError> {
            Err(EngineError::Internal("n/a".into()))
        }
        fn summarize(&self, _s: &GameState) -> String {
            "甲".repeat(400)
        }
    }

    fn long_manager() -> GameManager {
        let mut reg = EngineRegistry::new();
        reg.register(LongEngine);
        GameManager::new(reg)
    }

    #[test]
    fn truncate_chars_is_utf8_safe() {
        assert_eq!(truncate_chars("abc", 5), "abc");
        let s = truncate_chars(&"甲".repeat(400), SUMMARY_MAX_CHARS);
        assert_eq!(s.chars().count(), SUMMARY_MAX_CHARS);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn state_summary_truncates_engine_output() {
        let mgr = long_manager();
        let (st, _) = mgr.start_game("long", "g", vec!["p1".into()]).unwrap();
        let sum = mgr.state_summary(&st.room_id).unwrap();
        assert_eq!(sum.chars().count(), SUMMARY_MAX_CHARS);
    }

    #[test]
    fn state_summary_missing_room_errors() {
        let mgr = GameManager::new(EngineRegistry::new());
        assert!(mgr.state_summary("nope").is_err());
    }

    #[test]
    fn create_game_rejects_duplicate_active() {
        let mgr = long_manager();
        let (_st, events) = mgr.create_game("long", "g", "host").unwrap();
        assert_eq!(events.len(), 1);
        // 同一群再次创建被拒绝
        assert!(mgr.create_game("long", "g", "host2").is_err());
    }

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut a = SeededRng::new(42);
        let mut b = SeededRng::new(42);
        let xa: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let xb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(xa, xb);
        assert_ne!(xa[0], xa[1]);
    }

    #[test]
    fn shuffle_is_deterministic_for_seed() {
        let mut a = SeededRng::new(7);
        let mut b = SeededRng::new(7);
        let mut va: Vec<u8> = (1..=9).collect();
        let mut vb: Vec<u8> = (1..=9).collect();
        shuffle(&mut a, &mut va);
        shuffle(&mut b, &mut vb);
        assert_eq!(va, vb);
        let mut sorted = va.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=9).collect::<Vec<u8>>());
    }

    #[test]
    fn roll_die_in_range() {
        let mut rng = SeededRng::new(1);
        for _ in 0..100 {
            let d = roll_die(&mut rng);
            assert!((1..=6).contains(&d));
        }
    }

    #[test]
    fn seeded_rng_single_run_snapshot_matches_restore() {
        // 固定种子的两次独立运行应得到一致序列（快照+重放的基础）。
        let mut r1 = SeededRng::new(2026);
        let mut r2 = SeededRng::new(2026);
        assert_eq!(roll_die(&mut r1), roll_die(&mut r2));
    }
}
