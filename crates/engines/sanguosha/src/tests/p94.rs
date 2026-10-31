//! P4.4.2 一将成名批次 Y6（8 将）：郭皇后 / 李严 / 孙登 / 刘虞 / 岑昏 / 孙资刘放 / 黄皓 / 张让。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

fn equip_slot_card(st: &mut SgsState, seat: usize, slot: EquipSlot) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(slot))
    });
    take_card(st, &card);
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    super::support::give_def(st, seat, def)
}

// ---------------------------------------------------------------------------
// 郭皇后 · 矫诏 / 殚心
// ---------------------------------------------------------------------------

#[test]
fn jiazhao_declares_and_uses() {
    let mgr = manager(2900);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_380");
        stage_play(st, seat);
        st.seats[seat].hp = 1;
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:jiazhao").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiazhao"), "矫诏：由最近角色声明");
    let declaimer = st.pending.as_ref().map(|p| p.actor.clone()).unwrap();
    mgr.execute_action(&room, &declaimer, "choose:tao").unwrap();
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &pid);
    assert!(
        acts.iter()
            .any(|a| a.id == format!("skill:jiazhao_use:{card}")),
        "矫诏：展示牌可当【桃】使用"
    );
    mgr.execute_action(&room, &pid, &format!("skill:jiazhao_use:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, 2, "矫诏：当【桃】回复体力");
}

#[test]
fn danxin_modifies_jiazhao() {
    let mgr = manager(2901);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_380");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        damage_and_advance(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("danxin"), "殚心：受伤后询问");
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].flags.contains_key("jiaozhao_modified"),
        "殚心：修改矫诏"
    );
}

#[test]
fn jiazhao_modified_self_declare() {
    let mgr = manager(2902);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_380");
        stage_play(st, seat);
        st.seats[seat]
            .flags
            .insert("jiaozhao_modified".into(), true);
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:jiazhao").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let declaimer = st.pending.as_ref().map(|p| p.actor.clone()).unwrap();
    assert_eq!(declaimer, pid, "矫诏（修改）：由自己声明");
}

// ---------------------------------------------------------------------------
// 李严 · 督粮 / 腹鳞
// ---------------------------------------------------------------------------

#[test]
fn duliang_gains_and_marks_draw_bonus() {
    let mgr = manager(2903);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_381");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:duliang").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before + 1,
        "督粮：获得其一张手牌"
    );
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].flags.contains_key("duliang"),
        "督粮：标记下回合多摸"
    );
}

#[test]
fn duliang_basic_gain_mode() {
    let mgr = manager(2904);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_381");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:duliang").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("督粮")));
}

#[test]
fn fulin_raises_hand_limit() {
    let mgr = manager(2905);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut base, mut limit) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_381");
        st.turn_counters.clear();
        base = st.hand_limit(seat);
        st.set_counter("fulin_gain", 2);
        limit = crate::skills::hand_limit(st, seat);
    });
    assert_eq!(limit, base + 2, "腹鳞：本回合获得的牌不计入手牌上限");
}

// ---------------------------------------------------------------------------
// 孙登 · 匡弼
// ---------------------------------------------------------------------------

#[test]
fn kuangbi_places_and_collects() {
    let mgr = manager(2906);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut helper, mut helper_pid, mut cards) =
        (0, String::new(), 0, String::new(), Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_382");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        helper = other_seats(st, seat)[0];
        set_general(st, helper, "sgs_hero_1");
        helper_pid = st.seats[helper].player_id.clone();
        cards.push(give_def(st, helper, "sha"));
        cards.push(give_def(st, helper, "shan"));
    });
    mgr.execute_action(&room, &pid, "skill:kuangbi").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{helper_pid}"))
        .unwrap();
    for card in &cards {
        mgr.execute_action(&room, &helper_pid, &format!("choose:{card}"))
            .unwrap();
    }
    mgr.execute_action(&room, &helper_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::mark_cards(&st, seat, "kuangbi").len(),
        2,
        "匡弼：牌置于武将牌上"
    );
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::mark_cards(&st, seat, "kuangbi").len(),
        0,
        "匡弼：准备阶段回收"
    );
    assert!(st.history.iter().any(|e| e.text.contains("匡弼：获得")));
}

#[test]
fn kuangbi_pass_places_nothing() {
    let mgr = manager(2907);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut helper_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_382");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        let helper = other_seats(st, seat)[0];
        set_general(st, helper, "sgs_hero_1");
        helper_pid = st.seats[helper].player_id.clone();
        give_def(st, helper, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:kuangbi").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{helper_pid}"))
        .unwrap();
    mgr.execute_action(&room, &helper_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(crate::skills::mark_cards(&st, seat, "kuangbi").len(), 0);
}

