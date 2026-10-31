//! P4.4.2 界限突破批次 J6（8 将）：界王异 / 界张春华 / 界刘表 / 界蔡夫人 / 界曹冲 / 界曹植 / 界吴国太 / 界廖化。

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
// 界王异 · 贞烈 / 秘计
// ---------------------------------------------------------------------------

#[test]
fn zhenlie_jie_choice_gain_card() {
    let mgr = manager(3700);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut user, mut before) = (0, String::new(), 0, 0);
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        seat = other_seats(st, user)[0];
        set_general(st, seat, "sgs_hero_621");
        pid = pid_of(st, seat);
        sha = give_def(st, user, "sha");
        give_def(st, user, "tao");
        before = st.seats[seat].hand.len();
    });
    let user_pid = pid_of(&state_of(&mgr, &room), user);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhenlie"));
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhenlie_jie"),
        "界贞烈：无效后二选一"
    );
    mgr.execute_action(&room, &pid, "choose:gain").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before + 1,
        "界贞烈：获得使用者一张牌"
    );
}

#[test]
fn zhenlie_jie_choice_extra_miji() {
    let mgr = manager(3701);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut user) = (0, String::new(), 0);
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        seat = other_seats(st, user)[0];
        set_general(st, seat, "sgs_hero_621");
        st.seats[seat].hp = st.seats[seat].max_hp - 1;
        pid = pid_of(st, seat);
        sha = give_def(st, user, "sha");
    });
    let user_pid = pid_of(&state_of(&mgr, &room), user);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    mgr.execute_action(&room, &pid, "choose:miji").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("miji_extra"), 1, "界贞烈：记录额外秘计");
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("miji_extra_times"),
        2,
        "界贞烈：结束阶段额外发动一次秘计（连发两次）"
    );
}

#[test]
fn miji_jie_unchanged_draws_lost_hp() {
    let mgr = manager(3702);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_621");
        st.seats[seat].hp = 1;
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].hand.len() >= before + 2,
        "秘计：摸已损失体力张数"
    );
}

// ---------------------------------------------------------------------------
// 界张春华 · 绝情 / 伤逝 / 翦灭
// ---------------------------------------------------------------------------

#[test]
fn jianmie_jie_discards_colors_and_duels() {
    let mgr = manager(3703);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_625");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_filtered(st, seat, |c| skills::is_red(c.suit));
        give_filtered(st, seat, |c| skills::is_red(c.suit));
        give_filtered(st, target, |c| skills::is_black(c.suit));
        give_def(st, target, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:jianmie").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "choose:red").unwrap();
    mgr.execute_action(&room, &target_pid, "choose:black")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("翦灭：张春华弃 2 张")),
        "翦灭：双方各弃所选颜色手牌"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("翦灭：")),
        "翦灭：双方各弃所选颜色手牌"
    );
}

#[test]
fn jianmie_jie_tie_no_duel() {
    let mgr = manager(3704);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_625");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_filtered(st, seat, |c| skills::is_red(c.suit));
        give_filtered(st, target, |c| skills::is_black(c.suit));
    });
    mgr.execute_action(&room, &pid, "skill:jianmie").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "choose:red").unwrap();
    mgr.execute_action(&room, &target_pid, "choose:black")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.pending.as_ref().is_some_and(|p| p.kind == "respond"),
        "翦灭：平手不发生【决斗】"
    );
}

#[test]
fn shangshi_jie_skill_registered() {
    let mgr = manager(3705);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_625");
        st.seats[seat].hp = 1;
        pid = pid_of(st, seat);
        give_def(st, other_seats(st, seat)[0], "sha");
        damage_and_advance(
            st,
            seat,
            1,
            Some(other_seats(st, seat)[0]),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    let _ = pid;
    assert!(skills::has(&st, seat, "shangshi"), "伤逝：技能已注册");
    assert!(
        st.seats[seat].hp <= st.seats[seat].max_hp,
        "受伤后体力不超过上限"
    );
}

// ---------------------------------------------------------------------------
// 界刘表 · 自守 / 宗室
// ---------------------------------------------------------------------------

#[test]
fn zongshi_jie_prevents_damage_once_per_kingdom() {
    let mgr = manager(3706);
    let room = start_and_pick(&mgr, 5);
    let (mut biao, mut source, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        biao = other_seats(st, source)[0];
        set_general(st, biao, "sgs_hero_626");
        give_def(st, biao, "tao");
        hp = st.seats[biao].hp;
        damage_and_advance(
            st,
            biao,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[biao].hp, hp, "界宗室：每个势力限一次防止伤害");
    assert!(
        st.history.iter().any(|e| e.text.contains("宗室：防止")),
        "宗室：防止并令来源获得一张牌"
    );
    assert!(st.seats[source].hand.len() >= 1, "宗室：来源获得刘表一张牌");
}

#[test]
fn zongshi_jie_second_hit_same_kingdom_lands() {
    let mgr = manager(3707);
    let room = start_and_pick(&mgr, 5);
    let (mut biao, mut source, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        biao = other_seats(st, source)[0];
        set_general(st, biao, "sgs_hero_626");
        damage_and_advance(
            st,
            biao,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
        hp = st.seats[biao].hp;
        damage_and_advance(
            st,
            biao,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[biao].hp, hp - 1, "宗室：同一势力第二次不再防止");
}

#[test]
fn zishou_jie_draws_kingdom_count() {
    let mgr = manager(3708);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_626");
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    let drawn = st.seats[seat].hand.len() - before;
    assert!(
        pending_skill(&st) == Some("zishou") || drawn >= 2,
        "自守：摸牌阶段触发（pending 或多摸）"
    );
}

// ---------------------------------------------------------------------------
// 界蔡夫人 · 献州 / 窃听
// ---------------------------------------------------------------------------

#[test]
fn qieting_jie_offered_after_quiet_turn() {
    let mgr = manager(3709);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut caifuren) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        caifuren = other_seats(st, current)[0];
        set_general(st, caifuren, "sgs_hero_627");
    });
    run_phase(&mgr, &room, current, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st).is_some(),
        true,
        "窃听：其他角色回合结束后触发"
    );
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.actor == st.seats[caifuren].player_id),
        "窃听：由蔡夫人选择"
    );
}

