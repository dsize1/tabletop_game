//! P2 收尾：视图三分区字段级契约（harness `check:sgs-view-contract`）。
//!
//! 目标：把 `public_board` / `player_area` / `special_zones` 的**字段集**冻结下来，
//! 防 P3 端侧接入时字段漂移或隐藏信息泄漏。新增字段必须先改本文件的期望集
//! （等价于一次契约变更评审）。

use super::p24::set_general;
use super::{
    clear_hands, edit, give, manager, pick_all, player_view, public_view, set_top, state_of,
};
use crate::state::{EquipSlot, TurnPhase};
use game_engine_core::GameManager;
use serde_json::Value;

const ROOT_KEYS: &[&str] = &[
    "schema_version",
    "game",
    "phase",
    "public_board",
    "player_area",
    "special_zones",
];

const BOARD_KEYS: &[&str] = &[
    "phase",
    "round",
    "turn_index",
    "turn",
    "config",
    "min_players",
    "max_players",
    "seat_count",
    "seats",
    "deck",
    "discard_top",
    "discard",
    "pending",
    "setup",
    "winner",
    "aborted",
    "history",
];

const SEAT_KEYS: &[&str] = &[
    "seat",
    "player_id",
    "identity",
    "general",
    "general_name",
    "kingdom",
    "hp",
    "max_hp",
    "hand_count",
    "equips",
    "judge_zone",
    "marks",
    "marks_count",
    "abolished_slots",
    "judge_abolished",
    "flipped",
    "alive",
    "is_bot",
    "has_picked",
];

const AREA_KEYS: &[&str] = &[
    "seat",
    "player_id",
    "identity",
    "general",
    "general_gid",
    "kingdom",
    "general_name",
    "hp",
    "max_hp",
    "hand",
    "equips",
    "judge_zone",
    "marks",
    "marks_count",
    "transforms",
    "abolished_slots",
    "judge_abolished",
    "flipped",
    "skills",
    "equipment_skills",
    "is_bot",
    "is_current",
    "pending",
    "setup",
];

const CARD_KEYS: &[&str] = &[
    "id",
    "def",
    "name",
    "suit",
    "rank",
    "category",
    "equip_slot",
    "range_value",
    "range_type",
    "target_mode",
    "delayed",
];

const EQUIP_SLOTS: &[&str] = &[
    "weapon",
    "armor",
    "horse_offense",
    "horse_defense",
    "treasure",
];

const PENDING_KEYS: &[&str] = &["kind", "actor", "skill", "options", "data"];
const OPTION_KEYS: &[&str] = &["id", "label"];
const SKILL_KEYS: &[&str] = &["code", "name", "type", "implemented"];

/// 期望集（排序后比较）。
fn assert_keys(value: &Value, expected: &[&str], label: &str) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{label} 不是对象：{value}"));
    let mut got: Vec<String> = obj.keys().cloned().collect();
    got.sort();
    let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "{label} 字段集不符：{value}");
}

