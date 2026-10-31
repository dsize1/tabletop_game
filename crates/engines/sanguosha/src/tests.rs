use super::*;
use game_engine_core::{EngineRegistry, GameManager, SeededRng};
use serde_json::Value;

fn manager(seed: u64) -> GameManager {
    let mut reg = EngineRegistry::new();
    reg.register(SanguoshaEngine);
    GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
}

fn state_of(mgr: &GameManager, room: &str) -> SgsState {
    SgsState::parse(&mgr.get_state(room).expect("room").data).expect("parse sgs state")
}

fn public_view(mgr: &GameManager, room: &str) -> Value {
    mgr.public_state(room).expect("public").data
}

fn player_view(mgr: &GameManager, room: &str, player: &str) -> Value {
    mgr.player_view(room, player).expect("player view").data
}

/// 建房并让 u1..uN 入座，返回 room_id。
fn lobby_with(mgr: &GameManager, players: usize) -> String {
    let (st, _) = mgr
        .create_game("sanguosha", "group-1", "u1")
        .expect("create");
    let room = st.room_id.clone();
    for i in 2..=players {
        mgr.execute_action(&room, &format!("u{i}"), "lobby:join")
            .expect("join");
    }
    room
}

fn pick_first(mgr: &GameManager, room: &str, player: &str) {
    let actions = mgr.legal_actions(room, player).expect("legal");
    let action = actions
        .iter()
        .find(|a| a.id.starts_with("setup:choose_general:"))
        .unwrap_or_else(|| panic!("no general offer for {player}: {actions:?}"));
    mgr.execute_action(room, player, &action.id).expect("pick");
}

/// 主公先选，其余按座位顺序各选第一张候选。
fn pick_all(mgr: &GameManager, room: &str) {
    let st = state_of(mgr, room);
    let lord = st.setup.lord.clone();
    let mut order = vec![lord.clone()];
    order.extend(
        st.seats
            .iter()
            .map(|s| s.player_id.clone())
            .filter(|p| p != &lord),
    );
    for pid in order {
        let st = state_of(mgr, room);
        if st.phase == Phase::Setup && !st.setup.picks.contains_key(&pid) {
            pick_first(mgr, room, &pid);
        }
    }
}

/// 建房 + 设定 `Config` + 开赛选将（测试专用）。
fn start_with_config<F: FnOnce(&mut Config)>(
    mgr: &GameManager,
    players: usize,
    configure: F,
) -> String {
    let room = lobby_with(mgr, players);
    edit(mgr, &room, |st| configure(&mut st.config));
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

/// 把一张牌放到武将牌上的标记区（测试专用；从任意区域取出）。
fn place_mark(st: &mut SgsState, seat: usize, key: &str, card_id: &str) {
    take_card(st, card_id);
    st.seats[seat]
        .marks
        .entry(key.to_string())
        .or_default()
        .push(card_id.to_string());
}

/// 将座位翻面（测试专用）。
fn flip_seat(st: &mut SgsState, seat: usize) {
    st.seats[seat].flipped = true;
}

/// 直接改写引擎状态（测试专用）：从任意区域取牌、指定手牌/体力/牌堆顶。
fn edit<F: FnOnce(&mut SgsState)>(mgr: &GameManager, room: &str, f: F) {
    let full = mgr.get_state(room).expect("room");
    let mut st = SgsState::parse(&full.data).expect("parse");
    f(&mut st);
    st.check_conservation().expect("测试改写后牌数应守恒");
    let mut next = full.clone();
    next.data = serde_json::to_value(&st).expect("serialize");
    mgr.restore(next);
}

fn take_card(st: &mut SgsState, id: &str) {
    for zone in [
        &mut st.deck.draw,
        &mut st.deck.discard,
        &mut st.deck.processing,
        &mut st.deck.void,
    ] {
        zone.retain(|c| c != id);
    }
    for seat in &mut st.seats {
        seat.hand.retain(|c| c != id);
        seat.judge_zone.retain(|c| c != id);
        for slot in EquipSlot::ALL {
            if seat.equips.get(&slot).and_then(|v| v.as_ref()) == Some(&id.to_string()) {
                seat.equips.insert(slot, None);
            }
        }
    }
}

fn find_card(st: &SgsState, code: &str) -> String {
    st.cards
        .iter()
        .find(|(_, c)| c.def == code)
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| panic!("牌堆中没有 {code}"))
}

