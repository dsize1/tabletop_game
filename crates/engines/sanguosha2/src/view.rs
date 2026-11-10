//! 视图投影（M3）：`public_board` / `player_area` / `special_zones`。
//!
//! 隐藏信息（手牌明细、牌堆顺序、他人 pending 选项/数据、他人身份、私密展示）
//! **只出现在 `player_area`**；公共区只给数量/公共牌区。
//!
//! 牌区投影覆盖 9 类区（`kernel::Zone`）：
//! - 公共：`discard` / `revealed` / 各座位的 `equips` / `judge_zone` / `marks`；
//! - 数量：`draw_count` / `void_count` / `processing_count` / `frame_count` / `hand_count`；
//! - 私有：`player_area.hand`。
//!
//! 字段集由 `tests/view_contract.rs` 冻结；新增字段必须先改契约测试。

use serde_json::{json, Map, Value};

use crate::cards::{DamageKind, TargetMode};
use crate::kernel::{CardId, Game, Slot, Suit, Zone};

pub const SCHEMA_VERSION: u32 = 1;

fn suit_key(suit: Suit) -> &'static str {
    match suit {
        Suit::Spade => "spade",
        Suit::Heart => "heart",
        Suit::Club => "club",
        Suit::Diamond => "diamond",
    }
}

fn kind_key(kind: crate::cards::CardKind) -> &'static str {
    match kind {
        crate::cards::CardKind::Basic => "basic",
        crate::cards::CardKind::Trick => "trick",
        crate::cards::CardKind::Equip => "equip",
    }
}

fn damage_key(kind: DamageKind) -> &'static str {
    match kind {
        DamageKind::Normal => "normal",
        DamageKind::Fire => "fire",
        DamageKind::Thunder => "thunder",
    }
}

fn target_mode_key(mode: TargetMode) -> &'static str {
    match mode {
        TargetMode::Chosen => "chosen",
        TargetMode::AllAlive => "all",
        TargetMode::SelfOnly => "self",
    }
}

fn slot_key(slot: Slot) -> &'static str {
    match slot {
        Slot::Weapon => "weapon",
        Slot::Armor => "armor",
        Slot::HorseOffense => "horse_offense",
        Slot::HorseDefense => "horse_defense",
        Slot::Treasure => "treasure",
    }
}

const SLOTS: [Slot; 5] = [
    Slot::Weapon,
    Slot::Armor,
    Slot::HorseOffense,
    Slot::HorseDefense,
    Slot::Treasure,
];

/// 牌实例视图（`name` 是**生效牌名**：转化牌读虚拟定义）。
pub fn card_view(g: &Game, id: &CardId) -> Value {
    let Some(card) = g.cards.get(id) else {
        return json!({ "id": id, "missing": true });
    };
    let def = g.def_of(id);
    json!({
        "id": id,
        "name": g.name_of(id),
        "name_is_virtual": g.virtual_defs.contains_key(id),
        "suit": suit_key(card.suit),
        "suit_label": card.suit.label(),
        "rank": card.rank,
        "kind": kind_key(def.kind),
        "slot": def.slot.map(slot_key),
        "effect": format!("{:?}", def.effect),
        "damage_kind": damage_key(def.damage_kind),
        "target_mode": target_mode_key(def.tags.target_mode.unwrap_or(TargetMode::Chosen)),
        "delayed": def.tags.delayed,
        "revealed": g.is_revealed(id),
        // 落英标记（酒诗：背面朝上时该牌无距离限制且不能被响应；离开手牌即清除）。
        "marks": if g.luoying_cards.contains(id) { json!(["luoying"]) } else { json!([]) },
    })
}

fn cards_view(g: &Game, ids: &[CardId]) -> Value {
    Value::Array(ids.iter().map(|id| card_view(g, id)).collect())
}

fn equips_view(g: &Game, seat: usize) -> Value {
    let mut m = Map::new();
    for slot in SLOTS {
        let v = match g.equip_in(seat, slot) {
            Some(id) => card_view(g, &id),
            None => Value::Null,
        };
        m.insert(slot_key(slot).to_string(), v);
    }
    Value::Object(m)
}

fn marks_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    // 收集该座位所有 Mark 区（code 从 zone 推断，避免第二份归属表）。
    let mut codes: Vec<String> = Vec::new();
    for id in g.all_ids() {
        if let Some(Zone::Mark(s, code)) = g.zone_of(id) {
            if *s == seat && !codes.contains(code) {
                codes.push(code.clone());
            }
        }
    }
    for code in codes {
        let ids = g.mark_cards(seat, &code);
        if !ids.is_empty() {
            out.insert(code, cards_view(g, &ids));
        }
    }
    Value::Object(out)
}

fn marks_count_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    for (code, n) in &g.seats[seat].counters {
        // 只暴露「标记类」计数（ju / quan / chuang / ni 等），不暴露内部计数器。
        if matches!(code.as_str(), "ju" | "quan" | "chuang" | "ni") && *n > 0 {
            out.insert(code.clone(), json!(n));
        }
    }
    // 巧（界简雍）：回合内累计、回合切换清空（`turn.` 前缀计数），单独投影为标记。
    let qiao = g.counter(&format!("turn.qiao:{seat}"));
    if qiao > 0 {
        out.insert("qiao".to_string(), json!(qiao));
    }
    Value::Object(out)
}

