//! P4.4.2 界限突破批次 J1（8 将）：界黄忠 / 界徐庶 / 界华雄 / 界左慈 / 界张角 / 界于吉 / 界袁绍 / 界刘禅。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of};
use crate::skills;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// 界黄忠 · 烈弓
// ---------------------------------------------------------------------------

#[test]
fn liegong_jie_range_uses_card_point() {
    let mgr = manager(3200);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut far, mut far_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_26");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        far = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) >= 2)
            .expect("存在远处目标");
        set_general(st, far, "sgs_hero_1");
        far_pid = pid_of(st, far);
        sha = give_filtered(st, seat, |c| c.def == "sha" && c.rank >= 10);
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        pending_options_with(&st, "target:").contains(&format!("target:{far_pid}")),
        "烈弓①：距离 ≤ 此【杀】点数应允许指定远处目标"
    );
}

#[test]
fn liegong_jie_damage_bonus_when_hp_not_greater() {
    let mgr = manager(3201);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_26");
        st.seats[seat].hp = 2;
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hp = st.seats[target].max_hp;
        give_filtered(st, seat, |c| c.def == "sha");
    });
    let st = state_of(&mgr, &room);
    let target_pid = pid_of(&st, target);
    let hp = st.seats[target].hp;
    let sha = st.seats[seat].hand[0].clone();
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp - 2,
        "烈弓②：体力值不大于目标时【杀】伤害 +1"
    );
}

#[test]
fn liegong_jie_blocks_shan_when_hand_not_less() {
    let mgr = manager(3202);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut shan, mut sha) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    let mut hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_26");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        shan = give_def(st, target, "shan");
        sha = give_filtered(st, seat, |c| c.def == "sha");
        give_def(st, seat, "sha");
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let _ = target_pid;
    assert_eq!(
        st.seats[target].hp,
        hp - 1,
        "烈弓②：手牌数不小于其 → 其不能使用【闪】（持有【闪】仍受伤）"
    );
    assert!(
        st.seats[target].hand.iter().any(|c| c == &shan),
        "目标仍持有【闪】但无法使用"
    );
}

// ---------------------------------------------------------------------------
// 界徐庶 · 诛害 / 潜心 / 荐言
// ---------------------------------------------------------------------------

#[test]
fn zhuhai_triggers_at_end_phase_after_damage() {
    let mgr = manager(3203);
    let room = start_and_pick(&mgr, 5);
    let (mut turn_seat, mut xushu, mut xushu_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        turn_seat = st.turn;
        set_general(st, turn_seat, "sgs_hero_1");
        xushu = other_seats(st, turn_seat)
            .into_iter()
            .find(|t| st.seat_distance(turn_seat, *t) == 1)
            .expect("相邻座位");
        set_general(st, xushu, "sgs_hero_304");
        xushu_pid = pid_of(st, xushu);
        // 本回合造成过伤害。
        st.set_counter(&format!("dealt:{turn_seat}"), 1);
    });
    run_phase(&mgr, &room, turn_seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zhuhai"),
        "诛害应在他人结束阶段触发"
    );
    mgr.execute_action(&room, &xushu_pid, "choose:zhuhai")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("诛害：视为对")),
        "诛害：视为对其使用【杀】"
    );
}

#[test]
fn zhuhai_not_triggered_without_damage() {
    let mgr = manager(3204);
    let room = start_and_pick(&mgr, 5);
    let mut turn_seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        turn_seat = st.turn;
        set_general(st, turn_seat, "sgs_hero_1");
        let xushu = other_seats(st, turn_seat)
            .into_iter()
            .find(|t| st.seat_distance(turn_seat, *t) == 1)
            .expect("相邻座位");
        set_general(st, xushu, "sgs_hero_304");
        st.turn_counters.clear();
    });
    run_phase(&mgr, &room, turn_seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_ne!(pending_skill(&st), Some("zhuhai"), "未造成伤害不应触发诛害");
}

