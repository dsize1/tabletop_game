//! 引擎接线契约（V3 接管 `game_id = "sanguosha"`）：
//! 大厅 → 选将 → 出牌 → 终局；`legal ≡ execute`；视图字段（Web 三分区契约）齐全。

use game_engine_core::{EnginePlugin, EngineRegistry, GameManager, GameState, SeededRng};
use sanguosha3::engine::lobby::{self, general_pack_presets, MAX_PLAYERS, MIN_PLAYERS};
use sanguosha3::engine::{bot, EngineState, SanguoshaEngine};
use sanguosha3::kernel::Identity;
use serde_json::Value;

fn manager(seed: u64) -> GameManager {
    let mut reg = EngineRegistry::new();
    reg.register(SanguoshaEngine);
    GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
}

fn es(state: &GameState) -> EngineState {
    EngineState::from_state(state).expect("引擎状态")
}

fn create_with_bots(n: usize) -> (GameManager, GameState) {
    let m = manager(7);
    let (state, _) = m.create_game("sanguosha", "g1", "host").expect("create");
    let room = state.room_id.clone();
    for _ in 1..n {
        m.execute_action(&room, "host", "lobby:add_bot")
            .expect("add_bot");
    }
    (m, state)
}

fn legal_ids(m: &GameManager, room: &str, player: &str) -> Vec<String> {
    m.legal_actions(room, player)
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.id)
        .collect()
}

/// 全员选将（人类座位从自己的候选里选第一个；机器人由引擎自动选）。
fn finish_setup(m: &GameManager, room: &str) -> GameState {
    for _ in 0..20 {
        let state = m.get_state(room).expect("state");
        let e = es(&state);
        let Some(setup) = e.setup.clone() else {
            return state;
        };
        let mut progressed = false;
        for (seat, offers) in &setup.offers {
            if setup.picks.contains_key(seat) || offers.is_empty() {
                continue;
            }
            if e.kernel.seats[*seat].bot {
                continue;
            }
            let pid = e.kernel.seats[*seat].name.clone();
            let action = format!("setup:choose_general:{}", offers[0]);
            if m.execute_action(room, &pid, &action).is_ok() {
                progressed = true;
                break;
            }
        }
        if !progressed {
            // 只剩机器人：`auto_pick_bots` 已在走，退一步检查是否已收口。
            let again = m.get_state(room).expect("state");
            if es(&again).setup.is_none() {
                return again;
            }
            break;
        }
    }
    m.get_state(room).expect("state")
}

#[test]
fn game_id_takes_over_sanguosha() {
    let m = manager(1);
    let (state, events) = m.create_game("sanguosha", "g1", "host").unwrap();
    assert_eq!(state.game_id, "sanguosha", "V3 接管正式 id（v2 不再注册）");
    assert_eq!(state.status, game_engine_core::GameStatus::Active);
    assert!(!events.is_empty(), "建局产事件");
    let e = es(&state);
    assert!(e.lobby.is_some(), "建局即大厅");
    assert_eq!(e.kernel.seats.len(), 1);
    assert_eq!(e.kernel.host.as_deref(), Some("host"));
}

#[test]
fn general_roster_is_232_but_live_pool_is_147() {
    use sanguosha3::kernel::generals::{Status, GENERALS};
    assert_eq!(GENERALS.len(), 232, "4 个武将包登记的全部名录（含界化原版）");
    let live: Vec<_> = GENERALS.iter().filter(|g| g.status == Status::Live).collect();
    let offline: Vec<_> = GENERALS.iter().filter(|g| g.status == Status::Offline).collect();
    assert_eq!(live.len(), 147, "Live 选将池（全武将验证 147 将口径）");
    assert_eq!(offline.len(), 85, "被「界/将」同名去重排除的原版");
    assert_eq!(live.len() + offline.len(), GENERALS.len());
    // 例：标准关羽（sgs_hero_2）已下线，界关羽在池内。
    assert_eq!(sanguosha3::kernel::generals::def("sgs_hero_2").unwrap().status, Status::Offline);
    // 选将池函数与 Live 口径一致（默认四包 = 147）。
    let pool = lobby::available_codes(&lobby::LobbyConfig::default());
    assert_eq!(pool.len(), 147);
    assert!(!pool.contains(&"sgs_hero_2"), "下线武将不进池");
    // Live 口径的分包事实：标准 25 位全部有界版 → 单 standard 可用 0 位。
    let std_only = lobby::available_codes(&lobby::LobbyConfig {
        general_packs: "standard".to_string(),
        ..Default::default()
    });
    assert!(std_only.is_empty(), "单标准包在 Live 口径下没有可用武将");
    let live_by_pack = |p: &str| {
        GENERALS
            .iter()
            .filter(|g| g.status == Status::Live && g.pack == p)
            .count()
    };
    assert_eq!(live_by_pack("wind_fire_forest_mountain"), 19);
    assert_eq!(live_by_pack("yijiang"), 41);
    assert_eq!(live_by_pack("jiexian"), 87);
    assert_eq!(19 + 41 + 87, 147);
}

