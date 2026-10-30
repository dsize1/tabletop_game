//! P4.4.1 山包用例（一）：张郃 / 邓艾 / 姜维 / 刘禅。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, place_mark, state_of,
    take_card,
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
    let claimed = |id: &str| {
        st.deck.draw.iter().any(|c| c == id)
            || st.deck.processing.iter().any(|c| c == id)
            || st.seats.iter().any(|s| {
                s.hand.iter().any(|c| c == id)
                    || s.marks.values().flatten().any(|c| c == id)
                    || s.judge_zone.iter().any(|c| c == id)
                    || s.equips.values().flatten().any(|c| c == id)
            })
    };
    let id = st
        .cards
        .iter()
        .find(|(id, c)| filter(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"));
    take_card(st, &id);
    st.deck.draw.push(id.clone());
    id
}

// ---------------------------------------------------------------------------
// 张郃 · 巧变
// ---------------------------------------------------------------------------

#[test]
fn qiaobian_skip_draw_gains_two_hand_cards() {
    let mgr = manager(900);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut t1, mut t2, mut cost) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        t1 = st.seats_from_left_of(cur)[0];
        t2 = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_50");
        set_general(st, t1, "sgs_hero_2");
        set_general(st, t2, "sgs_hero_15");
        give(st, cur, "sha");
        cost = give(st, cur, "tao");
        give(st, t1, "shan");
        give(st, t2, "wuxie_keji");
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_c}|choice=choose:qiaobian:draw"),
                format!("{pid_c}|choose_many=choose:{cost}"),
                format!("{pid_c}|choose_many=confirm"),
                format!("{pid_c}|target=target:{}", pid(st, t1)),
                format!("{pid_c}|target=target:{}", pid(st, t2)),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "2 - 1 费 + 2 获得");
    assert!(st.history.iter().any(|e| e.text.contains("巧变")));
}

#[test]
fn qiaobian_skip_play_moves_field_equipment() {
    let mgr = manager(901);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut other, mut equip, mut cost) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        other = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_50");
        set_general(st, other, "sgs_hero_2");
        equip = free_card_where(st, |c| c.def == "qinggang_jian");
        take_card(st, &equip);
        st.seats[other]
            .equips
            .insert(EquipSlot::Weapon, Some(equip.clone()));
        cost = give(st, cur, "sha");
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_c}|choice=choose:qiaobian:play"),
                format!("{pid_c}|choose_many=choose:{cost}"),
                format!("{pid_c}|choose_many=confirm"),
                format!("{pid_c}|target=move:{equip}"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur]
            .equips
            .get(&EquipSlot::Weapon)
            .cloned()
            .flatten(),
        Some(equip),
        "巧变：移动场牌到自己装备区"
    );
}

#[test]
fn qiaobian_skip_discard_keeps_hand() {
    let mgr = manager(902);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cost) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_50");
        st.seats[cur].hp = 2;
        for _ in 0..4 {
            give(st, cur, "sha");
        }
        cost = st.seats[cur].hand[0].clone();
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_c}|choice=choose:qiaobian:discard"),
                format!("{pid_c}|choose_many=choose:{cost}"),
                format!("{pid_c}|choose_many=confirm"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        5,
        "4 - 1 费 + 摸牌 2 = 5；跳过弃牌阶段保留"
    );
}

// ---------------------------------------------------------------------------
// 邓艾 · 屯田 / 凿险 / 急袭
// ---------------------------------------------------------------------------