/// 把一张指定定义的牌放进某玩家手牌（从任意区域取出）。
fn give(st: &mut SgsState, seat: usize, code: &str) -> String {
    let candidates: Vec<String> = st
        .cards
        .iter()
        .filter(|(_, c)| c.def == code)
        .map(|(id, _)| id.clone())
        .collect();
    let in_any_hand =
        |st: &SgsState, id: &str| st.seats.iter().any(|s| s.hand.iter().any(|c| c == id));
    // 优先取不在任何手牌里的副本，避免把同一张牌反复给同一人。
    let id = candidates
        .iter()
        .find(|id| !in_any_hand(st, id))
        .or_else(|| {
            candidates
                .iter()
                .find(|id| !st.seats[seat].hand.iter().any(|c| c == *id))
        })
        .unwrap_or_else(|| panic!("没有可用的 {code}"))
        .clone();
    take_card(st, &id);
    st.seats[seat].hand.push(id.clone());
    id
}

/// 让牌堆顶按给定顺序被摸（`pop` 顺序）。
fn set_top(st: &mut SgsState, codes: &[&str]) {
    let ids: Vec<String> = codes.iter().map(|c| find_card(st, c)).collect();
    for id in ids {
        take_card(st, &id);
        st.deck.draw.push(id);
    }
}

fn clear_hands(st: &mut SgsState) {
    for i in 0..st.seats.len() {
        let hand = std::mem::take(&mut st.seats[i].hand);
        for c in hand {
            st.deck.discard.push(c);
        }
    }
}

/// 测试驱动：推进结算栈，遇到响应类 pending 一律 pass（模拟无人响应）。
///
/// P2.5 起还有「必须选择」的 pending（观星 `arrange`、五谷 `wugu`），没有 `pass` 选项时
/// 取第一个候选（端侧默认顺序 / 第一张亮牌）。
fn drive(st: &mut SgsState) {
    let mut events = Vec::new();
    for _ in 0..300 {
        crate::resolve::advance(st, &mut events).unwrap();
        let Some(p) = st.pending.clone() else { return };
        let action = if p.options.iter().any(|o| o.id == "pass") {
            "pass".to_string()
        } else {
            p.options
                .first()
                .unwrap_or_else(|| panic!("pending {} 无候选", p.kind))
                .id
                .clone()
        };
        crate::resolve::execute_pending(st, &p.actor, &action).unwrap();
    }
    panic!("drive 未收敛");
}

/// 注入格式：`"<openid>=<action>"`；未命中则 pass（无 pass 候选时取首项）。
fn take_inject(inject: &mut Vec<String>, actor: &str, kind: &str) -> String {
    for key in [format!("{actor}|{kind}="), format!("{actor}=")] {
        if let Some(i) = inject.iter().position(|x| x.starts_with(&key)) {
            let entry = inject.remove(i);
            return entry.splitn(2, '=').nth(1).unwrap_or("pass").to_string();
        }
    }
    "pass".into()
}

/// 观星（P2.5）在准备阶段自动进入；测试中如只需推进，就提交端侧默认顺序。
fn resolve_arrange(mgr: &GameManager, room: &str) {
    for _ in 0..8 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else { return };
        if p.kind != "arrange" {
            return;
        }
        let action = p
            .options
            .first()
            .unwrap_or_else(|| panic!("观星无提交选项"))
            .id
            .clone();
        mgr.execute_action(room, &p.actor, &action).unwrap();
    }
}

