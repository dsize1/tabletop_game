//! P4.4.1 SP-2 用例（二）：周妃 / 毌丘俭 / 陆抗 / 张绣。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_phases, edit, give, manager, state_of};
use crate::skills;
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices().into_iter().filter(|i| *i != cur).collect()
}

// ---------------------------------------------------------------------------
// 周妃 · 良姻 / 箜声
// ---------------------------------------------------------------------------

#[test]
fn kongsheng_keeps_cards_and_returns_non_equips() {
    let mgr = manager(1020);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut kept) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_411");
        give(st, cur, "sha");
        kept = give(st, cur, "shan");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=choose:{kept}"), format!("{cur_pid}=confirm")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(skills::mark_cards(&st, cur, "kong"), vec![kept.clone()], "箜声：置于武将牌上");
    assert_eq!(st.seats[cur].hand.len(), 3, "准备阶段正常摸 2（手里 1 张保留）");
    // 结束阶段：获得非装备牌。
    edit(&mgr, &room, |st| {
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].hand.iter().any(|c| *c == kept), "箜声：结束阶段获得非装备牌");
    assert!(skills::mark_cards(&st, cur, "kong").is_empty());
}

/// 箜声准备阶段可置**装备区**的牌，并触发良姻「移出」（每回合首次）。
#[test]
fn kongsheng_can_keep_equipment_cards() {
    let mgr = manager(1025);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut equip) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_411");
        // 把一张武器牌放进装备区，验证「装备区的牌也能置为箜声」。
        equip = give(st, cur, "qinggang_jian");
        st.seats[cur].hand.retain(|c| c != &equip);
        st.seats[cur]
            .equips
            .insert(EquipSlot::Weapon, Some(equip.clone()));
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut inject = vec![format!("{cur_pid}=choose:{equip}"), format!("{cur_pid}=confirm")];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        skills::mark_cards(&st, cur, "kong"),
        vec![equip.clone()],
        "箜声：装备区的牌也能置于武将牌上"
    );
    assert!(
        st.seats[cur].equips.get(&EquipSlot::Weapon).cloned().flatten().is_none(),
        "装备区应已移除该牌"
    );
}

