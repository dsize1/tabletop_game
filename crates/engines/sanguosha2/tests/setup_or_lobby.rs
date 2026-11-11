//! S1：大厅 / 选将 / 身份局 / 机器人座位（引擎侧验收）。
//!
//! 覆盖：
//! - 大厅动作协议（join / add_bot / kick / set_* / start / abort）与 `legal ≡ execute` 同源；
//! - 选将：主公先选 → 候选互斥 → 全员选完发牌；
//! - 武将包过滤 + 武将池完整性（技能未实现的不进池）；
//! - 身份配比 / 主公体力 +1 / 主公先手 / 死亡奖惩 / 胜负；
//! - 机器人座位自动选将；
//! - 牌堆守恒（160）与「移出未选中的包」不破坏归属。

use game_engine_core::{EnginePlugin, SeededRng};
use sanguosha2::kernel::{Game, Zone};
use sanguosha2::{generals, lobby, Sanguosha2Engine};

fn engine() -> Sanguosha2Engine {
    Sanguosha2Engine
}

fn game_of(state: &game_engine_core::GameState) -> Game {
    serde_json::from_value(state.data.clone()).expect("状态可反序列化")
}

fn new_lobby(players: &[&str]) -> (game_engine_core::GameState, SeededRng) {
    let ids: Vec<String> = players.iter().map(|s| (*s).to_string()).collect();
    let mut rng = SeededRng::new(7);
    let (state, _) = engine().new_game("r1", "g1", ids, &mut rng);
    (state, rng)
}

/// 走真实链路把大厅推进到「可开局」：补到 n 人。
fn seat_players(players: usize) -> (game_engine_core::GameState, SeededRng) {
    let (mut state, mut rng) = new_lobby(&["host"]);
    let e = engine();
    for i in 1..players {
        state = e
            .execute_action(&state, &format!("p{i}"), "lobby:join", &mut rng)
            .expect("join 应成功")
            .state;
    }
    (state, rng)
}

#[test]
fn new_game_starts_in_lobby_and_join_is_legal() {
    let (state, mut rng) = new_lobby(&["host"]);
    let g = game_of(&state);
    assert!(g.lobby.is_some(), "新建局应停在大厅");
    assert!(g.setup.is_none());
    assert_eq!(g.seats.len(), 1, "create_game 只带房主");
    assert!(!g.identity_game, "大厅阶段尚未进入身份局");

    // 未入座玩家：只有 lobby:join。
    let outsider = engine().legal_actions(&state, "p1");
    assert_eq!(outsider.len(), 1);
    assert_eq!(outsider[0].id, "lobby:join");
    // legal ≡ execute：同一 id 必须可执行。
    let res = engine()
        .execute_action(&state, "p1", &outsider[0].id, &mut rng)
        .expect("合法动作必须可执行");
    assert_eq!(game_of(&res.state).seats.len(), 2);
}

#[test]
fn lobby_config_and_bot_actions() {
    let (state, mut rng) = seat_players(5);
    let e = engine();
    // 房主可加机器人（直到 8 人）。
    let mut s = state;
    for _ in 0..3 {
        s = e
            .execute_action(&s, "host", "lobby:add_bot", &mut rng)
            .expect("加机器人应成功")
            .state;
    }
    let g = game_of(&s);
    assert_eq!(g.seats.len(), 8);
    assert_eq!(g.seats.iter().filter(|x| x.bot).count(), 3);

    // 非房主改配置 → 权限拒绝。
    let err = e.execute_action(&s, "p1", "lobby:set_max_hand_cards:10", &mut rng);
    assert!(err.is_err(), "仅房主可改配置");

    // 房主改配置 → 生效。
    let s = e
        .execute_action(&s, "host", "lobby:set_max_hand_cards:10", &mut rng)
        .expect("房主改配置")
        .state;
    assert_eq!(game_of(&s).cfg.max_hand_cards, 10);

    // 8 人可切双内局。
    let s = e
        .execute_action(&s, "host", "lobby:set_double_spy:1", &mut rng)
        .expect("开启双内局")
        .state;
    assert!(game_of(&s).cfg.double_spy);

    // 踢人（非房主）。
    let s = e
        .execute_action(&s, "host", "lobby:kick:p1", &mut rng)
        .expect("踢出 p1")
        .state;
    assert!(game_of(&s).seats.iter().all(|x| x.name != "p1"));
}

#[test]
fn start_requires_five_players() {
    let (state, mut rng) = seat_players(4);
    let err = engine().execute_action(&state, "host", "lobby:start", &mut rng);
    assert!(err.is_err(), "4 人不得开局");
    let actions = engine().legal_actions(&state, "host");
    assert!(
        !actions.iter().any(|a| a.id == "lobby:start"),
        "不足 5 人时 legal 不提供 start"
    );
}