#[test]
fn qianxin_awakens_and_grants_jianyan() {
    let mgr = manager(3205);
    let room = start_and_pick(&mgr, 5);
    let (mut xushu, mut victim, mut max_before) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        xushu = st.turn;
        set_general(st, xushu, "sgs_hero_304");
        st.seats[xushu].hp = st.seats[xushu].max_hp - 1;
        max_before = st.seats[xushu].max_hp;
        victim = other_seats(st, xushu)[0];
        set_general(st, victim, "sgs_hero_1");
        damage_and_advance(
            st,
            victim,
            1,
            Some(xushu),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert!(skills::has(&st, xushu, "jianyan"), "潜心觉醒应获得「荐言」");
    assert_eq!(
        st.seats[xushu].max_hp,
        max_before - 1,
        "潜心：减少 1 点体力上限"
    );
}

#[test]
fn jianyan_once_per_play_phase_grants_male_target() {
    let mgr = manager(3206);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_304");
        skills::grant_skill(st, seat, "jianyan");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        before = st.seats[target].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:jianyan").unwrap();
    let st = state_of(&mgr, &room);
    let modes: Vec<String> = pending_options_with(&st, "mode:");
    assert!(!modes.is_empty(), "荐言应提供类别/颜色选项");
    let mode = modes
        .iter()
        .find(|m| m.as_str() == "mode:basic")
        .cloned()
        .unwrap_or_else(|| modes[0].clone());
    mgr.execute_action(&room, &pid, &mode).unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hand.len(),
        before + 1,
        "荐言：男性角色获得亮出的牌"
    );
    assert!(
        mgr.execute_action(&room, &pid, "skill:jianyan").is_err(),
        "荐言出牌阶段限一次"
    );
}

// ---------------------------------------------------------------------------
// 界华雄 · 耀武 / 势斩
// ---------------------------------------------------------------------------

#[test]
fn yaowu_red_damage_card_lets_source_draw() {
    let mgr = manager(3207);
    let room = start_and_pick(&mgr, 5);
    let (mut huaxiong, mut source, mut before, mut red) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        huaxiong = st.turn;
        set_general(st, huaxiong, "sgs_hero_446");
        source = other_seats(st, huaxiong)[0];
        set_general(st, source, "sgs_hero_1");
        red = find_cards(st, |c| skills::is_red(c.suit), 1)[0].clone();
        before = st.seats[source].hand.len();
        damage_and_advance(
            st,
            huaxiong,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&red),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[source].hand.len(),
        before + 1,
        "耀武：红色伤害牌 → 伤害来源摸 1"
    );
}

#[test]
fn yaowu_non_red_damage_lets_huaxiong_draw() {
    let mgr = manager(3208);
    let room = start_and_pick(&mgr, 5);
    let (mut huaxiong, mut source, mut before, mut black) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        huaxiong = st.turn;
        set_general(st, huaxiong, "sgs_hero_446");
        source = other_seats(st, huaxiong)[0];
        set_general(st, source, "sgs_hero_1");
        black = find_cards(st, |c| skills::is_black(c.suit), 1)[0].clone();
        before = st.seats[huaxiong].hand.len();
        damage_and_advance(
            st,
            huaxiong,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&black),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[huaxiong].hand.len(),
        before + 1,
        "耀武：非红色伤害牌 → 华雄摸 1"
    );
}

#[test]
fn shizhan_duel_target_responds_first_and_limited_twice() {
    let mgr = manager(3209);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut other, mut other_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_446");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        other_pid = pid_of(st, other);
        give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:shizhan").unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{other_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.clone()),
        Some(pid.clone()),
        "势斩：【决斗】由华雄先出【杀】"
    );
    assert!(st.history.iter().any(|e| e.text.contains("势斩")));
    // 限两次：第三次发动应被拒绝。
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("shizhan_used"), 1);
    edit(&mgr, &room, |st| st.set_counter("shizhan_used", 2));
    assert!(
        mgr.execute_action(&room, &pid, "skill:shizhan").is_err(),
        "势斩出牌阶段限两次"
    );
}

// ---------------------------------------------------------------------------
// 界左慈 · 化身 / 新生
// ---------------------------------------------------------------------------

#[test]
fn huashen_jie_starts_with_three_cards() {
    let mgr = manager(3210);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_447");
        st.seats[seat].huashen.clear();
        st.seats[seat].huashen_active = None;
        crate::resolve_mountain::init_huashen(st, seat);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].huashen.len(),
        3,
        "界化身：开局获得三张化身牌"
    );
    assert!(st.seats[seat].huashen_active.is_some());
}

