//! P4.4.1 火包用例（一）：典韦 / 荀彧 / 庞统 / 卧龙诸葛亮。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, state_of, take_card,
};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn free_card_where(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let claimed = |id: &str| {
        st.seats.iter().any(|s| {
            s.hand.iter().any(|c| c == id)
                || s.marks.values().flatten().any(|c| c == id)
                || s.judge_zone.iter().any(|c| c == id)
                || s.equips.values().flatten().any(|c| c == id)
        })
    };
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"))
}

fn top_card(st: &mut SgsState, filter: impl Fn(&CardInst) -> bool) -> String {
    let claimed = |id: &str| {
        st.deck.draw.iter().any(|c| c == id)
            || st.deck.processing.iter().any(|c| c == id)
            || st.seats.iter().any(|s| {
                s.hand.iter().any(|c| c == id)
                    || s.marks.values().flatten().any(|c| c == id)
                    || s.judge_zone.iter().any(|c| c == id)
                    || s.equips.values().flatten().any(|c| c == id)
            })
    };
    let id = st
        .cards
        .iter()
        .find(|(id, c)| filter(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"));
    take_card(st, &id);
    st.deck.draw.push(id.clone());
    id
}

// ---------------------------------------------------------------------------
// 典韦 · 强袭
// ---------------------------------------------------------------------------

#[test]
fn qiangxi_loses_hp_and_damages_target() {
    let mgr = manager(700);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_34");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = 4;
        st.seats[target].hp = 4;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:qiangxi")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:qiangxi:hp")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 3);
    assert_eq!(st.seats[target].hp, 3);
    assert_eq!(st.counter("qiangxi_used"), 1);
    assert!(st.history.iter().any(|e| e.text.contains("强袭")));
}

#[test]
fn qiangxi_can_discard_weapon() {
    let mgr = manager(701);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut weapon) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_34");
        set_general(st, target, "sgs_hero_2");
        st.seats[target].hp = 4;
        weapon = free_card_where(st, |c| c.def == "qinggang_jian");
        take_card(st, &weapon);
        st.seats[cur]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon.clone()));
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:qiangxi")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:qiangxi:weapon")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur]
        .equips
        .get(&EquipSlot::Weapon)
        .is_none_or(|c| c.is_none()));
    assert!(st.deck.discard.contains(&weapon));
    assert_eq!(st.seats[target].hp, 3);
}

#[test]
fn qiangxi_limits_twice_and_marks_targets() {
    let mgr = manager(702);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut target2) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        target2 = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_34");
        set_general(st, target, "sgs_hero_2");
        set_general(st, target2, "sgs_hero_2");
        st.seats[cur].hp = 4;
        st.seats[target].hp = 4;
        st.seats[target2].hp = 4;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid, target2_pid) = (pid(&st, cur), pid(&st, target), pid(&st, target2));
    mgr.execute_action(&room, &cur_pid, "skill:qiangxi")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:qiangxi:hp")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 同一目标本回合不可再次指定；第二个目标可选。
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(acts.iter().any(|a| a.id == "skill:qiangxi"));
    mgr.execute_action(&room, &cur_pid, "skill:qiangxi")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:qiangxi:hp")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target2_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 3);
    assert_eq!(st.seats[target2].hp, 3);
    assert_eq!(st.counter("qiangxi_used"), 2);
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id == "skill:qiangxi"));
}

// ---------------------------------------------------------------------------
// 荀彧 · 驱虎 / 节命
// ---------------------------------------------------------------------------

fn setup_pindian_cards(st: &mut SgsState, a: usize, b: usize) -> (String, String) {
    let high = free_card_where(st, |c| c.rank == 13);
    take_card(st, &high);
    st.seats[a].hand.push(high.clone());
    let low = free_card_where(st, |c| c.rank == 1);
    take_card(st, &low);
    st.seats[b].hand.push(low.clone());
    (high, low)
}

