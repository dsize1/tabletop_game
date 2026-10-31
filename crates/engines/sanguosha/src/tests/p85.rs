//! P4.4.2 界标 25 将批次 5（群 4 将）：界华佗 / 界吕布 / 界公孙瓒 / 界貂蝉。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, state_of, take_card};
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
// 界华佗 · 急救 / 除疠
// ---------------------------------------------------------------------------

#[test]
fn chuli_discards_and_spade_holders_draw() {
    let mgr = manager(1400);
    let room = start_and_pick(&mgr, 5);
    let (mut hua, mut a, mut b, mut hua_pid, mut a_pid, mut b_pid) =
        (0, 0, 0, String::new(), String::new(), String::new());
    let mut spade = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        hua = st.turn;
        set_general(st, hua, "sgs_hero_318");
        let others = other_seats(st, hua);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1"); // 蜀
        set_general(st, b, "sgs_hero_15"); // 魏
        for seat in others {
            if seat != a && seat != b {
                set_general(st, seat, "sgs_hero_8"); // 吴，未选
            }
        }
        give_suit(st, hua, Suit::Heart);
        give_suit(st, a, Suit::Heart);
        spade = give_suit(st, b, Suit::Spade);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        hua_pid = st.seats[hua].player_id.clone();
        a_pid = st.seats[a].player_id.clone();
        b_pid = st.seats[b].player_id.clone();
    });
    mgr.execute_action(&room, &hua_pid, "skill:chuli").unwrap();
    mgr.execute_action(&room, &hua_pid, &format!("choose:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &hua_pid, &format!("choose:{b_pid}"))
        .unwrap();
    mgr.execute_action(&room, &hua_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[hua].hand.len(), 0, "除疠：你也弃置一张牌");
    assert_eq!(st.seats[a].hand.len(), 0, "除疠：目标弃置一张红桃牌");
    assert_eq!(st.seats[b].hand.len(), 1, "除疠：弃置黑桃牌的角色摸 1 张");
    assert_eq!(st.counter("chuli_used"), 1);
    let _ = spade;
}

#[test]
fn chuli_requires_distinct_kingdoms() {
    let mgr = manager(1401);
    let room = start_and_pick(&mgr, 5);
    let (mut hua, mut a, mut a2, mut hua_pid, mut a_pid, mut a2_pid) =
        (0, 0, 0, String::new(), String::new(), String::new());
    let mut offers_same = true;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        hua = st.turn;
        set_general(st, hua, "sgs_hero_318");
        let others = other_seats(st, hua);
        a = others[0];
        a2 = others[1];
        set_general(st, a, "sgs_hero_1"); // 蜀
        set_general(st, a2, "sgs_hero_2"); // 蜀（同势力）
        for seat in others {
            if seat != a && seat != a2 {
                set_general(st, seat, "sgs_hero_8"); // 吴
            }
        }
        give_suit(st, hua, Suit::Heart);
        give_suit(st, a, Suit::Heart);
        give_suit(st, a2, Suit::Heart);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        hua_pid = st.seats[hua].player_id.clone();
        a_pid = st.seats[a].player_id.clone();
        a2_pid = st.seats[a2].player_id.clone();
    });
    mgr.execute_action(&room, &hua_pid, "skill:chuli").unwrap();
    mgr.execute_action(&room, &hua_pid, &format!("choose:{a_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    if let Some(p) = &st.pending {
        offers_same = p.options.iter().any(|o| o.id == format!("choose:{a2_pid}"));
    }
    assert!(!offers_same, "除疠：同势力角色不可再选");
}

#[test]
fn jijiu_red_card_as_tao_outside_turn() {
    let mgr = manager(1402);
    let room = start_and_pick(&mgr, 5);
    let (mut hua, mut red) = (0, String::new());
    let mut outside = false;
    let mut inside = true;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        hua = st.turn;
        set_general(st, hua, "sgs_hero_318");
        red = give_card(st, hua, |c| matches!(c.suit, Suit::Heart | Suit::Diamond));
        st.turn = (hua + 1) % st.seats.len();
        outside = crate::skills::conversion_for(st, hua, &red, "tao").is_some();
        st.turn = hua;
        inside = crate::skills::conversion_for(st, hua, &red, "tao").is_some();
    });
    assert!(outside, "急救：回合外红色牌可当【桃】");
    assert!(!inside, "急救：自己回合内不可当【桃】");
}

