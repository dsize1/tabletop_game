//! P4.6 回归：
//! - 观星右列（牌堆底）真实回写到牌堆底（顶侧牌 → 原牌堆 → 底侧牌）；
//! - 界观星「全部置底 → 结束阶段再次发动」全流程，且不复用上一次的观星牌；
//! - 寒冰剑逐张弃牌选择（手牌/装备）与放弃时保留火/雷伤害类型（铁索传导）。

use super::p24::{in_hand, set_general, start_and_pick};
use super::{clear_hands, edit, give, manager, state_of, take_card};
use crate::state::*;
use game_engine_core::GameManager;
use serde_json::{json, Value};

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn equip_weapon(st: &mut SgsState, seat: usize, code: &str) {
    let id = super::find_card(st, code);
    take_card(st, &id);
    st.seats[seat].equips.insert(EquipSlot::Weapon, Some(id));
}

/// 下一位玩家换成指定武将并结束当前回合，停在观星 `arrange` pending。
fn enter_guanxing(mgr: &GameManager, room: &str, general: &str) -> (usize, String) {
    edit(mgr, room, |st| {
        clear_hands(st);
        let next = st.seats_from_left_of(st.turn)[0];
        set_general(st, next, general);
    });
    let st = state_of(mgr, room);
    let cur_pid = pid(&st, st.turn);
    mgr.execute_action(room, &cur_pid, "end_play").unwrap();
    let st = state_of(mgr, room);
    let pending = st.pending.clone().expect("观星应在准备阶段停下");
    assert_eq!(pending.kind, "arrange");
    (st.turn, pending.actor)
}

fn slot_card(st: &SgsState, slot: &str) -> String {
    st.special_zones["guanxing"].payload["slots"][slot]
        .as_str()
        .unwrap_or_else(|| panic!("槽位 {slot} 应有牌"))
        .to_string()
}

/// 模拟端侧拖拽：把已经填在左列的牌按顺序移到右列（右列 = 牌堆底侧）。
fn move_left_to_right(mgr: &GameManager, room: &str, from: usize, count: usize) {
    edit(mgr, room, |st| {
        let zone = st.special_zones.get_mut("guanxing").expect("观星区");
        let slots = zone
            .payload
            .get_mut("slots")
            .and_then(Value::as_object_mut)
            .expect("slots");
        let ids: Vec<String> = (from..from + count)
            .map(|i| {
                slots[&format!("L{i}")]
                    .as_str()
                    .expect("左列有牌")
                    .to_string()
            })
            .collect();
        for (n, id) in ids.iter().enumerate() {
            slots.insert(format!("L{}", from + n), Value::Null);
            slots.insert(format!("R{}", n + 1), json!(id));
        }
    });
}

#[test]
fn guanxing_right_column_goes_to_deck_bottom() {
    let mgr = manager(1700);
    let room = start_and_pick(&mgr, 5);
    let (owner, owner_pid) = enter_guanxing(&mgr, &room, "sgs_hero_4");

    let st = state_of(&mgr, &room);
    let cards: Vec<String> = (1..=5).map(|k| slot_card(&st, &format!("L{k}"))).collect();
    let draw_before = st.deck.draw.clone();

    // L1、L2 留顶；L3..L5 → R1..R3（牌堆底）。
    move_left_to_right(&mgr, &room, 3, 3);

    let st = state_of(&mgr, &room);
    let action = format!(
        "arrange:{},{}|{},{},{}",
        cards[0], cards[1], cards[2], cards[3], cards[4]
    );
    // 端侧拖拽只改本地槽位；pending 的默认选项仍是初始布局 → 提交必须自带货列分组。
    assert!(st.pending.as_ref().unwrap().options[0].id.starts_with("arrange:"));
    mgr.execute_action(&room, &owner_pid, &action).unwrap();

    // 观星后摸牌阶段摸走新牌堆顶 2 张 = 左列两张。
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn, owner);
    assert_eq!(
        st.seats[owner].hand,
        vec![cards[0].clone(), cards[1].clone()],
        "左列首张/次张成为新牌堆顶并被摸走"
    );

    // 用户口径：牌堆（底 → 顶）= R3,R2,R1 + 原牌堆；顶 → 底 = L1,L2,原牌堆,R1,R2,R3。
    let mut expected: Vec<String> = vec![
        cards[4].clone(),
        cards[3].clone(),
        cards[2].clone(),
    ];
    expected.extend(draw_before.iter().cloned());
    assert_eq!(
        st.deck.draw, expected,
        "右列必须真实放在牌堆底（原牌堆之上为顶侧牌）"
    );
    st.check_conservation().unwrap();
}

