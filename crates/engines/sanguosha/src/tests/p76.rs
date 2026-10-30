//! P4.4.1 SP-1 用例（一）：袁术 / 王基 / 严颜 / 王平。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, give, manager, state_of};
use crate::skills;
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

// ---------------------------------------------------------------------------
// 袁术 · 庸肆 / 伪帝
// ---------------------------------------------------------------------------

#[test]
fn yongsi_draws_extra_by_kingdom_count() {
    let mgr = manager(960);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut expected = 0u32;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_100");
        let gens = ["sgs_hero_1", "sgs_hero_15", "sgs_hero_8", "sgs_hero_17"];
        for (k, seat) in other_seats(st, cur).into_iter().enumerate() {
            set_general(st, seat, gens[k % gens.len()]);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        expected = 2 + skills::kingdom_count(st);
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        expected as usize,
        "庸肆：摸牌阶段摸 2 + 势力数"
    );
    assert!(st.history.iter().any(|e| e.text.contains("庸肆")));
}

#[test]
fn yongsi_discard_phase_discards_extra() {
    let mgr = manager(961);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_100");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        for _ in 0..8 {
            give(st, cur, "sha");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("庸肆")),
        "庸肆：弃牌阶段开始时先弃 X 张"
    );
    assert!(st.seats[cur].hand.len() < 8);
}

#[test]
fn weidi_borrows_lords_lord_skill_only_when_not_lord() {
    let mgr = manager(962);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut yuanshu) = (0, 0);
    edit(&mgr, &room, |st| {
        lord = st.lord_index().unwrap();
        set_general(st, lord, "sgs_hero_1"); // 刘备：激将（主公技）
        yuanshu = st.alive_indices().into_iter().find(|i| *i != lord).unwrap();
        set_general(st, yuanshu, "sgs_hero_100");
    });
    let st = state_of(&mgr, &room);
    assert!(
        skills::has(&st, yuanshu, "jijiang"),
        "伪帝：视为拥有主公的主公技"
    );
    // 袁术自己成为主公时，不借用（主公技只属于真实主公）。
    edit(&mgr, &room, |st| {
        st.seats[yuanshu].identity = Identity::Lord;
        st.seats[lord].identity = Identity::Loyalist;
    });
    let st = state_of(&mgr, &room);
    assert!(!skills::has(&st, yuanshu, "jijiang"), "袁术为主公时不借用");
}

// ---------------------------------------------------------------------------
// 王基 · 奇制 / 进趋
// ---------------------------------------------------------------------------

#[test]
fn qizhi_discards_and_target_draws() {
    let mgr = manager(963);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_362");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "sha");
        give(st, target, "shan");
        sha = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:qizhi")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("qizhi_count"), 1, "奇制计入本回合次数");
    assert!(st.history.iter().any(|e| e.text.contains("奇制")));
}

#[test]
fn jinzou_discards_down_to_qizhi_count() {
    let mgr = manager(964);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cards, mut cur_pid) = (0, Vec::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_362");
        for _ in 0..5 {
            cards.push(give(st, cur, "sha"));
        }
        st.turn_counters.clear();
        st.set_counter("qizhi_count", 2);
        st.turn_phase = TurnPhase::End;
        cur_pid = st.seats[cur].player_id.clone();
        // 摸 2 → 7 张；弃至 2 张 → 需弃 5 张。
        let _ = &cards;
        let mut inject = vec![format!("{cur_pid}=choose:jinzou")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 2, "进趋：弃至 X=2 张");
    assert!(st.history.iter().any(|e| e.text.contains("进趋")));
}

#[test]
fn jinzou_with_zero_qizhi_discards_all() {
    let mgr = manager(965);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cards, mut cur_pid) = (0, Vec::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_362");
        for _ in 0..3 {
            cards.push(give(st, cur, "sha"));
        }
        st.turn_counters.clear();
        st.turn_phase = TurnPhase::End;
        cur_pid = st.seats[cur].player_id.clone();
        let _ = &cards;
        let mut inject = vec![format!("{cur_pid}=choose:jinzou")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 0, "X=0 时弃光手牌");
}

// ---------------------------------------------------------------------------
// 严颜 · 拒战（转换技）
// ---------------------------------------------------------------------------

