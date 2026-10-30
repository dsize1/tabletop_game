//! P4.4.1 风包技能用例（魏延/夏侯渊/曹仁/小乔/黄忠/周泰/张角）。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, flip_seat, give, manager, place_mark,
    state_of, take_card,
};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

/// 取一张满足条件、不在任何玩家手牌里的牌。
fn free_card_where(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let claimed = |id: &str| {
        st.seats.iter().any(|s| {
            s.hand.iter().any(|c| c == id)
                || s.marks.values().flatten().any(|c| c == id)
                || s.judge_zone.iter().any(|c| c == id)
                || s.equips.values().flatten().any(|c| c == id)
        })
    };
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"))
}

fn top_judge(st: &mut SgsState, filter: impl Fn(&CardInst) -> bool) -> String {
    let id = free_card_where(st, filter);
    take_card(st, &id);
    st.deck.draw.push(id.clone());
    id
}

// ---------------------------------------------------------------------------
// 魏延
// ---------------------------------------------------------------------------

#[test]
fn kuanggu_heals_after_damage() {
    let mgr = manager(500);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_27");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = st.seats[cur].max_hp - 1;
        st.seats[target].hp = 2;
        sha = give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{target_pid}|respond=pass"),
            format!("{cur_pid}|choice=choose:heal"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 1, "杀应造成 1 点伤害");
    assert_eq!(
        st.seats[cur].hp, st.seats[cur].max_hp,
        "狂骨应回复 1 点体力"
    );
    assert!(st.history.iter().any(|e| e.text.contains("狂骨")));
}

#[test]
fn kuanggu_can_draw_instead() {
    let mgr = manager(501);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_27");
        set_general(st, target, "sgs_hero_2");
        st.seats[target].hp = 2;
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    let sha = st.seats[cur]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).is_some_and(|c| c.def == "sha"))
        .cloned()
        .expect("有杀");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let hand_before_kuanggu = state_of(&mgr, &room).seats[cur].hand.len();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{target_pid}|respond=pass"),
            format!("{cur_pid}|choice=choose:draw"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), hand_before_kuanggu + 1);
}

#[test]
fn qimou_loses_hp_and_boosts_distance_and_sha_limit() {
    let mgr = manager(502);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut far = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        far = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_27");
        st.seats[cur].hp = 4;
    });
    let st = state_of(&mgr, &room);
    assert!(!crate::turn::trick_distance_ok(&st, cur, far));
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, "skill:qimou").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("qimou")
    );
    mgr.execute_action(&room, &cur_pid, "choose:qimou:2")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, 2);
    assert_eq!(st.counter("qimou"), 2);
    assert!(st.seats[cur].flags.contains_key("qimou_used"));
    assert_eq!(crate::turn::sha_limit(&st, cur), 3);
    assert!(crate::turn::trick_distance_ok(&st, cur, far));
    // 限定技：本局不可再发动。
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id == "skill:qimou"));
}

// ---------------------------------------------------------------------------
// 夏侯渊
// ---------------------------------------------------------------------------

#[test]
fn shensu_skips_judge_and_draw_and_uses_virtual_sha() {
    let mgr = manager(510);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_28");
        set_general(st, target, "sgs_hero_2");
        let pid_cur = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_cur}|choice=choose:shensu:1"),
                format!(
                    "{pid_cur}|target=target:{}",
                    pid(st, st.seats_from_left_of(cur)[0])
                ),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        st.seats[target].max_hp - 1,
        "神速虚拟杀应命中"
    );
    assert!(st.history.iter().any(|e| e.text.contains("神速①")));
    assert!(st
        .history
        .iter()
        .any(|e| e.text.contains("神速：跳过判定阶段")));
    assert!(st.history.iter().any(|e| e.text.contains("跳过摸牌阶段")));
    assert_eq!(st.turn_phase, TurnPhase::Play);
}

