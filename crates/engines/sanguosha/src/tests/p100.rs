//! P4.4.2 界限突破批次 J5（8 将）：界凌统 / 界陈宫 / 界曹彰 / 界简雍 / 界虞翻 / 界高顺 / 界法正 / 界程普。

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
// 界凌统 · 旋风
// ---------------------------------------------------------------------------

#[test]
fn xuanfeng_jie_triggers_after_two_discards() {
    let mgr = manager(3600);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut victim, mut victim_pid) = (0, String::new(), 0, String::new());
    let (mut c1, mut c2) = (String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_588");
        pid = pid_of(st, seat);
        victim = other_seats(st, seat)[0];
        set_general(st, victim, "sgs_hero_1");
        victim_pid = pid_of(st, victim);
        give_def(st, victim, "tao");
        c1 = give_filtered(st, seat, |c| c.def == "sha");
        take_card(st, &c1);
        crate::turn::discard_by(st, Some(seat), &c1);
        c2 = give_filtered(st, seat, |c| c.def == "tao");
        take_card(st, &c2);
        crate::turn::discard_by(st, Some(seat), &c2);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("xuanfeng"),
        "界旋风：失去 ≥2 张牌后触发"
    );
    mgr.execute_action(&room, &pid, &format!("target:{victim_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("旋风：弃置")),
        "旋风：弃置其他角色一张牌"
    );
}

#[test]
fn xuanfeng_jie_single_discard_no_trigger() {
    let mgr = manager(3601);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut c1) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_588");
        c1 = give_filtered(st, seat, |c| c.def == "sha");
        take_card(st, &c1);
        crate::turn::discard_by(st, Some(seat), &c1);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_ne!(pending_skill(&st), Some("xuanfeng"), "单张弃置不触发界旋风");
}

#[test]
fn xuanfeng_jie_once_per_phase() {
    let mgr = manager(3602);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_588");
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "tao");
        }
        for _ in 0..2 {
            let c = give_filtered(st, seat, |c| c.def == "sha");
            take_card(st, &c);
            crate::turn::discard_by(st, Some(seat), &c);
        }
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("xuanfeng"));
    let pid = pid_of(&st, seat);
    mgr.execute_action(&room, &pid, "pass").unwrap();
    // 再弃两张也不应再触发（本阶段限一次）。
    edit(&mgr, &room, |st| {
        for _ in 0..2 {
            let c = give_filtered(st, seat, |c| c.def == "tao");
            take_card(st, &c);
            crate::turn::discard_by(st, Some(seat), &c);
        }
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_ne!(pending_skill(&st), Some("xuanfeng"), "旋风本阶段限一次");
}

// ---------------------------------------------------------------------------
// 界陈宫 · 智迟 / 明策
// ---------------------------------------------------------------------------

#[test]
fn zhichi_jie_unchanged_out_of_turn_damage() {
    let mgr = manager(3603);
    let room = start_and_pick(&mgr, 5);
    let (mut chengong, mut source) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        chengong = other_seats(st, source)[0];
        set_general(st, chengong, "sgs_hero_597");
        damage_and_advance(
            st,
            chengong,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter(&format!("zhichi:{chengong}")),
        1,
        "智迟：回合外受伤后生效"
    );
}

#[test]
fn mingce_jie_draw_option_both_draw() {
    let mgr = manager(3604);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    let (mut card, mut before_self, mut before_target) = (String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_597");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        card = give_def(st, seat, "sha");
        before_self = st.seats[seat].hand.len();
        before_target = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:mingce").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:mingce:draw")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before_self,
        "界明策：交出一张、双方各摸一张 → 陈宫手牌数不变"
    );
    assert_eq!(
        st.seats[target].hand.len(),
        before_target + 2,
        "界明策：目标得到交出的牌 +1，再摸 1 → +2"
    );
}

#[test]
fn mingce_jie_sha_option_draws_both_on_damage() {
    let mgr = manager(3605);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut victim, mut victim_pid) =
        (0, String::new(), 0, String::new(), 0, String::new());
    let mut card = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_597");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        target = others[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        victim = others[1];
        set_general(st, victim, "sgs_hero_1");
        victim_pid = pid_of(st, victim);
        card = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:mingce").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:mingce:sha")
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{victim_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("此【杀】造成伤害，双方各摸 1 张")),
        "界明策：杀造成伤害后双方各摸一张"
    );
}

