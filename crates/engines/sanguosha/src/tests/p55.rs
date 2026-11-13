//! P4.2：丈八蛇矛响应转化（两张手牌当【杀】打出）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn pass_wuxie(mgr: &game_engine_core::GameManager, room: &str) {
    for _ in 0..20 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending else { return };
        if p.kind != "wuxie" {
            return;
        }
        mgr.execute_action(room, &p.actor, "pass").unwrap();
    }
}

fn equip(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    st.seats[seat].equips.insert(EquipSlot::Weapon, Some(id));
}

#[test]
fn zhangba_responds_nanman_with_two_cards() {
    let mgr = manager(2100);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut zhangba = 0;
    let mut nanman = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zhangba = st.seats_from_left_of(cur)[0];
        set_general(st, zhangba, "sgs_hero_2");
        equip(st, zhangba, "zhangba_shemao");
        nanman = give(st, cur, "nanman_ruqin");
        give(st, zhangba, "shan");
        give(st, zhangba, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let zb_pid = pid(&st, zhangba);
    let hp = st.seats[zhangba].hp;
    let cards: Vec<String> = st.seats[zhangba].hand.clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("南蛮应询问【杀】");
    assert_eq!(pending.actor, zb_pid);
    let opt = pending
        .options
        .iter()
        .find(|o| o.id.starts_with("respond:") && o.id.contains(','))
        .unwrap_or_else(|| panic!("应有丈八双牌选项：{:?}", pending.options))
        .id
        .clone();
    mgr.execute_action(&room, &zb_pid, &opt).unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhangba].hp, hp, "丈八双牌当杀免伤");
    assert!(
        cards.iter().all(|c| st.deck.discard.contains(c)),
        "两张牌均应进弃牌堆"
    );
    assert!(st.history.iter().any(|e| e.text.contains("丈八蛇矛")));
    st.check_conservation().unwrap();
}

#[test]
fn zhangba_respond_needs_two_cards() {
    let mgr = manager(2101);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut zhangba = 0;
    let mut nanman = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zhangba = st.seats_from_left_of(cur)[0];
        set_general(st, zhangba, "sgs_hero_2");
        equip(st, zhangba, "zhangba_shemao");
        nanman = give(st, cur, "nanman_ruqin");
        give(st, zhangba, "tao"); // 只有 1 张
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let zb_pid = pid(&st, zhangba);
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("南蛮应询问");
    assert_eq!(pending.actor, zb_pid);
    assert!(
        !pending.options.iter().any(|o| o.id.contains(',')),
        "单张手牌不应有丈八双牌选项：{:?}",
        pending.options
    );
}