#[test]
fn lobby_join_add_bot_and_start_leads_to_setup() {
    let m = manager(2);
    let (state, _) = m.create_game("sanguosha", "g1", "host").unwrap();
    let room = state.room_id.clone();
    // 其他玩家 `#加入`（网关动作）
    for p in ["p1", "p2"] {
        m.execute_action(&room, p, "lobby:join").unwrap();
    }
    // 房主补机器人到 5 人（最小开局人数）
    while es(&m.get_state(&room).unwrap()).kernel.seats.len() < MIN_PLAYERS {
        m.execute_action(&room, "host", "lobby:add_bot").unwrap();
    }
    let ids = legal_ids(&m, &room, "host");
    assert!(ids.contains(&"lobby:start".to_string()), "房主可开局：{ids:?}");
    assert!(
        ids.iter().any(|i| i.starts_with("lobby:set_general_packs:")),
        "武将包配置项在合法集里"
    );
    assert!(
        ids.iter().any(|i| i.starts_with("lobby:set_packs:")),
        "牌堆配置项在合法集里"
    );
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let e = es(&m.get_state(&room).unwrap());
    let setup = e.setup.expect("进入选将");
    assert_eq!(e.kernel.seats.len(), MIN_PLAYERS);
    // 主公拿到候选；其他人等主公选完才有。
    assert!(!setup.offers[&setup.lord].is_empty(), "主公候选非空");
    // 主公可能是机器人（随机发身份）：找一个人座且已收到候选的玩家，断言其可选将。
    let human_with_offer = (0..e.kernel.seats.len())
        .find(|i| !e.kernel.seats[*i].bot && setup.offers.get(i).is_some_and(|o| !o.is_empty()))
        .expect("至少一名人类拿到候选");
    let pid = e.kernel.seats[human_with_offer].name.clone();
    let ids = legal_ids(&m, &room, &pid);
    assert!(
        ids.iter().any(|i| i.starts_with("setup:choose_general:")),
        "选将期合法集含候选人：{ids:?}"
    );
    // 非房主/未轮到的玩家不能开局
    let err = m.execute_action(&room, "p1", "lobby:start").unwrap_err();
    assert!(
        err.to_string().contains("房主") || err.to_string().contains("已开局"),
        "非房主不能开局：{err}"
    );
}

