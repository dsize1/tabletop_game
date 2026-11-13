//! P4.4.1 林包用例（一）：徐晃 / 曹丕 / 孙坚 / 董卓。

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
// 徐晃 · 断粮 / 截辎
// ---------------------------------------------------------------------------

#[test]
fn duanliang_black_basic_as_bingliang() {
    let mgr = manager(800);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut black) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_42");
        set_general(st, target, "sgs_hero_2");
        black = free_card_where(st, |c| crate::skills::is_black(c.suit) && c.def == "sha");
        take_card(st, &black);
        st.seats[cur].hand.push(black.clone());
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    assert!(mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .iter()
        .any(|a| a.id == format!("skill:duanliang:{black}")));
    mgr.execute_action(&room, &cur_pid, &format!("skill:duanliang:{black}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.seats[target]
        .judge_zone
        .iter()
        .any(|id| { st.effective_def_code(id) == Some("bingliang_cunduan") }));
}

#[test]
fn duanliang_ignores_distance_for_high_hand_target() {
    let mgr = manager(801);
    let room = start_and_pick(&mgr, 6);
    let (mut cur, mut far, mut black) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = st.seats_from_left_of(cur)[2];
        set_general(st, cur, "sgs_hero_42");
        set_general(st, far, "sgs_hero_2");
        for _ in 0..4 {
            give(st, far, "tao");
        }
        assert!(st.seats[far].hand.len() >= st.seats[cur].hand.len());
        black = free_card_where(st, |c| crate::skills::is_black(c.suit) && c.def == "sha");
        take_card(st, &black);
        st.seats[cur].hand.push(black.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, &format!("skill:duanliang:{black}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending.as_ref().is_some_and(|p| p
            .options
            .iter()
            .any(|o| o.id == format!("target:{}", pid(&st, far)))),
        "对牌数≥你的目标应无距离限制"
    );
}

#[test]
fn jiezi_draws_when_other_skips_draw() {
    let mgr = manager(802);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut xuhuang) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        xuhuang = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, xuhuang, "sgs_hero_42");
        st.set_counter("skip_draw", 1);
        st.turn_phase = TurnPhase::Draw;
        let pid_x = pid(st, xuhuang);
        drive_state_inject(st, &mut Vec::new());
        assert_eq!(st.seats[xuhuang].hand.len(), 1);
        let _ = pid_x;
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[xuhuang].hand.len(), 1, "截辎应摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("截辎")));
}

// ---------------------------------------------------------------------------
// 曹丕 · 行殇 / 放逐 / 颂威
// ---------------------------------------------------------------------------

#[test]
fn xingshang_gains_dead_players_cards() {
    let mgr = manager(810);
    let room = start_and_pick(&mgr, 5);
    let (mut caopi, mut victim, mut card) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let lord = st.lord_index().unwrap();
        // 曹丕坐在当前回合，避免终局；找一个非主公受害者。
        caopi = st.turn;
        if caopi == lord {
            caopi = st.seats_from_left_of(lord)[0];
            st.turn = caopi;
        }
        set_general(st, caopi, "sgs_hero_43");
        victim = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != caopi && Some(*i) != st.lord_index())
            .unwrap();
        set_general(st, victim, "sgs_hero_2");
        st.seats[victim].hp = 1;
        card = give(st, victim, "sha");
        let pid_c = pid(st, caopi);
        let pid_v = pid(st, victim);
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(caopi),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:xingshang")]);
        let _ = pid_v;
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[victim].alive);
    assert!(st.seats[caopi].hand.contains(&card), "行殇应获得死者所有牌");
    assert!(!st.deck.discard.contains(&card));
}

#[test]
fn fangzhu_flips_and_draws_lost_hp() {
    let mgr = manager(811);
    let room = start_and_pick(&mgr, 5);
    let (mut caopi, mut target, mut src) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caopi = st.turn;
        target = st.seats_from_left_of(caopi)[0];
        src = st.seats_from_left_of(caopi)[1];
        set_general(st, caopi, "sgs_hero_43");
        set_general(st, target, "sgs_hero_2");
        st.seats[caopi].hp = st.seats[caopi].max_hp - 2;
        let pid_c = pid(st, caopi);
        let pid_t = pid(st, target);
        crate::resolve::apply_damage(
            st,
            caopi,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(st, &mut vec![format!("{pid_c}|target=target:{pid_t}")]);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].flipped);
    assert_eq!(st.seats[target].hand.len(), 3, "放逐应摸已损失体力张");
}

