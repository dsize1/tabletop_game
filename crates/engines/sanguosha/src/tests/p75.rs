//! P4.4.1 山包用例（二）：孙策 / 张昭张纮 / 左慈 / 蔡文姬。

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
// 孙策 · 激昂 / 魂姿 / 制霸
// ---------------------------------------------------------------------------

#[test]
fn jiang_draws_on_red_sha() {
    let mgr = manager(940);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_54");
        set_general(st, target, "sgs_hero_2");
        sha = free_card_where(st, |c| c.def == "sha" && crate::skills::is_red(c.suit));
        take_card(st, &sha);
        st.seats[cur].hand.push(sha.clone());
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 1, "激昂：使用红色杀摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("激昂")));
}

#[test]
fn hunzi_awakening_grants_yingzi_and_yinghun() {
    let mgr = manager(941);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut max_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_54");
        st.seats[cur].hp = 1;
        max_before = st.seats[cur].max_hp;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("awaken_hunzi"));
    assert!(crate::skills::has(&st, cur, "yingzi"));
    assert!(crate::skills::has(&st, cur, "yinghun"));
    assert_eq!(st.seats[cur].max_hp, max_before - 1);
}

#[test]
fn zhiba_lord_gains_cards_when_challenger_does_not_win() {
    let mgr = manager(942);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut challenger, mut low, mut high) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.lord_index().unwrap();
        set_general(st, lord, "sgs_hero_54");
        challenger = st.alive_indices().into_iter().find(|i| *i != lord).unwrap();
        set_general(st, challenger, "sgs_hero_9"); // 甘宁（吴）
        st.turn = challenger;
        st.turn_phase = TurnPhase::Play;
        low = free_card_where(st, |c| c.rank == 1);
        take_card(st, &low);
        st.seats[challenger].hand.push(low.clone());
        high = free_card_where(st, |c| c.rank == 13);
        take_card(st, &high);
        st.seats[lord].hand.push(high.clone());
    });
    let st = state_of(&mgr, &room);
    let (lord_pid, ch_pid) = (pid(&st, lord), pid(&st, challenger));
    mgr.execute_action(&room, &ch_pid, "skill:zhiba").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{ch_pid}|choice=choose:{low}"),
            format!("{lord_pid}|choice=choose:{high}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert!(st.seats[lord].hand.contains(&low));
    assert!(st.seats[lord].hand.contains(&high));
    assert!(st.history.iter().any(|e| e.text.contains("制霸")));
}

// ---------------------------------------------------------------------------
// 张昭张纮 · 直谏 / 固政
// ---------------------------------------------------------------------------

#[test]
fn zhijian_equips_ally_and_draws() {
    let mgr = manager(950);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut equip) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_55");
        set_general(st, target, "sgs_hero_2");
        equip = give(st, cur, "qinggang_jian");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("skill:zhijian:{equip}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target]
            .equips
            .get(&EquipSlot::Weapon)
            .cloned()
            .flatten(),
        Some(equip)
    );
    assert_eq!(st.seats[cur].hand.len(), 1, "直谏后摸 1 张");
}

#[test]
fn guzheng_returns_discard_to_owner() {
    let mgr = manager(951);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut discarder, mut cards) = (0, 0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        discarder = st.turn;
        owner = st.seats_from_left_of(discarder)[0];
        set_general(st, owner, "sgs_hero_55");
        set_general(st, discarder, "sgs_hero_2");
        st.seats[discarder].hp = 2;
        for _ in 0..5 {
            cards.push(give(st, discarder, "sha"));
        }
        let pid_d = pid(st, discarder);
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!(
                    "{pid_d}|discard=discard:{},{},{}",
                    cards[0], cards[1], cards[2]
                ),
                format!("{}|choice=choose:guzheng", pid(st, owner)),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[discarder].hand.len(), 3, "弃 3 得 1 = 3");
    assert_eq!(
        st.seats[owner].hand.len(),
        4,
        "固政获得剩余 2 张；随后轮到其回合摸 2 张"
    );
    let _ = owner;
}

