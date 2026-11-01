//! AI 行为树 / 视图 / 剪枝的单测（门禁：`cargo test -p sanguosha ai_`）。
//!
//! 覆盖：
//! - 可见信息口径（他人手牌、他人身份不得进入视图）；
//! - 隐藏信息不变性（篡改他人手牌/牌堆顺序 → 决策不变）；
//! - 11 种 pending kind 的 handler 定向用例；
//! - 剪枝边界（≤26、保底项保留）；
//! - 5 人自对弈跑到终局（无非法动作、无卡死）。

use super::card;
use super::prune;
use super::tree;
use super::view::SgsView;
use crate::state::*;
use game_engine_core::{Action, EngineRegistry, GameManager, SeededRng};
use serde_json::json;

// ---------------------------------------------------------------------------
// 测试脚手架（独立于 `crate::tests`，避免耦合）
// ---------------------------------------------------------------------------

fn manager(seed: u64) -> GameManager {
    let mut reg = EngineRegistry::new();
    reg.register(crate::SanguoshaEngine);
    GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
}

fn lobby(mgr: &GameManager, players: usize) -> String {
    let (st, _) = mgr.create_game("sanguosha", "ai-group", "u1").expect("create");
    let room = st.room_id.clone();
    for i in 2..=players {
        mgr.execute_action(&room, &format!("u{i}"), "lobby:join")
            .expect("join");
    }
    room
}

fn state_of(mgr: &GameManager, room: &str) -> SgsState {
    SgsState::parse(&mgr.get_state(room).expect("room").data).expect("parse")
}

fn pick_all(mgr: &GameManager, room: &str) {
    for _ in 0..12 {
        let st = state_of(mgr, room);
        if st.phase != Phase::Setup {
            break;
        }
        let Some(pid) = st
            .seats
            .iter()
            .map(|s| s.player_id.clone())
            .find(|pid| {
                !mgr.legal_actions(room, pid)
                    .unwrap_or_default()
                    .iter()
                    .all(|a| !a.id.starts_with("setup:choose_general:"))
            })
        else {
            break;
        };
        let action = mgr
            .legal_actions(room, &pid)
            .unwrap_or_default()
            .into_iter()
            .find(|a| a.id.starts_with("setup:choose_general:"))
            .expect("offer");
        mgr.execute_action(room, &pid, &action.id).expect("pick");
    }
}

