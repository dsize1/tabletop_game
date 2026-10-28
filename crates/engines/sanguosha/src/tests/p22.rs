//! P2.2 测试：回合流程、基础牌、装备/距离、判定区 LIFO、持有上限、濒死。

use super::{
    clear_hands, drive as drive_state, edit, find_card, give, lobby_with, manager, pick_all,
    player_view, public_view, resolve_arrange, state_of, take_card,
};
use crate::state::*;
use game_engine_core::GameManager;

fn start_and_pick(mgr: &GameManager, players: usize) -> String {
    let room = lobby_with(mgr, players);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

/// 当前玩家攻击范围内的第一个其他存活玩家。
fn attack_target(st: &SgsState, from: usize) -> usize {
    st.alive_indices()
        .into_iter()
        .find(|t| *t != from && st.in_attack_range(from, *t))
        .expect("攻击范围内应有目标")
}

fn find_card_where(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    st.cards
        .iter()
        .find(|(_, c)| f(c))
        .map(|(id, _)| id.clone())
        .expect("没有满足条件的牌")
}

#[test]
fn turn_flow_auto_draws_and_stops_at_play() {
    let mgr = manager(20);
    let room = start_and_pick(&mgr, 5);
    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(st.seats[st.turn].hand.len(), 6);
    assert!(st.history.iter().any(|e| e.text.contains("准备阶段")));
    assert!(st.history.iter().any(|e| e.text.contains("摸牌阶段")));
    let cur = st.seats[st.turn].player_id.clone();
    let acts = mgr.legal_actions(&room, &cur).unwrap();
    assert!(acts.iter().any(|a| a.id == "end_play"));
}

#[test]
fn sha_damages_target_without_shan() {
    let mgr = manager(21);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let target = attack_target(&st, cur);
    let target_pid = st.seats[target].player_id.clone();
    let hp_before = st.seats[target].hp;

    let play = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .into_iter()
        .find(|a| a.id.starts_with("play:"))
        .expect("应有可用的杀")
        .id;
    let res = mgr.execute_action(&room, &cur_pid, &play).unwrap();
    assert_eq!(
        state_of(&mgr, &room)
            .pending
            .as_ref()
            .map(|p| p.kind.as_str()),
        Some("target")
    );
    assert!(res.message.contains("目标"));

    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 目标无手牌、无装备/技能替代 → 引擎自动放弃响应，不再产生 respond pending。
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "无手牌目标不应被询问");
    assert_eq!(st.seats[target].hp, hp_before - 1);
    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert!(st.history.iter().any(|e| e.text.contains("受到 1 点")));
    st.check_conservation().unwrap();
}

#[test]
fn shan_negates_sha() {
    let mgr = manager(22);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = attack_target(st, cur);
        give(st, target, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let target = attack_target(&st, cur);
    let target_pid = st.seats[target].player_id.clone();
    let hp_before = st.seats[target].hp;
    let shan = st.seats[target].hand[0].clone();

    let play = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .into_iter()
        .find(|a| a.id.starts_with("play:"))
        .unwrap()
        .id;
    mgr.execute_action(&room, &cur_pid, &play).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp_before);
    assert!(st.pending.is_none());
    assert!(st.deck.discard.contains(&shan));
    st.check_conservation().unwrap();
}

#[test]
fn sha_once_per_play_phase_unless_zhuge() {
    let mgr = manager(23);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let target_pid = st.seats[attack_target(&st, cur)].player_id.clone();
    let play = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .into_iter()
        .find(|a| a.id.starts_with("play:"))
        .unwrap()
        .id;
    mgr.execute_action(&room, &cur_pid, &play).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id.starts_with("play:")),
        "本回合不应再有【杀】：{acts:?}"
    );

    edit(&mgr, &room, |st| {
        let cur = st.turn;
        give(st, cur, "zhuge_liannu");
    });
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    let equip = acts
        .iter()
        .find(|a| a.id.starts_with("play:"))
        .expect("应能装备诸葛连弩")
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &equip).unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(acts.iter().any(|a| a.id.starts_with("play:")));
    let st = state_of(&mgr, &room);
    assert!(st.has_equip(st.turn, "zhuge_liannu"));
}

#[test]
fn equip_replaces_same_slot_and_discards_old() {
    let mgr = manager(24);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "qinggang_jian");
        give(st, cur, "qinglong_yanyue");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let first = st.seats[cur].hand[0].clone();
    let second = st.seats[cur].hand[1].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{first}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("play:{second}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].equips.get(&EquipSlot::Weapon),
        Some(&Some(second.clone()))
    );
    assert!(st.deck.discard.contains(&first));
    st.check_conservation().unwrap();
}

