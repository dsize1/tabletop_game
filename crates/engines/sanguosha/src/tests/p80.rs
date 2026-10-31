//! P4.4.2 一将成名用例（一）：曹植 / 高顺 / 陈宫 / 凌统。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, manager, state_of, take_card};
use crate::skills;
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

/// 取一张满足条件的牌放入手牌（从其它区域取出）。
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
// 曹植 · 落英 / 酒诗
// ---------------------------------------------------------------------------

#[test]
fn luoying_gains_club_card_others_discard() {
    let mgr = manager(1040);
    let room = start_and_pick(&mgr, 5);
    let (mut turn_seat, mut caozhi, mut caozhi_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        turn_seat = st.turn;
        set_general(st, turn_seat, "sgs_hero_1");
        caozhi = other_seats(st, turn_seat)[0];
        set_general(st, caozhi, "sgs_hero_151");
        for _ in 0..2 {
            give_card(st, turn_seat, |c| c.suit == Suit::Club);
        }
        st.seats[turn_seat].max_hp = 1;
        st.seats[turn_seat].hp = 1;
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        caozhi_pid = st.seats[caozhi].player_id.clone();
        let mut inject = vec![format!("{caozhi_pid}=choose:luoying")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[caozhi]
            .hand
            .iter()
            .any(|c| st.cards.get(c).is_some_and(|i| i.suit == Suit::Club)),
        "落英：获得其他角色弃置的梅花牌"
    );
    assert!(st.history.iter().any(|e| e.text.contains("落英")));
}

#[test]
fn luoying_ignores_own_club_discard() {
    let mgr = manager(1041);
    let room = start_and_pick(&mgr, 5);
    let mut caozhi = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caozhi = st.turn;
        set_general(st, caozhi, "sgs_hero_151");
        for _ in 0..2 {
            give_card(st, caozhi, |c| c.suit == Suit::Club);
        }
        st.seats[caozhi].max_hp = 1;
        st.seats[caozhi].hp = 1;
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(
        !st.history.iter().any(|e| e.text.contains("落英")),
        "落英：不获得自己弃置的梅花牌"
    );
    assert!(st.seats[caozhi].hand.len() <= 1);
}

#[test]
fn jiushi_flips_and_counts_as_jiu() {
    let mgr = manager(1042);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_151");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        crate::turn::execute(st, &cur_pid, "skill:jiushi").unwrap();
        let mut inject = vec![format!("{cur_pid}=choose:jiushi")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flipped, "酒诗：翻至背面");
    assert_eq!(st.counter("jiu"), 1, "酒诗：视为使用【酒】");
}

#[test]
fn jiushi_flips_back_after_damage() {
    let mgr = manager(1043);
    let room = start_and_pick(&mgr, 5);
    let (mut caozhi, mut other) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caozhi = st.turn;
        set_general(st, caozhi, "sgs_hero_151");
        other = other_seats(st, caozhi)[0];
        set_general(st, other, "sgs_hero_1");
        st.seats[caozhi].flipped = true;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        crate::resolve::apply_damage(
            st,
            caozhi,
            1,
            Some(other),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[caozhi].flipped, "酒诗：受伤后翻回正面");
}

// ---------------------------------------------------------------------------
// 高顺 · 陷阵 / 禁酒
// ---------------------------------------------------------------------------

fn schedule_pindian(st: &mut SgsState, initiator: usize, target: usize, inject: &mut Vec<String>) {
    let (a_pid, b_pid) = (
        st.seats[initiator].player_id.clone(),
        st.seats[target].player_id.clone(),
    );
    let a_card = st.seats[initiator].hand[0].clone();
    let b_card = st.seats[target].hand[0].clone();
    inject.push(format!("{a_pid}=choose:{a_card}"));
    inject.push(format!("{b_pid}=choose:{b_card}"));
    crate::resolve::push(
        st,
        crate::resolve::Frame::XianZhen(crate::resolve_yijiang::XianZhenFrame {
            actor: initiator,
            target: Some(target),
            stage: crate::resolve_yijiang::XianZhenStage::WaitPindian,
        }),
    );
    crate::resolve_fire::start_pindian(st, initiator, target, serde_json::json!({}));
}

#[test]
fn xianzhen_win_grants_bonuses() {
    let mgr = manager(1044);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_152");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_card(st, cur, |c| c.rank == 13);
        give_card(st, target, |c| c.rank == 1);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        let mut inject = Vec::new();
        schedule_pindian(st, cur, target, &mut inject);
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("xz_win"), 1, "陷阵：拼点赢");
    assert_eq!(
        crate::resolve_yijiang::xianzhen_win_target(&st),
        Some(target)
    );
    assert!(!skills::sha_blocked(&st, cur), "陷阵赢：不封锁【杀】");
}

#[test]
fn xianzhen_lose_blocks_sha_and_hand_limit() {
    let mgr = manager(1045);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_152");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_card(st, cur, |c| c.rank == 1);
        give_card(st, target, |c| c.rank == 13);
        give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        let mut inject = Vec::new();
        schedule_pindian(st, cur, target, &mut inject);
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("xz_lose"), 1, "陷阵：没赢");
    assert!(
        skills::sha_blocked(&st, cur),
        "陷阵没赢：本回合不能使用【杀】"
    );
    assert!(
        skills::hand_limit(&st, cur) > st.seats[cur].hp as usize,
        "陷阵没赢：手牌上限额外计入【杀】"
    );
}