/// 开一局（5 人，标准包，停在出牌阶段）。
fn started(seed: u64) -> (GameManager, String) {
    let mgr = manager(seed);
    let room = lobby(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").expect("start");
    pick_all(&mgr, &room);
    (mgr, room)
}

fn find_card(st: &SgsState, code: &str) -> Option<String> {
    st.cards
        .keys()
        .find(|id| st.effective_def_code(id) == Some(code))
        .cloned()
}

fn seat_with_identity(st: &SgsState, identity: Identity) -> usize {
    st.seats
        .iter()
        .position(|s| s.identity == identity)
        .expect("identity")
}

/// 把 pending 注入到状态副本并投影视图（不执行动作）。
fn view_with(
    st: &mut SgsState,
    pid: &str,
    kind: &str,
    skill: Option<&str>,
    options: &[(&str, &str)],
    data: serde_json::Value,
) -> SgsView {
    st.pending = Some(Pending {
        kind: kind.to_string(),
        actor: pid.to_string(),
        skill: skill.map(str::to_string),
        options: options
            .iter()
            .map(|(id, label)| PendingOption {
                id: (*id).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
        data,
    });
    SgsView::from_state(st, pid).expect("view")
}

fn decide_id(view: &SgsView, legal: &[Action]) -> String {
    tree::decide(view, legal).expect("decision").action
}

// ---------------------------------------------------------------------------
// 1. 可见信息口径
// ---------------------------------------------------------------------------

#[test]
fn ai_view_hides_other_hands_and_identities() {
    let (mgr, room) = started(11);
    let st = state_of(&mgr, &room);
    let rebel = seat_with_identity(&st, Identity::Rebel);
    let me = st.seats[rebel].player_id.clone();
    let view = SgsView::from_state(&st, &me).expect("view");

    // 本人可见自己的手牌与身份。
    assert_eq!(view.identity, Identity::Rebel);
    assert!(!view.hand.is_empty(), "本人应看到自己的手牌明细");

    for s in &view.seats {
        if s.index == rebel {
            continue;
        }
        assert!(
            s.identity.is_none() || s.identity == Some(Identity::Lord),
            "非主公的他人身份必须隐藏（seat {} = {:?}）",
            s.index,
            s.identity
        );
        assert_eq!(
            s.hand_count, st.seats[s.index].hand.len(),
            "他人只能看到手牌张数"
        );
    }
    // 他人手牌 id 不得进入 known_cards。
    for (i, seat) in st.seats.iter().enumerate() {
        if i == rebel {
            continue;
        }
        for id in &seat.hand {
            assert!(
                !view.known_cards.contains_key(id),
                "他人手牌 {id} 泄漏到 known_cards"
            );
        }
    }
}

#[test]
fn hidden_info_invariance() {
    let (mgr, room) = started(12);
    let mut st = state_of(&mgr, &room);
    let actor = st.seats[st.turn].player_id.clone();
    st.turn_phase = TurnPhase::Play;
    let legal = mgr.legal_actions(&room, &actor).expect("legal");
    assert!(legal.len() >= 2, "用例需要 ≥2 个合法动作：{legal:?}");
    let before = decide_id(&SgsView::from_state(&st, &actor).expect("view"), &legal);

    // 篡改隐藏信息：他人手牌换成牌堆顶的牌（保持张数），牌堆顺序反转。
    let my = st.seat_index(&actor).expect("seat");
    let deck: Vec<String> = st.deck.draw.clone();
    let mut cursor = 0usize;
    for (i, seat) in st.seats.iter_mut().enumerate() {
        if i == my {
            continue;
        }
        for c in seat.hand.iter_mut() {
            if cursor < deck.len() {
                *c = deck[cursor].clone();
                cursor += 1;
            }
        }
    }
    st.deck.draw.reverse();

    let after = decide_id(&SgsView::from_state(&st, &actor).expect("view"), &legal);
    assert_eq!(before, after, "隐藏信息变化不应影响决策");
}

// ---------------------------------------------------------------------------
// 2. 各 pending kind 的 handler
// ---------------------------------------------------------------------------

#[test]
fn dying_self_uses_tao_and_enemy_takes_pass() {
    let (mgr, room) = started(21);
    let mut st = state_of(&mgr, &room);
    let tao = find_card(&st, "tao").expect("tao");
    let rebel = seat_with_identity(&st, Identity::Rebel);
    let lord = seat_with_identity(&st, Identity::Lord);
    let rebel_pid = st.seats[rebel].player_id.clone();
    let lord_pid = st.seats[lord].player_id.clone();
    st.seats[rebel].hand.push(tao.clone());
    st.seats[rebel].hp = 0;

    // 自己濒死：必出桃。
    let opts = [
        (format!("respond:{tao}"), "使用【桃】".to_string()),
        ("pass".to_string(), "放弃".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &rebel_pid,
        "dying",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "victim": rebel_pid, "hp": 0 }),
    );
    assert_eq!(decide_id(&view, &legal), format!("respond:{tao}"));

    // 敌方（主公）濒死：不救（选 pass）。
    let view = view_with(
        &mut st,
        &rebel_pid,
        "dying",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "victim": lord_pid, "hp": 0 }),
    );
    assert_eq!(decide_id(&view, &legal), "pass");
}

#[test]
fn respond_uses_dodge_when_danger() {
    let (mgr, room) = started(22);
    let mut st = state_of(&mgr, &room);
    let shan = find_card(&st, "shan").expect("shan");
    let me = st.seats[st.turn].player_id.clone();
    let my = st.turn;
    st.seats[my].hand.push(shan.clone());
    st.seats[my].hp = 1;

    let opts = [
        (format!("respond:{shan}"), "使用【闪】".to_string()),
        ("pass".to_string(), "放弃".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "respond",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "kind": "shan", "damage": 1 }),
    );
    assert_eq!(decide_id(&view, &legal), format!("respond:{shan}"));
}

