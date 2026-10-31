//! P4.4.2 一将成名批次 Y5（8 将）：刘谌 / 夏侯氏 / 张嶷 / 孙休 / 朱治 / 全琮 / 公孙渊 / 郭图逢纪。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

fn equip_slot_card(st: &mut SgsState, seat: usize, slot: EquipSlot) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(slot))
    });
    take_card(st, &card);
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    super::support::give_def(st, seat, def)
}

// ---------------------------------------------------------------------------
// 刘谌 · 战绝 / 勤王
// ---------------------------------------------------------------------------

#[test]
fn zhanjue_all_hand_as_duel() {
    let mgr = manager(2700);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target_pid, mut card) =
        (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_335");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        set_general(st, others[0], "sgs_hero_1");
        target_pid = st.seats[others[0]].player_id.clone();
        card = give_card(st, seat, |c| c.def == "sha");
        give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:zhanjue").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 0, "战绝：所有手牌被使用");
    assert_eq!(st.effective_def_code(&card), Some("juedou"));
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("target"));
    let _ = target_pid;
}

#[test]
fn zhanjue_draws_after_damage() {
    let mgr = manager(2701);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_335");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:zhanjue").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("respond") {
        mgr.execute_action(&room, &target_pid, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(st.counter("zhanjue_draws") >= 1, "战绝：刘谌摸牌计数");
    assert!(!st.seats[target].hand.is_empty() || st.seats[seat].hand.len() >= 1);
    assert!(st.history.iter().any(|e| e.text.contains("战绝")));
}

#[test]
fn qinwang_responder_draws() {
    let mgr = manager(2702);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut ally, mut ally_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_335");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        ally = other_seats(st, seat)[0];
        set_general(st, ally, "sgs_hero_2"); // 关羽（蜀）
        ally_pid = st.seats[ally].player_id.clone();
        give_def(st, seat, "shan");
        give_def(st, ally, "sha");
        before = st.seats[ally].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:qinwang").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("requisition"));
    let sha = st.seats[ally]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "sha"))
        .cloned()
        .expect("sha");
    mgr.execute_action(&room, &ally_pid, &format!("respond:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("勤王")));
    assert!(st.seats[ally].hand.len() >= before, "勤王：响应者摸回一张");
}

// ---------------------------------------------------------------------------
// 夏侯氏 · 樵拾 / 燕语
// ---------------------------------------------------------------------------

#[test]
fn qiaoshi_equal_hands_both_draw() {
    let mgr = manager(2703);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut owner, mut owner_pid, mut cur_hand, mut owner_hand) =
        (0, 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        owner = other_seats(st, cur)[0];
        set_general(st, owner, "sgs_hero_336");
        owner_pid = st.seats[owner].player_id.clone();
        give_def(st, cur, "sha");
        give_def(st, owner, "sha");
        cur_hand = st.seats[cur].hand.len();
        owner_hand = st.seats[owner].hand.len();
    });
    run_phase(&mgr, &room, cur, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qiaoshi"), "樵拾：手牌相等时询问");
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].hand.len() > cur_hand);
    assert!(st.seats[owner].hand.len() > owner_hand);
}

#[test]
fn yanyu_recasts_sha() {
    let mgr = manager(2704);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut sha) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_336");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        sha = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("skill:yanyu:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("yanyu_count"), 1, "燕语：重铸计数");
    assert!(st.deck.discard.contains(&sha));
}

