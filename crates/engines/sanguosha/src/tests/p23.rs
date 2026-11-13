//! P2.3 测试：无懈链、锦囊、判定前无懈、死亡奖惩与胜负。

use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, find_card, give, lobby_with, manager,
    pick_all, state_of, take_card,
};
use crate::state::*;
use game_engine_core::GameManager;

fn start_and_pick(mgr: &GameManager, players: usize) -> String {
    let room = lobby_with(mgr, players);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

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

fn discards(st: &SgsState, code: &str) -> usize {
    st.deck
        .discard
        .iter()
        .filter(|id| st.cards.get(*id).map(|c| c.def.as_str()) == Some(code))
        .count()
}

fn others_from_left(st: &SgsState, cur: usize) -> Vec<usize> {
    st.seats_from_left_of(cur)
        .into_iter()
        .filter(|i| st.seats[*i].alive)
        .collect()
}

#[test]
fn wuxie_negates_aoe_for_one_target_only() {
    let mgr = manager(40);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "nanman_ruqin");
        let b = st.seats_from_left_of(cur)[0];
        give(st, b, "wuxie_keji");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let order = others_from_left(&st, cur);
    let b = order[0];
    let b_pid = pid(&st, b);
    let nanman = in_hand(&st, cur, "nanman_ruqin");
    let wuxie = in_hand(&st, b, "wuxie_keji");
    let hp_before: Vec<i32> = order.iter().map(|i| st.seats[*i].hp).collect();
    let nanman_before = discards(&st, "nanman_ruqin");
    let wuxie_before = discards(&st, "wuxie_keji");

    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    let mut inject = vec![format!("{b_pid}|wuxie=respond:{wuxie}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[b].hp, hp_before[0], "B 被无懈，不应受伤");
    for (k, idx) in order.iter().enumerate().skip(1) {
        assert_eq!(st.seats[*idx].hp, hp_before[k] - 1, "其他目标应受 1 点伤害");
    }
    assert_eq!(discards(&st, "nanman_ruqin"), nanman_before + 1);
    assert_eq!(discards(&st, "wuxie_keji"), wuxie_before + 1);
    st.check_conservation().unwrap();
}

#[test]
fn wuxie_nesting_three_layers_cancels_effect() {
    let mgr = manager(41);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wanjian_qifa");
        let order = st.seats_from_left_of(cur);
        for i in 0..3 {
            give(st, order[i], "wuxie_keji");
        }
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let order = others_from_left(&st, cur);
    let first = order[0];
    let hp_before: Vec<i32> = order.iter().map(|i| st.seats[*i].hp).collect();
    let aoe = in_hand(&st, cur, "wanjian_qifa");
    let wuxie_before = discards(&st, "wuxie_keji");
    let mut inject = vec![
        format!(
            "{}|wuxie=respond:{}",
            pid(&st, order[0]),
            in_hand(&st, order[0], "wuxie_keji")
        ),
        format!(
            "{}|wuxie=respond:{}",
            pid(&st, order[1]),
            in_hand(&st, order[1], "wuxie_keji")
        ),
        format!(
            "{}|wuxie=respond:{}",
            pid(&st, order[2]),
            in_hand(&st, order[2], "wuxie_keji")
        ),
    ];

    mgr.execute_action(&room, &cur_pid, &format!("play:{aoe}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[first].hp, hp_before[0],
        "3 层无懈（奇数）应抵消第一个目标的效果"
    );
    for (k, idx) in order.iter().enumerate().skip(1) {
        assert_eq!(st.seats[*idx].hp, hp_before[k] - 1);
    }
    assert_eq!(discards(&st, "wuxie_keji"), wuxie_before + 3);
}

#[test]
fn judge_wuxie_skips_judgment_and_discards_lebu() {
    let mgr = manager(42);
    let room = start_and_pick(&mgr, 5);
    let mut draw_at_start = 0usize;
    let mut lebu_in_discard = 0usize;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lebu_in_discard = discards(st, "lebu_sishu");
        let cur = st.turn;
        let lebu = find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[cur].judge_zone.push(lebu);
        let b = st.seats_from_left_of(cur)[0];
        let wuxie = give(st, b, "wuxie_keji");
        let b_pid = st.seats[b].player_id.clone();
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        draw_at_start = st.deck.draw.len();
        let mut inject = vec![format!("{b_pid}|wuxie=respond:{wuxie}")];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play, "无懈后不应跳过出牌阶段");
    assert!(!st.history.iter().any(|e| e.text.starts_with("判定【")));
    // 无懈后乐不思蜀直接进弃牌堆（弃牌堆里可能已有其他副本，按增量断言）。
    assert_eq!(discards(&st, "lebu_sishu"), lebu_in_discard + 1);
    assert_eq!(st.deck.draw.len(), draw_at_start - 2);
    st.check_conservation().unwrap();
}

#[test]
fn lightning_wuxie_moves_to_left_neighbor() {
    let mgr = manager(43);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        let lightning = find_card(st, "shandian");
        take_card(st, &lightning);
        st.seats[cur].judge_zone.push(lightning);
        let b = st.seats_from_left_of(cur)[0];
        let wuxie = give(st, b, "wuxie_keji");
        let b_pid = st.seats[b].player_id.clone();
        st.turn_phase = TurnPhase::Prepare;
        let mut inject = vec![format!("{b_pid}|wuxie=respond:{wuxie}")];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    let damaged: Vec<usize> = st
        .seats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.hp < s.max_hp)
        .map(|(i, _)| i)
        .collect();
    assert!(damaged.is_empty(), "被无懈的闪电不应造成伤害");
    let moved: Vec<usize> = st
        .seats
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.judge_zone
                .iter()
                .any(|id| st.cards.get(id).map(|c| c.def.as_str()) == Some("shandian"))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(moved.len(), 1, "闪电应流转到一名玩家判定区");
    assert_eq!(discards(&st, "shandian"), 0, "被无懈的闪电不进入弃牌堆");
}

#[test]
fn shunshou_takes_equipment_into_hand() {
    let mgr = manager(44);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "shunshou_qianyang");
        let b = st.seats_from_left_of(cur)[0];
        let weapon = find_card(st, "qinggang_jian");
        take_card(st, &weapon);
        st.seats[b].equips.insert(EquipSlot::Weapon, Some(weapon));
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let b = others_from_left(&st, cur)[0];
    let b_pid = pid(&st, b);
    let weapon_id = st.seats[b]
        .equips
        .get(&EquipSlot::Weapon)
        .cloned()
        .unwrap()
        .unwrap();
    let card = in_hand(&st, cur, "shunshou_qianyang");
    let shunshou_before = discards(&st, "shunshou_qianyang");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![
        format!("{cur_pid}|target=target:{b_pid}"),
        format!("{cur_pid}|pick_card=pick:equip:weapon"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert!(st.seats[b]
        .equips
        .get(&EquipSlot::Weapon)
        .unwrap()
        .is_none());
    assert!(
        st.seats[cur].hand.contains(&weapon_id),
        "顺手的装备应进入手牌"
    );
    assert_eq!(discards(&st, "shunshou_qianyang"), shunshou_before + 1);
    st.check_conservation().unwrap();
}

#[test]
fn guohe_discards_random_hand_card() {
    let mgr = manager(45);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "guohe_chaiqiao");
        let b = st.seats_from_left_of(cur)[0];
        give(st, b, "sha");
        give(st, b, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let b = others_from_left(&st, cur)[0];
    let b_pid = pid(&st, b);
    let hand_before = st.seats[b].hand.len();
    let discard_before = st.deck.discard.len();
    let card = in_hand(&st, cur, "guohe_chaiqiao");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![
        format!("{cur_pid}|target=target:{b_pid}"),
        format!("{cur_pid}|pick_card=pick:hand"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[b].hand.len(), hand_before - 1);
    assert_eq!(st.deck.discard.len(), discard_before + 2);
    st.check_conservation().unwrap();
}

#[test]
fn wuzhong_draws_two_after_wuxie_window() {
    let mgr = manager(46);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let card = in_hand(&st, cur, "wuzhong_shengyou");
    let wuzhong_before = discards(&st, "wuzhong_shengyou");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 2, "无中生有应摸 2 张");
    assert_eq!(discards(&st, "wuzhong_shengyou"), wuzhong_before + 1);
}

#[test]
fn nanman_sha_response_avoids_damage() {
    let mgr = manager(47);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "nanman_ruqin");
        let c = st.seats_from_left_of(cur)[2];
        give(st, c, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let order = others_from_left(&st, cur);
    let c = order[2];
    let c_pid = pid(&st, c);
    let sha = in_hand(&st, c, "sha");
    let hp: Vec<i32> = order.iter().map(|i| st.seats[*i].hp).collect();
    let card = in_hand(&st, cur, "nanman_ruqin");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{c_pid}|respond=respond:{sha}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[c].hp, hp[2], "出了杀的目标不受伤害");
    for (k, idx) in order.iter().enumerate() {
        if *idx != c {
            assert_eq!(st.seats[*idx].hp, hp[k] - 1);
        }
    }
}

#[test]
fn wanjian_shan_response_avoids_damage() {
    let mgr = manager(48);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wanjian_qifa");
        let c = st.seats_from_left_of(cur)[1];
        give(st, c, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let order = others_from_left(&st, cur);
    let c = order[1];
    let c_pid = pid(&st, c);
    let shan = in_hand(&st, c, "shan");
    let hp: Vec<i32> = order.iter().map(|i| st.seats[*i].hp).collect();
    let card = in_hand(&st, cur, "wanjian_qifa");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{c_pid}|respond=respond:{shan}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[c].hp, hp[1]);
    for (k, idx) in order.iter().enumerate() {
        if *idx != c {
            assert_eq!(st.seats[*idx].hp, hp[k] - 1);
        }
    }
}

#[test]
fn taoyuan_heals_only_wounded() {
    let mgr = manager(49);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "taoyuan_jieyi");
        st.seats[cur].hp -= 1;
        let c = st.seats_from_left_of(cur)[1];
        st.seats[c].hp -= 1;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let hurt_max: Vec<(usize, i32, i32)> = st
        .seats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.hp < s.max_hp)
        .map(|(i, s)| (i, s.hp, s.max_hp))
        .collect();
    let card = in_hand(&st, cur, "taoyuan_jieyi");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    for (idx, hp, _) in hurt_max {
        assert_eq!(st.seats[idx].hp, hp + 1);
    }
}