#[test]
fn huashen_jie_exchange_swaps_cards() {
    let mgr = manager(3211);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    let mut pid = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_447");
        st.seats[seat].huashen.clear();
        st.seats[seat].huashen_active = None;
        crate::resolve_mountain::init_huashen(st, seat);
        pid = pid_of(st, seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("huashen"));
    assert!(
        pending_options_with(&st, "choose:").contains(&"choose:exchange".to_string()),
        "界化身：提供换牌选项"
    );
    mgr.execute_action(&room, &pid, "choose:exchange").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].huashen.len(), 3, "换牌后化身牌数不变");
    assert!(
        st.history.iter().any(|e| e.text.contains("化身：移去")),
        "界化身：移去至多两张并获得等量新化身牌"
    );
}

#[test]
fn huashen_jie_kingdom_follows_active_card() {
    let mgr = manager(3212);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_447");
        // 展示的化身牌为吴势力（界大乔）。
        st.seats[seat].huashen = vec!["sgs_hero_309".to_string()];
        st.seats[seat].huashen_active = Some("sgs_hero_309".to_string());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        skills::kingdom_of(&st, seat),
        "wu",
        "界化身：势力视为与化身牌相同"
    );
    assert!(!skills::is_male(&st, seat), "界大乔为女性 → 性别视为女性");
}

#[test]
fn xinsheng_gains_extra_huashen_card() {
    let mgr = manager(3213);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_447");
        st.seats[seat].huashen.clear();
        st.seats[seat].huashen_active = None;
        crate::resolve_mountain::init_huashen(st, seat);
        before = st.seats[seat].huashen.len();
        damage_and_advance(st, seat, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].huashen.len(),
        before + 1,
        "新生：受到 1 点伤害后获得一张新的化身牌"
    );
}

// ---------------------------------------------------------------------------
// 界张角 · 雷击 / 鬼道 / 黄天
// ---------------------------------------------------------------------------

#[test]
fn leiji_jie_spade_deals_two_thunder_damage() {
    let mgr = manager(3214);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut zj, mut attacker_pid, mut zj_pid, mut shan_id) =
        (0, 0, String::new(), String::new(), String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        stage_play(st, attacker);
        attacker_pid = pid_of(st, attacker);
        zj = other_seats(st, attacker)
            .into_iter()
            .find(|t| st.seat_distance(attacker, *t) == 1)
            .expect("相邻座位");
        set_general(st, zj, "sgs_hero_448");
        zj_pid = pid_of(st, zj);
        sha = give_def(st, attacker, "sha");
        shan_id = give_def(st, zj, "shan");
        put_on_top(st, |c| c.suit == Suit::Spade && c.rank == 10);
    });
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{zj_pid}"))
        .unwrap();
    mgr.execute_action(&room, &zj_pid, &format!("respond:{shan_id}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("leiji_jie"),
        "界雷击：判定后选择目标"
    );
    let hp = st.seats[attacker].hp;
    mgr.execute_action(&room, &zj_pid, &format!("target:{attacker_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[attacker].hp, hp - 2, "黑桃：2 点雷电伤害");
}

#[test]
fn leiji_jie_club_recovers_then_one_damage() {
    let mgr = manager(3215);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut zj, mut attacker_pid, mut zj_pid, mut shan_id) =
        (0, 0, String::new(), String::new(), String::new());
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        stage_play(st, attacker);
        attacker_pid = pid_of(st, attacker);
        zj = other_seats(st, attacker)
            .into_iter()
            .find(|t| st.seat_distance(attacker, *t) == 1)
            .expect("相邻座位");
        set_general(st, zj, "sgs_hero_448");
        st.seats[zj].hp = 2;
        zj_pid = pid_of(st, zj);
        sha = give_def(st, attacker, "sha");
        shan_id = give_def(st, zj, "shan");
        put_on_top(st, |c| c.suit == Suit::Club && c.rank == 7);
    });
    mgr.execute_action(&room, &attacker_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &attacker_pid, &format!("target:{zj_pid}"))
        .unwrap();
    mgr.execute_action(&room, &zj_pid, &format!("respond:{shan_id}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("leiji_jie"));
    assert_eq!(st.seats[zj].hp, 3, "梅花：先回复 1 点体力");
    let hp = st.seats[attacker].hp;
    mgr.execute_action(&room, &zj_pid, &format!("target:{attacker_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[attacker].hp, hp - 1, "梅花：可造成 1 点雷电伤害");
}

#[test]
fn guidao_jie_spade_two_to_nine_draws() {
    let mgr = manager(3216);
    let room = start_and_pick(&mgr, 5);
    let (mut zj, mut zj_pid, mut rewrite) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zj = st.turn;
        set_general(st, zj, "sgs_hero_448");
        zj_pid = pid_of(st, zj);
        put_in_judge_zone(st, zj, "lebu_sishu");
        rewrite = give_filtered(st, zj, |c| {
            c.suit == Suit::Spade && (2..=9).contains(&c.rank)
        });
        put_on_top(st, |c| c.suit == Suit::Heart && c.def != "lebu_sishu");
    });
    run_phase(&mgr, &room, zj, TurnPhase::Judge, true);
    pass_until_skill(&mgr, &room, "guidao", 8);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("guidao"),
        "鬼道：判定牌生效前可改判"
    );
    mgr.execute_action(&room, &zj_pid, &format!("respond:{rewrite}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("鬼道：黑桃 2~9，摸 1 张")),
        "界鬼道：黑桃 2~9 改判后摸 1"
    );
}

// ---------------------------------------------------------------------------
// 界于吉 · 蛊惑 / 缠怨
// ---------------------------------------------------------------------------

#[test]
fn guhuo_jie_false_card_lets_questioner_draw() {
    let mgr = manager(3217);
    let room = start_and_pick(&mgr, 5);
    let (mut yj, mut yj_pid, mut tao) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yj = st.turn;
        set_general(st, yj, "sgs_hero_449");
        stage_play(st, yj);
        yj_pid = pid_of(st, yj);
        tao = give_def(st, yj, "tao");
    });
    mgr.execute_action(&room, &yj_pid, "skill:guhuo").unwrap();
    mgr.execute_action(&room, &yj_pid, &format!("choose:{tao}"))
        .unwrap();
    mgr.execute_action(&room, &yj_pid, "confirm").unwrap();
    mgr.execute_action(&room, &yj_pid, "declare:sha").unwrap();
    let st = state_of(&mgr, &room);
    let q_pid = st.pending.as_ref().expect("质疑窗口").actor.clone();
    let q = st.seat_index(&q_pid).expect("质疑者");
    let before = st.seats[q].hand.len();
    mgr.execute_action(&room, &q_pid, "choose:question")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[q].hand.len(),
        before + 1,
        "界蛊惑：牌为假 → 质疑者摸一张牌"
    );
    assert!(st.history.iter().any(|e| e.text.contains("此牌作废")));
}

