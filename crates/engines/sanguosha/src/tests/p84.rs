//! P4.4.2 界标 25 将批次 4（魏 8 将）：界曹操 / 界司马懿 / 界夏侯惇 / 界张辽 / 界许褚 / 界郭嘉 / 界李典 / 界甄姬。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, manager, state_of, take_card,
};
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

fn set_top(st: &mut SgsState, cards: &[String]) {
    for id in cards {
        take_card(st, id);
        st.deck.draw.push(id.clone());
    }
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
// 界曹操 · 奸雄 / 护驾
// ---------------------------------------------------------------------------

#[test]
fn jie_jianxiong_draws_then_gains_card() {
    let mgr = manager(1300);
    let room = start_and_pick(&mgr, 5);
    let (mut caocao, mut caocao_pid, mut card) = (0, String::new(), String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caocao = st.turn;
        set_general(st, caocao, "sgs_hero_311");
        card = give_def(st, caocao, "sha");
        st.seats[caocao].hand.retain(|c| c != &card);
        st.deck.processing.push(card.clone());
        before = st.seats[caocao].hand.len();
        st.turn_counters.clear();
        caocao_pid = st.seats[caocao].player_id.clone();
        let src = other_seats(st, caocao)[0];
        crate::resolve::apply_damage(
            st,
            caocao,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            Some(&card),
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("jianxiong")
    );
    mgr.execute_action(&room, &caocao_pid, "choose:gain")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[caocao].hand.len(),
        before + 2,
        "界奸雄：摸 1 并获得了伤害牌"
    );
}

#[test]
fn jie_jianxiong_pass_draws_nothing() {
    let mgr = manager(1301);
    let room = start_and_pick(&mgr, 5);
    let (mut caocao, mut caocao_pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caocao = st.turn;
        set_general(st, caocao, "sgs_hero_311");
        let card = give_def(st, caocao, "sha");
        st.seats[caocao].hand.retain(|c| c != &card);
        st.deck.processing.push(card.clone());
        before = st.seats[caocao].hand.len();
        st.turn_counters.clear();
        caocao_pid = st.seats[caocao].player_id.clone();
        let src = other_seats(st, caocao)[0];
        crate::resolve::apply_damage(
            st,
            caocao,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            Some(&card),
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    mgr.execute_action(&room, &caocao_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[caocao].hand.len(),
        before,
        "界奸雄：不获得则不摸牌"
    );
}

#[test]
fn jie_hujia_lord_draws_once() {
    let mgr = manager(1302);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut ally) = (0, 0);
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.seats.iter().position(|s| s.identity.is_lord()).unwrap();
        set_general(st, lord, "sgs_hero_311");
        ally = (0..st.seats.len())
            .find(|i| *i != lord && st.seats[*i].alive)
            .unwrap();
        st.turn = lord;
        st.turn_counters.clear();
        before = st.seats[lord].hand.len();
        crate::skills::hujia_lord_draw(st, ally);
        crate::skills::hujia_lord_draw(st, ally);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[lord].hand.len(),
        before + 1,
        "界护驾：每回合限一次摸 1"
    );
    assert_eq!(st.counter("hujia_lord_draw"), 1);
}

// ---------------------------------------------------------------------------
// 界司马懿 · 反馈 / 鬼才
// ---------------------------------------------------------------------------

fn push_test_quick_judge(st: &mut SgsState, owner: usize, source: usize) {
    crate::resolve::push(
        st,
        crate::resolve::Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
            owner,
            reason: "ganglie".into(),
            stage: crate::resolve::QuickJudgeStage::Reveal,
            judge_card: None,
            rewrite_order: Vec::new(),
            rewrite_cursor: 0,
            data: serde_json::json!({ "victim": owner, "source": source }),
            saved_outcome: None,
        }),
    );
}