#[test]
fn duel_alternates_sha_and_target_takes_damage() {
    let mgr = manager(50);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "juedou");
        let b = st.seats_from_left_of(cur)[0];
        give(st, b, "sha");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let b = others_from_left(&st, cur)[0];
    let b_pid = pid(&st, b);
    let b_sha = in_hand(&st, b, "sha");
    let cur_sha = in_hand(&st, cur, "sha");
    let hp = st.seats[b].hp;
    let card = in_hand(&st, cur, "juedou");
    let juedou_before = discards(&st, "juedou");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    // 只在一开始有无懈窗口，之后交替出杀（B → 使用者 → B 放弃）
    let mut inject = vec![
        format!("{cur_pid}|target=target:{b_pid}"),
        format!("{b_pid}|respond=respond:{b_sha}"),
        format!("{cur_pid}|respond=respond:{cur_sha}"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[b].hp, hp - 1, "B 未能出杀，应受 1 点伤害");
    assert_eq!(discards(&st, "juedou"), juedou_before + 1);
}

#[test]
fn jiedao_asks_weapon_owner_even_without_sha() {
    let mgr = manager(51);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "jiedao_sharen");
        let a = st.seats_from_left_of(cur)[0];
        let weapon = find_card(st, "qinggang_jian");
        take_card(st, &weapon);
        st.seats[a].equips.insert(EquipSlot::Weapon, Some(weapon));
        // A 没有【杀】：仍必须被询问
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let order = others_from_left(&st, cur);
    let a = order[0];
    let a_pid = pid(&st, a);
    let card = in_hand(&st, cur, "jiedao_sharen");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    // 杀的合法目标由引擎给出（已排除空城等锁定技），不假定座位顺序。
    let b_pid = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .find_map(|act| act.id.strip_prefix("target:").map(str::to_string))
        .expect("借刀第二阶段应有合法杀目标");
    let mut inject = vec![
        format!("{cur_pid}|target=target:{b_pid}"),
        format!("{cur_pid}|target=confirm"),
        format!("{a_pid}|respond=pass"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert!(
        st.seats[a]
            .equips
            .get(&EquipSlot::Weapon)
            .unwrap()
            .is_none(),
        "A 未出杀，武器应被拿走"
    );
    assert_eq!(
        st.seats[cur]
            .hand
            .iter()
            .filter(|id| st.cards.get(*id).map(|c| c.def.as_str()) == Some("qinggang_jian"))
            .count(),
        1,
        "武器应交给借刀使用者"
    );
    assert!(st.history.iter().any(|e| e.text.contains("未出杀")));
    st.check_conservation().unwrap();
}

#[test]
fn killing_rebel_draws_three_for_killer() {
    let mgr = manager(52);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        st.seats[cur].identity = Identity::Lord;
        give(st, cur, "sha");
        let t = st.seats_from_left_of(cur)[0];
        st.seats[t].identity = Identity::Rebel;
        st.seats[t].hp = 1;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let t = others_from_left(&st, cur)[0];
    let t_pid = pid(&st, t);
    let card = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|target=target:{t_pid}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert!(!st.seats[t].alive);
    assert_eq!(st.seats[cur].hand.len(), 3, "杀反贼应摸 3 张");
}

#[test]
fn lord_killing_loyalist_discards_all() {
    let mgr = manager(53);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        st.seats[cur].identity = Identity::Lord;
        give(st, cur, "sha");
        give(st, cur, "shan");
        let weapon = find_card(st, "qinggang_jian");
        take_card(st, &weapon);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(weapon));
        let t = st.seats_from_left_of(cur)[0];
        st.seats[t].identity = Identity::Loyalist;
        st.seats[t].hp = 1;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let t = others_from_left(&st, cur)[0];
    let t_pid = pid(&st, t);
    let card = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|target=target:{t_pid}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert!(!st.seats[t].alive);
    assert!(st.seats[cur].hand.is_empty(), "主公应弃置所有手牌");
    assert!(st.seats[cur]
        .equips
        .get(&EquipSlot::Weapon)
        .unwrap()
        .is_none());
}