#[test]
fn wuxie_and_judge_rewrite_default_to_pass() {
    let (mgr, room) = started(23);
    let mut st = state_of(&mgr, &room);
    let wuxie = find_card(&st, "wuxie_keji").expect("wuxie");
    let me = st.seats[st.turn].player_id.clone();
    let my = st.turn;
    st.seats[my].hand.push(wuxie.clone());

    let opts = [
        (format!("respond:{wuxie}"), "使用【无懈可击】".to_string()),
        ("pass".to_string(), "放弃".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    // 冲着自己来的锦囊：会用无懈（root 含本人武将名）。
    let my_name = st.seats[my].general_name.clone();
    let view = view_with(
        &mut st,
        &me,
        "wuxie",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "root": format!("对 {my_name} 使用【决斗】"), "layers": 1 }),
    );
    assert_eq!(decide_id(&view, &legal), format!("respond:{wuxie}"));

    // 与己无关：pass。
    let view = view_with(
        &mut st,
        &me,
        "wuxie",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "root": "对 某人 使用【决斗】", "layers": 1 }),
    );
    assert_eq!(decide_id(&view, &legal), "pass");

    // 改判：不用。
    let view = view_with(
        &mut st,
        &me,
        "judge_rewrite",
        Some("guicai"),
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "judge": "lebu" }),
    );
    assert_eq!(decide_id(&view, &legal), "pass");
}

#[test]
fn discard_dumps_lowest_value_card() {
    let (mgr, room) = started(24);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let my = st.turn;
    // 保证手里有「杀」和「闪电」：闪电价值最低。
    let sha = find_card(&st, "sha").expect("sha");
    let shandian = find_card(&st, "shandian").expect("shandian");
    st.seats[my].hand = vec![sha.clone(), shandian.clone()];

    let opts = [
        (format!("discard:{sha}"), "弃置【杀】".to_string()),
        (format!("discard:{shandian}"), "弃置【闪电】".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "discard",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "required": 1, "reason": "end_discard" }),
    );
    assert_eq!(decide_id(&view, &legal), format!("discard:{shandian}"));
}

#[test]
fn choose_many_selects_first_then_confirms() {
    let (mgr, room) = started(25);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let my = st.turn;
    let a = st.seats[my].hand[0].clone();
    let b = st.seats[my].hand[1].clone();

    let opts = [
        (format!("choose:{a}"), "选择卡牌 A".to_string()),
        (format!("choose:{b}"), "选择卡牌 B".to_string()),
        ("confirm".to_string(), "确认".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "choose_many",
        Some("rende"),
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "purpose": "rende", "selected": [], "min": 1, "max": 25 }),
    );
    let chosen = decide_id(&view, &legal);
    assert_ne!(chosen, "confirm", "未选够下限时不应确认");

    let view = view_with(
        &mut st,
        &me,
        "choose_many",
        Some("rende"),
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "purpose": "rende", "selected": [a], "min": 1, "max": 25 }),
    );
    assert_eq!(decide_id(&view, &legal), "confirm");
}

#[test]
fn pick_card_prefers_judge_zone() {
    let (mgr, room) = started(26);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let opts = [
        ("pick:hand".to_string(), "手牌".to_string()),
        ("pick:equip:weapon".to_string(), "武器".to_string()),
        ("pick:judge:judge_x".to_string(), "判定区".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "pick_card",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "card": "guohe_chaiqiao" }),
    );
    assert_eq!(decide_id(&view, &legal), "pick:judge:judge_x");
}

#[test]
fn wugu_pick_takes_highest_value() {
    let (mgr, room) = started(27);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let my = st.turn;
    let tao = find_card(&st, "tao").expect("tao");
    let shandian = find_card(&st, "shandian").expect("shandian");
    // 五谷亮出的牌进特殊区 → known_cards 可查到牌面。
    st.special_zones.insert(
        "wugu".to_string(),
        SpecialZone {
            owner: None,
            visibility: Visibility::Public,
            payload: {
                let mut m = serde_json::Map::new();
                m.insert(
                    "cards".into(),
                    json!([
                        { "id": tao, "def": "tao", "suit": "heart", "rank": 3 },
                        { "id": shandian, "def": "shandian", "suit": "spade", "rank": 1 },
                    ]),
                );
                m
            },
        },
    );
    let opts = [
        (format!("pick:{tao}"), "选择【桃】".to_string()),
        (format!("pick:{shandian}"), "选择【闪电】".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "wugu",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "zone": "wugu", "remaining": 2 }),
    );
    assert_eq!(decide_id(&view, &legal), format!("pick:{tao}"));
    let _ = my;
}

#[test]
fn arrange_submits_single_option() {
    let (mgr, room) = started(28);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let opts = [("arrange:c1,c2".to_string(), "确认观星顺序".to_string())];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "arrange",
        Some("guanxing"),
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({ "zone": "guanxing" }),
    );
    assert_eq!(decide_id(&view, &legal), "arrange:c1,c2");
}

