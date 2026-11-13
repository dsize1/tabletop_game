//! P4.4.2 UI 增强：公共视图下发「手牌上限」（技能修正后可显示 n/limit）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, public_view};

#[test]
fn view_hand_limit_uses_skill_modified_limit() {
    let mgr = manager(1700);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_308"); // 界周瑜：英姿手牌上限 = 体力上限
        st.seats[seat].max_hp = 3;
        st.seats[seat].hp = 1;
    });
    let public = public_view(&mgr, &room);
    let s = &public["public_board"]["seats"][seat];
    assert_eq!(s["hand_count"], 0);
    assert_eq!(s["hand_limit"], 3, "界英姿：手牌上限显示为体力上限而非体力");
}

#[test]
fn view_hand_limit_null_when_unlimited() {
    let mgr = manager(1701);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_10"); // 吕蒙：克己未出杀 → 跳过弃牌
        st.set_counter("sha_used", 0);
    });
    let public = public_view(&mgr, &room);
    let s = &public["public_board"]["seats"][seat];
    assert!(
        s["hand_limit"].is_null(),
        "克己（无上限）时 hand_limit 为 null"
    );
}