/// 状态级驱动：推进结算栈，按注入规则响应，否则 pass（无 pass 候选时取首项）。
fn drive_state_inject(st: &mut SgsState, inject: &mut Vec<String>) {
    let mut events = Vec::new();
    for _ in 0..800 {
        crate::resolve::advance(st, &mut events).unwrap();
        let Some(p) = st.pending.clone() else { return };
        let mut action = take_inject(inject, &p.actor, &p.kind);
        if action == "pass" && !p.options.iter().any(|o| o.id == "pass") {
            action = p.options.first().map(|o| o.id.clone()).unwrap_or(action);
        }
        crate::resolve::execute_pending(st, &p.actor, &action).unwrap();
    }
    panic!("drive_state_inject 未收敛");
}

/// 阶段推进器：`advance` 直到没有 pending；`choose_many` 自动选未选中的牌后确认。
///
/// 注入格式 `"{actor}={action}"`，按顺序消费（选项存在才使用，否则退回自动策略）。
fn drive_phases(st: &mut SgsState, inject: &mut Vec<String>) {
    let mut events = Vec::new();
    for _ in 0..1200 {
        crate::resolve::advance(st, &mut events).unwrap();
        let Some(p) = st.pending.clone() else { return };
        let key = format!("{}=", p.actor);
        if let Some(pos) = inject.iter().position(|e| e.starts_with(&key)) {
            let entry = inject.remove(pos);
            let action = entry.splitn(2, '=').nth(1).unwrap_or("pass").to_string();
            if p.options.iter().any(|o| o.id == action) {
                crate::resolve::execute_pending(st, &p.actor, &action).unwrap();
                continue;
            }
            inject.insert(pos, entry);
        }
        let action = if p.kind == "choose_many" {
            let selected: Vec<String> = p
                .data["selected"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let min = p.data["min"].as_u64().unwrap_or(0) as usize;
            if selected.len() >= min && p.options.iter().any(|o| o.id == "confirm") {
                "confirm".to_string()
            } else if let Some(opt) = p.options.iter().find(|o| {
                o.id.starts_with("choose:")
                    && !selected.contains(&o.id["choose:".len()..].to_string())
            }) {
                opt.id.clone()
            } else if p.options.iter().any(|o| o.id == "pass") {
                "pass".to_string()
            } else {
                p.options.first().map(|o| o.id.clone()).unwrap_or_default()
            }
        } else if p.options.iter().any(|o| o.id == "pass") {
            "pass".to_string()
        } else {
            p.options.first().map(|o| o.id.clone()).unwrap_or_default()
        };
        crate::resolve::execute_pending(st, &p.actor, &action).unwrap();
    }
    panic!("drive_phases 未收敛");
}

/// 管理器级驱动：推进 pending，按注入规则响应，否则 pass（无 pass 候选时取首项）。
fn drive_mgr_inject(mgr: &GameManager, room: &str, inject: &mut Vec<String>) {
    for _ in 0..800 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else { return };
        let mut action = take_inject(inject, &p.actor, &p.kind);
        if action == "pass" && !p.options.iter().any(|o| o.id == "pass") {
            action = p.options.first().map(|o| o.id.clone()).unwrap_or(action);
        }
        mgr.execute_action(room, &p.actor, &action).unwrap();
    }
    panic!("drive_mgr_inject 未收敛");
}

#[test]
fn lobby_requires_five_players_and_host() {
    let mgr = manager(1);
    let (st, _) = mgr.create_game("sanguosha", "g", "u1").unwrap();
    let room = st.room_id.clone();
    assert_eq!(state_of(&mgr, &room).phase, Phase::Lobby);

    // 1 人不能开始
    assert!(mgr.execute_action(&room, "u1", "lobby:start").is_err());

    for p in ["u2", "u3", "u4"] {
        mgr.execute_action(&room, p, "lobby:join").unwrap();
    }
    // 4 人仍不能开始
    assert!(mgr.execute_action(&room, "u1", "lobby:start").is_err());
    // 非房主不能开始
    assert!(mgr.execute_action(&room, "u2", "lobby:start").is_err());

    mgr.execute_action(&room, "u5", "lobby:join").unwrap();
    let res = mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    assert_eq!(state_of(&mgr, &room).phase, Phase::Setup);
    assert!(res.events.iter().any(|e| e.event_type == "game_started"));
}