#[test]
fn juzhan_yang_both_draw_and_deny() {
    let mgr = manager(966);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut yan, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        yan = other_seats(st, attacker)[0];
        set_general(st, yan, "sgs_hero_400");
        sha = give(st, attacker, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[yan].flags.remove("yj:juzhan");
    });
    let st = state_of(&mgr, &room);
    let (att_pid, yan_pid) = (pid(&st, attacker), pid(&st, yan));
    mgr.execute_action(&room, &att_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &att_pid, &format!("target:{yan_pid}"))
        .unwrap();
    mgr.execute_action(&room, &yan_pid, "choose:juzhan").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[yan].hand.len(), 1, "拒战（阳）：严颜摸 1");
    assert_eq!(st.seats[attacker].hand.len(), 1, "拒战（阳）：使用者摸 1");
    assert!(skills::deny_target(&st, attacker, yan), "使用者本回合不能再指定严颜");
    assert!(skills::yj_yin(&st, yan, "juzhan"), "阳面发动后转为阴面");
}

#[test]
fn juzhan_yin_gains_card_and_denies_target() {
    let mgr = manager(967);
    let room = start_and_pick(&mgr, 5);
    let (mut yan, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yan = st.turn;
        set_general(st, yan, "sgs_hero_400");
        target = other_seats(st, yan)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "shan");
        give(st, target, "sha");
        sha = give(st, yan, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[yan].flags.insert("yj:juzhan".into(), true);
    });
    let st = state_of(&mgr, &room);
    let (yan_pid, target_pid) = (pid(&st, yan), pid(&st, target));
    mgr.execute_action(&room, &yan_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &yan_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &yan_pid, "choose:juzhan").unwrap();
    mgr.execute_action(&room, &yan_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[yan].hand.len(), 1, "拒战（阴）：获得目标一张牌");
    assert!(skills::deny_target(&st, yan, target), "严颜本回合不能再对其使用牌");
    assert!(!skills::yj_yin(&st, yan, "juzhan"), "阴面发动后转回阳面");
}

#[test]
fn juzhan_deny_removes_target_from_sha_list() {
    let mgr = manager(968);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut yan, mut before, mut after) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        yan = crate::turn::sha_targets(st, attacker)[0];
        set_general(st, yan, "sgs_hero_400");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        before = crate::turn::sha_targets(st, attacker).len();
        skills::set_deny_target(st, attacker, yan);
        after = crate::turn::sha_targets(st, attacker).len();
    });
    assert!(before > 0);
    assert_eq!(after, before - 1, "拒战后该角色不能成为【杀】目标");
}

// ---------------------------------------------------------------------------
// 王平 · 飞军 / 兵略
// ---------------------------------------------------------------------------

#[test]
fn feijun_takes_card_and_binglue_draws() {
    let mgr = manager(969);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cost) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_401");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, cur, "shan");
        cost = give(st, cur, "sha");
        for _ in 0..4 {
            give(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:feijun").unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:feijun:take")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 4, "飞军：弃 1 得 1，兵略摸 2");
    assert_eq!(st.seats[target].hand.len(), 3, "目标交出一张手牌");
    assert_eq!(st.counter("feijun_used"), 1);
    assert!(st.history.iter().any(|e| e.text.contains("兵略")));
}

#[test]
fn feijun_makes_target_discard_equip() {
    let mgr = manager(970);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cost, mut weapon) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_401");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        cost = give(st, cur, "shan");
        // 目标装备两件（> 王平 0 件）
        weapon = give(st, target, "qinggang_jian");
        st.seats[target].hand.retain(|c| c != &weapon);
        st.seats[target]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon.clone()));
        let armor = give(st, target, "bagua_zhen");
        st.seats[target].hand.retain(|c| c != &armor);
        st.seats[target].equips.insert(EquipSlot::Armor, Some(armor));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:feijun").unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:feijun:equip")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.deck.discard.iter().any(|c| c == &weapon),
        "飞军：目标弃置装备区的牌"
    );
    assert!(st.seats[target].equips[&EquipSlot::Weapon].is_none());
}

#[test]
fn feijun_once_per_play_phase() {
    let mgr = manager(971);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_401");
        give(st, cur, "shan");
        let target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..4 {
            give(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.set_counter("feijun_used", 1);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let err = mgr
        .execute_action(&room, &cur_pid, "skill:feijun")
        .expect_err("飞军每阶段限一次");
    assert!(format!("{err:?}").contains("飞军"));
}