#[test]
fn jie_guanxing_all_bottom_repeats_at_end_without_reuse() {
    let mgr = manager(1701);
    let room = start_and_pick(&mgr, 5);
    let (owner, owner_pid) = enter_guanxing(&mgr, &room, "sgs_hero_440");

    let st = state_of(&mgr, &room);
    let k = crate::resolve_zone::guanxing_count(&st);
    assert_eq!(k, 5, "5 人局界观星看 5 张");
    let cards: Vec<String> = (1..=k).map(|i| slot_card(&st, &format!("L{i}"))).collect();
    move_left_to_right(&mgr, &room, 1, k);

    let action = format!("arrange:|{}", cards.join(",")); // 左列空 → 全部右列
    mgr.execute_action(&room, &owner_pid, &action).unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.turn, owner);
    assert_eq!(st.turn_phase, TurnPhase::Play, "观星后进入出牌阶段");
    assert_eq!(st.counter("guanxing_bottom"), 1, "全部置底标记结束阶段复发动");
    // 全部置底：观星牌应位于牌堆底部（底 → 顶 = R5..R1 + 原牌堆）。
    let mut expected: Vec<String> = cards.iter().rev().cloned().collect();
    expected.extend(
        st.deck
            .draw
            .iter()
            .filter(|id| !cards.contains(id))
            .cloned()
            .collect::<Vec<_>>(),
    );
    assert_eq!(st.deck.draw, expected, "全部置底应落在牌堆底部");

    // 结束出牌 → 结束阶段再次观星。
    mgr.execute_action(&room, &owner_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    let p = st
        .pending
        .clone()
        .expect("界观星：结束阶段应再次发动观星");
    assert_eq!(p.kind, "arrange");
    assert_eq!(p.actor, owner_pid);
    let zone_cards: Vec<String> = st.special_zones["guanxing"].payload["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(zone_cards.len(), crate::resolve_zone::guanxing_count(&st));
    assert!(
        zone_cards.iter().all(|id| !cards.contains(id)),
        "第二次观星必须看新牌堆顶，不得复用上一次的牌：{zone_cards:?}"
    );

    // 提交第二次观星（默认顺序：不置底）→ 同回合不得第三次观星。
    let action = p.options[0].id.clone();
    mgr.execute_action(&room, &owner_pid, &action).unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending.as_ref().is_none_or(|p| p.kind != "arrange"),
        "同回合最多再观星一次"
    );
    st.check_conservation().unwrap();
}

#[test]
fn jie_guanxing_mixed_placement_does_not_repeat_at_end() {
    let mgr = manager(1702);
    let room = start_and_pick(&mgr, 5);
    let (owner, owner_pid) = enter_guanxing(&mgr, &room, "sgs_hero_440");

    let st = state_of(&mgr, &room);
    let cards: Vec<String> = (1..=5).map(|k| slot_card(&st, &format!("L{k}"))).collect();
    // 仅 2 张置底（混合布局）。
    move_left_to_right(&mgr, &room, 4, 2);
    let action = format!(
        "arrange:{},{},{}|{},{}",
        cards[0], cards[1], cards[2], cards[3], cards[4]
    );
    mgr.execute_action(&room, &owner_pid, &action).unwrap();

    mgr.execute_action(&room, &owner_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| !(p.kind == "arrange" && p.actor == owner_pid)),
        "未全部置底时结束阶段不得再次观星"
    );
    let _ = owner;
    st.check_conservation().unwrap();
}

/// 【杀】指定目标后：若有【闪】响应窗口则显式放弃。
fn sha_and_give_up(mgr: &GameManager, room: &str, cur: usize, target: usize, card: &str) {
    let st = state_of(mgr, room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    mgr.execute_action(room, &cur_pid, &format!("play:{card}"))
        .unwrap();
    mgr.execute_action(room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(mgr, room);
    if st.pending.as_ref().is_some_and(|p| p.kind == "respond") {
        mgr.execute_action(room, &target_pid, "pass").unwrap();
    }
}

#[test]
fn hanbing_pick_equipment_explicitly() {
    let mgr = manager(1703);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        equip_weapon(st, cur, "hanbing_jian");
        give(st, cur, "sha");
        give(st, target, "tao");
        // 目标另有一张防具：寒冰剑应允许指定弃置装备。
        let armor = super::find_card(st, "bagua_zhen");
        take_card(st, &armor);
        st.seats[target].equips.insert(EquipSlot::Armor, Some(armor));
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp = st.seats[target].hp;
    let sha = in_hand(&st, cur, "sha");
    sha_and_give_up(&mgr, &room, cur, target, &sha);

    assert!(
        state_of(&mgr, &room)
            .pending
            .as_ref()
            .is_some_and(|p| p.skill.as_deref() == Some("hanbing"))
    );
    mgr.execute_action(&room, &cur_pid, "choose:hanbing")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == "pick:equip:armor")),
        "寒冰剑可指定弃置装备区道具"
    );
    mgr.execute_action(&room, &cur_pid, "pick:equip:armor")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "pick:hand").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "弃牌后防止伤害");
    assert!(
        st.seats[target].equips.get(&EquipSlot::Armor).is_none_or(Option::is_none),
        "装备被弃置"
    );
    st.check_conservation().unwrap();
}