fn start(mgr: &GameManager) -> String {
    let room = super::lobby_with(mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

#[test]
fn view_contract_midgame_three_zones_shape() {
    let mgr = manager(301);
    let room = start(&mgr);
    // 让装备区 / 判定区 / 弃牌堆都有内容，覆盖非空分支。
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        give(st, cur, "lebu_sishu");
        let weapon = super::find_card(st, "qinggang_jian");
        super::take_card(st, &weapon);
        st.seats[cur]
            .equips
            .insert(crate::state::EquipSlot::Weapon, Some(weapon));
        let armor = super::find_card(st, "bagua_zhen");
        super::take_card(st, &armor);
        st.seats[cur]
            .equips
            .insert(crate::state::EquipSlot::Armor, Some(armor));
        let tao = super::find_card(st, "tao");
        super::take_card(st, &tao);
        st.deck.discard.push(tao);
    });

    let public = public_view(&mgr, &room);
    assert_keys(&public, ROOT_KEYS, "view_data");
    assert_keys(&public["public_board"], BOARD_KEYS, "public_board");
    let seats = public["public_board"]["seats"].as_array().expect("seats");
    assert_eq!(seats.len(), 5);
    for seat in seats {
        assert_keys(seat, SEAT_KEYS, "public_board.seat");
        // 隐藏信息：公共区不得出现手牌明细 / 候选武将。
        assert!(seat.get("hand").is_none(), "公共区不得含 hand：{seat}");
        assert!(seat.get("candidates").is_none(), "公共区不得含 candidates");
        // 装备 5 槽恒定存在。
        let mut slots: Vec<String> = seat["equips"]
            .as_object()
            .expect("equips")
            .keys()
            .cloned()
            .collect();
        slots.sort();
        let mut want: Vec<String> = EQUIP_SLOTS.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(slots, want, "装备槽位应恒为 5 个");
        for (slot, card) in seat["equips"].as_object().expect("equips") {
            if !card.is_null() {
                assert_keys(card, CARD_KEYS, &format!("equips.{slot}"));
            }
        }
    }
    assert_keys(
        &public["public_board"]["discard_top"],
        CARD_KEYS,
        "discard_top",
    );

    let mine = player_view(&mgr, &room, "u1");
    assert_keys(&mine, ROOT_KEYS, "player view_data");
    let area = &mine["player_area"];
    assert_keys(area, AREA_KEYS, "player_area");
    for item in area["equipment_skills"]
        .as_array()
        .expect("equipment_skills")
    {
        assert_keys(
            item,
            &["slot", "code", "name", "card", "trigger"],
            "player_area.equipment_skills[]",
        );
    }
    for card in area["hand"].as_array().expect("hand") {
        assert_keys(card, CARD_KEYS, "player_area.hand[]");
    }
    for card in area["judge_zone"].as_array().expect("judge_zone") {
        assert_keys(card, CARD_KEYS, "player_area.judge_zone[]");
    }
    if let Some(skills) = area["skills"].as_array() {
        for s in skills {
            assert_keys(s, SKILL_KEYS, "player_area.skills[]");
        }
    }
    // 只有本人 area 有手牌明细。
    let other = player_view(&mgr, &room, "u2");
    assert_eq!(
        other["player_area"]["player_id"], "u2",
        "个人视图只投影本人"
    );
}