#[test]
fn jie_guicai_can_use_equip_to_rewrite() {
    let mgr = manager(1310);
    let room = start_and_pick(&mgr, 5);
    let (mut simayi, mut sima_pid, mut armor) = (0, String::new(), String::new());
    let mut judge_before = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        simayi = (st.turn + 1) % st.seats.len();
        set_general(st, simayi, "sgs_hero_312");
        armor = equip_card(st, simayi, "bagua_zhen");
        let judge_card = give_suit(st, (st.turn + 2) % st.seats.len(), Suit::Heart);
        set_top(st, &[judge_card.clone()]);
        judge_before = judge_card;
        st.turn_counters.clear();
        sima_pid = st.seats[simayi].player_id.clone();
        let source = (st.turn + 3) % st.seats.len();
        push_test_quick_judge(st, simayi, source);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("界鬼才应可用装备改判");
    assert_eq!(pending.skill.as_deref(), Some("guicai"));
    assert!(
        pending
            .options
            .iter()
            .any(|o| o.id == format!("respond:{armor}")),
        "装备区的牌应出现在改判选项"
    );
    mgr.execute_action(&room, &sima_pid, &format!("respond:{armor}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.equip_def(simayi, EquipSlot::Armor).map(|d| d.code),
        None,
        "鬼才改判后装备被移出"
    );
    assert!(
        st.deck.discard.contains(&armor) || st.deck.processing.iter().any(|c| c == &armor),
        "鬼才改判后装备作为判定牌进入弃牌堆/处理区"
    );
    let _ = judge_before;
}

#[test]
fn standard_guicai_cannot_use_equip() {
    let mgr = manager(1311);
    let room = start_and_pick(&mgr, 5);
    let (mut simayi, mut has_equip_option) = (0, true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        simayi = (st.turn + 1) % st.seats.len();
        set_general(st, simayi, "sgs_hero_16");
        let armor = equip_card(st, simayi, "bagua_zhen");
        let judge_card = give_suit(st, (st.turn + 2) % st.seats.len(), Suit::Heart);
        set_top(st, &[judge_card]);
        st.turn_counters.clear();
        let source = (st.turn + 3) % st.seats.len();
        push_test_quick_judge(st, simayi, source);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        has_equip_option = st
            .pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == format!("respond:{armor}")));
    });
    assert!(!has_equip_option, "标准鬼才只能用一张手牌，不能用装备");
}

#[test]
fn jie_fankui_gains_source_card() {
    let mgr = manager(1312);
    let room = start_and_pick(&mgr, 5);
    let (mut simayi, mut sima_pid, mut src, mut src_pid) = (0, String::new(), 0, String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        simayi = st.turn;
        set_general(st, simayi, "sgs_hero_312");
        src = other_seats(st, simayi)[0];
        set_general(st, src, "sgs_hero_1");
        give_def(st, src, "sha");
        st.turn_counters.clear();
        sima_pid = st.seats[simayi].player_id.clone();
        src_pid = st.seats[src].player_id.clone();
        before = st.seats[simayi].hand.len();
        crate::resolve::apply_damage(
            st,
            simayi,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("pick_card")
    );
    let _ = (src_pid, before);
    mgr.execute_action(&room, &sima_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[simayi].hand.len(),
        before + 1,
        "反馈：获得来源一张牌"
    );
}

// ---------------------------------------------------------------------------
// 界夏侯惇 · 刚烈 / 清俭
// ---------------------------------------------------------------------------

fn setup_ganglie(seed: u64, judge_red: bool) -> (String, i32, i32) {
    let mgr = manager(seed);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut dun, mut attacker_pid, mut dun_pid) =
        (0, 0, String::new(), String::new());
    let mut sha = String::new();
    let mut hp_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        dun = neighbor(st, attacker);
        set_general(st, dun, "sgs_hero_313");
        sha = give_def(st, attacker, "sha");
        give_def(st, attacker, "shan");
        let judge = give_card(st, dun, |c| {
            if judge_red {
                matches!(c.suit, Suit::Heart | Suit::Diamond)
            } else {
                matches!(c.suit, Suit::Spade | Suit::Club)
            }
        });
        st.seats[dun].hand.retain(|c| c != &judge);
        set_top(st, &[judge]);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        attacker_pid = st.seats[attacker].player_id.clone();
        dun_pid = st.seats[dun].player_id.clone();
        hp_before = st.seats[attacker].hp;
    });
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{dun_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    (room, hp_before, st.seats[attacker].hp)
}

#[test]
fn jie_ganglie_red_damages_source() {
    let (room, hp_before, hp_after) = setup_ganglie(1320, true);
    assert_eq!(
        hp_after,
        hp_before - 1,
        "界刚烈：红色判定对来源造成 1 点伤害"
    );
    let _ = room;
}

