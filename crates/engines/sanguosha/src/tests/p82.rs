//! P4.4.2 界标 25 将批次 2（蜀 7 将）：界刘备 / 界关羽 / 界张飞 / 界赵云 / 界马超 / 界诸葛亮 / 界黄月英。

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

fn equip_card(st: &mut SgsState, seat: usize, def: &str) -> String {
    let card = give_def(st, seat, def);
    st.seats[seat].hand.retain(|c| c != &card);
    let slot = crate::data::card_def(def)
        .and_then(|d| d.equip_slot)
        .expect("equip slot");
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

fn set_deck_top(st: &mut SgsState, card_id: &str) {
    take_card(st, card_id);
    st.deck.draw.push(card_id.to_string());
}

// ---------------------------------------------------------------------------
// 界刘备 · 仁德 / 激将
// ---------------------------------------------------------------------------

#[test]
fn jie_rende_each_target_once_per_phase() {
    let mgr = manager(1100);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_299");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, cur, "sha");
        give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    // 第一次：给同目标 1 张。
    mgr.execute_action(&room, &cur_pid, "skill:rende").unwrap();
    let st = state_of(&mgr, &room);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 第二次：同目标被拒绝。
    mgr.execute_action(&room, &cur_pid, "skill:rende").unwrap();
    let st = state_of(&mgr, &room);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let err = mgr
        .execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("界仁德"),
        "界仁德每名角色每阶段限一次：{err:?}"
    );
}

#[test]
fn jie_rende_second_card_offers_basic_card() {
    let mgr = manager(1101);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_299");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[cur].max_hp = 4;
        st.seats[cur].hp = 3;
        give_def(st, cur, "sha");
        give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:rende").unwrap();
    let st = state_of(&mgr, &room);
    let cards = st.seats[cur].hand.clone();
    for card in &cards {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{card}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应进入仁德·基本牌选择");
    assert_eq!(pending.skill.as_deref(), Some("rende_basic"));
    assert!(
        pending.options.iter().any(|o| o.id == "tao"),
        "界仁德第二张可视为使用基本牌"
    );
    mgr.execute_action(&room, &cur_pid, "tao").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 4, "视为使用【桃】回复 1 点体力");
    assert_eq!(st.counter("rende_basic_offered"), 1);
}

#[test]
fn jie_jijiang_lord_draws_once_per_turn() {
    let mgr = manager(1102);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut ally, mut lord_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.turn;
        set_general(st, lord, "sgs_hero_299");
        ally = other_seats(st, lord)[0];
        set_general(st, ally, "sgs_hero_1");
        st.turn_counters.clear();
        lord_pid = st.seats[lord].player_id.clone();
        crate::resolve_jiexian::jijiang_lord_draw(st, ally);
        crate::resolve_jiexian::jijiang_lord_draw(st, ally);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hand.len(), 1, "界激将每回合限一次摸 1 张");
    assert_eq!(st.counter("jijiang_lord_draw"), 1);
    let _ = lord_pid;
}

// ---------------------------------------------------------------------------
// 界关羽 · 义绝 / 武圣
// ---------------------------------------------------------------------------

#[test]
fn yijue_black_seals_hand_and_skills() {
    let mgr = manager(1110);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_300");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, cur, "shan");
        give_card(st, target, |c| matches!(c.suit, Suit::Spade | Suit::Club));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:yijue").unwrap();
    let st = state_of(&mgr, &room);
    let cost = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter(&format!("nonlocked_lost:{target}")), 1);
    assert_eq!(st.counter(&format!("hand_sealed:{target}")), 1);
    assert_eq!(st.counter(&format!("yijue_heart:{target}")), 1);
    assert!(!crate::skills::has(&st, target, "rende"), "非锁定技失效");
    assert!(st.history.iter().any(|e| e.text.contains("义绝")));
}

#[test]
fn yijue_red_gains_card_and_heals() {
    let mgr = manager(1111);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_300");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, cur, "shan");
        give_card(st, target, |c| {
            matches!(c.suit, Suit::Heart | Suit::Diamond)
        });
        st.seats[target].hp = st.seats[target].max_hp - 1;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, "skill:yijue").unwrap();
    let st = state_of(&mgr, &room);
    let cost = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 1, "红色：获得展示牌");
    assert_eq!(st.seats[target].hp, hp + 1, "红色：令其回复 1 点体力");
}