#[test]
fn shensu_second_option_discards_equipment_and_skips_play() {
    let mgr = manager(511);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut equip) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_28");
        let pid_cur = pid(st, cur);
        equip = give(st, cur, "bagua_zhen");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_cur}|choice=choose:shensu:2"),
                format!("{pid_cur}|choice=choose:{equip}"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&equip), "装备应被弃置");
    assert!(st.history.iter().any(|e| e.text.contains("神速②")));
    assert!(st.history.iter().any(|e| e.text.contains("出牌阶段被跳过")));
}

// ---------------------------------------------------------------------------
// 曹仁
// ---------------------------------------------------------------------------

#[test]
fn jushou_flips_draws_and_uses_equipment() {
    let mgr = manager(520);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut equip) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_29");
        equip = give(st, cur, "qinggang_jian");
        let pid_cur = pid(st, cur);
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_cur}|choice=choose:jushou"),
                format!("{pid_cur}|choose_many=choose:{equip}"),
                format!("{pid_cur}|choose_many=confirm"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flipped, "据守应翻面");
    assert_eq!(
        st.seats[cur]
            .equips
            .get(&EquipSlot::Weapon)
            .cloned()
            .flatten(),
        Some(equip),
        "据守弃置的装备应改为使用"
    );
    assert!(st.history.iter().any(|e| e.text.contains("据守")));
}

#[test]
fn jiewei_moves_field_card_after_flip_back() {
    let mgr = manager(521);
    let room = start_and_pick(&mgr, 5);
    let (mut caoren, mut other, mut equip) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caoren = st.turn;
        other = st.seats_from_left_of(caoren)[0];
        set_general(st, caoren, "sgs_hero_29");
        set_general(st, other, "sgs_hero_2");
        flip_seat(st, caoren);
        give(st, caoren, "shan");
        equip = give(st, other, "qinggang_jian");
        take_card(st, &equip);
        st.seats[other]
            .equips
            .insert(EquipSlot::Weapon, Some(equip.clone()));
        let pid_cr = pid(st, caoren);
        let hand = st.seats[caoren].hand[0].clone();
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_cr}|choice=choose:jiewei"),
                format!("{pid_cr}|choice=choose:{hand}"),
                format!("{pid_cr}|target=move:{equip}"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[caoren].flipped, "翻回正面");
    assert_eq!(
        st.seats[caoren]
            .equips
            .get(&EquipSlot::Weapon)
            .cloned()
            .flatten(),
        Some(equip),
        "应移动到曹仁装备区"
    );
    assert!(st.seats[other]
        .equips
        .get(&EquipSlot::Weapon)
        .is_none_or(|c| c.is_none()));
    assert!(st.history.iter().any(|e| e.text.contains("解围")));
}

#[test]
fn jiewei_equipment_works_as_wuxie() {
    let mgr = manager(522);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut caoren, mut equip, mut wuzhong) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        caoren = st.seats_from_left_of(cur)[0];
        set_general(st, caoren, "sgs_hero_29");
        equip = give(st, caoren, "bagua_zhen");
        take_card(st, &equip);
        st.seats[caoren]
            .equips
            .insert(EquipSlot::Armor, Some(equip.clone()));
        wuzhong = give(st, cur, "wuzhong_shengyou");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let cr_pid = pid(&st, caoren);
    mgr.execute_action(&room, &cur_pid, &format!("play:{wuzhong}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![format!("{cr_pid}|wuxie=skill:jiewei:{equip}")],
    );
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&equip), "装备应进入弃牌堆");
    assert!(st.history.iter().any(|e| e.text.contains("解围")));
    assert!(st.history.iter().any(|e| e.text.contains("效果被抵消")));
}

// ---------------------------------------------------------------------------
// 小乔
// ---------------------------------------------------------------------------

