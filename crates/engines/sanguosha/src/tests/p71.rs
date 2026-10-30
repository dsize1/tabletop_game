//! P4.4.1 火包用例（二）：太史慈 / 庞德 / 颜良文丑 / 袁绍。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, state_of, take_card,
};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn free_card_where(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let claimed = |id: &str| {
        st.seats.iter().any(|s| {
            s.hand.iter().any(|c| c == id)
                || s.marks.values().flatten().any(|c| c == id)
                || s.judge_zone.iter().any(|c| c == id)
                || s.equips.values().flatten().any(|c| c == id)
        })
    };
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"))
}

fn top_card(st: &mut SgsState, filter: impl Fn(&CardInst) -> bool) -> String {
    let id = free_card_where(st, filter);
    take_card(st, &id);
    st.deck.draw.push(id.clone());
    id
}

/// 给 a 一张 K、给 b 一张 A（a 赢）。
fn pindian_win(st: &mut SgsState, a: usize, b: usize) -> (String, String) {
    let high = free_card_where(st, |c| c.rank == 13);
    take_card(st, &high);
    st.seats[a].hand.push(high.clone());
    let low = free_card_where(st, |c| c.rank == 1);
    take_card(st, &low);
    st.seats[b].hand.push(low.clone());
    (high, low)
}

// ---------------------------------------------------------------------------
// 太史慈 · 天义
// ---------------------------------------------------------------------------

#[test]
fn tianyi_win_buffs_sha_and_allows_two_targets() {
    let mgr = manager(740);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut high, mut low, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_38");
        set_general(st, target, "sgs_hero_2");
        (high, low) = pindian_win(st, cur, target);
        sha = give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:tianyi").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{cur_pid}|choice=choose:{high}"),
            format!("{target_pid}|choice=choose:{low}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("tianyi"), 1);
    assert_eq!(crate::turn::sha_limit(&st, cur), 2);
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id.starts_with("targets:"))),
        "天义赢后应可多指定一个目标"
    );
}

#[test]
fn tianyi_lose_blocks_sha() {
    let mgr = manager(741);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut low, mut high, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_38");
        set_general(st, target, "sgs_hero_2");
        pindian_win(st, cur, target);
        // 交换：发起者小点 → 没赢。
        let a = st.seats[cur].hand.pop().unwrap();
        let b = st.seats[target].hand.pop().unwrap();
        st.seats[cur].hand.push(b);
        st.seats[target].hand.push(a);
        low = st.seats[cur].hand.last().cloned().unwrap();
        high = st.seats[target].hand.last().cloned().unwrap();
        sha = give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:tianyi").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{cur_pid}|choice=choose:{low}"),
            format!("{target_pid}|choice=choose:{high}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("tianyi_lose"), 1);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("play:{sha}")),
        "天义没赢：本回合不能使用【杀】"
    );
}

#[test]
fn tianyi_tie_counts_as_not_winning() {
    let mgr = manager(742);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut a, mut b) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_38");
        set_general(st, target, "sgs_hero_2");
        a = free_card_where(st, |c| c.rank == 7);
        take_card(st, &a);
        st.seats[cur].hand.push(a.clone());
        b = free_card_where(st, |c| c.rank == 7);
        take_card(st, &b);
        st.seats[target].hand.push(b.clone());
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:tianyi").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{cur_pid}|choice=choose:{a}"),
            format!("{target_pid}|choice=choose:{b}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("tianyi_lose"), 1);
}

// ---------------------------------------------------------------------------
// 庞德 · 马术 / 鞬出
// ---------------------------------------------------------------------------

#[test]
fn mashu_reduces_distance() {
    let mgr = manager(750);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_39");
        set_general(st, far, "sgs_hero_2");
    });
    let st = state_of(&mgr, &room);
    assert!(st.in_attack_range(cur, far), "马术应使距离 -1");
    assert!(crate::turn::trick_distance_ok(&st, cur, far));
}

#[test]
fn jiandu_discarding_equipment_blocks_shan() {
    let mgr = manager(751);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut armor) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_39");
        set_general(st, target, "sgs_hero_2");
        sha = give(st, cur, "sha");
        armor = free_card_where(st, |c| c.def == "renwang_dun");
        take_card(st, &armor);
        st.seats[target]
            .equips
            .insert(EquipSlot::Armor, Some(armor.clone()));
        st.seats[target].hp = 4;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("jiandu")
    );
    mgr.execute_action(&room, &cur_pid, "pick:equip:Armor")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&armor));
    assert_eq!(st.seats[target].hp, 3, "弃装备后不能闪，杀应命中");
}

