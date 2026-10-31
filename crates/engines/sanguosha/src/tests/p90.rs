//! P4.4.2 一将成名批次 Y2（8 将）：刘封 / 潘璋马忠 / 虞翻 / 朱然 / 伏皇后 / 李儒 / 法正 / 徐庶。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, place_mark, state_of, take_card};
use crate::state::*;

fn find_trick(st: &SgsState, black: bool) -> String {
    st.cards
        .iter()
        .find(|(_, c)| {
            crate::data::card_def(&c.def)
                .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
                && (!black || crate::skills::is_black(c.suit))
        })
        .map(|(id, _)| id.clone())
        .expect("没有可用的锦囊牌")
}

fn give_trick(st: &mut SgsState, seat: usize, black: bool) -> String {
    give_card(st, seat, |c| {
        crate::data::card_def(&c.def)
            .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
            && (!black || crate::skills::is_black(c.suit))
    })
}

fn danshou_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    used: u32,
) -> (usize, String, usize, String) {
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_182");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        for _ in 0..4 {
            give_def(st, seat, "shan");
        }
        give_def(st, target, "sha");
        give_def(st, target, "shan");
        if used > 0 {
            st.set_counter("danshou_used", used);
        }
    });
    (seat, pid, target, target_pid)
}

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

// ---------------------------------------------------------------------------
// 刘封 · 陷嗣
// ---------------------------------------------------------------------------

#[test]
fn xiansi_prepare_takes_cards_as_ni() {
    let mgr = manager(2100);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_179");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "sha");
        give_def(st, target, "shan");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("xiansi"));
    let target_pid = st.seats[target].player_id.clone();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::ni_count(&st, seat),
        1,
        "陷嗣：获得一张「逆」"
    );
    assert_eq!(st.seats[target].hand.len(), 1, "陷嗣：目标失去一张牌");
}

#[test]
fn xiansi_other_can_use_sha() {
    let mgr = manager(2101);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut user, mut user_pid, mut ids, mut hp) = (0, 0, String::new(), Vec::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_179");
        user = other_seats(st, owner)[0];
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        ids = find_cards(st, |c| c.def == "shan", 2);
        for id in &ids {
            place_mark(st, owner, "ni", id);
        }
        hp = st.seats[owner].hp;
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &user_pid);
    assert!(acts.iter().any(|a| a.id == "skill:xiansi"), "陷嗣：可选");
    mgr.execute_action(&room, &user_pid, "skill:xiansi")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::ni_count(&st, owner),
        0,
        "陷嗣：移去两张「逆」"
    );
    assert_eq!(st.counter("xiansi_used"), 1);
    assert_eq!(st.seats[owner].hp, hp - 1, "陷嗣：虚拟【杀】造成 1 点伤害");
}

#[test]
fn xiansi_requires_two_ni() {
    let mgr = manager(2102);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut user, mut user_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_179");
        user = other_seats(st, owner)[0];
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        let id = find_cards(st, |c| c.def == "shan", 1)
            .into_iter()
            .next()
            .expect("shan");
        place_mark(st, owner, "ni", &id);
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &user_pid);
    assert!(
        !acts.iter().any(|a| a.id == "skill:xiansi"),
        "陷嗣：不足两张「逆」不可发动"
    );
}

// ---------------------------------------------------------------------------
// 潘璋马忠 · 暗箭 / 夺刀
// ---------------------------------------------------------------------------

#[test]
fn anjian_adds_damage_out_of_range() {
    let mgr = manager(2103);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_180");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        // 长武器（范围 3）：可以打到攻击范围不含自己的远处目标。
        let weapon = give_card(st, seat, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| {
                d.equip_slot == Some(EquipSlot::Weapon) && d.range_value == Some(3)
            })
        });
        take_card(st, &weapon);
        st.seats[seat]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon));
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i) && !st.in_attack_range(*i, seat))
            .expect("远目标");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
        give_def(st, seat, "sha");
    });
    let st = state_of(&mgr, &room);
    let sha = st.seats[seat]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "sha"))
        .cloned()
        .expect("sha");
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "暗箭：不在攻击范围内伤害 +1");
}

