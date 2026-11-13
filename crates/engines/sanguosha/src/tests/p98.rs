//! P4.4.2 界限突破批次 J3（8 将）：界太史慈 / 界董卓 / 界邓艾 / 界祝融 / 界姜维 / 界贾诩 / 界鲁肃 / 界张郃。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of};
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
// 界太史慈 · 天义 / 酣战
// ---------------------------------------------------------------------------

#[test]
fn hanzhan_jie_random_pindian_and_gains_biggest_sha() {
    let mgr = manager(3400);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut big) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_463");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        big = give_filtered(st, seat, |c| c.def == "sha" && c.rank >= 10);
        give_filtered(st, target, |c| c.def == "sha" && c.rank <= 5);
    });
    mgr.execute_action(&room, &pid, "skill:tianyi").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{big}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("酣战：") && e.text.contains("随机手牌")),
        "酣战：对方随机手牌拼点"
    );
    assert!(
        st.seats[seat].hand.iter().any(|c| c == &big),
        "酣战：获得拼点牌中点数最大的【杀】"
    );
}

#[test]
fn hanzhan_jie_no_sha_no_gain() {
    let mgr = manager(3401);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_463");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_filtered(st, seat, |c| c.def == "tao");
        give_filtered(st, target, |c| c.def == "tao");
    });
    mgr.execute_action(&room, &pid, "skill:tianyi").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let own = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("拼点手牌");
    mgr.execute_action(&room, &pid, &own).unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.history
            .iter()
            .any(|e| e.text.contains("点数最大的【杀】")),
        "非【杀】拼点牌不获得"
    );
}

#[test]
fn hanzhan_jie_auto_random_only_for_opponent() {
    let mgr = manager(3402);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_463");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_filtered(st, seat, |c| c.def == "sha");
        give_filtered(st, target, |c| c.def == "sha");
        give_filtered(st, target, |c| c.def == "sha");
    });
    mgr.execute_action(&room, &pid, "skill:tianyi").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(pid.as_str()),
        "酣战：太史慈自己正常选牌"
    );
    let own = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("拼点手牌");
    mgr.execute_action(&room, &pid, &own).unwrap();
    let st = state_of(&mgr, &room);
    assert_ne!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(target_pid.as_str()),
        "酣战：对方由随机手牌代选（无询问）"
    );
}

// ---------------------------------------------------------------------------
// 界董卓 · 酒池 / 暴虐 / 崩坏
// ---------------------------------------------------------------------------

#[test]
fn jiuchi_jie_unlimited_jiu_per_turn() {
    let mgr = manager(3403);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut c1, mut c2) = (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_470");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        c1 = give_filtered(st, seat, |c| c.suit == Suit::Spade);
        c2 = give_filtered(st, seat, |c| c.suit == Suit::Spade);
        while st.seats[seat].hand.len() > 2 {
            st.seats[seat].hand.pop();
        }
    });
    mgr.execute_action(&room, &pid, &format!("skill:jiuchi:{c1}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == format!("skill:jiuchi:{c2}")),
        "界酒池：使用【酒】无次数限制"
    );
    mgr.execute_action(&room, &pid, &format!("skill:jiuchi:{c2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .filter(|e| e.text.contains("酒池："))
            .count()
            >= 2
    );
}

#[test]
fn jiuchi_jie_disables_bengkuai_after_drunk_sha_damage() {
    let mgr = manager(3404);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut victim, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_470");
        st.seats[seat].hp = 5;
        victim = other_seats(st, seat)[0];
        set_general(st, victim, "sgs_hero_1");
        st.seats[victim].hp = 1;
        st.set_counter(&format!("jiu_boost:{victim}"), 1);
        let sha = find_free_card(st, |c| c.def == "sha");
        damage_and_advance(
            st,
            victim,
            1,
            Some(seat),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
        hp = st.seats[seat].hp;
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("bengkuai_off"),
        1,
        "酒池：酒【杀】伤害后崩坏失效"
    );
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp, "崩坏失效：结束阶段不失去体力");
}

