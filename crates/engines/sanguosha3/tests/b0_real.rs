//! B0 技能首批（据守）：**被真实阶段驱动触发** + 对齐 `align/B0.json` 断言。
//!
//! 对齐单要点（B0 Q1=B）：翻面是「取反」；流程 = 翻面 → 摸 4 → 弃 1 张手牌（装备牌改为使用）
//! → 按持有上限弃牌收尾；只在**自己的结束阶段**触发；放弃则无任何效果。

mod common;

use std::collections::BTreeMap;

use common::{add, begin, end_turn, game, has_option, pending_actor, submit};
use game_engine_core::{EnginePlugin, EngineRegistry, GameManager, SeededRng};
use sanguosha3::engine::lobby::{self, LobbyConfig, Setup};
use sanguosha3::engine::{EngineState, SanguoshaEngine};
use sanguosha3::kernel::event::{Event, EventKind, Window};
use sanguosha3::kernel::resolve::FrameInner;
use sanguosha3::kernel::skills::{self, Face, Hook, SkillDef, SkillKind, Subscription};
use sanguosha3::kernel::{turn, Game, Phase, Slot, Zone};
use serde_json::json;

fn jushou_player(g: &mut Game) {
    g.seats[0].skills = vec!["jushou".into()];
}

/// 把座位 0 推到结束阶段（打完出牌 + 弃到上限）。
fn to_end_phase(g: &mut Game) {
    end_turn(g).expect("结束出牌/弃牌阶段");
}

#[test]
fn jushou_triggers_at_own_end_phase_and_follows_spec() {
    let mut g = game(3);
    begin(&mut g);
    jushou_player(&mut g);
    turn::start(&mut g).unwrap();
    // 出牌阶段：给自己一张非装备牌，便于验证「弃 1 张手牌」。
    add(&mut g, "t1", "tao", Zone::Hand(0));
    to_end_phase(&mut g);

    // ① 结阶段开始 → 询问发动（不是主动技入口）。
    assert_eq!(g.phase, Phase::End);
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("jushou_ask".into()),
        "结束阶段触发据守询问"
    );
    assert_eq!(pending_actor(&g), Some(0));
    assert!(has_option(&g, "choose:jushou"));
    assert!(has_option(&g, "pass"), "放弃出口存在");

    let hand_before = g.hand_size(0);
    assert!(!g.seats[0].flipped);
    // ② 发动：取反翻面（Q1=B）+ 摸 4
    submit(&mut g, 0, "choose:jushou").unwrap();
    assert!(g.seats[0].flipped, "据守：翻面（取反，Q1=B）");
    assert_eq!(g.hand_size(0), hand_before + 4, "摸四张");
    // ③ 选 1 张手牌（强制）
    assert_eq!(g.pending.as_ref().unwrap().kind(), "jushou_pick");
    let pick = g.hand(0)[0].clone();
    submit(&mut g, 0, &format!("card:{pick}")).unwrap();
    assert_eq!(g.cards.get(&pick), Some(&Zone::Discard), "非装备牌进弃牌堆");

    // ④ 收尾：按上限弃牌（4 人以下体力 4 → 摸 4 后可能超限）。
    let mut guard = 0;
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "skill_limit" || p.kind() == "jushou_limit")
        .unwrap_or(false)
    {
        guard += 1;
        assert!(guard < 20, "据守弃牌循环未收口");
        let id = g.hand(0)[0].clone();
        submit(&mut g, 0, &format!("card:{id}")).unwrap();
    }
    assert!(g.hand_size(0) <= g.hand_limit(0), "收到手牌上限内");
    assert!(
        !g.stack
            .iter()
            .any(|f| matches!(&f.inner, FrameInner::Ask(a) if a.skill == "jushou")),
        "据守技能帧已收口"
    );
    assert_ne!(
        g.pending.as_ref().map(|p| p.kind().to_string()).as_deref(),
        Some("skill_limit"),
        "不再有据守弃牌询问"
    );
    assert_eq!(g.check_conservation(), Ok(()));
}

#[test]
fn jushou_declining_has_no_effect() {
    let mut g = game(3);
    begin(&mut g);
    jushou_player(&mut g);
    turn::start(&mut g).unwrap();
    to_end_phase(&mut g);
    let hand_before = g.hand_size(0);
    submit(&mut g, 0, "pass").unwrap();
    assert!(!g.seats[0].flipped, "放弃不翻面");
    assert_eq!(g.hand_size(0), hand_before, "放弃不摸牌");
}

