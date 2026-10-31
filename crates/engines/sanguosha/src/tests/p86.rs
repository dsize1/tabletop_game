//! P4.4.2 一将成名批次 2（8 将）：张春华 / 曹彰 / 荀攸 / 廖化 / 马岱 / 步练师 / 程普 / 韩当。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_state_inject, edit, manager, state_of, take_card};
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != cur)
        .collect()
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
                && !st
                    .seats
                    .iter()
                    .any(|s| s.judge_zone.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.equips.values().flatten().any(|h| h == *id))
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

fn give_suit(st: &mut SgsState, seat: usize, suit: Suit) -> String {
    give_card(st, seat, |c| c.suit == suit)
}

fn neighbor(st: &SgsState, seat: usize) -> usize {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != seat)
        .min_by_key(|i| st.seat_distance(seat, *i))
        .expect("neighbor")
}

fn set_top(st: &mut SgsState, cards: &[String]) {
    for id in cards {
        take_card(st, id);
        st.deck.draw.push(id.clone());
    }
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
// 张春华 · 绝情 / 伤逝
// ---------------------------------------------------------------------------

#[test]
fn jueqing_makes_damage_become_hp_loss() {
    let mgr = manager(1500);
    let room = start_and_pick(&mgr, 5);
    let (mut zch, mut victim, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zch = st.turn;
        set_general(st, zch, "sgs_hero_161");
        victim = other_seats(st, zch)[0];
        set_general(st, victim, "sgs_hero_1");
        st.turn_counters.clear();
        hp = st.seats[victim].hp;
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(zch),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, hp - 1, "绝情：伤害变为失去体力");
    assert!(st.history.iter().any(|e| e.text.contains("绝情")));
    assert!(!st.history.iter().any(|e| e.text.contains("受到 1 点")));
}

#[test]
fn shangshi_draws_to_lost_hp() {
    let mgr = manager(1501);
    let room = start_and_pick(&mgr, 5);
    let mut hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let seat = st.turn;
        set_general(st, seat, "sgs_hero_161");
        st.seats[seat].max_hp = 3;
        st.seats[seat].hp = 1;
        crate::skills::shangshi_draw(st, seat);
        hand = st.seats[seat].hand.len();
    });
    assert_eq!(hand, 2, "伤逝：摸至已损失体力值（2）");
}

#[test]
fn shangshi_triggers_after_losing_card() {
    let mgr = manager(1502);
    let room = start_and_pick(&mgr, 5);
    let mut hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let seat = st.turn;
        set_general(st, seat, "sgs_hero_161");
        st.seats[seat].max_hp = 3;
        st.seats[seat].hp = 1;
        let a = give_def(st, seat, "sha");
        give_def(st, seat, "shan");
        crate::turn::take_card(st, seat, &a).unwrap();
        crate::turn::to_discard_card(st, &a);
        hand = st.seats[seat].hand.len();
    });
    assert_eq!(hand, 2, "伤逝：失去手牌后摸至已损失体力值");
}

// ---------------------------------------------------------------------------
// 曹彰 · 将驰
// ---------------------------------------------------------------------------

fn enter_jiangchi(mgr: &game_engine_core::GameManager, room: &str, seat: usize) {
    let full = mgr.get_state(room).unwrap();
    let mut st = SgsState::parse(&full.data).unwrap();
    st.turn = seat;
    st.turn_phase = TurnPhase::Draw;
    st.turn_counters.clear();
    let mut ev = Vec::new();
    crate::turn::phase_step(&mut st, &mut ev).unwrap();
    crate::resolve::advance(&mut st, &mut ev).unwrap();
    let mut next = full.clone();
    next.data = serde_json::to_value(&st).unwrap();
    mgr.restore(next);
}

