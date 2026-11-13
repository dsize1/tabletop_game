//! P4.4.1 林包用例（二）：祝融 / 孟获 / 贾诩 / 鲁肃。

use super::p24::{set_general, start_and_pick};
use super::{
    clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, state_of, take_card,
};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

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

fn top_card(st: &mut SgsState, filter: impl Fn(&CardInst) -> bool) -> String {
    let claimed = |id: &str| {
        st.deck.draw.iter().any(|c| c == id)
            || st.deck.processing.iter().any(|c| c == id)
            || st.seats.iter().any(|s| {
                s.hand.iter().any(|c| c == id)
                    || s.marks.values().flatten().any(|c| c == id)
                    || s.judge_zone.iter().any(|c| c == id)
                    || s.equips.values().flatten().any(|c| c == id)
            })
    };
    let id = st
        .cards
        .iter()
        .find(|(id, c)| filter(c) && !claimed(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"));
    take_card(st, &id);
    st.deck.draw.push(id.clone());
    id
}

// ---------------------------------------------------------------------------
// 祝融 · 巨象 / 烈刃
// ---------------------------------------------------------------------------

#[test]
fn juxiang_immune_and_gains_nanman() {
    let mgr = manager(840);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut zhurong, mut nanman) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zhurong = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, zhurong, "sgs_hero_46");
        nanman = give(st, cur, "nanman_ruqin");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp_before = st.seats[zhurong].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhurong].hp, hp_before, "巨象：南蛮入侵无效");
    assert!(
        st.seats[zhurong].hand.contains(&nanman),
        "巨象：其他角色的南蛮结算后获得之"
    );
}

#[test]
fn liewen_win_gains_target_card() {
    let mgr = manager(841);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut high, mut low) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_46");
        set_general(st, target, "sgs_hero_2");
        st.seats[target].hp = 2;
        sha = give(st, cur, "sha");
        high = free_card_where(st, |c| c.rank == 13);
        take_card(st, &high);
        st.seats[cur].hand.push(high.clone());
        low = free_card_where(st, |c| c.rank == 1);
        take_card(st, &low);
        st.seats[target].hand.push(low.clone());
        give(st, target, "tao");
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
            format!("{cur_pid}|choice=choose:{high}"),
            format!("{target_pid}|choice=choose:{low}"),
            format!("{cur_pid}|choice=pick:hand"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, 1);
    assert_eq!(st.seats[cur].hand.len(), 1, "烈刃赢：获得目标一张牌");
    assert!(st.history.iter().any(|e| e.text.contains("烈刃")));
}

#[test]
fn liewen_lose_gains_nothing() {
    let mgr = manager(842);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut sha, mut high, mut low) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_46");
        set_general(st, target, "sgs_hero_2");
        st.seats[target].hp = 2;
        sha = give(st, cur, "sha");
        low = free_card_where(st, |c| c.rank == 1);
        take_card(st, &low);
        st.seats[cur].hand.push(low.clone());
        high = free_card_where(st, |c| c.rank == 13);
        take_card(st, &high);
        st.seats[target].hand.push(high.clone());
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
            format!("{cur_pid}|choice=choose:{low}"),
            format!("{target_pid}|choice=choose:{high}"),
        ],
    );
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 0, "烈刃没赢：不获得牌");
}

// ---------------------------------------------------------------------------
// 孟获 · 祸首 / 再起
// ---------------------------------------------------------------------------

#[test]
fn huoshou_immune_to_nanman() {
    let mgr = manager(850);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut menghuo, mut nanman) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        menghuo = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, menghuo, "sgs_hero_47");
        for i in st.alive_indices() {
            if i != cur && i != menghuo {
                set_general(st, i, "sgs_hero_2");
            }
        }
        nanman = give(st, cur, "nanman_ruqin");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let hp_before = st.seats[menghuo].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[menghuo].hp, hp_before, "祸首：南蛮入侵无效");
}

#[test]
fn huoshou_becomes_damage_source() {
    let mgr = manager(851);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut menghuo) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        menghuo = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, menghuo, "sgs_hero_47");
    });
    let st = state_of(&mgr, &room);
    assert_eq!(crate::skills::huoshou_owner(&st, cur), Some(menghuo));
}

#[test]
fn zaizhi_reveals_hearts_and_heals() {
    let mgr = manager(852);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_47");
        st.seats[cur].hp = st.seats[cur].max_hp - 2;
        top_card(st, |c| c.suit == Suit::Heart);
        top_card(st, |c| c.suit == Suit::Heart);
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:zaizhi")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, st.seats[cur].max_hp, "两张红桃回复 2 点");
    assert_eq!(st.seats[cur].hand.len(), 2, "获得亮出的牌且跳过摸牌");
    assert!(st.history.iter().any(|e| e.text.contains("再起")));
}

// ---------------------------------------------------------------------------
// 贾诩 · 完杀 / 乱武 / 帷幕
// ---------------------------------------------------------------------------