#[test]
fn lobby_limits_and_kick() {
    let mgr = manager(2);
    let room = lobby_with(&mgr, 8);
    assert_eq!(state_of(&mgr, &room).seats.len(), 8);
    // 第 9 人无法加入
    assert!(mgr.execute_action(&room, "u9", "lobby:join").is_err());
    // 重复加入报错
    assert!(mgr.execute_action(&room, "u2", "lobby:join").is_err());
    // 踢人
    mgr.execute_action(&room, "u1", "lobby:kick:u3").unwrap();
    assert_eq!(state_of(&mgr, &room).seats.len(), 7);
    // 不能踢房主
    assert!(mgr.execute_action(&room, "u1", "lobby:kick:u1").is_err());
    // 被踢者可重新加入
    mgr.execute_action(&room, "u3", "lobby:join").unwrap();
    assert_eq!(state_of(&mgr, &room).seats.len(), 8);
}

#[test]
fn host_leave_or_abort_finishes_game() {
    let mgr = manager(3);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u2", "lobby:leave").unwrap();
    assert_eq!(state_of(&mgr, &room).seats.len(), 4);

    let res = mgr.execute_action(&room, "u1", "lobby:abort").unwrap();
    assert!(res.finished);
    assert!(mgr.get_active_game("group-1").is_none());
    assert!(mgr.execute_action(&room, "u1", "lobby:join").is_err());
}

#[test]
fn host_can_abort_after_start() {
    let mgr = manager(16);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    assert_eq!(state_of(&mgr, &room).phase, Phase::Setup);
    // 非房主不能中断
    assert!(mgr.execute_action(&room, "u2", "lobby:abort").is_err());
    let res = mgr.execute_action(&room, "u1", "lobby:abort").unwrap();
    assert!(res.finished);
    assert_eq!(state_of(&mgr, &room).phase, Phase::Finished);
    assert!(mgr.get_active_game("group-1").is_none());
}

#[test]
fn start_assigns_identities_and_lord_offers() {
    let mgr = manager(4);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);

    assert_eq!(st.phase, Phase::Setup);
    let mut lord = 0;
    let mut loyalist = 0;
    let mut rebel = 0;
    let mut spy = 0;
    for s in &st.seats {
        match s.identity {
            Identity::Lord => lord += 1,
            Identity::Loyalist => loyalist += 1,
            Identity::Rebel => rebel += 1,
            Identity::Spy => spy += 1,
        }
    }
    assert_eq!((lord, loyalist, rebel, spy), (1, 1, 2, 1));
    // 主公先手
    assert_eq!(st.seats[st.turn].identity, Identity::Lord);
    // 主公候选 = 全部主公将(3) + N(5) 非主公
    let lord_offers = st.setup.offers.get(&st.setup.lord).expect("lord offers");
    assert_eq!(lord_offers.len(), 8);
    for code in ["sgs_hero_1", "sgs_hero_8", "sgs_hero_15"] {
        assert!(lord_offers.contains(&code.to_string()), "缺少主公将 {code}");
    }
    // 主公未选前，其他玩家没有候选
    for s in &st.seats {
        if s.player_id != st.setup.lord {
            assert!(st.setup.offers.get(&s.player_id).is_none());
        }
    }
}