#[test]
fn songwei_black_judge_draws_for_lord() {
    let mgr = manager(812);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut wei, mut owner, mut before) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.lord_index().unwrap();
        set_general(st, lord, "sgs_hero_43");
        wei = st.alive_indices().into_iter().find(|i| *i != lord).unwrap();
        set_general(st, wei, "sgs_hero_16"); // 司马懿（魏）
        owner = wei;
        let lebu = super::find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[owner].judge_zone.push(lebu);
        top_card(st, |c| crate::skills::is_black(c.suit));
        before = st.seats[lord].hand.len();
        st.turn = owner;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hand.len(), before + 1, "颂威应令主公摸 1 张");
}

// ---------------------------------------------------------------------------
// 孙坚 · 英魂
// ---------------------------------------------------------------------------

#[test]
fn yinghun_option1_draw_then_discard_one() {
    let mgr = manager(820);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_44");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = st.seats[cur].max_hp - 2;
        let pid_c = pid(st, cur);
        let pid_t = pid(st, target);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_c}|target=target:{pid_t}"),
                format!("{pid_c}|choice=choose:yinghun:1"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 1, "英魂选项 1：摸 2 弃 1");
    assert!(st.history.iter().any(|e| e.text.contains("英魂")));
}

#[test]
fn yinghun_option2_draw_one_discard_x() {
    let mgr = manager(821);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_44");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = st.seats[cur].max_hp - 2;
        let pid_c = pid(st, cur);
        let pid_t = pid(st, target);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_c}|target=target:{pid_t}"),
                format!("{pid_c}|choice=choose:yinghun:2"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 0, "英魂选项 2：摸 1 弃 2");
}

#[test]
fn yinghun_not_offered_when_full_hp() {
    let mgr = manager(822);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_44");
        st.seats[cur].hp = st.seats[cur].max_hp;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut Vec::new());
        assert!(st.history.iter().all(|e| !e.text.contains("英魂")));
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[cur].flipped);
}

// ---------------------------------------------------------------------------
// 董卓 · 酒池 / 肉林 / 崩坏 / 暴虐
// ---------------------------------------------------------------------------

#[test]
fn jiuchi_spade_as_jiu() {
    let mgr = manager(830);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut spade) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_45");
        spade = free_card_where(st, |c| c.suit == Suit::Spade);
        take_card(st, &spade);
        st.seats[cur].hand.push(spade.clone());
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, &format!("skill:jiuchi:{spade}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiu"), 1);
    assert!(st.deck.discard.contains(&spade));
}

#[test]
fn jiuchi_spade_saves_self_when_dying() {
    let mgr = manager(831);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut spade = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_45");
        st.seats[cur].hp = 1;
        spade = free_card_where(st, |c| c.suit == Suit::Spade);
        take_card(st, &spade);
        st.seats[cur].hand.push(spade.clone());
        let pid_c = pid(st, cur);
        crate::resolve::apply_damage(st, cur, 1, None, crate::resolve::DamageType::Normal, None);
        drive_state_inject(st, &mut vec![format!("{pid_c}|dying=respond:{spade}")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 1);
    assert!(st.deck.discard.contains(&spade));
}

#[test]
fn roulin_requires_two_shan() {
    let mgr = manager(832);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut shan) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_45");
        set_general(st, target, "sgs_hero_30"); // 小乔（女）
        sha = give(st, cur, "sha");
        shan = give(st, target, "shan");
        give(st, target, "shan");
        st.seats[target].hp = 3;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending.as_ref().is_some_and(|p| p.kind == "respond"),
        "肉林：一张【闪】不足，应继续要求第二张"
    );
    // 再出一张【闪】抵消。
    let st = state_of(&mgr, &room);
    let second = st.seats[target]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "shan"))
        .cloned()
        .expect("second shan");
    mgr.execute_action(&room, &target_pid, &format!("respond:{second}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 3, "两张【闪】抵消【杀】");
}

#[test]
fn bengkuai_reduces_max_hp() {
    let mgr = manager(833);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut max_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        let other = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_45");
        set_general(st, other, "sgs_hero_2");
        st.seats[cur].hp = 4;
        st.seats[other].hp = 2;
        max_before = st.seats[cur].max_hp;
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:bengkuai:max")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].max_hp, max_before - 1);
}

#[test]
fn baonue_spade_heals_lord() {
    let mgr = manager(834);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut qun, mut victim) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.lord_index().unwrap();
        set_general(st, lord, "sgs_hero_45");
        st.seats[lord].hp = st.seats[lord].max_hp - 1;
        qun = st.alive_indices().into_iter().find(|i| *i != lord).unwrap();
        set_general(st, qun, "sgs_hero_23"); // 吕布（群）
        victim = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != lord && *i != qun)
            .unwrap();
        set_general(st, victim, "sgs_hero_2");
        set_general(st, qun, "sgs_hero_23");
        top_card(st, |c| c.suit == Suit::Spade);
        st.seats[victim].hp = 4;
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(qun),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[lord].hp, st.seats[lord].max_hp,
        "暴虐黑桃应回复 1 点"
    );
}
