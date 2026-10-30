//! P3 反馈修复用例：闪电只对自己、顺手/过河目标须有牌、麒麟弓、历史全量。

use super::p24::{in_hand, start_and_pick};
use super::{clear_hands, edit, give, manager, player_view, state_of};
use crate::resolve::{DamageType, Frame};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn first_in_range(st: &SgsState, cur: usize, need_cards: bool) -> usize {
    st.alive_indices()
        .into_iter()
        .find(|i| {
            *i != cur
                && st.in_attack_range(cur, *i)
                && (!need_cards || {
                    !st.seats[*i].hand.is_empty()
                        || !st.seats[*i].judge_zone.is_empty()
                        || st.seats[*i].equips.values().any(|v| v.is_some())
                })
        })
        .expect("攻击范围内目标")
}

#[test]
fn shandian_targets_self_only_and_judges_first() {
    let mgr = manager(501);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "shandian");
        // 已有乐不思蜀在判定区（先入），闪电后入 → LIFO 下闪电先判
        let lebu = crate::tests::find_card(st, "lebu_sishu");
        crate::tests::take_card(st, &lebu);
        st.seats[cur].judge_zone.push(lebu);
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let card = in_hand(&st, cur, "shandian");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "闪电不应要求选择目标");
    assert_eq!(st.seats[cur].judge_zone.len(), 2);
    assert_eq!(
        st.seats[cur].judge_zone.last().map(|c| c.as_str()),
        Some(card.as_str()),
        "闪电置入自己判定区，且后入 → 下回合最先判定"
    );
    let view = player_view(&mgr, &room, &cur_pid);
    let opts = view["player_area"]["pending"].clone();
    assert!(opts.is_null(), "个人视图无目标待办");
    st.check_conservation().unwrap();
}

#[test]
fn shunshou_and_guohe_require_target_with_cards() {
    let mgr = manager(502);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "shunshou_qianyang");
        give(st, cur, "guohe_chaiqiao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let shunshou = in_hand(&st, cur, "shunshou_qianyang");
    let guohe = in_hand(&st, cur, "guohe_chaiqiao");

    // 全场无牌：两张锦囊都不可用
    let legal = mgr.legal_actions(&room, &cur_pid).unwrap();
    let ids: Vec<String> = legal.into_iter().map(|a| a.id).collect();
    assert!(
        !ids.contains(&format!("play:{shunshou}")),
        "无目标有牌时顺手不可用：{ids:?}"
    );
    assert!(
        !ids.contains(&format!("play:{guohe}")),
        "无目标有牌时过河不可用：{ids:?}"
    );

    // 给一个目标一张手牌 → 两张都可用，且目标列表只包含有牌者
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        let target = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur && st.in_attack_range(cur, *i))
            .expect("目标");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let target = first_in_range(&st, cur, true);
    let target_pid = pid(&st, target);
    let legal = mgr.legal_actions(&room, &cur_pid).unwrap();
    let ids: Vec<String> = legal.into_iter().map(|a| a.id).collect();
    assert!(
        ids.contains(&format!("play:{shunshou}")),
        "有目标有牌 → 顺手可用"
    );
    assert!(
        ids.contains(&format!("play:{guohe}")),
        "有目标有牌 → 过河可用"
    );

    mgr.execute_action(&room, &cur_pid, &format!("play:{shunshou}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应选目标");
    assert_eq!(pending.kind, "target");
    let targets: Vec<String> = pending.options.iter().map(|o| o.id.clone()).collect();
    assert_eq!(
        targets,
        vec![format!("target:{target_pid}")],
        "只列出有牌的目标"
    );
    st.check_conservation().unwrap();
}