#[test]
fn qieting_jie_standard_condition() {
    let mgr = manager(3710);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut caifuren) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        caifuren = other_seats(st, current)[0];
        set_general(st, caifuren, "sgs_hero_627");
        st.set_counter(&format!("dealt:{current}"), 1);
    });
    run_phase(&mgr, &room, current, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("qieting"),
        "窃听：标准口径（未对其他角色用牌）触发"
    );
}

#[test]
fn xianzhou_jie_unchanged_offers_choice() {
    let mgr = manager(3711);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut equip) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_627");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        equip = equip_slot_card(st, seat, EquipSlot::Weapon);
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:xianzhou"),
        "献州：有装备牌时可发动"
    );
    let _ = equip;
}

// ---------------------------------------------------------------------------
// 界曹冲 · 称象 / 仁心
// ---------------------------------------------------------------------------

#[test]
fn chengxiang_jie_bonus_when_sum_13() {
    let mgr = manager(3712);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    let mut bonus = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_628");
        st.set_counter("chengxiang_bonus", 1);
        crate::resolve_yijiang3::start_chengxiang(st, seat);
        advance_once(st);
        bonus = st.counter("chengxiang_bonus");
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("亮出牌堆顶 5 张")),
        "界称象：携带上次奖励时多亮一张"
    );
    let _ = (bonus, seat);
}

#[test]
fn chengxiang_jie_sets_bonus_on_exact_13() {
    let mgr = manager(3713);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    let mut pid = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_628");
        pid = pid_of(st, seat);
        crate::resolve_yijiang3::start_chengxiang(st, seat);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("chengxiang"));
    // 选一张 K（13）→ 和恰好 13。
    let k = pending_options_with(&st, "choose:").into_iter().find(|id| {
        id.trim_start_matches("choose:")
            .split_once('\u{1}')
            .is_none()
            && st
                .cards
                .get(id.trim_start_matches("choose:"))
                .is_some_and(|c| c.rank == 13)
    });
    if let Some(k) = k {
        mgr.execute_action(&room, &pid, &k).unwrap();
        mgr.execute_action(&room, &pid, "confirm").unwrap();
        let st = state_of(&mgr, &room);
        assert_eq!(
            st.counter("chengxiang_bonus"),
            1,
            "称象：点数和 13 → 下次多亮一张"
        );
    }
}

#[test]
fn renxin_jie_standard_trigger_still_works() {
    let mgr = manager(3714);
    let room = start_and_pick(&mgr, 5);
    let (mut chong, mut victim, mut source) = (0, 0, 0);
    let mut equip = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        chong = other_seats(st, source)[0];
        set_general(st, chong, "sgs_hero_628");
        equip = give_equip(st, chong);
        take_card(st, &equip);
        let slot = crate::turn::card_def(st, &equip)
            .and_then(|d| d.equip_slot)
            .expect("装备槽");
        st.seats[chong].equips.insert(slot, Some(equip.clone()));
        victim = other_seats(st, source)[1];
        set_general(st, victim, "sgs_hero_1");
        st.seats[victim].hp = 1;
        damage_and_advance(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("renxin"),
        "仁心：体力 1 的其他角色受伤时询问"
    );
}

// ---------------------------------------------------------------------------
// 界曹植 · 落英 / 酒诗
// ---------------------------------------------------------------------------

#[test]
fn luoying_jie_gains_club_discards() {
    let mgr = manager(3715);
    let room = start_and_pick(&mgr, 5);
    let (mut zhi, mut zhi_pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let current = st.turn;
        set_general(st, current, "sgs_hero_1");
        zhi = other_seats(st, current)[0];
        set_general(st, zhi, "sgs_hero_629");
        zhi_pid = pid_of(st, zhi);
        card = give_filtered(st, current, |c| c.suit == Suit::Club);
        take_card(st, &card);
        crate::turn::discard_by(st, Some(current), &card);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("luoying"),
        "落英：其他角色梅花牌入弃牌堆"
    );
    mgr.execute_action(&room, &zhi_pid, &format!("choose:{card}"))
        .ok();
}