#[test]
fn jie_wusheng_diamond_sha_has_no_distance() {
    let mgr = manager(1112);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut card, mut jie_top) = (0, String::new(), String::new(), false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_300");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        card = give_card(st, cur, |c| c.suit == Suit::Diamond && c.def != "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("skill:wusheng:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    for f in &st.resolution {
        if let crate::resolve::Frame::Sha(s) = f {
            jie_top = s.no_distance;
        }
    }
    assert!(jie_top, "界武圣：方片牌当【杀】无距离限制");
}

// ---------------------------------------------------------------------------
// 界张飞 · 替身 / 咆哮
// ---------------------------------------------------------------------------

#[test]
fn tishen_heals_all_and_draws_once() {
    let mgr = manager(1120);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut healed, mut second) = (0, 0, true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_301");
        st.seats[cur].hp = 1;
        st.turn_counters.clear();
        healed = crate::resolve_jiexian::tishen_prepare(st, cur)
            .then_some(st.seats[cur].hp)
            .expect("替身应发动");
        // 限定技：再次调用不生效。
        second = crate::resolve_jiexian::tishen_prepare(st, cur);
    });
    let st = state_of(&mgr, &room);
    let max_hp = st.seats[cur].max_hp;
    assert_eq!(healed, max_hp, "替身回复所有体力");
    assert_eq!(
        st.seats[cur].hand.len(),
        (max_hp - 1) as usize,
        "每回复 1 点摸 1 张"
    );
    assert!(!second, "替身为限定技，只发动一次");
}

#[test]
fn jie_paoxiao_bonus_after_dodged_sha() {
    let mgr = manager(1121);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut sha = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_301");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        sha.push(give_def(st, cur, "sha"));
        sha.push(give_def(st, cur, "sha"));
        give_def(st, target, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{}", sha[0]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[target].hand[0].clone();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("paoxiao_bonus"), 1, "被抵消后下一次杀伤害 +1");
    mgr.execute_action(&room, &cur_pid, &format!("play:{}", sha[1]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 目标已无手牌：引擎按公共信息自动放弃响应，直接结算伤害。
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "咆哮：下一次【杀】伤害 +1");
}

#[test]
fn jie_paoxiao_sha_count_unlimited() {
    let mgr = manager(1122);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut sha) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_301");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        sha = give_def(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.set_counter("sha_used", 5);
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::turn::sha_limit(&st, cur),
        u32::MAX,
        "界咆哮（锁定）：使用【杀】无次数限制"
    );
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter().any(|a| a.id == format!("play:{sha}")),
        "已使用 5 张【杀】后仍可使用"
    );
}

// ---------------------------------------------------------------------------
// 界赵云 · 涯角 / 龙胆
// ---------------------------------------------------------------------------

#[test]
fn jie_longdan_tao_as_jiu_and_jiu_as_tao() {
    let mgr = manager(1130);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut tao_card, mut jiu_id) =
        (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_302");
        tao_card = give_def(st, cur, "tao");
        jiu_id = give_def(st, cur, "jiu");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::skills::conversion_for(&st, cur, &jiu_id, "tao").is_some(),
        "界龙胆：【酒】当【桃】"
    );
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter()
            .any(|a| a.id == format!("skill:longdan:{tao_card}")),
        "界龙胆：【桃】当【酒】应出现在合法动作"
    );
    mgr.execute_action(&room, &cur_pid, &format!("skill:longdan:{tao_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiu_used"), 1);
    assert_eq!(st.counter("jiu"), 1);
}

#[test]
fn yajiao_same_type_gives_revealed_card() {
    let mgr = manager(1131);
    let room = start_and_pick(&mgr, 5);
    let (mut zhaoyun, mut zhao_pid, mut top_card, mut triggered) =
        (0, String::new(), String::new(), false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhaoyun = (st.turn + 2) % st.seats.len();
        set_general(st, zhaoyun, "sgs_hero_302");
        let played = give_def(st, zhaoyun, "wuzhong_shengyou");
        top_card = give_def(st, zhaoyun, "guohe_chaiqiao");
        set_deck_top(st, &top_card);
        zhao_pid = st.seats[zhaoyun].player_id.clone();
        st.turn_counters.clear();
        triggered = crate::resolve_jiexian::yajiao_trigger(st, zhaoyun, &played);
        let mut inject = vec![format!("{zhao_pid}=target:{zhao_pid}")];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(triggered, "类型相同：展示牌堆顶并询问是否交出");
    assert!(
        st.seats[zhaoyun].hand.contains(&top_card),
        "涯角：类型相同可令一名角色（含自己）获得展示牌"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("涯角")),
        "应有涯角历史"
    );
}

