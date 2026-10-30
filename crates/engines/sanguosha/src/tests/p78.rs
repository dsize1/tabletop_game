//! P4.4.1 SP-2 用例（一）：卢植 / 郝昭 / 陈到 / 诸葛瞻。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, give, manager, state_of, take_card};
use crate::skills;
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

/// 把一张指定花色的牌放入某座位的手牌（从其它区域取出）。
fn give_suit(st: &mut SgsState, seat: usize, red: bool) -> String {
    let id = st
        .cards
        .iter()
        .find(|(id, c)| {
            skills::is_red(c.suit) == red
                && !st
                    .seats
                    .iter()
                    .any(|s| s.hand.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.marks.values().flatten().any(|h| h == *id))
                && !st.deck.draw.iter().any(|d| d == *id)
                && !st.deck.processing.iter().any(|d| d == *id)
        })
        .map(|(id, _)| id.clone())
        .expect("没有可用的同色牌");
    take_card(st, &id);
    st.seats[seat].hand.push(id.clone());
    id
}

// ---------------------------------------------------------------------------
// 卢植 · 明任 / 贞良
// ---------------------------------------------------------------------------

#[test]
fn mingren_places_ren_from_hand() {
    let mgr = manager(1000);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut picked) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_407");
        give(st, cur, "sha");
        give(st, cur, "shan");
        cur_pid = st.seats[cur].player_id.clone();
        crate::resolve_sp2::init_mingren(st, cur);
        // 明任初始化：摸 2 后选择 1 张手牌为「任」。
        picked = st.seats[cur].hand[0].clone();
        let mut inject = vec![format!("{cur_pid}=choose:{picked}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        skills::mark_cards(&st, cur, "ren"),
        vec![picked.clone()],
        "明任：一张手牌置于武将牌上为「任」"
    );
    assert!(!st.seats[cur].hand.iter().any(|c| *c == picked));
    assert!(st.history.iter().any(|e| e.text.contains("明任")));
}

#[test]
fn zhenliang_yang_deals_damage_with_same_color() {
    let mgr = manager(1001);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    let mut hp_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_407");
        target = st.alive_indices()
            .into_iter()
            .find(|i| *i != cur && st.in_attack_range(cur, *i))
            .expect("攻击范围内应有目标");
        set_general(st, target, "sgs_hero_1");
        // 「任」为红牌，手里给红牌。
        let ren = give_suit(st, cur, true);
        skills::add_mark(st, cur, "ren", &ren);
        st.seats[cur].hand.retain(|c| c != &ren);
        give_suit(st, cur, true);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[cur].flags.remove("yj:zhenliang");
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp_before = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, "skill:zhenliang").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let card = st.pending.as_ref().map(|p| {
        p.options
            .first()
            .map(|o| o.id.clone())
            .unwrap_or_default()
    });
    if let Some(action) = card {
        mgr.execute_action(&room, &cur_pid, &action).unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_before - 1, "贞良（阳）：造成 1 点伤害");
    assert!(skills::yj_yin(&st, cur, "zhenliang"), "发动后转为阴面");
}