#[test]
fn qilin_bow_discards_one_horse_after_sha_damage() {
    let mgr = manager(503);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let bow = crate::tests::find_card(st, "qilin_gong");
        crate::tests::take_card(st, &bow);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(bow));
        let victim = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur && st.in_attack_range(cur, *i))
            .expect("目标");
        let horse1 = crate::tests::find_card(st, "chitu");
        crate::tests::take_card(st, &horse1);
        st.seats[victim]
            .equips
            .insert(EquipSlot::HorseOffense, Some(horse1));
        let horse2 = crate::tests::find_card(st, "dilu");
        crate::tests::take_card(st, &horse2);
        st.seats[victim]
            .equips
            .insert(EquipSlot::HorseDefense, Some(horse2));
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let victim = first_in_range(&st, cur, false);
    let victim_pid = pid(&st, victim);
    let sha = in_hand(&st, cur, "sha");
    let discard_before = st.deck.discard.len();

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    // 目标无闪 → 直接伤害 → 麒麟弓询问攻击方
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("麒麟弓应询问攻击方");
    assert_eq!(pending.kind, "choice");
    assert_eq!(pending.skill.as_deref(), Some("qilin"));
    assert_eq!(pending.actor, cur_pid, "由攻击方选择");
    let ids: Vec<String> = pending.options.iter().map(|o| o.id.clone()).collect();
    assert!(ids.contains(&"skill:qilin:horse_offense".to_string()));
    assert!(ids.contains(&"skill:qilin:horse_defense".to_string()));
    assert!(ids.contains(&"pass".to_string()));

    mgr.execute_action(&room, &cur_pid, "skill:qilin:horse_offense")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[victim].equips[&EquipSlot::HorseOffense].is_none());
    assert!(
        st.seats[victim].equips[&EquipSlot::HorseDefense].is_some(),
        "只弃置一匹"
    );
    assert_eq!(
        st.deck.discard.len(),
        discard_before + 2,
        "杀 + 坐骑进弃牌堆"
    );
    st.check_conservation().unwrap();
}

#[test]
fn qilin_bow_not_triggered_by_duel_damage() {
    let mgr = manager(504);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        let bow = crate::tests::find_card(st, "qilin_gong");
        crate::tests::take_card(st, &bow);
        st.seats[cur].equips.insert(EquipSlot::Weapon, Some(bow));
        let victim = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur)
            .expect("目标");
        let horse = crate::tests::find_card(st, "chitu");
        crate::tests::take_card(st, &horse);
        st.seats[victim]
            .equips
            .insert(EquipSlot::HorseOffense, Some(horse));
    });
    let full = mgr.get_state(&room).unwrap();
    let mut st = SgsState::parse(&full.data).unwrap();
    let cur = st.turn;
    let victim = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != cur)
        .expect("目标");
    let duel = crate::tests::find_card(&st, "juedou");
    crate::tests::take_card(&mut st, &duel);
    st.deck.processing.push(duel.clone());
    crate::resolve::apply_damage(
        &mut st,
        victim,
        1,
        Some(cur),
        DamageType::Normal,
        Some(&duel),
    );
    assert!(
        !st.resolution.iter().any(|f| matches!(f, Frame::Qilin(_))),
        "决斗（非杀）不触发麒麟弓"
    );
}

#[test]
fn history_is_complete_and_view_exposes_all() {
    let mgr = manager(505);
    let room = start_and_pick(&mgr, 5);
    let base = state_of(&mgr, &room).history.len();
    edit(&mgr, &room, |st| {
        for i in 0..300 {
            st.push_log(None, format!("压测日志 {i}"));
        }
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.history.len(), base + 300, "历史上限内不丢条");
    let view = player_view(&mgr, &room, "u1");
    let history = view["public_board"]["history"].as_array().expect("history");
    assert_eq!(
        history.len(),
        base + 300,
        "视图下发全量历史（不再只给最近 50）"
    );
    let tail = &history[history.len() - 300..];
    assert_eq!(tail[0]["text"], "压测日志 0");
    assert_eq!(tail[299]["text"], "压测日志 299");
}
