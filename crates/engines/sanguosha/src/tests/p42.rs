//! P4.1 批次 B：转化/防御技能（武圣、龙胆、倾国、急救）。
//!
//! 口径：`skills.md` §2/§4；转化牌在出牌阶段主动使用（`skill:<code>:<card>`）与响应
//! （`respond:<card>`）两条路径都必须重校验，且引擎未下发的转化选项不可伪造。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, give, manager, state_of};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

/// 连续 pass 无懈等窗口，停在指定 kind 的 pending（不自动处理该 pending）。
fn pass_until(mgr: &game_engine_core::GameManager, room: &str, kind: &str) -> SgsState {
    for _ in 0..30 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else {
            break;
        };
        if p.kind == kind {
            return st;
        }
        mgr.execute_action(room, &p.actor, "pass").unwrap();
    }
    panic!("没有等到 {kind} pending");
}

fn hp(st: &SgsState, idx: usize) -> i32 {
    st.seats[idx].hp
}

#[test]
fn wusheng_red_card_as_sha_active() {
    let mgr = manager(800);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut card = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2"); // 关羽：武圣
        target = st.seats_from_left_of(cur)[0];
        card = give(st, cur, "wuzhong_shengyou"); // 红桃 7/8/9/11
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.cards[&card].suit,
        Suit::Heart,
        "测试前提：无中生有为红桃"
    );
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("skill:wusheng:{card}")),
        "武圣应提供红牌当【杀】的主动使用选项：{acts:?}"
    );
    let target_hp = hp(&st, target);

    mgr.execute_action(&room, &cur_pid, &format!("skill:wusheng:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        target_hp - 1,
        "转化【杀】应造成 1 点伤害"
    );
    assert!(
        st.deck.discard.iter().any(|c| c == &card),
        "转化牌应进弃牌堆"
    );
    assert_eq!(st.counter("sha_used"), 1, "转化【杀】计入出杀次数");
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("武圣") && e.text.contains("当【杀】使用")),
        "应有武圣历史：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    st.check_conservation().unwrap();
}

#[test]
fn wusheng_rejects_black_card() {
    let mgr = manager(801);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut card = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_2");
        card = give(st, cur, "wuxie_keji"); // ♠11：黑色非【杀】
    });
    let st = state_of(&mgr, &room);
    assert!(
        matches!(st.cards[&card].suit, Suit::Spade | Suit::Club),
        "测试前提：无懈可击为黑色"
    );
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("skill:wusheng:{card}")),
        "黑色牌不应有武圣选项"
    );
    assert!(
        mgr.execute_action(&room, &cur_pid, &format!("skill:wusheng:{card}"))
            .is_err(),
        "直接提交黑色牌的武圣转化应被拒绝"
    );
}

#[test]
fn longdan_shan_as_sha_active() {
    let mgr = manager(802);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut card = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_5"); // 赵云：龙胆
        target = st.seats_from_left_of(cur)[0];
        card = give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = hp(&st, target);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("skill:longdan:{card}")),
        "龙胆应提供【闪】当【杀】的选项：{acts:?}"
    );
    mgr.execute_action(&room, &cur_pid, &format!("skill:longdan:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp - 1);
    st.check_conservation().unwrap();
}

#[test]
fn longdan_sha_as_shan_response() {
    let mgr = manager(803);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut sha = String::new();
    let mut target_sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_5"); // 赵云：龙胆
        sha = give(st, cur, "sha");
        target_sha = give(st, target, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = hp(&st, target);
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &target_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("respond:{target_sha}")),
        "龙胆应允许【杀】当【闪】响应：{acts:?}"
    );
    mgr.execute_action(&room, &target_pid, &format!("respond:{target_sha}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp, "龙胆出闪应抵消【杀】");
    assert!(st.history.iter().any(|e| e.text.contains("龙胆")));
    st.check_conservation().unwrap();
}

#[test]
fn qingguo_black_card_as_shan_response() {
    let mgr = manager(804);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut sha = String::new();
    let mut black = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_21"); // 甄姬：倾国
        sha = give(st, cur, "sha");
        black = give(st, target, "wuxie_keji");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = hp(&st, target);
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &target_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("respond:{black}")),
        "倾国应允许黑色牌当【闪】响应：{acts:?}"
    );
    mgr.execute_action(&room, &target_pid, &format!("respond:{black}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp, "倾国出闪应抵消【杀】");
    assert!(st.history.iter().any(|e| e.text.contains("倾国")));
    st.check_conservation().unwrap();
}

#[test]
fn wusheng_red_card_as_sha_response_to_nanman() {
    let mgr = manager(805);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut nanman = String::new();
    let mut red = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2"); // 关羽：武圣
        nanman = give(st, cur, "nanman_ruqin");
        red = give(st, target, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = hp(&st, target);
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    let st = pass_until(&mgr, &room, "respond");
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.clone()),
        Some(target_pid.clone())
    );
    let acts = mgr.legal_actions(&room, &target_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("respond:{red}")),
        "武圣应允许红牌当【杀】响应南蛮：{acts:?}"
    );
    mgr.execute_action(&room, &target_pid, &format!("respond:{red}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp, "武圣出杀应免伤");
    st.check_conservation().unwrap();
}

#[test]
fn jijiu_red_card_as_tao_outside_turn() {
    let mgr = manager(806);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut victim = 0;
    let mut hua = 0;
    let mut sha = String::new();
    let mut red = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        hua = st.seats_from_left_of(cur)[1];
        set_general(st, hua, "sgs_hero_22"); // 华佗：急救
        st.seats[victim].hp = 1;
        sha = give(st, cur, "sha");
        red = give(st, hua, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let victim_pid = pid(&st, victim);
    let hua_pid = pid(&st, hua);

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();

    // 不驱动 pending：直接检查濒死求桃窗口（驱动会自动 pass 掉）。
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应进入濒死求桃");
    assert_eq!(pending.kind, "dying");
    assert_eq!(pending.actor, hua_pid, "空手玩家跳过，应由华佗救援");
    assert!(
        pending
            .options
            .iter()
            .any(|o| o.id == format!("respond:{red}")),
        "急救应提供红牌当【桃】选项：{:?}",
        pending.options
    );
    mgr.execute_action(&room, &hua_pid, &format!("respond:{red}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[victim].alive);
    assert_eq!(st.seats[victim].hp, 1, "急救回复 1 点体力");
    assert!(st.history.iter().any(|e| e.text.contains("急救")));
    st.check_conservation().unwrap();
}

#[test]
fn jijiu_only_outside_own_turn() {
    let mgr = manager(807);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut red = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_22");
        red = give(st, cur, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::skills::conversion_for(&st, cur, &red, "tao").is_none(),
        "自己的回合内不能发动急救"
    );
    // 切到别人的回合（仅改状态做规则校验）。
    let mut st2 = st.clone();
    st2.turn = st.seats_from_left_of(cur)[0];
    assert!(
        crate::skills::conversion_for(&st2, cur, &red, "tao").is_some(),
        "回合外应可发动急救"
    );
}
