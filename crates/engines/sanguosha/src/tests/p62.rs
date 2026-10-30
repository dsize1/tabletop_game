//! P4.3 首批：酒 / 属性杀（火/雷）/ 藤甲 / 白银狮子 / 古锭刀。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn equip(st: &mut SgsState, seat: usize, code: &str, slot: EquipSlot) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    st.seats[seat].equips.insert(slot, Some(id));
}

fn play_sha(mgr: &game_engine_core::GameManager, room: &str, cur: usize, target: usize) {
    let st = state_of(mgr, room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let sha = st.seats[cur]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        })
        .cloned()
        .expect("手里没有可用的【杀】");
    mgr.execute_action(room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(mgr, room, &mut Vec::new());
}

#[test]
fn jiu_boosts_next_sha() {
    let mgr = manager(2600);
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
        give(st, cur, "jiu");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp = st.seats[target].hp;
    let jiu = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "jiu"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiu}"))
        .unwrap();
    play_sha(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "酒：下一张【杀】伤害 +1");
    assert_eq!(st.counter("jiu"), 0, "酒加成用后清零");
}

#[test]
fn fire_sha_ignores_tengjia_armor() {
    let mgr = manager(2601);
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
        equip(st, target, "tengjia", EquipSlot::Armor);
        give(st, cur, "huo_sha");
    });
    let st = state_of(&mgr, &room);
    let hp = st.seats[target].hp;
    play_sha(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "藤甲：火焰伤害 +1（1+1）");
    st.check_conservation().unwrap();
}

#[test]
fn tengjia_blocks_normal_sha() {
    let mgr = manager(2602);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip(st, target, "tengjia", EquipSlot::Armor);
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let hp = st.seats[target].hp;
    play_sha(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "藤甲：普通【杀】无效");
    assert!(st.history.iter().any(|e| e.text.contains("藤甲")));
    st.check_conservation().unwrap();
}

#[test]
fn baiyin_shizi_caps_damage_to_one() {
    let mgr = manager(2603);
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
        equip(st, target, "baiyin_shizi", EquipSlot::Armor);
        give(st, cur, "jiu");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp = st.seats[target].hp;
    let jiu = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "jiu"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiu}"))
        .unwrap();
    play_sha(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "白银狮子：受到的伤害改为 1");
    st.check_conservation().unwrap();
}

#[test]
fn guding_dao_boosts_sha_on_handless_target() {
    let mgr = manager(2604);
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
        equip(st, cur, "guding_dao", EquipSlot::Weapon);
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let hp = st.seats[target].hp;
    play_sha(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "古锭刀：目标无手牌时伤害 +1");
    st.check_conservation().unwrap();
}
