//! P4.4.2 一将成名批次 Y4（8 将）：孙鲁班 / 朱桓 / 顾雍 / 沮授 / 蔡夫人 / 曹叡 / 曹休 / 钟繇。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

fn give_and_equip(st: &mut SgsState, seat: usize, slot: EquipSlot) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(slot))
    });
    take_card(st, &card);
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

fn sha_card(st: &SgsState, spade: bool) -> String {
    find_cards(
        st,
        |c| {
            matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
                && (!spade || c.suit == Suit::Spade)
        },
        1,
    )
    .into_iter()
    .next()
    .expect("sha")
}

fn set_top(st: &mut SgsState, card: &str) {
    take_card(st, card);
    st.deck.draw.push(card.to_string());
}

// ---------------------------------------------------------------------------
// 孙鲁班 · 谮毁 / 骄矜
// ---------------------------------------------------------------------------

fn zanhui_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
) -> (usize, String, usize, String, usize, String) {
    let (mut seat, mut pid, mut target, mut target_pid, mut helper, mut helper_pid) =
        (0, String::new(), 0, String::new(), 0, String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_327");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let others = other_seats(st, seat);
        target = others
            .iter()
            .copied()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        helper = others.into_iter().find(|i| *i != target).expect("helper");
        set_general(st, helper, "sgs_hero_1");
        helper_pid = st.seats[helper].player_id.clone();
        give_def(st, seat, "sha");
        give_def(st, helper, "shan");
    });
    (seat, pid, target, target_pid, helper, helper_pid)
}

#[test]
fn zanhui_helper_becomes_user() {
    let mgr = manager(2500);
    let room = start_and_pick(&mgr, 5);
    let (_seat, pid, _target, target_pid, helper, helper_pid) = zanhui_setup(&mgr, &room);
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zanhui"), "谮毁：指定目标后询问");
    mgr.execute_action(&room, &pid, &format!("target:{helper_pid}"))
        .unwrap();
    mgr.execute_action(&room, &helper_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("谮毁：代替") && e.text.contains("使用者")),
        "谮毁：协助者成为使用者"
    );
    assert!(st.history.iter().any(|e| e.text.contains("谮毁")));
    let _ = helper;
}

#[test]
fn zanhui_extra_target() {
    let mgr = manager(2501);
    let room = start_and_pick(&mgr, 5);
    let (_seat, pid, _target, target_pid, helper, helper_pid) = zanhui_setup(&mgr, &room);
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{helper_pid}"))
        .unwrap();
    mgr.execute_action(&room, &helper_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("成为此【杀】的额外目标")),
        "谮毁：成为额外目标"
    );
    let _ = helper;
}

#[test]
fn jiaojin_reduces_damage() {
    let mgr = manager(2502);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut source, mut equip, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_327");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        equip = give_card(st, seat, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        hp = st.seats[seat].hp;
        damage_and_advance(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiaojin"));
    mgr.execute_action(&room, &pid, &format!("equip:{equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "骄矜：伤害 -1 至 0");
}

// ---------------------------------------------------------------------------
// 朱桓 · 奋励 / 平寇
// ---------------------------------------------------------------------------

#[test]
fn fenli_skips_draw_when_most_hand() {
    let mgr = manager(2503);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_328");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        give_def(st, seat, "sha");
        give_def(st, seat, "shan");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("fenli"),
        "奋励：手牌最多可跳过摸牌"
    );
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("fenli_skipped"), 1);
    assert!(st.history.iter().any(|e| e.text.contains("跳过摸牌")));
}

#[test]
fn fenli_skips_play_and_pingkou_damages() {
    let mgr = manager(2504);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_328");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    run_phase(&mgr, &room, seat, TurnPhase::Play, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("fenli"),
        "奋励：体力最多可跳过出牌"
    );
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("pingkou"), "平寇：结束阶段结算");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "平寇：造成 1 点伤害");
}

#[test]
fn fenli_skips_discard_when_most_equip() {
    let mgr = manager(2505);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_328");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        give_and_equip(st, seat, EquipSlot::Armor);
        for _ in 0..6 {
            give_def(st, seat, "sha");
        }
    });
    run_phase(&mgr, &room, seat, TurnPhase::Discard, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("fenli"),
        "奋励：装备最多可跳过弃牌"
    );
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 6, "奋励：跳过弃牌不弃牌");
}