#[test]
fn anjian_no_bonus_in_range() {
    let mgr = manager(2104);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut target, mut hp, mut sha) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_180");
        st.turn_counters.clear();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(*i, seat))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        hp = st.seats[target].hp;
        sha = find_cards(st, |c| c.def == "sha", 1)
            .into_iter()
            .next()
            .expect("sha");
        crate::resolve::apply_damage(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "暗箭：范围内不加伤");
}

#[test]
fn duodao_gains_weapon() {
    let mgr = manager(2105);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut source, mut weapon, mut cost, mut sha) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_180");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        weapon = give_card(st, source, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(EquipSlot::Weapon))
        });
        take_card(st, &weapon);
        st.seats[source]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon.clone()));
        cost = give_def(st, seat, "shan");
        sha = find_cards(st, |c| c.def == "sha", 1)
            .into_iter()
            .next()
            .expect("sha");
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("duodao"));
    mgr.execute_action(&room, &pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.contains(&weapon), "夺刀：获得武器");
    assert!(st.seats[source]
        .equips
        .get(&EquipSlot::Weapon)
        .cloned()
        .flatten()
        .is_none());
    assert!(st.deck.discard.contains(&cost));
}

// ---------------------------------------------------------------------------
// 虞翻 · 纵玄 / 直言
// ---------------------------------------------------------------------------

#[test]
fn zongxuan_puts_discarded_on_top() {
    let mgr = manager(2106);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_181");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "sha");
        turn_take_and_discard_by(st, seat, &card);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zongxuan"));
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.deck.draw.last(), Some(&card), "纵玄：置于牌堆顶");
    assert!(!st.deck.discard.contains(&card));
}

fn turn_take_and_discard_by(st: &mut SgsState, seat: usize, card: &str) {
    take_card(st, card);
    crate::turn::discard_by(st, Some(seat), card);
}

#[test]
fn zhiyan_reveals_equip_and_heals() {
    let mgr = manager(2107);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut equip) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_181");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        st.seats[target].hp = 2;
        equip = find_cards(
            st,
            |c| crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some()),
            1,
        )
        .into_iter()
        .next()
        .expect("equip");
        set_top(st, &[equip.clone()]);
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhiyan"));
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 3, "直言：装备牌回复 1");
    assert!(st.seats[target]
        .equips
        .values()
        .flatten()
        .any(|c| c == &equip));
}

#[test]
fn zhiyan_non_equip_no_heal() {
    let mgr = manager(2108);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut basic) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_181");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        st.seats[target].hp = 2;
        basic = find_cards(st, |c| c.def == "sha", 1)
            .into_iter()
            .next()
            .expect("sha");
        set_top(st, &[basic.clone()]);
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 2, "直言：非装备牌不回复");
    assert!(st.seats[target].hand.contains(&basic), "直言：牌进入手牌");
}

// ---------------------------------------------------------------------------
// 朱然 · 胆守
// ---------------------------------------------------------------------------

#[test]
fn danshou_x1_discards_target_card() {
    let mgr = manager(2109);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, target, target_pid) = danshou_setup(&mgr, &room, 0);
    assert!(
        crate::skills::has(&state_of(&mgr, &room), seat, "danshou"),
        "朱然拥有胆守"
    );
    mgr.execute_action(&room, &pid, "skill:danshou").unwrap();
    let st = state_of(&mgr, &room);
    let cost = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("cost");
    mgr.execute_action(&room, &pid, &cost).unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 1, "胆守①：目标失去一张牌");
}

#[test]
fn danshou_x2_gains_card() {
    let mgr = manager(2110);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, target_pid) = danshou_setup(&mgr, &room, 1);
    let st0 = state_of(&mgr, &room);
    assert!(crate::skills::has(&st0, seat, "danshou"), "朱然拥有胆守");
    let before = st0.seats[seat].hand.len();
    mgr.execute_action(&room, &pid, "skill:danshou").unwrap();
    let st = state_of(&mgr, &room);
    let costs: Vec<String> = pending_options_with(&st, "choose:")
        .into_iter()
        .take(2)
        .collect();
    for cost in &costs {
        mgr.execute_action(&room, &pid, cost).unwrap();
    }
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before - 2 + 1,
        "胆守②：弃 2 张后获得目标一张牌"
    );
}

