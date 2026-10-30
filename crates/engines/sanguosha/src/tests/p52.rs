//! P4.2：通用技能判定窗口 `judge_before`（铁骑 / 刚烈 支持鬼才改判）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, give, manager, state_of, take_card};
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

fn free_card(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let in_hand = |id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !in_hand(id))
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌")
}

#[test]
fn tieqi_judge_can_be_rewritten() {
    let mgr = manager(1800);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut simayi = 0;
    let mut black = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_6"); // 马超：铁骑
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        simayi = st.seats_from_left_of(cur)[1];
        set_general(st, simayi, "sgs_hero_16"); // 司马懿：鬼才
        give(st, cur, "sha");
        give(st, target, "shan");
        // 判定牌：红桃（铁骑生效）→ 被鬼才改为黑色
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
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("铁骑判定应进入改判窗口");
    assert_eq!(pending.kind, "judge_rewrite");
    assert_eq!(pending.actor, sima_pid);
    mgr.execute_action(&room, &sima_pid, &format!("respond:{black}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond"),
        "改为黑色后目标可以出【闪】"
    );
    let shan = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "改判黑色后铁骑未生效，闪抵消");
    assert!(st.history.iter().any(|e| e.text.contains("鬼才")));
    assert!(st.history.iter().any(|e| e.text.contains("铁骑")));
    st.check_conservation().unwrap();
}

#[test]
fn ganglie_judge_can_be_rewritten_to_heart() {
    let mgr = manager(1801);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut victim = 0;
    let mut simayi = 0;
    let mut heart = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_17"); // 夏侯惇：刚烈
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        simayi = st.seats_from_left_of(cur)[1];
        set_general(st, simayi, "sgs_hero_16"); // 司马懿：鬼才
        give(st, cur, "sha");
        give(st, cur, "shan");
        // 判定牌：梅花（刚烈生效）→ 被鬼才改为红桃
        let club = free_card(st, |c| c.suit == Suit::Club);
        take_card(st, &club);
        st.deck.draw.push(club);
        heart = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &heart);
        st.seats[simayi].hand.push(heart.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let victim_pid = pid(&st, victim);
    let sima_pid = pid(&st, simayi);
    let cur_hp = st.seats[cur].hp;
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("刚烈判定应进入改判窗口");
    assert_eq!(pending.kind, "judge_rewrite");
    assert_eq!(pending.actor, sima_pid);
    mgr.execute_action(&room, &sima_pid, &format!("respond:{heart}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "红桃判定后不触发刚烈二选一");
    assert_eq!(st.seats[cur].hp, cur_hp, "伤害来源未受刚烈伤害");
    assert!(st.history.iter().any(|e| e.text.contains("鬼才")));
    st.check_conservation().unwrap();
}
