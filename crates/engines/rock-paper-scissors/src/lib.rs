//! 猜拳（Rock-Paper-Scissors）引擎：一轮对话。
//!
//! 玩家开桌后出拳，引擎随机出拳判定输赢，返回结果文案。
//! 手势解析由引擎内「模拟决策」完成（本次不接 SystemOne）。

use game_engine_core::{
    roll_die, Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus,
    RandomSource,
};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hand {
    Rock,
    Paper,
    Scissors,
}

impl Hand {
    /// 解析玩家手势（中文 / 英文 / emoji，引擎内模拟决策）。
    fn parse(s: &str) -> Option<Hand> {
        match s.trim().to_lowercase().as_str() {
            "石头" | "rock" | "✊" => Some(Hand::Rock),
            "布" | "paper" | "✋" => Some(Hand::Paper),
            "剪刀" | "scissors" | "✌" | "✌️" => Some(Hand::Scissors),
            _ => None,
        }
    }

    fn beats(self, other: Hand) -> bool {
        matches!(
            (self, other),
            (Hand::Rock, Hand::Scissors)
                | (Hand::Scissors, Hand::Paper)
                | (Hand::Paper, Hand::Rock)
        )
    }

    fn name(self) -> &'static str {
        match self {
            Hand::Rock => "石头",
            Hand::Paper => "布",
            Hand::Scissors => "剪刀",
        }
    }
}

pub struct RpsEngine;

impl EnginePlugin for RpsEngine {
    fn game_id(&self) -> &'static str {
        "rps"
    }

    fn rules(&self) -> String {
        "猜拳：一轮定胜负。玩家出「石头/剪刀/布」，机器人随机出拳判定。".to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        _rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        let state = GameState {
            game_id: self.game_id().to_string(),
            room_id: room_id.to_string(),
            group_id: group_id.to_string(),
            status: GameStatus::Active,
            players,
            data: json!({ "player_hand": null, "bot_hand": null, "result": null }),
        };
        (state, vec![EngineEvent::new("game_started", json!({}))])
    }

    fn legal_actions(&self, state: &GameState, _player_id: &str) -> Vec<Action> {
        if state.status == GameStatus::Finished {
            return vec![];
        }
        vec![
            Action { id: "石头".into(), description: "出石头".into() },
            Action { id: "剪刀".into(), description: "出剪刀".into() },
            Action { id: "布".into(), description: "出布".into() },
        ]
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        // —— 无条件重校验：合法性 + 权限 ——
        if state.status == GameStatus::Finished {
            return Err(EngineError::IllegalAction("对局已结束".into()));
        }
        if !state.players.iter().any(|p| p == player_id) {
            return Err(EngineError::PermissionDenied(format!("玩家 {player_id} 不在本局")));
        }
        if state.data.get("player_hand").and_then(|v| v.as_str()).is_some() {
            return Err(EngineError::IllegalAction("本轮已出过拳".into()));
        }

        let player_hand = Hand::parse(action)
            .ok_or_else(|| EngineError::IllegalAction(format!("无法识别手势: {action}")))?;

        // 引擎随机出拳（模拟对手）
        let bot_hand = random_hand(rng);

        let result = if player_hand == bot_hand {
            "平局 🤝"
        } else if player_hand.beats(bot_hand) {
            "你赢了！🎉"
        } else {
            "你输了！"
        };

        let message = format!(
            "你出【{}】，我出【{}】——{}",
            player_hand.name(),
            bot_hand.name(),
            result
        );

        let new_state = GameState {
            game_id: state.game_id.clone(),
            room_id: state.room_id.clone(),
            group_id: state.group_id.clone(),
            status: GameStatus::Finished,
            players: state.players.clone(),
            data: json!({
                "player_hand": player_hand.name(),
                "bot_hand": bot_hand.name(),
                "result": result,
            }),
        };

        let events = vec![EngineEvent::new(
            "hand_played",
            json!({
                "player_id": player_id,
                "player_hand": player_hand.name(),
                "bot_hand": bot_hand.name(),
                "result": result,
            }),
        )
        .with_actor(player_id)];

        Ok(ActionResult {
            state: new_state,
            message,
            finished: true,
            events,
        })
    }

    fn summarize(&self, state: &GameState) -> String {
        match state.status {
            GameStatus::Active => "猜拳（一轮）：等待玩家出拳".to_string(),
            GameStatus::Finished => {
                let hand = state
                    .data
                    .get("player_hand")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知");
                let result = state
                    .data
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知");
                format!("猜拳：玩家出【{hand}】，结果 {result}")
            }
        }
    }
}