#[test]
fn target_picks_lord_for_rebel() {
    let (mgr, room) = started(29);
    let mut st = state_of(&mgr, &room);
    let rebel = seat_with_identity(&st, Identity::Rebel);
    let lord = seat_with_identity(&st, Identity::Lord);
    let me = st.seats[rebel].player_id.clone();
    let lord_pid = st.seats[lord].player_id.clone();
    let other = st
        .seats
        .iter()
        .enumerate()
        .find(|(i, s)| *i != rebel && *i != lord && s.alive)
        .map(|(_, s)| s.player_id.clone())
        .expect("other");
    let opts = [
        (format!("target:{other}"), "选择目标".to_string()),
        (format!("target:{lord_pid}"), "选择目标".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "target",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({}),
    );
    assert_eq!(decide_id(&view, &legal), format!("target:{lord_pid}"));
}

#[test]
fn safe_advance_on_unknown_pending() {
    let (mgr, room) = started(30);
    let mut st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let opts = [
        ("weird:1".to_string(), "未知选项 1".to_string()),
        ("pass".to_string(), "放弃".to_string()),
    ];
    let legal: Vec<Action> = opts
        .iter()
        .map(|(id, d)| Action {
            id: id.clone(),
            description: d.clone(),
        })
        .collect();
    let view = view_with(
        &mut st,
        &me,
        "not_registered",
        None,
        &opts
            .iter()
            .map(|(i, d)| (i.as_str(), d.as_str()))
            .collect::<Vec<_>>(),
        json!({}),
    );
    let d = tree::decide(&view, &legal).expect("decision");
    assert_eq!(d.action, "pass");
    assert_eq!(d.reason, "safe_advance_unknown_pending");
}

// ---------------------------------------------------------------------------
// 3. 剪枝
// ---------------------------------------------------------------------------

#[test]
fn prune_keeps_bound_must_keep_and_order() {
    let (mgr, room) = started(31);
    let st = state_of(&mgr, &room);
    let me = st.seats[st.turn].player_id.clone();
    let view = SgsView::from_state(&st, &me).expect("view");
    let mut legal: Vec<Action> = (0..40)
        .map(|i| Action {
            id: format!("play:c{i}"),
            description: format!("使用【牌{i}】"),
        })
        .collect();
    legal.push(Action {
        id: "pass".into(),
        description: "放弃".into(),
    });
    legal.push(Action {
        id: "end_play".into(),
        description: "结束出牌阶段".into(),
    });
    let pruned = prune::prune(&view, &legal, prune::DEFAULT_LIMIT);
    assert_eq!(pruned.len(), prune::DEFAULT_LIMIT);
    assert!(pruned.iter().any(|a| a.id == "pass"));
    assert!(pruned.iter().any(|a| a.id == "end_play"));
    // 顺序保持原合法集顺序（Q/Web 的下标语义）。
    let idx: Vec<usize> = pruned
        .iter()
        .map(|a| legal.iter().position(|x| x.id == a.id).expect("idx"))
        .collect();
    assert!(idx.windows(2).all(|w| w[0] < w[1]), "剪枝后应保持原顺序");
}

// ---------------------------------------------------------------------------
// 4. 自对弈（快速版；100 局见 `npm run check:sgs-ai`）
// ---------------------------------------------------------------------------

#[test]
fn self_play_5p_reaches_finish() {
    let r = super::selfplay::run_self_play_with(5, 7, 6000, "standard");
    assert_eq!(r.illegal, 0, "不应出现非法动作：{:?}", r.last_error);
    assert_eq!(r.stalled, 0, "不应卡死：{:?}", r.last_error);
    assert!(r.finished, "应跑到终局（steps={}）", r.steps);
    assert!(r.steps > 10);
    assert!(
        r.pending_kinds.len() >= 4,
        "pending 覆盖过少：{:?}",
        r.pending_kinds
    );
}

#[test]
fn self_play_is_deterministic() {
    let a = super::selfplay::run_self_play_with(5, 9, 6000, "standard");
    let b = super::selfplay::run_self_play_with(5, 9, 6000, "standard");
    assert_eq!(a.steps, b.steps);
    assert_eq!(a.winner_team, b.winner_team);
    assert_eq!(a.pending_kinds, b.pending_kinds);
}

// ---------------------------------------------------------------------------
// 5. 牌价值表（确保 `card` 模块被引用，避免 dead_code 告警）
// ---------------------------------------------------------------------------

#[test]
fn card_values_available() {
    assert!(card::base_value("tao") > card::base_value("shan"));
}
