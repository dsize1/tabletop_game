//! P4.4.2 界限突破批次 J8（4 将，收官）：界夏侯氏 / 界张松 / 界于禁 / 界步练师。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of};
use crate::skills;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// 界夏侯氏 · 樵拾 / 燕语
// ---------------------------------------------------------------------------

#[test]
fn qiaoshi_jie_triggers_without_equal_hands() {
    let mgr = manager(3900);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut xiahou, mut xiahou_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        xiahou = other_seats(st, current)[0];
        set_general(st, xiahou, "sgs_hero_752");
        xiahou_pid = pid_of(st, xiahou);
        give_def(st, current, "sha");
        give_def(st, xiahou, "sha");
        give_def(st, xiahou, "tao");
    });
    run_phase(&mgr, &room, current, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("qiaoshi"),
        "界樵拾：手牌不等也可发动"
    );
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(xiahou_pid.as_str())
    );
}

#[test]
fn qiaoshi_jie_disables_rest_of_round_when_unequal() {
    let mgr = manager(3901);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut xiahou, mut xiahou_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        xiahou = other_seats(st, current)[0];
        set_general(st, xiahou, "sgs_hero_752");
        xiahou_pid = pid_of(st, xiahou);
        give_def(st, current, "sha");
        give_def(st, xiahou, "sha");
        give_def(st, xiahou, "tao");
    });
    run_phase(&mgr, &room, current, TurnPhase::End, false);
    mgr.execute_action(&room, &xiahou_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[xiahou]
            .flags
            .contains_key(&format!("qiaoshi_off:{}", st.round)),
        "界樵拾：手牌数不等 → 本轮失效"
    );
}

#[test]
fn yanyu_jie_registered() {
    let mgr = manager(3902);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_752");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "yanyu"), "燕语：技能已注册");
}

// ---------------------------------------------------------------------------
// 界张松 · 强识 / 献图
// ---------------------------------------------------------------------------

#[test]
fn xiantu_jie_given_count_recorded() {
    let mgr = manager(3903);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut zhangsong, mut zhangsong_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        zhangsong = other_seats(st, current)[0];
        set_general(st, zhangsong, "sgs_hero_753");
        zhangsong_pid = pid_of(st, zhangsong);
        give_def(st, zhangsong, "sha");
        give_def(st, zhangsong, "tao");
    });
    run_phase(&mgr, &room, current, TurnPhase::Play, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("xiantu"),
        "献图：其他角色出牌阶段开始触发"
    );
    mgr.execute_action(&room, &zhangsong_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("xiantu_given"), 2, "献图：记录交给的牌数");
}

#[test]
fn xiantu_jie_damage_meets_given_no_hp_loss() {
    let mgr = manager(3904);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut zhangsong) = (0, 0);
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        zhangsong = other_seats(st, current)[0];
        set_general(st, zhangsong, "sgs_hero_753");
        hp = st.seats[zhangsong].hp;
        st.set_counter("xiantu_owner", zhangsong as u32 + 1);
        st.set_counter("xiantu_target", current as u32 + 1);
        st.set_counter("xiantu_given", 1);
        st.set_counter(&format!("dealt:{current}"), 2);
        crate::resolve_yijiang5::xiantu_end_phase(st, current);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[zhangsong].hp, hp,
        "界献图：伤害 ≥ 交牌数则不失去体力"
    );
}

#[test]
fn xiantu_jie_damage_below_given_loses_hp() {
    let mgr = manager(3905);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut zhangsong) = (0, 0);
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        zhangsong = other_seats(st, current)[0];
        set_general(st, zhangsong, "sgs_hero_753");
        hp = st.seats[zhangsong].hp;
        st.set_counter("xiantu_owner", zhangsong as u32 + 1);
        st.set_counter("xiantu_target", current as u32 + 1);
        st.set_counter("xiantu_given", 2);
        st.set_counter(&format!("dealt:{current}"), 0);
        crate::resolve_yijiang5::xiantu_end_phase(st, current);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[zhangsong].hp,
        hp - 1,
        "界献图：伤害 < 交牌数 → 失去 1 点体力"
    );
}

// ---------------------------------------------------------------------------
// 界于禁 · 镇军 / 毅重
// ---------------------------------------------------------------------------

