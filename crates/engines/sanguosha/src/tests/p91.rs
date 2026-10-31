//! P4.4.2 一将成名批次 Y3（8 将）：王异 / 关兴张苞 / 曹真 / 韩浩史涣 / 陈群 / 吴懿 / 周仓 / 张松。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

fn equip_card(st: &mut SgsState, seat: usize) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(EquipSlot::Weapon))
    });
    take_card(st, &card);
    st.seats[seat]
        .equips
        .insert(EquipSlot::Weapon, Some(card.clone()));
    card
}

fn sha_card(st: &SgsState, red: Option<bool>) -> String {
    find_cards(
        st,
        |c| {
            matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
                && red.is_none_or(|r| crate::skills::is_red(c.suit) == r)
        },
        1,
    )
    .into_iter()
    .next()
    .expect("sha")
}

fn give_trick_for_pindi(st: &mut SgsState, seat: usize) {
    // 保证陈群有一张非基本牌（第二次品第费用）。
    give_card(st, seat, |c| {
        crate::data::card_def(&c.def)
            .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
    });
}

fn trick_card(st: &SgsState) -> String {
    find_cards(
        st,
        |c| {
            crate::data::card_def(&c.def)
                .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
        },
        1,
    )
    .into_iter()
    .next()
    .expect("trick")
}

// ---------------------------------------------------------------------------
// 王异 · 贞烈 / 秘计
// ---------------------------------------------------------------------------

#[test]
fn zhenlie_cancels_sha_and_discards_user_card() {
    let mgr = manager(2300);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut hp, mut user_hand) =
        (0, String::new(), 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        owner = other_seats(st, user)[0];
        set_general(st, owner, "sgs_hero_187");
        owner_pid = st.seats[owner].player_id.clone();
        hp = st.seats[owner].hp;
        give_def(st, user, "sha");
        give_def(st, user, "shan");
        user_hand = st.seats[user].hand.len();
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, None);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhenlie"),
        "贞烈：成为【杀】目标后询问"
    );
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[owner].hp, hp - 1, "贞烈：失去 1 点体力");
    assert_eq!(
        st.seats[user].hand.len(),
        user_hand - 2,
        "贞烈：弃置使用者一张牌"
    );
    assert!(!st.history.iter().any(|e| e.text.contains("受到")));
}

#[test]
fn zhenlie_pass_keeps_normal_response() {
    let mgr = manager(2301);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        owner = other_seats(st, user)[0];
        set_general(st, owner, "sgs_hero_187");
        owner_pid = st.seats[owner].player_id.clone();
        hp = st.seats[owner].hp;
        give_def(st, user, "sha");
        give_def(st, owner, "shan");
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, None);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    mgr.execute_action(&room, &owner_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[owner].hp, hp, "贞烈：不发动不失去体力");
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond")
    );
}

#[test]
fn miji_draws_lost_hp_and_gives_cards() {
    let mgr = manager(2302);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hand_before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_187");
        st.turn_counters.clear();
        st.seats[seat].hp = 1;
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hand_before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("miji"), "秘计：结束阶段亮牌");
    let lost = (st.seats[seat].max_hp - st.seats[seat].hp).max(0) as usize;
    assert_eq!(
        st.seats[seat].hand.len(),
        hand_before + lost,
        "秘计：摸已损失体力张"
    );
    let some = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("give card");
    mgr.execute_action(&room, &pid, &some).unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        hand_before + lost - 1,
        "秘计：交出 1 张"
    );
    assert!(!st.seats[target].hand.is_empty(), "秘计：目标获得牌");
}

// ---------------------------------------------------------------------------
// 关兴张苞 · 父魂
// ---------------------------------------------------------------------------

fn fuhen_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
) -> (usize, String, usize, String, Vec<String>) {
    let (mut seat, mut pid, mut target, mut target_pid, mut cards) =
        (0, String::new(), 0, String::new(), Vec::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_188");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        cards.push(give_def(st, seat, "shan"));
        cards.push(give_def(st, seat, "tao"));
    });
    (seat, pid, target, target_pid, cards)
}