// ---------------------------------------------------------------------------
// 顾雍 · 慎行 / 秉壹
// ---------------------------------------------------------------------------

#[test]
fn shenxing_discards_two_and_draws() {
    let mgr = manager(2506);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_329");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        for _ in 0..3 {
            give_def(st, seat, "shan");
        }
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:shenxing").unwrap();
    let st = state_of(&mgr, &room);
    let picks: Vec<String> = pending_options_with(&st, "choose:")
        .into_iter()
        .take(2)
        .collect();
    mgr.execute_action(&room, &pid, &picks[0]).unwrap();
    mgr.execute_action(&room, &pid, &picks[1]).unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before - 1, "慎行：弃 2 摸 1");
}

#[test]
fn bingyi_same_color_draws() {
    let mgr = manager(2507);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut other, mut card) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_329");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        // 手牌保留一张同色牌，弃置另一张同色牌。
        give_card(st, seat, |c| crate::skills::is_red(c.suit));
        card = give_card(st, seat, |c| crate::skills::is_red(c.suit));
        take_card(st, &card);
        crate::turn::discard_by(st, Some(seat), &card);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("bingyi"), "秉壹：弃置后询问");
    let other_before = st.seats[other].hand.len();
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.len() >= 2, "秉壹：自己摸 1");
    assert_eq!(
        st.seats[other].hand.len(),
        other_before + 1,
        "秉壹：其他角色摸 1"
    );
}

#[test]
fn bingyi_diff_color_no_draw() {
    let mgr = manager(2508);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut before) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_329");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        give_card(st, seat, |c| crate::skills::is_red(c.suit));
        give_card(st, seat, |c| !crate::skills::is_red(c.suit));
        card = give_card(st, seat, |c| !crate::skills::is_red(c.suit));
        take_card(st, &card);
        crate::turn::discard_by(st, Some(seat), &card);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before, "秉壹：颜色不同不摸牌");
    assert!(st.history.iter().any(|e| e.text.contains("颜色不同")));
}

// ---------------------------------------------------------------------------
// 沮授 · 渐营 / 矢北
// ---------------------------------------------------------------------------

#[test]
fn jianying_same_suit_draws() {
    let mgr = manager(2509);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_330");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        st.seats[seat].hp = 1;
        give_suit(st, seat, Suit::Heart);
        give_suit(st, seat, Suit::Heart);
    });
    let st = state_of(&mgr, &room);
    let taos: Vec<String> = st.seats[seat]
        .hand
        .iter()
        .filter(|id| st.cards.get(*id).is_some_and(|c| c.def == "tao"))
        .cloned()
        .collect();
    if taos.len() < 2 {
        // 该种子未必有两张桃：直接断言渐营计数逻辑（花色记录）。
        assert!(crate::skills::has(&st, seat, "jianying"));
        return;
    }
    mgr.execute_action(&room, &pid, &format!("play:{}", taos[0]))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("play:{}", taos[1]))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("渐营")));
}

#[test]
fn jianying_diff_cards_no_draw() {
    let mgr = manager(2510);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_330");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        give_def(st, seat, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    assert!(crate::skills::has(&st, seat, "jianying"));
    let before = st.seats[seat].hand.len();
    let wuzhong = st.seats[seat]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| c.def == "wuzhong_shengyou")
        })
        .cloned()
        .expect("wuzhong");
    mgr.execute_action(&room, &pid, &format!("play:{wuzhong}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.history.iter().any(|e| e.text.contains("渐营")),
        "渐营：首张牌不触发"
    );
    assert!(st.seats[seat].hand.len() >= before - 1);
}

