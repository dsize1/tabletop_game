//! M4-1：`EnginePlugin` 完备性 + 注册前置契约（**不注册**，只做可注册性检查）。
//!
//! 口径（`memory_palace/plan/next-engine-replacement.md` §M4-1 / OQ3）：
//! 注册/网关在重构完成前不开发；本文件只验证「如果要注册，接口是否齐备」：
//! - `game_id` / `rules` / `new_game` / `legal_actions` / `execute_action` 完备；
//! - `public_state` / `player_view` 是脱敏视图（schema_version + 隐藏信息）；
//! - legal 与 execute 同源（合法动作必须能被接受）；
//! - 每个 pending 都有出口；动作后守恒/无帧遗留；
//! - `finished` 与 `GameStatus` 一致；`summarize` 可给主持人用。

use game_engine_core::{EnginePlugin, GameStatus, SeededRng};
use sanguosha2::kernel::Game;
use sanguosha2::Sanguosha2Engine;

fn new_state(players: usize, seed: u64) -> (game_engine_core::GameState, SeededRng) {
    let engine = Sanguosha2Engine;
    let mut rng = SeededRng::new(seed);
    let ids: Vec<String> = (0..players).map(|i| format!("p{i}")).collect();
    let (state, events) = engine.new_game("r1", "g1", ids, &mut rng);
    assert!(!events.is_empty(), "开局应有事件");
    (state, rng)
}

fn game_of(state: &game_engine_core::GameState) -> Game {
    serde_json::from_value(state.data.clone()).expect("状态可反序列化")
}

#[test]
fn plugin_identity_and_rules() {
    let engine = Sanguosha2Engine;
    assert_eq!(engine.game_id(), "sanguosha2");
    assert!(!engine.rules().is_empty());
    let (state, _) = new_state(3, 1);
    let sum = engine.summarize(&state);
    assert!(sum.contains("三国杀"), "{sum}");
    assert!(sum.chars().count() <= 300);
}

#[test]
fn new_game_builds_playable_state() {
    let (state, _) = new_state(5, 1);
    let g = game_of(&state);
    assert_eq!(g.seats.len(), 5);
    assert_eq!(g.cards.len(), 160, "真实牌堆");
    assert_eq!(g.seats[0].identity, "lord");
    assert!(g.seats.iter().skip(1).all(|s| s.identity == "rebel"));
    g.check_conservation().unwrap();
    g.check_no_orphan_frames().unwrap();
    assert_eq!(state.status, GameStatus::Active);
    // 初始每人 4 张手牌（主公 +1 留待身份局细化）。
    assert!(g.seats.iter().all(|s| s.hp > 0));
    let total_hand: usize = (0..5).map(|i| g.hand(i).len()).sum();
    assert_eq!(total_hand, 20, "5 人 × 4 张");
}

#[test]
fn public_and_player_views_are_redacted() {
    let engine = Sanguosha2Engine;
    let (state, _) = new_state(3, 2);
    let public = engine.public_state(&state);
    let pv: serde_json::Value =
        serde_json::from_value(public.data.clone()).expect("公共视图是 JSON");
    assert_eq!(pv["schema_version"], 1);
    assert!(pv["player_area"]["hand"].as_array().unwrap().is_empty());
    assert!(pv["player_area"]["identity"].is_null());

    let me = engine.player_view(&state, "p0");
    let mv: serde_json::Value = serde_json::from_value(me.data.clone()).unwrap();
    assert_eq!(mv["player_area"]["seat"], 0);
    assert!(!mv["player_area"]["hand"].as_array().unwrap().is_empty());
    assert_eq!(mv["player_area"]["identity"], "lord");

    let other = engine.player_view(&state, "p1");
    let ov: serde_json::Value = serde_json::from_value(other.data.clone()).unwrap();
    assert_eq!(ov["player_area"]["seat"], 1);
    // 非座玩家 → 公共视图（不泄漏任何人的手牌）。
    let stranger = engine.player_view(&state, "nobody");
    let sv: serde_json::Value = serde_json::from_value(stranger.data.clone()).unwrap();
    assert!(sv["player_area"]["seat"].is_null());
    assert!(sv["player_area"]["hand"].as_array().unwrap().is_empty());
}

#[test]
fn legal_actions_are_accepted_by_execute() {
    let engine = Sanguosha2Engine;
    let (state, mut rng) = new_state(5, 3);
    let g = game_of(&state);
    let actor = match &g.pending {
        Some(p) => g.seats[p.actor()].name.clone(),
        None => g.seats[g.turn].name.clone(),
    };
    let actions = engine.legal_actions(&state, &actor);
    assert!(!actions.is_empty(), "开局应有合法动作（含 pending 出口）");
    // 逐个在**状态副本**上执行：合法动作必须被接受（legal/execute 同源）。
    for action in actions.iter().take(8) {
        let mut copy = state.clone();
        match engine.execute_action(&mut copy, &actor, &action.id, &mut rng) {
            Ok(_) => {}
            Err(e) => panic!("合法动作 {} 被拒绝：{e}", action.id),
        }
        let g = game_of(&copy);
        g.check_conservation().expect("动作后守恒");
        g.check_no_orphan_frames().expect("动作后无帧遗留");
    }
}

#[test]
fn pending_always_has_escape_and_private_options() {
    let engine = Sanguosha2Engine;
    let (state, _) = new_state(3, 4);
    let g = game_of(&state);
    if let Some(p) = &g.pending {
        assert!(
            p.has_option(p.escape_id()),
            "pending 出口 {} 必须在 options 里",
            p.escape_id()
        );
        // 只有行动者的个人视图才带 options/data。
        let actor = g.seats[p.actor()].name.clone();
        let av: serde_json::Value =
            serde_json::from_value(engine.player_view(&state, &actor).data).unwrap();
        assert!(!av["player_area"]["pending"]["options"]
            .as_array()
            .unwrap()
            .is_empty());
        let pv: serde_json::Value =
            serde_json::from_value(engine.public_state(&state).data).unwrap();
        assert!(pv["public_board"]["pending"].get("options").is_none());
    }
}

#[test]
fn finished_action_sets_status_and_summary() {
    let engine = Sanguosha2Engine;
    let (mut state, mut rng) = new_state(2, 5);
    // 手动把主公打到濒死前：直接结束游戏，验证 execute 后的 status 映射。
    {
        let mut g = game_of(&state);
        g.finished = true;
        g.winner = Some("rebel".to_string());
        state.data = serde_json::to_value(&g).unwrap();
    }
    assert_eq!(state.status, GameStatus::Active, "手工改状态不自动映射");
    // 用任意合法动作走一遍 execute（pending 出口）。
    let g = game_of(&state);
    let actor = match &g.pending {
        Some(p) => g.seats[p.actor()].name.clone(),
        None => g.seats[g.turn].name.clone(),
    };
    let actions = engine.legal_actions(&state, &actor);
    let res = engine
        .execute_action(&state, &actor, &actions[0].id, &mut rng)
        .expect("终局后仍可执行 pending 出口");
    assert!(res.finished);
    assert_eq!(res.state.status, GameStatus::Finished);
}
