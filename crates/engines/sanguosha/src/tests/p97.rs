//! P4.4.2 界限突破批次 J2（8 将）：界孙策 / 界夏侯渊 / 界卧龙诸葛亮 / 界庞统 / 界魏延 / 界小乔 / 界孙坚 / 界庞德。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::skills;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn advance_once(st: &mut SgsState) {
    let mut ev = Vec::new();
    crate::resolve::advance(st, &mut ev).expect("advance");
}

// ---------------------------------------------------------------------------
// 界孙策 · 激昂 / 魂姿 / 制霸
// ---------------------------------------------------------------------------

#[test]
fn jiang_jie_gains_discarded_red_sha() {
    let mgr = manager(3300);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut red_sha, mut before) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_452");
        st.seats[seat].flags.insert("jaing_setup".into(), true);
        pid = pid_of(st, seat);
        red_sha = find_free_card(st, |c| c.def == "sha" && skills::is_red(c.suit));
        take_card(st, &red_sha);
        before = st.seats[seat].hp;
        crate::turn::discard_by(st, Some(seat), &red_sha);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("jiang_jie"),
        "界激昂：弃牌回收窗口"
    );
    mgr.execute_action(&room, &pid, "choose:jiang_jie").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].hand.iter().any(|c| c == &red_sha));
    assert_eq!(st.seats[seat].hp, before - 1, "激昂：失去 1 点体力");
}

#[test]
fn hunzi_jie_end_phase_draws_two() {
    let mgr = manager(3301);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_452");
        st.seats[seat].hp = 1;
        st.turn_counters.clear();
        pid = pid_of(st, seat);
        crate::resolve_mountain::awaken_triggers(st);
        assert!(skills::has(st, seat, "yingzi"), "魂姿觉醒获得英姿");
        assert!(st.seats[seat].flags.contains_key("hunzi_end"));
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("hunzi_end"),
        "界魂姿：结束阶段二选一"
    );
    mgr.execute_action(&room, &pid, "choose:draw").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 2, "魂姿：摸两张");
}

#[test]
fn zhiba_jie_active_pindian_winner_gains_cards() {
    let mgr = manager(3302);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut high, mut low) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_452");
        st.seats[seat].identity = Identity::Lord;
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_309");
        target_pid = pid_of(st, target);
        high = give_filtered(st, seat, |c| c.rank == 13);
        low = give_filtered(st, target, |c| c.rank == 1);
    });
    mgr.execute_action(&room, &pid, "skill:zhiba_jie").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{high}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("choose:{low}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].hand.iter().any(|c| c == &high)
            && st.seats[seat].hand.iter().any(|c| c == &low),
        "界制霸：拼点赢获得两张拼点牌"
    );
}

// ---------------------------------------------------------------------------
// 界夏侯渊 · 神速 / 设变
// ---------------------------------------------------------------------------

#[test]
fn shensu_jie_skip_judge_and_draw_for_sha() {
    let mgr = manager(3303);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_453");
        pid = pid_of(st, seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("shensu"));
    mgr.execute_action(&room, &pid, "choose:shensu:1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("神速")),
        "界神速：跳过判定与摸牌阶段视为使用【杀】"
    );
}

#[test]
fn shebian_moves_equipment_after_flip() {
    let mgr = manager(3304);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut owner, mut equip) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_453");
        pid = pid_of(st, seat);
        owner = other_seats(st, seat)[0];
        set_general(st, owner, "sgs_hero_1");
        equip = give_equip(st, owner);
        take_card(st, &equip);
        let slot = crate::turn::card_def(st, &equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[owner].equips.insert(slot, Some(equip.clone()));
        crate::resolve_jiexian3::shebian_on_flip(st, seat);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("shebian"),
        "设变：翻面时可移动装备"
    );
    let option = pending_options_with(&st, "move:")
        .into_iter()
        .next()
        .expect("移动选项");
    mgr.execute_action(&room, &pid, &option).unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("设变：将")),
        "设变：装备已移动"
    );
}

#[test]
fn shebian_pass_keeps_equipment() {
    let mgr = manager(3305);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut owner, mut equip) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_453");
        pid = pid_of(st, seat);
        owner = other_seats(st, seat)[0];
        set_general(st, owner, "sgs_hero_1");
        equip = give_equip(st, owner);
        take_card(st, &equip);
        let slot = crate::turn::card_def(st, &equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[owner].equips.insert(slot, Some(equip.clone()));
        crate::resolve_jiexian3::shebian_on_flip(st, seat);
        advance_once(st);
    });
    mgr.execute_action(&room, &pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[owner]
            .equips
            .values()
            .any(|c| c.as_deref() == Some(&equip)),
        "设变：不发动则装备不动"
    );
}