#[test]
fn baonue_jie_spade_heals_and_gains_judge_card() {
    let mgr = manager(3405);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut hp, mut judge) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st.turn;
        set_general(st, lord, "sgs_hero_470");
        st.seats[lord].identity = Identity::Lord;
        st.seats[lord].hp = st.seats[lord].max_hp - 1;
        hp = st.seats[lord].hp;
        judge = put_on_top(st, |c| c.suit == Suit::Spade);
        crate::resolve::push(
            st,
            crate::resolve::Frame::QuickJudge(crate::resolve::QuickJudgeFrame {
                owner: lord,
                reason: "baonue".into(),
                stage: crate::resolve::QuickJudgeStage::Reveal,
                judge_card: None,
                rewrite_order: Vec::new(),
                rewrite_cursor: 0,
                data: serde_json::json!({ "lord": lord }),
                saved_outcome: None,
            }),
        );
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hp, hp + 1, "界暴虐：黑桃回复 1 点体力");
    assert!(
        st.seats[lord].hand.iter().any(|c| c == &judge),
        "界暴虐：获得此判定牌"
    );
}

// ---------------------------------------------------------------------------
// 界邓艾 · 屯田 / 凿险
// ---------------------------------------------------------------------------

#[test]
fn tuotian_jie_judges_when_discarding_sha_in_own_turn() {
    let mgr = manager(3406);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut tian) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_471");
        let sha = give_filtered(st, seat, |c| c.def == "sha");
        put_on_top(st, |c| c.suit != Suit::Heart && c.def != "sha");
        crate::turn::take_card(st, seat, &sha).expect("取出【杀】");
        crate::turn::discard_by(st, Some(seat), &sha);
        advance_once(st);
        tian = st.seats[seat]
            .marks
            .get("tian")
            .map(|v| v.len())
            .unwrap_or(0);
    });
    assert_eq!(tian, 1, "界屯田：回合内弃置【杀】后也判定");
}

#[test]
fn zaoxian_jie_grants_extra_turn_after_this_turn() {
    let mgr = manager(3407);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut turn_index) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_471");
        // 从牌堆顶取 3 张非红桃作为「田」（不动其他区域，保证牌数守恒）。
        let mut tian = Vec::new();
        for _ in 0..3 {
            let id = loop {
                let Some(id) = st.deck.draw.pop() else {
                    panic!("牌堆不足");
                };
                if st.cards.get(&id).is_some_and(|c| c.suit != Suit::Heart) {
                    break id;
                }
                st.deck.draw.insert(0, id);
            };
            tian.push(id);
        }
        st.seats[seat].marks.insert("tian".into(), tian);
        turn_index = st.turn_index;
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, seat, "jixi"), "凿险：觉醒获得「急袭」");
    assert!(st.seats[seat].flags.contains_key("zaoxian_extra"));
    // 结束阶段收尾后应进入「自己的额外回合」。
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn, seat, "界凿险：本回合结束后执行额外回合");
    assert!(st.turn_index > turn_index);
}

#[test]
fn tuotian_jie_unchanged_outside_own_turn() {
    let mgr = manager(3408);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = other_seats(st, st.turn)[0];
        set_general(st, seat, "sgs_hero_471");
        // 回合外失去手牌：沿用标准屯田（此处直接调用钩子验证可用）。
        crate::skills::tuotian_on_lose(st, seat);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat]
            .marks
            .get("tian")
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        "屯田：回合外失去牌仍可判定"
    );
}

// ---------------------------------------------------------------------------
// 界祝融 · 巨象 / 烈刃 / 长标
// ---------------------------------------------------------------------------

#[test]
fn changbiao_jie_any_hand_cards_as_sha_and_draws() {
    let mgr = manager(3409);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut cards) =
        (0, String::new(), 0, String::new(), Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_479");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        cards.push(give_filtered(st, seat, |c| c.def == "sha"));
        cards.push(give_filtered(st, seat, |c| c.def == "tao"));
    });
    mgr.execute_action(&room, &pid, "skill:changbiao").unwrap();
    for c in &cards {
        mgr.execute_action(&room, &pid, &format!("choose:{c}"))
            .unwrap();
    }
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        1,
        "长标：造成 1 点伤害，阶段结束摸 1 张"
    );
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("长标：本阶段结束时摸")),
        "长标：阶段结束摸牌"
    );
}

#[test]
fn changbiao_jie_counts_toward_sha_limit() {
    let mgr = manager(3410);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut card) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_479");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        card = give_filtered(st, seat, |c| c.def == "sha");
        give_def(st, other_seats(st, seat)[0], "sha");
    });
    mgr.execute_action(&room, &pid, "skill:changbiao").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    let target_pid = st
        .pending
        .as_ref()
        .map(|p| p.options[0].id.clone())
        .expect("杀目标");
    mgr.execute_action(&room, &pid, &target_pid).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("sha_used"), 1, "长标【杀】计入使用次数");
}