#[test]
fn guhuo_jie_true_card_punishes_questioner() {
    let mgr = manager(3218);
    let room = start_and_pick(&mgr, 5);
    let (mut yj, mut yj_pid, mut sha) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yj = st.turn;
        set_general(st, yj, "sgs_hero_449");
        stage_play(st, yj);
        yj_pid = pid_of(st, yj);
        sha = give_def(st, yj, "sha");
        // 给其他角色各留一张手牌，保证「弃一张牌」分支可用。
        let others = other_seats(st, yj);
        for s in others {
            give_def(st, s, "sha");
        }
    });
    mgr.execute_action(&room, &yj_pid, "skill:guhuo").unwrap();
    mgr.execute_action(&room, &yj_pid, &format!("choose:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &yj_pid, "confirm").unwrap();
    mgr.execute_action(&room, &yj_pid, "declare:sha").unwrap();
    let st = state_of(&mgr, &room);
    let q_pid = st.pending.as_ref().expect("质疑窗口").actor.clone();
    let q_seat = st.seat_index(&q_pid).expect("质疑者");
    let before = st.seats[q_seat].hand.len();
    mgr.execute_action(&room, &q_pid, "choose:question")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[q_seat].flags.contains_key("chanyuan"),
        "界蛊惑：牌为真 → 质疑者获得「缠怨」"
    );
    assert!(st.seats[q_seat].flags.contains_key("chanyuan_jie"));
    assert!(
        st.seats[q_seat].hand.len() < before,
        "界蛊惑：为真时质疑者弃置一张牌"
    );
}