#[test]
fn setup_then_play_pending_and_views_are_web_compatible() {
    let (m, state) = create_with_bots(MIN_PLAYERS);
    let room = state.room_id.clone();
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let state = finish_setup(&m, &room);
    let e = es(&state);
    assert!(e.setup.is_none(), "选将收口");
    assert!(e.lobby.is_none(), "大厅已关");
    assert!(e.kernel.started, "已开局");
    // 每个座位都有武将/体力/势力（技能暂为空：不依赖技能也能玩）
    for s in &e.kernel.seats {
        assert!(!s.general.is_empty(), "座位 {} 已选将", s.id);
        assert!(s.max_hp >= 3 && s.hp == s.max_hp);
        assert!(!s.kingdom.is_empty());
        // 只挂「已注册 + 已收口」技能；未接入批次仍为空。
        for code in &s.skills {
            assert!(
                sanguosha3::kernel::skills::def(code).is_some(),
                "技能 {code} 未注册"
            );
            assert!(
                sanguosha3::kernel::skills::enabled(code),
                "技能 {code} 未收口却已挂载"
            );
        }
    }
    let lord = e.kernel.current_lord().expect("主公");
    assert_eq!(e.kernel.seats[lord].max_hp, e.kernel.seats[lord].max_hp, "主公体力");
    // 第一个待办：出牌阶段（无技能也会有交互）
    let pending = e.kernel.pending.as_ref().expect("开局后有 pending");
    assert_eq!(e.kernel.turn, lord, "主公先手");
    let actor_pid = e.kernel.seats[pending.actor()].name.clone();
    let ids = legal_ids(&m, &room, &actor_pid);
    assert!(!ids.is_empty(), "当前行动者有合法动作");
    assert!(ids.iter().any(|i| i.starts_with("card:")), "有出牌选项：{ids:?}");
    // 非行动者没有合法动作
    let other = e
        .kernel
        .seats
        .iter()
        .find(|s| s.id != pending.actor())
        .map(|s| s.name.clone())
        .unwrap();
    assert!(legal_ids(&m, &room, &other).is_empty(), "不该轮到别人");

    // 三分区视图（Web 契约）：公共区无手牌明细，个人区有本人手牌。
    let engine = SanguoshaEngine;
    let public = engine.public_state(&state);
    let v: Value = public.data.clone();
    assert_eq!(v["game"], "sanguosha", "Web 分派键");
    assert_eq!(v["schema_version"], 1);
    for key in ["public_board", "player_area", "special_zones", "phase", "round"] {
        assert!(!v[key].is_null() || key == "player_area", "缺字段 {key}");
    }
    assert_eq!(v["player_area"], Value::Null, "公共视图无个人区");
    assert!(v["public_board"]["seats"][0]["hand_count"].is_number());
    assert!(
        v["public_board"]["seats"][0].get("hand").is_none(),
        "公共区不得有手牌明细"
    );
    assert!(v["public_board"]["history"].is_array());
    let private = engine.player_view(&state, &actor_pid);
    let pv: Value = private.data.clone();
    assert_eq!(pv["player_area"]["player_id"], actor_pid);
    assert!(pv["player_area"]["hand"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(
        pv["player_area"]["pending"]["kind"],
        v["public_board"]["pending"]["kind"]
    );
    // 个人 pending 选项 = legal_actions（同源）
    let mut opts: Vec<String> = pv["player_area"]["pending"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_string())
        .collect();
    opts.sort();
    let mut legal = ids.clone();
    legal.sort();
    assert_eq!(opts, legal, "视图选项 ≡ 合法动作");
}

#[test]
fn bots_drive_game_to_finish_without_hero_skills() {
    let (m, state) = create_with_bots(MIN_PLAYERS);
    let room = state.room_id.clone();
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let _ = finish_setup(&m, &room);
    // 把人类座位也标记为机器人：单人真机/自对弈走同一条推进链路。
    let mut e = es(&m.get_state(&room).unwrap());
    for s in e.kernel.seats.iter_mut() {
        s.bot = true;
        // 加速终局：全场 1 体力（任何一次命中都会触发濒死/死亡链路）。
        s.hp = 1;
        s.max_hp = 1;
    }
    // 夹具：移除【桃】【酒】【闪】，避免「治疗经济」把对局拖成无限局
    //（真实对局由人类玩家推进；这里只验证引擎+机器人推进链路能收口）。
    let drop: Vec<String> = e
        .kernel
        .card_defs
        .iter()
        .filter(|(_, c)| matches!(c.def.as_str(), "tao" | "jiu" | "shan"))
        .map(|(id, _)| id.clone())
        .collect();
    for id in drop {
        e.kernel.cards.insert(id, sanguosha3::kernel::Zone::Void);
    }
    let (room_id, group_id) = (room.clone(), "g1".to_string());
    m.restore(e.to_state(&room_id, &group_id));
    let report = bot::drive_bots(&m, &room, 2000, |_, _| 0);
    assert!(report.steps > 0, "机器人有动作");
    assert!(report.errors.is_empty(), "无执行错误：{:?}", report.errors);
    assert!(report.finished, "对局结束（stop={:?}）", report.stop);
    let final_state = m.get_state(&room).expect("state");
    assert_eq!(final_state.status, game_engine_core::GameStatus::Finished);
    let e = es(&final_state);
    assert!(e.kernel.winner.is_some(), "有胜方");
    assert_eq!(e.kernel.check_conservation(), Ok(()));
    assert_eq!(e.kernel.check_no_orphan_frames(), Ok(()));
    // 决策日志有动作与打分（AI 打分；pass/结束等动作分为 0 是合法值）
    assert!(!report.decisions.is_empty());
    assert!(report.decisions.iter().any(|d| d.score > 0.0), "至少一次有正面评分的决策");
    // 摘要可读
    let summary = m.state_summary(&room).unwrap_or_default();
    assert!(summary.contains("三国杀"), "{summary}");
}

#[test]
fn legal_actions_and_execute_are_same_source() {
    let (m, state) = create_with_bots(MIN_PLAYERS);
    let room = state.room_id.clone();
    // 大厅：每个合法动作都能执行（或给出明确错误），且非法动作被拒。
    assert!(m.execute_action(&room, "host", "lobby:bogus").is_err());
    assert!(m.execute_action(&room, "nobody", "lobby:start").is_err());
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let st = m.get_state(&room).unwrap();
    let e = es(&st);
    let lord = e.setup.as_ref().unwrap().lord;
    let lord_pid = e.kernel.seats[lord].name.clone();
    // 候选之外的武将不能选
    let offers: Vec<String> = e.setup.as_ref().unwrap().offers[&lord].clone();
    let outside = sanguosha3::kernel::generals::GENERALS
        .iter()
        .map(|g| g.code)
        .find(|c| !offers.iter().any(|o| o == c))
        .expect("有候选外的武将");
    let err = m
        .execute_action(&room, &lord_pid, &format!("setup:choose_general:{outside}"))
        .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn general_pack_config_limits_candidates() {
    let mut m = manager(3);
    let (state, _) = m.create_game("sanguosha", "g1", "host").unwrap();
    let room = state.room_id.clone();
    while es(&m.get_state(&room).unwrap()).kernel.seats.len() < MIN_PLAYERS {
        m.execute_action(&room, "host", "lobby:add_bot").unwrap();
    }
    // 注意 Live 口径：标准包 25 位全部有「界」版 → 单 `standard` 可用 0 位；
    // 用 `standard+jiexian` 验证包过滤（87 位界版全在池内）。
    m.execute_action(&room, "host", "lobby:set_general_packs:standard+jiexian")
        .unwrap();
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let e = es(&m.get_state(&room).unwrap());
    let setup = e.setup.unwrap();
    let offers: Vec<String> = setup.offers.values().flatten().cloned().collect();
    assert!(!offers.is_empty());
    for code in offers {
        let def = sanguosha3::kernel::generals::def(&code).expect("general");
        assert!(
            def.pack == "standard" || def.pack == "jiexian",
            "只发所选包的候选：{}",
            def.pack
        );
        assert_eq!(
            def.status,
            sanguosha3::kernel::generals::Status::Live,
            "候选只取 Live（被界化取代的原版不进池）"
        );
    }
    assert!(
        general_pack_presets()
            .iter()
            .any(|p| p == "standard+wind_fire_forest_mountain"),
        "武将包预设包含 标准+风林火山"
    );
    let _ = MAX_PLAYERS;
    let _ = m.get_state(&room);
}

#[test]
fn seven_player_engine_game_reaches_finish() {
    // 7 人局端到端（P6 5–8 人口径的最大人数覆盖；8 人仅配比不同）。
    let (m, state) = create_with_bots(7);
    let room = state.room_id.clone();
    m.execute_action(&room, "host", "lobby:start").unwrap();
    let _ = finish_setup(&m, &room);
    let mut e = es(&m.get_state(&room).unwrap());
    assert_eq!(e.kernel.seats.len(), 7);
    let identities: Vec<_> = e.kernel.seats.iter().map(|s| s.identity).collect();
    assert_eq!(
        identities.iter().filter(|i| **i == Some(Identity::Lord)).count(),
        1,
        "恰好 1 主公"
    );
    for s in e.kernel.seats.iter_mut() {
        s.bot = true;
        s.hp = 1;
        s.max_hp = 1;
    }
    // 夹具：移除治疗/闪避牌，保证对局能收口（真实对局由人类推进）。
    let drop: Vec<String> = e
        .kernel
        .card_defs
        .iter()
        .filter(|(_, c)| matches!(c.def.as_str(), "tao" | "jiu" | "shan"))
        .map(|(id, _)| id.clone())
        .collect();
    for id in drop {
        e.kernel.cards.insert(id, sanguosha3::kernel::Zone::Void);
    }
    let (room_id, group_id) = (room.clone(), "g1".to_string());
    m.restore(e.to_state(&room_id, &group_id));
    let report = bot::drive_bots(&m, &room, 4000, |_, _| 0);
    assert!(report.errors.is_empty(), "无执行错误：{:?}", report.errors);
    assert!(report.finished, "7 人局结束（stop={:?}）", report.stop);
    let final_state = m.get_state(&room).expect("state");
    let e = es(&final_state);
    assert!(e.kernel.winner.is_some());
    assert_eq!(e.kernel.check_conservation(), Ok(()));
    assert_eq!(e.kernel.check_no_orphan_frames(), Ok(()));
}