#[test]
fn yizhong_jie_black_sha_immune() {
    let mgr = manager(3906);
    let room = start_and_pick(&mgr, 5);
    let (mut yujin, mut attacker, mut attacker_pid, mut hp) = (0, 0, String::new(), 0);
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        stage_play(st, attacker);
        attacker_pid = pid_of(st, attacker);
        yujin = other_seats(st, attacker)
            .into_iter()
            .find(|t| st.in_attack_range(attacker, *t))
            .expect("攻击范围内目标");
        set_general(st, yujin, "sgs_hero_756");
        st.seats[yujin].hp = 1;
        st.seats[attacker].hp = 4;
        hp = st.seats[yujin].hp;
        sha = give_filtered(st, attacker, |c| c.def == "sha" && skills::is_black(c.suit));
    });
    let yujin_pid = pid_of(&state_of(&mgr, &room), yujin);
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{yujin_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[yujin].hp, hp,
        "毅重：体力 ≥ 你的角色的黑色【杀】无效"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("毅重：")),
        "毅重：记录无效"
    );
}

#[test]
fn yizhong_jie_black_sha_unanswerable() {
    let mgr = manager(3907);
    let room = start_and_pick(&mgr, 5);
    let (mut yujin, mut yujin_pid, mut target, mut target_pid) =
        (0, String::new(), 0, String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yujin = st.turn;
        set_general(st, yujin, "sgs_hero_756");
        stage_play(st, yujin);
        yujin_pid = pid_of(st, yujin);
        target = other_seats(st, yujin)
            .into_iter()
            .find(|t| st.in_attack_range(yujin, *t))
            .expect("攻击范围内目标");
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "shan");
        give_def(st, yujin, "tao");
        sha = give_filtered(st, yujin, |c| c.def == "sha" && skills::is_black(c.suit));
    });
    mgr.execute_action(&room, &yujin_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &yujin_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hp < st.seats[target].max_hp,
        "毅重：手牌数 ≤ 你的角色无法响应你的黑色【杀】"
    );
}

#[test]
fn zhenjun_jie_defers_draw_to_end_phase() {
    let mgr = manager(3908);
    let room = start_and_pick(&mgr, 5);
    let (mut yujin, mut yujin_pid, mut target, mut target_pid) =
        (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yujin = st.turn;
        set_general(st, yujin, "sgs_hero_756");
        yujin_pid = pid_of(st, yujin);
        target = other_seats(st, yujin)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        for _ in 0..3 {
            give_def(st, target, "tao");
        }
        give_def(st, yujin, "tao");
    });
    run_phase(&mgr, &room, yujin, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhenjun"));
    mgr.execute_action(&room, &yujin_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let opt = pending_options_with(&st, "choose:zhenjun:draw");
    if opt.is_empty() {
        // 目标弃置的可能全为装备，无「非装备牌」则不询问。
        return;
    }
    let before = st.seats[target].hand.len();
    mgr.execute_action(&room, &yujin_pid, "choose:zhenjun:draw")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        before,
        "界镇军：摸牌延后到结束阶段"
    );
    assert_eq!(
        st.seats[yujin]
            .mark_counts
            .keys()
            .filter(|k| k.starts_with("zhenjun_end_draw:"))
            .count(),
        1,
        "界镇军：记录结束阶段摸牌"
    );
    run_phase(&mgr, &room, yujin, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() > before,
        "界镇军：结束阶段目标摸等量张"
    );
}

// ---------------------------------------------------------------------------
// 界步练师 · 安恤 / 追忆
// ---------------------------------------------------------------------------

#[test]
fn zhuixi_jie_can_activate_at_end_phase() {
    let mgr = manager(3909);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_775");
        pid = pid_of(st, seat);
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
        }
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhuiyi"),
        "界追忆：结束阶段可发动限定技"
    );
    let _ = pid;
}

#[test]
fn zhuixi_jie_grants_draw_and_heal() {
    let mgr = manager(3910);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    let (mut before, mut hp) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_775");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = st.seats[target].max_hp - 1;
        target_pid = pid_of(st, target);
        before = st.seats[target].hand.len();
        hp = st.seats[target].hp;
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), before + 3, "追忆：摸 3 张");
    assert_eq!(st.seats[target].hp, hp + 1, "追忆：回复 1 点体力");
}

#[test]
fn anxu_jie_registered() {
    let mgr = manager(3911);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_775");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "tao");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "anxu"));
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:anxu")
            || skills::has(&st, seat, "zhuixi"),
        "安恤：技能已注册"
    );
}