/// 良姻「弃置」分支：由两名玩家各自选牌（不是自动弃首张）。
#[test]
fn liangyin_discard_is_chosen_by_each_player() {
    let mgr = manager(1026);
    let room = start_and_pick(&mgr, 5);
    let (mut zhou, mut other, mut zhou_pid, mut other_pid) = (0, 0, String::new(), String::new());
    let (mut zhou_cards, mut other_cards) = (Vec::new(), Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhou = st.turn;
        set_general(st, zhou, "sgs_hero_411");
        other = other_seats(st, zhou)[0];
        set_general(st, other, "sgs_hero_1");
        for _ in 0..3 {
            zhou_cards.push(give(st, zhou, "sha"));
            other_cards.push(give(st, other, "shan"));
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        zhou_pid = st.seats[zhou].player_id.clone();
        other_pid = st.seats[other].player_id.clone();
        // 直接触发良姻「移出」分支（其余钩子行为另有用例）。
        crate::resolve_sp2::liangyin_trigger(st, crate::resolve_sp2::MoveDir::Out);
    });
    let mut ev = Vec::new();
    edit(&mgr, &room, |st| crate::resolve::advance(st, &mut ev).unwrap());
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("良姻应询问是否发动");
    assert_eq!(p.skill.as_deref(), Some("liangyin"));
    mgr.execute_action(&room, &zhou_pid, "choose:liangyin:discard")
        .unwrap();
    mgr.execute_action(&room, &zhou_pid, &format!("target:{other_pid}"))
        .unwrap();
    // 周妃自选（选最后一张，而非首张）。
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("周妃应自选弃牌");
    assert_eq!(p.kind, "discard");
    assert_eq!(p.actor, zhou_pid);
    let pick = p.options.last().unwrap().id.trim_start_matches("discard:").to_string();
    mgr.execute_action(&room, &zhou_pid, &format!("discard:{pick}"))
        .unwrap();
    // 对方自选。
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("对方应自选弃牌");
    assert_eq!(p.kind, "discard");
    assert_eq!(p.actor, other_pid);
    let pick2 = p.options.first().unwrap().id.trim_start_matches("discard:").to_string();
    mgr.execute_action(&room, &other_pid, &format!("discard:{pick2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhou].hand.len(), 2, "周妃自选弃 1 张");
    assert_eq!(st.seats[other].hand.len(), 2, "对方自选弃 1 张");
    assert!(!st.seats[zhou].hand.contains(&pick));
    assert!(!st.seats[other].hand.contains(&pick2));
}

/// 权计（钟会）把牌置于武将牌上为「权」= 移出游戏 → 触发良姻「移出」。
#[test]
fn quanji_triggers_liangyin_out() {
    let mgr = manager(1027);
    let room = start_and_pick(&mgr, 5);
    let (mut zhou, mut zhong, mut zhou_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        zhou = st.turn;
        set_general(st, zhou, "sgs_hero_411"); // 周妃：良姻
        zhong = other_seats(st, zhou)[0];
        set_general(st, zhong, "sgs_hero_173"); // 钟会：权计
        for _ in 0..3 {
            give(st, zhong, "shan");
        }
        zhou_pid = st.seats[zhou].player_id.clone();
        // 周妃回合内钟会受伤 → 权计置「权」→ 良姻「移出」。
        crate::resolve::apply_damage(
            st,
            zhong,
            1,
            Some(zhou),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        skills::mark_cards(&st, zhong, "quan").len(),
        1,
        "权计：一张手牌置于武将牌上为「权」"
    );
    let p = st.pending.clone().expect("良姻（移出）应询问周妃");
    assert_eq!(p.skill.as_deref(), Some("liangyin"));
    assert_eq!(p.actor, zhou_pid);
}

#[test]
fn kongsheng_equip_cards_used_by_target() {
    let mgr = manager(1021);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut hp) =
        (0, 0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_411");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        let weapon = give(st, cur, "qinggang_jian");
        st.seats[cur].hand.retain(|c| c != &weapon);
        skills::add_mark(st, cur, "kong", &weapon);
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
        let mut inject = vec![
            format!("{cur_pid}=target:{target_pid}"),
            format!("{cur_pid}=target:{target_pid}"),
        ];
        drive_phases(st, &mut inject);
        st.seats[target].equips.contains_key(&EquipSlot::Weapon);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].equips[&EquipSlot::Weapon].is_some(),
        "箜声：目标使用剩余装备牌"
    );
    assert_eq!(st.seats[target].hp, hp - 1, "箜声：目标失去 1 点体力");
}

#[test]
fn liangyin_triggers_on_card_leaving_game() {
    let mgr = manager(1022);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut other, mut owner_pid, mut other_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_411");
        other = other_seats(st, owner)[0];
        set_general(st, other, "sgs_hero_1");
        let card = give(st, owner, "sha");
        st.seats[owner].hand.retain(|c| c != &card);
        st.turn_counters.clear();
        owner_pid = st.seats[owner].player_id.clone();
        other_pid = st.seats[other].player_id.clone();
        // 牌移出游戏（void）→ 良姻触发。
        crate::turn::to_void_card(st, &card);
        let mut inject = vec![
            format!("{owner_pid}=choose:liangyin:draw"),
            format!("{owner_pid}=target:{other_pid}"),
        ];
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[other].hand.len(), 1, "良姻：与另一名角色各摸 1 张");
    assert_eq!(st.seats[owner].hand.len(), 1, "良姻：自己也摸 1 张");
    assert!(st.history.iter().any(|e| e.text.contains("良姻")));
}

// ---------------------------------------------------------------------------
// 毌丘俭 · 征荣 / 鸿举
// ---------------------------------------------------------------------------

#[test]
fn zhengrong_takes_card_as_rong() {
    let mgr = manager(1023);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut cur_pid, mut target_pid) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
            give(st, seat, "shan");
            give(st, seat, "sha");
        }
        target = crate::turn::sha_targets(st, cur)[0];
        sha = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 征荣（使用【杀】时）在目标选择前询问。
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.skill.clone()) == Some(Some("zhengrong".into())) {
        mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
            .unwrap();
    }
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(skills::mark_cards(&st, cur, "rong").len(), 1, "征荣：将目标一张牌置为「荣」");
    assert!(st.history.iter().any(|e| e.text.contains("征荣")));
}