#[test]
fn kuangbi_once_per_phase() {
    let mgr = manager(2908);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_382");
        stage_play(st, seat);
        st.set_counter("kuangbi_used", 1);
        pid = st.seats[seat].player_id.clone();
        let helper = other_seats(st, seat)[0];
        set_general(st, helper, "sgs_hero_1");
        give_def(st, helper, "sha");
    });
    let st = state_of(&mgr, &room);
    assert!(!crate::turn::legal_actions(&st, &pid)
        .iter()
        .any(|a| a.id == "skill:kuangbi"));
}

// ---------------------------------------------------------------------------
// 刘虞 · 止戈 / 宗祚
// ---------------------------------------------------------------------------

#[test]
fn zhige_makes_target_use_sha() {
    let mgr = manager(2909);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_383");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(*i, seat))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[seat].hp;
        for _ in 0..4 {
            give_def(st, seat, "shan");
        }
        give_def(st, target, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:zhige").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("止戈：对")),
        "止戈：目标对刘虞使用【杀】"
    );
    let _ = hp;
}

#[test]
fn zhige_equip_option() {
    let mgr = manager(2910);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut equip) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_383");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(*i, seat))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        equip = equip_slot_card(st, target, EquipSlot::Armor);
        for _ in 0..4 {
            give_def(st, seat, "shan");
        }
    });
    mgr.execute_action(&room, &pid, "skill:zhige").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.contains(&equip), "止戈：获得其装备牌");
}

#[test]
fn zongzuo_adjusts_max_hp() {
    let mgr = manager(2911);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut kingdoms, mut before, mut after) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_383");
        // 保证多个势力：wei / wu / qun / shu
        let others = other_seats(st, seat);
        let codes = ["sgs_hero_334", "sgs_hero_339", "sgs_hero_341", "sgs_hero_1"];
        for (i, s) in others.iter().enumerate() {
            set_general(st, *s, codes[i % codes.len()]);
        }
        before = st.seats[seat].max_hp;
        kingdoms = crate::skills::kingdom_count(st) as i32;
        crate::resolve_yijiang8::init_zongzuo(st);
        after = st.seats[seat].max_hp;
    });
    assert_eq!(after, before + kingdoms, "宗祚：按势力数增加体力上限");
    edit(&mgr, &room, |st| {
        let victim = other_seats(st, seat)
            .into_iter()
            .find(|i| {
                st.seats[*i]
                    .general
                    .as_deref()
                    .and_then(crate::data::general)
                    .is_some_and(|g| g.kingdom == "wei")
            })
            .expect("wei");
        st.seats[victim].alive = false;
        let before_max = st.seats[seat].max_hp;
        crate::resolve_yijiang8::zongzuo_on_death(st, victim);
        assert_eq!(
            st.seats[seat].max_hp,
            before_max - 1,
            "宗祚：势力全灭减上限"
        );
    });
}

// ---------------------------------------------------------------------------
// 岑昏 · 极奢 / 链祸
// ---------------------------------------------------------------------------

#[test]
fn jishe_draws_and_lowers_limit() {
    let mgr = manager(2912);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_384");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:jishe").unwrap();
    mgr.execute_action(&room, &pid, "skill:jishe").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 2, "极奢：摸 2 张");
    assert_eq!(
        crate::skills::hand_limit(&st, seat),
        st.seats[seat].hp.max(0) as usize - 2,
        "极奢：手牌上限 -2"
    );
}

#[test]
fn jishe_end_chains_character() {
    let mgr = manager(2913);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_384");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jishe"), "极奢：结束阶段横置");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].chained, "极奢：进入连环状态");
}

#[test]
fn lianhuo_adds_fire_damage() {
    let mgr = manager(2914);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut hp) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_384");
        st.turn_counters.clear();
        st.seats[seat].chained = true;
        hp = st.seats[seat].hp;
        crate::resolve::apply_damage(st, seat, 1, None, crate::resolve::DamageType::Fire, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 2, "链祸：火焰伤害 +1");
    assert!(!st.seats[seat].chained, "链祸：传导后重置横置");
}

// ---------------------------------------------------------------------------
// 孙资刘放 · 瑰藻 / 讥谀
// ---------------------------------------------------------------------------

#[test]
fn guizao_draws_after_distinct_suits() {
    let mgr = manager(2915);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_385");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        let hearts: Vec<String> = st
            .cards
            .iter()
            .filter(|(_, c)| c.suit == Suit::Heart)
            .map(|(id, _)| id.clone())
            .take(2)
            .collect();
        let spades: Vec<String> = st
            .cards
            .iter()
            .filter(|(_, c)| c.suit == Suit::Spade)
            .map(|(id, _)| id.clone())
            .take(2)
            .collect();
        for id in hearts.iter().take(1).chain(spades.iter().take(1)) {
            take_card(st, id);
            crate::turn::discard_by(st, Some(seat), id);
            st.phase_discarded.push(id.clone());
        }
        before = st.seats[seat].hand.len();
        assert!(
            crate::resolve_yijiang8::guizao_ready(st, seat),
            "瑰藻：花色均不同时可用"
        );
        crate::resolve::push(
            st,
            crate::resolve::Frame::GuiZao(crate::resolve_yijiang8::GuiZaoFrame { owner: seat }),
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("guizao"), "瑰藻：花色均不同时询问");
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 1, "瑰藻：摸一张");
}