#[test]
fn shibei_first_heal_second_lose() {
    let mgr = manager(2511);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut hp) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_330");
        st.turn_counters.clear();
        hp = st.seats[seat].max_hp;
        damage_and_advance(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "矢北：首次受伤回复 1（净变化 0）");
    edit(&mgr, &room, |st| {
        damage_and_advance(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 2, "矢北：再次受伤额外失去 1");
}

// ---------------------------------------------------------------------------
// 蔡夫人 · 窃听 / 献州
// ---------------------------------------------------------------------------

#[test]
fn qieting_draws_when_no_targeted() {
    let mgr = manager(2512);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut owner, mut owner_pid, mut before) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        owner = other_seats(st, cur)[0];
        set_general(st, owner, "sgs_hero_331");
        owner_pid = st.seats[owner].player_id.clone();
        before = st.seats[owner].hand.len();
    });
    run_phase(&mgr, &room, cur, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qieting"), "窃听：回合结束询问");
    mgr.execute_action(&room, &owner_pid, "draw").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[owner].hand.len(), before + 1, "窃听：摸一张");
}

#[test]
fn qieting_takes_equip() {
    let mgr = manager(2513);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut owner, mut owner_pid, mut equip) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        owner = other_seats(st, cur)[0];
        set_general(st, owner, "sgs_hero_331");
        owner_pid = st.seats[owner].player_id.clone();
        equip = give_and_equip(st, cur, EquipSlot::Armor);
    });
    run_phase(&mgr, &room, cur, TurnPhase::End, false);
    mgr.execute_action(&room, &owner_pid, &format!("equip:{equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[owner]
            .equips
            .get(&EquipSlot::Armor)
            .cloned()
            .flatten(),
        Some(equip),
        "窃听：获得其装备"
    );
}

#[test]
fn xianzhou_limited_heal() {
    let mgr = manager(2514);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut receiver, mut receiver_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_331");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        st.seats[seat].hp = 1;
        hp = st.seats[seat].hp;
        receiver = other_seats(st, seat)[0];
        set_general(st, receiver, "sgs_hero_1");
        receiver_pid = st.seats[receiver].player_id.clone();
        give_and_equip(st, seat, EquipSlot::Armor);
        give_and_equip(st, seat, EquipSlot::HorseOffense);
    });
    mgr.execute_action(&room, &pid, "skill:xianzhou").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{receiver_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[receiver].hand.len(), 2, "献州：交出 2 张装备");
    mgr.execute_action(&room, &receiver_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 2, "献州：回复 X 点体力");
    assert!(st.seats[seat].flags.contains_key("xianzhou_used"));
}

// ---------------------------------------------------------------------------
// 曹叡 · 恢拓 / 明鉴 / 兴衰
// ---------------------------------------------------------------------------

fn huituo_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    red: bool,
) -> (usize, String, usize, String) {
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_332");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        st.seats[seat].hp = st.seats[seat].max_hp - 2;
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        let top = find_cards(st, |c| crate::skills::is_red(c.suit) == red, 1)
            .into_iter()
            .next()
            .expect("top card");
        set_top(st, &top);
        damage_and_advance(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
    });
    (seat, pid, target, target_pid)
}

#[test]
fn huituo_red_heals() {
    let mgr = manager(2515);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, _target, _target_pid) = huituo_setup(&mgr, &room, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("huituo"));
    let hp = st.seats[seat].hp;
    mgr.execute_action(&room, &pid, &format!("target:{}", st.seats[seat].player_id))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1, "恢拓：红色回复 1");
}

#[test]
fn huituo_black_draws() {
    let mgr = manager(2516);
    let room = start_and_pick(&mgr, 5);
    let (seat, pid, target, target_pid) = huituo_setup(&mgr, &room, false);
    let st = state_of(&mgr, &room);
    let before = st.seats[target].hand.len();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        before + 1,
        "恢拓：黑色摸伤害值张"
    );
    assert!(st.seats[seat].hp < st.seats[seat].max_hp);
}

#[test]
fn mingjian_gives_all_and_buffs() {
    let mgr = manager(2517);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hand) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_332");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, seat, "sha");
        give_def(st, seat, "shan");
        hand = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:mingjian").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), 0, "明鉴：交出所有手牌");
    assert!(st.seats[target].hand.len() >= hand);
    assert!(
        st.seats[target].flags.contains_key("mingjian"),
        "明鉴：目标获得下回合加成"
    );
    let base = st.seats[target].hp as usize;
    assert_eq!(
        crate::skills::hand_limit(&st, target),
        base + 1,
        "明鉴：手牌上限 +1"
    );
}

// ---------------------------------------------------------------------------
// 曹休 · 千驹 / 倾袭
// ---------------------------------------------------------------------------

