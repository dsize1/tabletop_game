//! 三国杀引擎 v2（P-C 内核 spike）。
//!
//! 目标：把 plan §8.5 的 6 类缺陷变成「写不出来 / 一定被测试抓到」：
//! | 缺陷模式 | 结构性对策 |
//! |---|---|
//! | A 触发点逐路径硬编码 | 唯一移动出口 + 语义事件；技能只订阅事件 |
//! | B 就地压帧被父帧 pop | 技能 handler 只返回 `Step`，栈只由 `advance` 改 |
//! | C 帧里暂存牌丢失 | 牌区唯一归属（`Zone::Frame`）+ `check_no_orphan_frames` |
//! | D 同一规则两处各写一半 | `active()` 单一状态源 + legal/execute 同源 + 订阅派生 |
//! | E 文案与语义相反 | `POutcome` 枚举 → 唯一文案表 |
//! | F 白名单/枚举式 UI | 视图/动作由注册表派生 |
//!
//! 本 crate 在迁移完成前**不注册**进 `game-engine-mcp`（见计划 O5）。

pub mod cards;
pub mod kernel;
pub mod presentation;
pub mod skills;

use game_engine_core::{
    Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus, RandomSource,
};
use serde_json::json;

use kernel::resolve;
use kernel::{Card, Game, Seat, Slot, Suit};

pub struct Sanguosha2Engine;

fn default_cards() -> Vec<Card> {
    vec![
        Card::new("c1", "杀", Suit::Spade, 7),
        Card::new("c2", "闪", Suit::Heart, 2),
        Card::new("c3", "桃", Suit::Diamond, 3),
        Card::new("c4", "无懈可击", Suit::Club, 4),
        Card::new("c5", "杀", Suit::Diamond, 9),
        Card::new("c6", "闪", Suit::Spade, 6),
        Card::equip("c7", "青釭剑", Suit::Spade, 6, Slot::Weapon),
        Card::equip("c8", "八卦阵", Suit::Spade, 2, Slot::Armor),
        Card::equip("c9", "赤兔", Suit::Heart, 5, Slot::HorseOffense),
        Card::equip("c10", "的卢", Suit::Club, 5, Slot::HorseDefense),
        Card::equip("c11", "木牛流马", Suit::Diamond, 5, Slot::Treasure),
        Card::new("c12", "杀", Suit::Club, 8),
        Card::new("c13", "闪", Suit::Diamond, 10),
        Card::new("c14", "桃", Suit::Heart, 12),
        Card::new("c15", "决斗", Suit::Spade, 1),
        Card::new("c16", "过河拆桥", Suit::Spade, 3),
        Card::new("c17", "顺手牵羊", Suit::Diamond, 4),
        Card::new("c18", "杀", Suit::Heart, 10),
        Card::new("c19", "闪", Suit::Club, 2),
        Card::new("c20", "桃", Suit::Club, 9),
    ]
}

/// 测试/沙盒用：3 人最小局面（周妃 / 卢植 / 路人）。
pub fn demo_game() -> Game {
    let seats = vec![
        Seat::new("p0", "zhoufei", &["liangyin", "kongsheng"], 3),
        Seat::new("p1", "luzhi", &["zhenliang"], 4),
        Seat::new("p2", "guest", &[], 4),
    ];
    Game::new(seats, default_cards())
}

impl EnginePlugin for Sanguosha2Engine {
    fn game_id(&self) -> &'static str {
        "sanguosha2"
    }

    fn rules(&self) -> String {
        "三国杀（内核 v2 spike）：技能=声明+handler，事件带窗口，单一延迟队列。".to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        _rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        let mut seats: Vec<Seat> = players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (general, skills): (&'static str, &[&'static str]) = match i {
                    0 => ("zhoufei", &["liangyin", "kongsheng"]),
                    1 => ("luzhi", &["zhenliang"]),
                    _ => ("guest", &[]),
                };
                Seat::new(p, general, skills, 4)
            })
            .collect();
        if seats.is_empty() {
            seats.push(Seat::new("p0", "zhoufei", &["liangyin", "kongsheng"], 3));
        }
        let g = Game::new(seats, default_cards());
        let state = GameState {
            game_id: "sanguosha2".into(),
            room_id: room_id.to_string(),
            group_id: group_id.to_string(),
            status: GameStatus::Active,
            players,
            data: serde_json::to_value(&g).unwrap_or(json!({})),
        };
        let events = vec![EngineEvent::new("sgs2_started", json!({}))];
        (state, events)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let Ok(g) = serde_json::from_value::<Game>(state.data.clone()) else {
            return Vec::new();
        };
        let Some(seat) = g.seats.iter().position(|s| s.name == player_id) else {
            return Vec::new();
        };
        if let Some(p) = &g.pending {
            if p.actor() == seat {
                return p
                    .options()
                    .iter()
                    .map(|o| Action {
                        id: o.id.clone(),
                        description: o.label.clone(),
                    })
                    .collect();
            }
            return Vec::new();
        }
        skills::legal_actions(&g, seat)
            .into_iter()
            .map(|(id, description)| Action { id, description })
            .collect()
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        _rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        let mut g = serde_json::from_value::<Game>(state.data.clone())
            .map_err(|e| EngineError::Internal(format!("状态解析失败：{e}")))?;
        let seat = g
            .seats
            .iter()
            .position(|s| s.name == player_id)
            .ok_or_else(|| EngineError::PermissionDenied("不在座".into()))?;
        let message = if g.pending.is_some() {
            resolve::submit(&mut g, seat, action).map_err(EngineError::IllegalAction)?
        } else if action.starts_with("skill:") {
            skills::execute_active(&mut g, seat, action).map_err(EngineError::IllegalAction)?
        } else if action == "end_play" {
            resolve::begin_phase(&mut g, seat, kernel::Phase::Discard)
                .map_err(EngineError::Internal)?;
            "结束出牌阶段".to_string()
        } else {
            return Err(EngineError::IllegalAction(format!("未知动作：{action}")));
        };
        g.check_conservation().map_err(EngineError::Internal)?;
        let mut out = state.clone();
        out.data = serde_json::to_value(&g).unwrap_or(json!({}));
        Ok(ActionResult {
            state: out,
            message,
            finished: g.finished,
            events: Vec::new(),
        })
    }
}

#[cfg(test)]
mod smoke {
    use super::*;

    #[test]
    fn engine_plugin_roundtrip() {
        let engine = Sanguosha2Engine;
        let mut rng = game_engine_core::SeededRng::new(7);
        let (state, _) = engine.new_game("r1", "g1", vec!["p0".into(), "p1".into()], &mut rng);
        let actions = engine.legal_actions(&state, "p0");
        assert!(!actions.is_empty());
        let res = engine
            .execute_action(&state, "p0", &actions[0].id, &mut rng)
            .expect("动作应可执行");
        assert!(res.message.len() > 0);
    }
}