#[test]
fn yajiao_different_type_discards_target_card() {
    let mgr = manager(1132);
    let room = start_and_pick(&mgr, 5);
    let (mut zhaoyun, mut victim, mut victim_pid, mut victim_cards) = (0, 0, String::new(), 0usize);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let turn = st.turn;
        zhaoyun = (turn + 1) % st.seats.len();
        set_general(st, zhaoyun, "sgs_hero_302");
        victim = turn;
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "sha");
        let played = give_def(st, zhaoyun, "guohe_chaiqiao");
        let top_card = give_def(st, zhaoyun, "shan");
        set_deck_top(st, &top_card);
        victim_pid = st.seats[victim].player_id.clone();
        st.turn_counters.clear();
        let ok = crate::resolve_jiexian::yajiao_trigger(st, zhaoyun, &played);
        assert!(ok, "类型不同：应触发涯角");
        victim_cards = st.seats[victim].hand.len();
        let mut inject = vec![format!(
            "{}=discard:{victim_pid}",
            st.seats[zhaoyun].player_id
        )];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[victim].hand.len()
            + st.seats[victim].judge_zone.len()
            + st.seats[victim].equips.values().flatten().count(),
        victim_cards - 1,
        "涯角：弃置攻击范围包含你的角色一张牌"
    );
    assert!(st.history.iter().any(|e| e.text.contains("涯角")));
    let _ = victim;
}

// ---------------------------------------------------------------------------
// 界马超 · 铁骑
// ---------------------------------------------------------------------------

#[test]
fn jie_tieqi_discard_same_suit_enables_shan() {
    let mgr = manager(1140);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut sha = String::new();
    let mut same_suit = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_303");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        give_def(st, target, "shan");
        same_suit = give_suit(st, target, Suit::Spade);
        let top_card = give_suit(st, target, Suit::Spade);
        set_deck_top(st, &top_card);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("界铁骑应询问弃同花色牌");
    assert_eq!(pending.skill.as_deref(), Some("tieqi"));
    mgr.execute_action(&room, &target_pid, &format!("discard:{same_suit}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[target].hand.iter().find_map(|id| {
        st.cards
            .get(id)
            .filter(|c| c.def == "shan")
            .map(|_| id.clone())
    });
    mgr.execute_action(&room, &target_pid, &format!("respond:{}", shan.unwrap()))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "弃同花色牌后可以使用【闪】");
}

#[test]
fn jie_tieqi_without_discard_blocks_shan() {
    let mgr = manager(1141);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_303");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        give_def(st, target, "shan");
        give_suit(st, target, Suit::Spade);
        let top_card = give_suit(st, target, Suit::Spade);
        set_deck_top(st, &top_card);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "不弃牌则不能使用【闪】");
}

#[test]
fn jie_tieqi_no_same_suit_auto_blocks_shan() {
    let mgr = manager(1142);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_303");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        give_def(st, target, "shan");
        // 判定牌：黑桃；目标手里没有黑桃牌。
        let top_card = give_suit(st, target, Suit::Spade);
        set_deck_top(st, &top_card);
        st.seats[target].hand.retain(|c| c != &top_card);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "无同花色牌：不能使用【闪】");
    assert_eq!(st.counter(&format!("nonlocked_lost:{target}")), 1);
}

// ---------------------------------------------------------------------------
// 界诸葛亮 · 观星
// ---------------------------------------------------------------------------

#[test]
fn jie_guanxing_count_is_five_or_three() {
    let mgr = manager(1150);
    let room = start_and_pick(&mgr, 5);
    let (mut five, mut three, mut std_four) = (0usize, 0usize, 0usize);
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_440");
        five = crate::resolve_zone::guanxing_count(st);
        // 只剩 3 人存活 → 界观星 3 张。
        let others = other_seats(st, cur);
        st.seats[others[0]].alive = false;
        st.seats[others[1]].alive = false;
        three = crate::resolve_zone::guanxing_count(st);
        st.seats[others[0]].alive = true;
        st.seats[others[1]].alive = true;
        // 标准诸葛亮：5 人时 5 张（保持原口径）。
        set_general(st, cur, "sgs_hero_4");
        std_four = crate::resolve_zone::guanxing_count(st);
    });
    assert_eq!(five, 5, "界观星：5 人局 5 张");
    assert_eq!(three, 3, "界观星：存活 <4 时 3 张");
    assert_eq!(std_four, 5, "标准观星口径不变");
}

