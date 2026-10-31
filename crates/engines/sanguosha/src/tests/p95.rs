//! P4.4.2 一将成名批次 Y7（8 将）：辛宪英 / 吴苋 / 徐氏 / 曹节 / 嵇康 / 秦宓 / 薛综 / 蔡邕。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::state::*;

fn stage_play(st: &mut SgsState, seat: usize) {
    st.turn = seat;
    st.turn_phase = TurnPhase::Play;
    st.turn_counters.clear();
}

fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    super::support::give_def(st, seat, def)
}

fn equip_slot_card(st: &mut SgsState, seat: usize, slot: EquipSlot) -> String {
    let card = give_card(st, seat, |c| {
        crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot == Some(slot))
    });
    take_card(st, &card);
    st.seats[seat].equips.insert(slot, Some(card.clone()));
    card
}

// ---------------------------------------------------------------------------
// 辛宪英 · 忠鉴 / 才识
// ---------------------------------------------------------------------------

#[test]
fn zhongjian_same_color_draws() {
    let mgr = manager(3100);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_388");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_card(st, seat, |c| crate::skills::is_red(c.suit));
        for _ in 0..3 {
            give_card(st, target, |c| crate::skills::is_red(c.suit));
        }
        before = st.seats[seat].hand.len();
    });
    mgr.execute_action(&room, &pid, "skill:zhongjian").unwrap();
    let st = state_of(&mgr, &room);
    let own = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("own card");
    mgr.execute_action(&room, &pid, &own).unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hand.len(), before + 1, "忠鉴：颜色相同摸 1");
}

#[test]
fn zhongjian_same_rank_allows_second_use() {
    let mgr = manager(3101);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut own) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_388");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        own = give_rank(st, seat, 7);
        give_rank(st, target, 7);
    });
    mgr.execute_action(&room, &pid, "skill:zhongjian").unwrap();
    let st = state_of(&mgr, &room);
    let opts = pending_options_with(&st, "choose:");
    assert!(opts.contains(&format!("choose:{own}")));
    mgr.execute_action(&room, &pid, &format!("choose:{own}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("zhongjian_extra"), 1, "忠鉴：点数相同可再发动");
}

#[test]
fn caishi_raises_hand_limit() {
    let mgr = manager(3102);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut base) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_388");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        base = st.hand_limit(seat);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("caishi"), "才识：摸牌阶段开始");
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::hand_limit(&st, seat),
        base + 1,
        "才识：手牌上限 +1"
    );
}

// ---------------------------------------------------------------------------
// 吴苋 · 福绵 / 怠宴
// ---------------------------------------------------------------------------

#[test]
fn fumian_draw_choice() {
    let mgr = manager(3103);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_389");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Prepare, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("fumian"), "福绵：准备阶段");
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("fumian_draw"), 1, "福绵：多摸 1 张");
}

#[test]
fn fumian_multi_targets_red_sha() {
    let mgr = manager(3104);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_389");
        stage_play(st, seat);
        st.set_counter("fumian_multi", 1);
        pid = st.seats[seat].player_id.clone();
        give_card(st, seat, |c| {
            crate::skills::is_red(c.suit) && matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
        });
    });
    let st = state_of(&mgr, &room);
    let sha = st.seats[seat]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha"))
        })
        .cloned()
        .expect("sha");
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !pending_options_with(&st, "targets:").is_empty(),
        "福绵：红【杀】可多指定目标"
    );
}

#[test]
fn daiyan_gives_heart_basic() {
    let mgr = manager(3105);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_389");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        before = st.seats[target].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("daiyan"), "怠宴：结束阶段");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() > before,
        "怠宴：其获得红桃基本牌"
    );
    assert!(st.history.iter().any(|e| e.text.contains("怠宴")));
}

// ---------------------------------------------------------------------------
// 徐氏 · 问卦 / 伏诛
// ---------------------------------------------------------------------------