fn transforms_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    for code in crate::skills::TRANSFORM_SKILLS {
        if !crate::skills::active(g, seat, code) {
            continue;
        }
        out.insert(
            (*code).to_string(),
            json!(if g.yang(seat, code) { "yang" } else { "yin" }),
        );
    }
    Value::Object(out)
}

fn public_seat(g: &Game, seat: usize) -> Value {
    let s = &g.seats[seat];
    json!({
        "seat": seat,
        "player_id": s.name,
        "general": s.general,
        "kingdom": s.kingdom,
        "hp": s.hp,
        "max_hp": s.max_hp,
        "hand_count": g.hand(seat).len(),
        "hand_limit": g.hand_limit(seat),
        "equips": equips_view(g, seat),
        // 判定区按 LIFO（后置先判）投影，与判定阶段结算顺序一致。
        "judge_zone": cards_view(g, &g.judge_zone_lifo(seat)),
        "marks": marks_view(g, seat),
        "marks_count": marks_count_view(g, seat),
        "abolished_slots": s
            .abolished_slots
            .iter()
            .map(|slot| slot_key(*slot).to_string())
            .collect::<Vec<_>>(),
        "judge_abolished": s.judge_abolished,
        "flipped": s.flipped,
        "chained": s.chained,
        "alive": s.hp > 0,
        "skills": s.skills,
        "transforms": transforms_view(g, seat),
    })
}

fn pending_public(g: &Game) -> Value {
    match &g.pending {
        Some(p) => json!({
            "kind": p.kind(),
            "actor": p.actor(),
            "skill": p.skill(),
        }),
        None => Value::Null,
    }
}

fn pending_private(g: &Game, viewer: Option<usize>) -> Value {
    match (&g.pending, viewer) {
        (Some(p), Some(v)) if p.actor() == v => json!({
            "kind": p.kind(),
            "actor": p.actor(),
            "skill": p.skill(),
            "options": p.options().iter().map(|o| json!({ "id": o.id, "label": o.label })).collect::<Vec<_>>(),
            "escape": p.escape_id(),
            "data": p.data(),
        }),
        _ => Value::Null,
    }
}

fn presentation_view(g: &Game) -> Value {
    Value::Array(
        g.presentation
            .iter()
            .map(|p| {
                json!({
                    "skill": p.skill,
                    "actor": p.actor,
                    "targets": p.targets,
                    "cards": p.cards,
                    "outcome": serde_json::to_value(&p.outcome).unwrap_or(Value::Null),
                    "fx": p.fx,
                    "text": p.text(),
                })
            })
            .collect(),
    )
}

fn log_view(g: &Game) -> Value {
    Value::Array(
        g.log
            .iter()
            .map(|e| json!({ "seq": e.seq, "text": e.text }))
            .collect(),
    )
}

fn phase_key(phase: super::kernel::Phase) -> &'static str {
    match phase {
        super::kernel::Phase::Prepare => "prepare",
        super::kernel::Phase::Judge => "judge",
        super::kernel::Phase::Draw => "draw",
        super::kernel::Phase::Play => "play",
        super::kernel::Phase::Discard => "discard",
        super::kernel::Phase::End => "end",
    }
}

fn root(g: &Game, seat: Option<usize>) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "game_id": "sanguosha2",
        "turn": g.turn,
        "phase": phase_key(g.phase),
        "round": g.round,
        "public_board": {
            "seats": (0..g.seats.len()).map(|s| public_seat(g, s)).collect::<Vec<_>>(),
            "draw_count": g.ids_in(|z| *z == Zone::Draw).len(),
            "discard": cards_view(g, &g.discard_pile()),
            "void_count": g.ids_in(|z| *z == Zone::Void).len(),
            "processing_count": g.ids_in(|z| *z == Zone::Processing).len(),
            "frame_count": g.ids_in(|z| z.is_frame()).len(),
            "revealed": cards_view(g, &g.revealed.iter().cloned().collect::<Vec<_>>()),
            "pending": pending_public(g),
            "presentation": presentation_view(g),
            "log": log_view(g),
            "finished": g.finished,
            "winner": g.winner,
        },
        "player_area": {
            "seat": seat,
            "player_id": seat.map(|s| g.seats[s].name.clone()),
            "identity": seat.map(|s| g.seats[s].identity.clone()),
            "hand": match seat {
                Some(s) => cards_view(g, &g.hand(s)),
                None => Value::Array(Vec::new()),
            },
            "hand_limit": seat.map(|s| g.hand_limit(s)),
            "private_reveals": match seat {
                Some(s) => cards_view(
                    g,
                    &g.private_reveals
                        .get(&s)
                        .map(|set| set.iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default(),
                ),
                None => Value::Array(Vec::new()),
            },
            "pending": pending_private(g, seat),
        },
        "special_zones": [],
    })
}

/// 公共视图（`seat=None`：没有任何私有信息）。
pub fn public_state(g: &Game) -> Value {
    root(g, None)
}

/// 个人视图（只包含该座位的私有信息）。
pub fn player_view(g: &Game, seat: usize) -> Value {
    root(g, Some(seat))
}

/// player_id → 座位号。
pub fn seat_of(g: &Game, player_id: &str) -> Option<usize> {
    g.seats.iter().position(|s| s.name == player_id)
}