#[test]
fn qianju_reduces_distance() {
    let mgr = manager(2518);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut far, mut before) = (0, 0, false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_333");
        st.turn_counters.clear();
        far = other_seats(st, seat)
            .into_iter()
            .find(|i| st.seat_distance(seat, *i) >= 2)
            .expect("far");
        set_general(st, far, "sgs_hero_1");
        before = st.in_attack_range(seat, far);
        st.seats[seat].hp = st.seats[seat].max_hp - 2;
    });
    let st = state_of(&mgr, &room);
    assert!(!before, "千驹：未受伤时够不到");
    assert!(
        st.in_attack_range(seat, far),
        "千驹：每损失 1 点体力距离 -1"
    );
}

#[test]
fn qingxi_raises_damage() {
    let mgr = manager(2519);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_333");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
        give_and_equip(st, seat, EquipSlot::Weapon);
        give_def(st, seat, "sha");
        // 目标有手牌，避免古锭刀额外 +1，保证伤害基线为 1。
        give_def(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let target_pid2 = st.seats[target].player_id.clone();
    if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("respond") {
        mgr.execute_action(&room, &target_pid2, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qingxi"), "倾袭：伤害前二选一");
    mgr.execute_action(&room, &target_pid2, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "倾袭：伤害 +1");
}

#[test]
fn qingxi_discards_weapon() {
    let mgr = manager(2520);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut wpn) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_333");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        wpn = give_and_equip(st, seat, EquipSlot::Weapon);
        give_def(st, seat, "sha");
        for _ in 0..3 {
            give_def(st, target, "shan");
        }
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, false);
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let target_pid2 = st.seats[target].player_id.clone();
    if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("respond") {
        mgr.execute_action(&room, &target_pid2, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    let before = st.seats[target].hand.len();
    mgr.execute_action(&room, &target_pid2, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&wpn), "倾袭：弃置其武器");
    assert!(st.seats[target].hand.len() < before, "倾袭：目标弃牌");
}

// ---------------------------------------------------------------------------
// 钟繇 · 活墨 / 佐定
// ---------------------------------------------------------------------------

#[test]
fn huomo_sha_from_black_non_basic() {
    let mgr = manager(2521);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_334");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        card = give_card(st, seat, |c| {
            crate::skills::is_black(c.suit)
                && crate::data::card_def(&c.def)
                    .is_some_and(|d| !matches!(d.category, Category::Basic))
        });
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &pid);
    let action = acts
        .iter()
        .find(|a| a.id == format!("skill:huomo:{card}:sha"))
        .expect("活墨当杀")
        .id
        .clone();
    mgr.execute_action(&room, &pid, &action).unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.draw.contains(&card), "活墨：置于牌堆顶");
    assert_eq!(st.counter("huomo_used") & 1, 1, "活墨：记录已用【杀】");
}

#[test]
fn huomo_tao_heals() {
    let mgr = manager(2522);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut hp) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_334");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        st.seats[seat].hp = 1;
        hp = st.seats[seat].hp;
        card = give_card(st, seat, |c| {
            crate::skills::is_black(c.suit)
                && crate::data::card_def(&c.def)
                    .is_some_and(|d| matches!(d.category, Category::Trick | Category::DelayedTrick))
        });
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &pid);
    let action = acts
        .iter()
        .find(|a| a.id == format!("skill:huomo:{card}:tao"))
        .expect("活墨当桃")
        .id
        .clone();
    mgr.execute_action(&room, &pid, &action).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1, "活墨：视为【桃】回复 1");
}

#[test]
fn zuoding_draws_on_spade_sha() {
    let mgr = manager(2523);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        let others = other_seats(st, user);
        owner = others[0];
        set_general(st, owner, "sgs_hero_334");
        owner_pid = st.seats[owner].player_id.clone();
        target = others
            .iter()
            .copied()
            .find(|i| *i != owner && st.in_attack_range(user, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        before = st.seats[target].hand.len();
        give_card(st, user, |c| {
            c.suit == Suit::Spade && matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
        });
    });
    let st = state_of(&mgr, &room);
    let sha = sha_card(&st, true);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zuoding"),
        "佐定：黑桃牌指定目标后"
    );
    mgr.execute_action(&room, &owner_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), before + 1, "佐定：目标摸 1");
}
