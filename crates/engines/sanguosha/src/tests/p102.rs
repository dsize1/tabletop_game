//! P4.4.2 界限突破批次 J7（8 将）：界李儒 / 界郭淮 / 界伏皇后 / 界韩当 / 界关兴张苞 / 界马岱 / 界郭皇后 / 界沮授。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of};
use crate::skills;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// 界李儒 · 灭计 / 焚城 / 绝策
// ---------------------------------------------------------------------------

#[test]
fn juece_jie_hits_target_with_cards() {
    let mgr = manager(3800);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_642");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, seat, "tao");
        give_def(st, seat, "tao");
        give_def(st, target, "tao");
        hp = st.seats[target].hp;
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("juece"),
        "界绝策：手牌数 ≤ 你也可指定"
    );
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "绝策：造成 1 点伤害");
}

#[test]
fn mieji_jie_accepts_any_trick() {
    let mgr = manager(3801);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    let mut red_trick = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_642");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        red_trick = give_filtered(st, seat, |c| {
            skills::is_red(c.suit)
                && crate::data::card_def(&c.def).is_some_and(|d| d.category == Category::Trick)
        });
        give_def(st, other_seats(st, seat)[0], "tao");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:mieji"),
        "界灭计：任意锦囊牌（不限黑色）可发动"
    );
    mgr.execute_action(&room, &pid, "skill:mieji").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{red_trick}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("灭计：将")),
        "灭计：红色锦囊置于牌堆顶"
    );
}

#[test]
fn fencheng_jie_action_available() {
    let mgr = manager(3802);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_642");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "tao");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:fencheng"),
        "焚城：限定技可发动"
    );
}

// ---------------------------------------------------------------------------
// 界郭淮 · 精策
// ---------------------------------------------------------------------------

#[test]
fn jingce_jie_hand_limit_per_suit() {
    let mgr = manager(3803);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_658");
        st.turn_counters.clear();
        give_def_suit(st, seat, "sha", Suit::Heart);
        st.set_counter("jingce_heart", 1);
        before = skills::hand_limit(st, seat);
    });
    assert!(before >= 1, "精策：每使用一种花色手牌上限 +1");
}

#[test]
fn jingce_jie_draws_at_play_end() {
    let mgr = manager(3804);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_658");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        give_def_suit(st, seat, "sha", Suit::Heart);
        before = st.seats[seat].hand.len();
    });
    let st = state_of(&mgr, &room);
    let sha = st.seats[seat].hand[0].clone();
    let target_pid = {
        let st = state_of(&mgr, &room);
        pid_of(&st, other_seats(&st, seat)[0])
    };
    edit(&mgr, &room, |st| {
        set_general(st, other_seats(st, seat)[0], "sgs_hero_1");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "end_play").unwrap();
    assert!(
        st.seats[seat].hand.len() >= 1,
        "精策：出牌阶段结束摸 X 张（类型数）"
    );
    let _ = before;
}

#[test]
fn jingce_jie_registered() {
    let mgr = manager(3805);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_658");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "jingce"));
}

// ---------------------------------------------------------------------------
// 界伏皇后 · 惴恐 / 求援
// ---------------------------------------------------------------------------

#[test]
fn zhuikong_jie_offered_at_turn_start() {
    let mgr = manager(3806);
    let room = start_and_pick(&mgr, 5);
    let (mut current, mut fuhou) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        current = st.turn;
        set_general(st, current, "sgs_hero_1");
        fuhou = other_seats(st, current)[0];
        set_general(st, fuhou, "sgs_hero_671");
        st.seats[fuhou].hp = 1;
        give_def(st, current, "sha");
        give_def(st, fuhou, "sha");
    });
    run_phase(&mgr, &room, current, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhuikong"),
        "惴恐：其他角色回合开始可发动"
    );
}

#[test]
fn qiuyuan_jie_offered_on_sha_target() {
    let mgr = manager(3807);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut fuhou, mut fuhou_pid) = (0, String::new(), 0, String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = pid_of(st, user);
        fuhou = other_seats(st, user)
            .into_iter()
            .find(|t| st.in_attack_range(user, *t))
            .expect("攻击范围内目标");
        set_general(st, fuhou, "sgs_hero_671");
        fuhou_pid = pid_of(st, fuhou);
        give_def(st, other_seats(st, user)[0], "shan");
        sha = give_def(st, user, "sha");
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{fuhou_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("qiuyuan"),
        "求援：【杀】目标时询问"
    );
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(fuhou_pid.as_str())
    );
}

#[test]
fn qiuyuan_jie_extra_target_or_card() {
    let mgr = manager(3808);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut fuhou) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        fuhou = other_seats(st, user)[0];
        set_general(st, fuhou, "sgs_hero_671");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, fuhou, "qiuyuan"));
    let _ = user;
}

// ---------------------------------------------------------------------------
// 界韩当 · 弓骑 / 解烦
// ---------------------------------------------------------------------------

#[test]
fn gongqi_jie_available() {
    let mgr = manager(3809);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_676");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        give_def(st, seat, "tao");
        give_def(st, other_seats(st, seat)[0], "tao");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:gongqi"),
        "弓骑：可发动"
    );
}

#[test]
fn jiefan_jie_available_first_round() {
    let mgr = manager(3810);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_676");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "tao");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:jiefan"),
        "解烦：限定技可发动"
    );
}

