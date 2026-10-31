//! P4.4.2 界标 25 将批次 3（吴 8 将）：界甘宁 / 界吕蒙 / 界黄盖 / 界周瑜 / 界大乔 / 界陆逊 / 界孙权 / 界孙尚香。

use super::p24::{set_general, start_and_pick};
use super::{clear_hands, drive_mgr_inject, edit, manager, state_of, take_card};
use crate::state::*;

fn other_seats(st: &SgsState, cur: usize) -> Vec<usize> {
    st.alive_indices()
        .into_iter()
        .filter(|i| *i != cur)
        .collect()
}

fn give_card(st: &mut SgsState, seat: usize, pred: impl Fn(&CardInst) -> bool) -> String {
    let id = st
        .cards
        .iter()
        .find(|(id, c)| {
            pred(c)
                && !st.seats.iter().any(|s| s.hand.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.marks.values().flatten().any(|h| h == *id))
                && !st.deck.processing.iter().any(|d| d == *id)
                && !st
                    .seats
                    .iter()
                    .any(|s| s.judge_zone.iter().any(|h| h == *id))
                && !st
                    .seats
                    .iter()
                    .any(|s| s.equips.values().flatten().any(|h| h == *id))
        })
        .map(|(id, _)| id.clone())
        .expect("没有可用的牌");
    take_card(st, &id);
    st.seats[seat].hand.push(id.clone());
    id
}

fn give_def(st: &mut SgsState, seat: usize, def: &str) -> String {
    give_card(st, seat, |c| c.def == def)
}

fn give_suit(st: &mut SgsState, seat: usize, suit: Suit) -> String {
    give_card(st, seat, |c| c.suit == suit)
}

fn move_to_judge(st: &mut SgsState, seat: usize, def: &str) -> String {
    let id = give_def(st, seat, def);
    st.seats[seat].hand.retain(|c| c != &id);
    st.seats[seat].judge_zone.push(id.clone());
    id
}

fn hand_count(st: &SgsState, seat: usize) -> usize {
    st.seats[seat].hand.len()
        + st.seats[seat].judge_zone.len()
        + st.seats[seat].equips.values().flatten().count()
}

// ---------------------------------------------------------------------------
// 界甘宁 · 奇袭 / 奋威
// ---------------------------------------------------------------------------

#[test]
fn fenwei_invalidates_selected_targets() {
    let mgr = manager(1200);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut ganing, mut a, mut b) = (0, 0, 0, 0);
    let mut pids = (String::new(), String::new(), String::new(), String::new());
    let mut wanjian = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_2");
        let others = other_seats(st, user);
        ganing = others[0];
        a = others[1];
        b = others[2];
        set_general(st, ganing, "sgs_hero_305");
        for seat in others {
            if seat != ganing {
                set_general(st, seat, "sgs_hero_1");
            }
        }
        wanjian = give_def(st, user, "wanjian_qifa");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        pids = (
            st.seats[user].player_id.clone(),
            st.seats[ganing].player_id.clone(),
            st.seats[a].player_id.clone(),
            st.seats[b].player_id.clone(),
        );
    });
    let hp_a = state_of(&mgr, &room).seats[a].hp;
    let hp_b = state_of(&mgr, &room).seats[b].hp;
    mgr.execute_action(&room, &pids.0, &format!("play:{wanjian}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("fenwei"),
        "多目标锦囊指定目标后应询问奋威"
    );
    mgr.execute_action(&room, &pids.1, &format!("choose:{}", pids.2))
        .unwrap();
    mgr.execute_action(&room, &pids.1, "confirm").unwrap();
    let mut inject = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[a].hp, hp_a, "奋威：被选目标不受到【万箭齐发】伤害");
    assert_eq!(st.seats[b].hp, hp_b - 1, "未被奋威的目标照常受伤");
    assert!(st.seats[ganing].flags.contains_key("fenwei_used"));
}