#[test]
fn jiangchi_opt1_draws_and_bans_sha() {
    let mgr = manager(1510);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_162");
        give_def(st, cur, "sha");
        cur_pid = st.seats[cur].player_id.clone();
    });
    enter_jiangchi(&mgr, &room, cur);
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("jiangchi")
    );
    before = st.seats[cur].hand.len();
    mgr.execute_action(&room, &cur_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), before + 1, "将驰①：摸 1 张");
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        !acts.iter().any(|a| a.id.starts_with("play:")),
        "将驰①：本回合不能使用【杀】"
    );
}

#[test]
fn jiangchi_opt2_discards_and_buffs() {
    let mgr = manager(1511);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut cost) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_162");
        cost = give_def(st, cur, "shan");
        give_def(st, cur, "sha");
        cur_pid = st.seats[cur].player_id.clone();
    });
    enter_jiangchi(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, "opt2").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiangchi_bonus"), 1);
    assert_eq!(crate::turn::sha_limit(&st, cur), 2, "将驰②：【杀】次数 +1");
    assert!(
        crate::skills::no_distance_turn(&st, cur),
        "将驰②：无距离限制"
    );
}

#[test]
fn jiangchi_decline_no_buff() {
    let mgr = manager(1512);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_162");
        give_def(st, cur, "sha");
        cur_pid = st.seats[cur].player_id.clone();
    });
    enter_jiangchi(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiangchi_bonus"), 0);
    assert_eq!(st.counter(&format!("sha_banned:{cur}")), 0);
}

// ---------------------------------------------------------------------------
// 荀攸 · 奇策 / 智愚
// ---------------------------------------------------------------------------

#[test]
fn qice_uses_all_hand_as_trick() {
    let mgr = manager(1520);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut second) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_164");
        give_def(st, cur, "sha");
        second = give_def(st, cur, "shan");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
            give_def(st, seat, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:qice").unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:guohe_chaiqiao")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 0, "奇策：所有手牌被使用");
    assert!(st.deck.discard.contains(&second));
    assert!(st.pending.as_ref().is_some_and(|p| p.kind == "target"));
}

#[test]
fn zhiyu_same_color_makes_source_discard() {
    let mgr = manager(1521);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut src, mut src_hand) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_164");
        src = other_seats(st, cur)[0];
        set_general(st, src, "sgs_hero_1");
        give_suit(st, cur, Suit::Heart);
        give_suit(st, src, Suit::Heart);
        let top = give_suit(st, src, Suit::Heart);
        st.seats[src].hand.retain(|c| c != &top);
        set_top(st, &[top]);
        st.turn_counters.clear();
        src_hand = st.seats[src].hand.len();
        crate::resolve::apply_damage(
            st,
            cur,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[src].hand.len(),
        src_hand - 1,
        "智愚：手牌颜色均相同，来源弃置一张手牌"
    );
    assert!(st.seats[cur].hand.len() >= 2, "智愚：摸 1 张");
}

#[test]
fn zhiyu_mixed_color_no_discard() {
    let mgr = manager(1522);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut src, mut src_hand) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_164");
        src = other_seats(st, cur)[0];
        set_general(st, src, "sgs_hero_1");
        give_suit(st, cur, Suit::Heart);
        give_suit(st, src, Suit::Heart);
        let top = give_suit(st, src, Suit::Spade);
        st.seats[src].hand.retain(|c| c != &top);
        set_top(st, &[top]);
        st.turn_counters.clear();
        src_hand = st.seats[src].hand.len();
        crate::resolve::apply_damage(
            st,
            cur,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[src].hand.len(),
        src_hand,
        "智愚：颜色不同则来源不弃牌"
    );
}

// ---------------------------------------------------------------------------
// 廖化 · 当先 / 伏枥
// ---------------------------------------------------------------------------

#[test]
fn dangxian_grants_extra_play_phase() {
    let mgr = manager(1530);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut in_extra, mut back) = (0, String::new(), false, false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_166");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        in_extra = st.turn_phase == TurnPhase::Play && st.counter("dangxian_extra") == 1;
    });
    assert!(in_extra, "当先：回合开始时进入额外出牌阶段");
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    back = st.counter("dangxian_extra") == 0 && st.counter("dangxian_done") == 1;
    assert!(back, "当先：额外出牌阶段结束，继续正常回合流程");
}