#[test]
fn fuhen_two_cards_as_sha() {
    let mgr = manager(2303);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, _target_pid, cards) = fuhen_setup(&mgr, &room);
    assert!(crate::skills::has(&state_of(&mgr, &room), seat, "fuhen"));
    let action = format!("skill:fuhen:{},{}", cards[0], cards[1]);
    mgr.execute_action(&room, &pid, &action).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("target"));
    assert!(
        !st.seats[seat].hand.contains(&cards[1]),
        "父魂：第二张牌进弃牌堆"
    );
    assert!(
        st.deck.processing.contains(&cards[0]),
        "父魂：第一张牌进入处理区"
    );
}

#[test]
fn fuhen_grants_skills_after_damage() {
    let mgr = manager(2304);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, target_pid, cards) = fuhen_setup(&mgr, &room);
    assert!(crate::skills::has(&state_of(&mgr, &room), seat, "fuhen"));
    let action = format!("skill:fuhen:{},{}", cards[0], cards[1]);
    mgr.execute_action(&room, &pid, &action).unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    if pending_skill(&st) == Some("zhenlie") || st.pending.is_some() {
        // 目标无【闪】时直接结算；有【闪】时放弃响应。
        if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("respond") {
            mgr.execute_action(&room, &target_pid, "pass").unwrap();
        }
    }
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].extra_skills.iter().any(|c| c == "wusheng"),
        "父魂：获得武圣"
    );
    assert!(
        st.seats[seat].extra_skills.iter().any(|c| c == "paoxiao"),
        "父魂：获得咆哮"
    );
}

#[test]
fn fuhen_respond_options_available() {
    let mgr = manager(2305);
    let room = start_and_pick(&mgr, 5);
    let (seat, _pid, _target, _target_pid, cards) = fuhen_setup(&mgr, &room);
    assert!(crate::skills::has(&state_of(&mgr, &room), seat, "fuhen"));
    edit(&mgr, &room, |st| {
        let opts = crate::resolve_yijiang5::fuhen_respond_options(st, seat);
        assert!(
            opts.iter().any(|o| o.id.contains(&cards[0])),
            "父魂：可两张手牌当【杀】打出"
        );
    });
}

// ---------------------------------------------------------------------------
// 曹真 · 司敌
// ---------------------------------------------------------------------------

fn sidi_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
) -> (usize, String, usize, String, String) {
    let (mut cur, mut cur_pid, mut owner, mut owner_pid, mut cost) =
        (0, String::new(), 0, String::new(), String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        owner = other_seats(st, cur)[0];
        set_general(st, owner, "sgs_hero_321");
        owner_pid = st.seats[owner].player_id.clone();
        let weapon = equip_card(st, owner);
        // 与武器同色的非基本手牌作为司敌费用。
        let red = st
            .cards
            .get(&weapon)
            .is_some_and(|c| crate::skills::is_red(c.suit));
        cost = give_card(st, owner, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| !matches!(d.category, Category::Basic))
                && crate::skills::is_red(c.suit) == red
        });
        give_def(st, cur, "sha");
    });
    (cur, cur_pid, owner, owner_pid, cost)
}

