//! 工作台场景注入/状态校验单测（P5 基座）。

use super::*;
use crate::scene::{apply_scene, validate_state};

fn full_state(mgr: &GameManager, room: &str) -> game_engine_core::GameState {
    mgr.get_state(room).expect("room")
}

#[test]
fn apply_scene_sets_general_hand_phase_and_keeps_conservation() {
    let mgr = manager(7);
    let room = start_with_config(&mgr, 5, |c| c.general_packs = vec!["standard".into()]);
    let base = full_state(&mgr, &room);
    let before = SgsState::parse(&base.data).unwrap();
    let total_cards = before.cards.len();

    let scene = serde_json::json!({
        "phase": "turn",
        "turn": 0,
        "turn_phase": "play",
        "round": 2,
        "seats": [{
            "index": 0,
            "general": "sgs_hero_1",
            "identity": "lord",
            "hp": 3,
            "hand": ["sha", "shan"],
            "equips": { "weapon": "qinggang_jian" },
            "judge": ["lebu_sishu"],
            "mark_counts": { "test_mark": 2 },
            "flags": { "awakened": false }
        }]
    });
    let out = apply_scene(&base, &scene).expect("apply scene");
    validate_state(&out).expect("valid state");
    let st = SgsState::parse(&out.data).unwrap();

    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(st.round, 2);
    let seat = &st.seats[0];
    assert_eq!(seat.general.as_deref(), Some("sgs_hero_1"));
    assert_eq!(seat.general_name, "刘备");
    assert_eq!(seat.identity, Identity::Lord);
    // 主公 +1（刘备 4 → 5），随后显式 hp=3 覆盖。
    assert_eq!(seat.max_hp, 5);
    assert_eq!(seat.hp, 3);
    assert_eq!(seat.mark_counts.get("test_mark"), Some(&2));
    let hand_defs: Vec<&str> = seat
        .hand
        .iter()
        .map(|id| st.cards.get(id).unwrap().def.as_str())
        .collect();
    assert_eq!(hand_defs, vec!["sha", "shan"]);
    assert_eq!(st.equip_def(0, EquipSlot::Weapon).map(|d| d.code), Some("qinggang_jian"));
    assert_eq!(st.seats[0].judge_zone.len(), 1);
    // 牌实例总数不变（只移动不新增）。
    assert_eq!(st.cards.len(), total_cards);
    st.check_conservation().expect("conservation");
}

#[test]
fn apply_scene_deck_top_last_is_top_and_moves_only() {
    let mgr = manager(11);
    let room = start_with_config(&mgr, 5, |c| c.general_packs = vec!["standard".into()]);
    let base = full_state(&mgr, &room);
    let before = SgsState::parse(&base.data).unwrap();
    let top_two: Vec<String> = before.deck.draw.iter().rev().take(2).cloned().collect();
    assert_eq!(top_two.len(), 2);

    // 用牌实例 id 精确定位；数组末尾 = 牌堆顶。
    let scene = serde_json::json!({ "deck_top": [top_two[1].clone(), top_two[0].clone()] });
    let out = apply_scene(&base, &scene).expect("apply scene");
    let st = SgsState::parse(&out.data).unwrap();
    assert_eq!(st.deck.draw.last(), Some(&top_two[0]));
    assert_eq!(
        st.deck.draw.get(st.deck.draw.len().saturating_sub(2)),
        Some(&top_two[1])
    );
    assert_eq!(st.cards.len(), before.cards.len());
}

#[test]
fn apply_scene_rejects_invalid_inputs() {
    let mgr = manager(3);
    let room = start_with_config(&mgr, 5, |c| c.general_packs = vec!["standard".into()]);
    let base = full_state(&mgr, &room);

    let dup_id = SgsState::parse(&base.data).unwrap().deck.draw[0].clone();
    let cases = [
        serde_json::json!({ "seats": [{ "index": 0, "general": "nope" }] }),
        serde_json::json!({ "seats": [{ "index": 0, "hand": [dup_id, dup_id] }] }),
        serde_json::json!({ "seats": [{ "index": 0, "identity": "lord", "hp": 99 }] }),
        serde_json::json!({ "unknown_field": 1 }),
        serde_json::json!({ "seats": [{ "index": 99 }] }),
        serde_json::json!({ "seats": [{ "index": 0, "hand": ["not_a_card"] }] }),
    ];
    for (i, scene) in cases.iter().enumerate() {
        assert!(
            apply_scene(&base, scene).is_err(),
            "case {i} 应被拒绝：{scene}"
        );
    }
    // 原状态不被修改（纯函数）。
    validate_state(&base).unwrap();
}

#[test]
fn validate_state_rejects_broken_snapshots() {
    let mgr = manager(5);
    let room = start_with_config(&mgr, 5, |c| c.general_packs = vec!["standard".into()]);
    let base = full_state(&mgr, &room);
    validate_state(&base).expect("base valid");

    // 1) 牌从所有区域消失（仍留在 cards）→ 不守恒。
    let mut st = SgsState::parse(&base.data).unwrap();
    let card = st.deck.draw.pop().unwrap();
    let mut broken = base.clone();
    broken.data = serde_json::to_value(&st).unwrap();
    assert!(validate_state(&broken).is_err());
    // 复原
    st.deck.draw.push(card);

    // 2) 同一张牌出现在两个区域。
    let dup = st.deck.draw.last().cloned().unwrap();
    st.seats[0].hand.push(dup);
    broken.data = serde_json::to_value(&st).unwrap();
    assert!(validate_state(&broken).is_err());

    // 3) 未知武将。
    let mut st = SgsState::parse(&base.data).unwrap();
    st.seats[1].general = Some("nope".into());
    broken.data = serde_json::to_value(&st).unwrap();
    assert!(validate_state(&broken).is_err());
}