#[test]
fn hongyan_spade_is_heart_for_tianxiang() {
    let mgr = manager(530);
    let room = start_and_pick(&mgr, 5);
    let (mut xq, mut target, mut src, mut spade, mut target_before) = (0, 0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        xq = st.turn;
        target = st.seats_from_left_of(xq)[0];
        src = st.seats_from_left_of(xq)[1];
        set_general(st, xq, "sgs_hero_30");
        set_general(st, target, "sgs_hero_2");
        spade = free_card_where(st, |c| c.suit == Suit::Spade);
        take_card(st, &spade);
        st.seats[xq].hand.push(spade.clone());
        let pid_xq = pid(st, xq);
        let pid_target = pid(st, target);
        target_before = st.seats[target].hand.len();
        crate::resolve::apply_damage(
            st,
            xq,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_xq}|choice=choose:tianxiang"),
                format!("{pid_xq}|choose_many=choose:{spade}"),
                format!("{pid_xq}|choose_many=confirm"),
                format!("{pid_xq}|target=target:{pid_target}"),
                format!("{pid_xq}|choice=choose:tianxiang:damage"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[xq].hp, st.seats[xq].max_hp, "天香应防止伤害");
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp - 1);
    assert_eq!(
        st.seats[target].hand.len(),
        target_before + 1,
        "选项 1 应摸 1 张（已损失体力 1）"
    );
    assert!(st.deck.discard.contains(&spade));
    assert!(st.history.iter().any(|e| e.text.contains("天香")));
}

#[test]
fn tianxiang_can_decline_and_take_damage() {
    let mgr = manager(531);
    let room = start_and_pick(&mgr, 5);
    let mut xq = 0;
    let mut src = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        xq = st.turn;
        src = st.seats_from_left_of(xq)[0];
        set_general(st, xq, "sgs_hero_30");
        let heart = free_card_where(st, |c| c.suit == Suit::Heart);
        take_card(st, &heart);
        st.seats[xq].hand.push(heart);
        let pid_xq = pid(st, xq);
        crate::resolve::apply_damage(
            st,
            xq,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(st, &mut vec![format!("{pid_xq}|choice=pass")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[xq].hp, st.seats[xq].max_hp - 1);
    assert_eq!(st.seats[xq].hand.len(), 1, "红桃手牌未弃置");
}

#[test]
fn tianxiang_second_option_loses_hp_and_gains_card() {
    let mgr = manager(532);
    let room = start_and_pick(&mgr, 5);
    let (mut xq, mut target, mut src, mut heart) = (0, 0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        xq = st.turn;
        target = st.seats_from_left_of(xq)[0];
        src = st.seats_from_left_of(xq)[1];
        set_general(st, xq, "sgs_hero_30");
        set_general(st, target, "sgs_hero_2");
        heart = free_card_where(st, |c| c.suit == Suit::Heart);
        take_card(st, &heart);
        st.seats[xq].hand.push(heart.clone());
        let pid_xq = pid(st, xq);
        let pid_target = pid(st, target);
        crate::resolve::apply_damage(
            st,
            xq,
            1,
            Some(src),
            crate::resolve::DamageType::Normal,
            None,
        );
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_xq}|choice=choose:tianxiang"),
                format!("{pid_xq}|choose_many=choose:{heart}"),
                format!("{pid_xq}|choose_many=confirm"),
                format!("{pid_xq}|target=target:{pid_target}"),
                format!("{pid_xq}|choice=choose:tianxiang:lose"),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[xq].hp, st.seats[xq].max_hp);
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp - 1);
    assert!(st.seats[target].hand.contains(&heart), "目标获得弃置的牌");
}

// ---------------------------------------------------------------------------
// 黄忠
// ---------------------------------------------------------------------------

