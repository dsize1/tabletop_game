//! P4.4.2 授予技能补齐：清侧（毌丘俭·鸿举）/ 怀柔（陆抗·破势）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

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

fn equip_card(st: &mut SgsState, seat: usize, def: &str) -> String {
    let card = give_card(st, seat, |c| c.def == def);
    st.seats[seat].hand.retain(|c| c != &card);
    let slot = crate::data::card_def(def)
        .and_then(|d| d.equip_slot)
        .expect("equip slot");
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != cur)
        .collect()
}

// ---------------------------------------------------------------------------
// 清侧
// ---------------------------------------------------------------------------

#[test]
fn qingce_moves_rong_and_discards_enemy_equipment() {
    let mgr = manager(1600);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut enemy, mut cur_pid, mut enemy_pid, mut armor) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        crate::skills::grant_skill(st, cur, "qingce");
        enemy = other_seats(st, cur)[0];
        set_general(st, enemy, "sgs_hero_1");
        armor = equip_card(st, enemy, "bagua_zhen");
        let rong = give_card(st, cur, |c| c.def == "sha");
        st.seats[cur].hand.retain(|c| c != &rong);
        crate::skills::add_mark(st, cur, "rong", &rong);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        enemy_pid = st.seats[enemy].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:qingce").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("qingce")
    );
    mgr.execute_action(&room, &cur_pid, &format!("card:{enemy_pid}:{armor}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.equip_def(enemy, EquipSlot::Armor).map(|d| d.code),
        None,
        "清侧：弃置敌方装备牌"
    );
    assert!(st.deck.discard.contains(&armor));
    assert_eq!(
        crate::skills::mark_cards(&st, cur, "rong").len(),
        0,
        "清侧：移去一张「荣」"
    );
}

#[test]
fn qingce_cannot_target_own_cards() {
    let mgr = manager(1601);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut own_armor, mut cur_pid, mut has_own) =
        (0, String::new(), String::new(), true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        crate::skills::grant_skill(st, cur, "qingce");
        own_armor = equip_card(st, cur, "bagua_zhen");
        let rong = give_card(st, cur, |c| c.def == "shan");
        st.seats[cur].hand.retain(|c| c != &rong);
        crate::skills::add_mark(st, cur, "rong", &rong);
        let enemy = other_seats(st, cur)[0];
        set_general(st, enemy, "sgs_hero_1");
        equip_card(st, enemy, "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:qingce").unwrap();
    let st = state_of(&mgr, &room);
    let own_pid = st.seats[cur].player_id.clone();
    if let Some(p) = &st.pending {
        has_own = p
            .options
            .iter()
            .any(|o| o.id == format!("card:{own_pid}:{own_armor}"));
    }
    assert!(!has_own, "清侧：不能弃置自己区域里的牌");
}

#[test]
fn qingce_discards_delayed_trick() {
    let mgr = manager(1602);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut enemy, mut cur_pid, mut enemy_pid, mut lebu) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        crate::skills::grant_skill(st, cur, "qingce");
        enemy = other_seats(st, cur)[0];
        set_general(st, enemy, "sgs_hero_1");
        lebu = give_card(st, enemy, |c| c.def == "lebu_sishu");
        st.seats[enemy].hand.retain(|c| c != &lebu);
        st.seats[enemy].judge_zone.push(lebu.clone());
        let rong = give_card(st, cur, |c| c.def == "sha");
        st.seats[cur].hand.retain(|c| c != &rong);
        crate::skills::add_mark(st, cur, "rong", &rong);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        enemy_pid = st.seats[enemy].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:qingce").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("card:{enemy_pid}:{lebu}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.seats[enemy].judge_zone.contains(&lebu),
        "清侧：弃置敌方判定区延时锦囊"
    );
    assert!(st.deck.discard.contains(&lebu));
}

// ---------------------------------------------------------------------------
// 怀柔
// ---------------------------------------------------------------------------

#[test]
fn huairou_recasts_equipment_card() {
    let mgr = manager(1610);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut weapon, mut before) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_414");
        crate::skills::grant_skill(st, cur, "huairou");
        weapon = give_card(st, cur, |c| c.def == "qinggang_jian");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        before = st.seats[cur].hand.len();
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter()
            .any(|a| a.id == format!("skill:huairou:{weapon}")),
        "怀柔：装备牌可重铸"
    );
    mgr.execute_action(&room, &cur_pid, &format!("skill:huairou:{weapon}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&weapon), "怀柔：弃置装备牌");
    assert_eq!(
        st.seats[cur].hand.len(),
        before,
        "怀柔：弃置并摸一张（净手牌不变）"
    );
}