#[test]
fn view_contract_wugu_zone_shape() {
    let mgr = manager(302);
    let room = start(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wugu_fengdeng");
        set_top(st, &["sha", "shan", "tao", "juedou", "wuzhong_shengyou"]);
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[st.turn].player_id.clone();
    let wugu = super::p24::in_hand(&st, st.turn, "wugu_fengdeng");
    mgr.execute_action(&room, &cur_pid, &format!("play:{wugu}"))
        .unwrap();

    let public = public_view(&mgr, &room);
    let zone = &public["special_zones"]["wugu"];
    assert_keys(
        zone,
        &[
            "visibility",
            "cards",
            "choosers",
            "cursor",
            "current",
            "picked",
            "interaction",
        ],
        "special_zones.wugu",
    );
    assert_eq!(zone["visibility"], "public");
    assert_keys(
        &zone["interaction"],
        &["kind", "commit"],
        "wugu.interaction",
    );
    assert_eq!(zone["interaction"]["kind"], "one");
    for card in zone["cards"].as_array().expect("cards") {
        assert_keys(card, CARD_KEYS, "wugu.cards[]");
    }
    assert_eq!(
        zone["current"],
        Value::String(cur_pid.clone()),
        "释放者先选"
    );

    // 本人 pending 字段集（wugu 选牌）。
    let mine = player_view(&mgr, &room, &cur_pid);
    let pending = &mine["player_area"]["pending"];
    assert_keys(pending, PENDING_KEYS, "player_area.pending");
    for opt in pending["options"].as_array().expect("options") {
        assert_keys(opt, OPTION_KEYS, "pending.options[]");
    }
    // 公共区只给「谁在等」，不给选项内容。
    let pub_pending = public_view(&mgr, &room)["public_board"]["pending"].clone();
    assert_keys(&pub_pending, &["kind", "actor"], "public_board.pending");
}

#[test]
fn view_contract_guanxing_zone_shape_and_visibility() {
    let mgr = manager(303);
    let room = start(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let next = st.seats_from_left_of(st.turn)[0];
        set_general(st, next, "sgs_hero_4");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[st.turn].player_id.clone();
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("观星应停下");
    let owner = pending.actor.clone();

    // 公共视图不得含观星（owner 可见性）。
    let public = public_view(&mgr, &room);
    assert!(
        public["special_zones"].get("guanxing").is_none(),
        "公共视图不得含 guanxing"
    );
    for p in ["u1", "u2", "u3", "u4", "u5"] {
        if p == owner {
            continue;
        }
        assert!(
            player_view(&mgr, &room, p)["special_zones"]
                .get("guanxing")
                .is_none(),
            "{p} 不应看到 guanxing"
        );
    }

    let mine = player_view(&mgr, &room, &owner);
    let zone = &mine["special_zones"]["guanxing"];
    assert_keys(
        zone,
        &[
            "owner",
            "visibility",
            "rows",
            "cols",
            "slots",
            "rev",
            "skill",
            "interaction",
            "cards",
        ],
        "special_zones.guanxing",
    );
    assert_eq!(zone["visibility"], "owner");
    assert_eq!(zone["owner"], Value::String(owner.clone()));
    assert_eq!(zone["rows"], 5);
    assert_eq!(zone["cols"], 2);
    assert!(zone["rev"].as_u64().unwrap_or(0) >= 1);
    assert_keys(
        &zone["interaction"],
        &["kind", "commit", "slots"],
        "guanxing.interaction",
    );
    assert_eq!(zone["interaction"]["kind"], "ordered");
    assert_eq!(zone["interaction"]["commit"], "arrange");
    let mut slot_keys: Vec<&str> = zone["slots"]
        .as_object()
        .expect("slots")
        .keys()
        .map(String::as_str)
        .collect();
    slot_keys.sort();
    let mut want = vec!["L1", "L2", "L3", "L4", "L5", "R1", "R2", "R3", "R4", "R5"];
    want.sort();
    assert_eq!(slot_keys, want, "观星槽位应恒为 L1..L5,R1..R5");

    // arrange pending：字段集 + options 只给默认顺序。
    let pending_view = &mine["player_area"]["pending"];
    assert_keys(pending_view, PENDING_KEYS, "arrange pending");
    assert_keys(
        &pending_view["data"],
        &["zone", "rows", "cols", "slots", "rev", "interaction"],
        "arrange pending.data",
    );
    let opts = pending_view["options"].as_array().expect("options");
    assert_eq!(opts.len(), 1);
    assert_keys(&opts[0], OPTION_KEYS, "arrange option");
    assert!(
        opts[0]["id"].as_str().unwrap_or("").starts_with("arrange:"),
        "提交动作应为 arrange:<csv>"
    );
}

#[test]
fn view_contract_zuilun_zone_shape_and_visibility() {
    let mgr = manager(304);
    let room = start(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_410"); // 诸葛瞻
        st.turn_phase = TurnPhase::End;
        st.turn_counters.clear();
    });
    let mut st = state_of(&mgr, &room);
    let owner = st.seats[st.turn].player_id.clone();
    let mut events = Vec::new();
    // 结束阶段 → 罪论：亮出牌堆顶 3 张并写入特殊区。
    crate::resolve::advance(&mut st, &mut events).unwrap();
    assert!(st.special_zones.contains_key("zuilun"), "罪论应写入特殊区 zuilun");

    // 公共视图与他人视图不得含 zuilun（owner 可见性）。
    let public = crate::view::view_data(&st, None);
    assert!(
        public["special_zones"].get("zuilun").is_none(),
        "公共视图不得含 zuilun"
    );
    for other in ["u1", "u2", "u3", "u4", "u5"] {
        if other == owner {
            continue;
        }
        let view = crate::view::view_data(&st, Some(other));
        assert!(
            view["special_zones"].get("zuilun").is_none(),
            "{other} 不应看到 zuilun"
        );
    }

    // 本人视图：字段集 + 亮牌 + 条件数。
    let mine = crate::view::view_data(&st, Some(&owner));
    let zone = &mine["special_zones"]["zuilun"];
    assert_keys(
        zone,
        &["owner", "visibility", "cards", "conditions"],
        "zuilun",
    );
    assert_eq!(zone["visibility"], "owner");
    assert_eq!(zone["owner"], Value::String(owner.clone()));
    let cards = zone["cards"].as_array().expect("zuilun.cards");
    assert!(!cards.is_empty() && cards.len() <= 3, "罪论亮牌 1..=3 张");
    for c in cards {
        assert_keys(c, CARD_KEYS, "zuilun.cards[]");
    }
    assert!(
        zone["conditions"].as_u64().unwrap_or(99) <= 3,
        "罪论条件数 0..=3"
    );
}

#[test]
fn view_contract_abolished_slots_and_judge_zone() {
    let mgr = manager(306);
    let room = start(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_414"); // 陆抗
        crate::skills::abolish_slot(st, cur, EquipSlot::Armor);
        st.seats[cur].judge_abolished = true;
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let pid = st.seats[cur].player_id.clone();

    let public = public_view(&mgr, &room);
    let seat = &public["public_board"]["seats"][cur];
    assert_eq!(seat["judge_abolished"], Value::Bool(true), "公共区判定区废除");
    assert_eq!(
        seat["abolished_slots"],
        serde_json::json!(["armor"]),
        "公共区废除装备栏"
    );

    let mine = player_view(&mgr, &room, &pid);
    let area = &mine["player_area"];
    assert_eq!(area["judge_abolished"], Value::Bool(true), "个人区判定区废除");
    assert_eq!(
        area["abolished_slots"],
        serde_json::json!(["armor"]),
        "个人区废除装备栏"
    );
    // 废除后该槽视为无装备（装备技能失效）。
    assert!(st.equip_def(cur, EquipSlot::Armor).is_none(), "废除栏无装备定义");
}

#[test]
fn view_contract_deck_counts_and_void_zone() {
    let mgr = manager(307);
    let room = start(&mgr);
    let (mut cur, mut card) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        card = super::find_card(st, "sha");
        super::take_card(st, &card);
        // 模拟破军：一张牌移出游戏（void）。
        st.deck.void.push(card.clone());
        st.void_return.push(crate::state::VoidReturn {
            owner: cur,
            card: card.clone(),
            due_seat: cur,
        });
    });
    let public = public_view(&mgr, &room);
    let deck = &public["public_board"]["deck"];
    assert_keys(deck, &["draw", "discard", "processing", "void"], "public_board.deck");
    assert_eq!(deck["void"], serde_json::json!(1), "移出游戏牌数下发");
}