#[test]
fn liegong_blocks_shan_when_hand_threshold_met() {
    let mgr = manager(540);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_61");
        set_general(st, target, "sgs_hero_2");
        st.seats[cur].hp = 4;
        sha = give(st, cur, "sha");
        give(st, target, "shan");
        for _ in 0..3 {
            give(st, target, "tao");
        }
        assert!(st.seats[target].hand.len() >= st.seats[cur].hp as usize);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "烈弓封锁后不应有【闪】响应窗口");
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp - 1);
    assert!(st.history.iter().any(|e| e.text.contains("烈弓")));
}

#[test]
fn liegong_does_not_block_when_condition_fails() {
    let mgr = manager(541);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_61");
        set_general(st, target, "sgs_hero_2");
        // 攻击者 4 血、范围 1；目标 2 张手牌 → 2 < 4 且 2 > 1，不满足。
        sha = give(st, cur, "sha");
        give(st, target, "shan");
        give(st, target, "tao");
        assert_eq!(st.seats[target].hand.len(), 2);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.kind == "respond" && p.actor == target_pid),
        "未封锁时应询问【闪】"
    );
}

// ---------------------------------------------------------------------------
// 周泰
// ---------------------------------------------------------------------------

#[test]
fn buqu_unique_rank_saves_and_sets_hand_limit() {
    let mgr = manager(550);
    let room = start_and_pick(&mgr, 5);
    let mut zt = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zt = st.turn;
        set_general(st, zt, "sgs_hero_210");
        st.seats[zt].hp = 1;
        top_judge(st, |c| c.rank == 3);
        crate::resolve::apply_damage(st, zt, 1, None, crate::resolve::DamageType::Normal, None);
        drive_state_inject(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zt].hp, 1, "不屈应回复至 1 点体力");
    assert_eq!(crate::skills::buqu_cards(&st, zt).len(), 1);
    assert_eq!(crate::skills::hand_limit(&st, zt), 1);
    assert!(st.history.iter().any(|e| e.text.contains("不屈")));
}

#[test]
fn buqu_duplicate_rank_does_not_save() {
    let mgr = manager(551);
    let room = start_and_pick(&mgr, 5);
    let mut zt = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zt = st.turn;
        set_general(st, zt, "sgs_hero_210");
        st.seats[zt].hp = 1;
        // 已有「创」点数 5。
        let first = free_card_where(st, |c| c.rank == 5);
        place_mark(st, zt, "buqu", &first);
        top_judge(st, |c| c.rank == 5);
        crate::resolve::apply_damage(st, zt, 1, None, crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zt].hp, 0, "不屈未生效，仍处于濒死");
    assert_eq!(
        crate::skills::buqu_cards(&st, zt).len(),
        1,
        "重复点数的新「创」应置入弃牌堆"
    );
}

#[test]
fn fenji_loses_hp_to_let_target_draw() {
    let mgr = manager(552);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut zt, mut guohe) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zt = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, zt, "sgs_hero_210");
        give(st, zt, "shan");
        guohe = give(st, cur, "guohe_chaiqiao");
        st.seats[zt].hp = st.seats[zt].max_hp;
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, zt_pid) = (pid(&st, cur), pid(&st, zt));
    mgr.execute_action(&room, &cur_pid, &format!("play:{guohe}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{zt_pid}"))
        .unwrap();
    drive_mgr_inject(
        &mgr,
        &room,
        &mut vec![
            format!("{cur_pid}|pick_card=pick:hand"),
            format!("{zt_pid}|choice=choose:fenji"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[zt].hp,
        st.seats[zt].max_hp - 1,
        "奋激失去 1 点体力"
    );
    assert_eq!(st.seats[zt].hand.len(), 2, "应摸 2 张（原 1 张被弃）");
    assert!(st.history.iter().any(|e| e.text.contains("奋激")));
}

// ---------------------------------------------------------------------------
// 张角
// ---------------------------------------------------------------------------

#[test]
fn guidao_rewrites_judgement_with_black_card() {
    let mgr = manager(560);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut zj, mut black) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        zj = st.seats_from_left_of(owner)[1];
        set_general(st, owner, "sgs_hero_2");
        set_general(st, zj, "sgs_hero_212");
        black = free_card_where(st, |c| c.suit == Suit::Club);
        take_card(st, &black);
        st.seats[zj].hand.push(black.clone());
        let lebu = super::find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[owner].judge_zone.push(lebu);
        top_judge(st, |c| c.suit == Suit::Heart);
        let pid_zj = pid(st, zj);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![format!("{pid_zj}|judge_rewrite=respond:{black}")],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("鬼道")));
    assert!(st.history.iter().any(|e| e.text.contains("非红桃")));
}