// ---------------------------------------------------------------------------
// 界曹彰 · 将驰
// ---------------------------------------------------------------------------

#[test]
fn jiangchi_jie_opt1_sha_limit_minus_one_not_banned() {
    let mgr = manager(3606);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_599");
        pid = pid_of(st, seat);
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("jiangchi"),
        "将驰：摸牌阶段结束时触发"
    );
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !skills::sha_play_banned(&st, seat),
        "界将驰①：仍可使用【杀】（次数 -1）"
    );
    assert_eq!(st.counter("sha_limit_penalty"), 1);
    assert!(st.seats[seat].hand.len() > before, "将驰①：摸一张牌");
}

#[test]
fn jiangchi_jie_opt2_recasts_and_bonus() {
    let mgr = manager(3607);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card, mut before) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_599");
        pid = pid_of(st, seat);
        card = give_def(st, seat, "tao");
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    let opts: Vec<String> = pending_options_with(&st, "opt2");
    assert!(
        !opts.is_empty()
            || st
                .pending
                .as_ref()
                .is_some_and(|p| p.skill.as_deref() == Some("jiangchi"))
    );
    mgr.execute_action(&room, &pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    let pick = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("重铸选牌");
    mgr.execute_action(&room, &pid, &pick).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiangchi_bonus"), 1);
    assert_eq!(st.counter("jiangchi_no_distance"), 1);
    assert!(
        st.seats[seat].hand.len() >= before,
        "界将驰②：重铸（弃 1 摸 1）"
    );
    let _ = card;
}

#[test]
fn jiangchi_jie_opt1_hand_limit_counts_sha() {
    let mgr = manager(3608);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    let (mut with_sha, mut without) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_599");
        st.set_counter(&format!("sha_banned:{seat}"), 1);
        st.set_counter(&format!("jiangchi_no_ban:{seat}"), 1);
        st.turn_counters.insert("sha_banned".into(), 1);
        without = skills::hand_limit(st, seat);
        give_def(st, seat, "sha");
        give_def(st, seat, "sha");
        with_sha = skills::hand_limit(st, seat);
    });
    assert_eq!(with_sha, without + 2, "界将驰①：【杀】不计入手牌上限");
}

// ---------------------------------------------------------------------------
// 界简雍 · 纵适 / 巧说
// ---------------------------------------------------------------------------

#[test]
fn qiaoshuo_jie_active_action_and_pindian() {
    let mgr = manager(3609);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_600");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_rank(st, seat, 13);
        give_rank(st, target, 1);
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:qiaoshuo"),
        "界巧说：出牌阶段可主动发动"
    );
    mgr.execute_action(&room, &pid, "skill:qiaoshuo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("qiaoshuo_used"), 1);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.skill.as_deref() == Some("pindian")),
        "巧说：进入拼点"
    );
}

#[test]
fn qiaoshuo_jie_fail_blocks_tricks() {
    let mgr = manager(3610);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    let mut low = String::new();
    let mut trick = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_600");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        low = give_rank(st, seat, 1);
        give_rank(st, target, 13);
        trick = give_def(st, seat, "juedou");
    });
    mgr.execute_action(&room, &pid, "skill:qiaoshuo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{low}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:__deck")
        .err()
        .map(|_| ());
    // 目标选牌（若随机已自动则由 advance 完成）。
    let st = state_of(&mgr, &room);
    if st
        .pending
        .as_ref()
        .is_some_and(|p| p.skill.as_deref() == Some("pindian"))
    {
        let own = pending_options_with(&st, "choose:")
            .into_iter()
            .next()
            .expect("拼点牌");
        mgr.execute_action(&room, &target_pid, &own).unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("qiaoshuo_fail"),
        1,
        "巧说没赢：本回合不能使用锦囊"
    );
    assert!(
        mgr.execute_action(&room, &pid, &format!("play:{trick}"))
            .is_err(),
        "巧说没赢：锦囊被封锁"
    );
}

#[test]
fn qiaoshuo_jie_once_per_phase() {
    let mgr = manager(3611);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_600");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_rank(st, seat, 13);
        give_rank(st, target, 1);
    });
    mgr.execute_action(&room, &pid, "skill:qiaoshuo").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let own = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("拼点牌");
    mgr.execute_action(&room, &pid, &own).unwrap();
    let st = state_of(&mgr, &room);
    if st
        .pending
        .as_ref()
        .is_some_and(|p| p.skill.as_deref() == Some("pindian"))
    {
        let other = pending_options_with(&st, "choose:")
            .into_iter()
            .next()
            .expect("对方拼点牌");
        mgr.execute_action(&room, &target_pid, &other).unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(
        !crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:qiaoshuo"),
        "界巧说：本阶段限一次"
    );
}

