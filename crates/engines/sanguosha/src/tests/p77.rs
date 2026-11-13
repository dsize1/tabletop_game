//! P4.4.1 SP-1 用例（二）：陆绩 / 孙亮 / 蒯越蒯良 / 许攸。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, give, manager, state_of};
use crate::skills;
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

// ---------------------------------------------------------------------------
// 陆绩 · 怀橘 / 遗礼 / 整论
// ---------------------------------------------------------------------------

#[test]
fn huaiju_prevents_damage_and_consumes_mark() {
    let mgr = manager(980);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut victim) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = other_seats(st, cur)[0];
        set_general(st, victim, "sgs_hero_402");
        skills::add_ju(st, victim, 3);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        let hp = st.seats[victim].hp;
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(cur),
            crate::resolve::DamageType::Normal,
            None,
        );
        assert_eq!(st.seats[victim].hp, hp, "怀橘：防止伤害");
        assert_eq!(skills::ju_count(st, victim), 2, "怀橘：移除 1 枚「橘」");
        assert!(st.history.iter().any(|e| e.text.contains("怀橘")));
    });
    let _ = victim;
}

#[test]
fn huaiju_draw_phase_extra_card() {
    let mgr = manager(981);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_402");
        skills::add_ju(st, cur, 1);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "有「橘」摸牌阶段多摸 1 张");
}

#[test]
fn yili_gives_ju_to_other() {
    let mgr = manager(982);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_402");
        skills::add_ju(st, cur, 3);
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}=choose:yili:ju"),
            format!("{cur_pid}=target:{target_pid}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(skills::ju_count(&st, cur), 2, "遗礼：移除 1 枚「橘」");
    assert_eq!(skills::ju_count(&st, target), 1, "遗礼：令其他角色获得 1 枚「橘」");
}

#[test]
fn zhenglun_skips_draw_and_gains_ju() {
    let mgr = manager(983);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut cur_pid = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_402");
        st.turn_phase = TurnPhase::Draw;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=choose:zhenglun")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 0, "整论：跳过摸牌");
    assert_eq!(skills::ju_count(&st, cur), 1, "整论：获得 1 枚「橘」");
}

// ---------------------------------------------------------------------------
// 孙亮 · 溃诛 / 掣政 / 立军
// ---------------------------------------------------------------------------

#[test]
fn kuizhu_draw_option_makes_targets_draw() {
    let mgr = manager(984);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut pids) = (0, String::new(), Vec::new());
    let mut targets = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_403");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        for _ in 0..6 {
            give(st, cur, "sha");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        cur_pid = st.seats[cur].player_id.clone();
        pids = st
            .alive_indices()
            .into_iter()
            .map(|i| st.seats[i].player_id.clone())
            .collect();
        // 先自动弃牌（孙亮 max_hp=3 → 弃 3 张），再选溃诛①令 3 名角色各摸 1。
        let mut inject = vec![format!("{cur_pid}=choose:kuizhu:draw")];
        for p in pids.iter().take(3) {
            inject.push(format!("{cur_pid}=target:{p}"));
        }
        drive_phases(st, &mut inject);
        targets = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != cur)
            .collect();
    });
    let st = state_of(&mgr, &room);
    let total: usize = targets.iter().map(|i| st.seats[*i].hand.len()).sum();
    assert_eq!(total, 3, "溃诛①：令至多 X 名角色各摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("溃诛")));
}

#[test]
fn kuizhu_damage_option_combo() {
    let mgr = manager(985);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut victim, mut cur_pid, mut victim_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_403");
        victim = other_seats(st, cur)[0];
        set_general(st, victim, "sgs_hero_1");
        // 让 X=2：弃牌阶段弃 2 张。
        for _ in 0..5 {
            give(st, cur, "sha");
        }
        st.seats[victim].hp = 2;
        // 固定孙亮体力上限 3 → 弃牌阶段弃 2 张（X=2）。
        st.seats[cur].max_hp = 3;
        st.seats[cur].hp = 3;
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        cur_pid = st.seats[cur].player_id.clone();
        victim_pid = st.seats[victim].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}=choose:kuizhu:damage"),
            format!("{cur_pid}=combo:{victim_pid}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1, "溃诛②：对体力值之和为 X 的角色造成 1 伤");
}

