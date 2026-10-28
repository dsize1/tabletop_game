//! P2.4 测试：技能内核（奸雄 / 制衡 / 闭月 / 集智 / 奇才 / 克己 / 救援）。

use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, lobby_with, manager, pick_all,
    state_of,
};
use crate::data;
use crate::state::*;
use game_engine_core::GameManager;

pub(super) fn start_and_pick(mgr: &GameManager, players: usize) -> String {
    let room = lobby_with(mgr, players);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

pub(super) fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

pub(super) fn in_hand(st: &SgsState, idx: usize, code: &str) -> String {
    st.seats[idx]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).map(|c| c.def.as_str()) == Some(code))
        .cloned()
        .unwrap_or_else(|| panic!("{} 手里没有 {code}", st.seats[idx].player_id))
}

pub(super) fn others(st: &SgsState, cur: usize) -> Vec<usize> {
    st.seats_from_left_of(cur)
        .into_iter()
        .filter(|i| st.seats[*i].alive)
        .collect()
}

/// 把某座位改成指定武将（体力按主公 +1）。
pub(super) fn set_general(st: &mut SgsState, seat: usize, code: &str) {
    let def = data::general(code).expect("known general");
    let bonus = i32::from(st.seats[seat].identity.is_lord());
    st.seats[seat].general = Some(code.to_string());
    st.seats[seat].general_name = def.name.to_string();
    st.seats[seat].max_hp = (def.max_hp + bonus).min(MAX_HP_LIMIT);
    st.seats[seat].hp = st.seats[seat].max_hp;
}

#[test]
fn jianxiong_gains_damage_card() {
    let mgr = manager(60);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_15");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let victim = others(&st, cur)[0];
    let victim_pid = pid(&st, victim);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();

    // 受伤者无手牌 → 【闪】询问自动跳过，直接进入奸雄选择。
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("奸雄应询问");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.actor, victim_pid);
    let gain = pending
        .options
        .iter()
        .find(|o| o.id == "choose:gain")
        .expect("应有获得选项")
        .id
        .clone();
    mgr.execute_action(&room, &victim_pid, &gain).unwrap();

    let st = state_of(&mgr, &room);
    assert!(st.seats[victim].hand.contains(&sha), "奸雄应获得杀");
    assert!(!st.deck.processing.contains(&sha));
    st.check_conservation().unwrap();
}

#[test]
fn zhiheng_discards_and_draws_equal() {
    let mgr = manager(61);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_8");
        give(st, cur, "shan");
        give(st, cur, "tao");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let hand: Vec<String> = st.seats[cur].hand.clone();
    let draw_before = st.deck.draw.len();

    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(acts.iter().any(|a| a.id == "skill:zhiheng"));
    mgr.execute_action(&room, &cur_pid, "skill:zhiheng")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{}", hand[0]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{}", hand[1]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "弃 2 摸 2：原 3 张不变");
    assert_eq!(st.deck.draw.len(), draw_before - 2);
    assert!(st.deck.discard.contains(&hand[0]));
    assert!(st.deck.discard.contains(&hand[1]));
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id == "skill:zhiheng"));
    st.check_conservation().unwrap();
}

#[test]
fn biyue_draws_one_at_end_phase() {
    let mgr = manager(62);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_24");
        give(st, cur, "shan");
        st.turn_phase = TurnPhase::End;
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    let diaochan = st
        .seats
        .iter()
        .position(|s| s.general.as_deref() == Some("sgs_hero_24"))
        .unwrap();
    assert_eq!(st.seats[diaochan].hand.len(), 2, "1 张原有 + 闭月 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("闭月")));
}

#[test]
fn jizhi_draws_on_trick_use() {
    let mgr = manager(63);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_7");
        give(st, cur, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let card = in_hand(&st, cur, "wuzhong_shengyou");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    // 打出无中（-1）→ 集智 +1 → 无中 +2 = 3
    assert_eq!(st.seats[cur].hand.len(), 3);
    assert!(st.history.iter().any(|e| e.text.contains("集智")));
}

#[test]
fn qicai_ignores_trick_distance() {
    let mgr = manager(64);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_7");
        give(st, cur, "shunshou_qianyang");
        let far = st.seats_from_left_of(cur)[2];
        give(st, far, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let far = others(&st, cur)[2];
    let far_pid = pid(&st, far);
    let card = in_hand(&st, cur, "shunshou_qianyang");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let pending = state_of(&mgr, &room).pending.expect("应能选择远处目标");
    assert!(pending
        .options
        .iter()
        .any(|o| o.id == format!("target:{far_pid}")));
    mgr.execute_action(&room, &cur_pid, &format!("target:{far_pid}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|pick_card=pick:hand")];
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("顺手牵羊")));
}

#[test]
fn keji_skips_discard_phase() {
    let mgr = manager(65);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_10");
        st.seats[cur].hp = 1;
        for _ in 0..5 {
            give(st, cur, "shan");
        }
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    assert_eq!(st.seats[cur].hand.len(), 5);
    assert!(st.seats[cur].hand.len() > st.hand_limit(cur));

    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "克己应跳过弃牌 pending");
    assert_eq!(st.seats[cur].hand.len(), 5);
    assert_eq!(st.turn_index, 1);
}

#[test]
fn jiuyuan_adds_one_heal_from_wu_ally() {
    let mgr = manager(66);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        // 主公（孙权，濒死者）+ 吴势力持桃者
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_8");
        st.seats[lord].hp = 1;
        let healer = st.seats_from_left_of(cur)[1];
        set_general(st, healer, "sgs_hero_9");
        give(st, healer, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let victim = others(&st, cur)[0];
    assert!(st.seats[victim].identity.is_lord());
    let victim_pid = pid(&st, victim);
    let card = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();

    let mut healed = false;
    for _ in 0..20 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "dying" {
            break;
        }
        if let Some(tao) = p.options.iter().find(|o| o.id.starts_with("respond:")) {
            mgr.execute_action(&room, &p.actor, &tao.id).unwrap();
            healed = true;
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    assert!(healed);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 2, "桃 1 + 救援 1");
}