#[test]
fn hongju_awakening_replaces_rong() {
    let mgr = manager(1024);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut hand) = (0, String::new(), Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        for _ in 0..2 {
            hand.push(give(st, cur, "sha"));
        }
        for def in ["shan", "tao", "jiu"] {
            let card = give(st, cur, def);
            st.seats[cur].hand.retain(|c| c != &card);
            skills::add_mark(st, cur, "rong", &card);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        let mut inject = Vec::new();
        for card in hand.iter() {
            inject.push(format!("{cur_pid}=choose:{card}"));
        }
        inject.push(format!("{cur_pid}=confirm"));
        drive_phases(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("awaken_hongju"), "鸿举：已觉醒");
    assert!(skills::has(&st, cur, "qingce"), "鸿举：获得「清侧」");
    assert_eq!(skills::mark_cards(&st, cur, "rong").len(), 3, "鸿举：荣数量不变（等量替换）");
    assert!(st.history.iter().any(|e| e.text.contains("鸿举")));
}

#[test]
fn hongju_requires_three_rong() {
    let mgr = manager(1025);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_413");
        for def in ["shan", "tao"] {
            let card = give(st, cur, def);
            st.seats[cur].hand.retain(|c| c != &card);
            skills::add_mark(st, cur, "rong", &card);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(!st.seats[cur].flags.contains_key("awaken_hongju"), "荣不足 3 不觉醒");
    assert!(!skills::has(&st, cur, "qingce"));
}

// ---------------------------------------------------------------------------
// 陆抗 · 谦节 / 决堰 / 破势
// ---------------------------------------------------------------------------

#[test]
fn jueyan_abolishes_slot_and_draws() {
    let mgr = manager(1026);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_414");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:jueyan").unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:jueyan:armor")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[cur].abolished_slots.contains(&EquipSlot::Armor),
        "决堰：废除防具栏"
    );
    assert_eq!(st.seats[cur].hand.len(), 3, "决堰（防具栏）：摸 3 张");
    assert_eq!(st.counter("jueyan_hand_limit"), 3, "决堰（防具栏）：手牌上限 +3");
    assert_eq!(
        crate::skills::hand_limit(&st, cur),
        st.seats[cur].hp as usize + 3,
        "手牌上限计入决堰"
    );
}

#[test]
fn jueyan_horse_ignores_distance() {
    let mgr = manager(1027);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut far, mut cur_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_414");
        far = other_seats(st, cur)
            .into_iter()
            .find(|i| !st.in_attack_range(cur, *i))
            .expect("应有距离 >1 的角色");
        set_general(st, far, "sgs_hero_1");
        give(st, far, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:jueyan").unwrap();
    mgr.execute_action(&room, &cur_pid, "choose:jueyan:horse")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(crate::turn::trick_distance_ok(&st, cur, far), "决堰（坐骑栏）：无距离限制");
}

#[test]
fn poshi_awakening_swaps_skill() {
    let mgr = manager(1028);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut max_before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_414");
        st.seats[cur].hp = 1;
        max_before = st.seats[cur].max_hp;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_phases(st, &mut Vec::new());
    });
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].flags.contains_key("awaken_poshi"), "破势：已觉醒");
    assert_eq!(st.seats[cur].max_hp, max_before - 1, "破势：减少 1 点体力上限");
    assert!(!skills::has(&st, cur, "jueyan"), "破势：失去「决堰」");
    assert!(skills::has(&st, cur, "huairou"), "破势：获得「怀柔」");
}