#[test]
fn changbiao_jie_no_damage_no_draw() {
    let mgr = manager(3411);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut card) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_479");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        card = give_filtered(st, seat, |c| c.def == "sha");
        give_def(st, target, "shan");
    });
    mgr.execute_action(&room, &pid, "skill:changbiao").unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[target].hand[0].clone();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("changbiao_draw"),
        0,
        "长标：未造成伤害不累计摸牌"
    );
}

// ---------------------------------------------------------------------------
// 界姜维 · 挑衅 / 志继
// ---------------------------------------------------------------------------

#[test]
fn tiaoxin_jie_limits_two_per_play_phase() {
    let mgr = manager(3412);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_480");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) == 1)
            .expect("相邻目标");
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "tao");
        give_def(st, target, "sha");
        give_filtered(st, seat, |c| c.def == "shan");
    });
    mgr.execute_action(&room, &pid, "skill:tiaoxin").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    // 目标不出【杀】→ 姜维选择弃置其一张牌。
    mgr.execute_action(&room, &pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("tiaoxin_used"), 1);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:tiaoxin"),
        "界挑衅：本阶段可发动两次"
    );
}

#[test]
fn tiaoxin_jie_discards_when_sha_misses() {
    let mgr = manager(3413);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_480");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) == 1)
            .expect("相邻目标");
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "sha");
        give_def(st, target, "tao");
        give_def(st, target, "sha");
        give_filtered(st, seat, |c| c.def == "shan");
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:tiaoxin").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:tiaoxin:sha")
        .unwrap();
    let st = state_of(&mgr, &room);
    let shan = st.seats[seat].hand[0].clone();
    mgr.execute_action(&room, &pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("未用【杀】造成伤害")),
        "界挑衅：杀未造成伤害仍弃其一张牌"
    );
    assert!(st.seats[target].hand.len() < before);
}

#[test]
fn tiaoxin_jie_no_discard_when_sha_hits() {
    let mgr = manager(3414);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_480");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) == 1)
            .expect("相邻目标");
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "sha");
        give_def(st, target, "tao");
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:tiaoxin").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:tiaoxin:sha")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("不弃其牌")),
        "界挑衅：杀造成伤害则不弃其牌"
    );
    assert_eq!(
        st.seats[target].hand.len(),
        before - 1,
        "只消耗了那张【杀】"
    );
}

// ---------------------------------------------------------------------------
// 界贾诩 · 帷幕 / 乱武 / 完杀
// ---------------------------------------------------------------------------

#[test]
fn weimu_jie_prevents_damage_in_own_turn_and_draws() {
    let mgr = manager(3415);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut hp, mut before) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_484");
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        hp = st.seats[seat].hp;
        before = st.seats[seat].hand.len();
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
    assert_eq!(st.seats[seat].hp, hp, "界帷幕：回合内防止伤害");
    assert_eq!(
        st.seats[seat].hand.len(),
        before + 2,
        "界帷幕：摸防止值两倍"
    );
}

#[test]
fn weimu_jie_no_prevent_outside_turn() {
    let mgr = manager(3416);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut turn_seat, mut hp) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        turn_seat = st.turn;
        set_general(st, turn_seat, "sgs_hero_1");
        seat = other_seats(st, turn_seat)[0];
        set_general(st, seat, "sgs_hero_484");
        hp = st.seats[seat].hp;
        damage_and_advance(
            st,
            seat,
            1,
            Some(turn_seat),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 1, "回合外不防止伤害");
}

#[test]
fn luanwu_jie_final_sha_after_all_resolve() {
    let mgr = manager(3417);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_484");
        stage_play(st, seat);
        pid = pid_of(st, seat);
    });
    mgr.execute_action(&room, &pid, "skill:luanwu").unwrap();
    for _ in 0..8 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.skill.as_deref() == Some("luanwu") {
            mgr.execute_action(&room, &p.actor, "choose:luanwu:lose")
                .unwrap();
        } else {
            break;
        }
    }
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(pid.as_str()),
        "界乱武：结算完毕后贾诩视为使用【杀】"
    );
    assert!(
        !pending_options_with(&st, "target:").is_empty(),
        "界乱武：无距离限制【杀】选择目标"
    );
}

// ---------------------------------------------------------------------------
// 界鲁肃 · 好施 / 缔盟
// ---------------------------------------------------------------------------