#[test]
fn jie_ganglie_black_discards_source_card() {
    let mgr = manager(1321);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut dun, mut attacker_pid, mut dun_pid) =
        (0, 0, String::new(), String::new());
    let mut sha = String::new();
    let mut hand_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        dun = neighbor(st, attacker);
        set_general(st, dun, "sgs_hero_313");
        sha = give_def(st, attacker, "sha");
        give_def(st, attacker, "shan");
        give_def(st, attacker, "tao");
        let judge = give_card(st, dun, |c| matches!(c.suit, Suit::Spade | Suit::Club));
        st.seats[dun].hand.retain(|c| c != &judge);
        set_top(st, &[judge]);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        attacker_pid = st.seats[attacker].player_id.clone();
        dun_pid = st.seats[dun].player_id.clone();
        hand_before = st.seats[attacker].hand.len();
    });
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{dun_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[attacker].hand.len()
            + st.seats[attacker].judge_zone.len()
            + st.seats[attacker].equips.values().flatten().count(),
        hand_before - 2,
        "界刚烈：黑色判定弃置来源一张牌（【杀】+ 弃牌）"
    );
}

#[test]
fn qingjian_gives_card_and_raises_hand_limit() {
    let mgr = manager(1322);
    let room = start_and_pick(&mgr, 5);
    let (mut dun, mut victim, mut dun_pid, mut victim_pid, mut shunshou) =
        (0, 0, String::new(), String::new(), String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dun = st.turn;
        set_general(st, dun, "sgs_hero_313");
        victim = neighbor(st, dun);
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "sha");
        give_def(st, dun, "shan");
        shunshou = give_def(st, dun, "shunshou_qianyang");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        dun_pid = st.seats[dun].player_id.clone();
        victim_pid = st.seats[victim].player_id.clone();
        before = st.seats[victim].hand.len();
    });
    mgr.execute_action(&room, &dun_pid, &format!("play:{shunshou}"))
        .unwrap();
    mgr.execute_action(&room, &dun_pid, &format!("target:{victim_pid}"))
        .unwrap();
    let mut inject = vec![
        format!("{dun_pid}|pick_card=pick:hand"),
        format!("{dun_pid}|choice=target:{victim_pid}"),
    ];
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("qingjian_limit"), 1, "清俭：当前回合手牌上限 +1");
    assert_eq!(
        st.seats[victim].hand.len(),
        before,
        "清俭：获得后交给对方一张"
    );
    assert!(st.history.iter().any(|e| e.text.contains("清俭")));
}
// ---------------------------------------------------------------------------
// 界张辽 · 突袭
// ---------------------------------------------------------------------------

fn enter_draw_pending(mgr: &game_engine_core::GameManager, room: &str, seat: usize) {
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
fn tuxi_jie_one_target_draws_one() {
    let mgr = manager(1330);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut cur_pid, mut a_pid) = (0, 0, String::new(), String::new());
    let mut hand_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_314");
        a = other_seats(st, cur)[0];
        set_general(st, a, "sgs_hero_1");
        give_def(st, a, "sha");
        cur_pid = st.seats[cur].player_id.clone();
        a_pid = st.seats[a].player_id.clone();
        hand_before = st.seats[cur].hand.len();
    });
    enter_draw_pending(&mgr, &room, cur);
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("tuxi")
    );
    mgr.execute_action(&room, &cur_pid, "choose:tuxi").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before + 2,
        "界突袭：获得 1 张并少摸 1 张（仍摸 1）"
    );
    assert_eq!(st.turn_phase, TurnPhase::Play, "界突袭：跳过摸牌阶段");
}

#[test]
fn tuxi_jie_two_targets_draws_zero() {
    let mgr = manager(1331);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut pids = Vec::new();
    let mut hand_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_314");
        for seat in other_seats(st, cur).into_iter().take(2) {
            set_general(st, seat, "sgs_hero_1");
            give_def(st, seat, "sha");
            pids.push(st.seats[seat].player_id.clone());
        }
        cur_pid = st.seats[cur].player_id.clone();
        hand_before = st.seats[cur].hand.len();
    });
    enter_draw_pending(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, "choose:tuxi").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{}", pids[0]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{}", pids[1]))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before + 2,
        "界突袭：获得 2 张不额外摸牌"
    );
}