#[test]
fn sidi_blocks_color_in_play_phase() {
    let mgr = manager(2306);
    let room = start_and_pick(&mgr, 5);
    let (cur, _cur_pid, _owner, owner_pid, cost) = sidi_setup(&mgr, &room);
    run_phase(&mgr, &room, cur, TurnPhase::Play, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("sidi"));
    mgr.execute_action(&room, &owner_pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let blocked = st.counter(&format!("sidiqu:{cur}"));
    assert!(blocked == 1 || blocked == 2, "司敌：记录封锁颜色");
    let suit_red = st
        .cards
        .get(&cost)
        .is_some_and(|c| crate::skills::is_red(c.suit));
    let banned = find_cards(&st, |c| crate::skills::is_red(c.suit) == suit_red, 1)
        .into_iter()
        .next()
        .expect("same color card");
    edit(&mgr, &room, |st| {
        assert!(
            crate::skills::sidiqu_banned(st, cur, &banned),
            "司敌：同色牌被封锁"
        );
    });
}

#[test]
fn sidi_end_phase_uses_sha_if_no_sha() {
    let mgr = manager(2307);
    let room = start_and_pick(&mgr, 5);
    let (cur, cur_pid, owner, owner_pid, cost) = sidi_setup(&mgr, &room);
    run_phase(&mgr, &room, cur, TurnPhase::Play, false);
    assert!(crate::skills::has(&state_of(&mgr, &room), owner, "sidi"));
    mgr.execute_action(&room, &owner_pid, &format!("choose:{cost}"))
        .unwrap();
    let mut hp = 0;
    edit(&mgr, &room, |st| hp = st.seats[cur].hp);
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("respond") {
        mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp - 1, "司敌：未使用【杀】则视为被杀");
    assert!(st.history.iter().any(|e| e.text.contains("司敌")));
    let _ = owner;
}

#[test]
fn sidi_pass_no_effect() {
    let mgr = manager(2308);
    let room = start_and_pick(&mgr, 5);
    let (cur, _cur_pid, owner, owner_pid, _cost) = sidi_setup(&mgr, &room);
    run_phase(&mgr, &room, cur, TurnPhase::Play, false);
    assert!(crate::skills::has(&state_of(&mgr, &room), owner, "sidi"));
    mgr.execute_action(&room, &owner_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter(&format!("sidiqu:{cur}")),
        0,
        "司敌：不发动无封锁"
    );
}

// ---------------------------------------------------------------------------
// 韩浩史涣 · 慎断 / 勇略
// ---------------------------------------------------------------------------

#[test]
fn shenduan_black_basic_to_bingliang() {
    let mgr = manager(2309);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut target, mut target_pid) =
        (0, String::new(), String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_322");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        card = give_card(st, seat, |c| {
            crate::skills::is_black(c.suit)
                && crate::data::card_def(&c.def).is_some_and(|d| d.category == Category::Basic)
        });
        take_card(st, &card);
        crate::turn::discard_by(st, Some(seat), &card);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("shenduan"),
        "慎断：黑基本牌弃置后询问"
    );
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("target"));
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].judge_zone.contains(&card),
        "慎断：置入判定区"
    );
    assert_eq!(
        st.effective_def_code(&card),
        Some("bingliang_cunduan"),
        "慎断：生效定义为兵粮寸断"
    );
}

#[test]
fn yonglue_uses_judge_card_as_sha() {
    let mgr = manager(2310);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut owner, mut owner_pid, mut judge_card, mut hp) =
        (0, String::new(), 0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        owner = st.seats_from_left_of(cur)[0];
        set_general(st, owner, "sgs_hero_322");
        owner_pid = st.seats[owner].player_id.clone();
        judge_card = trick_card(st);
        take_card(st, &judge_card);
        st.seats[cur].judge_zone.push(judge_card.clone());
        hp = st.seats[cur].hp;
    });
    run_phase(&mgr, &room, cur, TurnPhase::Judge, false);
    let st = state_of(&mgr, &room);
    eprintln!("DEBUG cur={cur} owner={owner} judge={:?} judge_zone={:?} pending={:?} phase={:?}/{:?} range={}",
        judge_card, st.seats[cur].judge_zone, st.pending.as_ref().map(|p| p.skill.clone()), st.phase, st.turn_phase,
        st.in_attack_range(owner, cur));
    assert_eq!(pending_skill(&st), Some("yonglue"));
    mgr.execute_action(&room, &owner_pid, &format!("choose:{judge_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp - 1, "勇略：视为【杀】造成伤害");
    assert!(!st.seats[cur].judge_zone.contains(&judge_card));
}

#[test]
fn yonglue_draws_if_no_damage() {
    let mgr = manager(2311);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut owner, mut owner_pid, mut judge_card, mut before) =
        (0, String::new(), 0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        owner = st.seats_from_left_of(cur)[0];
        set_general(st, owner, "sgs_hero_322");
        owner_pid = st.seats[owner].player_id.clone();
        judge_card = trick_card(st);
        take_card(st, &judge_card);
        st.seats[cur].judge_zone.push(judge_card.clone());
        give_def(st, cur, "shan");
        before = st.seats[owner].hand.len();
    });
    run_phase(&mgr, &room, cur, TurnPhase::Judge, false);
    mgr.execute_action(&room, &owner_pid, &format!("choose:{judge_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "shan"))
        .cloned()
        .expect("shan");
    mgr.execute_action(&room, &cur_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[owner].hand.len(),
        before + 1,
        "勇略：未造成伤害则摸 1"
    );
}

// ---------------------------------------------------------------------------
// 陈群 · 品第 / 法恩
// ---------------------------------------------------------------------------

#[test]
fn pindi_x1_makes_target_draw() {
    let mgr = manager(2312);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_323");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, seat, "sha");
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:pindi").unwrap();
    let st = state_of(&mgr, &room);
    let cost = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("cost");
    mgr.execute_action(&room, &pid, &cost).unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), before + 1, "品第：X=1 摸 1");
    assert_eq!(st.counter("pindi_count"), 1);
}

