//! P4.4.2 一将成名批次 Y1（8 将）：将华雄 / 刘表 / 钟会 / 曹冲 / 郭淮 / 满宠 / 关平 / 简雍。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, place_mark, state_of, take_card};
use crate::state::*;

// ---------------------------------------------------------------------------
// 将华雄 · 恃勇
// ---------------------------------------------------------------------------

#[test]
fn shiyong_red_sha_reduces_max_hp() {
    let mgr = manager(1900);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_171");
        before = st.seats[seat].max_hp;
        let red_sha = st
            .cards
            .iter()
            .find(|(_, c)| c.def == "sha" && crate::skills::is_red(c.suit))
            .map(|(id, _)| id.clone())
            .expect("红色杀");
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            None,
            crate::resolve::DamageType::Normal,
            Some(&red_sha),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].max_hp, before - 1, "恃勇：体力上限 -1");
    assert!(st.history.iter().any(|e| e.text.contains("恃勇")));
}

#[test]
fn shiyong_jiu_sha_reduces_max_hp() {
    let mgr = manager(1901);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_171");
        before = st.seats[seat].max_hp;
        let black_sha = st
            .cards
            .iter()
            .find(|(_, c)| c.def == "sha" && !crate::skills::is_red(c.suit))
            .map(|(id, _)| id.clone())
            .expect("黑色杀");
        st.set_counter(&format!("jiu_boost:{seat}"), 1);
        crate::resolve::apply_damage(
            st,
            seat,
            2,
            None,
            crate::resolve::DamageType::Normal,
            Some(&black_sha),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].max_hp, before - 1, "恃勇：酒【杀】上限 -1");
}

#[test]
fn shiyong_black_sha_no_change() {
    let mgr = manager(1902);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_171");
        before = st.seats[seat].max_hp;
        let black_sha = st
            .cards
            .iter()
            .find(|(_, c)| c.def == "sha" && !crate::skills::is_red(c.suit))
            .map(|(id, _)| id.clone())
            .expect("黑色杀");
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            None,
            crate::resolve::DamageType::Normal,
            Some(&black_sha),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].max_hp, before, "恃勇：黑色杀不触发");
}

// ---------------------------------------------------------------------------
// 刘表 · 自守 / 宗室
// ---------------------------------------------------------------------------

#[test]
fn zishou_draws_extra_and_prevents_damage() {
    let mgr = manager(1903);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut hand_before, mut other, mut kingdoms) = (0, String::new(), 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_172");
        st.turn_counters.clear();
        give_def(st, seat, "shan");
        pid = st.seats[seat].player_id.clone();
        hand_before = st.seats[seat].hand.len();
        other = other_seats(st, seat)[0];
        kingdoms = crate::skills::kingdom_count(st) as usize;
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zishou"), "自守：摸牌阶段询问");
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        hand_before + 2 + kingdoms,
        "自守：多摸势力数张"
    );
    assert_eq!(st.counter("zishou"), 1);
    // 本回合防止对其他角色造成的伤害。
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        hp = st.seats[other].hp;
        crate::resolve::apply_damage(
            st,
            other,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[other].hp, hp, "自守：防止你对其他角色造成的伤害");
}

#[test]
fn zishou_pass_draws_normal() {
    let mgr = manager(1904);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut hand_before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_172");
        st.turn_counters.clear();
        give_def(st, seat, "shan");
        pid = st.seats[seat].player_id.clone();
        hand_before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    mgr.execute_action(&room, &pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        hand_before + 2,
        "自守：不发动只摸 2"
    );
    assert_eq!(st.counter("zishou"), 0);
}

#[test]
fn zongshi_hand_limit_counts_kingdoms() {
    let mgr = manager(1905);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut limit, mut base, mut kingdoms, mut jian) = (0, 0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_172");
        st.seats[seat].hp = 2;
        limit = crate::skills::hand_limit(st, seat);
        base = st.hand_limit(seat);
        kingdoms = crate::skills::kingdom_count(st) as usize;
        jian = other_seats(st, seat)[0];
        set_general(st, jian, "sgs_hero_178");
        st.seats[jian].hp = 3;
    });
    assert_eq!(limit, base + kingdoms, "宗室：每有一个势力手牌上限 +1");
    edit(&mgr, &room, |st| {
        assert_eq!(
            crate::skills::hand_limit(st, jian),
            st.hand_limit(jian),
            "宗室不作用于简雍"
        );
    });
}

// ---------------------------------------------------------------------------
// 钟会 · 权计 / 自立 / 排异
// ---------------------------------------------------------------------------