#[test]
fn lord_death_makes_rebels_win() {
    let mgr = manager(54);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        st.seats[cur].identity = Identity::Rebel;
        give(st, cur, "sha");
        let t = st.seats_from_left_of(cur)[0];
        st.seats[t].identity = Identity::Lord;
        st.seats[t].hp = 1;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = others_from_left(&st, cur)[0];
    let lord_pid = pid(&st, lord);
    let card = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|target=target:{lord_pid}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Finished, "主公死亡应结束对局");
    let winner = st.winner.expect("应有胜方");
    assert_eq!(winner.team, "rebel");
}

#[test]
fn spy_wins_when_last_alive_after_lord_death() {
    let mgr = manager(55);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        st.seats[cur].identity = Identity::Spy;
        give(st, cur, "sha");
        let lord = st.seats_from_left_of(cur)[0];
        st.seats[lord].identity = Identity::Lord;
        st.seats[lord].hp = 1;
        // 其余玩家全部死亡（牌已清空）
        for i in 0..st.seats.len() {
            if i != cur && i != lord {
                st.seats[i].alive = false;
                st.seats[i].hp = 0;
            }
        }
        st.alive_indices();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.alive_indices().len(), 2, "应只剩主公与内奸");
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = others_from_left(&st, cur)[0];
    let lord_pid = pid(&st, lord);
    let card = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let mut inject = vec![format!("{cur_pid}|target=target:{lord_pid}")];
    drive_mgr_inject(&mgr, &room, &mut inject);

    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Finished);
    assert_eq!(st.winner.expect("winner").team, "spy");
}