// ---------------------------------------------------------------------------
// 界吕布 · 无双 / 利驭
// ---------------------------------------------------------------------------

#[test]
fn liyu_take_non_equip_makes_target_draw() {
    let mgr = manager(1410);
    let room = start_and_pick(&mgr, 5);
    let (mut lb, mut target, mut lb_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lb = st.turn;
        set_general(st, lb, "sgs_hero_319");
        target = neighbor(st, lb);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, lb, "sha");
        give_def(st, target, "tao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        lb_pid = st.seats[lb].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &lb_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &lb_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("liyu")
    );
    mgr.execute_action(&room, &lb_pid, "choose:liyu").unwrap();
    mgr.execute_action(&room, &lb_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lb].hand.len(), 1, "利驭：获得其一张手牌");
    assert_eq!(
        st.seats[target].hand.len(),
        1,
        "利驭：获得的不是装备牌，其摸 1 张"
    );
}

#[test]
fn liyu_take_equip_triggers_duel() {
    let mgr = manager(1411);
    let room = start_and_pick(&mgr, 5);
    let (mut lb, mut target, mut third, mut lb_pid, mut target_pid, mut third_pid, mut sha) = (
        0,
        0,
        0,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lb = st.turn;
        set_general(st, lb, "sgs_hero_319");
        target = neighbor(st, lb);
        third = (target + 1) % st.seats.len();
        set_general(st, target, "sgs_hero_1");
        set_general(st, third, "sgs_hero_2");
        sha = give_def(st, lb, "sha");
        equip_card(st, target, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        lb_pid = st.seats[lb].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        third_pid = st.seats[third].player_id.clone();
    });
    mgr.execute_action(&room, &lb_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &lb_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &lb_pid, "choose:liyu").unwrap();
    mgr.execute_action(&room, &lb_pid, "pick:equip:weapon")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("liyu"),
        "利驭：获得装备牌后由目标指定决斗对象"
    );
    let third_hp = state_of(&mgr, &room).seats[third].hp;
    mgr.execute_action(&room, &lb_pid, &format!("target:{third_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.equip_def(target, EquipSlot::Weapon).map(|d| d.code),
        None,
        "利驭：装备被获得"
    );
    assert_eq!(
        st.seats[third].hp,
        third_hp - 1,
        "利驭：视为使用【决斗】，目标无【杀】受伤"
    );
    assert!(st.history.iter().any(|e| e.text.contains("利驭")));
}

#[test]
fn wushuang_sha_needs_two_shan() {
    let mgr = manager(1412);
    let room = start_and_pick(&mgr, 5);
    let (mut lb, mut target, mut lb_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lb = st.turn;
        set_general(st, lb, "sgs_hero_319");
        target = neighbor(st, lb);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, lb, "sha");
        give_def(st, target, "shan");
        give_def(st, target, "tao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        lb_pid = st.seats[lb].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &lb_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &lb_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[target]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "shan"))
        .cloned()
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("respond"),
        "无双：还需再打出 1 张【闪】"
    );
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "无双：仅 1 张【闪】不足抵消");
}

// ---------------------------------------------------------------------------
// 界公孙瓒 · 义从 / 趫猛
// ---------------------------------------------------------------------------

#[test]
fn yicong_attack_distance_reduced() {
    let mgr = manager(1420);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far, mut with_yicong) = (0, 0, false);
    let mut without = true;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = (cur + 2) % st.seats.len();
        set_general(st, far, "sgs_hero_1");
        without = st.in_attack_range(cur, far);
        set_general(st, cur, "sgs_hero_320");
        with_yicong = st.in_attack_range(cur, far);
    });
    assert!(!without, "基础攻击范围 1 时距离 2 不可达");
    assert!(with_yicong, "义从：计算与其他角色距离 -1（距离 2 可达）");
}

#[test]
fn yicong_defense_when_hp_low() {
    let mgr = manager(1421);
    let room = start_and_pick(&mgr, 5);
    let (mut gsz, mut attacker) = (0, 0);
    let mut low_hp = true;
    let mut high_hp = false;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        gsz = (st.turn + 1) % st.seats.len();
        set_general(st, gsz, "sgs_hero_320");
        attacker = neighbor(st, gsz);
        st.seats[gsz].hp = 2;
        low_hp = st.in_attack_range(attacker, gsz);
        st.seats[gsz].hp = 3;
        high_hp = st.in_attack_range(attacker, gsz);
    });
    assert!(!low_hp, "义从：体力 ≤2 时其他角色与你的距离 +1");
    assert!(high_hp, "义从：体力 >2 时无防御距离加成");
}