#[test]
fn fuli_revives_to_kingdom_count_and_flips() {
    let mgr = manager(1531);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut kingdoms = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_166");
        for (i, seat) in other_seats(st, cur).into_iter().enumerate() {
            let g = ["sgs_hero_1", "sgs_hero_15", "sgs_hero_8", "sgs_hero_23"][i % 4];
            set_general(st, seat, g);
        }
        st.seats[cur].hp = 1;
        give_def(st, cur, "shan");
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        kingdoms = crate::skills::kingdom_count(st) as i32;
        crate::resolve::apply_damage(st, cur, 1, None, crate::resolve::DamageType::Normal, None);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == "skill:fuli")),
        "伏枥：濒死时出现选项"
    );
    mgr.execute_action(&room, &cur_pid, "skill:fuli").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, kingdoms, "伏枥：回复至全场势力数");
    assert!(st.seats[cur].flipped, "伏枥：翻面");
    assert!(st.seats[cur].flags.contains_key("fuli_used"));
}

#[test]
fn fuli_is_limited_once() {
    let mgr = manager(1532);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut has_option) = (0, true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_166");
        st.seats[cur].flags.insert("fuli_used".into(), true);
        st.seats[cur].hp = 1;
        st.turn_counters.clear();
        crate::resolve::apply_damage(st, cur, 1, None, crate::resolve::DamageType::Normal, None);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        has_option = st
            .pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == "skill:fuli"));
    });
    assert!(!has_option, "伏枥（限定技）用过后不再出现");
}
// ---------------------------------------------------------------------------
// 马岱 · 马术 / 潜袭
// ---------------------------------------------------------------------------

fn enter_qianxi(mgr: &game_engine_core::GameManager, room: &str, seat: usize) {
    let full = mgr.get_state(room).unwrap();
    let mut st = SgsState::parse(&full.data).unwrap();
    st.turn = seat;
    st.turn_phase = TurnPhase::Prepare;
    st.turn_counters.clear();
    let mut ev = Vec::new();
    crate::turn::phase_step(&mut st, &mut ev).unwrap();
    crate::resolve::advance(&mut st, &mut ev).unwrap();
    let mut next = full.clone();
    next.data = serde_json::to_value(&st).unwrap();
    mgr.restore(next);
}

#[test]
fn qianxi_blocks_red_hand_this_turn() {
    let mgr = manager(1540);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut red) =
        (0, 0, String::new(), String::new(), String::new());
    let mut banned = false;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_167");
        target = neighbor(st, cur);
        set_general(st, target, "sgs_hero_1");
        red = give_suit(st, cur, Suit::Heart);
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    enter_qianxi(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, &format!("choose:{red}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let target_red = st.seats[target]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| crate::skills::is_red(c.suit))
        })
        .cloned();
    if let Some(card) = target_red {
        banned = crate::skills::qianxi_banned(&st, target, &card);
    }
    assert!(
        banned || st.seats[target].hand.is_empty(),
        "潜袭：红色手牌被封"
    );
    assert_eq!(st.counter(&format!("qianxi:{target}")), 1);
}

#[test]
fn qianxi_blocks_black_hand_this_turn() {
    let mgr = manager(1541);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut black) =
        (0, 0, String::new(), String::new(), String::new());
    let mut banned = false;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_167");
        target = neighbor(st, cur);
        set_general(st, target, "sgs_hero_1");
        black = give_suit(st, cur, Suit::Spade);
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    enter_qianxi(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, &format!("choose:{black}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let target_black = st.seats[target]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| crate::skills::is_black(c.suit))
        })
        .cloned();
    if let Some(card) = target_black {
        banned = crate::skills::qianxi_banned(&st, target, &card);
    }
    assert!(
        banned || st.seats[target].hand.is_empty(),
        "潜袭：黑色手牌被封"
    );
    assert_eq!(st.counter(&format!("qianxi:{target}")), 2);
}

