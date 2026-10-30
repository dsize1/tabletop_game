//! P4.5 阶段三：AfterDodge 效果函数注册表（handler 表）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn in_hand(st: &SgsState, idx: usize, code: &str) -> String {
    st.seats[idx]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).map(|c| c.def.as_str()) == Some(code))
        .cloned()
        .unwrap_or_else(|| panic!("{} 手里没有 {code}", st.seats[idx].player_id))
}

fn equip(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    st.seats[seat].equips.insert(EquipSlot::Weapon, Some(id));
}

#[test]
fn after_dodge_handlers_are_registered() {
    let codes: Vec<&str> = crate::resolve_skill::AFTER_DODGE_HANDLERS
        .iter()
        .map(|h| h.code)
        .collect();
    assert_eq!(codes, vec!["qinglong_yanyue", "guanshi_fu"]);
    for code in &codes {
        let def = crate::skills::EQUIPMENT_SKILLS
            .iter()
            .find(|d| d.code == *code)
            .unwrap_or_else(|| panic!("注册表缺少 {code}"));
        assert_eq!(
            def.trigger,
            crate::skills::Trigger::AfterDodge,
            "{code} 的触发时机应为 AfterDodge"
        );
    }
}

#[test]
fn qinglong_without_sha_has_no_window() {
    let mgr = manager(2400);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip(st, cur, "qinglong_yanyue");
        give(st, cur, "sha");
        give(st, cur, "shan");
        give(st, cur, "tao");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending.is_none(),
        "手里没有第二张【杀】时青龙不应开追加窗口"
    );
    assert_eq!(st.seats[target].hp, hp, "闪抵消");
    st.check_conservation().unwrap();
}