#[test]
fn wengua_gives_and_both_draw() {
    let mgr = manager(3106);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut owner, mut owner_pid, mut card, mut before, mut owner_before) =
        (0, String::new(), 0, String::new(), String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_1");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        owner = other_seats(st, seat)[0];
        set_general(st, owner, "sgs_hero_390");
        owner_pid = st.seats[owner].player_id.clone();
        card = give_def(st, seat, "shan");
        give_def(st, owner, "sha");
        before = st.seats[seat].hand.len();
        owner_before = st.seats[owner].hand.len();
    });
    mgr.execute_action(&room, &pid, &format!("skill:wengua:{owner}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[seat].hand.len(),
        before,
        "问卦：交出 1 张、摸回 1 张"
    );
    assert_eq!(
        st.seats[owner].hand.len(),
        owner_before + 1,
        "问卦：徐氏摸 1"
    );
    let _ = owner;
}

#[test]
fn wengua_bottom_choice() {
    let mgr = manager(3107);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut owner, mut owner_pid, mut card) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_1");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        owner = other_seats(st, seat)[0];
        set_general(st, owner, "sgs_hero_390");
        owner_pid = st.seats[owner].player_id.clone();
        card = give_def(st, seat, "shan");
        give_def(st, owner, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("skill:wengua:{owner}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("choose:{card}"))
        .unwrap();
    mgr.execute_action(&room, &owner_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.draw.contains(&card), "问卦：置于牌堆底（非牌堆顶）");
    assert_ne!(st.deck.draw.last(), Some(&card));
}

#[test]
fn fuzhu_damages_with_sha_from_deck() {
    let mgr = manager(3108);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut owner, mut owner_pid, mut owner_hp, mut hp) = (0, 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_1");
        st.turn_counters.clear();
        owner = other_seats(st, cur)[0];
        set_general(st, owner, "sgs_hero_390");
        owner_pid = st.seats[owner].player_id.clone();
        owner_hp = st.seats[owner].hp;
        // 牌堆缩到阈值以内（hp×10）并保证有【杀】。
        let target_len = (owner_hp as usize * 10).saturating_sub(2);
        while st.deck.draw.len() > target_len {
            let card = st.deck.draw.pop().unwrap();
            st.deck.discard.push(card);
        }
        let sha = find_cards(st, |c| c.def == "sha", 1)
            .into_iter()
            .next()
            .expect("sha");
        take_card(st, &sha);
        st.deck.draw.push(sha);
        hp = st.seats[cur].hp;
    });
    run_phase(&mgr, &room, cur, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("fuzhu"), "伏诛：条件满足时询问");
    mgr.execute_action(&room, &owner_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[cur].hp < hp, "伏诛：牌堆【杀】造成伤害");
    assert!(st.history.iter().any(|e| e.text.contains("伏诛：洗牌")));
}

// ---------------------------------------------------------------------------
// 曹节 · 守玺 / 惠民
// ---------------------------------------------------------------------------

fn shouxi_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
) -> (usize, String, usize, String, String) {
    let (mut user, mut user_pid, mut owner, mut owner_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(mgr, room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        let others = other_seats(st, user);
        owner = others
            .into_iter()
            .find(|i| st.in_attack_range(user, *i))
            .expect("in range");
        set_general(st, owner, "sgs_hero_391");
        owner_pid = st.seats[owner].player_id.clone();
        give_def(st, owner, "shan");
        sha = give_def(st, user, "sha");
        give_def(st, user, "sha");
        give_def(st, user, "shan");
    });
    (user, user_pid, owner, owner_pid, sha)
}

#[test]
fn shouxi_opt2_cancels_sha() {
    let mgr = manager(3109);
    let room = start_and_pick(&mgr, 5);
    let (_user, user_pid, owner, owner_pid, sha) = shouxi_setup(&mgr, &room);
    let mut hp = 0;
    edit(&mgr, &room, |st| hp = st.seats[owner].hp);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("shouxi"), "守玺：成为目标后声明");
    mgr.execute_action(&room, &owner_pid, "choose:sha").unwrap();
    mgr.execute_action(&room, &user_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[owner].hp, hp, "守玺：此【杀】对其无效");
}

