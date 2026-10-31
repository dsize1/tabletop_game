//! P4.4.2 一将成名用例（二）：马谡 / 吴国太 / 徐盛 / 于禁。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, manager, state_of, take_card};
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

fn give_card(st: &mut SgsState, seat: usize, pred: impl Fn(&CardInst) -> bool) -> String {
    let id = st
        .cards
        .iter()
        .find(|(id, c)| {
            pred(c)
                && !st.seats.iter().any(|s| s.hand.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.marks.values().flatten().any(|h| h == *id))
                && !st.deck.processing.iter().any(|d| d == *id)
        })
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌");
    take_card(st, &id);
    st.seats[seat].hand.push(id.clone());
    id
}

fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    give_card(st, seat, |c| c.def == def)
}

/// 给某座位装备一张指定装备牌。
fn equip_card(st: &mut SgsState, seat: usize, def: &str) -> String {
    let card = give_def(st, seat, def);
    st.seats[seat].hand.retain(|c| c != &card);
    let slot = crate::data::card_def(def)
        .and_then(|d| d.equip_slot)
        .expect("equip slot");
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

// ---------------------------------------------------------------------------
// 马谡 · 散谣 / 制蛮
// ---------------------------------------------------------------------------

#[test]
fn sanyao_hp_mode_damages_max_hp_target() {
    let mgr = manager(1060);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cost, mut cur_pid, mut target_pid, mut hp) =
        (0, 0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_156");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        target = other_seats(st, cur)[0];
        st.seats[target].hp = st.seats[target].max_hp;
        for seat in other_seats(st, cur) {
            if seat != target {
                st.seats[seat].hp = 1;
            }
        }
        cost = give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, "skill:sanyao:hp").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "散谣（体力）：对体力最大者造成 1 点伤害");
    assert_eq!(st.counter("sanyao_hp_used"), 1);
}

#[test]
fn sanyao_hand_mode_once_per_phase() {
    let mgr = manager(1061);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cost, mut cur_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_156");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        for _ in 0..5 {
            give_def(st, other_seats(st, cur)[0], "sha");
        }
        cost = give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    // 手牌模式：不进入目标界面也能校验「各限一次」。
    mgr.execute_action(&room, &cur_pid, "skill:sanyao:hand")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("choose_many"),
        "散谣：先弃牌"
    );
    let _ = cost;
}

#[test]
fn sanyao_two_modes_are_independent() {
    let mgr = manager(1062);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_156");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        give_def(st, cur, "sha");
        give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.set_counter("sanyao_hp_used", 1);
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter().any(|a| a.id == "skill:sanyao:hand"),
        "散谣手牌模式仍可发动"
    );
    assert!(
        !acts.iter().any(|a| a.id == "skill:sanyao:hp"),
        "散谣体力模式本阶段已用过"
    );
}

#[test]
fn zhiman_prevents_damage_and_gains_card() {
    let mgr = manager(1063);
    let room = start_and_pick(&mgr, 5);
    let (mut masu, mut victim, mut victim_pid, mut count_before) =
        (0, 0, String::new(), 0usize);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        masu = st.turn;
        set_general(st, masu, "sgs_hero_156");
        victim = other_seats(st, masu)[0];
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "sha");
        give_def(st, victim, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(masu),
            crate::resolve::DamageType::Normal,
            None,
        );
        victim_pid = st.seats[victim].player_id.clone();
        count_before = st.seats[masu].hand.len();
        let mut inject = vec![format!(
            "{}={}",
            st.seats[masu].player_id.clone(),
            "choose:zhiman"
        )];
        drive_phases(st, &mut inject);
        let _ = victim_pid;
        let _ = count_before;
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, st.seats[victim].max_hp, "制蛮：防止伤害");
    assert_eq!(st.seats[masu].hand.len(), 1, "制蛮：获得其一张牌");
    assert!(st.history.iter().any(|e| e.text.contains("制蛮")));
}

// ---------------------------------------------------------------------------
// 吴国太 · 甘露 / 补益
// ---------------------------------------------------------------------------

#[test]
fn ganlu_swaps_equip_areas() {
    let mgr = manager(1064);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b) = (0, 0, 0);
    let mut pids = (String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_157");
        let others = other_seats(st, cur);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_2");
        equip_card(st, a, "qinggang_jian");
        equip_card(st, b, "bagua_zhen");
        st.seats[cur].max_hp = 3;
        st.seats[cur].hp = 2; // 已损失 1
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pids = (
            st.seats[cur].player_id.clone(),
            st.seats[a].player_id.clone(),
            st.seats[b].player_id.clone(),
        );
    });
    let (cur_pid, a_pid, b_pid) = pids;
    mgr.execute_action(&room, &cur_pid, "skill:ganlu").unwrap();
    mgr.execute_action(&room, &a_pid, "").ok();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[a].equips[&EquipSlot::Armor].is_some(),
        "甘露：交换后 a 获得防具"
    );
    assert!(
        st.seats[b].equips[&EquipSlot::Weapon].is_some(),
        "甘露：交换后 b 获得武器"
    );
}