#[test]
fn pindi_x2_discards_and_chains_self() {
    let mgr = manager(2313);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_323");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        st.seats[target].hp = st.seats[target].max_hp - 1;
        give_def(st, seat, "sha");
        give_trick_for_pindi(st, seat);
        give_def(st, target, "sha");
        give_def(st, target, "tao");
        give_def(st, target, "shan");
        // 第一次：基本牌
        st.set_counter("pindi_count", 1);
        st.set_counter("pindi_types", 1);
    });
    mgr.execute_action(&room, &pid, "skill:pindi").unwrap();
    let st = state_of(&mgr, &room);
    let cost = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("cost");
    mgr.execute_action(&room, &pid, &cost).unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].chained, "品第：目标已受伤，陈群横置");
    assert_eq!(st.counter("pindi_count"), 2);
}

#[test]
fn faen_draws_on_flip_or_chain() {
    let mgr = manager(2314);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_323");
        before = st.seats[seat].hand.len();
        crate::skills::faen_trigger(st, seat);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 1, "法恩：摸 1 张");
}

// ---------------------------------------------------------------------------
// 吴懿 · 奔袭
// ---------------------------------------------------------------------------

#[test]
fn benxi_counts_cards_used() {
    let mgr = manager(2315);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_324");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        give_def(st, seat, "tao");
        give_def(st, seat, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let tao = st.seats[seat]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "tao"))
        .cloned()
        .expect("tao");
    edit(&mgr, &room, |st| st.seats[seat].hp = 1);
    mgr.execute_action(&room, &pid, &format!("play:{tao}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("benxi"), 1, "奔袭：使用牌后距离 -1 累计");
}

#[test]
fn benxi_offers_options_when_distance_one() {
    let mgr = manager(2316);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_324");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        // 已使用过 1 张牌：与其他角色距离 -1（5 人环形最远 2 → 均 ≤1）。
        st.set_counter("benxi", 1);
        let target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, seat, "sha");
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, Some(false));
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("benxi"), "奔袭：距离均为 1 时询问");
    let _ = target_pid;
}

#[test]
fn benxi_selected_no_respond_applies() {
    let mgr = manager(2317);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_324");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        st.set_counter("benxi", 1);
        let target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, seat, "sha");
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, Some(false));
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt_nores").unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("不能被抵消")),
        "奔袭：不能被抵消"
    );
}

// ---------------------------------------------------------------------------
// 周仓 · 忠勇
// ---------------------------------------------------------------------------

fn zhongyong_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    red: bool,
    target_shan: bool,
) -> (usize, String, usize, String, usize, String, String) {
    let (mut seat, mut pid, mut target, mut target_pid, mut receiver, mut receiver_pid, mut sha) = (
        0,
        String::new(),
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
    );
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_325");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        target = others
            .iter()
            .copied()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("in range target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        receiver = others.into_iter().find(|i| *i != target).expect("receiver");
        set_general(st, receiver, "sgs_hero_1");
        receiver_pid = st.seats[receiver].player_id.clone();
        sha = give_card(st, seat, |c| {
            matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
                && crate::skills::is_red(c.suit) == red
        });
        if target_shan {
            give_def(st, target, "shan");
        }
    });
    (seat, pid, target, target_pid, receiver, receiver_pid, sha)
}