#[test]
fn start_assigns_identities_and_lord_picks_first() {
    let (state, mut rng) = seat_players(5);
    let e = engine();
    let state = e
        .execute_action(&state, "host", "lobby:start", &mut rng)
        .expect("5 人开局")
        .state;
    let g = game_of(&state);
    assert!(g.lobby.is_none() && g.setup.is_some(), "进入选将阶段");
    assert_eq!(g.seats.iter().filter(|s| s.identity == "lord").count(), 1);
    // 只有主公收到候选。
    let setup = g.setup.as_ref().unwrap();
    assert_eq!(setup.offers.len(), 1);
    assert!(setup.offers.contains_key(&setup.lord));

    // 主公候选包含全部主公将 + N 张非主公。
    let offers = &setup.offers[&setup.lord];
    assert!(offers.len() >= 5, "主公候选至少 5 张：{}", offers.len());
    assert!(offers
        .iter()
        .any(|c| generals::by_code(c).is_some_and(|d| d.lord)));

    // 非主公在主人选完前没有候选。
    let other = (0..5).find(|i| *i != setup.lord).unwrap();
    assert!(!setup.offers.contains_key(&other));

    // 主公从候选中选一张 → 其他人立刻收到互斥候选。
    let lord_pid = g.seats[setup.lord].name.clone();
    let pick = format!("setup:choose_general:{}", offers[0]);
    let state = e
        .execute_action(&state, &lord_pid, &pick, &mut rng)
        .expect("主公选将")
        .state;
    let g = game_of(&state);
    let setup = g.setup.as_ref().unwrap();
    assert!(setup.lord_picked);
    assert_eq!(setup.picks.len(), 1);
    let mut all: Vec<String> = Vec::new();
    for (seat, offers) in &setup.offers {
        if *seat == setup.lord {
            continue;
        }
        assert!(!offers.is_empty(), "非主公应收到候选");
        all.extend(offers.iter().cloned());
    }
    let unique: std::collections::BTreeSet<_> = all.iter().collect();
    assert_eq!(unique.len(), all.len(), "非主公候选互不重复");
    assert!(
        !all.contains(&setup.picks[&setup.lord]),
        "已选武将不再出现在他人候选"
    );
}

#[test]
fn everyone_picking_finishes_setup_and_deals() {
    let (state, mut rng) = seat_players(5);
    let e = engine();
    let mut state = e
        .execute_action(&state, "host", "lobby:start", &mut rng)
        .expect("开局")
        .state;
    // 反复找「当前可选的玩家」并选第一个候选，直到进入回合。
    for _ in 0..20 {
        let g = game_of(&state);
        let Some(setup) = g.setup.clone() else { break };
        let Some((seat, offers)) = setup
            .offers
            .iter()
            .find(|(i, o)| !setup.picks.contains_key(i) && !o.is_empty())
        else {
            break;
        };
        let pid = g.seats[*seat].name.clone();
        let action = format!("setup:choose_general:{}", offers[0]);
        state = e
            .execute_action(&state, &pid, &action, &mut rng)
            .expect("合法选将必须可执行")
            .state;
    }
    let g = game_of(&state);
    assert!(g.setup.is_none(), "全员选完应离开选将阶段");
    assert!(g.identity_game, "进入身份局");
    assert_eq!(g.round, 1);
    // 主公先手。
    let lord = g.seats.iter().position(|s| s.identity == "lord").unwrap();
    assert_eq!(g.turn, lord, "主公先手");
    // 每人至少 4 张手牌（部分武将开局技能会额外摸牌）+ 牌堆守恒。
    let total_hand: usize = (0..5).map(|i| g.hand(i).len()).sum();
    for i in 0..5 {
        assert!(g.hand(i).len() >= 4, "座位 {i} 手牌不足 4 张");
    }
    g.check_conservation().unwrap();
    assert_eq!(g.ids_in(|z| *z == Zone::Draw).len() + total_hand, 160);
}

#[test]
fn general_pool_excludes_unimplemented_skills_and_filters_packs() {
    let all = lobby::available_codes(&lobby::LobbyConfig::default());
    assert_eq!(all.len(), 147, "默认武将池 = 147 live 且技能齐全");
    let standard = lobby::available_codes(&lobby::LobbyConfig {
        general_packs: "standard".into(),
        ..Default::default()
    });
    assert!(standard.len() < all.len());
    assert!(standard.iter().all(|c| {
        generals::by_code(c).is_some_and(|d| d.pack == "standard" && d.status == generals::Status::Live)
    }));
    // 池内武将的技能必须全部能在 v2 注册表找到（否则玩家选到会缺技能）。
    for code in &all {
        let def = generals::by_code(code).unwrap();
        for name in def.skills {
            assert!(
                sanguosha2::skills::code_by_name(name).is_some(),
                "{} 的技能「{}」未注册",
                def.name,
                name
            );
        }
    }
}