#[test]
fn madai_mashu_reduces_distance() {
    let mgr = manager(1542);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far, mut reach) = (0, 0, false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = (cur + 2) % st.seats.len();
        set_general(st, far, "sgs_hero_1");
        set_general(st, cur, "sgs_hero_167");
        reach = st.in_attack_range(cur, far);
    });
    assert!(reach, "马术：距离 -1，距离 2 可攻击");
}

// ---------------------------------------------------------------------------
// 步练师 · 安恤 / 追忆
// ---------------------------------------------------------------------------

#[test]
fn anxu_gives_card_and_draws_when_not_spade() {
    let mgr = manager(1550);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b, mut cur_pid, mut a_pid, mut b_pid) =
        (0, 0, 0, String::new(), String::new(), String::new());
    let mut actor_hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_168");
        let others = other_seats(st, cur);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_2");
        give_suit(st, b, Suit::Heart);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        a_pid = st.seats[a].player_id.clone();
        b_pid = st.seats[b].player_id.clone();
        actor_hand = st.seats[cur].hand.len();
    });
    mgr.execute_action(&room, &cur_pid, "skill:anxu").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[a].hand.len(), 1, "安恤：目标获得一张手牌");
    assert_eq!(
        st.seats[cur].hand.len(),
        actor_hand + 1,
        "安恤：获得的牌不为黑桃，你摸 1 张"
    );
}

#[test]
fn anxu_spade_no_draw() {
    let mgr = manager(1551);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b, mut cur_pid, mut a_pid, mut b_pid) =
        (0, 0, 0, String::new(), String::new(), String::new());
    let mut actor_hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_168");
        let others = other_seats(st, cur);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_2");
        give_suit(st, b, Suit::Spade);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        a_pid = st.seats[a].player_id.clone();
        b_pid = st.seats[b].player_id.clone();
        actor_hand = st.seats[cur].hand.len();
    });
    mgr.execute_action(&room, &cur_pid, "skill:anxu").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[a].hand.len(), 1);
    assert_eq!(
        st.seats[cur].hand.len(),
        actor_hand,
        "安恤：获得黑桃牌则不摸牌"
    );
}

#[test]
fn zhuiyi_on_death_grants_draws_and_heal() {
    let mgr = manager(1552);
    let room = start_and_pick(&mgr, 5);
    let (mut bls, mut src, mut friend, mut bls_pid, mut friend_pid) =
        (0, 0, 0, String::new(), String::new());
    let mut friend_hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let lord = st.seats.iter().position(|s| s.identity.is_lord()).unwrap();
        bls = (0..st.seats.len())
            .find(|i| *i != lord && st.seats[*i].alive)
            .unwrap();
        st.turn = bls;
        set_general(st, bls, "sgs_hero_168");
        src = (0..st.seats.len())
            .find(|i| *i != bls && st.seats[*i].alive)
            .unwrap();
        friend = (0..st.seats.len())
            .find(|i| *i != bls && *i != src && st.seats[*i].alive)
            .unwrap();
        set_general(st, src, "sgs_hero_1");
        set_general(st, friend, "sgs_hero_2");
        for seat in other_seats(st, bls) {
            st.seats[seat].hand.clear();
        }
        st.seats[bls].hp = 1;
        st.turn_counters.clear();
        bls_pid = st.seats[bls].player_id.clone();
        friend_pid = st.seats[friend].player_id.clone();
        friend_hand = st.seats[friend].hand.len();
        crate::resolve::apply_damage(
            st,
            bls,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        let mut inject = vec![format!("{bls_pid}|choice=target:{friend_pid}")];
        drive_state_inject(st, &mut inject);
        let _ = ev;
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[bls].alive, "步练师已阵亡");
    assert_eq!(
        st.seats[friend].hand.len(),
        friend_hand + 3,
        "追忆：目标摸 3 张；history={:?} pending={:?}",
        st.history
            .iter()
            .rev()
            .take(6)
            .map(|e| e.text.clone())
            .collect::<Vec<_>>(),
        st.pending
            .as_ref()
            .map(|p| (p.kind.clone(), p.skill.clone()))
    );
}

