//! P4.2：八卦阵 / 洛神 迁移到通用 `judge_before`（可被鬼才改判）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_state_inject, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn free_card(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let in_hand = |id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !in_hand(id))
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌")
}

#[test]
fn bagua_judge_can_be_rewritten() {
    let mgr = manager(2000);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut simayi = 0;
    let mut black = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        simayi = st.seats_from_left_of(cur)[1];
        set_general(st, simayi, "sgs_hero_16"); // 鬼才
        let bagua = super::find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(bagua));
        give(st, cur, "sha");
        // 判定牌：红桃（八卦生效）→ 鬼才改黑
        let red = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &red);
        st.deck.draw.push(red);
        black = free_card(st, |c| matches!(c.suit, Suit::Spade | Suit::Club));
        take_card(st, &black);
        st.seats[simayi].hand.push(black.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let sima_pid = pid(&st, simayi);
    let hp = st.seats[target].hp;
    let sha = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.as_ref().is_some_and(|p| p.kind == "respond"));
    mgr.execute_action(&room, &target_pid, "skill:bagua")
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("八卦判定应进入改判窗口");
    assert_eq!(pending.kind, "judge_rewrite");
    assert_eq!(pending.actor, sima_pid);
    mgr.execute_action(&room, &sima_pid, &format!("respond:{black}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp - 1,
        "改为黑色后八卦未生效，目标受伤"
    );
    assert!(st.history.iter().any(|e| e.text.contains("鬼才")));
    st.check_conservation().unwrap();
}

#[test]
fn luoshen_judge_can_be_rewritten_to_red() {
    let mgr = manager(2001);
    let room = start_and_pick(&mgr, 5);
    let mut zhenji = 0;
    let mut simayi = 0;
    let mut red = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhenji = st.turn;
        set_general(st, zhenji, "sgs_hero_21"); // 甄姬：洛神
        simayi = st.seats_from_left_of(zhenji)[0];
        set_general(st, simayi, "sgs_hero_16"); // 鬼才
                                                // 判定牌：黑桃（洛神本应获得）→ 鬼才改红
        let black = free_card(st, |c| c.suit == Suit::Spade);
        take_card(st, &black);
        st.deck.draw.push(black);
        red = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &red);
        st.seats[simayi].hand.push(red.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let zj = st.seats[zhenji].player_id.clone();
        let sima = st.seats[simayi].player_id.clone();
        let mut inject = vec![
            format!("{zj}|choice=choose:luoshen"),
            format!("{sima}|judge_rewrite=respond:{red}"),
        ];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("鬼才")));
    assert!(
        st.history.iter().any(|e| e.text.contains("洛神结束")),
        "改为红色后洛神结束：{:?}",
        st.history
            .iter()
            .rev()
            .take(6)
            .map(|e| &e.text)
            .collect::<Vec<_>>()
    );
    assert!(
        !st.seats[zhenji].hand.contains(&red),
        "红色判定牌不应被洛神获得"
    );
    st.check_conservation().unwrap();
}