#[test]
fn pack_subset_voids_unused_cards_and_keeps_expanded_deck() {
    let (state, mut rng) = seat_players(5);
    let e = engine();
    let mut state = e
        .execute_action(&state, "host", "lobby:set_packs:standard", &mut rng)
        .expect("切标准包")
        .state;
    state = e
        .execute_action(&state, "host", "lobby:start", &mut rng)
        .expect("开局")
        .state;
    for _ in 0..20 {
        let g = game_of(&state);
        let Some(setup) = g.setup.clone() else { break };
        let Some((seat, offers)) = setup
            .offers
            .iter()
            .find(|(i, o)| !setup.picks.contains_key(i) && !o.is_empty())
        else {
            break;
        };
        let pid = g.seats[*seat].name.clone();
        state = e
            .execute_action(
                &state,
                &pid,
                &format!("setup:choose_general:{}", offers[0]),
                &mut rng,
            )
            .expect("选将")
            .state;
    }
    let g = game_of(&state);
    g.check_conservation().expect("移出未选牌堆后仍守恒");
    let draw = g.ids_in(|z| *z == Zone::Draw).len();
    let void = g.ids_in(|z| *z == Zone::Void).len();
    let total_hand: usize = (0..g.seats.len()).map(|i| g.hand(i).len()).sum();
    assert_eq!(draw + total_hand, 108, "标准包 108 张（含已发手牌）");
    assert_eq!(void, 160 - 108, "军争 52 张移出游戏");
}

#[test]
fn bots_pick_automatically() {
    let (state, mut rng) = seat_players(3);
    let e = engine();
    let mut state = state;
    for _ in 0..2 {
        state = e
            .execute_action(&state, "host", "lobby:add_bot", &mut rng)
            .expect("补机器人")
            .state;
    }
    let state = e
        .execute_action(&state, "host", "lobby:start", &mut rng)
        .expect("5 人（含 2 机器人）开局")
        .state;
    let g = game_of(&state);
    let bots: Vec<usize> = (0..g.seats.len()).filter(|i| g.seats[*i].bot).collect();
    assert_eq!(bots.len(), 2);
    if let Some(setup) = g.setup.as_ref() {
        // 机器人选将不产生待办：其候选在发放后立刻被消费。
        for b in &bots {
            if setup.offers.contains_key(b) {
                assert!(setup.picks.contains_key(b), "机器人应已自动选将");
            }
        }
    } else {
        // 主公是机器人时可能直接打到回合阶段。
        assert!(g.identity_game);
    }
}

#[test]
fn abort_finishes_without_scoring() {
    let (state, mut rng) = seat_players(5);
    let res = engine()
        .execute_action(&state, "host", "lobby:abort", &mut rng)
        .expect("中断");
    let g = game_of(&res.state);
    assert!(g.finished);
    assert_eq!(g.abort_reason.as_deref(), Some("房主中断"));
    assert!(res.finished);
}

#[test]
fn host_leaving_aborts_lobby() {
    let (mut state, mut rng) = seat_players(3);
    let e = engine();
    state = e
        .execute_action(&state, "host", "lobby:leave", &mut rng)
        .expect("房主离开")
        .state;
    let g = game_of(&state);
    assert!(g.finished);
    assert_eq!(g.abort_reason.as_deref(), Some("房主离开，对局废弃"));
}

#[test]
fn kill_rebel_reward_applies_in_identity_game() {
    let mut rng = SeededRng::new(11);
    let (state, _) = engine().new_started_game(
        "r",
        "g",
        vec!["p0".into(), "p1".into(), "p2".into(), "p3".into(), "p4".into()],
        &mut rng,
    );
    let mut g = game_of(&state);
    // 构造「主公杀死反贼」。
    g.seats[0].identity = "lord".into();
    g.seats[1].identity = "rebel".into();
    g.seats[1].hp = 0;
    let before = g.hand(0).len();
    sanguosha2::identity::on_death(&mut g, 1, Some(0));
    assert_eq!(g.hand(0).len(), before + 3);
}

#[test]
fn sandbox_start_does_not_trigger_gain_skills() {
    // 走查（2026-11-11）：开局发 4 张手牌曾进入事件队列，导致「周泰开场就发动奋激」。
    // 发牌不是对局内动作：finish_setup 必须清空发牌产生的事件，且开局无 pending。
    use sanguosha2::sandbox::{new_sandbox_game, SandboxSeat, SandboxSpec};
    let seat = |i: usize, code: &str, identity: Option<&str>| SandboxSeat {
        index: Some(i),
        player_id: None,
        general: Some(code.to_string()),
        identity: identity.map(str::to_string),
    };
    let spec = SandboxSpec {
        players: 5,
        seats: vec![
            seat(0, "sgs_hero_29", Some("lord")),
            seat(1, "sgs_hero_210", None), // 周泰（奋激）
            seat(2, "sgs_hero_43", None),
            seat(3, "sgs_hero_299", None),
            seat(4, "sgs_hero_311", None),
        ],
    };
    let mut rng = SeededRng::new(20261105);
    let (state, _) = new_sandbox_game("r1", "g1", &spec, &mut rng).expect("沙盒开局");
    let g = game_of(&state);
    assert!(g.events.is_empty(), "开局发牌事件必须清空：{:?}", g.events);
    assert!(
        g.pending.is_none(),
        "开局不得触发获得/弃置类技能：{:?}",
        g.pending
    );
}