#[test]
fn distance_and_horse_affect_sha_targets() {
    let mgr = manager(25);
    let room = start_and_pick(&mgr, 5);
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let others: Vec<usize> = st
        .alive_indices()
        .into_iter()
        .filter(|i| *i != cur)
        .collect();
    assert!(st.in_attack_range(cur, others[0]));
    assert!(!st.in_attack_range(cur, others[2]), "5 人局对侧距离应为 2");
    edit(&mgr, &room, |st| {
        let id = find_card(st, "chitu");
        take_card(st, &id);
        st.seats[cur]
            .equips
            .insert(EquipSlot::HorseOffense, Some(id));
    });
    let st = state_of(&mgr, &room);
    for t in others {
        assert!(st.in_attack_range(cur, t), "进攻马后应能覆盖座位 {t}");
    }
}

#[test]
fn discard_phase_limits_to_hp() {
    let mgr = manager(26);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        st.seats[cur].hp = 2;
        give(st, cur, "shan");
        give(st, cur, "tao");
        give(st, cur, "tao");
        give(st, cur, "shan");
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let ids: Vec<String> = st.seats[cur].hand.clone();

    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应进入弃牌 pending");
    assert_eq!(pending.kind, "discard");
    assert_eq!(pending.data["required"], 3);
    assert_eq!(pending.data["reason"], "discard_phase");

    // 数量不足（required=3，一次只给 2 张）→ 拒绝
    assert!(mgr
        .execute_action(&room, &cur_pid, &format!("discard:{},{}", ids[0], ids[1]))
        .is_err());
    // 逐张弃也允许：先 1 张，pending 变成 required=2
    mgr.execute_action(&room, &cur_pid, &format!("discard:{}", ids[0]))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("还需继续弃牌");
    assert_eq!(pending.data["required"], 2);
    mgr.execute_action(&room, &cur_pid, &format!("discard:{},{}", ids[1], ids[2]))
        .unwrap();
    // 下家若是诸葛亮，准备阶段的「观星」会先要一次排序提交。
    resolve_arrange(&mgr, &room);
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none());
    assert_eq!(st.turn_index, 1);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    // 下家原本空手牌，摸 2 张
    assert_eq!(st.seats[st.turn].hand.len(), 2);
    st.check_conservation().unwrap();
}

#[test]
fn hand_cap_forces_immediate_discard_after_draw() {
    let mgr = manager(27);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:set_max_hand_cards:5")
        .unwrap();
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(&mgr, &room);
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let pending = st.pending.clone().expect("超过持有上限应要求弃牌");
    assert_eq!(pending.kind, "discard");
    assert_eq!(pending.data["required"], 1);
    assert_eq!(pending.data["reason"], "hand_cap");
    let id = st.seats[cur].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("discard:{id}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none());
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(st.seats[cur].hand.len(), 5);
}

#[test]
fn judge_zone_is_lifo_lightning_then_lebu() {
    let mgr = manager(28);
    let room = start_and_pick(&mgr, 5);
    // 先放乐不思蜀、再放闪电 → 闪电在栈顶先判（黑桃 2 → 3 点雷电）；
    // 随后乐不思蜀判定为梅花（非红桃）→ 跳过出牌阶段。
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        let lebu = find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[cur].judge_zone.push(lebu);
        let lightning = find_card(st, "shandian");
        take_card(st, &lightning);
        st.seats[cur].judge_zone.push(lightning);
        let spade2 = find_card_where(st, |c| c.suit == Suit::Spade && c.rank == 2);
        let club3 = find_card_where(st, |c| c.suit == Suit::Club && c.rank == 3);
        // 先 push 乐的判定牌、再 push 闪电的判定牌 → pop 顺序 = 闪电、乐（LIFO）
        for id in [club3, spade2] {
            take_card(st, &id);
            st.deck.draw.push(id);
        }
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    // 出牌阶段被跳过 → 回合已移交，受伤的是上一位玩家，按体力差定位。
    let damaged: Vec<usize> = st
        .seats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.hp < s.max_hp)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(damaged.len(), 1, "应只有一名玩家受到闪电伤害");
    let hurt = damaged[0];
    assert_eq!(st.seats[hurt].hp, st.seats[hurt].max_hp - 3);
    assert!(st.seats[hurt].judge_zone.is_empty(), "判定区应清空");
    let judge_logs: Vec<&str> = st
        .history
        .iter()
        .filter(|e| e.text.starts_with("判定【"))
        .map(|e| e.text.as_str())
        .collect();
    assert!(judge_logs.len() >= 2);
    assert!(
        judge_logs[0].contains("闪电"),
        "应先判闪电（LIFO）：{judge_logs:?}"
    );
    assert!(judge_logs[1].contains("乐不思蜀"));
    // 乐不思蜀生效 → 出牌阶段被跳过，回合已移交下一位玩家
    assert_eq!(st.turn_index, 1);
    st.check_conservation().unwrap();
}

