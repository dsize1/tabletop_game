//! P2 收尾：交互原语描述符门禁（harness `check:interaction-primitives`）。
//!
//! 约束（`docs/conventions/interaction-primitives.md` §4 与
//! `docs/games/sanguosha/special-zones.md` §4）：
//! - 引擎下发的 `interaction.kind` 只能是已登记原语 `one / many / partition / ordered`；
//! - `commit` 必须非空；
//! - `many`/`partition` 语义的基数必须 `min <= max`（技能多选帧）；
//! - `pending.kind` 必须是 `state-machine.md` §2.2 登记过的类型；
//! - `ordered` 必须带槽位（观星 `L1..L5,R1..R5`）。

use super::p24::set_general;
use super::{clear_hands, edit, give, lobby_with, manager, pick_all, set_top, state_of};
use crate::state::*;
use game_engine_core::GameManager;
use serde_json::Value;
use std::collections::BTreeSet;

const REGISTERED_PRIMITIVES: &[&str] = &["one", "many", "partition", "ordered"];

const REGISTERED_PENDING_KINDS: &[&str] = &[
    "target",
    "respond",
    "dying",
    "discard",
    "wuxie",
    "pick_card",
    "choice",
    "choose_many",
    "wugu",
    "arrange",
    "judge_rewrite",
];

fn check_descriptor(value: &Value, label: &str) -> String {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{label} 不是对象：{value}"));
    let kind = obj
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{label} 缺少 interaction.kind：{value}"));
    assert!(
        REGISTERED_PRIMITIVES.contains(&kind),
        "{label} 使用了未登记原语 {kind}（已登记：{REGISTERED_PRIMITIVES:?}）"
    );
    let commit = obj.get("commit").and_then(Value::as_str).unwrap_or("");
    assert!(!commit.is_empty(), "{label} 的 interaction.commit 不能为空");
    if kind == "ordered" {
        assert!(
            obj.get("slots").is_some(),
            "{label} 使用 ordered 时必须声明 slots"
        );
    }
    kind.to_string()
}

/// 校验一份完整引擎状态里的所有交互描述符；返回本次见到的原语种类。
fn validate_state(st: &SgsState) -> Vec<String> {
    let mut seen = Vec::new();
    for (id, zone) in &st.special_zones {
        if let Some(desc) = zone.payload.get("interaction") {
            seen.push(check_descriptor(
                desc,
                &format!("special_zones.{id}.interaction"),
            ));
        }
    }
    let Some(p) = &st.pending else {
        return seen;
    };
    assert!(
        REGISTERED_PENDING_KINDS.contains(&p.kind.as_str()),
        "pending.kind={} 未在 state-machine.md §2.2 登记",
        p.kind
    );
    if let Some(desc) = p.data.get("interaction") {
        seen.push(check_descriptor(
            desc,
            &format!("pending[{}].interaction", p.kind),
        ));
    }
    if p.kind == "choose_many" {
        let min = p
            .data
            .get("min")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("choose_many 缺少 min：{:?}", p.data));
        let max = p
            .data
            .get("max")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("choose_many 缺少 max：{:?}", p.data));
        assert!(min <= max, "choose_many 基数非法：min={min} > max={max}");
    }
    seen
}

/// 随机对局驱动（确定性）：pending 优先 pass/首项，否则首个可出牌。
fn drive_random_game(seed: u64, steps: usize) -> BTreeSet<String> {
    let mgr = manager(seed);
    let room = lobby_with(&mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(&mgr, &room);

    let mut seen = BTreeSet::new();
    for _ in 0..steps {
        let st = state_of(&mgr, &room);
        seen.extend(validate_state(&st));
        if st.phase == Phase::Finished {
            break;
        }
        let (actor, action) = if let Some(p) = &st.pending {
            let acts = mgr.legal_actions(&room, &p.actor).unwrap();
            let a = acts
                .iter()
                .find(|a| a.id == "pass")
                .unwrap_or_else(|| &acts[0]);
            (p.actor.clone(), a.id.clone())
        } else {
            let pid = st.seats[st.turn].player_id.clone();
            let acts = mgr.legal_actions(&room, &pid).unwrap();
            let a = acts
                .iter()
                .find(|a| a.id.starts_with("play:"))
                .or_else(|| acts.iter().find(|a| a.id == "end_play"))
                .expect("出牌阶段应有合法动作");
            (pid, a.id.clone())
        };
        mgr.execute_action(&room, &actor, &action).unwrap();
    }
    seen
}

fn start_room(mgr: &GameManager) -> String {
    let room = lobby_with(mgr, 5);
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(mgr, &room);
    room
}

#[test]
fn interaction_primitives_are_registered_and_bounded() {
    let seen = drive_random_game(401, 160);
    // 随机对局至少会覆盖 these（target/respond/wuxie/choice/... 等 pending 类型都经过登记校验）。
    assert!(
        !seen.is_empty(),
        "随机对局没有观察到任何 interaction 描述符"
    );
}

#[test]
fn interaction_descriptor_one_for_wugu() {
    let mgr = manager(402);
    let room = start_room(&mgr);
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

    let st = state_of(&mgr, &room);
    let seen = validate_state(&st);
    assert!(
        seen.contains(&"one".to_string()),
        "五谷应为 one 原语：{seen:?}"
    );
    let desc = &st.special_zones["wugu"].payload["interaction"];
    assert_eq!(desc["commit"], "pick");
}

#[test]
fn interaction_descriptor_ordered_for_guanxing() {
    let mgr = manager(403);
    let room = start_room(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let next = st.seats_from_left_of(st.turn)[0];
        set_general(st, next, "sgs_hero_4");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[st.turn].player_id.clone();
    mgr.execute_action(&room, &cur_pid, "end_play").unwrap();

    let st = state_of(&mgr, &room);
    let seen = validate_state(&st);
    assert!(
        seen.contains(&"ordered".to_string()),
        "观星应为 ordered 原语：{seen:?}"
    );
    let desc = &st.special_zones["guanxing"].payload["interaction"];
    assert_eq!(desc["commit"], "arrange");
    assert_eq!(desc["slots"], "L1..L5,R1..R5");
}

#[test]
fn interaction_descriptor_many_has_valid_bounds() {
    let mgr = manager(404);
    let room = start_room(&mgr);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        set_general(st, cur, "sgs_hero_8"); // 孙权 · 制衡
        for _ in 0..3 {
            give(st, cur, "shan");
        }
    });
    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[st.turn].player_id.clone();
    mgr.execute_action(&room, &cur_pid, "skill:zhiheng")
        .unwrap();

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("制衡应进入多选");
    assert_eq!(pending.kind, "choose_many");
    // 校验 min/max 且 min>=1（制衡至少弃 1 张）。
    validate_state(&st);
    assert_eq!(pending.data["min"], 1);
    assert_eq!(pending.data["max"], 3);
}