#[test]
fn chezheng_prevents_damage_out_of_range() {
    let mgr = manager(986);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_403");
        // 找一个攻击范围不含孙亮的角色（座位距离 > 1）。
        far = other_seats(st, cur)
            .into_iter()
            .find(|i| !st.in_attack_range(*i, cur))
            .expect("应有距离 >1 的角色");
        set_general(st, far, "sgs_hero_1");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        let hp = st.seats[far].hp;
        crate::resolve::apply_damage(
            st,
            far,
            1,
            Some(cur),
            crate::resolve::DamageType::Normal,
            None,
        );
        assert_eq!(st.seats[far].hp, hp, "掣政：防止伤害");
        assert!(st.history.iter().any(|e| e.text.contains("掣政")));
    });
    let _ = far;
}

#[test]
fn chezheng_end_play_discards_from_outsider() {
    let mgr = manager(987);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_403");
        target = other_seats(st, cur)
            .into_iter()
            .find(|i| !st.in_attack_range(*i, cur))
            .expect("应有距离 >1 的角色");
        set_general(st, target, "sgs_hero_1");
        give(st, target, "sha");
        give(st, target, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("play_cards_used"), 0);
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 1, "掣政：弃置目标一张牌");
    assert!(st.history.iter().any(|e| e.text.contains("掣政")));
}

// ---------------------------------------------------------------------------
// 蒯越蒯良 · 荐降 / 审时（转换技）
// ---------------------------------------------------------------------------

#[test]
fn jianxiang_lets_min_hand_draw() {
    let mgr = manager(988);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut kuai, mut sha, mut poor) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        kuai = crate::turn::sha_targets(st, attacker)[0];
        set_general(st, kuai, "sgs_hero_404");
        // 先让其他所有人各有 2 张手牌，再让 poor 只剩 1 张 → 手牌最少者唯一。
        give(st, kuai, "shan");
        give(st, kuai, "shan");
        for seat in other_seats(st, attacker) {
            if seat == kuai {
                continue;
            }
            set_general(st, seat, "sgs_hero_1");
            give(st, seat, "shan");
            give(st, seat, "shan");
        }
        poor = other_seats(st, attacker)
            .into_iter()
            .find(|i| *i != kuai)
            .unwrap();
        // poor 只剩 1 张 → 手牌最少者唯一。
        let dropped: Vec<String> = std::mem::take(&mut st.seats[poor].hand);
        for c in dropped {
            st.deck.discard.push(c);
        }
        give(st, poor, "sha");
        give(st, attacker, "shan");
        sha = give(st, attacker, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
    });
    let st = state_of(&mgr, &room);
    let (att_pid, kuai_pid, poor_pid) = (pid(&st, attacker), pid(&st, kuai), pid(&st, poor));
    mgr.execute_action(&room, &att_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &att_pid, &format!("target:{kuai_pid}"))
        .unwrap();
    mgr.execute_action(&room, &kuai_pid, "choose:jianxiang")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[poor].hand.len(), 2, "荐降：手牌最少者摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("荐降")));
    let _ = poor_pid;
}

#[test]
fn shenshi_yang_gives_card_and_damages() {
    let mgr = manager(989);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cost, mut cur_pid, mut target_pid) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_404");
        give(st, cur, "shan");
        cost = give(st, cur, "sha");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..3 {
            give(st, target, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[cur].flags.remove("yj:shenshi");
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let hp_before = st.seats[target].hp;
    mgr.execute_action(&room, &cur_pid, "skill:shenshi").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_before - 1, "审时（阳）：造成 1 点伤害");
    assert_eq!(st.seats[target].hand.len(), 4, "审时（阳）：交给其一张牌");
    assert!(skills::yj_yin(&st, cur, "shenshi"), "审时发动后转为阴面");
}

