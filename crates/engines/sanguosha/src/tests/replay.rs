//! P2 收尾：事件溯源重放（`docs/games/sanguosha/development-plan.md` P2 验收）。
//!
//! 口径：**同 seed + 同动作序列 → 完全相同的状态与事件序列**。
//! 做法：跑一局并记录每一步 `(玩家, 动作)` 与产出事件 → 用同一 seed 的新管理器
//! 重放同一脚本 → 比对终态 JSON 与事件序列。

use super::{manager, state_of};
use crate::state::{Phase, SgsState};
use game_engine_core::GameManager;
use serde_json::Value;

type Step = (String, String);

fn create_room(mgr: &GameManager) -> String {
    let (st, _) = mgr
        .create_game("sanguosha", "group-1", "u1")
        .expect("create_game");
    st.room_id.clone()
}

/// 执行一步并把动作与事件记入脚本。
fn exec(
    mgr: &GameManager,
    room: &str,
    script: &mut Vec<Step>,
    events: &mut Vec<Value>,
    actor: &str,
    action: &str,
) {
    let res = mgr
        .execute_action(room, actor, action)
        .unwrap_or_else(|e| panic!("{actor} 执行 {action} 失败: {e}"));
    for ev in &res.events {
        events.push(serde_json::to_value(ev).expect("event -> json"));
    }
    script.push((actor.to_string(), action.to_string()));
}

/// 由状态推导下一步动作（确定性：优先 pass，其次首个候选/首个可出牌）。
fn choose(mgr: &GameManager, room: &str, st: &SgsState) -> (String, String) {
    if let Some(p) = &st.pending {
        let acts = mgr.legal_actions(room, &p.actor).expect("legal");
        assert!(!acts.is_empty(), "pending {} 无合法动作", p.kind);
        let a = acts
            .iter()
            .find(|a| a.id == "pass")
            .unwrap_or_else(|| &acts[0]);
        return (p.actor.clone(), a.id.clone());
    }
    if st.phase == Phase::Setup {
        for seat in &st.seats {
            let acts = mgr.legal_actions(room, &seat.player_id).expect("legal");
            if let Some(a) = acts
                .iter()
                .find(|a| a.id.starts_with("setup:choose_general:"))
            {
                return (seat.player_id.clone(), a.id.clone());
            }
        }
        panic!("选将阶段没有任何候选人");
    }
    let pid = st.seats[st.turn].player_id.clone();
    let acts = mgr.legal_actions(room, &pid).expect("legal");
    let a = acts
        .iter()
        .find(|a| a.id.starts_with("play:"))
        .or_else(|| acts.iter().find(|a| a.id == "end_play"))
        .expect("出牌阶段应有合法动作");
    (pid, a.id.clone())
}

/// 跑一局并记录脚本、终态与事件序列。
fn record(seed: u64, max_steps: usize) -> (Vec<Step>, String, Vec<Value>) {
    let mgr = manager(seed);
    let room = create_room(&mgr);
    let mut script: Vec<Step> = Vec::new();
    let mut events: Vec<Value> = Vec::new();

    for p in ["u2", "u3", "u4", "u5"] {
        exec(&mgr, &room, &mut script, &mut events, p, "lobby:join");
    }
    exec(&mgr, &room, &mut script, &mut events, "u1", "lobby:start");

    for _ in 0..max_steps {
        let st = state_of(&mgr, &room);
        if st.phase == Phase::Finished {
            break;
        }
        let (actor, action) = choose(&mgr, &room, &st);
        exec(&mgr, &room, &mut script, &mut events, &actor, &action);
    }

    let state = mgr.get_state(&room).expect("state");
    let json = serde_json::to_string(&state.data).expect("state -> json");
    (script, json, events)
}

/// 用同一 seed 重放脚本，返回终态 JSON 与事件序列。
fn replay(seed: u64, script: &[Step]) -> (String, Vec<Value>) {
    let mgr = manager(seed);
    let room = create_room(&mgr);
    let mut events: Vec<Value> = Vec::new();
    for (actor, action) in script {
        let res = mgr
            .execute_action(&room, actor, action)
            .unwrap_or_else(|e| panic!("重放 {actor} {action} 失败: {e}"));
        for ev in &res.events {
            events.push(serde_json::to_value(ev).expect("event -> json"));
        }
    }
    let state = mgr.get_state(&room).expect("state");
    (
        serde_json::to_string(&state.data).expect("state -> json"),
        events,
    )
}

#[test]
fn replay_reproduces_state_and_event_sequence() {
    let seed = 2026_1028;
    let (script, final_state, events) = record(seed, 240);

    assert!(
        script.len() > 40,
        "脚本过短（{} 步），无法覆盖一局流程",
        script.len()
    );
    assert!(
        final_state.contains("\"schema_version\":2"),
        "终态应为 schema v2"
    );
    // 并非每步都产事件（如 `pass`、部分弃牌），但一局应有稳定数量的事件。
    assert!(
        events.len() >= 40,
        "事件过少（{} 事件 / {} 步）",
        events.len(),
        script.len()
    );
    // 记录里必须包含选将 + 出牌两类动作，确认不是空跑。
    assert!(
        script
            .iter()
            .any(|(_, a)| a.starts_with("setup:choose_general:")),
        "脚本应包含选将动作"
    );
    assert!(
        script
            .iter()
            .any(|(_, a)| a.starts_with("play:") || a == "end_play"),
        "脚本应包含出牌阶段动作"
    );

    let (replayed_state, replayed_events) = replay(seed, &script);
    assert_eq!(
        final_state, replayed_state,
        "同 seed + 同动作序列应得到完全相同的状态"
    );
    assert_eq!(
        events, replayed_events,
        "同 seed + 同动作序列应得到完全相同的事件序列"
    );
}

#[test]
fn replay_is_stable_across_repeated_runs() {
    let seed = 2026_1101;
    let (script, final_state, events) = record(seed, 160);
    let (a_state, a_events) = replay(seed, &script);
    let (b_state, b_events) = replay(seed, &script);
    assert_eq!(final_state, a_state);
    assert_eq!(a_state, b_state);
    assert_eq!(events, a_events);
    assert_eq!(a_events, b_events);
}