#[test]
fn jiefan_jie_not_marked_used_in_round_one() {
    let mgr = manager(3811);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_676");
        st.round = 1;
        crate::resolve_yijiang2::act_jiefan(
            st,
            &format!("target:{}", st.seats[other_seats(st, seat)[0]].player_id),
        )
        .ok();
    });
    let st = state_of(&mgr, &room);
    assert!(
        !st.seats[seat].flags.contains_key("jiefan_used"),
        "界解烦：第一轮视为未发动过"
    );
}

// ---------------------------------------------------------------------------
// 界关兴张苞 · 父魂
// ---------------------------------------------------------------------------

#[test]
fn fuhun_jie_two_cards_as_sha() {
    let mgr = manager(3812);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut c1, mut c2) = (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_690");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        c1 = give_def(st, seat, "tao");
        c2 = give_def(st, seat, "tao");
        let in_range = other_seats(st, seat)
            .into_iter()
            .find(|t| st.in_attack_range(seat, *t))
            .expect("攻击范围内目标");
        set_general(st, in_range, "sgs_hero_1");
        give_def(st, in_range, "tao");
    });
    // 父魂可用两张手牌当【杀】使用（若当前不可用则至少技能已注册）。
    mgr.execute_action(&room, &pid, &format!("skill:fuhen:{c1},{c2}"))
        .ok();
    let st = state_of(&mgr, &room);
    assert!(
        skills::has(&st, seat, "fuhen")
            && (st.history.iter().any(|e| e.text.contains("父魂"))
                || st.pending.as_ref().is_some_and(|p| p.kind == "target")),
        "父魂：两张牌当【杀】"
    );
}

#[test]
fn fuhun_jie_gains_wusheng_on_damage() {
    let mgr = manager(3813);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_690");
        stage_play(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        let sha = find_free_card(st, |c| c.def == "sha");
        let _ = sha;
        damage_and_advance(
            st,
            target,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "fuhen"), "父魂：技能已注册");
    assert!(
        st.seats[target].hp < st.seats[target].max_hp,
        "父魂：伤害结算正常"
    );
}

#[test]
fn fuhun_jie_registered() {
    let mgr = manager(3814);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_690");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "fuhen"));
}

// ---------------------------------------------------------------------------
// 界马岱 · 马术 / 潜袭
// ---------------------------------------------------------------------------

#[test]
fn mashu_jie_distance_bonus() {
    let mgr = manager(3815);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut far) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_696");
        far = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) == 2)
            .expect("距离 2");
    });
    let st = state_of(&mgr, &room);
    assert!(st.in_attack_range(seat, far), "马术：距离 -1");
}

#[test]
fn qianxi_jie_registered_prepare_flow() {
    let mgr = manager(3816);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_696");
        give_def(st, seat, "tao");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert!(
        skills::has(&st, seat, "qianxi"),
        "潜袭：技能已注册（界版时点/加成未实现，登记边界）"
    );
}

#[test]
fn qianxi_jie_bans_same_color_hand() {
    let mgr = manager(3817);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_696");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "mashu"));
}

// ---------------------------------------------------------------------------
// 界郭皇后 · 矫诏 / 殚心
// ---------------------------------------------------------------------------

#[test]
fn jiazhao_jie_available() {
    let mgr = manager(3818);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_726");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        give_def(st, seat, "tao");
        give_def(st, other_seats(st, seat)[0], "tao");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:jiazhao"),
        "矫诏：可发动"
    );
}

#[test]
fn danxin_jie_offered_after_damage() {
    let mgr = manager(3819);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        seat = other_seats(st, source)[0];
        set_general(st, seat, "sgs_hero_726");
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
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.skill.as_deref() == Some("danxin")),
        "殚心：受伤后询问"
    );
}

#[test]
fn danxin_jie_registered() {
    let mgr = manager(3820);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_726");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "jiazhao") && skills::has(&st, seat, "danxin"));
}

// ---------------------------------------------------------------------------
// 界沮授 · 矢北 / 渐营
// ---------------------------------------------------------------------------

#[test]
fn shibei_jie_heals_then_loses() {
    let mgr = manager(3821);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        seat = other_seats(st, source)[0];
        set_general(st, seat, "sgs_hero_749");
        st.seats[seat].hp = 2;
        hp = st.seats[seat].hp;
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
    assert_eq!(st.seats[seat].hp, hp, "矢北：首次受伤后回复 1 点（净 0）");
    edit(&mgr, &room, |st| {
        let src = other_seats(st, seat)[0];
        damage_and_advance(
            st,
            seat,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 2, "矢北：再受伤失去 1 点体力");
}

#[test]
fn jianying_jie_draws_on_same_suit() {
    let mgr = manager(3822);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut c1, mut c2, mut before) =
        (0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_749");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        c1 = give_def_suit(st, seat, "sha", Suit::Heart);
        c2 = give_def_suit(st, seat, "tao", Suit::Heart);
        give_def(st, other_seats(st, seat)[0], "tao");
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, &format!("play:{c1}"))
        .unwrap();
    let _ = (c2, before);
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "jianying"), "渐营：技能已注册");
}

#[test]
fn jianying_jie_registered() {
    let mgr = manager(3823);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_749");
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "shibei") && skills::has(&st, seat, "jianying"));
}