#[test]
fn tuxi_jie_decline_draws_normally() {
    let mgr = manager(1332);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut hand_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_314");
        cur_pid = st.seats[cur].player_id.clone();
        hand_before = st.seats[cur].hand.len();
    });
    enter_draw_pending(&mgr, &room, cur);
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    let mut ev = Vec::new();
    let full = mgr.get_state(&room).unwrap();
    let mut st = SgsState::parse(&full.data).unwrap();
    crate::turn::phase_step(&mut st, &mut ev).unwrap();
    let st2 = state_of(&mgr, &room);
    assert_eq!(
        st2.seats[cur].hand.len(),
        hand_before + 2,
        "界突袭：不发动则正常摸 2"
    );
    let _ = st;
}

// ---------------------------------------------------------------------------
// 界许褚 · 裸衣
// ---------------------------------------------------------------------------

#[test]
fn luoyi_jie_reveals_and_keeps_equipment_and_basics() {
    let mgr = manager(1340);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut hand_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_315");
        let basic = give_def(st, cur, "sha");
        let weapon = give_def(st, cur, "qinggang_jian");
        let duel = give_def(st, cur, "juedou");
        let junk = give_def(st, cur, "wuzhong_shengyou");
        for id in [&basic, &weapon, &duel, &junk] {
            st.seats[cur].hand.retain(|c| c != id);
        }
        // 牌堆顶（末尾）：basic / weapon / duel；junk 在其下。
        st.deck.draw.push(junk);
        st.deck.draw.push(basic);
        st.deck.draw.push(weapon);
        st.deck.draw.push(duel);
        st.turn_phase = TurnPhase::Draw;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        hand_before = st.seats[cur].hand.len();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("luoyi")
    );
    mgr.execute_action(&room, &cur_pid, "choose:luoyi").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before + 3,
        "界裸衣：获得亮出 3 张中的基本牌/武器/【决斗】"
    );
    assert!(st.seats[cur].flags.contains_key("luoyi_bonus"));
    assert_eq!(st.turn_phase, TurnPhase::Play, "界裸衣：跳过摸牌阶段");
}

#[test]
fn luoyi_jie_bonus_boosts_sha_damage() {
    let mgr = manager(1341);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut sha) =
        (0, 0, String::new(), String::new(), String::new());
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_315");
        target = neighbor(st, cur);
        set_general(st, target, "sgs_hero_1");
        sha = give_def(st, cur, "sha");
        st.seats[cur].flags.insert("luoyi_bonus".into(), true);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 2, "界裸衣：【杀】伤害 +1");
}

#[test]
fn luoyi_jie_bonus_clears_next_prepare() {
    let mgr = manager(1342);
    let room = start_and_pick(&mgr, 5);
    let mut cleared = false;
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_315");
        st.seats[cur].flags.insert("luoyi_bonus".into(), true);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        cleared = !st.seats[cur].flags.contains_key("luoyi_bonus");
    });
    assert!(cleared, "界裸衣：伤害加成持续到下回合开始");
}

// ---------------------------------------------------------------------------
// 界郭嘉 · 天妒 / 遗计
// ---------------------------------------------------------------------------