#[test]
fn quanji_places_quan_after_damage() {
    let mgr = manager(1906);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut limit, mut base) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_173");
        st.turn_counters.clear();
        give_def(st, seat, "shan");
        give_def(st, seat, "sha");
        crate::resolve::apply_damage(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
        base = st.hand_limit(seat);
        limit = crate::skills::hand_limit(st, seat);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::quan_count(&st, seat),
        1,
        "权计：获得一张「权」"
    );
    assert_eq!(limit, base + 1, "权计：每张「权」手牌上限 +1");
}

#[test]
fn zili_awaken_grants_paiyi() {
    let mgr = manager(1907);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_173");
        st.turn_counters.clear();
        before = st.seats[seat].max_hp;
        pid = st.seats[seat].player_id.clone();
        let ids: Vec<String> = st
            .cards
            .keys()
            .filter(|id| {
                !st.seats.iter().any(|s| s.hand.iter().any(|h| h == *id))
                    && !st.deck.processing.iter().any(|d| d == *id)
            })
            .take(3)
            .cloned()
            .collect();
        for id in ids {
            place_mark(st, seat, "quan", &id);
        }
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zili"), "自立：准备阶段觉醒");
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].max_hp, before - 1, "自立：体力上限 -1");
    assert!(
        st.seats[seat].extra_skills.iter().any(|c| c == "paiyi"),
        "自立：获得「排异」"
    );
    assert!(st.history.iter().any(|e| e.text.contains("自立：摸 2 张")));
}

#[test]
fn paiyi_moves_quan_and_punishes_bigger_hand() {
    let mgr = manager(1908);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut hp) = (0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_173");
        crate::skills::grant_skill(st, seat, "paiyi");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        give_def(st, seat, "shan");
        pid = st.seats[seat].player_id.clone();
        let quan = st.cards.keys().next().cloned().expect("card");
        place_mark(st, seat, "quan", &quan);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "shan");
        give_def(st, target, "sha");
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &pid, "skill:paiyi").unwrap();
    let st = state_of(&mgr, &room);
    let target_pid = st.seats[target].player_id.clone();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::quan_count(&st, seat),
        0,
        "排异：移去一张「权」"
    );
    assert_eq!(st.seats[target].hand.len(), 4, "排异：目标摸 2 张");
    assert_eq!(st.seats[target].hp, hp - 1, "排异：手牌更多则受到 1 点伤害");
}

// ---------------------------------------------------------------------------
// 曹冲 · 称象 / 仁心
// ---------------------------------------------------------------------------

#[test]
fn chengxiang_gains_cards_within_13() {
    let mgr = manager(1909);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    let before;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_174");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        crate::resolve::apply_damage(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("chengxiang"));
    let first = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("亮出的牌");
    before = st.seats[seat].hand.len();
    mgr.execute_action(&room, &pid, &first).unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 1, "称象：获得所选牌");
    assert_eq!(st.deck.processing.len(), 0, "称象：处理区清空");
    assert_eq!(st.deck.discard.len() >= 3, true, "称象：未选牌进弃牌堆");
}

#[test]
fn chengxiang_rejects_sum_over_13() {
    let mgr = manager(1910);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_174");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        let kings = find_cards(st, |c| c.rank == 13, 4);
        assert_eq!(kings.len(), 4, "需要 4 张 K");
        set_top(st, &kings);
        crate::resolve::apply_damage(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    let picks: Vec<String> = pending_options_with(&st, "choose:")
        .into_iter()
        .take(2)
        .collect();
    mgr.execute_action(&room, &pid, &picks[0]).unwrap();
    assert!(
        mgr.execute_action(&room, &pid, &picks[1]).is_err(),
        "点数和超过 13 不能选择"
    );
}

#[test]
fn renxin_prevents_damage_with_equipment() {
    let mgr = manager(1911);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut victim, mut equip, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_174");
        st.turn_counters.clear();
        equip = give_equip(st, owner);
        owner_pid = st.seats[owner].player_id.clone();
        victim = other_seats(st, owner)[0];
        set_general(st, victim, "sgs_hero_1");
        st.seats[victim].hp = 1;
        hp = st.seats[victim].hp;
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            None,
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("renxin"));
    mgr.execute_action(&room, &owner_pid, &format!("equip:{equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, hp, "仁心：防止伤害");
    assert!(st.seats[owner].flipped, "仁心：翻面");
    assert!(st.deck.discard.contains(&equip), "仁心：弃置装备牌");
}

// ---------------------------------------------------------------------------
// 郭淮 · 精策
// ---------------------------------------------------------------------------

#[test]
fn jingce_hand_limit_by_used_suits() {
    let mgr = manager(1912);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut limit, mut base) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_175");
        st.turn_counters.clear();
        st.seats[seat].hp = 3;
        base = st.hand_limit(seat);
        let a = give_suit(st, seat, Suit::Heart);
        crate::skills::note_card_used(st, seat, &a);
        let b = give_suit(st, seat, Suit::Spade);
        crate::skills::note_card_used(st, seat, &b);
        limit = crate::skills::hand_limit(st, seat);
    });
    assert_eq!(limit, base + 2, "精策：两种花色手牌上限 +2");
}

