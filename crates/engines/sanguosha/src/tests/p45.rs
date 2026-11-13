//! P4.1 批次 C2-2：无双、铁骑、刚烈、洛神、反间。

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

fn drive(mgr: &game_engine_core::GameManager, room: &str, inject: &mut Vec<String>) {
    super::drive_mgr_inject(mgr, room, inject);
}

/// 取一张满足条件、且不在任何玩家手牌里的牌。
fn free_card(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let in_hand = |id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !in_hand(id))
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌")
}

/// 连续 pass 无懈窗口，直到出现指定 kind 的 pending（返回状态）。
fn pass_wuxie(mgr: &game_engine_core::GameManager, room: &str) {
    for _ in 0..20 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else { return };
        if p.kind != "wuxie" {
            return;
        }
        mgr.execute_action(room, &p.actor, "pass").unwrap();
    }
    panic!("无懈窗口未收敛");
}

#[test]
fn wushuang_sha_needs_two_shan() {
    let mgr = manager(1100);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_23"); // 吕布：无双
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        sha = give(st, cur, "sha");
        give(st, target, "shan");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    // 第一张闪：仍需一张
    let st = state_of(&mgr, &room);
    let shan1 = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan1}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "还没结算伤害");
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond"),
        "无双：还需第二张【闪】"
    );
    let shan2 = in_hand(&st, target, "shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan2}"))
        .unwrap();
    drive(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "两张闪抵消【杀】");
    st.check_conservation().unwrap();
}

#[test]
fn wushuang_duel_needs_two_sha() {
    let mgr = manager(1101);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut juedou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_23"); // 吕布
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        juedou = give(st, cur, "juedou");
        give(st, target, "sha");
        give(st, target, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let cur_hp = st.seats[cur].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{juedou}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    pass_wuxie(&mgr, &room);

    let st = state_of(&mgr, &room);
    let sha1 = in_hand(&st, target, "sha");
    mgr.execute_action(&room, &target_pid, &format!("respond:{sha1}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(target_pid.as_str()),
        "无双：目标还需第二张【杀】"
    );
    let sha2 = in_hand(&st, target, "sha");
    mgr.execute_action(&room, &target_pid, &format!("respond:{sha2}"))
        .unwrap();
    // 两张杀打完后轮到吕布；吕布无手牌 → 自动放弃并受到 1 点伤害。
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hp,
        cur_hp - 1,
        "无双决斗：目标两张杀后吕布无杀，吕布受伤"
    );
    st.check_conservation().unwrap();
}

#[test]
fn tieqi_red_judge_blocks_shan() {
    let mgr = manager(1102);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    let mut sha = String::new();
    let mut shan = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_6"); // 马超：铁骑
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        sha = give(st, cur, "sha");
        shan = give(st, target, "shan");
        // 判定牌：红桃（红色 → 封锁【闪】）
        let red = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &red);
        st.deck.draw.push(red);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("铁骑")),
        "应有铁骑判定历史"
    );
    assert_eq!(st.seats[target].hp, hp - 1, "铁骑红色：不能出闪，直接受伤");
    assert!(
        st.seats[target].hand.contains(&shan),
        "【闪】仍在手牌中（未被迫打出）"
    );
    st.check_conservation().unwrap();
}

#[test]
fn ganglie_forces_source_choice() {
    let mgr = manager(1103);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut victim = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_17"); // 夏侯惇：刚烈
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        sha = give(st, cur, "sha");
        // 伤害来源只有 1 张手牌 → 无法弃 2 张，只剩「受到 1 点伤害」。
        give(st, cur, "shan");
        // 判定牌：梅花（非红桃 → 刚烈生效）
        let black = free_card(st, |c| c.suit == Suit::Club);
        take_card(st, &black);
        st.deck.draw.push(black);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let victim_pid = pid(&st, victim);
    let cur_hp = st.seats[cur].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("刚烈应询问伤害来源");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("ganglie"));
    assert_eq!(pending.actor, cur_pid);
    assert!(
        pending.options.iter().all(|o| o.id != "choose:discard"),
        "手牌不足 2 张时不应给出弃牌选项"
    );
    mgr.execute_action(&room, &cur_pid, "choose:damage")
        .unwrap();
    drive(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, cur_hp - 1, "来源选择受到 1 点伤害");
    st.check_conservation().unwrap();
}

#[test]
fn luoshen_repeats_on_black() {
    let mgr = manager(1104);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut black = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_21"); // 甄姬：洛神
        let red = free_card(st, |c| c.suit == Suit::Heart);
        black = free_card(st, |c| c.suit == Suit::Spade);
        // 先 push 红、再 push 黑 → 第一次判定黑，第二次判定红
        for id in [red, black.clone()] {
            take_card(st, &id);
            st.deck.draw.push(id);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}|choice=choose:luoshen"),
            format!("{cur_pid}|choice=choose:luoshen"),
        ];
        super::drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("洛神")),
        "应有洛神历史"
    );
    assert!(
        st.seats[cur].hand.contains(&black),
        "黑色判定牌应被洛神获得"
    );
    assert_eq!(st.turn_phase, TurnPhase::Play, "洛神后继续回合流程");
    st.check_conservation().unwrap();
}

#[test]
fn fanjian_target_suit_then_damage() {
    let mgr = manager(1105);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_12"); // 周瑜：反间
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        // 周瑜只有一张红桃手牌 → 目标选黑桃必然花色不同
        give(st, cur, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, "skill:fanjian")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("目标应选择花色");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.actor, target_pid);
    mgr.execute_action(&room, &target_pid, "choose:suit:spade")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "花色不同 → 受到 1 点伤害");
    assert!(st.seats[target].hand.contains(&card), "目标获得反间牌");
    assert_eq!(st.counter("fanjian_used"), 1);
    st.check_conservation().unwrap();
}