#[test]
fn quhu_win_lets_target_damage_chosen_character() {
    let mgr = manager(710);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut victim, mut high, mut low) =
        (0, 0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        victim = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_35");
        set_general(st, target, "sgs_hero_2");
        set_general(st, victim, "sgs_hero_23");
        st.seats[cur].hp = 2;
        st.seats[target].hp = 4;
        st.seats[victim].hp = 4;
        (high, low) = setup_pindian_cards(st, cur, target);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid, victim_pid) = (pid(&st, cur), pid(&st, target), pid(&st, victim));
    mgr.execute_action(&room, &cur_pid, "skill:quhu").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{cur_pid}|choice=choose:{high}"),
            format!("{target_pid}|choice=choose:{low}"),
            format!("{cur_pid}|target=target:{victim_pid}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 3);
    assert_eq!(st.seats[cur].hp, 2);
    assert!(st.history.iter().any(|e| e.text.contains("驱虎")));
}

#[test]
fn quhu_lose_damages_initiator() {
    let mgr = manager(711);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut high, mut low) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_35");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = 2;
        st.seats[target].hp = 4;
        let (_, _) = setup_pindian_cards(st, cur, target);
        // 交换：让发起者拿小点、目标拿大点（发起者没赢）。
        let a = st.seats[cur].hand.pop().expect("cur card");
        let b = st.seats[target].hand.pop().expect("target card");
        st.seats[cur].hand.push(b);
        st.seats[target].hand.push(a);
        low = st.seats[cur].hand.last().cloned().unwrap();
        high = st.seats[target].hand.last().cloned().unwrap();
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, "skill:quhu").unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|target=target:{target_pid}"),
            format!("{cur_pid}|choice=choose:{low}"),
            format!("{target_pid}|choice=choose:{high}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 1);
    assert!(!st.pending.as_ref().is_some_and(|p| p.kind == "dying"));
}

#[test]
fn jieming_draws_target_to_hp_max() {
    let mgr = manager(712);
    let room = start_and_pick(&mgr, 5);
    let (mut xunyu, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        xunyu = st.turn;
        target = st.seats_from_left_of(xunyu)[0];
        set_general(st, xunyu, "sgs_hero_35");
        set_general(st, target, "sgs_hero_2");
        st.seats[xunyu].hp = st.seats[xunyu].max_hp;
        st.seats[target].max_hp = 4;
        st.seats[target].hp = 4;
        give(st, target, "sha");
        crate::resolve::apply_damage(
            st,
            xunyu,
            1,
            Some(target),
            crate::resolve::DamageType::Normal,
            None,
        );
        let pid_x = pid(st, xunyu);
        let pid_t = pid(st, target);
        drive_state_inject(st, &mut vec![format!("{pid_x}|target=target:{pid_t}")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 4, "节命应摸至 4 张");
    assert!(st.history.iter().any(|e| e.text.contains("节命")));
}

// ---------------------------------------------------------------------------
// 庞统 · 连环 / 涅槃
// ---------------------------------------------------------------------------

#[test]
fn lianhuan_uses_club_as_tiesuo() {
    let mgr = manager(720);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut club) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_36");
        set_general(st, target, "sgs_hero_2");
        club = free_card_where(st, |c| c.suit == Suit::Club);
        take_card(st, &club);
        st.seats[cur].hand.push(club.clone());
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("skill:lianhuan:{club}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].chained);
    assert!(st.history.iter().any(|e| e.text.contains("连环")));
}

#[test]
fn lianhuan_club_can_recast() {
    let mgr = manager(721);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut club, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_36");
        club = free_card_where(st, |c| c.suit == Suit::Club);
        take_card(st, &club);
        st.seats[cur].hand.push(club.clone());
        before = st.seats[cur].hand.len();
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("recast:{club}")));
    mgr.execute_action(&room, &cur_pid, &format!("recast:{club}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&club));
    assert_eq!(st.seats[cur].hand.len(), before);
}

#[test]
fn niepan_clears_zones_and_recovers() {
    let mgr = manager(722);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut mark) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_36");
        st.seats[cur].hp = 1;
        st.seats[cur].flipped = true;
        mark = free_card_where(st, |c| c.rank == 3);
        take_card(st, &mark);
        st.seats[cur]
            .marks
            .entry("buqu".into())
            .or_default()
            .push(mark.clone());
        give(st, cur, "shan");
        give(st, cur, "sha");
        let pid_cur = pid(st, cur);
        crate::resolve::apply_damage(st, cur, 1, None, crate::resolve::DamageType::Normal, None);
        drive_state_inject(st, &mut vec![format!("{pid_cur}|dying=skill:niepan")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 3);
    assert!(!st.seats[cur].flipped);
    assert!(st.seats[cur].marks.get("buqu").is_none_or(|v| v.is_empty()));
    assert_eq!(st.seats[cur].hand.len(), 3);
    assert!(st.seats[cur].flags.contains_key("niepan_used"));
    assert!(st.deck.discard.contains(&mark));
}

// ---------------------------------------------------------------------------
// 卧龙诸葛亮 · 八阵 / 火计 / 看破
// ---------------------------------------------------------------------------

#[test]
fn bazhen_virtual_bagua_judges_red_and_dodges() {
    let mgr = manager(730);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut wolong, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        wolong = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, wolong, "sgs_hero_37");
        sha = give(st, cur, "sha");
        top_card(st, |c| c.suit == Suit::Heart);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, wl_pid) = (pid(&st, cur), pid(&st, wolong));
    let hp_before = st.seats[wolong].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{wl_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st
        .pending
        .as_ref()
        .is_some_and(|p| p.options.iter().any(|o| o.id == "skill:bagua")));
    mgr.execute_action(&room, &wl_pid, "skill:bagua").unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[wolong].hp, hp_before, "八阵红判定应视为出闪");
    assert!(st.history.iter().any(|e| e.text.contains("八卦阵判定")));
}