#[test]
fn zhongyong_gives_sha_to_another() {
    let mgr = manager(2318);
    let room = start_and_pick(&mgr, 5);
    let (_seat, pid, _target, target_pid, _receiver, receiver_pid, sha) =
        zhongyong_setup(&mgr, &room, false, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhongyong"),
        "忠勇：使用【杀】后询问"
    );
    mgr.execute_action(&room, &pid, "give:sha").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{receiver_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[_receiver].hand.contains(&sha),
        "忠勇：此【杀】交给他人"
    );
}

#[test]
fn zhongyong_gives_respond_shan() {
    let mgr = manager(2319);
    let room = start_and_pick(&mgr, 5);
    let (_seat, pid, _target, target_pid, _receiver, receiver_pid, sha) =
        zhongyong_setup(&mgr, &room, false, true);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[_target]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "shan"))
        .cloned()
        .expect("shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhongyong"));
    mgr.execute_action(&room, &pid, "give:shan").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{receiver_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[_receiver].hand.contains(&shan), "忠勇：交出【闪】");
}

#[test]
fn zhongyong_red_card_lets_receiver_sha() {
    let mgr = manager(2320);
    let room = start_and_pick(&mgr, 5);
    let (_seat, pid, _target, target_pid, _receiver, receiver_pid, sha) =
        zhongyong_setup(&mgr, &room, true, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "give:sha").unwrap();
    let st = state_of(&mgr, &room);
    let receiver_pid2 = st.seats[_receiver].player_id.clone();
    mgr.execute_action(&room, &pid, &format!("target:{receiver_pid2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhongyong"),
        "忠勇：红色牌可让接收者出杀"
    );
    let target2 = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != _receiver && st.in_attack_range(_seat, *i))
        .expect("target2");
    mgr.execute_action(
        &room,
        &receiver_pid,
        &format!("target:{}", st.seats[target2].player_id),
    )
    .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("忠勇：视为对")));
}

// ---------------------------------------------------------------------------
// 张松 · 强识 / 献图
// ---------------------------------------------------------------------------

#[test]
fn qiangshi_reveals_and_records_type() {
    let mgr = manager(2321);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_326");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Play, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qiangshi"), "强识：出牌阶段开始时");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shown = st.counter("qiangshi_type");
    assert!(
        shown == 1 || shown == 2 || shown == 4,
        "强识：记录展示牌类型"
    );
}

#[test]
fn qiangshi_same_type_draw() {
    let mgr = manager(2322);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_326");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        // 已展示过「基本牌」。
        st.set_counter("qiangshi_type", 1);
        give_def(st, seat, "tao");
        st.seats[seat].hp = 1;
        before = st.seats[seat].hand.len();
    });
    let st = state_of(&mgr, &room);
    let tao = st.seats[seat]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "tao"))
        .cloned()
        .expect("tao");
    mgr.execute_action(&room, &pid, &format!("play:{tao}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before,
        "强识：用同类牌摸 1（桃本身被用掉）"
    );
}

#[test]
fn xiantu_draws_gives_and_loses_hp_at_end() {
    let mgr = manager(2323);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut cur, mut cur_pid, mut hp, mut cur_hand) =
        (0, String::new(), 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_326");
        st.turn_counters.clear();
        owner_pid = st.seats[owner].player_id.clone();
        cur = other_seats(st, owner)[0];
        set_general(st, cur, "sgs_hero_1");
        cur_pid = st.seats[cur].player_id.clone();
        hp = st.seats[owner].hp;
        cur_hand = st.seats[cur].hand.len();
        give_def(st, owner, "sha");
        give_def(st, owner, "shan");
    });
    run_phase(&mgr, &room, cur, TurnPhase::Play, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("xiantu"),
        "献图：他人出牌阶段开始时"
    );
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), cur_hand + 2, "献图：交给其两张牌");
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[owner].hp,
        hp - 1,
        "献图：未杀死角色则失去 1 点体力"
    );
}
