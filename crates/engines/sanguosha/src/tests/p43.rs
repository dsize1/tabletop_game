//! P4.1 批次 C1：锁定/触发类技能（咆哮、马术、英姿、裸衣、天妒、谦逊、连营、枭姬）。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive as drive_state, drive_state_inject, edit, find_card, give, manager,
    state_of, take_card,
};
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

#[test]
fn paoxiao_allows_multiple_sha() {
    let mgr = manager(900);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_3"); // 张飞：咆哮
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "sha");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = st.seats[target].hp;

    for _ in 0..2 {
        let st = state_of(&mgr, &room);
        let sha = in_hand(&st, cur, "sha");
        let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
        assert!(
            acts.iter().any(|a| a.id == format!("play:{sha}")),
            "咆哮应允许继续出杀：{acts:?}"
        );
        mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
            .unwrap();
        mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
            .unwrap();
        super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp - 2, "两杀应各造成 1 点");
    assert_eq!(st.counter("sha_used"), 2);
    st.check_conservation().unwrap();
}

#[test]
fn mashu_extends_attack_range() {
    let mgr = manager(901);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = (cur + 2) % st.seats.len(); // 座位距离 2（5 人局）
        set_general(st, target, "sgs_hero_2");
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seat_distance(cur, target), 2);
    assert!(
        !crate::turn::sha_targets(&st, cur).contains(&target),
        "无马术时距离 2 超出攻击范围 1"
    );
    edit(&mgr, &room, |st| {
        set_general(st, cur, "sgs_hero_6"); // 马超：马术
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::sha_targets(&st, cur).contains(&target),
        "马术（距离 -1）后距离 2 应进入攻击范围"
    );
}

#[test]
fn yingzi_draws_three() {
    let mgr = manager(902);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_12"); // 周瑜：英姿
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(st.seats[cur].hand.len(), 3, "英姿：摸牌阶段摸 3 张");
}

#[test]
fn luoyi_draws_one_less_and_boosts_sha_damage() {
    let mgr = manager(903);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_19"); // 许褚：裸衣
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "sha");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        // 裸衣为手动触发：注入「发动裸衣」。
        let pid = st.seats[cur].player_id.clone();
        drive_state_inject(st, &mut vec![format!("{pid}|choice=choose:luoyi")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(
        st.seats[cur].hand.len(),
        2,
        "裸衣：原有 1 张 + 摸 1 张（2-1）"
    );
    assert_eq!(st.counter("luoyi"), 1, "裸衣本回合标记");
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = st.seats[target].hp;
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp - 2, "裸衣：【杀】伤害 +1");
    st.check_conservation().unwrap();
}

#[test]
fn tiandu_gains_judge_card() {
    let mgr = manager(904);
    let room = start_and_pick(&mgr, 5);
    let mut judge_card = String::new();
    let mut owner = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_20"); // 郭嘉：天妒
        let lebu = find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[owner].judge_zone.push(lebu);
        judge_card = st
            .cards
            .iter()
            .find(|(_, c)| c.suit == Suit::Heart)
            .map(|(id, _)| id.clone())
            .expect("红桃牌");
        take_card(st, &judge_card);
        st.deck.draw.push(judge_card.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("天妒")),
        "应有天妒历史：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert!(
        st.seats[owner].hand.contains(&judge_card),
        "判定牌应进入郭嘉手牌"
    );
    assert!(!st.deck.discard.contains(&judge_card), "判定牌不应进弃牌堆");
    st.check_conservation().unwrap();
}

#[test]
fn qianxun_blocks_shunshou_and_guohe() {
    let mgr = manager(905);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut luxun = 0;
    let mut right = 0;
    let mut shunshou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        luxun = st.seats_from_left_of(cur)[0];
        set_general(st, luxun, "sgs_hero_14"); // 陆逊：谦逊
        give(st, luxun, "shan");
        give(st, luxun, "sha");
        // 距离 1 且不是陆逊的对照目标（顺手要求距离 ≤1；随机武将里可能已有陆逊）。
        let n = st.seats.len();
        let left_n = (cur + 1) % n;
        let right_n = (cur + n - 1) % n;
        right = if left_n != luxun { left_n } else { right_n };
        set_general(st, right, "sgs_hero_2");
        give(st, right, "shan");
        shunshou = give(st, cur, "shunshou_qianyang");
        give(st, cur, "guohe_chaiqiao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let luxun_pid = pid(&st, luxun);
    mgr.execute_action(&room, &cur_pid, &format!("play:{shunshou}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("target:{luxun_pid}")),
        "谦逊：不能成为顺手牵羊目标：{acts:?}"
    );
    assert!(mgr
        .execute_action(&room, &cur_pid, &format!("target:{luxun_pid}"))
        .is_err());
    let other = acts
        .iter()
        .find(|a| a.id.starts_with("target:"))
        .unwrap_or_else(|| panic!("其他合法目标：{acts:?}"))
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &other).unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    state_of(&mgr, &room).check_conservation().unwrap();
}

#[test]
fn lianying_draws_after_losing_last_hand() {
    let mgr = manager(906);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_14"); // 陆逊：连营
        st.seats[cur].hp = st.seats[cur].max_hp - 1;
        give(st, cur, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let tao = in_hand(&st, cur, "tao");
    let hp = st.seats[cur].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{tao}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp + 1);
    assert!(
        st.history.iter().any(|e| e.text.contains("连营")),
        "应有连营历史"
    );
    assert_eq!(st.seats[cur].hand.len(), 1, "连营：失去最后手牌后摸 1 张");
    st.check_conservation().unwrap();
}

#[test]
fn xiaoji_draws_after_losing_equip() {
    let mgr = manager(907);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_25"); // 孙尚香：枭姬
        let bagua = find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[cur].equips.insert(EquipSlot::Armor, Some(bagua));
        give(st, cur, "renwang_dun");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dun = in_hand(&st, cur, "renwang_dun");
    mgr.execute_action(&room, &cur_pid, &format!("play:{dun}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("枭姬")),
        "应有枭姬历史：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert_eq!(st.seats[cur].hand.len(), 2, "枭姬：失去装备后摸 2 张");
    assert_eq!(
        st.seats[cur]
            .equips
            .get(&EquipSlot::Armor)
            .cloned()
            .flatten()
            .as_deref(),
        Some(dun.as_str())
    );
    st.check_conservation().unwrap();
}

#[test]
fn luoyi_can_be_declined_for_normal_draw() {
    let mgr = manager(908);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_19"); // 许褚：裸衣
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "sha");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let pid = st.seats[cur].player_id.clone();
        drive_state_inject(st, &mut vec![format!("{pid}|choice=pass")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(
        st.seats[cur].hand.len(),
        3,
        "不发动裸衣应正常摸 2 张（1 + 2）"
    );
    assert_eq!(st.counter("luoyi"), 0, "放弃裸衣不叠加伤害");
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let target_hp = st.seats[target].hp;
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp - 1, "未发动裸衣伤害 +0");
    st.check_conservation().unwrap();
}