#[test]
fn view_contract_equip_swap_after_ganlu() {
    let mgr = manager(308);
    let room = start(&mgr);
    let (mut a, mut b) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        let others = st
            .alive_indices()
            .into_iter()
            .filter(|i| *i != cur)
            .collect::<Vec<_>>();
        a = others[0];
        b = others[1];
        let wa = super::find_card(st, "qinggang_jian");
        super::take_card(st, &wa);
        st.seats[a]
            .equips
            .insert(crate::state::EquipSlot::Weapon, Some(wa));
        let ab = super::find_card(st, "bagua_zhen");
        super::take_card(st, &ab);
        st.seats[b]
            .equips
            .insert(crate::state::EquipSlot::Armor, Some(ab));
        // 直接交换（等价甘露结算后状态）。
        let ea = st.seats[a].equips.clone();
        let eb = st.seats[b].equips.clone();
        st.seats[a].equips = eb;
        st.seats[b].equips = ea;
    });
    let public = public_view(&mgr, &room);
    let seats = public["public_board"]["seats"].as_array().expect("seats");
    assert!(
        seats[a]["equips"]["armor"].is_object(),
        "甘露交换后 a 装备区出现防具"
    );
    assert!(
        seats[b]["equips"]["weapon"].is_object(),
        "甘露交换后 b 装备区出现武器"
    );
    for seat in seats {
        assert_keys(&seat["equips"], EQUIP_SLOTS, "seat.equips");
    }
}
