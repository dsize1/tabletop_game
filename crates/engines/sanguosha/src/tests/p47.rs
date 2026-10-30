//! P4.1-D 回归：跨技能交互 + 补齐「每武将 ≥3 用例」。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, give, manager, state_of, take_card};
use crate::state::*;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

fn in_hand(st: &SgsState, idx: usize, code: &str) -> String {
    st.seats[idx]
        .hand
        .iter()
        .find(|id| st.cards.get(*id).map(|c| c.def.as_str()) == Some(code))
        .cloned()
        .unwrap_or_else(|| panic!("{} 手里没有 {code}", st.seats[idx].player_id))
}

fn free_card(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let in_hand = |id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !in_hand(id))
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌")
}

fn pass_wuxie(mgr: &game_engine_core::GameManager, room: &str) {
    for _ in 0..20 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending else { return };
        if p.kind != "wuxie" {
            return;
        }
        mgr.execute_action(room, &p.actor, "pass").unwrap();
    }
    panic!("无懈窗口未收敛");
}

/// 赵云：龙胆把【闪】当【杀】响应决斗。
#[test]
fn longdan_responds_duel_with_shan_as_sha() {
    let mgr = manager(1300);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut zhaoyun = 0;
    let mut juedou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        zhaoyun = st.seats_from_left_of(cur)[0];
        set_general(st, zhaoyun, "sgs_hero_5"); // 赵云：龙胆
        juedou = give(st, cur, "juedou");
        give(st, zhaoyun, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let zy_pid = pid(&st, zhaoyun);
    let cur_hp = st.seats[cur].hp;
    let zy_hp = st.seats[zhaoyun].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{juedou}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{zy_pid}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    let st = state_of(&mgr, &room);
    let shan = in_hand(&st, zhaoyun, "shan");
    mgr.execute_action(&room, &zy_pid, &format!("respond:{shan}"))
        .unwrap();
    // 赵云出杀后轮到周瑜方（无杀）→ 自动放弃并受伤
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhaoyun].hp, zy_hp, "赵云未受伤");
    assert_eq!(st.seats[cur].hp, cur_hp - 1, "决斗发起方无杀而受伤");
    assert!(st.history.iter().any(|e| e.text.contains("龙胆")));
    st.check_conservation().unwrap();
}

/// 马超：马术使距离 2 的顺手牵羊合法。
#[test]
fn mashu_lets_shunshou_reach_distance_two() {
    let mgr = manager(1301);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut far = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_6"); // 马超：马术
        far = (cur + 2) % st.seats.len();
        set_general(st, far, "sgs_hero_2");
        give(st, far, "shan");
        give(st, cur, "shunshou_qianyang");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let far_pid = pid(&st, far);
    assert_eq!(st.seat_distance(cur, far), 2);
    assert!(
        crate::turn::trick_distance_ok(&st, cur, far),
        "马术：距离 2 的顺手应合法"
    );
    let shunshou = in_hand(&st, cur, "shunshou_qianyang");
    mgr.execute_action(&room, &cur_pid, &format!("play:{shunshou}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        acts.iter().any(|a| a.id == format!("target:{far_pid}")),
        "顺手目标应包含距离 2 的角色：{acts:?}"
    );
}

/// 黄盖：苦肉摸牌后触发持有上限弃牌。
#[test]
fn kurou_respects_hand_cap() {
    let mgr = manager(1302);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_11"); // 黄盖：苦肉
        st.config.max_hand_cards = 5;
        for code in ["sha", "shan", "tao", "juedou"] {
            give(st, cur, code);
        }
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, "skill:kurou").unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("超过持有上限应要求弃牌");
    assert_eq!(pending.kind, "discard");
    assert_eq!(pending.actor, cur_pid);
    assert_eq!(pending.data["reason"].as_str(), Some("hand_cap"));
    assert_eq!(st.seats[cur].hand.len(), 6, "4 + 苦肉摸 2");
    st.check_conservation().unwrap();
}

/// 周瑜：反间花色相同不造成伤害。
#[test]
fn fanjian_same_suit_no_damage() {
    let mgr = manager(1303);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut target = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_12"); // 周瑜：反间
        target = st.seats_from_left_of(cur)[0];
        set_general(st, target, "sgs_hero_2");
        give(st, cur, "wuzhong_shengyou"); // 红桃
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let target_pid = pid(&st, target);
    let hp = st.seats[target].hp;
    mgr.execute_action(&room, &cur_pid, "skill:fanjian")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:suit:heart")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "花色相同不受伤");
    assert_eq!(st.seats[target].hand.len(), 1, "目标获得反间牌");
    st.check_conservation().unwrap();
}

