//! P2.4 收尾测试：装备可选效果（青釭剑/八卦阵）与技能边界。

use super::p24::{in_hand, others, pid, set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, find_card, give, manager, state_of, take_card};
use crate::state::*;

#[test]
fn qinggang_ignores_renwang() {
    let mgr = manager(80);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_19");
        give(st, target, "shan");
        // 青釭剑（进攻方）与 仁王盾（防守方）
        let sword = find_card(st, "qinggang_jian");
        take_card(st, &sword);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(sword));
        let shield = find_card(st, "renwang_dun");
        take_card(st, &shield);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(shield));
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");
    let hp = st.seats[target].hp;

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond"),
        "青釭剑应无视仁王盾，目标仍需出闪"
    );
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1);
}

#[test]
fn bagua_red_counts_as_shan() {
    let mgr = manager(81);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = st.seats_from_left_of(cur)[0];
        let bagua = find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(bagua));
        // 判定牌：红桃 5
        let red = st
            .cards
            .iter()
            .find(|(_, c)| c.suit == Suit::Heart && c.rank == 5)
            .map(|(id, _)| id.clone())
            .unwrap();
        take_card(st, &red);
        st.deck.draw.push(red);
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");
    let hp = st.seats[target].hp;

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let pending = state_of(&mgr, &room).pending.expect("应询问闪");
    assert!(pending.options.iter().any(|o| o.id == "skill:bagua"));
    mgr.execute_action(&room, &target_pid, "skill:bagua")
        .unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "八卦阵红色应视为出闪");
    assert!(st.pending.is_none());
    st.check_conservation().unwrap();
}

#[test]
fn bagua_black_fails_then_pass_takes_damage() {
    let mgr = manager(82);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = st.seats_from_left_of(cur)[0];
        let bagua = find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(bagua));
        let black = st
            .cards
            .iter()
            .find(|(_, c)| c.suit == Suit::Spade && c.rank == 5)
            .map(|(id, _)| id.clone())
            .unwrap();
        take_card(st, &black);
        st.deck.draw.push(black);
        // 手里留一张【闪】：黑色八卦失败后仍可正常出闪（且不能再判八卦）
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");
    let hp = st.seats[target].hp;

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "skill:bagua")
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("八卦阵黑色应继续询问闪");
    assert_eq!(pending.kind, "respond");
    assert!(
        !pending.options.iter().any(|o| o.id == "skill:bagua"),
        "每次需要【闪】只能判定一次八卦阵"
    );
    assert!(
        mgr.execute_action(&room, &target_pid, "skill:bagua")
            .is_err(),
        "重复使用八卦阵应被拒绝"
    );
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1);
}

#[test]
fn qinggang_disables_bagua_option() {
    let mgr = manager(83);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let sword = find_card(st, "qinggang_jian");
        take_card(st, &sword);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(sword));
        let target = st.seats_from_left_of(cur)[0];
        let bagua = find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(bagua));
        // 给目标一张无关手牌：驱动「有手牌时仍显式询问」（无手牌会被自动跳过）。
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let pending = state_of(&mgr, &room).pending.expect("应询问闪");
    assert!(
        !pending.options.iter().any(|o| o.id == "skill:bagua"),
        "青釭剑应无视八卦阵"
    );
}

#[test]
fn keji_with_sha_still_discards() {
    let mgr = manager(84);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_10");
        st.seats[cur].hp = 1;
        give(st, cur, "sha");
        for _ in 0..4 {
            give(st, cur, "shan");
        }
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = others(&st, cur)[0];
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("discard"),
        "使用过杀后克己不生效，应进入弃牌"
    );
}

#[test]
fn qixi_red_card_not_offered() {
    let mgr = manager(85);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_9");
        let red = st
            .cards
            .iter()
            .find(|(_, c)| matches!(c.suit, Suit::Heart | Suit::Diamond))
            .map(|(id, _)| id.clone())
            .unwrap();
        take_card(st, &red);
        st.seats[cur].hand.push(red);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, st.turn);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id.starts_with("skill:qixi:")));
}