#[test]
fn fenwei_limited_once_and_requires_multiple_targets() {
    let mgr = manager(1201);
    let room = start_and_pick(&mgr, 5);
    let mut used_again = true;
    let mut single = true;
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_305");
        st.seats[cur].flags.insert("fenwei_used".into(), true);
        used_again = crate::resolve_jiexian::maybe_fenwei(st, &[0, 1]);
        st.seats[cur].flags.remove("fenwei_used");
        single = crate::resolve_jiexian::maybe_fenwei(st, &[0]);
    });
    assert!(!used_again, "奋威为限定技，用过不再询问");
    assert!(!single, "仅一个目标时不触发奋威");
}

#[test]
fn jie_ganing_qixi_black_card_as_guohe() {
    let mgr = manager(1202);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut black) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_305");
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
            give_def(st, seat, "sha");
        }
        black = give_card(st, cur, |c| matches!(c.suit, Suit::Spade | Suit::Club));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter().any(|a| a.id == format!("skill:qixi:{black}")),
        "奇袭：黑色牌当【过河拆桥】"
    );
}

// ---------------------------------------------------------------------------
// 界吕蒙 · 克己 / 勤学 / 博图
// ---------------------------------------------------------------------------

#[test]
fn qinxue_awakening_grants_gongxin() {
    let mgr = manager(1210);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut max_hp) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_306");
        for _ in 0..4 {
            give_def(st, cur, "sha");
        }
        st.seats[cur].hp = 1;
        max_hp = st.seats[cur].max_hp;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        st.seats[cur].flags.remove("qinxue_offered");
        st.seats[cur].flags.remove("qinxue_awakened");
        cur_pid = st.seats[cur].player_id.clone();
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应询问勤学");
    assert_eq!(pending.skill.as_deref(), Some("qinxue"));
    mgr.execute_action(&room, &cur_pid, "heal").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].max_hp, max_hp - 1, "勤学：减 1 点体力上限");
    assert!(
        st.seats[cur].extra_skills.iter().any(|c| c == "gongxin"),
        "勤学：获得【攻心】"
    );
    assert!(st.seats[cur].flags.contains_key("qinxue_awakened"));
}

#[test]
fn qinxue_draw_option() {
    let mgr = manager(1211);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut hand_before) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_306");
        for _ in 0..4 {
            give_def(st, cur, "shan");
        }
        st.seats[cur].hp = 1;
        hand_before = st.seats[cur].hand.len();
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
        st.seats[cur].flags.remove("qinxue_offered");
        st.seats[cur].flags.remove("qinxue_awakened");
        cur_pid = st.seats[cur].player_id.clone();
        assert!(
            crate::resolve_jiexian::qinxue_ready(st, cur),
            "结束阶段满足觉醒条件"
        );
        let mut ev = Vec::new();
        crate::turn::phase_step(st, &mut ev).unwrap();
        crate::resolve::advance(st, &mut ev).unwrap();
    });
    mgr.execute_action(&room, &cur_pid, "draw").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before + 2,
        "勤学（摸牌）：摸 2 张"
    );
}

#[test]
fn botu_grants_extra_turn_after_four_suits_discarded() {
    let mgr = manager(1212);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut after_turn, mut turn_index_after, mut turns_played) = (0, 0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_306");
        st.turn_counters.clear();
        // 本回合进入弃牌堆的牌覆盖四种花色。
        st.set_counter("discard_suits", 15);
        st.turn_phase = TurnPhase::Discard;
        let mut ev = Vec::new();
        for _ in 0..4 {
            crate::turn::phase_step(st, &mut ev).unwrap();
        }
        after_turn = st.turn;
        turn_index_after = st.turn_index;
        turns_played = st
            .seats
            .get(cur)
            .map(|s| {
                s.mark_counts
                    .iter()
                    .filter(|(k, _)| k.starts_with("botu:"))
                    .map(|(_, v)| *v)
                    .sum()
            })
            .unwrap_or(0);
    });
    assert_eq!(after_turn, cur, "博图：四种花色齐 → 自己获得额外回合");
    assert_eq!(turns_played, 1, "博图：本轮使用次数 +1");
    let _ = turn_index_after;
}