#[test]
fn wansha_blocks_others_tao() {
    let mgr = manager(860);
    let room = start_and_pick(&mgr, 5);
    let (mut jiaxu, mut victim, mut helper, mut tao, mut pid_helper) =
        (0, 0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        jiaxu = st.turn;
        victim = st.seats_from_left_of(jiaxu)[0];
        helper = st.seats_from_left_of(jiaxu)[1];
        set_general(st, jiaxu, "sgs_hero_48");
        set_general(st, victim, "sgs_hero_2");
        set_general(st, helper, "sgs_hero_22"); // 华佗（可急救）
        st.seats[victim].hp = 1;
        tao = give(st, helper, "tao");
        pid_helper = pid(st, helper);
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(jiaxu),
            crate::resolve::DamageType::Normal,
            None,
        );
        crate::resolve::advance(st, &mut Vec::new()).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert!(st.pending.as_ref().is_some_and(|p| p.actor == pid_helper));
    let options = &st.pending.as_ref().unwrap().options;
    assert!(
        !options.iter().any(|o| o.id == format!("respond:{tao}")),
        "完杀：非濒死角色不能使用【桃】"
    );
    assert!(options.iter().any(|o| o.id == "pass"));
    let _ = jiaxu;
}

#[test]
fn luanwu_forces_each_other_lose_hp() {
    let mgr = manager(861);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut others) = (0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_48");
        others = st.seats_from_left_of(cur);
        for i in &others {
            set_general(st, *i, "sgs_hero_2");
            st.seats[*i].hp = 4;
        }
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let mut inject: Vec<String> = others
        .iter()
        .map(|i| format!("{}|choice=choose:luanwu:lose", pid(&st, *i)))
        .collect();
    mgr.execute_action(&room, &cur_pid, "skill:luanwu").unwrap();
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    for i in &others {
        assert_eq!(st.seats[*i].hp, 3, "乱武：选择失去 1 点体力");
    }
    assert!(st.seats[cur].flags.contains_key("luanwu_used"));
}

#[test]
fn weimu_blocks_black_trick_target() {
    let mgr = manager(862);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut jiaxu, mut guohe) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        jiaxu = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_2");
        set_general(st, jiaxu, "sgs_hero_48");
        guohe = free_card_where(st, |c| {
            c.def == "guohe_chaiqiao" && crate::skills::is_black(c.suit)
        });
        take_card(st, &guohe);
        st.seats[cur].hand.push(guohe.clone());
        give(st, jiaxu, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, &format!("play:{guohe}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let jiaxu_pid = pid(&st, jiaxu);
    assert!(
        !st.pending.as_ref().is_some_and(|p| p
            .options
            .iter()
            .any(|o| o.id == format!("target:{jiaxu_pid}"))),
        "帷幕：不能成为黑色锦囊牌目标"
    );
}

// ---------------------------------------------------------------------------
// 鲁肃 · 好施 / 缔盟
// ---------------------------------------------------------------------------

#[test]
fn haoshi_gives_half_hand_to_least_hand_target() {
    let mgr = manager(870);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut hand) = (0, 0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        target = st.seats_from_left_of(cur)[0];
        set_general(st, cur, "sgs_hero_49");
        set_general(st, target, "sgs_hero_2");
        for _ in 0..3 {
            hand.push(give(st, cur, "sha"));
        }
        let pid_c = pid(st, cur);
        let pid_t = pid(st, target);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let mut inject = vec![format!("{pid_c}|choice=choose:haoshi")];
        for id in hand.iter().take(3) {
            inject.push(format!("{pid_c}|choose_many=choose:{id}"));
        }
        inject.push(format!("{pid_c}|choose_many=confirm"));
        inject.push(format!("{pid_c}|target=target:{pid_t}"));
        drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 4, "7 张手牌交出一半（3 张）");
    assert_eq!(st.seats[target].hand.len(), 3);
    assert!(st.history.iter().any(|e| e.text.contains("好施")));
}

#[test]
fn haoshi_no_give_when_hand_small() {
    let mgr = manager(871);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_49");
        let pid_c = pid(st, cur);
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        drive_state_inject(st, &mut vec![format!("{pid_c}|choice=choose:haoshi")]);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        4,
        "2 + 好施 2 = 4，未超过 5 不交牌"
    );
    assert!(!st.history.iter().any(|e| e.text.contains("交给")));
}

#[test]
fn dimeng_swaps_two_players_hands() {
    let mgr = manager(872);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut a, mut b, mut discard) = (0, 0, 0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        a = st.seats_from_left_of(cur)[0];
        b = st.seats_from_left_of(cur)[1];
        set_general(st, cur, "sgs_hero_49");
        set_general(st, a, "sgs_hero_2");
        set_general(st, b, "sgs_hero_15");
        for _ in 0..3 {
            give(st, a, "sha");
        }
        give(st, b, "shan");
        for _ in 0..2 {
            discard.push(give(st, cur, "tao"));
        }
    });
    let st = state_of(&mgr, &room);
    let (pid_c, pid_a, pid_b) = (pid(&st, cur), pid(&st, a), pid(&st, b));
    let mut inject = vec![
        format!("{pid_c}|target=target:{pid_a}"),
        format!("{pid_c}|target=target:{pid_b}"),
    ];
    for id in &discard {
        inject.push(format!("{pid_c}|choose_many=choose:{id}"));
    }
    inject.push(format!("{pid_c}|choose_many=confirm"));
    mgr.execute_action(&room, &pid_c, "skill:dimeng").unwrap();
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[a].hand.len(), 1, "交换后 a 得到 b 的 1 张");
    assert_eq!(st.seats[b].hand.len(), 3, "交换后 b 得到 a 的 3 张");
    assert!(st.history.iter().any(|e| e.text.contains("缔盟")));
}