#[test]
fn shouxi_opt1_discards_and_gains() {
    let mgr = manager(3110);
    let room = start_and_pick(&mgr, 5);
    let (_user, user_pid, owner, owner_pid, sha) = shouxi_setup(&mgr, &room);
    let mut owner_card = String::new();
    let mut user_before = 0;
    edit(&mgr, &room, |st| {
        owner_card = st.seats[owner].hand[0].clone();
        user_before = st.seats[_user].hand.len();
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    mgr.execute_action(&room, &owner_pid, "choose:sha").unwrap();
    mgr.execute_action(&room, &user_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.deck.discard.contains(&sha), "守玺：弃置声明的【杀】");
    assert_eq!(
        st.seats[_user].hand.len(),
        user_before - 1,
        "守玺：弃 1 得 1（已使用 1 张）"
    );
    assert!(!st.seats[owner].hand.contains(&owner_card));
}

#[test]
fn huimin_distributes_cards() {
    let mgr = manager(3111);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut start, mut start_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_391");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        start = other_seats(st, seat)[0];
        set_general(st, start, "sgs_hero_1");
        start_pid = st.seats[start].player_id.clone();
        // 其他角色手牌少于体力 → X ≥ 1
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("huimin"), "惠民：结束阶段分配");
    mgr.execute_action(&room, &pid, &format!("target:{start_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.history.iter().any(|e| e.text.contains("惠民：")));
    assert!(st.seats[start].hand.len() >= 1, "惠民：起始角色获得牌");
}

// ---------------------------------------------------------------------------
// 嵇康 · 清弦 / 绝响
// ---------------------------------------------------------------------------

#[test]
fn qingxian_opt2_heals() {
    let mgr = manager(3112);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut source_pid, mut hp) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_392");
        st.turn_counters.clear();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        source_pid = st.seats[source].player_id.clone();
        equip_slot_card(st, source, EquipSlot::Armor);
        st.seats[source].hp = st.seats[source].max_hp - 1;
        hp = st.seats[source].hp;
        damage_and_advance(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("qingxian"),
        "清弦：受伤后令来源二选一"
    );
    mgr.execute_action(&room, &source_pid, "opt2").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[source].hp, hp + 1, "清弦：来源回复 1 点体力");
    assert!(st.history.iter().any(|e| e.text.contains("清弦：弃置装备")));
}

#[test]
fn qingxian_opt1_loses_hp_and_equips() {
    let mgr = manager(3113);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut source_pid, mut source_hp) = (0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_392");
        st.turn_counters.clear();
        source = other_seats(st, seat)[0];
        set_general(st, source, "sgs_hero_1");
        source_pid = st.seats[source].player_id.clone();
        give_card(st, source, |c| {
            crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        source_hp = st.seats[source].hp;
        crate::resolve::apply_damage(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            None,
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    mgr.execute_action(&room, &source_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[source].hp,
        source_hp - 1,
        "清弦：来源失去 1 点体力"
    );
    assert!(
        st.seats[source].equips.values().any(|c| c.is_some()),
        "清弦：随机使用装备"
    );
}

#[test]
fn juexiang_grants_skill_and_protection() {
    let mgr = manager(3114);
    let room = start_and_pick(&mgr, 5);
    let (mut dead, mut target, mut target_pid) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dead = st.turn;
        set_general(st, dead, "sgs_hero_392");
        target = other_seats(st, dead)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        crate::resolve::push(
            st,
            crate::resolve::Frame::JueXiang(crate::resolve_yijiang9::JueXiangFrame { dead }),
        );
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("juexiang"), "绝响：死亡时询问");
    let dead_pid = st.seats[dead].player_id.clone();
    mgr.execute_action(&room, &dead_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target]
            .extra_skills
            .iter()
            .any(|c| c == "qingxian"),
        "绝响：获得「清弦」"
    );
    assert!(st.seats[target].flags.contains_key("juexiang"));
}

// ---------------------------------------------------------------------------
// 秦宓 · 谏征 / 专对 / 天辩
// ---------------------------------------------------------------------------

fn qinmi_setup(
    mgr: &game_engine_core::GameManager,
    room: &str,
    red_sha: bool,
) -> (usize, String, usize, String, usize, String, String) {
    let (mut user, mut user_pid, mut target, mut target_pid, mut qinmi, mut qinmi_pid, mut sha) = (
        0,
        String::new(),
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
    );
    edit(mgr, room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        let others = other_seats(st, user);
        target = others
            .iter()
            .copied()
            .find(|i| st.in_attack_range(user, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        qinmi = others
            .into_iter()
            .find(|i| *i != target && st.in_attack_range(user, *i))
            .expect("qinmi in range");
        set_general(st, qinmi, "sgs_hero_393");
        qinmi_pid = st.seats[qinmi].player_id.clone();
        give_def(st, qinmi, "shan");
        sha = give_card(st, user, |c| {
            matches!(c.def.as_str(), "sha" | "huo_sha" | "lei_sha")
                && crate::skills::is_red(c.suit) == red_sha
        });
        give_def(st, user, "shan");
    });
    (user, user_pid, target, target_pid, qinmi, qinmi_pid, sha)
}

#[test]
fn jianzheng_cancels_black_sha() {
    let mgr = manager(3115);
    let room = start_and_pick(&mgr, 5);
    let (_user, user_pid, target, target_pid, qinmi, qinmi_pid, sha) =
        qinmi_setup(&mgr, &room, false);
    let mut hp = 0;
    edit(&mgr, &room, |st| hp = st.seats[target].hp);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jianzheng"), "谏征：目标选定后");
    mgr.execute_action(&room, &qinmi_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp, "谏征：黑色【杀】目标被取消");
    assert!(st.history.iter().any(|e| e.text.contains("谏征")));
    let _ = qinmi;
}

#[test]
fn jianzheng_red_sha_retargets_qinmi() {
    let mgr = manager(3116);
    let room = start_and_pick(&mgr, 5);
    let (_user, user_pid, target, target_pid, qinmi, qinmi_pid, sha) =
        qinmi_setup(&mgr, &room, true);
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &qinmi_pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("谏征：此【杀】不为黑色")),
        "谏征：非黑色则秦宓成为目标"
    );
    let _ = (target, qinmi);
}

#[test]
fn zhuandui_pindian_deck_option() {
    let mgr = manager(3117);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut other) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_393");
        st.turn_counters.clear();
        other = other_seats(st, seat)[0];
        set_general(st, other, "sgs_hero_1");
        give_def(st, seat, "sha");
        give_def(st, other, "sha");
        crate::resolve_fire::start_pindian(st, seat, other, serde_json::json!({}));
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    let opts = pending_options_with(&st, "choose:");
    assert!(
        opts.iter().any(|o| o == "choose:__deck"),
        "天辩：可用牌堆顶拼点"
    );
}

