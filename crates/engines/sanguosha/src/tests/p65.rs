//! P4.3：火攻（目标亮牌 → 同花色弃牌 → 火焰伤害）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of};
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

#[test]
fn huogong_discards_same_suit_for_fire_damage() {
    let mgr = manager(2900);
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
        give(st, cur, "huo_gong");
        give(st, cur, "wuzhong_shengyou"); // 红桃，与目标亮牌同花色（红桃）
        give(st, target, "tao"); // 红桃
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let huo = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "huo_gong"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{huo}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("火攻应询问是否弃同花色");
    assert_eq!(pending.skill.as_deref(), Some("huogong"));
    let opt = pending
        .options
        .iter()
        .find(|o| o.id.starts_with("choose:"))
        .unwrap_or_else(|| panic!("应有同花色弃牌选项：{:?}", pending.options))
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &opt).unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "火攻造成 1 点火焰伤害");
    assert!(st.history.iter().any(|e| e.text.contains("火攻")));
    st.check_conservation().unwrap();
}

#[test]
fn huogong_can_decline() {
    let mgr = manager(2901);
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
        give(st, cur, "huo_gong");
        give(st, cur, "wuzhong_shengyou");
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let huo = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "huo_gong"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{huo}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "放弃弃牌则不造成伤害");
    st.check_conservation().unwrap();
}
