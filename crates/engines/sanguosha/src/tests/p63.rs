//! P4.3：兵粮寸断（延时锦囊：非黑桃跳过摸牌）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive as drive_state, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

#[test]
fn bingliang_skips_draw_on_non_spade() {
    let mgr = manager(2700);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        let delayed = super::find_card(st, "bingliang_cunduan");
        take_card(st, &delayed);
        st.seats[target].judge_zone.push(delayed);
        // 判定：梅花（非黑桃）→ 跳过摸牌
        let club = st
            .cards
            .iter()
            .find(|(_, c)| c.suit == Suit::Club)
            .map(|(id, _)| id.clone())
            .expect("梅花牌");
        take_card(st, &club);
        st.deck.draw.push(club);
        st.turn = target;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play, "跳过摸牌后直接进入出牌阶段");
    assert_eq!(st.seats[target].hand.len(), 0, "未摸牌");
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("非黑桃，跳过摸牌阶段")),
        "兵粮应生效：{:?}",
        st.history
            .iter()
            .rev()
            .take(6)
            .map(|e| &e.text)
            .collect::<Vec<_>>()
    );
    st.check_conservation().unwrap();
}

#[test]
fn bingliang_target_requires_distance_one() {
    let mgr = manager(2701);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut far = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = (cur + 2) % st.seats.len();
        set_general(st, far, "sgs_hero_2");
        give(st, cur, "bingliang_cunduan");
    });
    let st = state_of(&mgr, &room);
    let far_pid = pid(&st, far);
    let cur_pid = pid(&st, cur);
    let card = st.seats[cur]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| c.def == "bingliang_cunduan")
        })
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("兵粮应询问目标");
    assert!(
        !pending
            .options
            .iter()
            .any(|o| o.id == format!("target:{far_pid}")),
        "距离 2 不应成为兵粮目标：{:?}",
        pending.options
    );
    let _ = far;
    st.check_conservation().unwrap();
}