#[test]
fn random_play_keeps_conservation() {
    let mgr = manager(42);
    let room = start_and_pick(&mgr, 5);
    let mut last;
    for step in 0..400 {
        let st = state_of(&mgr, &room);
        if st.phase == Phase::Finished {
            break;
        }
        let (actor, action) = if let Some(p) = &st.pending {
            let acts = mgr.legal_actions(&room, &p.actor).unwrap();
            assert!(!acts.is_empty(), "pending {} 无动作", p.kind);
            (
                p.actor.clone(),
                acts.iter()
                    .find(|a| a.id == "pass")
                    .unwrap_or(&acts[0])
                    .id
                    .clone(),
            )
        } else {
            let pid = pid(&st, st.turn);
            let acts = mgr.legal_actions(&room, &pid).unwrap();
            let a = acts
                .iter()
                .find(|a| a.id.starts_with("play:"))
                .or_else(|| acts.iter().find(|a| a.id == "end_play"))
                .expect("无合法动作");
            (pid, a.id.clone())
        };
        last = format!("step {step}: {actor} {action}");
        if let Err(e) = mgr.execute_action(&room, &actor, &action) {
            panic!("{last} 执行失败: {e}");
        }
        let st = state_of(&mgr, &room);
        if let Err(e) = st.check_conservation() {
            eprintln!("{last} 之后不守恒: {e}");
            eprintln!(
                "zones: draw={} discard={} processing={} hands={:?} equips={:?} judge={:?}",
                st.deck.draw.len(),
                st.deck.discard.len(),
                st.deck.processing.len(),
                st.seats.iter().map(|s| s.hand.len()).collect::<Vec<_>>(),
                st.seats
                    .iter()
                    .map(|s| s.equips.values().flatten().count())
                    .collect::<Vec<_>>(),
                st.seats
                    .iter()
                    .map(|s| s.judge_zone.len())
                    .collect::<Vec<_>>(),
            );
            panic!("conservation failed at {last}");
        }
    }
}