#[test]
fn jinjiu_turns_jiu_into_sha() {
    let mgr = manager(1046);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut jiu, mut cur_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_152");
        let target = crate::turn::sha_targets(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        jiu = give_def(st, cur, "jiu");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert!(
        skills::conversion_for(&st, cur, &jiu, "sha").is_some(),
        "禁酒：手牌【酒】视为【杀】"
    );
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter().any(|a| a.id == format!("skill:jinjiu:{jiu}")),
        "禁酒：出牌阶段可当【杀】使用"
    );
}

// ---------------------------------------------------------------------------
// 陈宫 · 明策 / 智迟
// ---------------------------------------------------------------------------

#[test]
fn mingce_gives_card_and_target_chooses_sha() {
    let mgr = manager(1047);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut victim, mut card) = (0, 0, 0, String::new());
    let mut pids = (String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_153");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        victim = other_seats(st, cur)
            .into_iter()
            .find(|i| *i != target && st.in_attack_range(target, *i))
            .expect("目标攻击范围内应有第三人");
        set_general(st, victim, "sgs_hero_2");
        card = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pids = (
            st.seats[cur].player_id.clone(),
            st.seats[target].player_id.clone(),
            st.seats[victim].player_id.clone(),
        );
    });
    let (cur_pid, target_pid, victim_pid) = pids;
    mgr.execute_action(&room, &cur_pid, "skill:mingce").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:mingce:sha")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("明策")), "明策：已交牌");
    assert_eq!(st.seats[target].hand.len(), 1, "明策：目标获得交出的【杀】");
    let _ = (cur, target, victim);
}

#[test]
fn mingce_target_can_draw_instead() {
    let mgr = manager(1048);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut card) = (0, 0, String::new());
    let mut pids = (String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_153");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        card = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pids = (
            st.seats[cur].player_id.clone(),
            st.seats[target].player_id.clone(),
        );
    });
    let (cur_pid, target_pid) = pids;
    mgr.execute_action(&room, &cur_pid, "skill:mingce").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:mingce:draw")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        2,
        "明策：目标选择摸 1（交牌 1 + 摸牌 1）"
    );
}

#[test]
fn zhichi_blocks_sha_outside_turn() {
    let mgr = manager(1049);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut chengong, mut sha, mut att_pid, mut cg_pid, mut hp) =
        (0, 0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        chengong = crate::turn::sha_targets(st, attacker)[0];
        set_general(st, chengong, "sgs_hero_153");
        sha = give_def(st, attacker, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        // 回合外受伤 → 智迟生效。
        crate::resolve::apply_damage(
            st,
            chengong,
            1,
            Some(attacker),
            crate::resolve::DamageType::Normal,
            None,
        );
        st.seats[chengong].hp = st.seats[chengong].max_hp;
        att_pid = st.seats[attacker].player_id.clone();
        cg_pid = st.seats[chengong].player_id.clone();
        hp = st.seats[chengong].hp;
    });
    mgr.execute_action(&room, &att_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &att_pid, &format!("target:{cg_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[chengong].hp, hp, "智迟：【杀】无效");
    assert!(st.history.iter().any(|e| e.text.contains("智迟")));
}

// ---------------------------------------------------------------------------
// 凌统 · 旋风
// ---------------------------------------------------------------------------

#[test]
fn xuanfeng_triggers_after_discard_phase() {
    let mgr = manager(1050);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_155");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "shan");
        give_def(st, target, "sha");
        for _ in 0..4 {
            give_def(st, cur, "sha");
        }
        st.seats[cur].max_hp = 2;
        st.seats[cur].hp = 2;
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}=target:{target_pid}"),
            format!("{cur_pid}=target:{target_pid}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 0, "旋风：弃置目标至多两张牌");
    assert!(st.history.iter().any(|e| e.text.contains("旋风")));
}

#[test]
fn xuanfeng_not_triggered_by_single_discard() {
    let mgr = manager(1052);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_155");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
            give_def(st, seat, "shan");
        }
        for _ in 0..2 {
            give_def(st, cur, "sha");
        }
        st.seats[cur].max_hp = 1;
        st.seats[cur].hp = 1; // 只弃 1 张
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(
        !st.history.iter().any(|e| e.text.contains("旋风")),
        "旋风：弃牌阶段未满 2 张不触发"
    );
    let _ = cur;
}

#[test]
fn xuanfeng_triggers_on_losing_equip() {
    let mgr = manager(1051);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_155");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "shan");
        equip_card(st, cur, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        // 模拟「失去装备区的牌」（被拆/换装）。
        let lost = st.seats[cur].equips.insert(EquipSlot::Weapon, None).flatten();
        if let Some(card) = lost {
            crate::turn::to_discard_card(st, &card);
        }
        crate::skills::xiaoji_on_lose_equip(st, cur);
        let mut inject = vec![format!("{cur_pid}=target:{target_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 0, "旋风：失去装备后弃置目标一张牌");
    assert!(st.history.iter().any(|e| e.text.contains("旋风")));
}