#[test]
fn setup_offers_are_disjoint_and_then_game_starts() {
    let mgr = manager(5);
    let room = lobby_with(&mgr, 6);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);
    let lord = st.setup.lord.clone();
    let lord_pick = st.setup.offers[&lord][0].clone();
    mgr.execute_action(&room, &lord, &format!("setup:choose_general:{lord_pick}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert!(st.setup.lord_picked);
    let mut seen = std::collections::BTreeSet::new();
    for s in &st.seats {
        if s.player_id == lord {
            continue;
        }
        let offers = st.setup.offers.get(&s.player_id).expect("offers");
        // 内置池只有标准 25 将：池子不够 M×人数时按均分降级（4..=5）。
        assert!((4..=5).contains(&offers.len()), "候选张数异常: {offers:?}");
        for c in offers {
            assert!(seen.insert(c.clone()), "候选重复发放：{c}");
            assert_ne!(c, &lord_pick, "主公已选武将不应再发");
        }
    }
    assert_eq!(seen.len(), 24, "主公未选武将应进入候选池（25-1）");

    pick_all(&mgr, &room);
    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.round, 1);
    // 选将完成后自动推进：摸牌阶段已结算，停在出牌阶段。
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(st.seats[st.turn].identity, Identity::Lord);

    let lord_def = data::general(st.seats[st.turn].general.as_deref().expect("lord general"))
        .expect("known general");
    assert_eq!(st.seats[st.turn].max_hp, lord_def.max_hp + 1);
    assert_eq!(st.seats[st.turn].hp, st.seats[st.turn].max_hp);

    let counts = st.check_conservation().expect("conservation");
    // P4.0：默认牌堆 = 标准 + 军争 = 160 张（`Config.packs`）。
    assert_eq!(counts.instances, FULL_DECK_SIZE);
    assert_eq!(st.config.packs, vec!["standard", "junzheng"]);
    for (i, s) in st.seats.iter().enumerate() {
        assert!(s.general.is_some());
        // 初始 4 张；主公（首个回合）已过摸牌阶段 +2。
        assert_eq!(s.hand.len(), if i == st.turn { 6 } else { 4 });
    }
    assert_eq!(st.deck.draw.len(), FULL_DECK_SIZE - 4 * st.seats.len() - 2);
    assert_eq!(counts.total_in_zones(), FULL_DECK_SIZE);
}

#[test]
fn setup_emits_deck_seed_and_deal_events() {
    let mgr = manager(6);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);
    let lord = st.setup.lord.clone();
    let others: Vec<String> = st
        .seats
        .iter()
        .map(|s| s.player_id.clone())
        .filter(|p| p != &lord)
        .collect();

    let lord_pick = st.setup.offers[&lord][0].clone();
    mgr.execute_action(&room, &lord, &format!("setup:choose_general:{lord_pick}"))
        .unwrap();

    let mut last_events = Vec::new();
    for pid in others {
        let actions = mgr.legal_actions(&room, &pid).unwrap();
        let pick = actions
            .iter()
            .find(|a| a.id.starts_with("setup:choose_general:"))
            .unwrap()
            .id
            .clone();
        let res = mgr.execute_action(&room, &pid, &pick).unwrap();
        last_events = res.events;
    }
    assert!(last_events.iter().any(|e| e.event_type == "setup_done"));
    let shuffle = last_events
        .iter()
        .find(|e| e.event_type == "deck_shuffled")
        .expect("deck_shuffled");
    assert_eq!(shuffle.payload["size"], FULL_DECK_SIZE);
    assert_eq!(
        shuffle.payload["packs"],
        serde_json::json!(["standard", "junzheng"])
    );
    assert_eq!(shuffle.payload["seed"], state_of(&mgr, &room).deck.seed);
    assert!(last_events
        .iter()
        .any(|e| e.event_type == "initial_hand_dealt"));
}

#[test]
fn public_view_hides_hand_and_identities() {
    let mgr = manager(7);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(&mgr, &room);

    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Turn);
    let public = public_view(&mgr, &room);
    assert_eq!(public["phase"], "turn");
    assert_eq!(public["public_board"]["seat_count"], 5);

    let lord_seat = st.seats.iter().position(|s| s.identity.is_lord()).unwrap();
    let seats = public["public_board"]["seats"].as_array().unwrap();
    // 只有主公身份公开
    assert_eq!(seats[lord_seat]["identity"], "lord");
    for (i, s) in seats.iter().enumerate() {
        if i != lord_seat {
            assert!(s["identity"].is_null(), "非主公身份泄露: {s}");
        }
        // 公共视图不含手牌明细，只有张数（当前回合玩家已摸 2 张）
        assert!(s.get("hand").is_none());
        assert_eq!(s["hand_count"], if i == st.turn { 6 } else { 4 });
        // 选将完成后武将公开
        assert!(s["general"].is_string());
    }

    let pv = player_view(&mgr, &room, "u2");
    let area = &pv["player_area"];
    assert_eq!(area["player_id"], "u2");
    assert!(area["identity"].is_string());
    let hand = area["hand"].as_array().unwrap();
    let u2_idx = st.seat_index("u2").expect("seat");
    assert_eq!(hand.len(), if u2_idx == st.turn { 6 } else { 4 });
    for c in hand {
        assert!(c["name"].is_string());
        assert!(c["suit"].is_string());
        assert!(c["rank"].is_number());
    }
    // 其他玩家的手牌明细不在本人视图里
    let dump = pv.to_string();
    for s in &st.seats {
        if s.player_id == "u2" {
            continue;
        }
        for id in &s.hand {
            assert!(!dump.contains(&format!("\"{id}\"")), "他人手牌泄露: {id}");
        }
    }
}