#[test]
fn jiyu_blocks_suit() {
    let mgr = manager(2916);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut banned_card) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_385");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        // 目标只有红桃手牌，讥谀封锁红桃。
        give_suit(st, target, Suit::Heart);
        banned_card = give_suit(st, seat, Suit::Heart);
        give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:jiyu").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        crate::resolve_yijiang8::jiyu_banned(&st, seat, &banned_card),
        "讥谀：本回合不能使用该花色"
    );
    assert!(crate::turn::legal_actions(&st, &pid)
        .iter()
        .all(|a| a.id != format!("play:{banned_card}")));
}

#[test]
fn jiyu_spade_flips_and_loses_hp() {
    let mgr = manager(2917);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_385");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_suit(st, target, Suit::Spade);
        hp = st.seats[target].hp;
        give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:jiyu").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "讥谀：黑桃失去 1 点体力");
    assert!(st.seats[seat].flipped, "讥谀：自己翻面");
}

// ---------------------------------------------------------------------------
// 黄皓 · 寝情 / 贿生
// ---------------------------------------------------------------------------

#[test]
fn qinqing_discards_and_draws() {
    let mgr = manager(2918);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_386");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        let lord = st.lord_index().unwrap_or(0);
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| *i != lord && st.in_attack_range(*i, lord))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
        give_def(st, target, "shan");
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qinqing"), "寝情：结束阶段");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("寝情：弃置")));
}

#[test]
fn qinqing_extra_draw_when_target_ahead() {
    let mgr = manager(2919);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_386");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        let lord = st.lord_index().unwrap_or(0);
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| *i != lord && st.in_attack_range(*i, lord))
            .expect("in range");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        for _ in 0..3 {
            give_def(st, target, "sha");
        }
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].hand.len() > before,
        "寝情：手牌多于主公则你摸牌"
    );
}

#[test]
fn huisheng_prevents_damage() {
    let mgr = manager(2920);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut source_pid, mut hp, mut before) = (0, 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_386");
        st.turn_counters.clear();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        source_pid = st.seats[source].player_id.clone();
        give_def(st, seat, "shan");
        hp = st.seats[seat].hp;
        before = st.seats[source].hand.len();
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
    assert_eq!(pending_skill(&st), Some("huisheng"), "贿生：受伤时询问");
    mgr.execute_action(&room, &source_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "贿生：防止此伤害");
    assert!(st.seats[source].hand.len() > before, "贿生：来源获得展示牌");
    assert!(
        st.seats[seat]
            .flags
            .contains_key(&format!("huisheng_no:{source}")),
        "贿生：本局不能对其再发动"
    );
}

// ---------------------------------------------------------------------------
// 张让 · 滔乱
// ---------------------------------------------------------------------------

#[test]
fn taoluan_sha_counts_once() {
    let mgr = manager(2921);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut target_pid) =
        (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_387");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "tao");
        let target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &pid);
    assert!(
        acts.iter()
            .any(|a| a.id == format!("skill:taoluan:{card}:sha")),
        "滔乱：可将桃当【杀】使用"
    );
    mgr.execute_action(&room, &pid, &format!("skill:taoluan:{card}:sha"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("target"));
    assert!(
        st.seats[seat].flags.contains_key("taoluan_used:sha"),
        "滔乱：牌名限一次"
    );
    assert!(!crate::turn::legal_actions(&st, &pid)
        .iter()
        .any(|a| a.id.ends_with(":sha") && a.id.starts_with("skill:taoluan:")));
    let _ = target_pid;
}

#[test]
fn taoluan_tao_heals() {
    let mgr = manager(2922);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut hp) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_387");
        stage_play(st, seat);
        st.seats[seat].hp = 1;
        hp = st.seats[seat].hp;
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "shan");
    });
    mgr.execute_action(&room, &pid, &format!("skill:taoluan:{card}:tao"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1, "滔乱：当【桃】回复");
    assert!(
        st.seats[seat].flags.contains_key("taoluan_used:tao"),
        "滔乱：记录牌名"
    );
}

#[test]
fn taoluan_choice_opt2_disables() {
    let mgr = manager(2923);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut target, mut target_pid, mut hp) =
        (0, String::new(), String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_387");
        stage_play(st, seat);
        st.seats[seat].hp = 2;
        hp = st.seats[seat].hp;
        pid = st.seats[seat].player_id.clone();
        card = give_def(st, seat, "shan");
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_def(st, target, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("skill:taoluan:{card}:tao"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("taoluan"),
        "滔乱：使用后令他人选择"
    );
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1 - 1, "滔乱：失去 1 点体力");
    assert_eq!(st.counter("taoluan_disabled"), 1, "滔乱：本回合失效");
    assert!(!crate::turn::legal_actions(&st, &pid)
        .iter()
        .any(|a| a.id.starts_with("skill:taoluan:")));
}