// ---------------------------------------------------------------------------
// 界黄盖 · 苦肉 / 诈降
// ---------------------------------------------------------------------------

#[test]
fn jie_kurou_discards_loses_hp_and_draws_with_zhaxiang() {
    let mgr = manager(1220);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut cost, mut hand_before, mut hp_before) =
        (0, String::new(), String::new(), 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_307");
        give_def(st, cur, "shan");
        cost = give_def(st, cur, "sha");
        hand_before = st.seats[cur].hand.len();
        hp_before = st.seats[cur].hp;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("skill:kurou:{cost}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, hp_before - 1, "界苦肉：失去 1 点体力");
    assert_eq!(st.counter("kurou_used"), 1, "界苦肉：本阶段限一次");
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before - 1 + 2 + 3,
        "界苦肉：弃 1 摸 2；诈降再摸 3"
    );
    assert!(st.history.iter().any(|e| e.text.contains("诈降")));
}

#[test]
fn zhaxiang_red_sha_no_distance_no_respond() {
    let mgr = manager(1221);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut red_sha, mut far, mut far_pid) =
        (0, String::new(), String::new(), 0, String::new());
    let mut no_distance = false;
    let mut no_respond = false;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_307");
        let others = other_seats(st, cur);
        far = others[2];
        for seat in others {
            set_general(st, seat, "sgs_hero_1");
        }
        red_sha = give_card(st, cur, |c| {
            c.def == "sha" && matches!(c.suit, Suit::Heart | Suit::Diamond)
        });
        give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.set_counter("zhaxiang_red", 1);
        cur_pid = st.seats[cur].player_id.clone();
        far_pid = st.seats[far].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, &format!("play:{red_sha}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    for f in &st.resolution {
        if let crate::resolve::Frame::Sha(s) = f {
            no_distance = s.no_distance;
            no_respond = s.no_respond;
        }
    }
    assert!(no_distance, "诈降：红杀无距离限制");
    assert!(no_respond, "诈降：红杀不能被抵消");
    let _ = (far, far_pid);
}

#[test]
fn zhaxiang_draw_on_damage() {
    let mgr = manager(1222);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_307");
        st.turn_counters.clear();
        before = st.seats[cur].hand.len();
        crate::resolve::apply_damage(
            st,
            cur,
            1,
            Some(other_seats(st, cur)[0]),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[cur].hand.len(),
        before + 3,
        "诈降：失去体力后摸 3 张"
    );
}

// ---------------------------------------------------------------------------
// 界周瑜 · 英姿 / 反间
// ---------------------------------------------------------------------------

#[test]
fn jie_yingzi_hand_limit_is_max_hp() {
    let mgr = manager(1230);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut limit) = (0, 0);
    edit(&mgr, &room, |st| {
        cur = st.turn;
        set_general(st, cur, "sgs_hero_308");
        st.seats[cur].max_hp = 3;
        st.seats[cur].hp = 1;
        limit = crate::skills::hand_limit(st, cur);
    });
    assert_eq!(limit, 3, "界英姿：手牌上限为体力上限");
}

#[test]
fn jie_fanjian_discard_same_suit() {
    let mgr = manager(1231);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut give) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_308");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give = give_suit(st, cur, Suit::Spade);
        for _ in 0..2 {
            give_suit(st, target, Suit::Spade);
        }
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:fanjian")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{give}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.actor.as_str()),
        Some(target_pid.as_str()),
        "反间：由目标二选一"
    );
    mgr.execute_action(&room, &target_pid, "discard").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hand.len(), 0, "反间：弃置所有同花色手牌");
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp);
}