#[test]
fn jiandu_discarding_hand_gives_sha_to_target() {
    let mgr = manager(752);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_39");
        set_general(st, target, "sgs_hero_2");
        sha = give(st, cur, "sha");
        give(st, target, "shan");
        st.seats[target].hp = 4;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 4, "非装备牌：目标获得【杀】，不受伤");
    assert!(st.seats[target].hand.contains(&sha));
    assert!(st.history.iter().any(|e| e.text.contains("获得此【杀】")));
}

// ---------------------------------------------------------------------------
// 颜良文丑 · 双雄
// ---------------------------------------------------------------------------

#[test]
fn shuangxiong_judges_and_gains_card() {
    let mgr = manager(760);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut judge) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_40");
        judge = top_card(st, |c| crate::skills::is_red(c.suit));
        let pid_cur = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![format!("{pid_cur}|choice=choose:shuangxiong")],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].hand.contains(&judge));
    assert_eq!(st.counter("shuangxiong_color"), 1);
    assert!(st.history.iter().any(|e| e.text.contains("双雄")));
    assert_eq!(st.turn_phase, TurnPhase::Play, "双雄替代摸牌，不额外摸牌");
}

#[test]
fn shuangxiong_opposite_color_as_duel() {
    let mgr = manager(761);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut black) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        cur_pid = pid(st, cur);
        set_general(st, cur, "sgs_hero_40");
        st.set_counter("shuangxiong_color", 1); // 判定为红
        black = free_card_where(st, |c| crate::skills::is_black(c.suit));
        take_card(st, &black);
        st.seats[cur].hand.push(black.clone());
    });
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("skill:shuangxiong:{black}")));
    mgr.execute_action(&room, &cur_pid, &format!("skill:shuangxiong:{black}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.as_ref().is_some_and(|p| p.kind == "target"));
    assert!(st.history.iter().any(|e| e.text.contains("决斗")));
}

#[test]
fn shuangxiong_same_color_not_offered() {
    let mgr = manager(762);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut red) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_40");
        st.set_counter("shuangxiong_color", 1); // 判定为红
        red = free_card_where(st, |c| crate::skills::is_red(c.suit));
        take_card(st, &red);
        st.seats[cur].hand.push(red.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id.starts_with("skill:shuangxiong:")));
}

// ---------------------------------------------------------------------------
// 袁绍 · 乱击 / 血裔
// ---------------------------------------------------------------------------

#[test]
fn luanji_two_same_suit_as_wanjian() {
    let mgr = manager(770);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut c1, mut c2) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_41");
        c1 = free_card_where(st, |c| c.suit == Suit::Club);
        take_card(st, &c1);
        st.seats[cur].hand.push(c1.clone());
        c2 = free_card_where(st, |c| c.suit == Suit::Club);
        take_card(st, &c2);
        st.seats[cur].hand.push(c2.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("skill:luanji:{c1},{c2}")));
    mgr.execute_action(&room, &cur_pid, &format!("skill:luanji:{c1},{c2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&c2), "第二张牌作为费用弃置");
    let damaged: Vec<usize> = st
        .seats
        .iter()
        .enumerate()
        .filter(|(i, s)| *i != cur && s.hp == s.max_hp - 1)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(damaged.len(), 4, "万箭齐发应对所有其他角色各造成 1 点伤害");
    assert!(st.history.iter().any(|e| e.text.contains("乱击")));
}

#[test]
fn luanji_requires_same_suit() {
    let mgr = manager(771);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_41");
        give(st, cur, "sha");
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id.starts_with("skill:luanji:")));
}

#[test]
fn xueyi_increases_hand_limit_for_qun_allies() {
    let mgr = manager(772);
    let room = start_and_pick(&mgr, 5);
    let mut lord = 0;
    let mut none_lord = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.lord_index().expect("lord");
        let others: Vec<usize> = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != lord)
            .collect();
        set_general(st, lord, "sgs_hero_41");
        st.seats[lord].hp = 3;
        set_general(st, others[0], "sgs_hero_23"); // 吕布（群）
        set_general(st, others[1], "sgs_hero_24"); // 貂蝉（群）
        set_general(st, others[2], "sgs_hero_2"); // 关羽（蜀）
        set_general(st, others[3], "sgs_hero_15"); // 曹操（魏）
        none_lord = others[2];
    });
    let st = state_of(&mgr, &room);
    assert_eq!(crate::skills::hand_limit(&st, lord), 3 + 2 * 2);
    // 血裔是主公技：非主公不生效。
    st.seats.iter().for_each(|_| {});
    assert_eq!(
        crate::skills::hand_limit(&st, none_lord),
        st.seats[none_lord].hp.max(0) as usize
    );
}