#[test]
fn yanyu_end_play_gift() {
    let mgr = manager(2705);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut male, mut male_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_336");
        stage_play(st, seat);
        st.set_counter("yanyu_count", 2);
        pid = st.seats[seat].player_id.clone();
        male = other_seats(st, seat)
            .into_iter()
            .find(|i| crate::skills::is_male(st, *i))
            .expect("male");
        set_general(st, male, "sgs_hero_1");
        male_pid = st.seats[male].player_id.clone();
        before = st.seats[male].hand.len();
    });
    mgr.execute_action(&room, &pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yanyu"), "燕语：阶段结束询问");
    mgr.execute_action(&room, &pid, &format!("target:{male_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("燕语：令") && e.text.contains("摸 2")),
        "燕语：令男性摸 2"
    );
    assert!(st.seats[male].hand.len() >= before + 2);
}

// ---------------------------------------------------------------------------
// 张嶷 · 怃戎 / 矢志
// ---------------------------------------------------------------------------

fn wurong_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    own_sha: bool,
    their_shan: bool,
) -> (usize, String, usize, String, String) {
    let (mut seat, mut pid, mut target, mut target_pid, mut own) =
        (0, String::new(), 0, String::new(), String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_337");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        own = if own_sha {
            give_def(st, seat, "sha")
        } else {
            give_def(st, seat, "tao")
        };
        if their_shan {
            give_def(st, target, "shan");
        } else {
            give_def(st, target, "sha");
        }
    });
    (seat, pid, target, target_pid, own)
}

#[test]
fn wurong_sha_vs_not_shan_damages() {
    let mgr = manager(2706);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, target, target_pid, own) = wurong_setup(&mgr, &room, true, false);
    assert!(crate::skills::has(&state_of(&mgr, &room), seat, "wurong"));
    let mut hp = 0;
    edit(&mgr, &room, |st| hp = st.seats[target].hp);
    mgr.execute_action(&room, &pid, "skill:wurong").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{own}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let their = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("their card");
    mgr.execute_action(&room, &target_pid, &their).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "怃戎：造成 1 点伤害");
    assert!(st.deck.discard.contains(&own), "怃戎：弃置展示的【杀】");
}

#[test]
fn wurong_nonsha_vs_shan_gains_card() {
    let mgr = manager(2707);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, target, target_pid, own) = wurong_setup(&mgr, &room, false, true);
    assert!(crate::skills::has(&state_of(&mgr, &room), seat, "wurong"));
    let mut target_hand = 0;
    edit(&mgr, &room, |st| target_hand = st.seats[target].hand.len());
    mgr.execute_action(&room, &pid, "skill:wurong").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{own}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let their = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("their card");
    mgr.execute_action(&room, &target_pid, &their).unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() < target_hand,
        "怃戎：获得其一张牌"
    );
    assert!(st.deck.discard.contains(&own));
}

#[test]
fn shizhi_shan_counts_as_sha() {
    let mgr = manager(2708);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut shan, mut conversion) = (0, String::new(), None);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_337");
        st.turn_counters.clear();
        st.seats[seat].hp = 1;
        shan = give_def(st, seat, "shan");
        conversion =
            crate::skills::conversion_for(st, seat, &shan, "sha").map(|(code, _)| code.to_string());
    });
    assert_eq!(
        conversion.as_deref(),
        Some("shizhi"),
        "矢志：体力 1 时【闪】当【杀】"
    );
}

// ---------------------------------------------------------------------------
// 孙休 · 宴诛 / 兴学 / 诏缚
// ---------------------------------------------------------------------------

#[test]
fn yanzhu_gives_equips_and_upgrades_xingxue() {
    let mgr = manager(2709);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut equip) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_338");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        equip = equip_slot_card(st, target, EquipSlot::Armor);
    });
    mgr.execute_action(&room, &pid, "skill:yanzhu").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.contains(&equip), "宴诛：获得其装备");
    assert!(
        !crate::skills::has(&st, seat, "yanzhu"),
        "宴诛：孙休失去宴诛"
    );
    assert!(st.seats[seat].flags.contains_key("xingxue_upgraded"));
}

#[test]
fn yanzhu_target_discards() {
    let mgr = manager(2710);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_338");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
        give_def(st, target, "shan");
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:yanzhu").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), before - 1, "宴诛：目标弃 1 张");
}

#[test]
fn xingxue_draws_and_tops() {
    let mgr = manager(2711);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_338");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("xingxue"), "兴学：结束阶段");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("兴学")));
    assert!(st
        .pending
        .as_ref()
        .is_some_and(|p| p.skill.as_deref() == Some("xingxue")));
}

