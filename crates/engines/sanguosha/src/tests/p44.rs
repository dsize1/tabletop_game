//! P4.1 批次 C2-1：苦肉、青囊、结姻、反馈、遗计。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of};
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

/// 让 `attacker` 对 `victim` 出一张【杀】，并通过所有响应直到产生下一个 pending。
fn sha_hit(mgr: &game_engine_core::GameManager, room: &str) -> (usize, usize) {
    let st = state_of(mgr, room);
    let attacker = st.turn;
    let victim = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != attacker && st.in_attack_range(attacker, *i))
        .expect("攻击范围内目标");
    let attacker_pid = pid(&st, attacker);
    let victim_pid = pid(&st, victim);
    let sha = in_hand(&st, attacker, "sha");
    mgr.execute_action(room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(room, &attacker_pid, &format!("target:{victim_pid}"))
        .unwrap();
    // 目标无手牌 → 自动放弃响应；若仍有 pending 则交给调用方处理。
    (attacker, victim)
}

#[test]
fn kurou_loses_hp_and_draws_two() {
    let mgr = manager(1000);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_11"); // 黄盖：苦肉
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp = st.seats[cur].hp;
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(acts.iter().any(|a| a.id == "skill:kurou"), "苦肉应可发动");
    mgr.execute_action(&room, &cur_pid, "skill:kurou").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp - 1, "苦肉失去 1 点体力");
    assert_eq!(st.seats[cur].hand.len(), 2, "苦肉摸 2 张");
    // 无次数限制：可再次发动
    mgr.execute_action(&room, &cur_pid, "skill:kurou").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp - 2);
    assert_eq!(st.seats[cur].hand.len(), 4);
    st.check_conservation().unwrap();
}

#[test]
fn kurou_can_enter_dying_at_one_hp() {
    let mgr = manager(1001);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_11");
        st.seats[cur].hp = 1;
        // 给一名其他玩家【桃】，让濒死 pending 停在求桃窗口。
        let other = st.seats_from_left_of(cur)[0];
        give(st, other, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, "skill:kurou").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 0);
    let pending = st.pending.expect("1 体力苦肉应进入濒死");
    assert_eq!(pending.kind, "dying");
    assert_eq!(
        pending.data["victim"].as_str(),
        Some(cur_pid.as_str()),
        "濒死者应为黄盖"
    );
}

#[test]
fn qingnang_heals_wounded_target() {
    let mgr = manager(1002);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut wounded = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_22"); // 华佗：青囊
        wounded = st.seats_from_left_of(cur)[0];
        set_general(st, wounded, "sgs_hero_2");
        st.seats[cur].hp = st.seats[cur].max_hp - 1;
        st.seats[wounded].hp = st.seats[wounded].max_hp - 1;
        give(st, cur, "shan");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let wounded_pid = pid(&st, wounded);
    let wounded_hp = st.seats[wounded].hp;
    let card = st.seats[cur].hand[0].clone();

    mgr.execute_action(&room, &cur_pid, "skill:qingnang")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("青囊应询问回复目标");
    assert_eq!(pending.kind, "target");
    assert_eq!(pending.skill.as_deref(), Some("qingnang"));
    mgr.execute_action(&room, &cur_pid, &format!("target:{wounded_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[wounded].hp, wounded_hp + 1, "目标回复 1 点体力");
    assert_eq!(st.counter("qingnang_used"), 1);
    assert!(st.deck.discard.iter().any(|c| c == &card), "弃置牌进弃牌堆");
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == "skill:qingnang"),
        "青囊每阶段限一次"
    );
    st.check_conservation().unwrap();
}

#[test]
fn jieyin_heals_both() {
    let mgr = manager(1003);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_25"); // 孙尚香：结姻
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_3"); // 张飞（男）
        st.seats[cur].hp = st.seats[cur].max_hp - 1;
        st.seats[target].hp = st.seats[target].max_hp - 1;
        give(st, cur, "shan");
        give(st, cur, "sha");
        give(st, cur, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = st.seats[target].hp;
    let self_hp = st.seats[cur].hp;
    let cards: Vec<String> = st.seats[cur].hand.iter().take(2).cloned().collect();

    mgr.execute_action(&room, &cur_pid, "skill:jieyin").unwrap();
    for c in &cards[..2] {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{c}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp + 1, "结姻对象回复 1");
    assert_eq!(st.seats[cur].hp, self_hp + 1, "自己也回复 1");
    assert_eq!(st.counter("jieyin_used"), 1);
    assert_eq!(st.seats[cur].hand.len(), 1, "弃置 2 张（原有 3）");
    st.check_conservation().unwrap();
}

#[test]
fn fankui_gains_source_card() {
    let mgr = manager(1004);
    let room = start_and_pick(&mgr, 5);
    let mut victim = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_16"); // 司马懿：反馈
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        give(st, cur, "sha");
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let victim_pid = pid(&st, victim);
    let source_hand = st.seats[st.turn].hand.len();
    let (_, _) = sha_hit(&mgr, &room);
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("反馈应询问获得哪张牌");
    assert_eq!(pending.kind, "pick_card");
    assert_eq!(pending.actor, victim_pid);
    mgr.execute_action(&room, &victim_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hand.len(), 1, "反馈获得来源一张手牌");
    assert_eq!(
        st.seats[st.turn].hand.len(),
        source_hand - 2,
        "来源：出杀 -1、被反馈 -1"
    );
    assert!(st.history.iter().any(|e| e.text.contains("反馈")));
    st.check_conservation().unwrap();
}

#[test]
fn yiji_draws_two_and_distributes() {
    let mgr = manager(1005);
    let room = start_and_pick(&mgr, 5);
    let mut victim = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_20"); // 郭嘉：遗计
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let victim_pid = pid(&st, victim);
    let (_, _) = sha_hit(&mgr, &room);
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("遗计应询问牌的去向");
    assert_eq!(pending.kind, "target");
    assert_eq!(pending.skill.as_deref(), Some("yiji"));
    // 两张都留给自己
    for _ in 0..2 {
        let st = state_of(&mgr, &room);
        let pending = st.pending.clone().expect("遗计继续分配");
        assert_eq!(pending.skill.as_deref(), Some("yiji"));
        mgr.execute_action(&room, &victim_pid, &format!("target:{victim_pid}"))
            .unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "分配完毕");
    assert_eq!(st.seats[victim].hand.len(), 2, "遗计摸 2 张");
    assert!(st.history.iter().any(|e| e.text.contains("遗计")));
    st.check_conservation().unwrap();
}
