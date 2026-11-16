//! 引擎接线（V3 接管 `game_id = "sanguosha"`；v2 已从注册表移除）。
//!
//! 三层：
//! 1. `EngineState`：`GameState.data` 的载荷 = 内核 `Game` + 大厅/选将/配置（单一事实源，可序列化）；
//! 2. 大厅/选将：动作 id 与 v1/v2 完全一致（网关 `#加入`/`#开始`/`#加机器人`/`#武将包` + Web 选将页）；
//! 3. 视图：输出 v1/v2 同形 `SgsViewData`（Web 三分区页零改动），
//!    隐藏信息只在 `player_area`（constitution I3）。
//!
//! 本阶段**不挂载武将技能**：目标是 B0 前置项之后「武将/手牌/装备/锦囊」完整可玩，
//! 技能按 B0–B20 批次再逐个接入（`skills::all()` 注册表驱动）。

pub mod bot;
pub mod lobby;
pub mod text;
pub mod view;

use serde::{Deserialize, Serialize};

use game_engine_core::{
    Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus,
    RandomSource,
};

use crate::kernel::Game;

pub use lobby::{Lobby, LobbyConfig, Setup};

/// 正式引擎 id：V3 接管（v2 不再注册）。
pub const GAME_ID: &str = "sanguosha";

/// `GameState.data` 的载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    pub kernel: Game,
    #[serde(default)]
    pub cfg: LobbyConfig,
    #[serde(default)]
    pub lobby: Option<Lobby>,
    #[serde(default)]
    pub setup: Option<Setup>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            kernel: Game::default(),
            cfg: LobbyConfig::default(),
            lobby: None,
            setup: None,
        }
    }
}

impl EngineState {
    pub fn from_state(state: &GameState) -> Result<Self, EngineError> {
        serde_json::from_value(state.data.clone())
            .map_err(|e| EngineError::Internal(format!("引擎状态解析失败：{e}")))
    }

    pub fn finished(&self) -> bool {
        self.kernel.finished()
    }

    pub fn to_state(&self, room_id: &str, group_id: &str) -> GameState {
        let status = if self.finished() {
            GameStatus::Finished
        } else {
            GameStatus::Active
        };
        GameState {
            game_id: GAME_ID.to_string(),
            room_id: room_id.to_string(),
            group_id: group_id.to_string(),
            status,
            players: self.kernel.seats.iter().map(|s| s.name.clone()).collect(),
            data: serde_json::to_value(self).unwrap_or_default(),
        }
    }
}

/// V3 引擎插件。
pub struct SanguoshaEngine;

fn new_lobby_state(players: &[String]) -> EngineState {
    let mut kernel = crate::kernel::turn::new_game(players.len().max(1));
    for (i, p) in players.iter().enumerate() {
        kernel.seats[i].name = p.clone();
    }
    kernel.host = players.first().cloned();
    // 大厅阶段：未选将 → 体力/上限为 0（视图投影为 null）。
    for s in kernel.seats.iter_mut() {
        s.hp = 0;
        s.max_hp = 0;
        s.alive = true;
    }
    EngineState {
        kernel,
        cfg: LobbyConfig::default(),
        lobby: players.first().map(|h| Lobby { host: h.clone() }),
        setup: None,
    }
}

fn engine_events(events: &[crate::kernel::event::Event], from: usize) -> Vec<EngineEvent> {
    events
        .get(from..)
        .unwrap_or(&[])
        .iter()
        .map(|ev| {
            let actor = ev.seat.map(|s| s.to_string());
            let mut e = EngineEvent::new(
                format!("{:?}", ev.kind),
                serde_json::to_value(ev).unwrap_or_default(),
            );
            e.actor = actor;
            e
        })
        .collect()
}

