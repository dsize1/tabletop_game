//! P4.3：铁索连环（横置/重置、重铸、属性伤害传导）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

#[test]
fn tiesuo_toggles_chained_state() {
    let mgr = manager(3000);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "tiesuo_lianhuan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].chained, "铁索应横置目标");
    st.check_conservation().unwrap();
}

#[test]
fn tiesuo_recast_discards_and_draws() {
    let mgr = manager(3001);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        give(st, cur, "tiesuo_lianhuan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let card = st.seats[cur].hand[0].clone();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("recast:{card}")),
        "铁索应提供重铸选项：{acts:?}"
    );
    mgr.execute_action(&room, &cur_pid, &format!("recast:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.iter().any(|c| c == &card), "重铸弃置该牌");
    assert_eq!(st.seats[cur].hand.len(), 1, "重铸摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("重铸")));
    st.check_conservation().unwrap();
}

#[test]
fn fire_damage_chains_and_resets() {
    let mgr = manager(3002);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut a = 0;
    let mut b = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        a = st.seats_from_left_of(cur)[0];
        b = st.seats_from_left_of(cur)[1];
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_3");
        st.seats[a].max_hp = 5;
        st.seats[a].hp = 5;
        st.seats[b].max_hp = 5;
        st.seats[b].hp = 5;
        st.seats[a].chained = true;
        st.seats[b].chained = true;
        give(st, cur, "huo_sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let a_pid = pid(&st, a);
    let hp_a = st.seats[a].hp;
    let hp_b = st.seats[b].hp;
    let sha = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[a].hp, hp_a - 1, "目标受到火焰伤害");
    assert_eq!(st.seats[b].hp, hp_b - 1, "横置角色受到传导伤害");
    assert!(!st.seats[a].chained && !st.seats[b].chained, "传导后重置");
    st.check_conservation().unwrap();
}