/// 随机出拳：用注入的随机源（`roll_die % 3`）。
fn random_hand(rng: &mut dyn RandomSource) -> Hand {
    match roll_die(rng) % 3 {
        0 => Hand::Rock,
        1 => Hand::Paper,
        _ => Hand::Scissors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine_core::{EngineError, GameState, GameStatus, SeededRng};
    use serde_json::json;

    fn new_state() -> GameState {
        GameState {
            game_id: "rps".into(),
            room_id: "room-1".into(),
            group_id: "group-1".into(),
            status: GameStatus::Active,
            players: vec!["p1".into()],
            data: json!({ "player_hand": null, "bot_hand": null, "result": null }),
        }
    }

    fn rng() -> SeededRng {
        SeededRng::new(1)
    }

    #[test]
    fn hand_parse_variants() {
        assert_eq!(Hand::parse("石头"), Some(Hand::Rock));
        assert_eq!(Hand::parse("rock"), Some(Hand::Rock));
        assert_eq!(Hand::parse("✊"), Some(Hand::Rock));
        assert_eq!(Hand::parse("剪刀"), Some(Hand::Scissors));
        assert_eq!(Hand::parse("scissors"), Some(Hand::Scissors));
        assert_eq!(Hand::parse("布"), Some(Hand::Paper));
        assert_eq!(Hand::parse("paper"), Some(Hand::Paper));
        assert_eq!(Hand::parse("✋"), Some(Hand::Paper));
        assert_eq!(Hand::parse("无效"), None);
        assert_eq!(Hand::parse(""), None);
    }

    #[test]
    fn hand_beats_relation() {
        assert!(Hand::Rock.beats(Hand::Scissors));
        assert!(Hand::Scissors.beats(Hand::Paper));
        assert!(Hand::Paper.beats(Hand::Rock));
        assert!(!Hand::Rock.beats(Hand::Paper));
        assert!(!Hand::Paper.beats(Hand::Scissors));
        assert!(!Hand::Scissors.beats(Hand::Rock));
    }

    #[test]
    fn invalid_hand_rejected() {
        let engine = RpsEngine;
        let state = new_state();
        let err = engine
            .execute_action(&state, "p1", "无效手势", &mut rng())
            .unwrap_err();
        assert!(matches!(err, EngineError::IllegalAction(_)));
    }

    #[test]
    fn non_player_rejected() {
        let engine = RpsEngine;
        let state = new_state();
        let err = engine.execute_action(&state, "p2", "石头", &mut rng()).unwrap_err();
        assert!(matches!(err, EngineError::PermissionDenied(_)));
    }

    #[test]
    fn finished_game_rejected() {
        let engine = RpsEngine;
        let mut state = new_state();
        state.status = GameStatus::Finished;
        let err = engine.execute_action(&state, "p1", "石头", &mut rng()).unwrap_err();
        assert!(matches!(err, EngineError::IllegalAction(_)));
    }

    #[test]
    fn execute_returns_finished_with_message() {
        let engine = RpsEngine;
        let state = new_state();
        let result = engine.execute_action(&state, "p1", "石头", &mut rng()).unwrap();
        assert!(result.finished);
        assert!(result.message.contains("你出【石头】"));
        assert_eq!(result.state.status, GameStatus::Finished);
        assert!(result.state.data["player_hand"].is_string());
        assert!(result.state.data["bot_hand"].is_string());
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].event_type, "hand_played");
    }

    #[test]
    fn legal_actions_are_three_hands() {
        let engine = RpsEngine;
        let state = new_state();
        let actions = engine.legal_actions(&state, "p1");
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn summarize_reports_hand_and_result() {
        let engine = RpsEngine;
        let state = new_state();
        let result = engine.execute_action(&state, "p1", "石头", &mut rng()).unwrap();
        let sum = engine.summarize(&result.state);
        assert!(sum.contains("玩家出【石头】"), "{sum}");
    }
}