#[test]
fn candidates_are_private_during_setup() {
    let mgr = manager(8);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);
    let lord = st.setup.lord.clone();
    let lord_pick = st.setup.offers[&lord][0].clone();
    mgr.execute_action(&room, &lord, &format!("setup:choose_general:{lord_pick}"))
        .unwrap();

    let public = public_view(&mgr, &room);
    assert!(public["public_board"]["setup"]["lord_picked"]
        .as_bool()
        .unwrap());
    assert!(public.to_string().find("candidates").is_none());
    // 选将期间武将不公开
    for s in public["public_board"]["seats"].as_array().unwrap() {
        assert!(s["general"].is_null());
    }

    // 取两名非主公玩家比较候选（主公候选取法不同）。
    let non_lord: Vec<String> = state_of(&mgr, &room)
        .seats
        .iter()
        .filter(|s| !s.identity.is_lord())
        .map(|s| s.player_id.clone())
        .collect();
    let (a, b) = (non_lord[0].clone(), non_lord[1].clone());
    let pv_a = player_view(&mgr, &room, &a);
    let cand_a = pv_a["player_area"]["setup"]["candidates"]
        .as_array()
        .unwrap()
        .clone();
    assert!((4..=5).contains(&cand_a.len()));
    let pv_b = player_view(&mgr, &room, &b);
    let cand_b = pv_b["player_area"]["setup"]["candidates"]
        .as_array()
        .unwrap();
    for c in cand_b {
        assert!(!cand_a.contains(c), "候选不应重叠: {c}");
    }
}

#[test]
fn bots_auto_pick_and_game_starts() {
    let mgr = manager(9);
    let (st, _) = mgr.create_game("sanguosha", "g", "u1").unwrap();
    let room = st.room_id.clone();
    for _ in 0..4 {
        mgr.execute_action(&room, "u1", "lobby:add_bot").unwrap();
    }
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    // 主公可能是真人（u1）或机器人：主公先选，随后机器人自动选完，最后只剩 u1。
    if state_of(&mgr, &room).phase == Phase::Setup
        && !mgr.legal_actions(&room, "u1").unwrap().is_empty()
    {
        pick_first(&mgr, &room, "u1");
    }
    let st = state_of(&mgr, &room);
    assert_eq!(st.phase, Phase::Turn);
    assert_eq!(st.setup.picks.len(), 5);
    for (i, s) in st.seats.iter().enumerate() {
        assert!(s.general.is_some());
        assert_eq!(s.hand.len(), if i == st.turn { 6 } else { 4 });
    }
    st.check_conservation().unwrap();
}

#[test]
fn double_spy_requires_eight_players() {
    let mgr = manager(10);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:set_double_spy:1")
        .unwrap();
    assert!(mgr.execute_action(&room, "u1", "lobby:start").is_err());

    let mgr = manager(11);
    let room = lobby_with(&mgr, 8);
    mgr.execute_action(&room, "u1", "lobby:set_double_spy:1")
        .unwrap();
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);
    let spies = st
        .seats
        .iter()
        .filter(|s| s.identity == Identity::Spy)
        .count();
    let loyalists = st
        .seats
        .iter()
        .filter(|s| s.identity == Identity::Loyalist)
        .count();
    assert_eq!(spies, 2);
    assert_eq!(loyalists, 1);
}

