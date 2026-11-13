//! P4.1：改判窗口（鬼才）+ 空城（锁定技目标过滤）。
//!
//! 口径：`development-plan.md`「P4 开工口径」#4（判定牌亮出后自动弹 pending；询问顺序从判定者
//! 左手侧开始、仅有改判技能者被询问；可打出改判牌或 `pass`）与 `skills.md` §4（空城）。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, drive_state_inject, edit, give, manager, state_of};
use crate::state::*;
use serde_json::json;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

/// 取一张满足条件且不在任何玩家手牌里的牌（避免把别人手里的牌抽走）。
fn free_card_where(st: &SgsState, f: impl Fn(&CardInst) -> bool) -> String {
    let in_any_hand = |id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    st.cards
        .iter()
        .find(|(id, c)| f(c) && !in_any_hand(id))
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("没有可用的牌"))
}

/// 把判定牌放到牌堆顶（`pop()` 顺序 = 最后 push 的先出）。
fn push_judge_card(st: &mut SgsState, id: &str) {
    crate::tests::take_card(st, id);
    st.deck.draw.push(id.to_string());
}

/// 布置一次「乐不思蜀」判定：判定牌为梅花（非红桃，会跳过出牌阶段）。
/// 返回 (判定者, 司马懿座位, 判定牌 id)。
fn setup_lebu_judge(st: &mut SgsState, put_lebu: bool) -> (usize, usize, String) {
    clear_hands(st);
    let owner = st.turn;
    // 去掉准备阶段技能干扰（关羽无已实现技能）。
    set_general(st, owner, "sgs_hero_2");
    let simayi = st.seats_from_left_of(owner)[1];
    set_general(st, simayi, "sgs_hero_16");
    if put_lebu {
        let lebu = super::find_card(st, "lebu_sishu");
        crate::tests::take_card(st, &lebu);
        st.seats[owner].judge_zone.push(lebu);
    }
    let judge_card = free_card_where(st, |c| c.suit == Suit::Club);
    push_judge_card(st, &judge_card);
    st.turn_phase = TurnPhase::Prepare;
    st.turn_counters.clear();
    (owner, simayi, judge_card)
}

#[test]
fn guicai_rewrites_judge_card_and_cancels_lebu() {
    let mgr = manager(700);
    let room = start_and_pick(&mgr, 5);
    let mut owner = 0;
    let mut simayi = 0;
    let mut judge_card = String::new();
    let mut heart = String::new();
    edit(&mgr, &room, |st| {
        (owner, simayi, judge_card) = setup_lebu_judge(st, true);
        heart = free_card_where(st, |c| c.suit == Suit::Heart);
        crate::tests::take_card(st, &heart);
        st.seats[simayi].hand.push(heart.clone());
        let inject = format!("{}|judge_rewrite=respond:{}", pid(st, simayi), heart);
        drive_state_inject(st, &mut vec![inject]);
    });

    let st = state_of(&mgr, &room);
    assert_eq!(
        st.turn_phase,
        TurnPhase::Play,
        "改判为红桃后乐不思蜀不生效，应正常进入出牌阶段"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("鬼才")),
        "应有鬼才改判历史：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert!(st.seats[owner].judge_zone.is_empty(), "乐不思蜀应已结算");
    assert!(!st.seats[simayi].hand.contains(&heart), "改判牌应已打出");
    let zone = st.special_zones.get("judge").expect("判定公开区");
    assert_eq!(
        zone.payload["judge_card"]["id"],
        json!(heart),
        "公开区判定牌应为改判后的牌"
    );
    assert_ne!(
        zone.payload["judge_card"]["id"],
        json!(judge_card),
        "公开区不应再显示原判定牌"
    );
    st.check_conservation().unwrap();
}