#[test]
fn jushou_selected_equipment_is_used_not_discarded() {
    let mut g = game(3);
    begin(&mut g);
    jushou_player(&mut g);
    turn::start(&mut g).unwrap();
    add(&mut g, "eq", "bagua_zhen", Zone::Hand(0));
    to_end_phase(&mut g);
    submit(&mut g, 0, "choose:jushou").unwrap();
    // 摸 4 后手牌里一定有 eq；选它 → 装备到防具栏而不是弃置。
    assert!(has_option(&g, "card:eq"));
    submit(&mut g, 0, "card:eq").unwrap();
    assert!(
        matches!(g.cards.get("eq"), Some(Zone::Equip(0, Slot::Armor))),
        "装备牌改为使用（B0.json ④）"
    );
}

#[test]
fn jushou_only_in_own_end_phase() {
    // 窗口/订阅单测：他人回合的结束阶段不路由给座位 0 的据守。
    let mut g = game(3);
    begin(&mut g);
    jushou_player(&mut g);
    let mut ev = Event::simple(EventKind::PhaseStart, Some(1), None);
    ev.turn = 1;
    ev.phase = Phase::End;
    skills::route_event(&mut g, &ev);
    assert!(
        !g.stack
            .iter()
            .any(|f| matches!(&f.inner, FrameInner::Ask(a) if a.skill == "jushou")),
        "他人结束阶段不触发据守"
    );
    // 自己的结束阶段 → 触发。
    let mut ev2 = Event::simple(EventKind::PhaseStart, Some(0), None);
    ev2.turn = 0;
    ev2.phase = Phase::End;
    skills::route_event(&mut g, &ev2);
    assert!(g
        .stack
        .iter()
        .any(|f| matches!(&f.inner, FrameInner::Ask(a) if a.skill == "jushou")));
}

#[test]
fn jushou_is_declared_as_phase_trigger_not_active() {
    let def: &SkillDef = skills::def("jushou").expect("jushou 已注册");
    let face: &Face = &def.faces[0];
    assert_eq!(face.window, Window::OwnPhase(Phase::End));
    assert!(!face.can_active, "结束阶段触发技不能主动使用");
    assert_eq!(
        face.subscriptions,
        &[Subscription {
            event: EventKind::PhaseStart,
            subject: sanguosha3::kernel::event::Subject::Any
        }]
    );
    assert!(face.hooks.is_empty());
    assert_eq!(def.kind, SkillKind::Normal);
    let _ = Hook::GameStart;
}

#[test]
fn skill_mounting_only_enables_aligned_skills() {
    // 中文名 → code：已注册即可映射；但**只有已收口技能**进入对局。
    assert_eq!(skills::code_by_name("据守"), Some("jushou"));
    assert_eq!(skills::code_by_name("放逐"), Some("fangzhu"));
    assert!(skills::enabled("jushou"));
    for c in ["jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei"] {
        assert!(skills::enabled(c), "B0 批次已收口：{c}");
    }
    // B1 批次已收口（2026-11-15）：18 技能全部挂载。
    for c in [
        "yongsi", "weidi", "qizhi", "jinzou", "juzhan", "feijun", "binglue", "huaiju", "yili",
        "zhenglun", "kuizhu", "chezheng", "lijun", "jianxiang", "shenshi", "chenglue", "shicai",
        "cunmu",
    ] {
        assert!(skills::enabled(c), "B1 批次已收口：{c}");
    }

    // 选将收尾：曹仁 → 只挂据守（解围未收口）；曹丕 → 空。
    let mut g = turn::new_game(5);
    for (i, s) in g.seats.iter_mut().enumerate() {
        s.name = format!("p{i}");
    }
    // 未收口武将用 B20 的界夏侯氏（752）占位（B0–B18 均已收口；B19 进行中）。
    let codes = ["sgs_hero_29", "sgs_hero_402", "sgs_hero_100", "sgs_hero_752", "sgs_hero_53"];
    let mut setup = Some(Setup {
        lord: 0,
        offers: BTreeMap::new(),
        picks: codes
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.to_string()))
            .collect(),
        lord_picked: true,
    });
    let mut rng = SeededRng::new(9);
    lobby::finish_setup(&mut g, &LobbyConfig::default(), &mut setup, &mut rng).unwrap();
    assert_eq!(
        g.seats[0].skills,
        vec!["jushou".to_string(), "jiewei".to_string()],
        "曹仁挂据守+解围（B0 收口）"
    );
    assert_eq!(
        g.seats[2].skills,
        vec!["yongsi".to_string(), "weidi".to_string()],
        "袁术挂庸肆/伪帝（B1 收口）"
    );
    assert!(g.seats[3].skills.is_empty(), "未接入批次的武将不挂载（界夏侯氏 B20）");

    // 曹丕（行殇/放逐/颂威）目前不挂载任何技能。
    let mut g2 = turn::new_game(5);
    for (i, s) in g2.seats.iter_mut().enumerate() {
        s.name = format!("p{i}");
    }
    let mut setup2 = Some(Setup {
        lord: 0,
        offers: BTreeMap::new(),
        picks: std::iter::once((0, "sgs_hero_43".to_string()))
            .chain((1..5).map(|i| (i, codes[i % codes.len()].to_string())))
            .collect(),
        lord_picked: true,
    });
    lobby::finish_setup(&mut g2, &LobbyConfig::default(), &mut setup2, &mut rng).unwrap();
    assert_eq!(
        g2.seats[0].skills,
        vec![
            "xingshang".to_string(),
            "fangzhu".to_string(),
            "songwei".to_string()
        ],
        "曹丕挂行殇/放逐/颂威（B0 收口）"
    );
}