#[test]
fn qiaomeng_gains_mount_after_sha_damage() {
    let mgr = manager(1422);
    let room = start_and_pick(&mgr, 5);
    let (mut gsz, mut target, mut gsz_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    let mut mount = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        gsz = st.turn;
        set_general(st, gsz, "sgs_hero_320");
        target = neighbor(st, gsz);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, gsz, "sha");
        mount = equip_card(st, target, "chitu");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        gsz_pid = st.seats[gsz].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &gsz_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &gsz_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[gsz].hand.iter().any(|c| c == &mount),
        "趫猛：弃置坐骑牌后获得之"
    );
    assert_eq!(
        st.equip_def(target, EquipSlot::HorseOffense)
            .map(|d| d.code),
        None
    );
}

// ---------------------------------------------------------------------------
// 界貂蝉 · 离间 / 闭月
// ---------------------------------------------------------------------------

#[test]
fn biyue_jie_empty_hand_draws_two() {
    let mgr = manager(1430);
    let room = start_and_pick(&mgr, 5);
    let mut drawn = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let seat = st.turn;
        set_general(st, seat, "sgs_hero_445");
        st.turn_counters.clear();
        st.set_counter("skip_draw", 0);
        let before = st.seats[seat].hand.len();
        let mut ev = Vec::new();
        crate::skills::on_phase_start(st, TurnPhase::End, &mut ev);
        drawn = st.seats[seat].hand.len() - before;
    });
    assert_eq!(drawn, 2, "界闭月：没有手牌摸 2 张");
}

#[test]
fn biyue_jie_with_hand_draws_one() {
    let mgr = manager(1431);
    let room = start_and_pick(&mgr, 5);
    let mut drawn = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let seat = st.turn;
        set_general(st, seat, "sgs_hero_445");
        give_def(st, seat, "sha");
        st.turn_counters.clear();
        let before = st.seats[seat].hand.len();
        let mut ev = Vec::new();
        crate::skills::on_phase_start(st, TurnPhase::End, &mut ev);
        drawn = st.seats[seat].hand.len() - before;
    });
    assert_eq!(drawn, 1, "界闭月：有手牌摸 1 张");
}

#[test]
fn lijian_jie_discards_equip_and_skips_wuxie() {
    let mgr = manager(1432);
    let room = start_and_pick(&mgr, 5);
    let (mut dc, mut male_a, mut male_b, mut dc_pid) = (0, 0, 0, String::new());
    let mut pids = (String::new(), String::new());
    let mut weapon = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dc = st.turn;
        set_general(st, dc, "sgs_hero_445");
        let others = other_seats(st, dc);
        male_a = others[0];
        male_b = others[1];
        set_general(st, male_a, "sgs_hero_1");
        set_general(st, male_b, "sgs_hero_2");
        weapon = equip_card(st, dc, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        dc_pid = st.seats[dc].player_id.clone();
        pids = (
            st.seats[male_a].player_id.clone(),
            st.seats[male_b].player_id.clone(),
        );
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &dc_pid);
    assert!(
        acts.iter().any(|a| a.id == "skill:lijian"),
        "界离间：无手牌有装备也可发动"
    );
    mgr.execute_action(&room, &dc_pid, "skill:lijian").unwrap();
    mgr.execute_action(&room, &dc_pid, &format!("choose:{weapon}"))
        .unwrap();
    mgr.execute_action(&room, &dc_pid, "confirm").unwrap();
    mgr.execute_action(&room, &dc_pid, &format!("target:{}", pids.0))
        .unwrap();
    let defender_hp = state_of(&mgr, &room).seats[male_b].hp;
    mgr.execute_action(&room, &dc_pid, &format!("target:{}", pids.1))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.equip_def(dc, EquipSlot::Weapon).map(|d| d.code),
        None,
        "界离间：弃置装备牌"
    );
    assert!(st.deck.discard.contains(&weapon));
    assert_eq!(
        st.seats[male_b].hp,
        defender_hp - 1,
        "界离间：视为决斗结算（无无懈窗口），无【杀】则受伤"
    );
    assert!(!st.history.iter().any(|e| e.text.contains("无懈")));
    let _ = male_a;
}
