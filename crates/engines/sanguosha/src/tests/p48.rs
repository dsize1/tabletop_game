//! P4.1 收尾：国色（大乔）—— 方块手牌当【乐不思蜀】（虚拟定义 `virtual_defs`）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive as drive_state, edit, give, manager, state_of, take_card};
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
fn guose_diamond_as_lebu() {
    let mgr = manager(1400);
    let room = start_and_pick(&mgr, 5);
    let mut dq = 0;
    let mut target = 0;
    let mut diamond = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dq = st.turn;
        set_general(st, dq, "sgs_hero_13"); // 大乔：国色
        target = st.seats_from_left_of(dq)[0];
        set_general(st, target, "sgs_hero_2");
        diamond = free_card(st, |c| c.suit == Suit::Diamond);
        take_card(st, &diamond);
        st.seats[dq].hand.push(diamond.clone());
    });
    let st = state_of(&mgr, &room);
    let dq_pid = pid(&st, dq);
    let target_pid = pid(&st, target);
    let acts = mgr.legal_actions(&room, &dq_pid).unwrap();
    assert!(
        acts.iter()
            .any(|a| a.id == format!("skill:guose:{diamond}")),
        "国色应提供方块当乐不思蜀选项：{acts:?}"
    );
    mgr.execute_action(&room, &dq_pid, &format!("skill:guose:{diamond}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("target"));
    mgr.execute_action(&room, &dq_pid, &format!("target:{target_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].judge_zone.iter().any(|c| c == &diamond),
        "国色牌应置入目标判定区"
    );
    assert_eq!(
        st.effective_def_code(&diamond),
        Some("lebu_sishu"),
        "虚拟定义应为乐不思蜀"
    );
    assert!(
        crate::turn::judge_zone_has(&st, target, "lebu_sishu"),
        "判定区同名检查应识别虚拟定义"
    );

    // 模拟目标回合：判定牌非红桃 → 跳过出牌阶段。
    edit(&mgr, &room, |st| {
        st.turn = target;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let club = free_card(st, |c| c.suit == Suit::Club);
        take_card(st, &club);
        st.deck.draw.push(club);
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("非红桃，跳过出牌阶段")),
        "国色判定应生效：{:?}",
        st.history
            .iter()
            .rev()
            .take(8)
            .map(|e| &e.text)
            .collect::<Vec<_>>()
    );
    st.check_conservation().unwrap();
}

#[test]
fn guose_rejects_non_diamond() {
    let mgr = manager(1401);
    let room = start_and_pick(&mgr, 5);
    let mut dq = 0;
    let mut black = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dq = st.turn;
        set_general(st, dq, "sgs_hero_13");
        black = give(st, dq, "wuxie_keji"); // ♠11
    });
    let st = state_of(&mgr, &room);
    let dq_pid = pid(&st, dq);
    let acts = mgr.legal_actions(&room, &dq_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("skill:guose:{black}")),
        "黑色牌不应有国色选项"
    );
    assert!(
        mgr.execute_action(&room, &dq_pid, &format!("skill:guose:{black}"))
            .is_err(),
        "直接提交黑色牌的国色应被拒绝"
    );
}

#[test]
fn guose_excludes_target_with_existing_lebu() {
    let mgr = manager(1402);
    let room = start_and_pick(&mgr, 5);
    let mut dq = 0;
    let mut target = 0;
    let mut diamond = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dq = st.turn;
        set_general(st, dq, "sgs_hero_13");
        target = st.seats_from_left_of(dq)[0];
        set_general(st, target, "sgs_hero_2");
        // 目标判定区已有（物理）乐不思蜀
        let lebu = super::find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[target].judge_zone.push(lebu);
        diamond = free_card(st, |c| c.suit == Suit::Diamond);
        take_card(st, &diamond);
        st.seats[dq].hand.push(diamond.clone());
    });
    let st = state_of(&mgr, &room);
    let dq_pid = pid(&st, dq);
    let target_pid = pid(&st, target);
    mgr.execute_action(&room, &dq_pid, &format!("skill:guose:{diamond}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("国色应询问目标");
    assert!(
        !pending
            .options
            .iter()
            .any(|o| o.id == format!("target:{target_pid}")),
        "已有乐不思蜀的目标不应可选：{:?}",
        pending.options
    );
    st.check_conservation().unwrap();
}