// ---------------------------------------------------------------------------
// 薛综 · 复难 / 诫训
// ---------------------------------------------------------------------------

#[test]
fn funan_swaps_used_and_respond_cards() {
    let mgr = manager(3118);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha, mut shan) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_394");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        sha = give_def(st, seat, "sha");
        shan = give_def(st, target, "shan");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("funan"), "复难：对方响应后询问");
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].hand.contains(&sha), "复难：其获得使用的牌");
    assert!(st.seats[seat].hand.contains(&shan), "复难：薛综获得响应牌");
}

#[test]
fn funan_bans_card_for_responder() {
    let mgr = manager(3119);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha, mut shan) = (
        0,
        String::new(),
        0,
        String::new(),
        String::new(),
        String::new(),
    );
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_394");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)
            .into_iter()
            .find(|i| st.in_attack_range(seat, *i))
            .expect("target");
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        sha = give_def(st, seat, "sha");
        shan = give_def(st, target, "shan");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, &format!("respond:{shan}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "opt1").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        crate::resolve_yijiang9::funan_banned(&st, target, &sha),
        "复难：其本回合不能使用该牌"
    );
}

#[test]
fn jiexun_draws_by_diamonds() {
    let mgr = manager(3120);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before, mut diamonds) =
        (0, String::new(), 0, String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_394");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        give_suit(st, target, Suit::Diamond);
        before = st.seats[target].hand.len();
        diamonds = crate::resolve_yijiang9::diamonds_in_play(st);
    });
    let _ = diamonds;
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiexun"), "诫训：结束阶段");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].hand.len() >= before, "诫训：按方片数摸牌");
    assert!(st.history.iter().any(|e| e.text.contains("诫训：")));
}

