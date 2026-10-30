//! P4.2：方天画戟（最后一张手牌【杀】可额外指定至多两个目标）。

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
fn fangtian_hits_multiple_targets() {
    let mgr = manager(1900);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        equip(st, cur, "fangtian_huaji");
        for t in st.seats_from_left_of(cur).into_iter().take(3) {
            set_general(st, t, "sgs_hero_2");
        }
        give(st, cur, "sha"); // 唯一手牌
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp: Vec<i32> = st.seats.iter().map(|s| s.hp).collect();
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    let combo = acts
        .iter()
        .find(|a| a.id.starts_with("targets:"))
        .unwrap_or_else(|| panic!("方天应提供多目标组合：{acts:?}"))
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &combo).unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    let damaged = st
        .seats
        .iter()
        .enumerate()
        .filter(|(i, s)| *i != cur && s.hp < hp[*i])
        .count();
    assert_eq!(damaged, 2, "方天组合（2 目标）应各受 1 点伤害");
    assert!(
        st.deck.discard.iter().any(|c| c == &sha),
        "【杀】结算后弃置"
    );
    st.check_conservation().unwrap();
}

#[test]
fn fangtian_requires_last_hand_card() {
    let mgr = manager(1901);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        equip(st, cur, "fangtian_huaji");
        give(st, cur, "sha");
        give(st, cur, "shan"); // 手里还有别的牌 → 非最后一张
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("targets:")),
        "非最后一张手牌不应提供多目标：{acts:?}"
    );
}

#[test]
fn fangtian_extra_target_can_dodge() {
    let mgr = manager(1902);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut dodger = 0;
    let mut extra = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        equip(st, cur, "fangtian_huaji");
        dodger = st.seats_from_left_of(cur)[0];
        extra = st.seats_from_left_of(cur)[1];
        set_general(st, dodger, "sgs_hero_2");
        set_general(st, extra, "sgs_hero_2");
        give(st, cur, "sha");
        give(st, dodger, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dodger_pid = pid(&st, dodger);
    let extra_pid = pid(&st, extra);
    let dodger_hp = st.seats[dodger].hp;
    let extra_hp = st.seats[extra].hp;
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    let act = format!("targets:{dodger_pid},{extra_pid}");
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(acts.iter().any(|a| a.id == act), "组合选项应存在：{acts:?}");
    mgr.execute_action(&room, &cur_pid, &act).unwrap();

    let st = state_of(&mgr, &room);
    let shan = in_hand(&st, dodger, "shan");
    mgr.execute_action(&room, &dodger_pid, &format!("respond:{shan}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dodger].hp, dodger_hp, "第一个目标出闪抵消");
    assert_eq!(st.seats[extra].hp, extra_hp - 1, "第二个目标仍受伤");
    st.check_conservation().unwrap();
}