// ---------------------------------------------------------------------------
// 界虞翻 · 纵玄 / 直言
// ---------------------------------------------------------------------------

#[test]
fn zhiyan_jie_upper_end_phase_triggers() {
    let mgr = manager(3612);
    let room = start_and_pick(&mgr, 5);
    let (mut upper, mut yufan, mut yufan_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        upper = st.turn;
        set_general(st, upper, "sgs_hero_1");
        // 挑选上家为 upper 的座位。
        yufan = other_seats(st, upper)
            .into_iter()
            .find(|i| crate::resolve_jiexian6::upper_seat(st, *i) == Some(upper))
            .expect("上家匹配");
        set_general(st, yufan, "sgs_hero_603");
        yufan_pid = pid_of(st, yufan);
        give_def(st, yufan, "tao");
    });
    run_phase(&mgr, &room, upper, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhiyan"),
        "界直言：上家的结束阶段触发"
    );
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(yufan_pid.as_str())
    );
}

#[test]
fn zhiyan_jie_non_equip_loses_hp() {
    let mgr = manager(3613);
    let room = start_and_pick(&mgr, 5);
    let (mut yufan, mut yufan_pid, mut target, mut target_pid) =
        (0, String::new(), 0, String::new());
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yufan = st.turn;
        set_general(st, yufan, "sgs_hero_603");
        st.seats[yufan].hp = 3;
        yufan_pid = pid_of(st, yufan);
        target = other_seats(st, yufan)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = 4;
        target_pid = pid_of(st, target);
        put_on_top(st, |c| {
            !crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        hp = st.seats[target].hp;
        crate::resolve::push(
            st,
            crate::resolve::Frame::ZhiYan(crate::resolve_yijiang4::ZhiYanFrame { actor: yufan }),
        );
        advance_once(st);
    });
    mgr.execute_action(&room, &yufan_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp - 1,
        "界直言：非装备且体力不同 → 失去 1 点体力"
    );
}

#[test]
fn zhiyan_jie_equip_heals() {
    let mgr = manager(3614);
    let room = start_and_pick(&mgr, 5);
    let (mut yufan, mut yufan_pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yufan = st.turn;
        set_general(st, yufan, "sgs_hero_603");
        yufan_pid = pid_of(st, yufan);
        target = other_seats(st, yufan)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = st.seats[target].max_hp - 1;
        hp = st.seats[target].hp;
        target_pid = pid_of(st, target);
        put_on_top(st, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        crate::resolve::push(
            st,
            crate::resolve::Frame::ZhiYan(crate::resolve_yijiang4::ZhiYanFrame { actor: yufan }),
        );
        advance_once(st);
    });
    mgr.execute_action(&room, &yufan_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp + 1,
        "直言：装备牌 → 使用并回复 1 点体力"
    );
}

// ---------------------------------------------------------------------------
// 界高顺 · 陷阵 / 禁酒
// ---------------------------------------------------------------------------

#[test]
fn jinjiu_jie_blocks_others_jiu_in_gaoshun_turn() {
    let mgr = manager(3615);
    let room = start_and_pick(&mgr, 5);
    let (mut gaoshun, mut other, mut other_pid, mut jiu) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        gaoshun = st.turn;
        set_general(st, gaoshun, "sgs_hero_604");
        other = other_seats(st, gaoshun)[0];
        set_general(st, other, "sgs_hero_1");
        other_pid = pid_of(st, other);
        jiu = give_def(st, other, "jiu");
    });
    assert!(
        mgr.execute_action(&room, &other_pid, &format!("play:{jiu}"))
            .is_err(),
        "界禁酒：高顺回合其他角色不能使用【酒】"
    );
}

#[test]
fn jinjiu_jie_other_turn_allows_jiu() {
    let mgr = manager(3616);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut jiu) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = pid_of(st, user);
        let gs = other_seats(st, user)[0];
        set_general(st, gs, "sgs_hero_604");
        jiu = give_def(st, user, "jiu");
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{jiu}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("jiu_used"), 1, "非高顺回合可正常使用【酒】");
}