// ---------------------------------------------------------------------------
// 朱治 · 安国
// ---------------------------------------------------------------------------

#[test]
fn anguo_hand_min_draws() {
    let mgr = manager(2712);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_339");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        target = others[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        for i in others.iter().skip(1) {
            set_general(st, *i, "sgs_hero_1");
            give_def(st, *i, "sha");
        }
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:anguo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("安国：") && e.text.contains("摸 1 张")),
        "安国：手牌最少摸一张"
    );
    assert!(st.seats[target].hand.len() >= before);
}

#[test]
fn anguo_hp_min_heals() {
    let mgr = manager(2713);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_339");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        target = others[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = 1;
        target_pid = st.seats[target].player_id.clone();
        for i in others.iter().skip(1) {
            set_general(st, *i, "sgs_hero_1");
            give_def(st, *i, "sha");
        }
    });
    mgr.execute_action(&room, &pid, "skill:anguo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 2, "安国：体力最低回复 1");
}

#[test]
fn anguo_equip_min_uses_equip() {
    let mgr = manager(2714);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_339");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        target = others[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "sha");
        give_def(st, target, "sha");
        target_pid = st.seats[target].player_id.clone();
        for i in others.iter().skip(1) {
            set_general(st, *i, "sgs_hero_1");
            give_def(st, *i, "sha");
            equip_slot_card(st, *i, EquipSlot::Armor);
        }
        let equip = give_card(st, target, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(EquipSlot::Weapon))
        });
        let _ = equip;
    });
    mgr.execute_action(&room, &pid, "skill:anguo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].equips.values().any(|c| c.is_some()),
        "安国：装备最少随机使用装备"
    );
}

// ---------------------------------------------------------------------------
// 全琮 · 邀名
// ---------------------------------------------------------------------------

fn yaoming_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    owner_hand: usize,
    target_hand: usize,
) -> (usize, String, usize, String) {
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_340");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        for _ in 0..owner_hand {
            give_def(st, seat, "shan");
        }
        for _ in 0..target_hand {
            give_def(st, target, "shan");
        }
    });
    (seat, pid, target, target_pid)
}

#[test]
fn yaoming_discards_from_more() {
    let mgr = manager(2715);
    let room = start_and_pick(&mgr, 5);
    let (seat, _pid, target, _target_pid) = yaoming_setup(&mgr, &room, 1, 3);
    let mut owner_pid = String::new();
    edit(&mgr, &room, |st| {
        owner_pid = st.seats[seat].player_id.clone();
        damage_and_advance(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yaoming"), "邀名：造成伤害后询问");
    mgr.execute_action(
        &room,
        &owner_pid,
        &format!("discard:{}", st.seats[target].player_id),
    )
    .unwrap();
    let st2 = state_of(&mgr, &room);
    assert_eq!(st2.seats[target].hand.len(), 2, "邀名：弃置其一张手牌");
}

#[test]
fn yaoming_draw_for_less() {
    let mgr = manager(2716);
    let room = start_and_pick(&mgr, 5);
    let (seat, _pid, target, _target_pid) = yaoming_setup(&mgr, &room, 3, 0);
    let mut owner_pid = String::new();
    edit(&mgr, &room, |st| {
        owner_pid = st.seats[seat].player_id.clone();
        damage_and_advance(
            st,
            seat,
            1,
            Some(target),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yaoming"), "邀名：受到伤害后询问");
    mgr.execute_action(
        &room,
        &owner_pid,
        &format!("draw:{}", st.seats[target].player_id),
    )
    .unwrap();
    let st2 = state_of(&mgr, &room);
    assert_eq!(st2.seats[target].hand.len(), 1, "邀名：令手牌更少者摸一张");
}

#[test]
fn yaoming_once_per_turn() {
    let mgr = manager(2717);
    let room = start_and_pick(&mgr, 5);
    let (seat, _pid, target, _target_pid) = yaoming_setup(&mgr, &room, 1, 3);
    let mut has_pending = true;
    edit(&mgr, &room, |st| {
        st.set_counter("yaoming_used", 1);
        damage_and_advance(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
        has_pending = st.pending.as_ref().and_then(|p| p.skill.as_deref()) == Some("yaoming");
    });
    assert!(!has_pending, "邀名：每回合限一次");
}

// ---------------------------------------------------------------------------
// 公孙渊 · 怀异
// ---------------------------------------------------------------------------

#[test]
fn huaiyi_discards_color_and_gains() {
    let mgr = manager(2718);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_341");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_card(st, seat, |c| crate::skills::is_red(c.suit));
        give_card(st, target, |c| crate::skills::is_red(c.suit));
    });
    mgr.execute_action(&room, &pid, "skill:huaiyi").unwrap();
    mgr.execute_action(&room, &pid, "opt_red").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 0, "怀异：弃置所有红色手牌");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 1, "怀异：获得一张牌");
    assert_eq!(
        st.seats[seat].hp, st.seats[seat].max_hp,
        "怀异：仅 1 张不失体力"
    );
}

#[test]
fn huaiyi_two_cards_loses_hp() {
    let mgr = manager(2719);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_341");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        for _ in 0..2 {
            give_card(st, seat, |c| crate::skills::is_red(c.suit));
        }
        for _ in 0..2 {
            give_card(st, target, |c| !crate::skills::is_red(c.suit));
        }
        hp = st.seats[seat].hp;
    });
    mgr.execute_action(&room, &pid, "skill:huaiyi").unwrap();
    mgr.execute_action(&room, &pid, "opt_red").unwrap();
    for _ in 0..2 {
        mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
            .unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 2, "怀异：获得 2 张");
    assert_eq!(st.seats[seat].hp, hp - 1, "怀异：获得 ≥2 失去 1 点体力");
}