// ---------------------------------------------------------------------------
// 界卧龙诸葛亮 · 八阵 / 藏拙 / 火计 / 看破
// ---------------------------------------------------------------------------

#[test]
fn cangzhuo_keeps_tricks_out_of_hand_limit() {
    let mgr = manager(3306);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut base, mut with_tricks, mut after_trick) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_454");
        st.turn_counters.clear();
        base = skills::hand_limit(st, seat);
        give_def(st, seat, "wuzhong_shengyou");
        give_def(st, seat, "juedou");
        with_tricks = skills::hand_limit(st, seat);
        st.bump_counter("trick_used");
        after_trick = skills::hand_limit(st, seat);
    });
    assert_eq!(
        with_tricks,
        base + 2,
        "藏拙：未使用锦囊时锦囊牌不计入手牌上限"
    );
    assert_eq!(after_trick, base, "藏拙：已使用锦囊则正常计入");
}

#[test]
fn huoji_jie_can_use_red_equipment() {
    let mgr = manager(3307);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut equip) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_454");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        // 装备区放一张红牌（任意红色定义）。
        equip = find_free_card(st, |c| skills::is_red(c.suit));
        take_card(st, &equip);
        let slot = crate::turn::card_def(st, &equip)
            .and_then(|d| d.equip_slot)
            .unwrap_or(EquipSlot::Weapon);
        st.seats[seat].equips.insert(slot, Some(equip.clone()));
        give_def(st, other_seats(st, seat)[0], "sha");
    });
    let st = state_of(&mgr, &room);
    let mut acts = crate::turn::legal_actions(&st, &pid);
    let has_equip_action = acts.iter().any(|a| a.id == format!("skill:huoji:{equip}"));
    if !has_equip_action {
        // 若随机装备牌恰为黑色，换成红色装备再试。
        edit(&mgr, &room, |st| {
            let slots: Vec<EquipSlot> = EquipSlot::ALL.to_vec();
            for slot in slots {
                st.seats[seat].equips.insert(slot, None);
            }
            let red_equip = find_free_card(st, |c| {
                skills::is_red(c.suit)
                    && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
            });
            take_card(st, &red_equip);
            let slot = crate::turn::card_def(st, &red_equip)
                .and_then(|d| d.equip_slot)
                .expect("装备槽");
            st.seats[seat].equips.insert(slot, Some(red_equip.clone()));
            equip = red_equip;
        });
        let st = state_of(&mgr, &room);
        acts = crate::turn::legal_actions(&st, &pid);
    }
    assert!(
        acts.iter().any(|a| a.id == format!("skill:huoji:{equip}")),
        "界火计：装备区红色牌可当【火攻】"
    );
    mgr.execute_action(&room, &pid, &format!("skill:huoji:{equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        pending_options_with(&st, "target:").len() > 0,
        "界火计：进入目标选择"
    );
}

