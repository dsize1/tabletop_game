//! P4.3：朱雀羽扇（普通【杀】当火杀）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

#[test]
fn zhuque_turns_sha_into_fire_sha() {
    let mgr = manager(2800);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        st.seats[target].max_hp = 5;
        st.seats[target].hp = 5;
        let fang = super::find_card(st, "zhuque_yushan");
        take_card(st, &fang);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(fang));
        let teng = super::find_card(st, "tengjia");
        take_card(st, &teng);
        st.seats[target].equips.insert(EquipSlot::Armor, Some(teng));
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    let opt = acts
        .iter()
        .find(|a| a.id.starts_with("skill:zhuque:"))
        .unwrap_or_else(|| panic!("朱雀应提供转化选项：{acts:?}"))
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &opt).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp - 2,
        "朱雀火杀对藤甲造成 2 点（火伤 +1）"
    );
    assert!(st.history.iter().any(|e| e.text.contains("朱雀羽扇")));
    st.check_conservation().unwrap();
}

#[test]
fn zhuque_only_converts_plain_sha() {
    let mgr = manager(2801);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        let fang = super::find_card(st, "zhuque_yushan");
        take_card(st, &fang);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(fang));
        give(st, cur, "huo_sha");
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("skill:zhuque:")),
        "火杀本身不应出现朱雀转化选项：{acts:?}"
    );
}