#[test]
fn xianzhen_jie_unchanged_pindian_action() {
    let mgr = manager(3617);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_604");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        give_def(st, seat, "sha");
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "sha");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:xianzhen"),
        "陷阵：可发动拼点"
    );
}

// ---------------------------------------------------------------------------
// 界法正 · 眩惑 / 恩怨
// ---------------------------------------------------------------------------

#[test]
fn enyuan_jie_requires_red_hand_card() {
    let mgr = manager(3618);
    let room = start_and_pick(&mgr, 5);
    let (mut fazheng, mut source) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        fazheng = other_seats(st, source)[0];
        set_general(st, fazheng, "sgs_hero_610");
        // 来源只有黑色手牌 → 界恩怨不可交牌。
        give_filtered(st, source, |c| skills::is_black(c.suit));
        damage_and_advance(
            st,
            fazheng,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("enyuan"));
    let opts = pending_options_with(&st, "opt1");
    assert!(opts.is_empty(), "界恩怨：无红色手牌时不能交牌");
}

#[test]
fn enyuan_jie_gives_red_card() {
    let mgr = manager(3619);
    let room = start_and_pick(&mgr, 5);
    let (mut fazheng, mut source, mut source_pid, mut red) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        fazheng = other_seats(st, source)[0];
        set_general(st, fazheng, "sgs_hero_610");
        source_pid = pid_of(st, source);
        red = give_filtered(st, source, |c| skills::is_red(c.suit));
        give_filtered(st, source, |c| skills::is_black(c.suit));
        damage_and_advance(
            st,
            fazheng,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    mgr.execute_action(&room, &source_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[fazheng].hand.iter().any(|c| c == &red),
        "界恩怨：来源交给法正一张红色手牌"
    );
}

#[test]
fn xuanhuo_jie_offered_at_draw_phase() {
    let mgr = manager(3620);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_610");
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "tao");
        }
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("xuanhuo"),
        "眩惑：界法正摸牌阶段仍可发动"
    );
}

// ---------------------------------------------------------------------------
// 界程普 · 疠火 / 醇醪
// ---------------------------------------------------------------------------

#[test]
fn lihuo_jie_damage_discards_card_as_cost() {
    let mgr = manager(3621);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut victim, mut before) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_620");
        victim = other_seats(st, seat)[0];
        set_general(st, victim, "sgs_hero_1");
        give_def(st, seat, "tao");
        before = st.seats[seat].hand.len();
        let card = find_free_card(st, |c| c.def == "sha");
        st.set_counter(&format!("lihuo:{card}"), 1);
        damage_and_advance(
            st,
            victim,
            1,
            Some(seat),
            crate::resolve::DamageType::Fire,
            Some(&card),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before - 1,
        "界疠火：弃置一张牌作为代价"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("疠火：弃置")),
        "疠火：代价记录"
    );
}

#[test]
fn lihuo_jie_loses_hp_without_cards() {
    let mgr = manager(3622);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut victim, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_620");
        victim = other_seats(st, seat)[0];
        set_general(st, victim, "sgs_hero_1");
        hp = st.seats[seat].hp;
        let card = find_free_card(st, |c| c.def == "sha");
        st.set_counter(&format!("lihuo:{card}"), 1);
        damage_and_advance(
            st,
            victim,
            1,
            Some(seat),
            crate::resolve::DamageType::Fire,
            Some(&card),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 1, "界疠火：无牌则失去 1 点体力");
}

#[test]
fn chunlao_jie_places_sha_from_adjacent_discard() {
    let mgr = manager(3623);
    let room = start_and_pick(&mgr, 5);
    let (mut chengpu, mut other, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        chengpu = st.turn;
        set_general(st, chengpu, "sgs_hero_620");
        other = other_seats(st, chengpu)
            .into_iter()
            .find(|t| crate::resolve_jiexian6::adjacent(st, chengpu, *t))
            .expect("相邻座位");
        set_general(st, other, "sgs_hero_1");
        sha = give_filtered(st, other, |c| c.def == "sha");
        take_card(st, &sha);
        crate::turn::discard_by(st, Some(other), &sha);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[chengpu]
            .marks
            .get("chun")
            .is_some_and(|v| v.iter().any(|c| c == &sha)),
        "界醇醪：相邻角色的【杀】因弃置进入「醇」"
    );
}