#[test]
fn jie_fanjian_target_can_lose_hp() {
    let mgr = manager(1232);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut give) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_308");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        give = give_suit(st, cur, Suit::Spade);
        give_suit(st, target, Suit::Heart);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hp = state_of(&mgr, &room).seats[target].hp;
    mgr.execute_action(&room, &cur_pid, "skill:fanjian")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{give}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "damage").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, hp - 1, "反间：目标选择失去 1 点体力");
}
// ---------------------------------------------------------------------------
// 界大乔 · 流离 / 国色
// ---------------------------------------------------------------------------

#[test]
fn jie_guose_limited_once_and_draws() {
    let mgr = manager(1240);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut target, mut cur_pid, mut target_pid, mut diamond) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_309");
        target = other_seats(st, cur)[0];
        set_general(st, target, "sgs_hero_1");
        diamond = give_card(st, cur, |c| c.suit == Suit::Diamond);
        give_def(st, cur, "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        target_pid = st.seats[target].player_id.clone();
    });
    let hand_before = state_of(&mgr, &room).seats[cur].hand.len();
    mgr.execute_action(&room, &cur_pid, &format!("skill:guose:{diamond}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("guose_used"), 1, "界国色：本阶段限一次");
    assert_eq!(
        st.seats[cur].hand.len(),
        hand_before,
        "界国色：用牌后再摸 1 张（净手牌不变）"
    );
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        !acts.iter().any(|a| a.id.starts_with("skill:guose:")),
        "界国色：本阶段已发动过"
    );
}

#[test]
fn jie_guose_exchange_discards_lebu() {
    let mgr = manager(1241);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut victim, mut cur_pid, mut diamond, mut lebu) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_309");
        victim = other_seats(st, cur)[0];
        set_general(st, victim, "sgs_hero_1");
        lebu = move_to_judge(st, victim, "lebu_sishu");
        diamond = give_card(st, cur, |c| c.suit == Suit::Diamond);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    let acts = crate::turn::legal_actions(&st, &cur_pid);
    assert!(
        acts.iter().any(|a| a.id == "skill:guose:exchange"),
        "界国色：选项二可弃方片+场上【乐不思蜀】"
    );
    mgr.execute_action(&room, &cur_pid, "skill:guose:exchange")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{diamond}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !st.seats[victim].judge_zone.contains(&lebu),
        "界国色：场上【乐不思蜀】被弃置"
    );
    assert!(
        st.deck.discard.contains(&diamond) && st.deck.discard.contains(&lebu),
        "界国色：方片牌与乐不思蜀均进入弃牌堆"
    );
    assert_eq!(st.counter("guose_used"), 1);
}

#[test]
fn jie_guose_exchange_needs_lebu_on_field() {
    let mgr = manager(1242);
    let room = start_and_pick(&mgr, 5);
    let (mut cur_pid, mut has_action) = (String::new(), true);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_309");
        give_card(st, cur, |c| c.suit == Suit::Diamond);
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        has_action = crate::turn::legal_actions(st, &cur_pid)
            .iter()
            .any(|a| a.id == "skill:guose:exchange");
    });
    assert!(!has_action, "界国色：场上无【乐不思蜀】时选项二不可用");
}

// ---------------------------------------------------------------------------
// 界陆逊 · 谦逊 / 连营
// ---------------------------------------------------------------------------