#[test]
fn guzheng_can_decline() {
    let mgr = manager(952);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut discarder, mut cards) = (0, 0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        discarder = st.turn;
        owner = st.seats_from_left_of(discarder)[0];
        set_general(st, owner, "sgs_hero_55");
        set_general(st, discarder, "sgs_hero_2");
        st.seats[discarder].hp = 2;
        for _ in 0..5 {
            cards.push(give(st, discarder, "sha"));
        }
        let pid_d = pid(st, discarder);
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!(
                    "{pid_d}|discard=discard:{},{},{}",
                    cards[0], cards[1], cards[2]
                ),
                format!("{}|choice=pass", pid(st, owner)),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[discarder].hand.len(), 2, "固政放弃：弃 3 剩 2");
    assert_eq!(
        st.seats[owner].hand.len(),
        2,
        "放弃固政；随后轮到其回合摸 2 张"
    );
}

// ---------------------------------------------------------------------------
// 左慈 · 化身 / 新生
// ---------------------------------------------------------------------------

#[test]
fn huashen_initializes_two_generals_and_one_skill() {
    let mgr = manager(960);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_56");
        crate::resolve_mountain::init_huashen(st, cur);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].huashen.len(), 2);
    assert!(st.seats[cur].huashen_active.is_some());
    assert_eq!(st.seats[cur].extra_skills.len(), 1);
}

#[test]
fn huashen_can_swap_active_general() {
    let mgr = manager(961);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut old_active = None;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_56");
        crate::resolve_mountain::init_huashen(st, cur);
        old_active = st.seats[cur].huashen_active.clone();
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:huashen")]);
    });
    let st = state_of(&mgr, &room);
    assert_ne!(st.seats[cur].huashen_active, old_active);
    assert_eq!(st.seats[cur].extra_skills.len(), 1);
}

#[test]
fn xinsheng_adds_new_huashen_after_damage() {
    let mgr = manager(962);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_56");
        crate::resolve_mountain::init_huashen(st, cur);
        crate::resolve::apply_damage(st, cur, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].huashen.len(), 3, "新生：获得一张新化身牌");
}

// ---------------------------------------------------------------------------
// 蔡文姬 · 悲歌 / 断肠
// ---------------------------------------------------------------------------

#[test]
fn beige_heart_heals_target() {
    let mgr = manager(970);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut victim, mut source, mut card, mut sha) =
        (0, 0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        victim = st.seats_from_left_of(owner)[0];
        source = st.seats_from_left_of(owner)[1];
        set_general(st, owner, "sgs_hero_57");
        set_general(st, victim, "sgs_hero_2");
        set_general(st, source, "sgs_hero_15");
        card = give(st, owner, "shan");
        sha = free_card_where(st, |c| c.def == "sha");
        st.seats[victim].hp = 3;
        top_card(st, |c| c.suit == Suit::Heart);
        let pid_o = pid(st, owner);
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
        drive_state_inject(st, &mut vec![format!("{pid_o}|choice=choose:{card}")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 3, "悲歌红桃：回复 1 点");
    assert!(st.history.iter().any(|e| e.text.contains("悲歌")));
}

#[test]
fn beige_spade_flips_damage_source() {
    let mgr = manager(971);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut victim, mut source, mut card, mut sha) =
        (0, 0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        victim = st.seats_from_left_of(owner)[0];
        source = st.seats_from_left_of(owner)[1];
        set_general(st, owner, "sgs_hero_57");
        set_general(st, victim, "sgs_hero_2");
        set_general(st, source, "sgs_hero_15");
        card = give(st, owner, "shan");
        sha = free_card_where(st, |c| c.def == "sha");
        st.seats[victim].hp = 4;
        top_card(st, |c| c.suit == Suit::Spade);
        let pid_o = pid(st, owner);
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
        drive_state_inject(st, &mut vec![format!("{pid_o}|choice=choose:{card}")]);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[source].flipped, "悲歌黑桃：伤害来源翻面");
}

#[test]
fn duanchang_killer_loses_skills() {
    let mgr = manager(972);
    let room = start_and_pick(&mgr, 5);
    let (mut victim, mut source) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        victim = st.turn;
        source = st.seats_from_left_of(victim)[0];
        set_general(st, victim, "sgs_hero_57");
        set_general(st, source, "sgs_hero_2");
        st.seats[victim].hp = 1;
        let pid_s = pid(st, source);
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(st, &mut Vec::new());
        let _ = pid_s;
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[victim].alive);
    assert!(st.seats[source].flags.contains_key("skills_lost"));
    assert!(!crate::skills::has(&st, source, "wusheng"));
}