#[test]
fn leiji_spade_deals_two_thunder_damage() {
    let mgr = manager(561);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut zj, mut victim, mut sha, mut shan) = (0, 0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zj = st.seats_from_left_of(cur)[0];
        victim = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, zj, "sgs_hero_212");
        set_general(st, victim, "sgs_hero_23");
        sha = give(st, cur, "sha");
        shan = give(st, zj, "shan");
        top_judge(st, |c| c.suit == Suit::Spade);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, zj_pid, victim_pid) = (pid(&st, cur), pid(&st, zj), pid(&st, victim));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{zj_pid}"))
        .unwrap();
    mgr.execute_action(&room, &zj_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("leiji"),
        "张角打出【闪】后应可发动雷击"
    );
    mgr.execute_action(&room, &zj_pid, &format!("target:{victim_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[victim].hp,
        st.seats[victim].max_hp - 2,
        "黑桃判定应造成 2 点雷电伤害"
    );
    assert!(st.history.iter().any(|e| e.text.contains("雷击")));
}

#[test]
fn huangtian_gives_shan_to_lord_zhangjiao() {
    let mgr = manager(562);
    let room = start_and_pick(&mgr, 5);
    let (mut giver, mut zj, mut shan) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let lord = st.lord_index().expect("lord");
        set_general(st, lord, "sgs_hero_212");
        zj = lord;
        giver = st.seats_from_left_of(lord)[0];
        set_general(st, giver, "sgs_hero_23"); // 吕布（群）
        shan = give(st, giver, "shan");
        st.turn = giver;
        st.turn_phase = TurnPhase::Play;
    });
    let st = state_of(&mgr, &room);
    let giver_pid = pid(&st, giver);
    assert!(crate::skills::huangtian_owner(&st, giver).is_some());
    mgr.execute_action(&room, &giver_pid, &format!("skill:huangtian:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[zj].hand.contains(&shan), "张角应获得【闪】");
    assert_eq!(st.counter("huangtian_used"), 1);
    assert!(!st.seats[giver].hand.contains(&shan));
}

#[test]
fn shensu_third_option_flips_and_skips_discard() {
    let mgr = manager(512);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_28");
        let pid_cur = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!("{pid_cur}|choice=choose:shensu:3"),
                format!(
                    "{pid_cur}|target=target:{}",
                    pid(st, st.seats_from_left_of(cur)[0])
                ),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flipped, "神速③应翻面");
    assert_eq!(st.counter("shensu_skip_discard"), 1);
    assert!(st.history.iter().any(|e| e.text.contains("神速③")));
}

#[test]
fn liegong_blocks_when_target_hand_within_attack_range() {
    let mgr = manager(542);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_61");
        set_general(st, target, "sgs_hero_2");
        // 攻击者 4 血、攻击范围 1；目标 1 张【闪】→ 1 <= 1，满足封锁条件。
        st.seats[cur].hp = 4;
        sha = give(st, cur, "sha");
        give(st, target, "shan");
        assert_eq!(st.seats[target].hand.len(), 1);
    });
    let st = state_of(&mgr, &room);
    let (cur_pid, target_pid) = (pid(&st, cur), pid(&st, target));
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "手牌数 ≤ 攻击范围时烈弓封锁【闪】");
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp - 1);
}
