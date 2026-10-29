//! 视图三分区投影（ADR-0020 / `data-model.md` §4.2）。
//!
//! - `public_board`：所有端可见的公共局面（**只读**）；
//! - `player_area`：本人私有资源（手牌明细、身份、候选武将、个人 pending）；
//! - `special_zones`：按 `visibility` 下发的特殊交互区。
//!
//! 隐藏信息（手牌明细、非主公身份、候选武将、牌堆顺序）**只出现在 `player_area`**。

use crate::data;
use crate::state::*;
use serde_json::{json, Map, Value};

/// 牌实例视图（含展示用牌名/分类）。
pub fn card_view(st: &SgsState, id: &str) -> Value {
    let Some(c) = st.cards.get(id) else {
        return json!({ "id": id, "missing": true });
    };
    let def = data::card_def(&c.def);
    json!({
        "id": id,
        "def": c.def,
        "name": def.map(|d| d.name).unwrap_or(c.def.as_str()),
        "suit": c.suit,
        "rank": c.rank,
        "category": def.map(|d| d.category),
        "equip_slot": def.and_then(|d| d.equip_slot),
        "range_value": def.and_then(|d| d.range_value),
        "range_type": def.and_then(|d| d.range_type),
        "target_mode": def.and_then(|d| d.target_mode),
        "delayed": def.is_some_and(|d| d.delayed),
    })
}

fn equips_view(st: &SgsState, seat: &Seat) -> Value {
    let mut m = Map::new();
    for slot in EquipSlot::ALL {
        let v = match seat.equips.get(&slot) {
            Some(Some(id)) => card_view(st, id),
            _ => Value::Null,
        };
        let key = serde_json::to_value(slot)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        m.insert(key, v);
    }
    Value::Object(m)
}

fn pending_view(p: &Pending) -> Value {
    json!({
        "kind": p.kind,
        "actor": p.actor,
        "skill": p.skill,
        "options": p.options.iter().map(|o| json!({ "id": o.id, "label": o.label })).collect::<Vec<_>>(),
        "data": p.data,
    })
}

/// 公共局面区。
pub fn public_board(st: &SgsState) -> Value {
    // 选将期间不公开其他玩家已选武将（选将完成后统一亮明）。
    let reveal_generals = matches!(st.phase, Phase::Turn | Phase::Dying | Phase::Finished);
    let seats: Vec<Value> = st
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "seat": i,
                "player_id": s.player_id,
                // 仅主公身份公开；其余死亡/终局前不公开（P2.3 处理亮身份）。
                // 主公公开；死亡后公开身份（规则 §8.3）。
                "identity": if s.identity.is_lord() || !s.alive { Some(s.identity.code()) } else { None },
                "general": if reveal_generals { s.general.clone() } else { None },
                "general_name": if reveal_generals { s.general_name.clone() } else { String::new() },
                "kingdom": if reveal_generals {
                    s.general.as_deref().and_then(data::general).map(|g| g.kingdom)
                } else {
                    None
                },
                "hp": s.hp,
                "max_hp": s.max_hp,
                "hand_count": s.hand.len(),
                "equips": equips_view(st, s),
                "judge_zone": s.judge_zone.iter().map(|id| card_view(st, id)).collect::<Vec<_>>(),
                "alive": s.alive,
                "is_bot": s.is_bot,
                "has_picked": st.setup.picks.contains_key(&s.player_id),
            })
        })
        .collect();

    json!({
        "phase": st.phase,
        "round": st.round,
        "turn_index": st.turn_index,
        "turn": {
            "seat": st.turn,
            "phase": st.turn_phase,
            "player_id": st.current_player(),
        },
        "config": st.config,
        "min_players": MIN_PLAYERS,
        "max_players": MAX_PLAYERS,
        "seat_count": st.seats.len(),
        "seats": seats,
        "deck": {
            "draw": st.deck.draw.len(),
            "discard": st.deck.discard.len(),
            "processing": st.deck.processing.len(),
            "void": st.deck.void.len(),
        },
        "discard_top": st.deck.discard.last().map(|id| card_view(st, id)),
        // 弃牌堆全量（公共信息；ui-spec §4「点击开浮层列表」）
        "discard": st.deck.discard.iter().map(|id| card_view(st, id)).collect::<Vec<_>>(),
        // 公共区只给 pending 的「谁在等」，不给选项内容。
        "pending": st.pending.as_ref().map(|p| json!({ "kind": p.kind, "actor": p.actor })),
        "setup": if st.phase == Phase::Setup {
            json!({
                "lord": st.setup.lord,
                "lord_picked": st.setup.lord_picked,
                "picked_count": st.setup.picks.len(),
            })
        } else {
            Value::Null
        },
        "winner": st.winner,
        "aborted": st.abort_reason,
        // 出牌历史（全量，按时间正序；端侧倒序展示）。
        "history": st
            .history
            .iter()
            .map(|e| json!({ "seq": e.seq, "turn": e.turn, "actor": e.actor, "text": e.text }))
            .collect::<Vec<_>>(),
    })
}