#[test]
fn config_actions_are_host_only_and_validated() {
    let mgr = manager(12);
    let room = lobby_with(&mgr, 5);
    assert!(mgr
        .execute_action(&room, "u2", "lobby:set_max_hand_cards:10")
        .is_err());
    mgr.execute_action(&room, "u1", "lobby:set_max_hand_cards:10")
        .unwrap();
    assert_eq!(state_of(&mgr, &room).config.max_hand_cards, 10);
    assert!(mgr
        .execute_action(&room, "u1", "lobby:set_max_hand_cards:999")
        .is_err());
    // 开局后不能改
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    assert!(mgr
        .execute_action(&room, "u1", "lobby:set_max_hand_cards:20")
        .is_err());
}

#[test]
fn same_seed_same_sequence_is_deterministic() {
    let run = |seed: u64| {
        let mgr = manager(seed);
        let room = lobby_with(&mgr, 5);
        mgr.execute_action(&room, "u1", "lobby:start").unwrap();
        pick_all(&mgr, &room);
        serde_json::to_string(&mgr.get_state(&room).unwrap().data).unwrap()
    };
    assert_eq!(run(2026), run(2026));
    // 不同种子应产生不同的身份/候选/牌序（极小概率相同，取两个明确不同的种子）
    assert_ne!(run(2026), run(2027));
}

#[test]
fn seats_from_left_of_wraps_in_seat_order() {
    let mgr = manager(13);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats_from_left_of(0), vec![1, 2, 3, 4]);
    assert_eq!(st.seats_from_left_of(3), vec![4, 0, 1, 2]);
    assert_eq!(st.seats_from_left_of(4), vec![0, 1, 2, 3]);
}

#[test]
fn hand_cap_config_reaches_state_and_view() {
    let mgr = manager(14);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:set_max_hand_cards:12")
        .unwrap();
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let public = public_view(&mgr, &room);
    assert_eq!(public["public_board"]["config"]["max_hand_cards"], 12);
    assert_eq!(public["public_board"]["config"]["lord_candidate_count"], 5);
    assert_eq!(public["public_board"]["config"]["other_candidate_count"], 5);
}

#[test]
fn resume_is_noop_and_unknown_actions_are_rejected() {
    let mgr = manager(15);
    let room = lobby_with(&mgr, 5);
    let before = serde_json::to_string(&mgr.get_state(&room).unwrap().data).unwrap();
    let res = mgr.execute_action(&room, "u1", "resume").unwrap();
    assert!(res.events.is_empty());
    let after = serde_json::to_string(&mgr.get_state(&room).unwrap().data).unwrap();
    assert_eq!(before, after);
    assert!(mgr.execute_action(&room, "u1", "play:c1").is_err());
    assert!(mgr
        .execute_action(&room, "u1", "setup:choose_general:sgs_hero_1")
        .is_err());
}