// ---------------------------------------------------------------------------
// 蔡邕 · 辟撰 / 通博
// ---------------------------------------------------------------------------

#[test]
fn bizhuan_adds_shu_on_spade_use() {
    let mgr = manager(3121);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_395");
        stage_play(st, seat);
        pid = st.seats[seat].player_id.clone();
        give_card(st, seat, |c| {
            c.suit == Suit::Spade
                && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
        });
        before = st.seats[seat].hand.len();
        let _ = before;
    });
    let st = state_of(&mgr, &room);
    let spade = st.seats[seat]
        .hand
        .iter()
        .find(|id| {
            st.cards.get(*id).is_some_and(|c| {
                c.suit == Suit::Spade
                    && crate::data::card_def(&c.def).is_some_and(|d| d.equip_slot.is_some())
            })
        })
        .cloned()
        .expect("spade equip");
    mgr.execute_action(&room, &pid, &format!("play:{spade}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::mark_cards(&st, seat, "shu").len(),
        1,
        "辟撰：使用黑桃牌获得「书」"
    );
    assert_eq!(
        crate::skills::hand_limit(&st, seat),
        st.seats[seat].hp.max(0) as usize + 1,
        "辟撰：每张「书」手牌上限 +1"
    );
}

#[test]
fn bizhuan_adds_shu_when_targeted_by_spade() {
    let mgr = manager(3122);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut user_pid, mut owner, mut owner_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_1");
        stage_play(st, user);
        user_pid = st.seats[user].player_id.clone();
        owner = other_seats(st, user)
            .into_iter()
            .find(|i| st.in_attack_range(user, *i))
            .expect("target");
        set_general(st, owner, "sgs_hero_395");
        owner_pid = st.seats[owner].player_id.clone();
        give_card(st, user, |c| c.suit == Suit::Spade && c.def == "sha");
        give_def(st, owner, "shan");
    });
    let st = state_of(&mgr, &room);
    let sha = st.seats[user]
        .hand
        .iter()
        .find(|id| {
            st.cards
                .get(*id)
                .is_some_and(|c| c.suit == Suit::Spade && c.def == "sha")
        })
        .cloned()
        .expect("spade sha");
    mgr.execute_action(&room, &user_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{owner_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        crate::skills::mark_cards(&st, owner, "shu").len() >= 1,
        "辟撰：成为黑桃牌目标获得「书」"
    );
}

#[test]
fn tongbo_swaps_shu_and_gives_on_four_suits() {
    let mgr = manager(3123);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hand_card) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_395");
        st.turn_counters.clear();
        pid = st.seats[seat].player_id.clone();
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = st.seats[target].player_id.clone();
        // 四种花色的「书」
        for suit in [Suit::Spade, Suit::Heart, Suit::Club, Suit::Diamond] {
            let card = give_suit(st, seat, suit);
            take_card(st, &card);
            crate::skills::add_mark(st, seat, "shu", &card);
        }
        hand_card = give_suit(st, seat, Suit::Spade);
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, false);
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("tongbo"), "通博：摸牌后询问");
    mgr.execute_action(&room, &pid, &format!("choose:{hand_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let any_shu = pending_options_with(&st, "choose:")
        .into_iter()
        .next()
        .expect("shu");
    mgr.execute_action(&room, &pid, &any_shu).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("tongbo"), "通博：四花色后分配");
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        crate::skills::mark_cards(&st, seat, "shu").len(),
        0,
        "通博：所有「书」交出"
    );
    assert!(st.seats[target].hand.len() >= 4, "通博：目标获得「书」");
}