/// 大乔：流离目标排除【杀】使用者与空城角色。
#[test]
fn liuli_excludes_attacker_and_kongcheng() {
    let mgr = manager(1304);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut dq = 0;
    let mut kc = 0;
    let mut valid = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        dq = st.seats_from_left_of(cur)[0];
        kc = st.seats_from_left_of(cur)[1];
        valid = st.seats_from_left_of(cur)[2];
        set_general(st, dq, "sgs_hero_13"); // 大乔：流离
        set_general(st, kc, "sgs_hero_4"); // 诸葛亮：空城
        set_general(st, valid, "sgs_hero_2");
        // 给大乔武器扩大攻击范围，使 valid（距离 2）进入范围。
        let weapon = st
            .cards
            .iter()
            .find(|(_, c)| c.def == "qinglong_yanyue")
            .map(|(id, _)| id.clone())
            .expect("青龙偃月刀");
        take_card(st, &weapon);
        st.seats[dq].equips.insert(EquipSlot::Weapon, Some(weapon));
        give(st, cur, "sha");
        give(st, dq, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let dq_pid = pid(&st, dq);
    let kc_pid = pid(&st, kc);
    let valid_pid = pid(&st, valid);
    let sha = in_hand(&st, cur, "sha");
    let card = st.seats[dq].hand[0].clone();
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{dq_pid}"))
        .unwrap();
    mgr.execute_action(&room, &dq_pid, &format!("choose:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应选择转移目标");
    let ids: Vec<&str> = pending.options.iter().map(|o| o.id.as_str()).collect();
    assert!(
        ids.contains(&format!("target:{valid_pid}").as_str()),
        "{ids:?}"
    );
    assert!(
        !ids.contains(&format!("target:{cur_pid}").as_str()),
        "不能给使用者"
    );
    assert!(
        !ids.contains(&format!("target:{kc_pid}").as_str()),
        "不能给空城角色"
    );
    st.check_conservation().unwrap();
}

/// 张辽突袭拿走陆逊最后一张手牌 → 连营摸 1。
#[test]
fn lianying_when_tuxi_steals_last_card() {
    let mgr = manager(1305);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut luxun = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_18"); // 张辽：突袭
        luxun = st.seats_from_left_of(cur)[0];
        set_general(st, luxun, "sgs_hero_14"); // 陆逊：连营
        give(st, luxun, "shan");
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let cur_pid = st.seats[cur].player_id.clone();
        let luxun_pid = st.seats[luxun].player_id.clone();
        let mut inject = vec![
            format!("{cur_pid}|choice=choose:tuxi"),
            format!("{cur_pid}|target=target:{luxun_pid}"),
            format!("{cur_pid}|target=confirm"),
        ];
        super::drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("连营")),
        "连营应触发：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert_eq!(st.seats[luxun].hand.len(), 1, "陆逊失去最后手牌后摸 1");
    assert_eq!(st.seats[cur].hand.len(), 1, "张辽获得 1 张");
    st.check_conservation().unwrap();
}

/// 司马懿改判 + 郭嘉天妒：获得改判后的判定牌。
#[test]
fn guicai_rewrites_judge_then_tiandu_gains() {
    let mgr = manager(1306);
    let room = start_and_pick(&mgr, 5);
    let mut guojia = 0;
    let mut simayi = 0;
    let mut heart = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guojia = st.turn;
        set_general(st, guojia, "sgs_hero_20"); // 郭嘉：天妒
        simayi = st.seats_from_left_of(guojia)[1];
        set_general(st, simayi, "sgs_hero_16"); // 司马懿：鬼才
        let lebu = super::find_card(st, "lebu_sishu");
        take_card(st, &lebu);
        st.seats[guojia].judge_zone.push(lebu);
        // 原判定为梅花（会跳过出牌阶段）
        let club = free_card(st, |c| c.suit == Suit::Club);
        take_card(st, &club);
        st.deck.draw.push(club);
        // 司马懿改判为红桃
        heart = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &heart);
        st.seats[simayi].hand.push(heart.clone());
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        let simayi_pid = st.seats[simayi].player_id.clone();
        let mut inject = vec![format!("{simayi_pid}|judge_rewrite=respond:{heart}")];
        super::drive_state_inject(st, &mut inject);
    });
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("鬼才")));
    assert!(st.history.iter().any(|e| e.text.contains("天妒")));
    assert!(
        st.seats[guojia].hand.contains(&heart),
        "天妒应获得改判后的判定牌"
    );
    assert_eq!(st.turn_phase, TurnPhase::Play, "红桃改判后不跳过出牌");
    st.check_conservation().unwrap();
}

/// 夏侯惇刚烈：来源选择弃置两张手牌。
#[test]
fn ganglie_source_discards_two() {
    let mgr = manager(1307);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut victim = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_17"); // 夏侯惇：刚烈
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        sha = give(st, cur, "sha");
        give(st, cur, "shan");
        give(st, cur, "tao");
        let black = free_card(st, |c| c.suit == Suit::Club);
        take_card(st, &black);
        st.deck.draw.push(black);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let victim_pid = pid(&st, victim);
    let cur_hp = st.seats[cur].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.as_str()), Some("choice"));
    mgr.execute_action(&room, &cur_pid, "choose:discard")
        .unwrap();
    let st = state_of(&mgr, &room);
    let cards: Vec<String> = st.seats[cur].hand.iter().take(2).cloned().collect();
    assert_eq!(cards.len(), 2);
    for c in &cards {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{c}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 0, "弃置两张手牌");
    assert_eq!(st.seats[cur].hp, cur_hp, "选择弃牌则不受伤");
    st.check_conservation().unwrap();
}