#[test]
fn chanyuan_jie_disables_skills_at_one_hp() {
    let mgr = manager(3219);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut normal) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_449");
        st.seats[seat].hp = 1;
        assert!(skills::has(st, seat, "guhuo"));
        st.seats[seat].flags.insert("chanyuan".into(), true);
        st.seats[seat].flags.insert("chanyuan_jie".into(), true);
        // 标准缠怨不做技能失效：对照座位（界张角 + 仅标准缠怨）。
        normal = other_seats(st, seat)[0];
        set_general(st, normal, "sgs_hero_448");
        st.seats[normal].hp = 1;
        st.seats[normal].flags.insert("chanyuan".into(), true);
    });
    let st = state_of(&mgr, &room);
    assert!(
        !skills::has(&st, seat, "guhuo"),
        "界缠怨：体力值为 1 时其他技能失效"
    );
    assert!(
        skills::has(&st, normal, "leiji"),
        "标准缠怨不使技能失效（回归）"
    );
}

#[test]
fn xueyi_jie_marks_add_hand_limit() {
    let mgr = manager(3220);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_450");
        st.seats[seat].identity = Identity::Lord;
        crate::skills::remove_xueyi_mark(st, seat);
        before = skills::hand_limit(st, seat);
        crate::skills::add_xueyi_marks(st, seat, 2);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        skills::hand_limit(&st, seat),
        before + 2,
        "血裔：每枚「裔」手牌上限 +1"
    );
}

#[test]
fn xueyi_jie_play_phase_draws_by_removing_mark() {
    let mgr = manager(3221);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_450");
        st.seats[seat].identity = Identity::Lord;
        crate::skills::add_xueyi_marks(st, seat, 2);
        pid = pid_of(st, seat);
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Play, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("xueyi"),
        "血裔：出牌阶段开始时可移标记摸牌"
    );
    mgr.execute_action(&room, &pid, "choose:xueyi").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(skills::xueyi_marks(&st, seat), 1);
    assert_eq!(st.seats[seat].hand.len(), before + 1, "血裔：摸一张牌");
}

#[test]
fn luanji_jie_can_drop_one_target() {
    let mgr = manager(3222);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut c1, mut c2, mut drop) =
        (0, String::new(), String::new(), String::new(), 0);
    let mut drop_hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_450");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        c1 = give_suit(st, seat, Suit::Heart);
        c2 = give_suit(st, seat, Suit::Heart);
        drop = other_seats(st, seat)[0];
        set_general(st, drop, "sgs_hero_1");
        drop_hp = st.seats[drop].hp;
    });
    let st = state_of(&mgr, &room);
    let drop_pid = pid_of(&st, drop);
    mgr.execute_action(&room, &pid, &format!("skill:luanji:{c1},{c2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("luanji_pick"));
    mgr.execute_action(&room, &pid, &format!("drop:{drop_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("乱击：不指定")),
        "界乱击：可少选一个目标"
    );
    assert_eq!(
        st.seats[drop].hp, drop_hp,
        "界乱击：被少选的目标不结算【万箭齐发】"
    );
}

// ---------------------------------------------------------------------------
// 界刘禅 · 享乐 / 放权 / 若愚 / 思蜀
// ---------------------------------------------------------------------------

#[test]
fn ruoyu_jie_recovers_to_three_and_grants_sishu() {
    let mgr = manager(3223);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_451");
        st.seats[seat].identity = Identity::Lord;
        st.seats[seat].hp = 1;
        st.set_counter("awaken_ruoyu", 0);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, true);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, 3, "界若愚：回复至 3 点体力");
    assert_eq!(
        st.seats[seat].max_hp, 5,
        "界若愚：体力上限 +1（主公 4 + 1）"
    );
    assert!(skills::has(&st, seat, "sishu"), "界若愚：获得「思蜀」");
    assert!(skills::has(&st, seat, "jijiang"), "界若愚：获得「激将」");
}

#[test]
fn sishu_reverses_lebu_sishu_judgement() {
    let mgr = manager(3224);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_451");
        put_in_judge_zone(st, seat, "lebu_sishu");
        st.seats[seat].flags.insert("sishu_reverse".into(), true);
        put_on_top(st, |c| c.suit == Suit::Heart && c.def != "lebu_sishu");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Judge, true);
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("思蜀：红桃反转")),
        "思蜀：判定效果反转"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("跳过出牌阶段")),
        "思蜀：红桃反转为跳过出牌阶段"
    );
}

#[test]
fn fangquan_jie_resolves_at_discard_phase() {
    let mgr = manager(3225);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_451");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, "skill:fangquan").unwrap();
    run_phase(&mgr, &room, seat, TurnPhase::Discard, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("fangquan"),
        "界放权：弃牌阶段开始时结算"
    );
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn, target, "界放权：目标执行额外回合");
}