#[test]
fn qianjie_blocks_chaining_and_delayed_tricks() {
    let mgr = manager(1029);
    let room = start_and_pick(&mgr, 5);
    let (mut lu, mut target_pid, mut chained) = (0, String::new(), false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lu = other_seats(st, st.turn)[0];
        set_general(st, lu, "sgs_hero_414");
        target_pid = st.seats[lu].player_id.clone();
        // 铁索连环：直接对陆抗结算（谦节阻止横置）。
        st.seats[lu].chained = false;
        st.seats[lu].chained = false;
        let _ = &mut chained;
        st.seats[lu].chained = true;
        // 谦节持有人不能成为延时锦囊目标：由引擎目标枚举排除。
        st.seats[lu].chained = false;
        crate::skills::qianjie_blocks_target(st, lu);
        chained = st.seats[lu].chained;
    });
    let st = state_of(&mgr, &room);
    assert!(!chained);
    assert!(skills::qianjie_blocks_target(&st, lu), "谦节：不能成为延时锦囊/拼点目标");
    let _ = target_pid;
}

// ---------------------------------------------------------------------------
// 张绣 · 雄乱（限定）/ 从谏
// ---------------------------------------------------------------------------

#[test]
fn xiongluan_abolishes_and_seals_target() {
    let mgr = manager(1030);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid) = (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_415");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "shan");
        // 先装备一张武器，验证废除装备区时弃置。
        let weapon = give(st, cur, "qinggang_jian");
        st.seats[cur].hand.retain(|c| c != &weapon);
        st.seats[cur]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:xiongluan").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].judge_abolished, "雄乱：废除判定区");
    assert_eq!(
        st.seats[cur].abolished_slots.len(),
        EquipSlot::ALL.len(),
        "雄乱：废除全部装备栏"
    );
    assert!(skills::hand_sealed(&st, target), "雄乱：目标本回合不能使用/打出手牌");
    assert_eq!(
        skills::xiongluan_target(&st, cur),
        Some(target),
        "雄乱：记录本回合目标"
    );
    assert_eq!(crate::turn::sha_limit(&st, cur), u32::MAX, "雄乱：本回合【杀】无次数限制");
}

#[test]
fn xiongluan_sealed_target_cannot_play_shan() {
    let mgr = manager(1031);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut cur_pid, mut target_pid, mut hp) =
        (0, 0, String::new(), String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_415");
        target = crate::turn::sha_targets(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "shan");
        sha = give(st, cur, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        // 直接置雄乱状态（避免先选目标）。
        st.set_counter(&format!("hand_sealed:{target}"), 1);
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
        hp = st.seats[target].hp;
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    super::drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "雄乱：被封角色无法打出【闪】");
}

#[test]
fn congjian_gives_card_and_draws() {
    let mgr = manager(1032);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut zhangxiu, mut aoe, mut att_pid, mut zx_pid) =
        (0, 0, String::new(), String::new(), String::new());
    let mut others = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        zhangxiu = other_seats(st, attacker)[0];
        set_general(st, zhangxiu, "sgs_hero_415");
        give(st, zhangxiu, "shan");
        give(st, zhangxiu, "sha");
        for seat in other_seats(st, attacker) {
            if seat == zhangxiu {
                continue;
            }
            set_general(st, seat, "sgs_hero_2");
            others.push(st.seats[seat].player_id.clone());
        }
        aoe = give(st, attacker, "wanjian_qifa");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        att_pid = st.seats[attacker].player_id.clone();
        zx_pid = st.seats[zhangxiu].player_id.clone();
    });
    mgr.execute_action(&room, &att_pid, &format!("play:{aoe}"))
        .unwrap();
    // 从谏：成为多目标锦囊目标后询问。
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.clone()),
        Some("congjian".into()),
        "从谏：多目标锦囊触发"
    );
    mgr.execute_action(&room, &zx_pid, "choose:congjian")
        .unwrap();
    mgr.execute_action(&room, &zx_pid, &format!("choose:{}", {
        let st = state_of(&mgr, &room);
        st.seats[zhangxiu]
            .hand
            .iter()
            .find(|c| st.cards.get(*c).is_some_and(|i| i.def == "shan"))
            .cloned()
            .expect("手里有闪")
    }))
    .unwrap();
    mgr.execute_action(&room, &zx_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    if st.pending.as_ref().map(|p| p.kind.as_str()) == Some("target") {
        mgr.execute_action(&room, &zx_pid, &format!("target:{}", others[0]))
            .unwrap();
    }
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("从谏")), "从谏：已交牌并摸牌");
    let _ = (attacker, zhangxiu, aoe, att_pid, zx_pid);
}