/// 玩家个人资源区；`viewer` 不在座时返回 `None`。
pub fn player_area(st: &SgsState, viewer: &str) -> Option<Value> {
    let idx = st.seat_index(viewer)?;
    let s = &st.seats[idx];
    let picked = st.setup.picks.get(viewer).cloned();
    let general = s.general.clone().or_else(|| picked.clone());
    let skills: Vec<Value> = general
        .as_deref()
        .map(|code| {
            let defs = crate::skills::skills_of(code);
            if defs.is_empty() {
                data::general(code)
                    .map(|g| {
                        g.skills
                            .iter()
                            .map(|name| json!({ "code": name, "name": name, "type": "normal", "implemented": false }))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                defs.iter()
                    .map(|s| {
                        json!({
                            "code": s.code,
                            "name": s.name,
                            "type": s.kind.code(),
                            "implemented": s.implemented,
                        })
                    })
                    .collect()
            }
        })
        .unwrap_or_default();
    let setup = if st.phase == Phase::Setup {
        json!({
            "candidates": st.setup.offers.get(viewer),
            "picked": picked,
            "is_lord": s.identity.is_lord(),
        })
    } else {
        Value::Null
    };
    Some(json!({
        "seat": idx,
        "player_id": s.player_id,
        "identity": s.identity.code(),
        "general": general,
        "general_gid": general.as_deref().and_then(data::general).map(|g| g.gid),
        "kingdom": general.as_deref().and_then(data::general).map(|g| g.kingdom),
        "general_name": if s.general_name.is_empty() {
            general.as_deref().and_then(data::general).map(|g| g.name).unwrap_or_default().to_string()
        } else { s.general_name.clone() },
        "hp": s.hp,
        "max_hp": s.max_hp,
        "hand": s.hand.iter().map(|id| card_view(st, id)).collect::<Vec<_>>(),
        "equips": equips_view(st, s),
        "judge_zone": s.judge_zone.iter().map(|id| card_view(st, id)).collect::<Vec<_>>(),
        "skills": skills,
        "is_bot": s.is_bot,
        "is_current": st.phase == Phase::Turn && idx == st.turn,
        // 只有本人能看到 pending 的选项内容。
        "pending": st.pending.as_ref().filter(|p| p.actor == viewer).map(pending_view),
        "setup": setup,
    }))
}

/// 特殊交互区（公共 + 本人可见）。
pub fn special_zones(st: &SgsState, viewer: Option<&str>) -> Value {
    let mut out = Map::new();
    for (id, zone) in &st.special_zones {
        let visible = zone.visibility == Visibility::Public
            || viewer.is_some_and(|v| zone.owner.as_deref() == Some(v));
        if !visible {
            continue;
        }
        let mut obj = Map::new();
        if let Some(owner) = &zone.owner {
            obj.insert("owner".into(), json!(owner));
        }
        if let Ok(v) = serde_json::to_value(zone.visibility) {
            obj.insert("visibility".into(), v);
        }
        for (k, v) in &zone.payload {
            obj.insert(k.clone(), v.clone());
        }
        out.insert(id.clone(), Value::Object(obj));
    }
    Value::Object(out)
}

/// 视图数据根（`GameState.data` 的对外形态）。
pub fn view_data(st: &SgsState, viewer: Option<&str>) -> Value {
    json!({
        "schema_version": st.schema_version,
        "game": "sanguosha",
        "phase": st.phase,
        "public_board": public_board(st),
        "player_area": viewer.and_then(|v| player_area(st, v)),
        "special_zones": special_zones(st, viewer),
    })
}