#[test]
fn jingce_end_play_draws_type_count() {
    let mgr = manager(1913);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    let found;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_175");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        let basic = give_def(st, seat, "sha");
        crate::skills::note_card_used(st, seat, &basic);
        let trick = give_def(st, seat, "wuzhong_shengyou");
        crate::skills::note_card_used(st, seat, &trick);
    });
    mgr.execute_action(&room, &pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    found = st
        .history
        .iter()
        .any(|e| e.text.contains("精策：出牌阶段结束，摸 2 张"));
    assert!(found, "精策：按使用过的牌类型数摸牌");
}

#[test]
fn jingce_same_type_counts_once() {
    let mgr = manager(1914);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut types) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_175");
        st.turn_counters.clear();
        let a = give_def(st, seat, "sha");
        crate::skills::note_card_used(st, seat, &a);
        let b = give_def(st, seat, "shan");
        crate::skills::note_card_used(st, seat, &b);
        types = crate::skills::jingce_type_count(st);
    });
    assert_eq!(types, 1, "精策：同类型只计一次");
    let _ = seat;
}

// ---------------------------------------------------------------------------
// 满宠 · 峻刑 / 御策
// ---------------------------------------------------------------------------

#[test]
fn junxing_target_discards_different_type() {
    let mgr = manager(1915);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut cost, mut trick) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_176");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        cost = give_def(st, seat, "shan");
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "sha");
        trick = give_def(st, target, "wuzhong_shengyou");
    });
    mgr.execute_action(&room, &pid, "skill:junxing").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    let target_pid = st.seats[target].player_id.clone();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt1").unwrap();
    mgr.execute_action(&room, &target_pid, &format!("choose:{trick}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&trick), "峻刑：目标弃置异类型手牌");
    assert!(st.deck.discard.contains(&cost), "峻刑：发起者弃置手牌");
}

#[test]
fn junxing_target_flips_and_draws() {
    let mgr = manager(1916);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut cost, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_176");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        cost = give_def(st, seat, "shan");
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "sha");
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:junxing").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    let target_pid = st.seats[target].player_id.clone();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].flipped, "峻刑：目标翻面");
    assert_eq!(st.seats[target].hand.len(), before + 1, "峻刑：摸等量牌");
}

#[test]
fn yuce_source_choice_heals() {
    let mgr = manager(1917);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut source, mut source_pid, mut shown) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_176");
        st.turn_counters.clear();
        st.seats[seat].hp = 2;
        give_def(st, seat, "shan");
        pid = st.seats[seat].player_id.clone();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        give_def(st, source, "sha");
        source_pid = st.seats[source].player_id.clone();
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
        shown = st.seats[seat].hand[0].clone();
        assert_eq!(st.seats[seat].hp, 1);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yuce"));
    mgr.execute_action(&room, &pid, &format!("choose:{shown}"))
        .unwrap();
    mgr.execute_action(&room, &source_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, 2, "御策：来源选择令其回复 1 点体力");
    assert!(st.history.iter().any(|e| e.text.contains("御策")));
}

// ---------------------------------------------------------------------------
// 关平 · 龙吟
// ---------------------------------------------------------------------------

fn setup_longyin(
    mgr: &game_engine_core::GameManager,
    room: &str,
) -> (usize, String, usize, String, String) {
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut cost) =
        (0, String::new(), 0, String::new(), String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        user_pid = st.seats[user].player_id.clone();
        owner = other_seats(st, user)[0];
        set_general(st, owner, "sgs_hero_177");
        owner_pid = st.seats[owner].player_id.clone();
        cost = give_def(st, owner, "shan");
        give_card(st, user, |c| {
            c.def == "sha" && crate::skills::is_red(c.suit)
        });
    });
    (user, user_pid, owner, owner_pid, cost)
}

fn play_sha(mgr: &game_engine_core::GameManager, room: &str, pid: &str, user: usize) {
    let st = state_of(mgr, room);
    let sha = st.seats[user]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        })
        .cloned()
        .expect("sha");
    mgr.execute_action(room, pid, &format!("play:{sha}"))
        .unwrap();
}

#[test]
fn longyin_prevents_sha_count() {
    let mgr = manager(1918);
    let room = start_and_pick(&mgr, 5);
    let (user, user_pid, _owner, owner_pid, cost) = setup_longyin(&mgr, &room);
    play_sha(&mgr, &room, &user_pid, user);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("longyin"),
        "龙吟：使用【杀】时询问"
    );
    mgr.execute_action(&room, &owner_pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("sha_used"), 0, "龙吟：此【杀】不计入次数");
    assert!(st.deck.discard.contains(&cost));
}

