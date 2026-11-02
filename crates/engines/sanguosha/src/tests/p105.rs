//! P6.3 真机 bug 回归：借刀杀人 × 酒 的归属、铁索连环多目标 + 逐目标无懈。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn hp_of(mgr: &game_engine_core::GameManager, room: &str, seat: usize) -> i32 {
    state_of(mgr, room).seats[seat].hp
}

/// 把当前所有无懈窗口都 pass 掉（锦囊结算前必有）。
fn pass_wuxie(mgr: &game_engine_core::GameManager, room: &str) {
    for _ in 0..30 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else { return };
        if p.kind != "wuxie" {
            return;
        }
        mgr.execute_action(room, &p.actor, "pass").unwrap();
    }
    panic!("无懈窗口未结束");
}

// ---------------------------------------------------------------------------
// 1) 酒：只有**饮酒者本人**的【杀】享受 +1
// ---------------------------------------------------------------------------

#[test]
fn jiu_bonus_applies_to_drinker_only() {
    let mgr = manager(5001);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        let order = st.seats_from_left_of(cur);
        a = order[0];
        b = order[1];
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_3");
        // 饮酒者（当前回合玩家）
        give(st, cur, "jiu");
        // 借刀杀人（cur 使用）
        give(st, cur, "jiedao_sharen");
        // 被借刀者 A 手上有一张【杀】，且装备武器
        give(st, a, "sha");
        let weapon = super::find_card(st, "qinggang_jian");
        take_card(st, &weapon);
        st.seats[a].equips.insert(EquipSlot::Weapon, Some(weapon));
        // B 无手牌（无法出闪）
        st.seats[b].hand.clear();
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let a_pid = pid(&st, a);
    let b_pid = pid(&st, b);
    let jiu = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("jiu"))
        .expect("jiu")
        .clone();
    let jiedao = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("jiedao_sharen"))
        .expect("jiedao")
        .clone();
    let a_sha = st.seats[a]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("sha"))
        .expect("sha")
        .clone();
    let hp_before = hp_of(&mgr, &room, b);

    // cur 饮酒 → 借 A 的刀杀 B。
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiu}"))
        .expect("use jiu");
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiedao}"))
        .expect("use jiedao");
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .expect("pick weapon owner");
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .expect("pick sha target");
    mgr.execute_action(&room, &cur_pid, "confirm")
        .expect("confirm jiedao");
    // 借刀是锦囊：先过无懈窗口，再轮到 A 被迫出【杀】。
    pass_wuxie(&mgr, &room);
    mgr.execute_action(&room, &a_pid, &format!("respond:{a_sha}"))
        .expect("A plays sha");
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.actor.as_str()) == Some(b_pid.as_str()) {
        mgr.execute_action(&room, &b_pid, "pass").expect("B pass");
    }
    let hp_after = hp_of(&mgr, &room, b);
    assert_eq!(
        hp_after,
        hp_before - 1,
        "借刀杀人的伤害不应吃到**另一个人**的酒加成（实际掉 {} 血）",
        hp_before - hp_after
    );
    // 酒标记应保留在 cur 名下（未被这次借刀消耗掉）。
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.counter("jiu_src"),
        cur as u32 + 1,
        "饮酒者标记不应被他人【杀】消耗"
    );
    st.check_conservation().unwrap();
}

#[test]
fn jiu_bonus_still_applies_to_own_sha() {
    let mgr = manager(5002);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut b) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        b = st
            .seats_from_left_of(cur)
            .into_iter()
            .find(|i| st.in_attack_range(cur, *i))
            .expect("in range");
        set_general(st, b, "sgs_hero_3");
        give(st, cur, "jiu");
        give(st, cur, "sha");
        st.seats[b].hand.clear();
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let b_pid = pid(&st, b);
    let jiu = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("jiu"))
        .unwrap()
        .clone();
    let sha = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("sha"))
        .unwrap()
        .clone();
    let hp_before = hp_of(&mgr, &room, b);
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiu}"))
        .expect("jiu");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .expect("sha");
    mgr.execute_action(&room, &cur_pid, &format!("target:{b_pid}"))
        .expect("target");
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.actor.as_str()) == Some(b_pid.as_str()) {
        mgr.execute_action(&room, &b_pid, "pass").expect("pass");
    }
    assert_eq!(
        hp_of(&mgr, &room, b),
        hp_before - 2,
        "自己喝的酒对自己的【杀】仍然 +1"
    );
}

// ---------------------------------------------------------------------------
// 2) 铁索连环：可选 2 名目标，且**每个目标各一次无懈链**
// ---------------------------------------------------------------------------

#[test]
fn tiesuo_can_pick_two_targets_with_per_target_wuxie() {
    let mgr = manager(5003);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        let order = st.seats_from_left_of(cur);
        a = order[0];
        b = order[1];
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_3");
        give(st, cur, "tiesuo_lianhuan");
        // 其余人各留 1 张非无懈手牌：引擎会逐个询问无懈（无手牌会被自动跳过），
        // 这样才能验证「每个目标各一次无懈链」。
        for i in st.alive_indices() {
            if i != cur {
                st.seats[i].hand.clear();
                give(st, i, "shan");
            }
        }
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let a_pid = pid(&st, a);
    let b_pid = pid(&st, b);
    let card = st.seats[cur].hand[0].clone();

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .expect("play tiesuo");
    // 双目标走 `targets:<p1,p2>`（组合由引擎在选项里枚举）。
    let st = state_of(&mgr, &room);
    let combo = format!("targets:{a_pid},{b_pid}");
    assert!(
        st.pending
            .as_ref()
            .map(|p| p.options.iter().any(|o| o.id == combo))
            .unwrap_or(false),
        "目标选项应包含双目标组合：{:?}",
        st.pending.map(|p| p.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())
    );
    mgr.execute_action(&room, &cur_pid, &combo)
        .expect("pick two targets");

    // 逐个目标各一次无懈链：全 pass 推进，期间统计 wuxie pending 次数。
    let mut wuxie_windows = 0;
    for _ in 0..40 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind == "wuxie" {
            wuxie_windows += 1;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(
        wuxie_windows >= 2,
        "每个目标各一次无懈链（实际 {wuxie_windows} 次询问）"
    );
    assert!(st.seats[a].chained, "目标 A 应横置");
    assert!(st.seats[b].chained, "目标 B 应横置");
    assert!(st.pending.is_none(), "结算应结束");
    st.check_conservation().unwrap();
}

/// 只选 1 名也可以确认（规则「至多两名」）。
#[test]
fn tiesuo_allows_single_target() {
    let mgr = manager(5004);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        a = st.seats_from_left_of(cur)[0];
        set_general(st, a, "sgs_hero_2");
        give(st, cur, "tiesuo_lianhuan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let a_pid = pid(&st, a);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .expect("play");
    mgr.execute_action(&room, &cur_pid, &format!("target:{a_pid}"))
        .expect("pick one");
    let mut inject: Vec<String> = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert!(st.seats[a].chained, "单目标也应结算");
    st.check_conservation().unwrap();
}
