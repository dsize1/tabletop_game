//! P4.2：寒冰剑（damage_before）与雌雄双股剑（异性目标二选一）。

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

/// 打出【杀】并指定目标，随后让目标显式放弃【闪】响应。
fn sha_and_give_up(mgr: &game_engine_core::GameManager, room: &str, cur: usize, target: usize) {
    let st = state_of(mgr, room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(mgr, room);
    if st.pending.as_ref().is_some_and(|p| p.kind == "respond") {
        mgr.execute_action(room, &target_pid, "pass").unwrap();
    }
}

#[test]
fn hanbing_prevents_damage_and_discards_two() {
    let mgr = manager(1600);
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
        give(st, target, "shan"); // 手牌数 ≥2（无黑桃杀?）——这里给两张非闪
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    sha_and_give_up(&mgr, &room, cur, target);

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("寒冰剑应询问是否防止伤害");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("hanbing"));
    assert_eq!(pending.actor, cur_pid);
    mgr.execute_action(&room, &cur_pid, "choose:hanbing")
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "寒冰剑防止伤害");
    assert_eq!(st.seats[target].hand.len(), 0, "目标两张牌被弃置");
    assert!(
        st.history.iter().any(|e| e.text.contains("寒冰剑")),
        "应有寒冰剑历史"
    );
    let _ = target_pid;
    st.check_conservation().unwrap();
}

#[test]
fn hanbing_pass_applies_damage() {
    let mgr = manager(1601);
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
    let hp = st.seats[target].hp;
    sha_and_give_up(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.skill.as_deref() == Some("hanbing")),
        "应询问寒冰剑"
    );
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "放弃寒冰剑则正常受伤");
    st.check_conservation().unwrap();
}

#[test]
fn hanbing_not_offered_with_single_card() {
    let mgr = manager(1602);
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
        give(st, target, "tao"); // 只有 1 张 → 无法弃 2
    });
    let st = state_of(&mgr, &room);
    let hp = st.seats[target].hp;
    sha_and_give_up(&mgr, &room, cur, target);
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "目标不足两张牌时不询问");
    assert_eq!(st.seats[target].hp, hp - 1, "正常受伤");
    st.check_conservation().unwrap();
}

#[test]
fn cixiong_target_discards_card() {
    let mgr = manager(1603);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2"); // 关羽（男）
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_25"); // 孙尚香（女）
        equip(st, cur, "cixiong_shuanggujian");
        give(st, cur, "sha");
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let card = st.seats[target].hand[0].clone();
    sha_and_give_up(&mgr, &room, cur, target);

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("雌雄应询问目标");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("cixiong"));
    assert_eq!(pending.actor, target_pid);
    mgr.execute_action(&room, &target_pid, "choose:discard")
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "confirm").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("雌雄双股剑")),
        "应有雌雄历史"
    );
    assert_eq!(
        st.seats[target].hp,
        hp - 1,
        "弃牌后仍需响应【杀】（空手 → 受伤）"
    );
    assert!(
        st.deck.discard.iter().any(|c| c == &card),
        "弃置的牌进弃牌堆"
    );
    st.check_conservation().unwrap();
}

#[test]
fn cixiong_lets_attacker_draw() {
    let mgr = manager(1604);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_25");
        equip(st, cur, "cixiong_shuanggujian");
        give(st, cur, "sha");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    sha_and_give_up(&mgr, &room, cur, target);
    mgr.execute_action(&room, &target_pid, "choose:draw")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 1, "攻击方摸 1 张");
    assert_eq!(st.seats[target].hand.len(), 1, "目标未弃牌");
    // 目标仍可出【闪】抵消
    let shan = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "出闪抵消");
    st.check_conservation().unwrap();
}

#[test]
fn cixiong_not_offered_same_gender() {
    let mgr = manager(1605);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2"); // 关羽（男）
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_3"); // 张飞（男）
        equip(st, cur, "cixiong_shuanggujian");
        give(st, cur, "sha");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let target_pid = pid(&st, target);
    sha_and_give_up(&mgr, &room, cur, target);
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| p.skill.as_deref() != Some("cixiong")),
        "同性目标不应触发雌雄"
    );
    assert!(!st.history.iter().any(|e| e.text.contains("雌雄双股剑")));
    let _ = target_pid;
    st.check_conservation().unwrap();
}