#[test]
fn longyin_draws_on_red_sha() {
    let mgr = manager(1919);
    let room = start_and_pick(&mgr, 5);
    let (user, user_pid, owner, owner_pid, cost) = setup_longyin(&mgr, &room);
    let before = state_of(&mgr, &room).seats[owner].hand.len();
    play_sha(&mgr, &room, &user_pid, user);
    mgr.execute_action(&room, &owner_pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[owner].hand.len(),
        before,
        "龙吟：红色【杀】弃 1 摸 1"
    );
}

#[test]
fn longyin_pass_keeps_count() {
    let mgr = manager(1920);
    let room = start_and_pick(&mgr, 5);
    let (user, user_pid, _owner, owner_pid, _cost) = setup_longyin(&mgr, &room);
    play_sha(&mgr, &room, &user_pid, user);
    mgr.execute_action(&room, &owner_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("sha_used"), 1, "龙吟：不发动则计入次数");
}

// ---------------------------------------------------------------------------
// 简雍 · 巧说 / 纵适
// ---------------------------------------------------------------------------

fn setup_qiaoshuo(
    mgr: &game_engine_core::GameManager,
    room: &str,
    own_high: bool,
) -> (usize, String, usize, String) {
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_178");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_rank(st, seat, if own_high { 13 } else { 1 });
        give_def(st, seat, "sha");
        give_rank(st, seat, 5);
        give_rank(st, target, if own_high { 1 } else { 13 });
    });
    run_phase(mgr, room, seat, TurnPhase::Play, false);
    (seat, pid, target, target_pid)
}

#[test]
fn qiaoshuo_win_enables_extra_sha_target() {
    let mgr = manager(1921);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, target_pid) = setup_qiaoshuo(&mgr, &room, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qiaoshuo"));
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let high = st.seats[seat]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 13))
        .cloned()
        .expect("高点数手牌");
    mgr.execute_action(&room, &pid, &format!("choose:{high}"))
        .unwrap();
    let low = st.seats[_target]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 1))
        .cloned()
        .expect("低点数手牌");
    mgr.execute_action(&room, &target_pid, &format!("choose:{low}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("qiaoshuo_win"), 1, "巧说：拼点赢");
    // 下一张【杀】可多指定一个目标（组合选项）。
    play_sha(&mgr, &room, &pid, seat);
    let st = state_of(&mgr, &room);
    assert!(
        !pending_options_with(&st, "targets:").is_empty(),
        "巧说：下一张【杀】可多指定一个目标"
    );
}

#[test]
fn qiaoshuo_lose_bans_tricks() {
    let mgr = manager(1922);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, target_pid) = setup_qiaoshuo(&mgr, &room, false);
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let low = st.seats[seat]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 1))
        .cloned()
        .expect("低点数手牌");
    mgr.execute_action(&room, &pid, &format!("choose:{low}"))
        .unwrap();
    let high = st.seats[_target]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.rank == 13))
        .cloned()
        .expect("高点数手牌");
    mgr.execute_action(&room, &target_pid, &format!("choose:{high}"))
        .unwrap();
    let mut trick = String::new();
    edit(&mgr, &room, |st| {
        trick = give_def(st, seat, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("qiaoshuo_fail"), 1, "巧说：没赢");
    let acts = crate::turn::legal_actions(&st, &pid);
    assert!(
        !acts.iter().any(|a| a.id == format!("play:{trick}")),
        "巧说没赢：锦囊不在合法动作中"
    );
    assert!(
        mgr.execute_action(&room, &pid, &format!("play:{trick}"))
            .is_err(),
        "巧说没赢：不能使用锦囊牌"
    );
}

#[test]
fn zongshi_gains_pindian_card() {
    let mgr = manager(1923);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut big, mut small, mut other) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_178");
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        big = st
            .cards
            .iter()
            .find(|(_, c)| c.rank == 9)
            .map(|(id, _)| id.clone())
            .expect("9");
        small = st
            .cards
            .iter()
            .find(|(_, c)| c.rank == 2)
            .map(|(id, _)| id.clone())
            .expect("2");
        take_card(st, &big);
        take_card(st, &small);
        st.deck.discard.push(big.clone());
        st.deck.discard.push(small.clone());
        // 简雍没赢：获得自己的拼点牌（big）。
        crate::skills::zongshi_on_pindian(st, seat, other, Some(other), &big, &small);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.contains(&big), "纵适：没赢获得自己的牌");
    edit(&mgr, &room, |st| {
        take_card(st, &small);
        st.deck.discard.push(small.clone());
        // 简雍赢：获得点数较小的拼点牌（small）。
        crate::skills::zongshi_on_pindian(st, seat, other, Some(seat), &big, &small);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.contains(&small), "纵适：赢获得较小的牌");
}