#[test]
fn guicai_pass_keeps_original_judge_card() {
    let mgr = manager(701);
    let room = start_and_pick(&mgr, 5);
    let mut heart = String::new();
    let mut simayi = 0;
    edit(&mgr, &room, |st| {
        let (_owner, seat, _card) = setup_lebu_judge(st, true);
        simayi = seat;
        heart = free_card_where(st, |c| c.suit == Suit::Heart);
        crate::tests::take_card(st, &heart);
        st.seats[simayi].hand.push(heart.clone());
        let inject = format!("{}|judge_rewrite=pass", pid(st, simayi));
        drive_state_inject(st, &mut vec![inject]);
    });

    let st = state_of(&mgr, &room);
    assert!(
        !st.history.iter().any(|e| e.text.contains("鬼才")),
        "放弃改判不应写鬼才历史"
    );
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("非红桃，跳过出牌阶段")),
        "原判定（梅花）应生效：{:?}",
        st.history.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert!(
        st.seats[simayi].hand.contains(&heart),
        "放弃改判后手牌应保留"
    );
    // 原判定生效 → 跳过出牌阶段 → 回合结束（判定公开区随 `turn_end` 清理）。
    assert!(
        st.special_zones.get("judge").is_none(),
        "回合结束后判定区应清理"
    );
    assert!(st.turn_index >= 1, "跳过出牌阶段后应已移交回合");
    st.check_conservation().unwrap();
}

#[test]
fn guicai_asks_from_left_of_judge_and_owner_last() {
    let mgr = manager(702);
    let room = start_and_pick(&mgr, 5);
    let mut owner = 0;
    let mut other = 0;
    let mut other_card = String::new();
    let mut owner_card = String::new();
    edit(&mgr, &room, |st| {
        let (o, _s, _c) = setup_lebu_judge(st, true);
        owner = o;
        // 判定者本人也持有鬼才：应最后被询问（改判结果以他为准）。
        set_general(st, owner, "sgs_hero_16");
        other = st.seats_from_left_of(owner)[0];
        set_general(st, other, "sgs_hero_16");
        other_card = free_card_where(st, |c| c.suit == Suit::Spade);
        crate::tests::take_card(st, &other_card);
        st.seats[other].hand.push(other_card.clone());
        owner_card = free_card_where(st, |c| c.suit == Suit::Heart);
        crate::tests::take_card(st, &owner_card);
        st.seats[owner].hand.push(owner_card.clone());
        let inject = vec![
            format!("{}|judge_rewrite=respond:{}", pid(st, other), other_card),
            format!("{}|judge_rewrite=respond:{}", pid(st, owner), owner_card),
        ];
        drive_state_inject(st, &mut vec![inject].concat());
    });

    let st = state_of(&mgr, &room);
    let zone = st.special_zones.get("judge").expect("判定公开区");
    assert_eq!(
        zone.payload["judge_card"]["id"],
        json!(owner_card),
        "判定者应最后被询问：最终判定牌为判定者打出的牌"
    );
    assert!(!st.seats[owner].hand.contains(&owner_card));
    assert!(!st.seats[other].hand.contains(&other_card));
    // 两张改判牌都在弃牌堆（原判定牌 + 被覆盖的牌），新判定牌已结算。
    let discarded = |id: &str| st.deck.discard.iter().any(|c| c == id);
    assert!(discarded(&other_card), "被覆盖的判定牌应进弃牌堆");
    st.check_conservation().unwrap();
}

#[test]
fn kongcheng_blocks_sha_target() {
    let mgr = manager(703);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut kc = 0;
    let mut sha = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        kc = st.seats_from_left_of(cur)[0];
        set_general(st, kc, "sgs_hero_4"); // 诸葛亮：空城
        sha = give(st, cur, "sha");
    });

    let st = state_of(&mgr, &room);
    let kc_pid = pid(&st, kc);
    let cur_pid = pid(&st, cur);
    assert!(
        !crate::turn::sha_targets(&st, cur).contains(&kc),
        "空城（无手牌）不应出现在【杀】目标候选"
    );
    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("target:{kc_pid}")),
        "目标 pending 不应包含空城角色：{acts:?}"
    );
    assert!(
        mgr.execute_action(&room, &cur_pid, &format!("target:{kc_pid}"))
            .is_err(),
        "直接指定空城角色为目标应被拒绝"
    );
    // 选择其他合法目标完成结算，牌数守恒。
    let other = acts
        .iter()
        .find(|a| a.id.starts_with("target:"))
        .expect("应有其他合法目标")
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &other).unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    state_of(&mgr, &room).check_conservation().unwrap();
}