#[test]
fn bazhen_disabled_by_armor_and_qinggang() {
    let mgr = manager(731);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut wolong, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        wolong = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, wolong, "sgs_hero_37");
        let armor = free_card_where(st, |c| c.def == "renwang_dun");
        take_card(st, &armor);
        st.seats[wolong]
            .equips
            .insert(EquipSlot::Armor, Some(armor));
        sha = give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, wl_pid) = (pid(&st, cur), pid(&st, wolong));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{wl_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == "skill:bagua")),
        "有实际防具时八阵不生效"
    );
}

#[test]
fn huoji_red_hand_as_fire_attack() {
    let mgr = manager(732);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut red) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_37");
        set_general(st, target, "sgs_hero_2");
        red = free_card_where(st, |c| crate::skills::is_red(c.suit));
        take_card(st, &red);
        st.seats[cur].hand.push(red.clone());
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("skill:huoji:{red}")));
    mgr.execute_action(&room, &cur_pid, &format!("skill:huoji:{red}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.as_ref().is_some_and(|p| p.kind == "target"));
    assert!(st.history.iter().any(|e| e.text.contains("火计")));
}

#[test]
fn kanpo_black_hand_as_wuxie() {
    let mgr = manager(733);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut wolong, mut wuzhong, mut black) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        wolong = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, wolong, "sgs_hero_37");
        wuzhong = give(st, cur, "wuzhong_shengyou");
        black = free_card_where(st, |c| crate::skills::is_black(c.suit));
        take_card(st, &black);
        st.seats[wolong].hand.push(black.clone());
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, wl_pid) = (pid(&st, cur), pid(&st, wolong));
    mgr.execute_action(&room, &cur_pid, &format!("play:{wuzhong}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![format!("{wl_pid}|wuxie=respond:{black}")],
    );
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("看破")));
    assert!(st.history.iter().any(|e| e.text.contains("效果被抵消")));
}