#[test]
fn haoshi_jie_back_card_when_targeted_by_sha() {
    let mgr = manager(3418);
    let room = start_and_pick(&mgr, 5);
    let (mut lusu, mut lusu_pid, mut attacker, mut attacker_pid, mut helper, mut before) =
        (0, String::new(), 0, String::new(), 0, 0);
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        stage_play(st, attacker);
        attacker_pid = pid_of(st, attacker);
        lusu = other_seats(st, attacker)
            .into_iter()
            .find(|t| st.in_attack_range(attacker, *t))
            .expect("攻击范围内目标");
        set_general(st, lusu, "sgs_hero_485");
        lusu_pid = pid_of(st, lusu);
        helper = other_seats(st, attacker)
            .into_iter()
            .find(|t| *t != lusu)
            .expect("回礼者");
        set_general(st, helper, "sgs_hero_1");
        give_def(st, helper, "sha");
        crate::resolve_jiexian4::mark_haoshi_back(st, lusu, helper);
        before = st.seats[lusu].hand.len();
        sha = give_def(st, attacker, "sha");
    });
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{lusu_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[lusu].hand.len(),
        before + 1,
        "界好施：回礼者交给其一张手牌"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("好施：交给")),
        "界好施：回礼记录"
    );
}

#[test]
fn haoshi_jie_back_card_when_targeted_by_trick() {
    let mgr = manager(3419);
    let room = start_and_pick(&mgr, 5);
    let (mut lusu, mut lusu_pid, mut user, mut user_pid, mut helper, mut before, mut trick) =
        (0, String::new(), 0, String::new(), 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = pid_of(st, user);
        lusu = other_seats(st, user)[0];
        set_general(st, lusu, "sgs_hero_485");
        lusu_pid = pid_of(st, lusu);
        helper = other_seats(st, user)[1];
        set_general(st, helper, "sgs_hero_1");
        give_def(st, helper, "sha");
        crate::resolve_jiexian4::mark_haoshi_back(st, lusu, helper);
        before = st.seats[lusu].hand.len();
        trick = give_def(st, user, "juedou");
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{trick}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{lusu_pid}"))
        .unwrap();
    for _ in 0..6 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "wuxie" {
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[lusu].hand.len(),
        before + 1,
        "界好施：普通锦囊目标也回礼"
    );
}

#[test]
fn dimeng_jie_swaps_hands() {
    let mgr = manager(3420);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut a, mut a_pid, mut b, mut b_pid) =
        (0, String::new(), 0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_485");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        a = others[0];
        b = others[1];
        set_general(st, a, "sgs_hero_1");
        set_general(st, b, "sgs_hero_1");
        a_pid = pid_of(st, a);
        b_pid = pid_of(st, b);
        give_def(st, a, "sha");
        give_def(st, b, "tao");
        give_def(st, seat, "sha");
        give_def(st, seat, "tao");
    });
    mgr.execute_action(&room, &pid, "skill:dimeng").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{a_pid}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{b_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let sha_in_a = st.seats[a]
        .hand
        .iter()
        .any(|c| st.cards.get(c).is_some_and(|x| x.def == "sha"));
    assert!(!sha_in_a, "缔盟：两名角色互换手牌");
}

// ---------------------------------------------------------------------------
// 界张郃 · 巧变
// ---------------------------------------------------------------------------

#[test]
fn qiaobian_jie_starts_with_two_bian_marks() {
    let mgr = manager(3421);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_486");
        crate::resolve_jiexian4::init_qiaobian(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::resolve_jiexian4::bian_marks(&st, seat),
        2,
        "界巧变：开局 2 枚「变」"
    );
}

#[test]
fn qiaobian_jie_pay_mark_when_no_hand() {
    let mgr = manager(3422);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_486");
        crate::resolve_jiexian4::init_qiaobian(st);
        before = crate::resolve_jiexian4::bian_marks(st, seat);
        pid = pid_of(st, seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("qiaobian"));
    mgr.execute_action(&room, &pid, "choose:qiaobian:judge")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::resolve_jiexian4::bian_marks(&st, seat),
        before - 1,
        "界巧变：无手牌时用「变」支付"
    );
    assert_eq!(st.counter("qiaobian_skip_judge"), 1);
}

#[test]
fn qiaobian_jie_end_phase_gains_mark_when_new_hand_count() {
    let mgr = manager(3423);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_486");
        crate::resolve_jiexian4::init_qiaobian(st);
        before = crate::resolve_jiexian4::bian_marks(st, seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::resolve_jiexian4::bian_marks(&st, seat),
        before + 1,
        "界巧变：结束时手牌数为新值 → 获得 1 枚「变」"
    );
}