#[test]
fn zhenliang_yin_makes_target_draw() {
    let mgr = manager(1002);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut other, mut owner_pid, mut other_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_407");
        other = other_seats(st, owner)[0];
        set_general(st, other, "sgs_hero_1");
        // 「任」为黑牌；回合外使用/打出的黑牌入弃牌堆。
        let ren = give_suit(st, owner, false);
        skills::add_mark(st, owner, "ren", &ren);
        st.seats[owner].hand.retain(|c| c != &ren);
        let card = give_suit(st, owner, false);
        st.seats[owner].flags.insert("yj:zhenliang".into(), true);
        st.turn = other; // 卢植的回合外
        st.turn_counters.clear();
        owner_pid = st.seats[owner].player_id.clone();
        other_pid = st.seats[other].player_id.clone();
        crate::resolve_sp2::push_zhenliang_yin(st, owner, &card);
        let mut inject = vec![
            format!("{owner_pid}=choose:zhenliang_yin"),
            format!("{owner_pid}=target:{other_pid}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[other].hand.len(), 1, "贞良（阴）：令一名角色摸 1 张");
    assert!(!skills::yj_yin(&st, owner, "zhenliang"), "阴面发动后转回阳面");
    let _ = owner_pid;
}

// ---------------------------------------------------------------------------
// 郝昭 · 镇骨
// ---------------------------------------------------------------------------

#[test]
fn zhengu_adjusts_this_turn_and_next_turn() {
    let mgr = manager(1003);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_408");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..3 {
            give(st, cur, "sha");
        }
        for _ in 0..5 {
            give(st, target, "sha");
        }
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=target:{target_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 3, "镇骨：本回合结束时手牌调整至与郝昭相同");
    assert_eq!(st.scheduled.len(), 1, "镇骨：登记下回合结束的延迟结算");
    // 目标回合结束时再次调整。
    edit(&mgr, &room, |st| {
        st.turn = target;
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        for _ in 0..2 {
            give(st, target, "sha");
        }
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 3, "镇骨：目标下回合结束再次调整");
    assert!(st.scheduled.is_empty(), "镇骨：延迟结算已消费");
}

#[test]
fn zhengu_draws_up_to_owner_hand() {
    let mgr = manager(1004);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_408");
        // 目标选「非下一个行动者」，避免 drive 推进到目标自己的回合。
        target = other_seats(st, cur)[2];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..4 {
            give(st, cur, "sha");
        }
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=target:{target_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 4, "镇骨：目标摸至与郝昭相同");
    let _ = cur_pid;
}

#[test]
fn zhengu_caps_at_five_cards() {
    let mgr = manager(1012);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_408");
        target = other_seats(st, cur)[2];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..6 {
            give(st, cur, "sha");
        }
        for _ in 0..8 {
            give(st, target, "sha");
        }
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=target:{target_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 5, "镇骨：至多摸/弃至 5 张");
}

// ---------------------------------------------------------------------------
// 陈到 · 往烈
// ---------------------------------------------------------------------------

#[test]
fn wanglie_sha_cannot_be_responded() {
    let mgr = manager(1005);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut cur_pid, mut target_pid, mut hp) =
        (0, 0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_409");
        target = crate::turn::sha_targets(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "shan");
        sha = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, &format!("skill:wanglie:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "往烈：目标不能使用【闪】");
    assert!(st.history.iter().any(|e| e.text.contains("往烈")));
}

#[test]
fn wanglie_blocks_further_cards_this_phase() {
    let mgr = manager(1006);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut sha, mut cur_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_409");
        sha = give(st, cur, "sha");
        give(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.set_counter("wanglie_used", 1);
        st.set_counter("no_card_use", 1);
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let actions = crate::turn::legal_actions(&st, &cur_pid);
    assert_eq!(actions.len(), 1, "往烈：本阶段只剩「结束出牌」");
    assert_eq!(actions[0].id, "end_play");
    let _ = sha;
}

#[test]
fn wanglie_first_card_ignores_distance() {
    let mgr = manager(1007);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far, mut sha, mut cur_pid, mut far_pid) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_409");
        far = other_seats(st, cur)
            .into_iter()
            .find(|i| !st.in_attack_range(cur, *i))
            .expect("应有距离 >1 的角色");
        set_general(st, far, "sgs_hero_1");
        sha = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        far_pid = st.seats[far].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("skill:wanglie:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let options: Vec<String> = st
        .pending
        .as_ref()
        .map(|p| p.options.iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default();
    assert!(
        options.contains(&format!("target:{far_pid}")),
        "往烈：出牌阶段首张牌无距离限制"
    );
}

#[test]
fn wanglie_trick_skips_wuxie() {
    let mgr = manager(1008);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut trick, mut cur_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_409");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
            give(st, seat, "sha");
        }
        trick = give(st, cur, "guohe_chaiqiao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("skill:wanglie:{trick}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    // 无懈可击窗口被跳过 → 直接进入目标选择。
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("target"),
        "往烈：锦囊跳过无懈窗口"
    );
}

// ---------------------------------------------------------------------------
// 诸葛瞻 · 罪论 / 父荫
// ---------------------------------------------------------------------------

#[test]
fn zuilun_gains_cards_by_conditions() {
    let mgr = manager(1009);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_410");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        st.set_counter("damage_dealt", 1);
        // 手牌数全场最少 + 未弃牌 → 共 3 项满足。
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "罪论：满足 3 项各获得 1 张");
    assert!(st.special_zones.get("zuilun").is_none(), "罪论：特殊区已清理");
}

#[test]
fn zuilun_zero_conditions_loses_hp() {
    let mgr = manager(1010);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut other, mut cur_pid, mut other_pid, mut hp) =
        (0, 0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_410");
        other = other_seats(st, cur)[0];
        set_general(st, other, "sgs_hero_1");
        for _ in 0..2 {
            give(st, cur, "sha");
        }
        give(st, other, "sha");
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        // 未造成伤害 + 弃置过牌 + 手牌不是最少 → 0 项。
        st.set_counter("discard_done", 1);
        cur_pid = st.seats[cur].player_id.clone();
        other_pid = st.seats[other].player_id.clone();
        hp = st.seats[cur].hp;
        let mut inject = vec![format!("{cur_pid}=target:{other_pid}")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp - 1, "罪论：均不满足，自己失去 1 点体力");
    assert_eq!(st.seats[other].hp, st.seats[other].max_hp - 1, "被选角色失去 1 点体力");
}

#[test]
fn fuyin_blocks_first_sha_each_turn() {
    let mgr = manager(1011);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut zhuge, mut sha, mut att_pid, mut zhuge_pid, mut hp) =
        (0, 0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        zhuge = crate::turn::sha_targets(st, attacker)[0];
        set_general(st, zhuge, "sgs_hero_410");
        for _ in 0..2 {
            give(st, zhuge, "shan");
        }
        for _ in 0..3 {
            give(st, attacker, "sha");
        }
        sha = give(st, attacker, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        att_pid = st.seats[attacker].player_id.clone();
        zhuge_pid = st.seats[zhuge].player_id.clone();
        hp = st.seats[zhuge].hp;
    });
    mgr.execute_action(&room, &att_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &att_pid, &format!("target:{zhuge_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhuge].hp, hp, "父荫：此【杀】无效");
    assert!(st.history.iter().any(|e| e.text.contains("父荫")));
}