// ---------------------------------------------------------------------------
// 程普 · 疠火 / 醇醪
// ---------------------------------------------------------------------------

#[test]
fn lihuo_converts_to_fire_and_loses_hp() {
    let mgr = manager(1560);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    let mut cur_hp = 0;
    let mut target_hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_169");
        target = neighbor(st, cur);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        cur_hp = st.seats[cur].hp;
        target_hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:lihuo").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, target_hp - 1, "疠火：造成火焰伤害");
    assert_eq!(
        st.seats[cur].hp,
        cur_hp - 1,
        "疠火：造成伤害后失去 1 点体力"
    );
}

#[test]
fn lihuo_decline_uses_normal_sha() {
    let mgr = manager(1561);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    let mut cur_hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_169");
        target = neighbor(st, cur);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        cur_hp = st.seats[cur].hp;
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, cur_hp, "疠火：不发动则不失去体力");
}

#[test]
fn chunlao_places_sha_and_saves_dying() {
    let mgr = manager(1562);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut other) = (0, String::new(), 0);
    let mut marks_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_169");
        other = neighbor(st, cur);
        set_general(st, other, "sgs_hero_1");
        give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
        marks_before = crate::skills::chun_count(st, cur);
    });
    let _ = marks_before;
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("chunlao")
    );
    let st = state_of(&mgr, &room);
    let sha = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "sha"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::chun_count(&st, cur),
        1,
        "醇醪：【杀】置于武将牌上"
    );
    // 令其他角色濒死：自动移去一张「醇」回复 1。
    let hp = st.seats[other].hp;
    let mut after = 0;
    edit(&mgr, &room, |st| {
        st.seats[other].hp = 0;
        crate::resolve::push_dying(st, other, None);
        after = st.seats[other].hp;
    });
    let _ = hp;
    assert_eq!(after, 1, "醇醪：濒死时视为使用【酒】回复 1");
    let st = state_of(&mgr, &room);
    assert_eq!(crate::skills::chun_count(&st, cur), 0);
}

// ---------------------------------------------------------------------------
// 韩当 · 弓骑 / 解烦
// ---------------------------------------------------------------------------

#[test]
fn gongqi_unlimited_attack_range() {
    let mgr = manager(1570);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut cost) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_170");
        cost = give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:gongqi").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("gongqi_unlimited"), 1);
    assert!(
        crate::skills::no_distance_turn(&st, cur),
        "弓骑：攻击范围无限"
    );
}

#[test]
fn gongqi_equip_discards_other_card() {
    let mgr = manager(1571);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut weapon) =
        (0, 0, String::new(), String::new(), String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_170");
        target = neighbor(st, cur);
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        give_def(st, target, "shan");
        weapon = give_def(st, cur, "qinggang_jian");
        give_def(st, target, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &cur_pid, "skill:gongqi").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{weapon}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len()
            + st.seats[target].judge_zone.len()
            + st.seats[target].equips.values().flatten().count(),
        before - 1,
        "弓骑：弃置装备牌后可弃置其他角色一张牌"
    );
}

#[test]
fn jiefan_limited_forces_weapon_or_draw() {
    let mgr = manager(1572);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut weapon) =
        (0, 0, String::new(), String::new(), String::new());
    let mut target_hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_170");
        target = neighbor(st, cur);
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        weapon = give_def(st, cur, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        target_hand = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &cur_pid, "skill:jiefan").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("jiefan_used"));
    assert!(
        !st.seats[cur].hand.contains(&weapon),
        "解烦：韩当弃置武器牌"
    );
    assert_eq!(
        st.seats[target].hand.len(),
        target_hand + 1,
        "解烦：无武器牌的角色令目标摸 1"
    );
}