#[test]
fn jie_guanxing_all_bottom_repeats_at_end() {
    let mgr = manager(1151);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut pushed, mut bottom_flag) = (0, false, 0u32);
    edit(&mgr, &room, |st| {
        let seat = st.turn;
        cur = seat;
        set_general(st, seat, "sgs_hero_440");
        st.turn_phase = TurnPhase::End;
        st.set_counter("guanxing_bottom", 1);
        st.set_counter("guanxing_end", 0);
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        pushed = matches!(
            st.resolution.last(),
            Some(crate::resolve::Frame::Arrange(_))
        );
        bottom_flag = st.counter("guanxing_end");
    });
    assert!(pushed, "界观星：全部置底后结束阶段再次进入观星");
    assert_eq!(bottom_flag, 1);
    let b = state_of(&mgr, &room);
    assert!(crate::skills::has(&b, cur, "guanxing"));
}

#[test]
fn jie_guanxing_prepare_enters_zone() {
    let mgr = manager(1152);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut in_zone) = (0, false);
    edit(&mgr, &room, |st| {
        let seat = st.turn;
        cur = seat;
        set_general(st, seat, "sgs_hero_440");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
        in_zone = st
            .special_zones
            .contains_key(crate::resolve_zone::GUANXING_ZONE);
    });
    assert!(in_zone, "界观星：准备阶段进入观星特殊区");
    let st = state_of(&mgr, &room);
    assert!(crate::skills::has(&st, cur, "guanxing"));
}

// ---------------------------------------------------------------------------
// 界黄月英 · 集智 / 奇才
// ---------------------------------------------------------------------------

#[test]
fn jie_jizhi_discards_drawn_basic_for_hand_limit() {
    let mgr = manager(1160);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut limit_bonus, mut discarded) = (0, 0u32, false);
    edit(&mgr, &room, |st| {
        let seat = st.turn;
        cur = seat;
        set_general(st, seat, "sgs_hero_441");
        let top_card = give_def(st, seat, "sha");
        set_deck_top(st, &top_card);
        st.turn_counters.clear();
        let mut ev = Vec::new();
        crate::skills::on_non_delayed_trick_used(st, seat, &mut ev);
        limit_bonus = st.counter("jizhi_hand_limit");
        discarded = st.deck.discard.contains(&top_card);
    });
    assert_eq!(limit_bonus, 1, "界集智：弃基本牌 → 手牌上限 +1");
    assert!(discarded, "界集智：摸到的基本牌弃置");
    let st = state_of(&mgr, &room);
    let limit = crate::skills::hand_limit(&st, cur);
    assert!(limit >= 1);
}

#[test]
fn jie_qicai_protects_armor_and_treasure() {
    let mgr = manager(1161);
    let room = start_and_pick(&mgr, 5);
    let (mut hjy, mut user, mut user_pid, mut hjy_pid, mut armor) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        hjy = other_seats(st, cur)[0];
        set_general(st, hjy, "sgs_hero_441");
        user = cur;
        set_general(st, user, "sgs_hero_2");
        armor = equip_card(st, hjy, "bagua_zhen");
        give_def(st, hjy, "sha");
        give_def(st, user, "guohe_chaiqiao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        user_pid = st.seats[user].player_id.clone();
        hjy_pid = st.seats[hjy].player_id.clone();
        let st_ref = &*st;
        assert!(crate::skills::qicai_blocks_discard(
            st_ref,
            hjy,
            user,
            EquipSlot::Armor
        ));
        assert!(!crate::skills::qicai_blocks_discard(
            st_ref,
            hjy,
            hjy,
            EquipSlot::Armor
        ));
    });
    let st = state_of(&mgr, &room);
    let guohe = st.seats[user]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "guohe_chaiqiao"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("play:{guohe}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{hjy_pid}"))
        .unwrap();
    // 跳过所有无懈询问，直到拿牌界面。
    for _ in 0..12 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind == "pick_card" {
            assert!(
                !p.options.iter().any(|o| o.id == "pick:equip:armor"),
                "界奇才：防具不能被弃置"
            );
            return;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    panic!("未进入拿牌界面（守护 {}）", armor);
}

#[test]
fn jie_qicai_trick_has_no_distance() {
    let mgr = manager(1162);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far, mut cur_pid) = (0, 0, String::new());
    let mut shunshou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_441");
        let others = other_seats(st, cur);
        far = others[2];
        set_general(st, far, "sgs_hero_1");
        give_def(st, far, "sha");
        shunshou = give_def(st, cur, "shunshou_qianyang");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::skills::trick_distance_unlimited(&st, cur),
        "界奇才（锁定）：使用锦囊牌无距离限制"
    );
    assert!(
        crate::turn::trick_distance_ok(&st, cur, far),
        "距离 2 的【顺手牵羊】也可指定"
    );
    let _ = (cur_pid, shunshou);
}