#[test]
fn every_standard_card_def_and_general_is_resolvable() {
    // P4.0：标准 108 + 军争 52 = 160 张物理牌。
    assert_eq!(data::STANDARD_DECK.len(), STANDARD_DECK_SIZE);
    assert_eq!(data::JUNZHENG_DECK.len(), JUNZHENG_DECK_SIZE);
    assert_eq!(
        data::FULL_DECK_SIZE,
        STANDARD_DECK_SIZE + JUNZHENG_DECK_SIZE
    );
    assert_eq!(data::STANDARD_GENERALS.len(), STANDARD_GENERAL_COUNT);
    assert_eq!(
        data::WIND_FIRE_FOREST_MOUNTAIN_GENERALS.len(),
        data::WIND_FIRE_FOREST_MOUNTAIN_GENERAL_COUNT
    );
    assert_eq!(
        data::GENERAL_COUNT,
        STANDARD_GENERAL_COUNT
            + data::WIND_FIRE_FOREST_MOUNTAIN_GENERAL_COUNT
            + data::YIJIANG_GENERAL_COUNT
            + data::JIEXIAN_GENERAL_COUNT
    );
    for pc in data::STANDARD_DECK.iter().chain(data::JUNZHENG_DECK) {
        let def = data::card_def(pc.def).unwrap_or_else(|| panic!("缺少定义 {}", pc.def));
        assert!((1..=13).contains(&pc.rank), "非法点数: {pc:?}");
        assert!(
            data::DECK_PACKS.contains(&pc.pack),
            "未知牌堆包: {}",
            pc.pack
        );
        // 装备必须有槽位；基本/锦囊不能有槽位
        if matches!(
            def.category,
            Category::Weapon | Category::Armor | Category::Horse
        ) {
            assert!(def.equip_slot.is_some(), "装备缺少槽位: {}", def.code);
        } else {
            assert!(def.equip_slot.is_none(), "非装备带槽位: {}", def.code);
        }
    }
    let lords = data::STANDARD_GENERALS.iter().filter(|g| g.is_lord).count();
    assert_eq!(lords, 3, "标准包主公将应为 3 位");
    assert_eq!(data::all_general_codes().len(), data::GENERAL_COUNT);
    assert!(data::STANDARD_GENERALS.iter().all(|g| g.pack == "standard"));
    assert!(data::WIND_FIRE_FOREST_MOUNTAIN_GENERALS
        .iter()
        .all(|g| g.pack == "wind_fire_forest_mountain"));
    assert_eq!(data::GENERAL_PACK_KEYS.len(), data::GENERAL_PACKS.len());
    // 武将 code/gid 全局唯一；包内 gid 升序（生成脚本契约）。
    let mut codes = data::all_general_codes();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), data::GENERAL_COUNT);
    assert!(data::WIND_FIRE_FOREST_MOUNTAIN_GENERALS
        .windows(2)
        .all(|w| w[0].gid < w[1].gid));
    // 选将池按包过滤：标准只含 25 将，标准+风林火山含 73 将。
    assert_eq!(
        data::general_codes_in_packs(&["standard".to_string()]).len(),
        STANDARD_GENERAL_COUNT
    );
    let both = vec![
        "standard".to_string(),
        "wind_fire_forest_mountain".to_string(),
    ];
    assert_eq!(
        data::general_codes_in_packs(&both).len(),
        STANDARD_GENERAL_COUNT + data::WIND_FIRE_FOREST_MOUNTAIN_GENERAL_COUNT
    );
    // 全包合计 = GENERAL_COUNT。
    let all: Vec<String> = data::GENERAL_PACK_KEYS.iter().map(|k| k.to_string()).collect();
    assert_eq!(data::general_codes_in_packs(&all).len(), data::GENERAL_COUNT);
}

#[test]
fn general_skill_registry_matches_generated_data() {
    for general in data::GENERAL_PACKS.iter().flat_map(|p| p.iter()) {
        let defs = crate::skills::skills_of(general.code);
        if defs.is_empty() {
            // 未注册（未实现）的武将：允许为空白，但不得进选将池。
            assert!(!crate::skills::general_ready(general.code));
            continue;
        }
        let mut want: Vec<&str> = general.skills.to_vec();
        want.sort_unstable();
        let mut got: Vec<&str> = defs.iter().map(|s| s.name).collect();
        got.sort_unstable();
        assert_eq!(
            got, want,
            "{}（{}）技能名与注册表不一致",
            general.name, general.code
        );
        if defs.iter().all(|s| s.implemented) {
            assert!(crate::skills::general_ready(general.code));
        }
    }
}

mod interaction_contract;
mod p22;
mod p23;
mod p24;
mod p24b;
mod p24c;
mod p25;
mod p26;
mod p40;
mod p41;
mod p42;
mod p43;
mod p44;
mod p45;
mod p46;
mod p47;
mod p48;
mod p49;
mod p50;
mod p51;
mod p52;
mod p53;
mod p54;
mod p55;
mod p56;
mod p57;
mod p58;
mod p59;
mod p60;
mod p61;
mod p62;
mod p63;
mod p64;
mod p65;
mod p66;
mod p67;
mod p68;
mod p69;
mod p70;
mod p71;
mod p72;
mod p73;
mod p74;
mod p75;
mod p76;
mod p77;
mod p78;
mod p79;
mod p80;
mod p81;
mod p82;
mod p83;
mod p84;
mod p85;
mod p86;
mod p87;
mod p88;
mod replay;
mod view_contract;