#[test]
fn hujia_not_offered_without_wei_ally() {
    let mgr = manager(86);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wanjian_qifa");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_15"); // 曹操
        for i in st.seats_from_left_of(cur).into_iter().skip(1) {
            set_general(st, i, "sgs_hero_2"); // 关羽（蜀）
        }
        let lord_pid = st.seats[lord].player_id.clone();
        let cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}|target=target:{lord_pid}")];
        super::drive_state_inject(st, &mut inject);
        loop {
            let Some(p) = st.pending.clone() else { break };
            if p.actor == lord_pid && p.kind == "respond" {
                assert!(
                    !p.options.iter().any(|o| o.id == "skill:hujia"),
                    "没有魏势力时不应提供护驾"
                );
                break;
            }
            crate::resolve::execute_pending(st, &p.actor, "pass").unwrap();
            crate::resolve::advance(st, &mut Vec::new()).unwrap();
        }
    });
}

#[test]
fn jizhi_triggers_on_wuxie() {
    let mgr = manager(87);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "nanman_ruqin");
        let yueying = st.seats_from_left_of(cur)[0];
        set_general(st, yueying, "sgs_hero_7");
        give(st, yueying, "wuxie_keji");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let yueying = others(&st, cur)[0];
    let yueying_pid = pid(&st, yueying);
    let wuxie = in_hand(&st, yueying, "wuxie_keji");
    let nanman = in_hand(&st, cur, "nanman_ruqin");

    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    let mut inject = vec![format!("{yueying_pid}|wuxie=respond:{wuxie}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    // 打出无懈（-1）后集智摸 1
    assert_eq!(st.seats[yueying].hand.len(), 1);
    assert!(st.history.iter().any(|e| e.text.contains("集智")));
}

#[test]
fn jiuyuan_no_bonus_for_non_wu() {
    let mgr = manager(88);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_8");
        st.seats[lord].hp = 1;
        let healer = st.seats_from_left_of(cur)[1];
        set_general(st, healer, "sgs_hero_2"); // 关羽（蜀）
        give(st, healer, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = others(&st, cur)[0];
    let lord_pid = pid(&st, lord);
    let sha = in_hand(&st, cur, "sha");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{lord_pid}"))
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
    assert_eq!(st.seats[lord].hp, 1, "非吴势力救援不享受 +1");
}

#[test]
fn keji_hand_equal_limit_no_discard() {
    let mgr = manager(89);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_10");
        st.seats[cur].hp = 2;
        give(st, cur, "shan");
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none());
    assert_eq!(st.seats[cur].hand.len(), 2);
}

#[test]
fn tuxi_confirm_early_skips_draw() {
    let mgr = manager(90);
    let room = start_and_pick(&mgr, 5);
    let mut draw_before = 0usize;
    let mut hand_after = 0usize;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_18");
        give(st, cur, "shan");
        let a = st.seats_from_left_of(cur)[0];
        give(st, a, "sha");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        draw_before = st.deck.draw.len();
        let cur_pid = st.seats[cur].player_id.clone();
        let a_pid = st.seats[a].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}|choice=choose:tuxi"),
            format!("{cur_pid}|target=target:{a_pid}"),
            format!("{cur_pid}|target=confirm"),
        ];
        super::drive_state_inject(st, &mut inject);
        hand_after = st.seats[cur].hand.len();
    });
    assert_eq!(hand_after, 2, "只取 1 张后确认，跳过摸牌");
    assert_eq!(draw_before, state_of(&mgr, &room).deck.draw.len());
}

#[test]
fn qixi_transform_still_has_wuxie_window() {
    let mgr = manager(91);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_9");
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
    mgr.execute_action(&room, &cur_pid, &format!("skill:qixi:{black}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let pending = state_of(&mgr, &room)
        .pending
        .expect("转化的过河也应有无懈窗口");
    assert_eq!(pending.kind, "wuxie");
}