#[test]
fn danshou_x3_deals_damage() {
    let mgr = manager(2111);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, target, target_pid) = danshou_setup(&mgr, &room, 2);
    assert!(
        crate::skills::has(&state_of(&mgr, &room), seat, "danshou"),
        "朱然拥有胆守"
    );
    let mut hp = 0;
    edit(&mgr, &room, |st| hp = st.seats[target].hp);
    mgr.execute_action(&room, &pid, "skill:danshou").unwrap();
    let st = state_of(&mgr, &room);
    let costs: Vec<String> = pending_options_with(&st, "choose:")
        .into_iter()
        .take(3)
        .collect();
    for cost in &costs {
        mgr.execute_action(&room, &pid, cost).unwrap();
    }
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "胆守③：造成 1 点伤害");
    assert_eq!(st.counter("danshou_used"), 3);
}

// ---------------------------------------------------------------------------
// 伏皇后 · 惴恐 / 求援
// ---------------------------------------------------------------------------

fn zhuikong_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    owner_high: bool,
) -> (usize, usize, String, String) {
    let (mut current, mut owner, mut owner_pid, mut current_pid) =
        (0, 0, String::new(), String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        st.turn_counters.clear();
        current_pid = st.seats[current].player_id.clone();
        owner = other_seats(st, current)[0];
        set_general(st, owner, "sgs_hero_183");
        st.seats[owner].hp = st.seats[owner].max_hp - 1;
        owner_pid = st.seats[owner].player_id.clone();
        give_rank(st, owner, if owner_high { 13 } else { 1 });
        give_rank(st, current, if owner_high { 1 } else { 13 });
    });
    (current, owner, owner_pid, current_pid)
}

#[test]
fn zhuikong_win_denies_targets() {
    let mgr = manager(2112);
    let room = start_and_pick(&mgr, 5);
    let (current, owner, owner_pid, current_pid) = zhuikong_setup(&mgr, &room, true);
    run_phase(&mgr, &room, current, TurnPhase::Prepare, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhuikong"));
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    let high = st.seats[owner]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 13))
        .cloned()
        .expect("high");
    mgr.execute_action(&room, &owner_pid, &format!("choose:{high}"))
        .unwrap();
    let low = st.seats[current]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 1))
        .cloned()
        .expect("low");
    mgr.execute_action(&room, &current_pid, &format!("choose:{low}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let other = other_seats(&st, current)[0];
    assert!(
        crate::skills::deny_target(&st, current, other),
        "惴恐：赢了则不能对其他角色使用牌"
    );
}

#[test]
fn zhuikong_lose_distance_one() {
    let mgr = manager(2113);
    let room = start_and_pick(&mgr, 5);
    let (current, owner, owner_pid, current_pid) = zhuikong_setup(&mgr, &room, false);
    run_phase(&mgr, &room, current, TurnPhase::Prepare, false);
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    let low = st.seats[owner]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 1))
        .cloned()
        .expect("low");
    mgr.execute_action(&room, &owner_pid, &format!("choose:{low}"))
        .unwrap();
    let high = st.seats[current]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 13))
        .cloned()
        .expect("high");
    mgr.execute_action(&room, &current_pid, &format!("choose:{high}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let d = st.seat_distance(current, owner);
    assert_eq!(d, 1, "惴恐：没赢则距离视为 1");
}

#[test]
fn qiuyuan_helper_gives_shan() {
    let mgr = manager(2114);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut helper, mut helper_pid, mut shan) = (
        0,
        String::new(),
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        let others = other_seats(st, user);
        owner = others[0];
        set_general(st, owner, "sgs_hero_183");
        owner_pid = st.seats[owner].player_id.clone();
        helper = others[1];
        set_general(st, helper, "sgs_hero_1");
        helper_pid = st.seats[helper].player_id.clone();
        shan = give_def(st, helper, "shan");
        give_def(st, user, "sha");
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
    assert_eq!(pending_skill(&st), Some("qiuyuan"));
    mgr.execute_action(&room, &owner_pid, &format!("target:{helper_pid}"))
        .unwrap();
    mgr.execute_action(&room, &helper_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[owner].hand.contains(&shan), "求援：获得【闪】");
}

// ---------------------------------------------------------------------------
// 李儒 · 灭计 / 绝策 / 焚城
// ---------------------------------------------------------------------------

#[test]
fn mieji_puts_black_trick_on_top() {
    let mgr = manager(2115);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut trick, mut target_trick) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_184");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        trick = give_trick(st, seat, true);
        target_trick = give_trick(st, target, true);
        give_def(st, target, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:mieji").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{trick}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.draw.contains(&trick), "灭计：黑色锦囊置于牌堆顶");
    assert!(
        st.deck.discard.contains(&target_trick),
        "灭计：目标弃置锦囊牌"
    );
}

#[test]
fn juece_damages_handless() {
    let mgr = manager(2116);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_184");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("juece"));
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "绝策：对其造成 1 点伤害");
}