#[test]
fn kongcheng_applies_only_without_hand_cards() {
    let mgr = manager(704);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut kc = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        kc = st.seats_from_left_of(cur)[0];
        set_general(st, kc, "sgs_hero_4");
        give(st, cur, "sha");
        // 空城角色有手牌 → 可被指定。
        give(st, kc, "shan");
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::sha_targets(&st, cur).contains(&kc),
        "有手牌时不受空城影响"
    );
}

#[test]
fn kongcheng_blocks_juedou_target() {
    let mgr = manager(705);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut kc = 0;
    let mut juedou = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        kc = st.seats_from_left_of(cur)[0];
        set_general(st, kc, "sgs_hero_4");
        juedou = give(st, cur, "juedou");
    });
    let st = state_of(&mgr, &room);
    let kc_pid = pid(&st, kc);
    let cur_pid = pid(&st, cur);
    mgr.execute_action(&room, &cur_pid, &format!("play:{juedou}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("target:{kc_pid}")),
        "决斗目标候选不应包含空城角色：{acts:?}"
    );
    // 选其他目标并结算（目标不出杀 → 受伤）。
    let other = acts
        .iter()
        .find(|a| a.id.starts_with("target:"))
        .expect("应有其他合法目标")
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &other).unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    state_of(&mgr, &room).check_conservation().unwrap();
}

#[test]
fn kongcheng_blocks_jiedao_kill_target() {
    let mgr = manager(706);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut holder = 0;
    let mut kc = 0;
    let mut jiedao = String::new();
    let mut weapon = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        holder = st.seats_from_left_of(cur)[0];
        kc = st.seats_from_left_of(cur)[1];
        set_general(st, kc, "sgs_hero_4");
        jiedao = give(st, cur, "jiedao_sharen");
        weapon = give(st, holder, "qinglong_yanyue");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let holder_pid = pid(&st, holder);
    let kc_pid = pid(&st, kc);
    // 装备武器（出牌阶段：holder 不是当前回合玩家，直接改写状态更简单）。
    edit(&mgr, &room, |st| {
        crate::tests::take_card(st, &weapon);
        st.seats[holder]
            .equips
            .insert(EquipSlot::Weapon, Some(weapon.clone()));
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{jiedao}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{holder_pid}"))
        .unwrap();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    assert!(
        !acts.iter().any(|a| a.id == format!("target:{kc_pid}")),
        "借刀杀人的杀目标不应包含空城角色：{acts:?}"
    );
    assert!(mgr
        .execute_action(&room, &cur_pid, &format!("target:{kc_pid}"))
        .is_err());
    let other = acts
        .iter()
        .find(|a| a.id.starts_with("target:"))
        .expect("应有其他合法目标")
        .id
        .clone();
    mgr.execute_action(&room, &cur_pid, &other).unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    state_of(&mgr, &room).check_conservation().unwrap();
}

/// 空城判定牌过滤不应影响非【杀】/【决斗】锦囊（南蛮入侵仍可指定空城角色）。
#[test]
fn kongcheng_does_not_block_other_tricks() {
    let mgr = manager(707);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    let mut kc = 0;
    let mut nanman = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        kc = st.seats_from_left_of(cur)[0];
        set_general(st, kc, "sgs_hero_4");
        nanman = give(st, cur, "nanman_ruqin");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, cur);
    let kc_hp = st.seats[kc].hp;
    mgr.execute_action(&room, &cur_pid, &format!("play:{nanman}"))
        .unwrap();
    drive_mgr_inject(&mgr, &room, &mut Vec::new());
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[kc].hp,
        kc_hp - 1,
        "空城不阻挡南蛮入侵（未出杀则受伤）"
    );
    st.check_conservation().unwrap();
}
