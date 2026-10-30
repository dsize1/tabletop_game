//! P4.2（部分）：青龙偃月刀 / 贯石斧 —— 被【闪】抵消后的追加窗口。

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
fn qinglong_uses_second_sha_after_dodge() {
    let mgr = manager(1500);
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
        give(st, cur, "sha");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let sha1 = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha1}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("青龙偃月刀应提供追加窗口");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("after_dodge"));
    assert_eq!(pending.actor, cur_pid);
    let sha2 = in_hand(&st, cur, "sha");
    assert!(
        pending
            .options
            .iter()
            .any(|o| o.id == format!("choose:{sha2}")),
        "应可选择第二张【杀】：{:?}",
        pending.options
    );
    mgr.execute_action(&room, &cur_pid, &format!("choose:{sha2}"))
        .unwrap();
    // 目标已无【闪】→ 自动放弃响应并受伤。
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "第二张【杀】命中");
    assert!(
        st.history.iter().any(|e| e.text.contains("青龙偃月刀")),
        "应有青龙偃月刀历史"
    );
    st.check_conservation().unwrap();
}

#[test]
fn guanshi_discards_two_to_land_sha() {
    let mgr = manager(1501);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip(st, cur, "guanshi_fu");
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
    let pending = st.pending.clone().expect("贯石斧应提供追加窗口");
    assert!(
        pending.options.iter().any(|o| o.id == "choose:guanshi"),
        "应可发动贯石斧：{:?}",
        pending.options
    );
    mgr.execute_action(&room, &cur_pid, "choose:guanshi")
        .unwrap();
    let st = state_of(&mgr, &room);
    let cards: Vec<String> = st.seats[cur].hand.iter().take(2).cloned().collect();
    for c in &cards {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{c}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "贯石斧：目标仍受到伤害");
    assert_eq!(st.seats[cur].hand.len(), 0, "打出杀后弃置 2 张（共 3 张）");
    assert!(
        st.history.iter().any(|e| e.text.contains("贯石斧")),
        "应有贯石斧历史"
    );
    st.check_conservation().unwrap();
}

#[test]
fn no_after_dodge_without_weapon() {
    let mgr = manager(1502);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "sha");
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
    assert!(st.pending.is_none(), "无武器不应有追加窗口");
    assert_eq!(st.seats[target].hp, hp, "闪成功抵消");
    st.check_conservation().unwrap();
}