#[test]
fn fencheng_first_choice_fire_damage() {
    let mgr = manager(2117);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_184");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = st.seats_from_left_of(seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &pid, "skill:fencheng").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("fencheng"));
    mgr.execute_action(&room, &target_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "焚城：受到 2 点火焰伤害");
    assert!(st.seats[seat].flags.contains_key("fencheng_used"));
}

// ---------------------------------------------------------------------------
// 法正 · 恩怨 / 眩惑
// ---------------------------------------------------------------------------

#[test]
fn enyuan_source_loses_hp() {
    let mgr = manager(2118);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut source_pid, mut hp) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_185");
        st.turn_counters.clear();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        source_pid = st.seats[source].player_id.clone();
        give_def(st, source, "sha");
        hp = st.seats[source].hp;
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("enyuan"));
    mgr.execute_action(&room, &source_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[source].hp, hp - 1, "恩怨：来源失去 1 点体力");
}

#[test]
fn enyuan_gain_two_makes_giver_draw() {
    let mgr = manager(2119);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut other, mut before) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_185");
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        before = st.seats[other].hand.len();
        crate::resolve_yijiang4::enyuan_on_gain(st, seat, other, 2);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[other].hand.len(), before + 1, "恩怨：对方摸 1");
}

#[test]
fn xuanhuo_helper_lets_fazheng_gain_two() {
    let mgr = manager(2120);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut helper, mut helper_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_185");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        helper = other_seats(st, seat)[0];
        set_general(st, helper, "sgs_hero_1");
        helper_pid = st.seats[helper].player_id.clone();
        give_def(st, helper, "sha");
        give_def(st, helper, "shan");
        crate::turn::draw_cards(st, helper, 0, &mut Vec::new());
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("xuanhuo"));
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{helper_pid}"))
        .unwrap();
    mgr.execute_action(&room, &helper_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before + 2,
        "眩惑：获得其两张牌且不再正常摸牌"
    );
}

// ---------------------------------------------------------------------------
// 徐庶 · 无言 / 举荐
// ---------------------------------------------------------------------------

#[test]
fn wuyan_prevents_trick_damage_on_self() {
    let mgr = manager(2121);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut hp, mut trick) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_186");
        st.turn_counters.clear();
        hp = st.seats[seat].hp;
        trick = find_trick(st, false);
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            None,
            crate::resolve::DamageType::Fire,
            Some(&trick),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "无言：受到的锦囊伤害被防止");
    assert!(st.history.iter().any(|e| e.text.contains("无言")));
}

#[test]
fn wuyan_prevents_trick_damage_dealt() {
    let mgr = manager(2122);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut target, mut hp, mut trick) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_186");
        st.turn_counters.clear();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        hp = st.seats[target].hp;
        trick = find_trick(st, false);
        crate::resolve::apply_damage(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Fire,
            Some(&trick),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "无言：你使用的锦囊伤害被防止");
}

#[test]
fn jujian_discards_non_basic_and_target_draws() {
    let mgr = manager(2123);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut card, mut before) =
        (0, String::new(), 0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_186");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        card = give_trick(st, seat, false);
        before = st.seats[target].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jujian"));
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("举荐：") && e.text.contains("摸 2")),
        "举荐：目标摸 2"
    );
    assert!(st.seats[target].hand.len() >= before + 2);
    assert!(st.deck.discard.contains(&card));
}