#[test]
fn huaiyi_gain_can_stop() {
    let mgr = manager(2720);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_341");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_card(st, seat, |c| crate::skills::is_red(c.suit));
        give_card(st, target, |c| !crate::skills::is_red(c.suit));
    });
    mgr.execute_action(&room, &pid, "skill:huaiyi").unwrap();
    mgr.execute_action(&room, &pid, "opt_red").unwrap();
    mgr.execute_action(&room, &pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hp, st.seats[seat].max_hp,
        "怀异：放弃获得不受罚"
    );
    let _ = target_pid;
}

// ---------------------------------------------------------------------------
// 郭图逢纪 · 急攻 / 饰非
// ---------------------------------------------------------------------------

#[test]
fn jigong_draws_and_sets_limit() {
    let mgr = manager(2721);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_342");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Play, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jigong"), "急攻：出牌阶段开始询问");
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 2, "急攻：摸 2 张");
    assert_eq!(
        crate::skills::hand_limit(&st, seat),
        0,
        "急攻：手牌上限=本阶段伤害值"
    );
}

#[test]
fn jigong_limit_tracks_damage() {
    let mgr = manager(2722);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_342");
        st.turn_counters.clear();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[seat].flags.insert("jigong".into(), true);
        damage_and_advance(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::hand_limit(&st, seat),
        1,
        "急攻：手牌上限随本阶段伤害更新"
    );
}

#[test]
fn shifei_provides_shan() {
    let mgr = manager(2723);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        owner = other_seats(st, user)
            .into_iter()
            .find(|i| st.in_attack_range(user, *i))
            .expect("in range");
        set_general(st, owner, "sgs_hero_342");
        owner_pid = st.seats[owner].player_id.clone();
        hp = st.seats[owner].hp;
        give_def(st, user, "sha");
        give_def(st, owner, "sha");
    });
    let st = state_of(&mgr, &room);
    let sha = st.seats[user]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "sha"))
        .cloned()
        .expect("sha");
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond")
    );
    assert!(
        pending_options_with(&st, "skill:shifei").len() == 1,
        "饰非：可用选项"
    );
    mgr.execute_action(&room, &owner_pid, "skill:shifei")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[owner].hp, hp, "饰非：视为出【闪】未受伤");
    assert!(st.history.iter().any(|e| e.text.contains("饰非")));
}