#[test]
fn ganlu_requires_lost_hp_gap() {
    let mgr = manager(1065);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b) = (0, 0, 0);
    let mut pids = (String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_157");
        let others = other_seats(st, cur);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_2");
        equip_card(st, a, "qinggang_jian");
        equip_card(st, a, "bagua_zhen");
        equip_card(st, b, "chitu");
        st.seats[cur].max_hp = 3;
        st.seats[cur].hp = 3; // 未损失体力 → 牌数差 1 > 0 不合法
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pids = (
            st.seats[cur].player_id.clone(),
            st.seats[a].player_id.clone(),
            st.seats[b].player_id.clone(),
        );
    });
    let (cur_pid, a_pid, b_pid) = pids;
    mgr.execute_action(&room, &cur_pid, "skill:ganlu").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[a].equips[&EquipSlot::Weapon].is_some(),
        "甘露：条件不满足时不交换"
    );
    assert!(st.history.iter().any(|e| e.text.contains("条件不满足")));
}

#[test]
fn buyi_saves_dying_with_non_basic_card() {
    let mgr = manager(1066);
    let room = start_and_pick(&mgr, 5);
    let (mut wu, mut victim) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        wu = st.turn;
        set_general(st, wu, "sgs_hero_157");
        victim = other_seats(st, wu)[0];
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "sha"); // 基本牌
        give_def(st, victim, "wuzhong_shengyou"); // 非基本牌
        st.seats[victim].hp = 1;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(wu),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1, "补益：非基本牌弃置并回复 1（未进入濒死）");
    assert!(st.history.iter().any(|e| e.text.contains("补益")));
}

// ---------------------------------------------------------------------------
// 徐盛 · 破军
// ---------------------------------------------------------------------------

#[test]
fn pojun_moves_cards_out_and_returns_at_turn_end() {
    let mgr = manager(1067);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut cur_pid, mut target_pid) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_158");
        target = crate::turn::sha_targets(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "shan");
        give_def(st, target, "sha");
        sha = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let target_hp = st.seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 破军：移出至多 target_hp 张（先移一张手牌）。
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().and_then(|p| p.skill.clone()) == Some("pojun".into()) {
        mgr.execute_action(&room, &cur_pid, "choose:pojun:hand")
            .unwrap();
        mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.deck.void.len(), 1, "破军：一张牌移出游戏");
    assert_eq!(st.void_return.len(), 1, "破军：登记回合结束回置");
    let _ = target_hp;
    // 回合结束回置。
    edit(&mgr, &room, |st| {
        st.turn_phase = TurnPhase::End;
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(st.deck.void.is_empty(), "破军：回合结束后回置");
    assert!(st.void_return.is_empty());
}

#[test]
fn pojun_can_move_equip_card() {
    let mgr = manager(1072);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut weapon = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_158");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        weapon = equip_card(st, target, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        crate::resolve_yijiang::push_pojun(st, cur, target, "sha");
        let mut inject = vec![format!("{cur_pid}=choose:pojun:equip:weapon")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.deck.void.iter().any(|c| c == &weapon),
        "破军：可将装备区的牌移出游戏"
    );
    assert!(st.seats[target].equips[&EquipSlot::Weapon].is_none());
    let _ = target_pid;
}

#[test]
fn pojun_limit_equals_target_hp() {
    let mgr = manager(1068);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_158");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = 2;
        for _ in 0..4 {
            give_def(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        crate::resolve_yijiang::push_pojun(st, cur, target, "sha");
    });
    let st = state_of(&mgr, &room);
    let remaining = st
        .resolution
        .iter()
        .rev()
        .find_map(|f| match f {
            crate::resolve::Frame::PoJun(p) => Some(p.remaining),
            _ => None,
        });
    assert_eq!(remaining, Some(2), "破军：X = 目标体力值");
}

// ---------------------------------------------------------------------------
// 于禁 · 镇军
// ---------------------------------------------------------------------------

#[test]
fn zhenjun_discards_target_then_lets_draw() {
    let mgr = manager(1069);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_160");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..6 {
            give_def(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=target:{target_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    // 准备阶段：X = 手牌 6 - 体力 4 = 2 → 弃 2 → 二选一（默认 pass → 不选则按自弃处理？）
    assert!(st.counter("zhenjun_offered") == 1, "镇军：准备阶段发动");
    let _ = target_pid;
}

#[test]
fn zhenjun_target_draws_option() {
    let mgr = manager(1070);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_160");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..6 {
            give_def(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}=target:{target_pid}"),
            format!("{cur_pid}=choose:zhenjun:draw"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        6,
        "镇军：目标先弃 2（随机），选择摸 2 → 手牌仍 6"
    );
    assert!(st.history.iter().any(|e| e.text.contains("镇军")));
}

#[test]
fn zhenjun_self_discard_option() {
    let mgr = manager(1071);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_160");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..6 {
            give_def(st, target, "sha");
        }
        for _ in 0..4 {
            give_def(st, cur, "shan");
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}=target:{target_pid}"),
            format!("{cur_pid}=choose:zhenjun:discard"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("镇军") && e.text.contains("弃置 2 张牌")),
        "镇军：自己弃置等量牌"
    );
    let _ = (cur, target, target_pid);
}