/// 夏侯惇刚烈：红桃判定不生效。
#[test]
fn ganglie_heart_judge_does_not_trigger() {
    let mgr = manager(1308);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut victim = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        victim = st.seats_from_left_of(cur)[0];
        set_general(st, victim, "sgs_hero_17"); // 夏侯惇：刚烈
        st.seats[victim].max_hp = 5;
        st.seats[victim].hp = 5;
        sha = give(st, cur, "sha");
        give(st, cur, "shan");
        let red = free_card(st, |c| c.suit == Suit::Heart);
        take_card(st, &red);
        st.deck.draw.push(red);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let victim_pid = pid(&st, victim);
    let cur_hp = st.seats[cur].hp;
    let hand = st.seats[cur].hand.len();
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{victim_pid}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "红桃判定后无刚烈询问");
    assert_eq!(st.seats[cur].hp, cur_hp, "来源不受伤");
    assert_eq!(st.seats[cur].hand.len(), hand - 1, "只失去打出的【杀】");
    st.check_conservation().unwrap();
}

/// 甄姬倾国 + 吕布无双：黑色牌当闪也需要两张。
#[test]
fn qingguo_with_wushuang_needs_two_black_cards() {
    let mgr = manager(1309);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut zhenji = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_23"); // 吕布：无双
        zhenji = st.seats_from_left_of(cur)[0];
        set_general(st, zhenji, "sgs_hero_21"); // 甄姬：倾国
        sha = give(st, cur, "sha");
        give(st, zhenji, "wuxie_keji"); // ♠ 黑
        give(st, zhenji, "nanman_ruqin"); // ♠ 黑
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let zj_pid = pid(&st, zhenji);
    let hp = st.seats[zhenji].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{zj_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let b1 = st.seats[zhenji].hand[0].clone();
    mgr.execute_action(&room, &zj_pid, &format!("respond:{b1}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(zj_pid.as_str()),
        "无双：还需第二张【闪】"
    );
    let b2 = st.seats[zhenji].hand[0].clone();
    mgr.execute_action(&room, &zj_pid, &format!("respond:{b2}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[zhenji].hp, hp, "两张倾国闪抵消【杀】");
    assert!(st.history.iter().any(|e| e.text.contains("倾国")));
    st.check_conservation().unwrap();
}

/// 吕布无双 + 赵云龙胆：决斗中赵云需两张【闪】当【杀】。
#[test]
fn wushuang_duel_needs_two_longdan_sha() {
    let mgr = manager(1310);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut zhaoyun = 0;
    let mut juedou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_23"); // 吕布：无双
        zhaoyun = st.seats_from_left_of(cur)[0];
        set_general(st, zhaoyun, "sgs_hero_5"); // 赵云：龙胆
        juedou = give(st, cur, "juedou");
        give(st, zhaoyun, "shan");
        give(st, zhaoyun, "shan");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let zy_pid = pid(&st, zhaoyun);
    let cur_hp = st.seats[cur].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{juedou}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{zy_pid}"))
        .unwrap();
    pass_wuxie(&mgr, &room);
    let st = state_of(&mgr, &room);
    let s1 = st.seats[zhaoyun].hand[0].clone();
    mgr.execute_action(&room, &zy_pid, &format!("respond:{s1}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let s2 = st.seats[zhaoyun].hand[0].clone();
    mgr.execute_action(&room, &zy_pid, &format!("respond:{s2}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, cur_hp - 1, "吕布无杀而受伤");
    assert!(st.history.iter().any(|e| e.text.contains("龙胆")));
    st.check_conservation().unwrap();
}

/// 孙尚香枭姬 + 顺手牵羊拿走装备 → 摸 2。
#[test]
fn xiaoji_draws_when_shunshou_takes_equip() {
    let mgr = manager(1311);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut ssw = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        ssw = st.seats_from_left_of(cur)[0];
        set_general(st, ssw, "sgs_hero_25"); // 孙尚香：枭姬
        let bagua = super::find_card(st, "bagua_zhen");
        take_card(st, &bagua);
        st.seats[ssw]
            .equips
            .insert(EquipSlot::Armor, Some(bagua.clone()));
        give(st, cur, "shunshou_qianyang");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let ssw_pid = pid(&st, ssw);
    let shunshou = in_hand(&st, cur, "shunshou_qianyang");
    mgr.execute_action(&room, &cur_pid, &format!("play:{shunshou}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{ssw_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("pick_card")
    );
    mgr.execute_action(&room, &cur_pid, "pick:equip:armor")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("枭姬")),
        "枭姬应因失去装备触发：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert_eq!(st.seats[ssw].hand.len(), 2, "枭姬摸 2 张");
    st.check_conservation().unwrap();
}