#[test]
fn jie_qianxun_moves_hand_out_on_trick() {
    let mgr = manager(1250);
    let room = start_and_pick(&mgr, 5);
    let (mut user, mut luxun, mut user_pid, mut luxun_pid, mut guohe) =
        (0, 0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        user = st.turn;
        set_general(st, user, "sgs_hero_2");
        luxun = other_seats(st, user)[0];
        set_general(st, luxun, "sgs_hero_310");
        give_def(st, luxun, "sha");
        give_def(st, luxun, "shan");
        guohe = give_def(st, user, "guohe_chaiqiao");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        user_pid = st.seats[user].player_id.clone();
        luxun_pid = st.seats[luxun].player_id.clone();
    });
    mgr.execute_action(&room, &user_pid, &format!("play:{guohe}"))
        .unwrap();
    mgr.execute_action(&room, &user_pid, &format!("target:{luxun_pid}"))
        .unwrap();
    let mut inject = Vec::new();
    drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert!(st.seats[luxun].hand.is_empty(), "界谦逊：手牌移出游戏");
    assert_eq!(st.deck.void.len(), 2, "界谦逊：2 张手牌进入移出区");
    assert!(st
        .void_return
        .iter()
        .all(|v| v.owner == luxun && v.due_seat == user));
}

#[test]
fn jie_lianying_chooses_target_to_draw() {
    let mgr = manager(1251);
    let room = start_and_pick(&mgr, 5);
    let (mut luxun, mut other, mut other_pid, mut luxun_pid) = (0, 0, String::new(), String::new());
    let mut before = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        luxun = st.turn;
        set_general(st, luxun, "sgs_hero_310");
        let others = other_seats(st, luxun);
        other = others[0];
        set_general(st, other, "sgs_hero_1");
        let card = give_def(st, luxun, "sha");
        st.turn_counters.clear();
        crate::turn::take_card(st, luxun, &card).unwrap();
        crate::turn::to_discard_card(st, &card);
        let mut ev = Vec::new();
        crate::resolve::advance(st, &mut ev).unwrap();
        before = st.seats[other].hand.len();
        other_pid = st.seats[other].player_id.clone();
        luxun_pid = st.seats[luxun].player_id.clone();
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("lianying"),
        "界连营：失去所有手牌后询问"
    );
    mgr.execute_action(&room, &luxun_pid, &format!("target:{other_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[other].hand.len(),
        before + 1,
        "界连营：目标摸 1 张"
    );
}

#[test]
fn jie_qianxun_no_longer_blocks_shunshou() {
    let mgr = manager(1252);
    let room = start_and_pick(&mgr, 5);
    let (mut jie_blocked, mut std_blocked) = (true, false);
    edit(&mgr, &room, |st| {
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_310");
        jie_blocked = crate::skills::qianxun_blocks(st, cur);
        set_general(st, cur, "sgs_hero_14");
        std_blocked = crate::skills::qianxun_blocks(st, cur);
    });
    assert!(!jie_blocked, "界谦逊不再屏蔽【顺手牵羊】/【过河拆桥】");
    assert!(std_blocked, "标准谦逊口径不变");
}

// ---------------------------------------------------------------------------
// 界孙权 · 制衡 / 救援
// ---------------------------------------------------------------------------

#[test]
fn jie_zhiheng_draws_extra_when_all_hand_discarded() {
    let mgr = manager(1260);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut cards = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_442");
        cards.push(give_def(st, cur, "sha"));
        cards.push(give_def(st, cur, "shan"));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:zhiheng")
        .unwrap();
    for c in &cards {
        mgr.execute_action(&room, &cur_pid, &format!("choose:{c}"))
            .unwrap();
    }
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "界制衡：弃光手牌额外摸 1");
}

#[test]
fn jie_zhiheng_partial_no_extra_draw() {
    let mgr = manager(1261);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid) = (0, String::new());
    let mut cards = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_442");
        cards.push(give_def(st, cur, "sha"));
        cards.push(give_def(st, cur, "shan"));
        cards.push(give_def(st, cur, "tao"));
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:zhiheng")
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{}", cards[0]))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 3, "界制衡：未弃光不额外摸");
}

