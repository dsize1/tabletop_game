//! P4.4.1 于吉「蛊惑」：质疑 / 真假 / 缠怨。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, give, manager, state_of};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

#[test]
fn guhuo_real_card_questioned_still_works_and_grants_chanyuan() {
    let mgr = manager(600);
    let room = start_and_pick(&mgr, 5);
    let (mut yj, mut target, mut questioner, mut sha) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yj = st.turn;
        target = st.seats_from_left_of(yj)[0];
        questioner = st.seats_from_left_of(yj)[1];
        set_general(st, yj, "sgs_hero_211");
        set_general(st, target, "sgs_hero_2");
        sha = give(st, yj, "sha");
    });
    let st = state_of(&mgr, &room);
    let (yj_pid, target_pid, q_pid) = (pid(&st, yj), pid(&st, target), pid(&st, questioner));
    mgr.execute_action(&room, &yj_pid, "skill:guhuo").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{yj_pid}|choose_many=choose:{sha}"),
            format!("{yj_pid}|choose_many=confirm"),
            format!("{yj_pid}|choice=declare:sha"),
            format!("{q_pid}|choice=choose:question"),
            format!("{yj_pid}|target=target:{target_pid}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[questioner].flags.contains_key("chanyuan"),
        "质疑真牌者应获得缠怨"
    );
    assert_eq!(
        st.seats[target].hp,
        st.seats[target].max_hp - 1,
        "真牌质疑后仍按声明结算"
    );
    assert!(st.history.iter().any(|e| e.text.contains("为真")));
    assert!(st.history.iter().any(|e| e.text.contains("蛊惑")));
    assert_eq!(st.seats[target].player_id, target_pid);
}

#[test]
fn guhuo_fake_card_questioned_is_voided() {
    let mgr = manager(601);
    let room = start_and_pick(&mgr, 5);
    let (mut yj, mut target, mut questioner, mut tao) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yj = st.turn;
        target = st.seats_from_left_of(yj)[0];
        questioner = st.seats_from_left_of(yj)[1];
        set_general(st, yj, "sgs_hero_211");
        set_general(st, target, "sgs_hero_2");
        tao = give(st, yj, "tao");
    });
    // 用【桃】声明【杀】（假牌），且存在可杀目标。
    let st = state_of(&mgr, &room);
    let (yj_pid, target_pid, q_pid) = (pid(&st, yj), pid(&st, target), pid(&st, questioner));
    let hp_before = st.seats[target].hp;
    mgr.execute_action(&room, &yj_pid, "skill:guhuo").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{yj_pid}|choose_many=choose:{tao}"),
            format!("{yj_pid}|choose_many=confirm"),
            format!("{yj_pid}|choice=declare:sha"),
            format!("{q_pid}|choice=choose:question"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_before, "假牌被质疑后不产生效果");
    assert!(st.deck.discard.contains(&tao), "假牌应作废进入弃牌堆");
    assert!(st.history.iter().any(|e| e.text.contains("为假")));
    assert_eq!(st.seats[target].player_id, target_pid);
}

#[test]
fn guhuo_unquestioned_fake_still_takes_effect() {
    let mgr = manager(602);
    let room = start_and_pick(&mgr, 5);
    let (mut yj, mut target, mut tao) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yj = st.turn;
        target = st.seats_from_left_of(yj)[0];
        set_general(st, yj, "sgs_hero_211");
        set_general(st, target, "sgs_hero_2");
        tao = give(st, yj, "tao");
    });
    let st = state_of(&mgr, &room);
    let yj_pid = pid(&st, yj);
    mgr.execute_action(&room, &yj_pid, "skill:guhuo").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{yj_pid}|choose_many=choose:{tao}"),
            format!("{yj_pid}|choose_many=confirm"),
            format!("{yj_pid}|choice=declare:sha"),
            format!(
                "{yj_pid}|target=target:{}",
                pid(&state_of(&mgr, &room), target)
            ),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        st.seats[target].max_hp - 1,
        "无人质疑时按声明结算"
    );
}

#[test]
fn guhuo_responds_as_shan() {
    let mgr = manager(603);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut yj, mut sha, mut tao) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        yj = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, yj, "sgs_hero_211");
        sha = give(st, cur, "sha");
        tao = give(st, yj, "tao");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, yj_pid) = (pid(&st, cur), pid(&st, yj));
    let hp_before = st.seats[yj].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{yj_pid}"))
        .unwrap();
    mgr.execute_action(&room, &yj_pid, &format!("skill:guhuo:{tao}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[yj].hp, hp_before, "蛊惑当【闪】应抵消【杀】");
    assert!(st.deck.discard.contains(&tao));
    assert!(st.history.iter().any(|e| e.text.contains("蛊惑")));
}

#[test]
fn chanyuan_locks_hand_cards_at_one_hp() {
    let mgr = manager(604);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
        st.seats[cur].flags.insert("chanyuan".into(), true);
        st.seats[cur].hp = 1;
        give(st, cur, "sha");
        give(st, cur, "guohe_chaiqiao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("play:")),
        "缠怨 1 血不能使用手牌：{acts:?}"
    );
    assert!(acts.iter().any(|a| a.id == "end_play"));
}
