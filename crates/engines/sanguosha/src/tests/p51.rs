//! P4.2：丈八蛇矛（两张手牌当【杀】使用，主动使用部分）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, state_of, take_card};
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
fn zhangba_two_cards_as_sha() {
    let mgr = manager(1700);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip(st, cur, "zhangba_shemao");
        super::give(st, cur, "shan");
        super::give(st, cur, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let ids: Vec<String> = st.seats[cur].hand.clone();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    let opt = acts
        .iter()
        .find(|a| a.id.starts_with("skill:zhangba:"))
        .unwrap_or_else(|| panic!("丈八蛇矛应提供选项：{acts:?}"))
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &opt).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "两张手牌当【杀】造成 1 点伤害");
    assert_eq!(st.seats[cur].hand.len(), 0, "两张牌均已使用");
    assert!(
        ids.iter().all(|id| st.deck.discard.contains(id)),
        "两张牌均应进弃牌堆"
    );
    assert_eq!(st.counter("sha_used"), 1, "计入出杀次数");
    st.check_conservation().unwrap();
}

#[test]
fn zhangba_not_offered_with_one_card() {
    let mgr = manager(1701);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        equip(st, cur, "zhangba_shemao");
        super::give(st, cur, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("skill:zhangba:")),
        "只有 1 张手牌不应提供丈八选项：{acts:?}"
    );
}

#[test]
fn zhangba_respects_sha_limit() {
    let mgr = manager(1702);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        equip(st, cur, "zhangba_shemao");
        for code in ["sha", "shan", "tao"] {
            super::give(st, cur, code);
        }
        st.turn_counters.clear();
        st.set_counter("sha_used", 1);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("skill:zhangba:")),
        "已出过【杀】时丈八应受次数限制：{acts:?}"
    );
}