#[test]
fn lightning_moves_to_next_when_missed() {
    let mgr = manager(29);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        let lightning = find_card(st, "shandian");
        take_card(st, &lightning);
        st.seats[cur].judge_zone.push(lightning);
        let heart5 = find_card_where(st, |c| c.suit == Suit::Heart && c.rank == 5);
        take_card(st, &heart5);
        st.deck.draw.push(heart5);
        st.turn_phase = TurnPhase::Prepare;
        drive_state(st);
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    assert!(st.seats[cur].judge_zone.is_empty());
    let next = st.seats_from_left_of(cur)[0];
    assert_eq!(st.seats[next].judge_zone.len(), 1);
    assert_eq!(st.seats[next].judge_zone[0], find_card(&st, "shandian"));
    st.check_conservation().unwrap();
}

#[test]
fn dying_player_can_be_rescued_by_tao() {
    let mgr = manager(30);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = attack_target(st, cur);
        st.seats[target].hp = 1;
        let rescuer = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur && *i != target)
            .unwrap();
        give(st, rescuer, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let target = attack_target(&st, cur);
    let target_pid = st.seats[target].player_id.clone();
    let play = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .into_iter()
        .find(|a| a.id.starts_with("play:"))
        .unwrap()
        .id;
    mgr.execute_action(&room, &cur_pid, &play).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    // 只询问有手牌的存活玩家（无手牌者自动跳过）；遇到可救援者就出桃。
    let mut rescued = false;
    for _ in 0..20 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "dying" {
            break;
        }
        if let Some(tao) = p.options.iter().find(|o| o.id.starts_with("respond:")) {
            mgr.execute_action(&room, &p.actor, &tao.id).unwrap();
            rescued = true;
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    assert!(rescued, "应有玩家用桃救援");

    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Turn);
    assert!(st.pending.is_none());
    assert_eq!(st.seats[target].hp, 1);
    assert!(st.seats[target].alive);
    st.check_conservation().unwrap();
}

#[test]
fn player_dies_when_nobody_saves() {
    let mgr = manager(31);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        let target = attack_target(st, cur);
        st.seats[target].hp = 1;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let target = attack_target(&st, cur);
    let target_pid = st.seats[target].player_id.clone();
    let play = mgr
        .legal_actions(&room, &cur_pid)
        .unwrap()
        .into_iter()
        .find(|a| a.id.starts_with("play:"))
        .unwrap()
        .id;
    mgr.execute_action(&room, &cur_pid, &play).unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    // 全场无手牌：闪/求桃询问全部自动跳过，直接死亡。
    let st = state_of(&mgr, &room);
    assert!(!st.seats[target].alive, "无人救援应死亡");
    assert!(st.pending.is_none());
    let public = public_view(&mgr, &room);
    let seat = &public["public_board"]["seats"][target];
    assert!(seat["identity"].is_string(), "死亡后应公开身份");
    assert_eq!(seat["hand_count"], 0);
    st.check_conservation().unwrap();
}

#[test]
fn tao_heals_self_only_when_wounded() {
    let mgr = manager(32);
    let room = start_and_pick(&mgr, 5);
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "tao");
    });
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(!acts.iter().any(|a| a.id.starts_with("play:")));
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        st.seats[cur].hp -= 1;
    });
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    let play = acts
        .iter()
        .find(|a| a.id.starts_with("play:"))
        .expect("受伤后可使用桃")
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &play).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, st.seats[cur].max_hp);
    st.check_conservation().unwrap();
}

#[test]
fn shan_cannot_be_played_proactively() {
    let mgr = manager(33);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = st.seats[cur].player_id.clone();
    let shan = st.seats[cur].hand[0].clone();
    assert!(mgr
        .execute_action(&room, &cur_pid, &format!("play:{shan}"))
        .is_err());
}

#[test]
fn late_binding_public_view_keeps_turn_state() {
    let mgr = manager(34);
    let room = start_and_pick(&mgr, 5);
    let public = public_view(&mgr, &room);
    assert_eq!(public["public_board"]["turn"]["phase"], "play");
    assert!(public["public_board"]["history"].as_array().unwrap().len() > 2);
    let pv = player_view(
        &mgr,
        &room,
        public["public_board"]["turn"]["player_id"]
            .as_str()
            .unwrap(),
    );
    assert!(pv["player_area"]["is_current"].as_bool().unwrap());
}