#[test]
fn tuotian_judges_and_marks_tian_out_of_turn() {
    let mgr = manager(910);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut dengai, mut guohe, mut before) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        dengai = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, dengai, "sgs_hero_51");
        give(st, dengai, "shan");
        guohe = give(st, cur, "guohe_chaiqiao");
        top_card(st, |c| c.suit == Suit::Spade);
        before = st.seats[dengai].hand.len();
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, pid_d) = (pid(&st, cur), pid(&st, dengai));
    mgr.execute_action(&room, &cur_pid, &format!("play:{guohe}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{pid_d}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![format!("{cur_pid}|pick_card=pick:hand")],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dengai].hand.len(), before - 1);
    assert_eq!(
        st.seats[dengai]
            .marks
            .get("tian")
            .map(Vec::len)
            .unwrap_or(0),
        1,
        "屯田非红桃判定应置为「田」"
    );
    assert_eq!(crate::skills::tuotian_bonus(&st, dengai), 1);
}

#[test]
fn zaoxian_awakening_grants_jixi() {
    let mgr = manager(911);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut max_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_51");
        max_before = st.seats[cur].max_hp;
        for _ in 0..3 {
            let id = free_card_where(st, |c| c.rank == 3);
            place_mark(st, cur, "tian", &id);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("awaken_zaoxian"));
    assert!(crate::skills::has(&st, cur, "jixi"));
    assert_eq!(st.seats[cur].max_hp, max_before - 1);
}

#[test]
fn jixi_equipment_hand_as_sha() {
    let mgr = manager(912);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut equip) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_51");
        set_general(st, target, "sgs_hero_2");
        crate::skills::grant_skill(st, cur, "jixi");
        equip = give(st, cur, "bagua_zhen");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("skill:jixi:{equip}")));
    mgr.execute_action(&room, &cur_pid, &format!("skill:jixi:{equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.as_ref().is_some_and(|p| p.kind == "target"));
}

// ---------------------------------------------------------------------------
// 姜维 · 挑衅 / 志继
// ---------------------------------------------------------------------------

#[test]
fn tiaoxin_target_uses_sha() {
    let mgr = manager(920);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_52");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = 4;
        sha = give(st, target, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:tiaoxin")
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{target_pid}|choice=choose:tiaoxin:sha"),
            format!("{cur_pid}|respond=pass"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 3, "挑衅：目标出杀命中姜维");
    let _ = sha;
}

#[test]
fn tiaoxin_target_declines_and_loses_card() {
    let mgr = manager(921);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut card) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_52");
        set_general(st, target, "sgs_hero_2");
        card = give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    let before = st.seats[target].hand.len();
    mgr.execute_action(&room, &cur_pid, "skill:tiaoxin")
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{target_pid}|choice=pass"),
            format!("{cur_pid}|pick_card=pick:hand"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), before - 1);
    assert!(st.history.iter().any(|e| e.text.contains("挑衅")));
    let _ = card;
}

#[test]
fn zhiji_awakening_grants_guanxing() {
    let mgr = manager(922);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut max_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_52");
        max_before = st.seats[cur].max_hp;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let pid_c = pid(st, cur);
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:awaken:draw")]);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("awaken_zhiji"));
    assert!(crate::skills::has(&st, cur, "guanxing"));
    assert_eq!(st.seats[cur].max_hp, max_before - 1);
    assert_eq!(st.seats[cur].hand.len(), 4, "志继摸 2 + 正常摸牌 2");
}

// ---------------------------------------------------------------------------
// 刘禅 · 享乐 / 放权
// ---------------------------------------------------------------------------

#[test]
fn xiangle_blocks_sha_without_basic_discard() {
    let mgr = manager(930);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut liushan, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        liushan = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, liushan, "sgs_hero_53");
        st.seats[liushan].hp = 3;
        sha = give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, ls_pid) = (pid(&st, cur), pid(&st, liushan));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{ls_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("xiangle")
    );
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[liushan].hp, 3, "享乐：未弃基本牌则杀无效");
}

#[test]
fn xiangle_attacker_discards_basic_to_continue() {
    let mgr = manager(931);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut liushan, mut sha, mut shan) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        liushan = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, liushan, "sgs_hero_53");
        st.seats[liushan].hp = 3;
        sha = give(st, cur, "sha");
        shan = give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, ls_pid) = (pid(&st, cur), pid(&st, liushan));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{ls_pid}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|choice=choose:xiangle"),
            format!("{cur_pid}|choose_many=choose:{shan}"),
            format!("{cur_pid}|choose_many=confirm"),
            format!("{ls_pid}|respond=pass"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[liushan].hp, 2, "弃基本牌后杀正常结算");
}

#[test]
fn fangquan_grants_extra_turn() {
    let mgr = manager(932);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_53");
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:fangquan")
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![format!("{cur_pid}|target=target:{target_pid}")],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn, target, "放权：目标执行额外回合");
    assert!(st.history.iter().any(|e| e.text.contains("放权")));
}
