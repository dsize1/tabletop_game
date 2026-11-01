//! P6 Batch-1.5：自对弈暴露的**引擎侧合法集/结算不一致**回归（E1–E5 及同类）。
//!
//! 背景：全将（149 位）自对弈（`npm run check:sgs-ai:full`）暴露「合法集给出动作、
//! 结算却拒绝」的多处不一致。本批次修复后补用例锁死口径，避免再退化。
//!
//! 对应清单（`docs/games/sanguosha/ai.md` §7）：
//! - E3：`sha_used` 与「无距离」被混用 → 合法集给出【杀】但结算拒绝；
//! - E4：陷嗣选满 2 名角色后仍给出第三人；
//! - E5：界仁德未过滤「本阶段已发动过」的角色；
//! - 同源：司敌/讥谀封锁颜色未从合法集剔除。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of};
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

// ---------------------------------------------------------------------------
// E3：无距离 ≠ 不计次数
// ---------------------------------------------------------------------------

#[test]
fn sha_legal_respects_count_when_only_distance_free() {
    let mgr = manager(4100);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        stage_play(st, cur);
        target = other_seats(st, cur)
            .into_iter()
            .find(|i| st.in_attack_range(cur, *i))
            .expect("in-range target");
        set_general(st, cur, "sgs_hero_1");
        give_def(st, cur, "sha");
        // 将驰②：本回合【杀】无距离限制（但不是「不计次数」）。
        st.set_counter("jiangchi_no_distance", 1);
        st.set_counter("sha_used", 1);
        let _ = target;
    });
    let st = state_of(&mgr, &room);
    let pid = pid_of(&st, cur);
    let legal = mgr.legal_actions(&room, &pid).expect("legal");
    assert!(
        !legal.iter().any(|a| a.id.starts_with("play:")),
        "第 2 张【杀】不应出现在合法集（仅无距离不改次数）：{legal:?}"
    );
    // 结算侧同样拒绝（口径一致）。
    let sha = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.effective_def_code(id) == Some("sha"))
        .expect("sha")
        .clone();
    let err = mgr
        .execute_action(&room, &pid, &format!("play:{sha}"))
        .expect_err("应与合法集一致地拒绝");
    assert!(format!("{err}").contains("已使用过【杀】"), "错误文案：{err}");
}

// ---------------------------------------------------------------------------
// E5：界仁德每名角色每阶段限一次 —— 已发动过的目标不再出现在合法集
// ---------------------------------------------------------------------------

#[test]
fn jie_rende_excludes_already_targeted() {
    let mgr = manager(4101);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut pid, mut first, mut first_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        stage_play(st, cur);
        set_general(st, cur, "sgs_hero_299"); // 界刘备（仁德/激将）
        pid = pid_of(st, cur);
        first = other_seats(st, cur)[0];
        first_pid = pid_of(st, first);
        give_def(st, cur, "sha");
        give_def(st, cur, "tao");
    });
    // 第一次仁德：选 1 张给 first。
    mgr.execute_action(&room, &pid, "skill:rende").expect("rende");
    let st = state_of(&mgr, &room);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .expect("select");
    mgr.execute_action(&room, &pid, "confirm").expect("confirm");
    mgr.execute_action(&room, &pid, &format!("target:{first_pid}"))
        .expect("target");
    // 第二次仁德：Target 阶段不应再给出 first。
    mgr.execute_action(&room, &pid, "skill:rende")
        .expect("rende again");
    let st = state_of(&mgr, &room);
    let card = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .expect("select again");
    mgr.execute_action(&room, &pid, "confirm").expect("confirm again");
    let st = state_of(&mgr, &room);
    let opts = pending_options_with(&st, "target:");
    assert!(
        !opts.contains(&format!("target:{first_pid}")),
        "界仁德：已发动过的角色不应再出现在目标选项：{opts:?}"
    );
    assert!(!opts.is_empty(), "应仍有其他可选目标");
}

// ---------------------------------------------------------------------------
// E4：陷嗣选满 2 名角色后不再给出第三人
// ---------------------------------------------------------------------------

#[test]
fn xiansi_stops_offering_targets_after_two() {
    let mgr = manager(4102);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut pid, mut others) = (0, String::new(), Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        stage_play(st, cur);
        set_general(st, cur, "sgs_hero_179"); // 刘封（陷嗣）
        pid = pid_of(st, cur);
        others = other_seats(st, cur);
        for seat in &others {
            give_def(st, *seat, "sha");
        }
        // 直接压入陷嗣帧（准备阶段技能的「选择角色」阶段）。
        crate::resolve::push(
            st,
            crate::resolve::Frame::XianSi(crate::resolve_yijiang4::XianSiFrame {
                actor: cur,
                selected: Vec::new(),
            }),
        );
        st.pending = None;
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).expect("advance");
    });
    // 选两名角色。
    for seat in others.iter().take(2) {
        let st = state_of(&mgr, &room);
        let target_pid = pid_of(&st, *seat);
        mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
            .expect("select target");
    }
    let st = state_of(&mgr, &room);
    let opts = pending_options_with(&st, "target:");
    assert!(
        opts.len() <= 2,
        "陷嗣选满 2 人后只应保留可取消的已选项：{opts:?}"
    );
    for seat in others.iter().skip(2) {
        let target_pid = pid_of(&st, *seat);
        assert!(
            !opts.contains(&format!("target:{target_pid}")),
            "第三名角色不应出现在选项：{opts:?}"
        );
    }
    // 确认仍可结算。
    mgr.execute_action(&room, &pid, "confirm").expect("confirm");
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "陷嗣确认后应结束该交互");
}

// ---------------------------------------------------------------------------
// 同源：司敌封锁颜色的手牌不应出现在合法集
// ---------------------------------------------------------------------------

#[test]
fn sidiqu_banned_card_not_in_legal() {
    let mgr = manager(4103);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut banned) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        stage_play(st, cur);
        set_general(st, cur, "sgs_hero_1");
        banned = give_def(st, cur, "sha");
        // 司敌：本阶段不能使用某一颜色的牌（1=红，2=黑；按【杀】的颜色封锁）。
        let bit = if crate::skills::is_red(st.cards[&banned].suit) {
            1
        } else {
            2
        };
        st.set_counter(&format!("sidiqu:{}", cur), bit);
    });
    let st = state_of(&mgr, &room);
    let pid = pid_of(&st, cur);
    let legal = mgr.legal_actions(&room, &pid).expect("legal");
    assert!(
        !legal.iter().any(|a| a.id == format!("play:{banned}")),
        "被司敌封锁颜色的牌不应在合法集：{legal:?}"
    );
}