#[test]
fn shenshi_yin_records_and_draws_to_four() {
    let mgr = manager(990);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut source, mut cost, mut cur_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_404");
        source = other_seats(st, cur)[0];
        set_general(st, source, "sgs_hero_1");
        give(st, source, "shan");
        give(st, source, "sha");
        cost = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[cur].flags.insert("yj:shenshi".into(), true);
        cur_pid = st.seats[cur].player_id.clone();
        // 源对蒯越造成 1 点伤害 → 审时（阴）可发动。
        crate::resolve::apply_damage(
            st,
            cur,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut inject = vec![
            format!("{cur_pid}=choose:shenshi"),
            format!("{cur_pid}=choose:{cost}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.shenshi_marks.len(),
        1,
        "审时（阴）：记录交给目标的牌"
    );
    assert!(!skills::yj_yin(&st, cur, "shenshi"), "阴面发动后转回阳面");
    // 回合结束阶段：目标未失去该牌 → 蒯越摸至 4 张。
    let room2 = room.clone();
    edit(&mgr, &room2, |st| {
        st.turn_phase = TurnPhase::End;
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room2);
    assert_eq!(st.seats[cur].hand.len(), 4, "审时（阴）：摸至 4 张");
}

// ---------------------------------------------------------------------------
// 许攸 · 成略（转换技）/ 恃才 / 寸目
// ---------------------------------------------------------------------------

#[test]
fn chenglue_yang_draws_one_discards_two() {
    let mgr = manager(991);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_406");
        give(st, cur, "sha");
        give(st, cur, "shan");
        give(st, cur, "tao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[cur].flags.remove("yj:chenglue");
        cur_pid = st.seats[cur].player_id.clone();
        crate::turn::execute(st, &cur_pid, "skill:chenglue").unwrap();
        let mut inject = vec![format!("{cur_pid}=choose:chenglue")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 2, "成略（阳）：摸 1 弃 2 → 3+1-2");
    assert!(skills::yj_yin(&st, cur, "chenglue"), "成略发动后转为阴面");
    assert!(st.history.iter().any(|e| e.text.contains("成略")));
}

#[test]
fn chenglue_yin_draws_two_discards_one() {
    let mgr = manager(994);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_406");
        give(st, cur, "sha");
        give(st, cur, "shan");
        give(st, cur, "tao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        // 阴面用例：显式写入转换态。
        st.seats[cur].flags.insert("yj:chenglue".into(), true);
        cur_pid = st.seats[cur].player_id.clone();
        crate::turn::execute(st, &cur_pid, "skill:chenglue").unwrap();
        let mut inject = vec![format!("{cur_pid}=choose:chenglue")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 4, "成略（阴）：摸 2 弃 1 → 3+2-1");
    assert!(!skills::yj_yin(&st, cur, "chenglue"), "阴面发动后转回阳面");
    let suits = ["cl_spade", "cl_heart", "cl_club", "cl_diamond"];
    assert_eq!(
        suits.iter().filter(|k| st.counter(k) > 0).count(),
        1,
        "成略：弃置牌花色免限（仅记录弃置牌花色）"
    );
}

#[test]
fn cunmu_draws_from_bottom_of_deck() {
    let mgr = manager(992);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut bottom) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_406");
        // 把一张牌固定到牌堆底（draw[0]）。
        bottom = give(st, cur, "sha");
        st.seats[cur].hand.retain(|c| c != &bottom);
        st.deck.draw.insert(0, bottom.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[cur].hand.iter().any(|c| c == &bottom),
        "寸目：从牌堆底摸牌"
    );
}

#[test]
fn shicai_puts_trick_on_top_and_draws() {
    let mgr = manager(993);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut trick, mut hand_before) = (0, String::new(), 0usize);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_406");
        trick = give(st, cur, "wuzhong_shengyou");
        for _ in 0..2 {
            give(st, cur, "sha");
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        hand_before = st.seats[cur].hand.len();
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, &format!("play:{trick}"))
        .unwrap();
    drop(cur_pid);
    // 无懈窗口：无人响应 → 结算无中生有。
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("恃才")),
        "恃才：使用首个锦囊后置于牌堆顶并摸 1"
    );
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before + 2,
        "恃才摸 1 + 无中生有摸 2 - 使用 1"
    );
    assert_eq!(
        st.deck.draw.last(),
        Some(&trick),
        "恃才：使用的牌置于牌堆顶"
    );
}