#[test]
fn hanbing_pass_keeps_fire_damage_and_chain() {
    let mgr = manager(1704);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut other) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        other = st.seats_from_left_of(cur)[1];
        set_general(st, target, "sgs_hero_2");
        set_general(st, other, "sgs_hero_3");
        for s in [target, other] {
            st.seats[s].max_hp = 5;
            st.seats[s].hp = 5;
            st.seats[s].chained = true;
        }
        equip_weapon(st, cur, "hanbing_jian");
        give(st, cur, "huo_sha");
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp_t = st.seats[target].hp;
    let hp_o = st.seats[other].hp;
    let sha = in_hand(&st, cur, "huo_sha");
    sha_and_give_up(&mgr, &room, cur, target, &sha);

    assert!(
        state_of(&mgr, &room)
            .pending
            .as_ref()
            .is_some_and(|p| p.skill.as_deref() == Some("hanbing")),
        "寒冰剑应可发动"
    );
    // 放弃寒冰剑：必须按原伤害类型（火）结算，铁索继续传导。
    mgr.execute_action(&room, &cur_pid, "pass").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_t - 1, "火【杀】造成 1 点火焰伤害");
    assert_eq!(st.seats[other].hp, hp_o - 1, "横置角色受到铁索传导");
    assert!(
        !st.seats[target].chained && !st.seats[other].chained,
        "传导后重置连环状态"
    );
    st.check_conservation().unwrap();
}

#[test]
fn cixiong_keeps_fire_damage_and_chain() {
    let mgr = manager(1705);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut other) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        other = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_2"); // 关羽（男）
        set_general(st, target, "sgs_hero_25"); // 孙尚香（女）
        for s in [target, other] {
            st.seats[s].max_hp = 5;
            st.seats[s].hp = 5;
            st.seats[s].chained = true;
        }
        equip_weapon(st, cur, "cixiong_shuanggujian");
        give(st, cur, "huo_sha");
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp_t = st.seats[target].hp;
    let hp_o = st.seats[other].hp;
    let sha = in_hand(&st, cur, "huo_sha");
    let discard = in_hand(&st, target, "tao");
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    // 雌雄：目标弃 1 张手牌，然后仍需响应【杀】（无【闪】→ 受伤）。
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().unwrap().skill.as_deref(), Some("cixiong"));
    mgr.execute_action(&room, &target_pid, "choose:discard").unwrap();
    mgr.execute_action(&room, &target_pid, &format!("choose:{discard}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().is_some_and(|p| p.kind == "respond") {
        mgr.execute_action(&room, &target_pid, "pass").unwrap();
    }
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_t - 1, "火【杀】造成火焰伤害");
    assert_eq!(st.seats[other].hp, hp_o - 1, "雌雄窗口后铁索仍传导");
    assert!(!st.seats[target].chained && !st.seats[other].chained);
    st.check_conservation().unwrap();
}

#[test]
fn guanshi_keeps_fire_damage_and_chain() {
    let mgr = manager(1706);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut other) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        other = st.seats_from_left_of(cur)[1];
        set_general(st, target, "sgs_hero_2");
        for s in [target, other] {
            st.seats[s].max_hp = 5;
            st.seats[s].hp = 5;
            st.seats[s].chained = true;
        }
        equip_weapon(st, cur, "guanshi_fu");
        give(st, cur, "huo_sha");
        give(st, cur, "tao");
        give(st, cur, "tao");
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp_t = st.seats[target].hp;
    let hp_o = st.seats[other].hp;
    let sha = in_hand(&st, cur, "huo_sha");
    let shan = in_hand(&st, target, "shan");
    let discards: Vec<String> = st.seats[cur]
        .hand
        .iter()
        .filter(|id| st.cards.get(*id).is_some_and(|c| c.def == "tao"))
        .cloned()
        .collect();
    assert_eq!(discards.len(), 2);
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();

    // 贯石斧：弃 2 张手牌 → 【杀】依然造成伤害（按原火属性 → 传导）。
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().unwrap().skill.as_deref(),
        Some("after_dodge")
    );
    mgr.execute_action(&room, &cur_pid, "choose:guanshi").unwrap();
    for id in &discards {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{id}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_t - 1, "贯石斧：目标仍受到伤害");
    assert_eq!(st.seats[other].hp, hp_o - 1, "贯石斧窗口后铁索仍传导");
    assert!(!st.seats[target].chained && !st.seats[other].chained);
    st.check_conservation().unwrap();
}

#[test]
fn every_registered_equipment_has_effect_text() {
    for def in crate::skills::EQUIPMENT_SKILLS {
        assert!(
            crate::skills::equipment_effect(def.card).is_some(),
            "{} 缺少装备效果文案（长按牌面会显示空白）",
            def.code
        );
    }
    assert!(
        crate::skills::equipment_effect("sha").is_none(),
        "非装备牌不应有装备效果文案"
    );
}