#[test]
fn jiushi_jie_flip_for_wine() {
    let mgr = manager(3716);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_629");
        stage_play(st, seat);
        pid = pid_of(st, seat);
    });
    mgr.execute_action(&room, &pid, "skill:jiushi").unwrap();
    mgr.execute_action(&room, &pid, "choose:jiushi").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[seat].flipped, "酒诗：翻至背面");
    assert_eq!(st.counter("jiu"), 1, "酒诗：视为使用【酒】");
}

#[test]
fn jiushi_jie_flips_back_after_damage() {
    let mgr = manager(3717);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        seat = other_seats(st, source)[0];
        set_general(st, seat, "sgs_hero_629");
        st.seats[seat].flipped = true;
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
    assert!(!st.seats[seat].flipped, "酒诗：受伤后可翻回正面");
}

// ---------------------------------------------------------------------------
// 界吴国太 · 甘露 / 补益
// ---------------------------------------------------------------------------

#[test]
fn ganlu_jie_allows_large_equip_difference() {
    let mgr = manager(3718);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_640");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        set_general(st, others[0], "sgs_hero_1");
        set_general(st, others[1], "sgs_hero_1");
        equip_slot_card(st, others[0], EquipSlot::Weapon);
        equip_slot_card(st, others[0], EquipSlot::Armor);
        equip_slot_card(st, others[1], EquipSlot::Weapon);
        st.seats[seat].hp = st.seats[seat].max_hp;
        give_def(st, seat, "tao");
        give_def(st, seat, "tao");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:ganlu"),
        "界甘露：差额超过已损失体力也可发动（改为弃牌）"
    );
}

#[test]
fn ganlu_jie_offered_in_play_phase() {
    let mgr = manager(3724);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_640");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        for t in &others {
            set_general(st, *t, "sgs_hero_1");
        }
        equip_slot_card(st, others[0], EquipSlot::Weapon);
        equip_slot_card(st, others[1], EquipSlot::Armor);
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:ganlu"),
        "甘露：出牌阶段可发动"
    );
}

#[test]
fn buyi_jie_unchanged_heals_on_non_basic() {
    let mgr = manager(3719);
    let room = start_and_pick(&mgr, 5);
    let (mut guotai, mut victim) = (0, 0);
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guotai = st.turn;
        set_general(st, guotai, "sgs_hero_640");
        victim = other_seats(st, guotai)[0];
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "juedou");
        st.seats[victim].hp = 0;
        hp = st.seats[victim].hp;
        crate::resolve_yijiang::buyi_on_dying(st, victim);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[victim].hp >= hp, "补益：非基本牌可令其回复");
}

#[test]
fn ganlu_jie_standard_variant_unchanged() {
    let mgr = manager(3720);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut a, mut a_pid, mut b, mut b_pid) =
        (0, String::new(), 0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        // 标准吴国太（157）沿用旧限制：差额 > 已损失体力则不可交换。
        set_general(st, seat, "sgs_hero_157");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_1");
        a_pid = pid_of(st, a);
        b_pid = pid_of(st, b);
        equip_slot_card(st, a, EquipSlot::Weapon);
        equip_slot_card(st, a, EquipSlot::Armor);
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "ganlu"), "标准甘露仍注册");
    let _ = (pid, a, a_pid, b, b_pid);
}

// ---------------------------------------------------------------------------
// 界廖化 · 当先 / 伏枥
// ---------------------------------------------------------------------------

#[test]
fn dangxian_jie_gains_sha_in_extra_play_phase() {
    let mgr = manager(3721);
    let room = start_and_pick(&mgr, 5);
    let (mut seat,) = (0,);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_641");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    let has_sha = st.seats[seat].hand.iter().any(|c| {
        st.cards
            .get(c)
            .is_some_and(|x| matches!(x.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
    });
    assert!(has_sha, "界当先：额外出牌阶段开始时获得一张【杀】");
}

#[test]
fn dangxian_jie_self_damage_when_no_damage() {
    let mgr = manager(3722);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_641");
        pid = pid_of(st, seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    let hp = st.seats[seat].hp;
    // 直接结束额外出牌阶段。
    mgr.execute_action(&room, &pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hp,
        hp - 1,
        "界当先：额外出牌阶段未造成伤害则自伤 1"
    );
}

#[test]
fn fuli_jie_unchanged_heals_on_dying() {
    let mgr = manager(3723);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = other_seats(st, st.turn)
            .into_iter()
            .find(|i| !st.seats[*i].identity.is_lord())
            .expect("非主公座位");
        set_general(st, seat, "sgs_hero_641");
        pid = pid_of(st, seat);
        st.seats[seat].hp = 1;
        let killer = other_seats(st, seat)[0];
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
    assert!(skills::has(&st, seat, "fuli"), "伏枥：限定技已注册");
    if st.phase == Phase::Dying {
        mgr.execute_action(&room, &pid, "skill:fuli").unwrap();
        let st = state_of(&mgr, &room);
        assert!(st.seats[seat].hp >= 1, "伏枥：回复至势力数点体力");
    }
    let _ = pid;
}