impl EnginePlugin for SanguoshaEngine {
    fn game_id(&self) -> &'static str {
        GAME_ID
    }

    fn rules(&self) -> String {
        "三国杀（内核 V3）：回合驱动 + 事件生产 + 交互 pending + 完整卡牌/装备结算；\
         技能按 B0–B20 批次接入（当前批次前不依赖武将技能，武将/手牌/装备/锦囊可完整对局）。"
            .to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        let mut es = new_lobby_state(&players);
        es.kernel.rng = rng.next_u64() | 1;
        let events = vec![EngineEvent::new(
            "sgs3_lobby_created",
            serde_json::json!({ "seats": es.kernel.seats.len(), "host": es.kernel.host }),
        )];
        (es.to_state(room_id, group_id), events)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let Ok(es) = EngineState::from_state(state) else {
            return Vec::new();
        };
        // 大厅 / 选将
        if es.lobby.is_some() || es.setup.is_some() {
            let mut out: Vec<(String, String)> = lobby::legal_actions(
                &es.kernel,
                es.lobby.as_ref(),
                es.setup.as_ref(),
                player_id,
            )
            .into_iter()
            .filter(|(id, _)| !id.starts_with("lobby:set_") || es.kernel.host.as_deref() == Some(player_id))
            .collect();
            if es.lobby.is_some() && es.kernel.host.as_deref() == Some(player_id) {
                out.extend(lobby::config_actions(&es.kernel, &es.cfg));
            }
            out.sort();
            out.dedup_by(|a, b| a.0 == b.0);
            return out
                .into_iter()
                .map(|(id, description)| Action { id, description })
                .collect();
        }
        let Some(seat) = es.kernel.seat_of(player_id) else {
            return Vec::new();
        };
        if let Some(p) = &es.kernel.pending {
            if p.actor() == seat {
                return p
                    .options()
                    .iter()
                    // 强制选择类待办的占位出口不下发（与视图一致）。
                    .filter(|o| !(p.escape_hidden() && o.id == p.escape_id()))
                    .map(|o| Action {
                        id: o.id.clone(),
                        description: o.label.clone(),
                    })
                    .collect();
            }
        }
        // 回合外主动技（矫诏修改 1 起）：无 pending 时向该玩家暴露。
        if es.kernel.pending.is_none() {
            return crate::kernel::skills::out_of_turn_actions(&es.kernel, seat)
                .into_iter()
                .map(|code| {
                    let name = crate::kernel::skills::display_name(&es.kernel, seat, code);
                    Action {
                        id: format!("skill:{code}"),
                        description: format!("发动【{name}】（回合外）"),
                    }
                })
                .collect();
        }
        Vec::new()
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        let mut es = EngineState::from_state(state)?;
        if es.finished() {
            return Err(EngineError::IllegalAction("对局已结束".into()));
        }
        let log_before = es.kernel.log.len();
        let message = if action.starts_with("lobby:") || action.starts_with("setup:") {
            let mut lobby_slot = es.lobby.clone();
            let mut setup_slot = es.setup.clone();
            let msg = lobby::apply(
                &mut es.kernel,
                &mut es.cfg,
                &mut lobby_slot,
                &mut setup_slot,
                player_id,
                action,
                rng,
            )
            .map_err(EngineError::IllegalAction)?;
            es.lobby = lobby_slot;
            es.setup = setup_slot;
            msg
        } else {
            let seat = es
                .kernel
                .seat_of(player_id)
                .ok_or_else(|| EngineError::PermissionDenied("不在座".into()))?;
            if es.kernel.pending.is_none() {
                if let Some(code) = action.strip_prefix("skill:") {
                    if crate::kernel::skills::out_of_turn_actions(&es.kernel, seat)
                        .contains(&code)
                    {
                        crate::kernel::skills::start_out_of_turn(&mut es.kernel, seat, code)
                            .map_err(EngineError::IllegalAction)?;
                        crate::kernel::resolve::advance(&mut es.kernel)
                            .map_err(EngineError::IllegalAction)?;
                        es.kernel
                            .check_conservation()
                            .map_err(EngineError::Internal)?;
                        return Ok(ActionResult {
                            state: es.to_state(&state.room_id, &state.group_id),
                            message: "回合外发动技能".to_string(),
                            finished: es.finished(),
                            events: engine_events(&es.kernel.log, log_before),
                        });
                    }
                }
            }
            crate::kernel::resolve::submit(&mut es.kernel, seat, action)
                .map_err(EngineError::IllegalAction)?;
            // 文案：本动作产生的最后一条有效历史。
            es.kernel
                .log
                .get(log_before..)
                .unwrap_or(&[])
                .iter()
                .rev()
                .map(|ev| text::text_of(&es.kernel, ev))
                .find(|t| !t.is_empty())
                .unwrap_or_else(|| "已提交".to_string())
        };
        es.kernel
            .check_conservation()
            .map_err(EngineError::Internal)?;
        es.kernel
            .check_no_orphan_frames()
            .map_err(EngineError::Internal)?;
        let finished = es.finished();
        let events = engine_events(&es.kernel.log, log_before);
        Ok(ActionResult {
            state: es.to_state(&state.room_id, &state.group_id),
            message,
            finished,
            events,
        })
    }

    fn public_state(&self, state: &GameState) -> GameState {
        let Ok(es) = EngineState::from_state(state) else {
            return state.clone();
        };
        let mut out = es.to_state(&state.room_id, &state.group_id);
        out.data = view::public_state(
            &es.kernel,
            &es.cfg,
            es.lobby.as_ref(),
            es.setup.as_ref(),
        );
        out
    }

    fn player_view(&self, state: &GameState, viewer: &str) -> GameState {
        let Ok(es) = EngineState::from_state(state) else {
            return state.clone();
        };
        let mut out = es.to_state(&state.room_id, &state.group_id);
        let seat = es.kernel.seat_of(viewer);
        out.data = match seat {
            Some(s) => view::player_view(
                &es.kernel,
                &es.cfg,
                es.lobby.as_ref(),
                es.setup.as_ref(),
                s,
            ),
            None => view::public_state(&es.kernel, &es.cfg, es.lobby.as_ref(), es.setup.as_ref()),
        };
        out
    }

    fn summarize(&self, state: &GameState) -> String {
        let Ok(es) = EngineState::from_state(state) else {
            return "三国杀（状态不可解析）".to_string();
        };
        let g = &es.kernel;
        let stage = view::phase_label(g, es.lobby.as_ref(), es.setup.as_ref());
        if es.setup.is_some() {
            let picked = es.setup.as_ref().map(|s| s.picks.len()).unwrap_or(0);
            return format!("三国杀：选将中（{picked}/{} 已选）", g.seats.len());
        }
        if es.lobby.is_some() {
            return format!("三国杀：大厅（{}/8 人）", g.seats.len());
        }
        let current = g
            .seats
            .get(g.turn)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let hp: Vec<String> = g
            .seats
            .iter()
            .map(|s| format!("{}:{}/{}", s.name, s.hp, s.max_hp))
            .collect();
        let waiting = g
            .pending
            .as_ref()
            .and_then(|p| g.seats.get(p.actor()))
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let winner = g
            .winner
            .map(|w| format!("{w:?}"))
            .unwrap_or_else(|| "-".to_string());
        format!(
            "三国杀：{stage} 阶段，当前 {current}，等待 {waiting}，第 {} 轮，体力 {}，胜者 {winner}",
            g.round,
            hp.join(" ")
        )
    }

    fn validate_state(&self, state: &GameState) -> Result<(), String> {
        let es = EngineState::from_state(state).map_err(|e| e.to_string())?;
        es.kernel.check_conservation()?;
        es.kernel.check_no_orphan_frames()?;
        Ok(())
    }
}
