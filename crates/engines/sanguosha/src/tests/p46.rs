//! P4.1：流离（大乔）—— 成为【杀】目标时弃一张手牌转移。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

/// 布置：cur 使用【杀】指定大乔；返回 (cur, 大乔, 转移候选)。
fn setup_liuli(st: &mut SgsState, dq_hand: bool) -> (usize, usize, usize) {
    clear_hands(st);
    let cur = st.turn;
    let dq = st.seats_from_left_of(cur)[0];
    set_general(st, dq, "sgs_hero_13"); // 大乔：流离
    let victim = st.seats_from_left_of(cur)[1];
    set_general(st, victim, "sgs_hero_2"); // 关羽（非空城）
    give(st, cur, "sha");
    if dq_hand {
        give(st, dq, "tao");
    }
    (cur, dq, victim)
}

#[test]
fn liuli_redirects_sha_to_another_target() {
    let mgr = manager(1200);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut dq = 0;
    let mut victim = 0;
    edit(&mgr, &room, |st| {
        (cur, dq, victim) = setup_liuli(st, true);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dq_pid = pid(&st, dq);
    let victim_pid = pid(&st, victim);
    let sha = st.seats[cur].hand[0].clone();
    let card = st.seats[dq].hand[0].clone();
    let dq_hp = st.seats[dq].hp;
    let victim_hp = st.seats[victim].hp;

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{dq_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("大乔应可选是否流离");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("liuli"));
    assert_eq!(pending.actor, dq_pid);

    mgr.execute_action(&room, &dq_pid, &format!("choose:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("流离应选择转移目标");
    assert_eq!(pending.kind, "target");
    assert!(
        !pending
            .options
            .iter()
            .any(|o| o.id == format!("target:{cur_pid}")),
        "不能转移给【杀】的使用者：{:?}",
        pending.options
    );
    mgr.execute_action(&room, &dq_pid, &format!("target:{victim_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dq].hp, dq_hp, "大乔不受伤");
    assert_eq!(st.seats[victim].hp, victim_hp - 1, "被转移者受到伤害");
    assert!(st.deck.discard.iter().any(|c| c == &card), "弃置牌进弃牌堆");
    assert!(
        st.history.iter().any(|e| e.text.contains("流离")),
        "应有流离历史：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    st.check_conservation().unwrap();
}

#[test]
fn liuli_pass_keeps_original_target() {
    let mgr = manager(1201);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut dq = 0;
    edit(&mgr, &room, |st| {
        (cur, dq, _) = setup_liuli(st, true);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dq_pid = pid(&st, dq);
    let sha = st.seats[cur].hand[0].clone();
    let dq_hp = st.seats[dq].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{dq_pid}"))
        .unwrap();
    mgr.execute_action(&room, &dq_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.skill.as_deref()),
        Some(None),
        "放弃流离后进入普通【闪】响应"
    );
    mgr.execute_action(&room, &dq_pid, "pass").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dq].hp, dq_hp - 1, "放弃流离后大乔受伤");
    st.check_conservation().unwrap();
}

#[test]
fn liuli_not_offered_without_hand_card() {
    let mgr = manager(1202);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut dq = 0;
    edit(&mgr, &room, |st| {
        (cur, dq, _) = setup_liuli(st, false);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dq_pid = pid(&st, dq);
    let sha = st.seats[cur].hand[0].clone();
    let dq_hp = st.seats[dq].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{dq_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| p.skill.as_deref() != Some("liuli")),
        "无手牌不提供流离选项"
    );
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dq].hp, dq_hp - 1, "无手牌大乔直接受伤");
    assert!(!st.history.iter().any(|e| e.text.contains("流离")));
    st.check_conservation().unwrap();
}
