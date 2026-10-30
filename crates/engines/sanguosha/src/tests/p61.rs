//! P4.5 收尾：窗口选项标注装备来源（前端武器高亮依据）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn equip(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    st.seats[seat].equips.insert(EquipSlot::Weapon, Some(id));
}

#[test]
fn equipment_option_carries_source_code() {
    let mgr = manager(2500);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip(st, cur, "hanbing_jian");
        give(st, cur, "sha");
        give(st, target, "tao");
        give(st, target, "juedou");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let sha = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("寒冰剑应询问");
    assert_eq!(pending.skill.as_deref(), Some("hanbing"));
    assert_eq!(
        pending.data["option_sources"]["choose:hanbing"].as_str(),
        Some("hanbing_jian"),
        "选项应标注装备来源：{:?}",
        pending.data
    );
    st.check_conservation().unwrap();
}