#[test]
fn kanpo_jie_equip_black_card_and_no_counter() {
    let mgr = manager(3308);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut user, mut user_pid, mut black_equip, mut trick) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = pid_of(st, user);
        seat = other_seats(st, user)[0];
        set_general(st, seat, "sgs_hero_454");
        pid = pid_of(st, seat);
        black_equip = find_free_card(st, |c| {
            skills::is_black(c.suit)
                && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        take_card(st, &black_equip);
        let slot = crate::turn::card_def(st, &black_equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[seat]
            .equips
            .insert(slot, Some(black_equip.clone()));
        trick = give_def(st, user, "juedou");
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{trick}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{pid}"))
        .unwrap();
    // 依次 pass 直到询问卧龙。
    for _ in 0..8 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.actor == pid {
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(
        pending_options_with(&st, "respond:").contains(&format!("respond:{black_equip}")),
        "界看破：装备区黑牌可当【无懈可击】"
    );
    mgr.execute_action(&room, &pid, &format!("respond:{black_equip}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("效果被抵消")),
        "界看破：无懈不能被响应，直接生效果"
    );
}

// ---------------------------------------------------------------------------
// 界庞统 · 连环 / 涅槃
// ---------------------------------------------------------------------------

#[test]
fn lianhuan_jie_extra_target() {
    let mgr = manager(3309);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_455");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        card = give_filtered(st, seat, |c| c.suit == Suit::Club);
    });
    mgr.execute_action(&room, &pid, &format!("skill:lianhuan:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("lianhuan_pick"),
        "界连环：先选目标"
    );
    let first = pending_options_with(&st, "target:")[0].clone();
    mgr.execute_action(&room, &pid, &first).unwrap();
    let st = state_of(&mgr, &room);
    let second = pending_options_with(&st, "target:")
        .into_iter()
        .find(|o| o != &first)
        .expect("第二个目标");
    mgr.execute_action(&room, &pid, &second).unwrap();
    let st = state_of(&mgr, &room);
    let chained = st
        .history
        .iter()
        .filter(|e| {
            e.text.contains("铁索连环：") && (e.text.contains("横置") || e.text.contains("重置"))
        })
        .count();
    assert_eq!(chained, 2, "界连环：多指定一个目标（两名角色进入连环状态）");
}

#[test]
fn lianhuan_jie_single_target_ok() {
    let mgr = manager(3324);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_455");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        card = give_filtered(st, seat, |c| c.suit == Suit::Club);
    });
    mgr.execute_action(&room, &pid, &format!("skill:lianhuan:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let first = pending_options_with(&st, "target:")[0].clone();
    mgr.execute_action(&room, &pid, &first).unwrap();
    mgr.execute_action(&room, &pid, "keep").unwrap();
    let st = state_of(&mgr, &room);
    let chained = st
        .history
        .iter()
        .filter(|e| {
            e.text.contains("铁索连环：") && (e.text.contains("横置") || e.text.contains("重置"))
        })
        .count();
    assert_eq!(chained, 1, "界连环：也可以只指定一个目标");
}

#[test]
fn niepan_jie_grants_skill_choice() {
    let mgr = manager(3310);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut killer) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        // 选非主公座位，避免主公死亡直接终局。
        seat = other_seats(st, st.turn)
            .into_iter()
            .find(|i| !st.seats[*i].identity.is_lord())
            .expect("非主公座位");
        set_general(st, seat, "sgs_hero_455");
        st.seats[seat].hp = 1;
        pid = pid_of(st, seat);
        killer = other_seats(st, seat)[0];
        give_def(st, seat, "sha");
        set_general(st, killer, "sgs_hero_1");
        damage_and_advance(
            st,
            seat,
            2,
            Some(killer),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Dying, "界庞统进入濒死");
    mgr.execute_action(&room, &pid, "skill:niepan").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("niepan_skill"), "界涅槃：三选一");
    mgr.execute_action(&room, &pid, "choose:kanpo").unwrap();
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "kanpo"), "界涅槃：获得「看破」");
    assert_eq!(
        st.seats[seat].hp,
        3.min(st.seats[seat].max_hp),
        "涅槃回复至 3"
    );
}

// ---------------------------------------------------------------------------
// 界魏延 · 狂骨 / 奇谋
// ---------------------------------------------------------------------------

#[test]
fn qimou_jie_draws_lost_hp() {
    let mgr = manager(3311);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_456");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:qimou").unwrap();
    mgr.execute_action(&room, &pid, "choose:qimou:2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 2, "界奇谋：摸 X 张");
    assert_eq!(st.counter("qimou"), 2);
}

#[test]
fn kuanggu_heals_after_damage() {
    let mgr = manager(3312);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut victim, mut hp) = (0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_456");
        st.seats[seat].hp = st.seats[seat].max_hp - 1;
        stage_play(st, seat);
        pid = pid_of(st, seat);
        victim = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) <= 1)
            .expect("距离 1 内目标");
        set_general(st, victim, "sgs_hero_1");
        damage_and_advance(
            st,
            victim,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
        hp = st.seats[seat].hp;
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("kuanggu"));
    mgr.execute_action(&room, &pid, "choose:heal").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1, "界狂骨：回复 1 点体力");
}

#[test]
fn qimou_jie_pass_keeps_hp() {
    let mgr = manager(3313);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut hp) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_456");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        hp = st.seats[seat].hp;
    });
    mgr.execute_action(&room, &pid, "skill:qimou").unwrap();
    mgr.execute_action(&room, &pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "不发动奇谋不失去体力");
    assert_eq!(st.counter("qimou"), 0);
}

// ---------------------------------------------------------------------------
// 界小乔 · 红颜 / 天香 / 飘零
// ---------------------------------------------------------------------------

#[test]
fn hongyan_jie_hand_limit_equals_max_hp() {
    let mgr = manager(3314);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut with_heart, mut base) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_457");
        st.seats[seat].hp = 1;
        st.turn_counters.clear();
        base = skills::hand_limit(st, seat);
        let heart_equip = find_free_card(st, |c| {
            c.suit == Suit::Heart
                && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        take_card(st, &heart_equip);
        let slot = crate::turn::card_def(st, &heart_equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[seat]
            .equips
            .insert(slot, Some(heart_equip.clone()));
        with_heart = skills::hand_limit(st, seat);
    });
    let max_hp = {
        let st = state_of(&mgr, &room);
        st.seats[seat].max_hp
    };
    assert_eq!(base, 1, "体力 1 → 基础手牌上限 1");
    assert_eq!(
        with_heart, max_hp as usize,
        "界红颜：装备区红桃牌 → 手牌上限 = 体力上限"
    );
}

