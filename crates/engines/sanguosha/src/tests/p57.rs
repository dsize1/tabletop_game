//! P4.5 阶段二：`player_area.equipment_skills` 视图 + 按 Trigger 派发。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, player_view, public_view, state_of, take_card};
use crate::state::*;

fn equip(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    let slot = if code == "bagua_zhen" || code == "renwang_dun" {
        EquipSlot::Armor
    } else {
        EquipSlot::Weapon
    };
    st.seats[seat].equips.insert(slot, Some(id));
}

#[test]
fn player_view_exposes_equipment_skills() {
    let mgr = manager(2300);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
        equip(st, cur, "qinglong_yanyue");
        equip(st, cur, "bagua_zhen");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[cur].player_id.clone();
    let pv = player_view(&mgr, &room, &cur_pid);
    let list = pv["player_area"]["equipment_skills"]
        .as_array()
        .expect("equipment_skills")
        .clone();
    assert_eq!(list.len(), 2, "两件装备应各有一项技能");
    let codes: Vec<&str> = list.iter().filter_map(|v| v["code"].as_str()).collect();
    assert!(codes.contains(&"qinglong_yanyue"));
    assert!(codes.contains(&"bagua_zhen"));
    let qinglong = list
        .iter()
        .find(|v| v["code"] == "qinglong_yanyue")
        .unwrap();
    assert_eq!(qinglong["trigger"], "after_dodge");
    assert_eq!(qinglong["slot"], "weapon");
    let bagua = list.iter().find(|v| v["code"] == "bagua_zhen").unwrap();
    assert_eq!(bagua["trigger"], "judge_before");
    assert_eq!(bagua["slot"], "armor");

    let public = public_view(&mgr, &room);
    assert!(
        public["public_board"].get("equipment_skills").is_none(),
        "装备技能仅在个人资源区下发"
    );
}

#[test]
fn no_equipment_skills_without_equipment() {
    let mgr = manager(2301);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[cur].player_id.clone();
    let pv = player_view(&mgr, &room, &cur_pid);
    assert_eq!(
        pv["player_area"]["equipment_skills"]
            .as_array()
            .map(|a| a.len()),
        Some(0)
    );
}
