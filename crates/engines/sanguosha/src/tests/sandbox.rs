//! 工作台沙盒开局单测（P5）。

use game_engine_core::SeededRng;

use crate::sandbox::{new_sandbox_game, sandbox_meta, SandboxSeat, SandboxSpec};
use crate::state::{Identity, Phase, SgsState};

fn spec(players: usize) -> SandboxSpec {
    SandboxSpec {
        players,
        seats: Vec::new(),
    }
}

#[test]
fn sandbox_specifies_generals_identities_and_starts_game() {
    let pool = crate::catalog::default_general_pool();
    let codes: Vec<String> = pool.iter().take(5).map(|g| g.code.to_string()).collect();
    let mut seats = Vec::new();
    let identities = [
        Identity::Lord,
        Identity::Loyalist,
        Identity::Rebel,
        Identity::Rebel,
        Identity::Spy,
    ];
    for i in 0..5 {
        seats.push(SandboxSeat {
            index: Some(i),
            player_id: None,
            general: Some(codes[i].clone()),
            identity: Some(identities[i]),
        });
    }
    let mut rng = SeededRng::new(42);
    let (state, events) = new_sandbox_game(
        "sandbox-room",
        "sandbox-group",
        &SandboxSpec {
            players: 5,
            seats,
        },
        &mut rng,
    )
    .expect("sandbox game");
    assert_eq!(state.status, game_engine_core::GameStatus::Active);
    assert_eq!(events.first().map(|e| e.event_type.as_str()), Some("sandbox_created"));
    let st = SgsState::parse(&state.data).expect("parse");
    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.seats.len(), 5);
    assert_eq!(st.turn, 0, "主公（座位 0）先手");
    assert_eq!(st.setup.lord, "bot:1");
    assert_eq!(st.seats[0].general.as_deref(), Some(codes[0].as_str()));
    assert_eq!(st.seats[1].identity, Identity::Loyalist);
    assert_eq!(st.seats[2].identity, Identity::Rebel);
    assert_eq!(st.seats[3].identity, Identity::Rebel);
    assert_eq!(st.seats[4].identity, Identity::Spy);
    assert!(st.seats.iter().all(|s| s.is_bot), "所有座位都应是机器人");
    assert!(st.seats.iter().all(|s| s.hand.len() >= 4), "每人至少 4 张初始手牌");
    assert_eq!(st.lord_index(), Some(0));
    st.check_conservation().expect("conservation");
    crate::scene::validate_state(&state).expect("valid sandbox state");
}

#[test]
fn sandbox_random_is_deterministic_and_unique() {
    let mut a = SeededRng::new(7);
    let mut b = SeededRng::new(7);
    let (sa, _) = new_sandbox_game("r", "g", &spec(8), &mut a).expect("a");
    let (sb, _) = new_sandbox_game("r", "g", &spec(8), &mut b).expect("b");
    assert_eq!(sa.data, sb.data, "同 seed 应完全可复现");

    let st = SgsState::parse(&sa.data).expect("parse");
    let generals: Vec<&str> = st
        .seats
        .iter()
        .filter_map(|s| s.general.as_deref())
        .collect();
    assert_eq!(generals.len(), 8);
    let mut unique = std::collections::BTreeSet::new();
    for code in generals {
        assert!(unique.insert(code.to_string()), "随机武将不应重复：{code}");
    }
    st.check_conservation().expect("conservation");
}

#[test]
fn sandbox_rejects_duplicate_or_out_of_pool_inputs() {
    let pool = crate::catalog::default_general_pool();
    let code = pool[0].code.to_string();
    let cases = [
        // 玩家数越界
        SandboxSpec {
            players: 4,
            seats: Vec::new(),
        },
        // 武将重复
        SandboxSpec {
            players: 5,
            seats: vec![
                SandboxSeat {
                    index: Some(0),
                    player_id: None,
                    general: Some(code.clone()),
                    identity: None,
                },
                SandboxSeat {
                    index: Some(1),
                    player_id: None,
                    general: Some(code.clone()),
                    identity: None,
                },
            ],
        },
        // 主公超配比
        SandboxSpec {
            players: 5,
            seats: vec![
                SandboxSeat {
                    index: Some(0),
                    player_id: None,
                    general: None,
                    identity: Some(Identity::Lord),
                },
                SandboxSeat {
                    index: Some(1),
                    player_id: None,
                    general: None,
                    identity: Some(Identity::Lord),
                },
            ],
        },
        // 未知武将
        SandboxSpec {
            players: 5,
            seats: vec![SandboxSeat {
                index: Some(0),
                player_id: None,
                general: Some("not_a_general".into()),
                identity: None,
            }],
        },
    ];
    for (i, case) in cases.iter().enumerate() {
        let mut rng = SeededRng::new(1);
        assert!(
            new_sandbox_game("r", "g", case, &mut rng).is_err(),
            "case {i} 应被拒绝"
        );
    }
}

#[test]
fn sandbox_runs_opening_skills() {
    // 陆绩·怀橘：开局获得 3 枚「橘」。
    let mut seats = vec![SandboxSeat {
        index: Some(1),
        player_id: None,
        general: Some("sgs_hero_402".into()),
        identity: Some(Identity::Loyalist),
    }];
    // 明确 seat0 主公，避免随机到 1 号位。
    seats.push(SandboxSeat {
        index: Some(0),
        player_id: None,
        general: None,
        identity: Some(Identity::Lord),
    });
    let mut rng = SeededRng::new(11);
    let (state, _) = new_sandbox_game(
        "r",
        "g",
        &SandboxSpec {
            players: 5,
            seats,
        },
        &mut rng,
    )
    .expect("sandbox");
    let st = SgsState::parse(&state.data).expect("parse");
    assert_eq!(
        st.seats[1].mark_counts.get("ju").copied().unwrap_or(0),
        3,
        "陆绩开局应获得 3 枚「橘」"
    );
}

#[test]
fn sandbox_meta_has_dedup_pool_and_identity_setup() {
    let meta = sandbox_meta();
    let generals = meta["generals"].as_array().expect("generals");
    assert_eq!(generals.len(), 149, "默认去重池应为 149 位");
    assert!(generals.iter().all(|g| g["code"].is_string() && g["name"].is_string()));
    let ids = &meta["identities"];
    assert_eq!(ids["5"]["lord"], 1);
    assert_eq!(ids["5"]["rebel"], 2);
    assert_eq!(ids["8"]["spy"], 1);
}