#[test]
fn tianxiang_jie_can_discard_heart_equipment() {
    let mgr = manager(3315);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut source, mut heart_equip, mut target, mut target_pid) =
        (0, String::new(), 0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_457");
        pid = pid_of(st, seat);
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        target = other_seats(st, seat)[1];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        heart_equip = find_free_card(st, |c| {
            c.suit == Suit::Heart
                && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        take_card(st, &heart_equip);
        let slot = crate::turn::card_def(st, &heart_equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[seat]
            .equips
            .insert(slot, Some(heart_equip.clone()));
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
    assert_eq!(pending_skill(&st), Some("tianxiang"));
    mgr.execute_action(&room, &pid, "choose:tianxiang").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{heart_equip}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "choose:tianxiang:damage")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("天香：弃置")),
        "界天香：可弃置装备区红桃牌"
    );
}

#[test]
fn piaoling_jie_judges_and_gives_card() {
    let mgr = manager(3316);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_457");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        before = st.seats[target].hand.len();
        put_on_top(st, |c| c.suit == Suit::Heart && c.def != "lebu_sishu");
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("piaoling"),
        "飘零：红桃判定后可放置"
    );
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        before + 1,
        "飘零：判定牌交给目标"
    );
}

// ---------------------------------------------------------------------------
// 界孙坚 · 英魂 / 武烈
// ---------------------------------------------------------------------------

#[test]
fn wulie_jie_grants_lie_marks() {
    let mgr = manager(3317);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut hp) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_458");
        st.seats[seat].hp = 3;
        pid = pid_of(st, seat);
        hp = st.seats[seat].hp;
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("wulie"), "武烈：结束阶段限定技");
    mgr.execute_action(&room, &pid, "choose:wulie:2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 2, "武烈：失去 2 点体力");
    let marked = st
        .seats
        .iter()
        .filter(|s| s.mark_counts.get("lie").copied().unwrap_or(0) > 0)
        .count();
    assert_eq!(marked, 2, "武烈：两名角色获得「烈」");
}

#[test]
fn wulie_jie_prevents_damage() {
    let mgr = manager(3318);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut victim, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_1");
        victim = other_seats(st, seat)[0];
        set_general(st, victim, "sgs_hero_458");
        *st.seats[victim]
            .mark_counts
            .entry("lie".into())
            .or_insert(0) += 1;
        hp = st.seats[victim].hp;
        damage_and_advance(
            st,
            victim,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, hp, "武烈：有「烈」时防止伤害");
    assert_eq!(
        st.seats[victim]
            .mark_counts
            .get("lie")
            .copied()
            .unwrap_or(0),
        0,
        "武烈：移除「烈」"
    );
}

#[test]
fn yinghun_jie_unchanged() {
    let mgr = manager(3319);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_458");
        st.seats[seat].hp = 2;
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        before = st.seats[target].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yinghun"), "英魂：准备阶段发动");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let opts: Vec<String> = pending_options_with(&st, "choose:");
    assert!(!opts.is_empty(), "英魂：两项可选");
    mgr.execute_action(&room, &pid, &opts[0]).unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() >= before,
        "英魂：目标摸牌/弃牌结算"
    );
}

// ---------------------------------------------------------------------------
// 界庞德 · 马术 / 鞬出
// ---------------------------------------------------------------------------

#[test]
fn mashu_jie_distance_bonus() {
    let mgr = manager(3320);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut far) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_462");
        far = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) == 2)
            .expect("距离 2 目标");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "mashu"));
    assert!(
        st.in_attack_range(seat, far),
        "马术：距离 2 视为 1（攻击范围内）"
    );
}

#[test]
fn jiandu_jie_basic_card_returns_sha() {
    let mgr = manager(3321);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_462");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "sha");
        sha = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiandu"));
    mgr.execute_action(&room, &pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("其获得此【杀】")),
        "界鞬出：弃置基本牌 → 目标获得此【杀】"
    );
}

#[test]
fn jiandu_jie_non_basic_blocks_dodge_and_boosts() {
    let mgr = manager(3322);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_462");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "wuzhong_shengyou");
        sha = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiandu"));
    mgr.execute_action(&room, &pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("jiandu_sha_bonus"),
        1,
        "界鞬出：非基本牌 → 本回合【杀】次数 +1"
    );
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("其不能抵消此【杀】")),
        "界鞬出：非基本牌 → 不能抵消"
    );
}

#[test]
fn jiandu_jie_pass_falls_back_to_normal_sha() {
    let mgr = manager(3323);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_462");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "sha");
        sha = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiandu_sha_bonus"), 0, "不发动鞬出不增加次数");
    assert_eq!(st.seats[target].player_id, target_pid);
}