#[test]
fn jie_jiuyuan_redirects_ally_heal_to_lord() {
    let mgr = manager(1262);
    let room = start_and_pick(&mgr, 5);
    let (mut lord, mut ally, mut ally_pid, mut tao) = (0, 0, String::new(), String::new());
    let mut lord_hp = 0;
    let mut ally_hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        lord = st
            .seats
            .iter()
            .position(|s| s.identity.is_lord())
            .expect("lord");
        ally = (0..st.seats.len())
            .find(|i| *i != lord && st.seats[*i].alive)
            .unwrap();
        set_general(st, lord, "sgs_hero_442");
        set_general(st, ally, "sgs_hero_9");
        st.turn = ally;
        st.turn_phase = TurnPhase::Play;
        st.seats[ally].hp = st.seats[ally].max_hp - 1;
        st.seats[lord].hp = (st.seats[lord].max_hp - 3).max(1);
        tao = give_def(st, ally, "tao");
        st.turn_counters.clear();
        lord_hp = st.seats[lord].hp;
        ally_hp = st.seats[ally].hp;
        ally_pid = st.seats[ally].player_id.clone();
    });
    mgr.execute_action(&room, &ally_pid, &format!("play:{tao}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[lord].hp, lord_hp + 1, "界救援：主公回复 1 点体力");
    assert_eq!(st.seats[ally].hp, ally_hp, "界救援：吴势力角色自身不回复");
    assert_eq!(st.seats[ally].hand.len(), 1, "界救援：其摸 1 张");
}

// ---------------------------------------------------------------------------
// 界孙尚香 · 枭姬 / 结姻
// ---------------------------------------------------------------------------

#[test]
fn jie_jieyin_higher_hp_draws_lower_heals() {
    let mgr = manager(1270);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut male, mut cur_pid, mut male_pid, mut cost) =
        (0, 0, String::new(), String::new(), String::new());
    let mut male_hp = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_443");
        male = other_seats(st, cur)[0];
        set_general(st, male, "sgs_hero_1");
        cost = give_def(st, cur, "sha");
        st.seats[cur].hp = 2;
        st.seats[male].hp = 1;
        male_hp = 1;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        male_pid = st.seats[male].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:jieyin").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{male_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 1, "界结姻：体力较高者摸 1");
    assert_eq!(st.seats[male].hp, male_hp + 1, "界结姻：体力较低者回复 1");
}

#[test]
fn jie_jieyin_lower_hp_heals_self() {
    let mgr = manager(1271);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut male, mut cur_pid, mut male_pid, mut cost) =
        (0, 0, String::new(), String::new(), String::new());
    let mut cur_hp = 0;
    let mut male_hand = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_443");
        male = other_seats(st, cur)[0];
        set_general(st, male, "sgs_hero_1");
        cost = give_def(st, cur, "sha");
        st.seats[cur].hp = 1;
        st.seats[male].hp = st.seats[male].max_hp;
        cur_hp = 1;
        male_hand = st.seats[male].hand.len();
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        male_pid = st.seats[male].player_id.clone();
    });
    mgr.execute_action(&room, &cur_pid, "skill:jieyin").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("choose:{cost}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, "confirm").unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{male_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hp, cur_hp + 1, "界结姻：体力较低者回复 1");
    assert_eq!(
        st.seats[male].hand.len(),
        male_hand + 1,
        "界结姻：体力较高者摸 1"
    );
}

#[test]
fn jie_jieyin_can_target_full_hp_male() {
    let mgr = manager(1272);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut male, mut cur_pid, mut has_action) = (0, 0, String::new(), false);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_443");
        male = other_seats(st, cur)[0];
        set_general(st, male, "sgs_hero_1");
        give_def(st, cur, "sha");
        st.seats[male].hp = st.seats[male].max_hp;
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        cur_pid = st.seats[cur].player_id.clone();
        has_action = crate::turn::legal_actions(st, &cur_pid)
            .iter()
            .any(|a| a.id == "skill:jieyin");
    });
    assert!(has_action, "界结姻：可选未受伤男性角色");
    let _ = male;
}