#[test]
fn yiji_jie_excludes_self() {
    let mgr = manager(1350);
    let room = start_and_pick(&mgr, 5);
    let (mut guojia, mut guojia_pid, mut self_option) = (0, String::new(), true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guojia = st.turn;
        set_general(st, guojia, "sgs_hero_316");
        st.turn_counters.clear();
        guojia_pid = st.seats[guojia].player_id.clone();
        let src = other_seats(st, guojia)[0];
        crate::resolve::apply_damage(
            st,
            guojia,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        self_option = st.pending.as_ref().is_some_and(|p| {
            p.options
                .iter()
                .any(|o| o.id == format!("target:{guojia_pid}"))
        });
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("yiji")
    );
    assert!(!self_option, "界遗计：只能交给其他角色");
    let _ = guojia;
}

#[test]
fn yiji_jie_gives_card_to_other() {
    let mgr = manager(1351);
    let room = start_and_pick(&mgr, 5);
    let (mut guojia, mut other, mut guojia_pid, mut other_pid) =
        (0, 0, String::new(), String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guojia = st.turn;
        set_general(st, guojia, "sgs_hero_316");
        other = other_seats(st, guojia)[0];
        set_general(st, other, "sgs_hero_1");
        st.turn_counters.clear();
        guojia_pid = st.seats[guojia].player_id.clone();
        other_pid = st.seats[other].player_id.clone();
        before = st.seats[other].hand.len();
        let src = other_seats(st, guojia)[1];
        crate::resolve::apply_damage(
            st,
            guojia,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    mgr.execute_action(&room, &guojia_pid, &format!("target:{other_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[other].hand.len(),
        before + 1,
        "界遗计：将摸到的牌交给其他角色"
    );
}

#[test]
fn tiandu_jie_gains_judge_card() {
    let mgr = manager(1352);
    let room = start_and_pick(&mgr, 5);
    let (mut guojia, mut judge) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guojia = st.turn;
        set_general(st, guojia, "sgs_hero_316");
        let lebu = give_def(st, guojia, "lebu_sishu");
        st.seats[guojia].hand.retain(|c| c != &lebu);
        st.deck.processing.push(lebu.clone());
        judge = give_suit(st, guojia, Suit::Spade);
        take_card(st, &judge);
        st.deck.draw.push(judge.clone());
        st.turn_counters.clear();
        crate::resolve::push_judge(st, lebu, guojia);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        let mut inject = Vec::new();
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[guojia].hand.iter().any(|c| c == &judge),
        "天妒：判定牌生效后获得之"
    );
}

// ---------------------------------------------------------------------------
// 界李典 · 恂恂 / 忘隙
// ---------------------------------------------------------------------------

#[test]
fn xunxun_puts_two_on_top_and_rest_bottom() {
    let mgr = manager(1360);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut cards = Vec::new();
    let mut chosen = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_317");
        for _ in 0..4 {
            cards.push(give_def(st, cur, "sha"));
        }
        for c in &cards {
            take_card(st, c);
            st.deck.draw.push(c.clone());
        }
        st.turn_phase = TurnPhase::Draw;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
        chosen = st
            .pending
            .as_ref()
            .map(|p| p.options.iter().take(2).map(|o| o.id.clone()).collect())
            .unwrap_or_default();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("xunxun")
    );
    let pick: Vec<String> = chosen
        .iter()
        .map(|id| id.trim_start_matches("choose:").to_string())
        .collect();
    for p in &pick {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{p}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    let draw = &st.deck.draw;
    // 置于牌堆顶的两张随后被摸牌阶段摸走，因此出现在手牌中。
    for id in &pick {
        assert!(
            st.seats[cur].hand.iter().any(|c| c == id),
            "恂恂：牌堆顶的 2 张应被摸到（{id}）"
        );
    }
    let bottom: Vec<String> = cards
        .iter()
        .filter(|c| !pick.contains(c))
        .cloned()
        .collect();
    assert_eq!(&draw[0..2], bottom.as_slice(), "恂恂：其余置于牌堆底");
    assert_eq!(st.counter("xunxun_done"), 1);
}

#[test]
fn wangxi_after_dealing_damage() {
    let mgr = manager(1361);
    let room = start_and_pick(&mgr, 5);
    let (mut lidian, mut target, mut lidian_pid) = (0, 0, String::new());
    let mut target_before = 0;
    let mut lidian_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lidian = st.turn;
        set_general(st, lidian, "sgs_hero_317");
        target = other_seats(st, lidian)[0];
        set_general(st, target, "sgs_hero_1");
        st.turn_counters.clear();
        lidian_pid = st.seats[lidian].player_id.clone();
        target_before = st.seats[target].hand.len();
        lidian_before = st.seats[lidian].hand.len();
        crate::resolve::apply_damage(
            st,
            target,
            1,
            Some(lidian),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        assert!(
            st.pending.is_some(),
            "dbg pending={:?} resolution={:?}",
            st.pending.as_ref().map(|p| (
                p.kind.clone(),
                p.skill.clone(),
                p.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>()
            )),
            st.resolution
                .iter()
                .map(|f| format!("{f:?}").chars().take(60).collect::<String>())
                .collect::<Vec<_>>()
        );
        let mut inject = vec![format!("{lidian_pid}=choose:wangxi")];
        drive_state_inject(st, &mut inject);
        assert_eq!(
            st.seats[target].hand.len(),
            target_before + 1,
            "dbg after drive: hands lidian={:?} target={:?} hist={:?}",
            st.seats[lidian].hand.len(),
            st.seats[target].hand.len(),
            st.history
                .iter()
                .rev()
                .take(4)
                .map(|e| e.text.clone())
                .collect::<Vec<_>>()
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        target_before + 1,
        "忘隙：交给受伤角色一张"
    );
    assert_eq!(
        st.seats[lidian].hand.len(),
        lidian_before + 1,
        "忘隙：摸 2 交 1"
    );
    assert!(st.history.iter().any(|e| e.text.contains("忘隙")));
    let _ = lidian_pid;
}

#[test]
fn wangxi_after_receiving_damage() {
    let mgr = manager(1362);
    let room = start_and_pick(&mgr, 5);
    let (mut lidian, mut src) = (0, 0);
    let mut src_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lidian = st.turn;
        set_general(st, lidian, "sgs_hero_317");
        src = other_seats(st, lidian)[0];
        set_general(st, src, "sgs_hero_1");
        st.turn_counters.clear();
        src_before = st.seats[src].hand.len();
        crate::resolve::apply_damage(
            st,
            lidian,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        let mut inject = vec![format!("{}=choose:wangxi", st.seats[lidian].player_id)];
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[src].hand.len(),
        src_before + 1,
        "忘隙：受到伤害后交给来源一张"
    );
}

// ---------------------------------------------------------------------------
// 界甄姬 · 倾国 / 洛神
// ---------------------------------------------------------------------------

#[test]
fn jie_qingguo_black_card_as_shan() {
    let mgr = manager(1370);
    let room = start_and_pick(&mgr, 5);
    let (mut zhenji, mut black) = (0, String::new());
    let mut converted = false;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhenji = st.turn;
        set_general(st, zhenji, "sgs_hero_444");
        black = give_card(st, zhenji, |c| matches!(c.suit, Suit::Spade | Suit::Club));
        converted = crate::skills::conversion_for(st, zhenji, &black, "shan").is_some();
    });
    assert!(converted, "界倾国：黑色手牌当【闪】");
}

#[test]
fn jie_luoshen_ignores_hand_limit() {
    let mgr = manager(1371);
    let room = start_and_pick(&mgr, 5);
    let (mut zhenji, mut zhenji_pid) = (0, String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhenji = st.turn;
        set_general(st, zhenji, "sgs_hero_444");
        let keep = give_def(st, zhenji, "sha");
        st.config.max_hand_cards = 1;
        st.seats[zhenji].hp = 1;
        st.seats[zhenji].max_hp = 3;
        let black = give_suit(st, zhenji, Suit::Spade);
        st.seats[zhenji].hand.retain(|c| c != &black);
        take_card(st, &black);
        st.deck.draw.push(black.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        st.set_counter("skip_draw", 1);
        zhenji_pid = st.seats[zhenji].player_id.clone();
        before = st.seats[zhenji].hand.len();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
        let mut inject = vec![
            format!("{zhenji_pid}=choose:luoshen"),
            format!("{zhenji_pid}=pass"),
        ];
        drive_state_inject(st, &mut inject);
        let _ = keep;
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[zhenji].hand.len(),
        before + 1,
        "界洛神：获得黑色判定牌且不因手牌上限弃置"
    );
    assert_eq!(
        st.counter("luoshen_free"),
        1,
        "界洛神：获得的牌不计入手牌上限"
    );
    assert!(st.pending.is_none(), "界洛神：不触发超出上限弃牌");
}

#[test]
fn jie_luoshen_red_stops_without_gain() {
    let mgr = manager(1372);
    let room = start_and_pick(&mgr, 5);
    let (mut zhenji, mut zhenji_pid) = (0, String::new());
    let mut before = 0;
    let mut free = 1;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhenji = st.turn;
        set_general(st, zhenji, "sgs_hero_444");
        let red = give_suit(st, zhenji, Suit::Heart);
        st.seats[zhenji].hand.retain(|c| c != &red);
        take_card(st, &red);
        st.deck.draw.push(red.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        st.set_counter("skip_draw", 1);
        zhenji_pid = st.seats[zhenji].player_id.clone();
        before = st.seats[zhenji].hand.len();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
        let mut inject = vec![format!("{zhenji_pid}=choose:luoshen")];
        drive_state_inject(st, &mut inject);
        free = st.counter("luoshen_free");
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhenji].hand.len(), before, "界洛神：红色不获得");
    assert_eq!(free, 0);
    assert!(st.pending.is_none(), "界洛神：红色结束");
}
