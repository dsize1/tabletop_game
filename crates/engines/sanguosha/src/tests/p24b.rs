//! P2.4 测试（第二批）：仁德 / 离间 / 突袭 / 奇袭 / 激将 / 护驾。

use super::p24::{in_hand, others, pid, set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, state_of, take_card,
};
use crate::state::*;
use game_engine_core::GameManager;

#[test]
fn rende_gives_cards_and_heals_after_two() {
    let mgr = manager(70);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.seats[cur].hp -= 1;
        give(st, cur, "shan");
        give(st, cur, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let cards: Vec<String> = st.seats[cur].hand.clone();
    let hp_before = st.seats[cur].hp;

    mgr.execute_action(&room, &cur_pid, "skill:rende").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{}", cards[0]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{}", cards[1]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].hand.is_empty());
    assert!(st.seats[target].hand.contains(&cards[0]));
    assert!(st.seats[target].hand.contains(&cards[1]));
    assert_eq!(st.seats[cur].hp, hp_before + 1, "仁德给出 ≥2 张回复 1 点");
    st.check_conservation().unwrap();
}

#[test]
fn lijian_makes_two_males_duel() {
    let mgr = manager(71);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_24");
        give(st, cur, "shan");
        // 保证另外两人是男性（默认多为男性；显式设置）
        let a = st.seats_from_left_of(cur)[0];
        let b = st.seats_from_left_of(cur)[1];
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_3");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let a = others(&st, cur)[0];
    let b = others(&st, cur)[1];
    let a_pid = pid(&st, a);
    let b_pid = pid(&st, b);
    let card = st.seats[cur].hand[0].clone();
    let hp_b = st.seats[b].hp;

    mgr.execute_action(&room, &cur_pid, "skill:lijian").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();

    // B 被 A 决斗，B 不出杀 → B 受伤
    let mut inject = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[b].hp, hp_b - 1, "B 不出杀应受 1 点伤害");
    assert!(st.deck.discard.contains(&card));
    assert_eq!(st.seats[cur].hand.len(), 0);
}

#[test]
fn tuxi_replaces_draw_with_stealing() {
    let mgr = manager(72);
    let room = start_and_pick(&mgr, 5);
    let mut draw_before = 0usize;
    let mut cur_hand_after = 0usize;
    let mut a_hand_after = 0usize;
    let mut b_hand_after = 0usize;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_18");
        give(st, cur, "shan");
        let a = st.seats_from_left_of(cur)[0];
        let b = st.seats_from_left_of(cur)[1];
        give(st, a, "sha");
        give(st, b, "sha");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let cur_pid = st.seats[cur].player_id.clone();
        let a_pid = st.seats[a].player_id.clone();
        let b_pid = st.seats[b].player_id.clone();
        draw_before = st.deck.draw.len();
        let mut inject = vec![
            format!("{cur_pid}|choice=choose:tuxi"),
            format!("{cur_pid}|target=target:{a_pid}"),
            format!("{cur_pid}|target=target:{b_pid}"),
        ];
        drive_state_inject(st, &mut inject);
        cur_hand_after = st.seats[cur].hand.len();
        a_hand_after = st.seats[a].hand.len();
        b_hand_after = st.seats[b].hand.len();
    });
    assert_eq!(
        draw_before,
        state_of(&mgr, &room).deck.draw.len(),
        "突袭应跳过摸牌"
    );
    assert_eq!(cur_hand_after, 3, "原 1 张 + 突袭获得 2 张");
    assert_eq!(a_hand_after, 0);
    assert_eq!(b_hand_after, 0);
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert!(st.history.iter().any(|e| e.text.contains("突袭")));
    st.check_conservation().unwrap();
}

#[test]
fn qixi_treats_black_card_as_guohe() {
    let mgr = manager(73);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_9");
        // 给一张黑色牌与一张红色牌
        let black = st
            .cards
            .iter()
            .find(|(_, c)| matches!(c.suit, Suit::Spade | Suit::Club))
            .map(|(id, _)| id.clone())
            .unwrap();
        take_card(st, &black);
        st.seats[cur].hand.push(black);
        let b = st.seats_from_left_of(cur)[0];
        give(st, b, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let b = others(&st, cur)[0];
    let b_pid = pid(&st, b);
    let black = st.seats[cur].hand[0].clone();
    let hand_before = st.seats[b].hand.len();

    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("skill:qixi:{black}")),
        "黑色牌应可作为奇袭"
    );
    mgr.execute_action(&room, &cur_pid, &format!("skill:qixi:{black}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|pick_card=pick:hand")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[b].hand.len(), hand_before - 1);
    assert!(st.deck.discard.contains(&black), "转化的黑牌应进弃牌堆");
}