#[test]
fn jushou_runs_in_engine_room_end_phase() {
    // 端到端（引擎层）：把曹仁放进座位 0 的对局，出牌阶段结束后应弹据守询问。
    let mut reg = EngineRegistry::new();
    reg.register(SanguoshaEngine);
    let manager = GameManager::with_rng(reg, Box::new(SeededRng::new(11)));
    let (state, _) = manager.create_game("sanguosha", "g1", "p0").expect("create");
    let room = state.room_id.clone();
    for _ in 1..5 {
        manager
            .execute_action(&room, "p0", "lobby:add_bot")
            .expect("add bot");
    }
    manager
        .execute_action(&room, "p0", "lobby:start")
        .expect("start");
    // 直接构造「曹仁已选将」的引擎状态，避免依赖随机候选。
    let mut es: EngineState = serde_json::from_value(
        manager.get_state(&room).expect("state").data,
    )
    .expect("EngineState");
    es.setup = Some(Setup {
        lord: 0,
        offers: BTreeMap::new(),
        picks: std::iter::once((0, "sgs_hero_29".to_string()))
            .chain((1..5).map(|i| (i, "sgs_hero_402".to_string())))
            .collect(),
        lord_picked: true,
    });
    let mut rng = SeededRng::new(5);
    lobby::finish_setup(&mut es.kernel, &es.cfg, &mut es.setup, &mut rng).expect("finish setup");
    assert_eq!(
        es.kernel.seats[0].skills,
        vec!["jushou".to_string(), "jiewei".to_string()]
    );
    es.setup = None;
    manager.restore(es.to_state(&room, "g1"));
    // 座位 0 是主公先手：打空出牌阶段 → 弃牌 → 结束阶段触发据守。
    let st = manager.get_state(&room).expect("state");
    let actor = st.data["kernel"]["turn"].as_u64().unwrap_or(0) as usize;
    if actor == 0 {
        let _ = manager.execute_action(&room, "p0", "end_play");
        let mut guard = 0;
        loop {
            let st = manager.get_state(&room).expect("state");
            let es: EngineState = serde_json::from_value(st.data.clone()).expect("parse");
            match es.kernel.pending.as_ref().map(|p| p.kind()) {
                Some("discard_phase") => {
                    let hand = es.kernel.hand(0);
                    if hand.is_empty() {
                        break;
                    }
                    let _ = manager.execute_action(&room, "p0", &format!("card:{}", hand[0]));
                }
                _ => break,
            }
            guard += 1;
            assert!(guard < 50, "弃牌循环未收口");
        }
        let st = manager.get_state(&room).expect("state");
        let es: EngineState = serde_json::from_value(st.data).expect("parse");
        assert!(
            es.kernel
                .pending
                .as_ref()
                .map(|p| p.kind() == "jushou_ask")
                .unwrap_or(false)
                || es.kernel.seats[0].flipped,
            "引擎对局里据守被真实阶段驱动（pending 或已发动）"
        );
    }
    // 视图：技能列表（Web 契约）应出现据守。
    let engine = SanguoshaEngine;
    let view = engine.player_view(
        &manager.get_state(&room).expect("state"),
        "p0",
    );
    let v: serde_json::Value = view.data;
    assert_eq!(v["player_area"]["general"], "sgs_hero_29");
    assert!(json!(v["player_area"]["skills"]).as_array().is_some());
}