#[test]
fn jijiang_lets_shu_ally_play_sha() {
    let mgr = manager(74);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "nanman_ruqin");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_1");
        let ally = st.seats_from_left_of(cur)[1];
        set_general(st, ally, "sgs_hero_2"); // 关羽（蜀）
        give(st, ally, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = others(&st, cur)[0];
    let lord_pid = pid(&st, lord);
    let ally = others(&st, cur)[1];
    let ally_pid = pid(&st, ally);
    let sha = in_hand(&st, ally, "sha");
    let hp = st.seats[lord].hp;
    let card = in_hand(&st, cur, "nanman_ruqin");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![
        format!("{lord_pid}|respond=skill:jijiang"),
        format!("{ally_pid}|choice=respond:{sha}"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hp, hp, "激将成功，主公不应受伤");
    assert!(st.deck.discard.contains(&sha));
}

#[test]
fn hujia_lets_wei_ally_play_shan() {
    let mgr = manager(75);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wanjian_qifa");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_15"); // 曹操（魏）
        let ally = st.seats_from_left_of(cur)[1];
        set_general(st, ally, "sgs_hero_18"); // 张辽（魏）
        give(st, ally, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = others(&st, cur)[0];
    let lord_pid = pid(&st, lord);
    let ally = others(&st, cur)[1];
    let ally_pid = pid(&st, ally);
    let shan = in_hand(&st, ally, "shan");
    let hp = st.seats[lord].hp;
    let card = in_hand(&st, cur, "wanjian_qifa");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![
        format!("{lord_pid}|respond=skill:hujia"),
        format!("{ally_pid}|choice=respond:{shan}"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hp, hp, "护驾成功，主公不应受伤");
    assert!(st.deck.discard.contains(&shan));
}

#[test]
fn tuxi_decline_draws_normally() {
    let mgr = manager(76);
    let room = start_and_pick(&mgr, 5);
    let mut draw_before = 0usize;
    let mut hand_after = 0usize;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_18");
        give(st, cur, "shan");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        draw_before = st.deck.draw.len();
        let cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}|choice=pass")];
        drive_state_inject(st, &mut inject);
        hand_after = st.seats[cur].hand.len();
    });
    assert_eq!(hand_after, 3, "放弃突袭应正常摸 2 张");
    assert_eq!(draw_before - 2, state_of(&mgr, &room).deck.draw.len());
}

#[test]
fn jijiang_not_offered_without_shu_ally() {
    let mgr = manager(77);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "nanman_ruqin");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_1"); // 刘备（蜀），但其他人都不是蜀
        for i in st.seats_from_left_of(cur).into_iter().skip(1) {
            set_general(st, i, "sgs_hero_15");
        }
        let lord_pid = st.seats[lord].player_id.clone();
        let cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}|target=target:{lord_pid}")];
        drive_state_inject(st, &mut inject);
        // 走到主公响应时检查选项
        loop {
            let Some(p) = st.pending.clone() else { break };
            if p.actor == lord_pid && p.kind == "respond" {
                assert!(
                    !p.options.iter().any(|o| o.id == "skill:jijiang"),
                    "没有蜀势力存活角色时不应提供激将"
                );
                break;
            }
            let action = if p.kind == "wuxie" { "pass" } else { "pass" };
            crate::resolve::execute_pending(st, &p.actor, action).unwrap();
            crate::resolve::advance(st, &mut Vec::new()).unwrap();
        }
    });
}

#[test]
fn lijian_once_per_turn() {
    let mgr = manager(78);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_24");
        give(st, cur, "shan");
        give(st, cur, "tao");
        let a = st.seats_from_left_of(cur)[0];
        let b = st.seats_from_left_of(cur)[1];
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_3");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let a = others(&st, cur)[0];
    let b = others(&st, cur)[1];
    let (a_pid, b_pid) = (pid(&st, a), pid(&st, b));
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, "skill:lijian").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == "skill:lijian"),
        "离间每阶段只能发动一次"
    );
}
