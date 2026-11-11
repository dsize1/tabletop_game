//! 视图投影：`public_board` / `player_area` / `special_zones`。
//!
//! **v1 兼容目标**（OQ2=A，2026-11-10）：v2 接管 `game_id=sanguosha` 时，Web 三分区页
//! 不换 schema —— 本文件输出 v1 消费的字段（`turn` / `deck` / `history` / `setup` /
//! `player_area.identity` 等），再加上 v2 自己的补充字段（`presentation` / `log` /
//! `private_reveals` / 9 类区计数）。
//! 契约清单：`docs/games/sanguosha/engine-replacement/view-required.json`
//! （机检 `check:sgs2-view-parity --strict`；字段集由 `tests/view_contract.rs` 冻结）。
//!
//! 隐藏信息（手牌明细、牌堆顺序、他人 pending 选项/数据、非主公身份、候选武将、私密展示）
//! **只出现在 `player_area`**；公共区只给数量/公共牌区。

use serde_json::{json, Map, Value};

use crate::cards::{DamageKind, TargetMode};
use crate::generals;
use crate::kernel::{CardId, Game, Slot, Suit, Zone};
use crate::skills;

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
        "def": g.name_of(id),
        "name": g.name_of(id),
        "name_is_virtual": g.virtual_defs.contains_key(id),
        "suit": suit_key(card.suit),
        "suit_label": card.suit.label(),
        "rank": card.rank,
        "kind": kind_key(def.kind),
        // v1 兼容：`category` / `equip_slot` / `range_value`（Web 牌面详情用）。
        "category": kind_key(def.kind),
        "equip_slot": def.slot.map(slot_key),
        "range_value": def.weapon_range,
        "range_type": if def.weapon_range.is_some() {
            json!("attack")
        } else if def.horse.is_some() {
            json!("defense")
        } else {
            Value::Null
        },
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

/// 可见计数标记注册表（counters 表示法）。新增标记必须在此登记，
/// 否则 `check:sgs2-conventions` 会失败（防「技能加了标记但 Web/AI 看不到」）。
pub const VISIBLE_COUNTER_MARKS: &[&str] = &[
    "ju",     // 橘（陆绩）
    "quan",   // 权（钟会）
    "chuang", // 创（周泰·不屈）
    "ni",     // 逆（刘封·陷嗣）
    "yi",     // 裔（界袁绍·血裔）
    "bian",   // 变（界张郃·巧变）
    "zai",    // 辎（界徐晃·截辎）
];

fn marks_count_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    for (code, n) in &g.seats[seat].counters {
        if VISIBLE_COUNTER_MARKS.contains(&code.as_str()) && *n > 0 {
            out.insert(code.clone(), json!(n));
        }
    }
    // 巧（界简雍）：回合内累计、回合切换清空（`turn.` 前缀计数），单独投影为标记。
    let qiao = g.counter(&format!("turn.qiao:{seat}"));
    if qiao > 0 {
        out.insert("qiao".to_string(), json!(qiao));
    }
    // 护甲（矢北）：复用标记区显示「护N」。
    if g.seats[seat].armor > 0 {
        out.insert("hu".to_string(), json!(g.seats[seat].armor));
    }
    Value::Object(out)
}

fn transforms_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    for code in skills::TRANSFORM_SKILLS {
        if !skills::active(g, seat, code) {
            continue;
        }
        out.insert(
            (*code).to_string(),
            json!(if g.yang(seat, code) { "yang" } else { "yin" }),
        );
    }
    Value::Object(out)
}

/// 武将名：`sgs_hero_N` → 官方名；未知 code 原样返回（夹具/化身）。
fn general_name(code: &str) -> String {
    if code.is_empty() {
        return String::new();
    }
    generals::by_code(code)
        .map(|g| g.name.to_string())
        .unwrap_or_else(|| code.to_string())
}

fn general_def(code: &str) -> Option<&'static generals::GeneralDef> {
    generals::by_code(code)
}

/// 技能列表（公开信息）：code → `{code, name, type, implemented}`（Web 契约）。
fn skills_view(g: &Game, seat: usize) -> Value {
    let s = &g.seats[seat];
    let codes: Vec<&str> = s
        .skills
        .iter()
        .chain(s.granted.iter())
        .map(String::as_str)
        .collect();
    let mut out: Vec<Value> = Vec::new();
    for code in codes {
        if out.iter().any(|v| v["code"] == json!(code)) {
            continue;
        }
        let (name, kind) = skills::def(code)
            .map(|d| (d.name, "normal"))
            .unwrap_or((code, "normal"));
        out.push(json!({
            "code": code,
            "name": name,
            "type": kind,
            "implemented": skills::def(code).is_some(),
        }));
    }
    Value::Array(out)
}

/// 武将是否已对公共区公开：选将阶段只公开主公已选；进入回合/终局后全员公开。
fn public_general(g: &Game, seat: usize) -> Option<String> {
    if g.lobby.is_some() {
        return None;
    }
    if let Some(setup) = &g.setup {
        if setup.lord == seat {
            return setup.picks.get(&seat).cloned();
        }
        return None;
    }
    let code = g.seats[seat].general.clone();
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

/// 阶段名（v1 兼容：lobby / setup / prepare / judge / draw / play / discard / end）。
pub fn phase_label(g: &Game) -> &'static str {
    if g.abort_reason.is_some() && g.lobby.is_none() && g.setup.is_none() {
        return "finished";
    }
    if g.lobby.is_some() {
        return "lobby";
    }
    if g.setup.is_some() {
        return "setup";
    }
    match g.phase {
        crate::kernel::Phase::Prepare => "prepare",
        crate::kernel::Phase::Judge => "judge",
        crate::kernel::Phase::Draw => "draw",
        crate::kernel::Phase::Play => "play",
        crate::kernel::Phase::Discard => "discard",
        crate::kernel::Phase::End => "end",
    }
}

fn public_seat(g: &Game, seat: usize) -> Value {
    let s = &g.seats[seat];
    let code = public_general(g, seat);
    let def = code.as_deref().and_then(general_def);
    let has_skills = code.is_some();
    json!({
        "seat": seat,
        "player_id": s.name,
        // 仅主公身份公开；死亡后公开身份（规则 §8.3）。
        "identity": if s.identity == crate::identity::LORD || s.hp <= 0 { Some(s.identity.clone()) } else { None },
        "general": code,
        "general_name": code.as_deref().map(general_name).unwrap_or_default(),
        "kingdom": def.map(|d| d.kingdom).or(Some(s.kingdom.as_str())),
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
        "is_bot": s.bot,
        "has_picked": code.is_some(),
        "skills": if has_skills { skills_view(g, seat) } else { json!([]) },
        "transforms": transforms_view(g, seat),
    })
}

fn pending_public(g: &Game) -> Value {
    match &g.pending {
        // v1 契约：`actor` 是 **player_id**（不是座位下标）；Web/网关按 pid 展示与提交。
        Some(p) => json!({
            "kind": p.kind(),
            "actor": g.seats.get(p.actor()).map(|s| s.name.clone()),
            "skill": p.skill(),
        }),
        None => Value::Null,
    }
}

fn pending_private(g: &Game, viewer: Option<usize>) -> Value {
    match (&g.pending, viewer) {
        (Some(p), Some(v)) if p.actor() == v => json!({
            "kind": p.kind(),
            "actor": g.seats.get(p.actor()).map(|s| s.name.clone()),
            "skill": p.skill(),
            // 强制选择类待办的占位出口不下发（端侧不展示；服务端提交时会自动改选实质选项）。
            "options": p.options().iter().filter(|o| !(p.escape_hidden() && o.id == p.escape_id())).map(|o| json!({ "id": o.id, "label": o.label })).collect::<Vec<_>>(),
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

/// v1 兼容的出牌历史（Web 倒序展示 `history[].text`）。
fn history_view(g: &Game) -> Value {
    Value::Array(
        g.log
            .iter()
            .map(|e| {
                json!({
                    "seq": e.seq,
                    "turn": g.round,
                    "actor": Value::Null,
                    "text": e.text,
                })
            })
            .collect(),
    )
}

/// 胜方：`{team, players}`（v1 契约；Web 只读 `team`）。
fn winner_view(g: &Game) -> Value {
    match &g.winner {
        Some(team) => {
            let seats: Vec<String> = g
                .seats
                .iter()
                .filter(|s| match team.as_str() {
                    crate::identity::LORD => {
                        matches!(
                            s.identity.as_str(),
                            crate::identity::LORD | crate::identity::LOYALIST
                        )
                    }
                    other => s.identity == other,
                })
                .map(|s| s.name.clone())
                .collect();
            json!({ "team": team, "players": seats })
        }
        None => Value::Null,
    }
}

fn setup_public(g: &Game) -> Value {
    match &g.setup {
        Some(s) => json!({
            "lord": g.seats.get(s.lord).map(|x| x.name.clone()),
            "lord_picked": s.lord_picked,
            "picked_count": s.picks.len(),
        }),
        None => Value::Null,
    }
}

fn config_view(g: &Game) -> Value {
    json!({
        "max_hand_cards": g.cfg.max_hand_cards,
        "lord_candidate_count": g.cfg.lord_candidate_count,
        "other_candidate_count": g.cfg.other_candidate_count,
        "double_spy": g.cfg.double_spy,
        "packs": g.cfg.packs,
        "general_packs": g.cfg.general_packs,
        "min_players": crate::lobby::MIN_PLAYERS,
        "max_players": crate::lobby::MAX_PLAYERS,
    })
}

fn root(g: &Game, seat: Option<usize>) -> Value {
    let discard = g.discard_pile();
    let stage = phase_label(g);
    let turn_name = g.seats.get(g.turn).map(|s| s.name.clone());
    json!({
        "schema_version": SCHEMA_VERSION,
        // v1 兼容分派键（Web `index.ts` 按 `data.game === "sanguosha"` 走三分区页）。
        "game": "sanguosha",
        "game_id": "sanguosha",
        "turn": g.turn,
        "phase": stage,
        "round": g.round,
        "public_board": {
            "phase": stage,
            "round": g.round,
            "turn_index": g.turn_seq,
            "turn": {
                "seat": g.turn,
                "phase": stage,
                "player_id": turn_name,
            },
            "config": config_view(g),
            "seat_count": g.seats.len(),
            "seats": (0..g.seats.len()).map(|s| public_seat(g, s)).collect::<Vec<_>>(),
            "deck": {
                "draw": g.ids_in(|z| *z == Zone::Draw).len(),
                "discard": discard.len(),
                "processing": g.ids_in(|z| *z == Zone::Processing).len(),
                "void": g.ids_in(|z| *z == Zone::Void).len(),
            },
            "draw_count": g.ids_in(|z| *z == Zone::Draw).len(),
            "void_count": g.ids_in(|z| *z == Zone::Void).len(),
            "processing_count": g.ids_in(|z| *z == Zone::Processing).len(),
            "frame_count": g.ids_in(|z| z.is_frame()).len(),
            "discard_top": discard.last().map(|id| card_view(g, id)),
            "discard": cards_view(g, &discard),
            "revealed": cards_view(g, &g.revealed.iter().cloned().collect::<Vec<_>>()),
            "pending": pending_public(g),
            "setup": setup_public(g),
            "winner": winner_view(g),
            "aborted": g.abort_reason,
            "history": history_view(g),
            "presentation": presentation_view(g),
            "log": log_view(g),
            "finished": g.finished,
        },
        // v1 契约：公共视图（无 viewer）的 `player_area` 为 **null**（不是空对象）。
        "player_area": match seat {
            Some(_) => player_area(g, seat),
            None => Value::Null,
        },
        "special_zones": special_zones(g, seat),
    })
}

fn player_area(g: &Game, seat: Option<usize>) -> Value {
    let Some(seat) = seat else {
        return json!({
            "seat": Value::Null,
            "player_id": Value::Null,
            "identity": Value::Null,
            "hand": [],
            "hand_limit": Value::Null,
            "private_reveals": [],
            "pending": Value::Null,
            "setup": Value::Null,
        });
    };
    let s = &g.seats[seat];
    let picked = g
        .setup
        .as_ref()
        .and_then(|st| st.picks.get(&seat).cloned());
    let code = if s.general.is_empty() {
        picked.clone().unwrap_or_default()
    } else {
        s.general.clone()
    };
    let def = general_def(&code);
    let setup = match &g.setup {
        Some(st) => {
            let offers = st.offers.get(&seat).cloned().unwrap_or_default();
            let details: Vec<Value> = offers
                .iter()
                .map(|c| {
                    let d = general_def(c);
                    json!({
                        "code": c,
                        "name": d.map(|x| x.name).unwrap_or(c.as_str()),
                        "kingdom": d.map(|x| x.kingdom),
                        "max_hp": d.map(|x| x.max_hp),
                        "is_lord": d.map(|x| x.lord).unwrap_or(false),
                        "skills": d.map(|x| x.skills.to_vec()).unwrap_or_default(),
                    })
                })
                .collect();
            json!({
                "candidates": offers,
                "candidate_details": details,
                "picked": st.picks.get(&seat),
                "is_lord": st.lord == seat,
            })
        }
        None => Value::Null,
    };
    json!({
        "seat": seat,
        "player_id": s.name,
        "identity": s.identity,
        "general": if code.is_empty() { Value::Null } else { json!(code) },
        "general_gid": def.map(|d| d.gid),
        "kingdom": def.map(|d| d.kingdom).or(Some(s.kingdom.as_str())),
        "general_name": general_name(&code),
        "hp": s.hp,
        "max_hp": s.max_hp,
        "hand": cards_view(g, &g.hand(seat)),
        "hand_limit": g.hand_limit(seat),
        "equips": equips_view(g, seat),
        "judge_zone": cards_view(g, &g.judge_zone_lifo(seat)),
        "marks": marks_view(g, seat),
        "marks_count": marks_count_view(g, seat),
        "transforms": transforms_view(g, seat),
        "abolished_slots": s
            .abolished_slots
            .iter()
            .map(|slot| slot_key(*slot).to_string())
            .collect::<Vec<_>>(),
        "judge_abolished": s.judge_abolished,
        "flipped": s.flipped,
        "chained": s.chained,
        "skills": skills_view(g, seat),
        // 装备技能（P4.5）：v2 尚未注册「装备技能」表 → 空数组（Web 兼容）。
        "equipment_skills": [],
        "is_bot": s.bot,
        "is_current": g.turn == seat && g.lobby.is_none() && g.setup.is_none(),
        "pending": pending_private(g, Some(seat)),
        "setup": setup,
        "private_reveals": cards_view(
            g,
            &g.private_reveals
                .get(&seat)
                .map(|set| set.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
    })
}

/// 特殊交互区（v1 兼容 shape）：
/// - `wugu`（五谷丰登，公开）：当前 pending 是 `harvest` 时投影展示牌/选牌顺序；
/// - `guanxing`（观星，owner 可见）：pending.skill == "guanxing" 时投影私密展示牌；
/// - `huashen`（化身，owner 可见）：界左慈的化身牌与当前展示。
///
/// 说明：v2 的观星交互是「逐张顶/底」（pending `choice`），不是 v1 的 `arrange` 网格；
/// 这里仍给出 `cards` 供前端展示，具体按钮用 pending options。
fn special_zones(g: &Game, viewer: Option<usize>) -> Value {
    let mut out = Map::new();
    if let Some(p) = &g.pending {
        match p.kind() {
            "harvest" => {
                let pool: Vec<CardId> = p.data()["pool"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let pickers: Vec<String> = p.data()["pickers"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as usize))
                            .filter_map(|i| g.seats.get(i).map(|s| s.name.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                out.insert(
                    "wugu".to_string(),
                    json!({
                        "visibility": "public",
                        "cards": cards_view(g, &pool),
                        "choosers": pickers,
                        "cursor": p.data()["cursor"].as_u64().unwrap_or(0),
                        "current": g.seats.get(p.actor()).map(|s| s.name.clone()),
                        "picked": [],
                        "interaction": { "kind": "one", "commit": "pick" },
                    }),
                );
            }
            _ if p.skill() == Some("guanxing") => {
                if let Some(v) = viewer {
                    if v == p.actor() {
                        let cards = g
                            .private_reveals
                            .get(&v)
                            .map(|set| set.iter().cloned().collect::<Vec<_>>())
                            .unwrap_or_default();
                        out.insert(
                            "guanxing".to_string(),
                            json!({
                                "owner": g.seats[v].name,
                                "cards": cards_view(g, &cards),
                                "visibility": "owner",
                                "rows": 2,
                                "cols": 5,
                                "slots": {},
                                "rev": 0,
                                "skill": "guanxing",
                                // v2 交互：逐张选择「置于牌堆顶 / 牌堆底」（pending options）。
                                "interaction": { "kind": "one", "commit": "choose" },
                            }),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    // 化身（界左慈）：只对本人可见。
    if let Some(v) = viewer {
        if let Some(h) = g.huashen.get(&v) {
            if !h.cards.is_empty() {
                let cards: Vec<Value> = h
                    .cards
                    .iter()
                    .map(|code| {
                        let d = general_def(code);
                        let skills: Vec<&str> = d.map(|x| x.skills.to_vec()).unwrap_or_default();
                        json!({
                            "code": code,
                            "name": d.map(|x| x.name).unwrap_or(code.as_str()),
                            "kingdom": d.map(|x| x.kingdom),
                            "skills": skills,
                            "active": h.active.as_deref() == Some(code.as_str()),
                        })
                    })
                    .collect();
                out.insert(
                    "huashen".to_string(),
                    json!({
                        "visibility": "owner",
                        "owner": g.seats[v].name,
                        "cards": cards,
                        "active": h.active,
                    }),
                );
            }
        }
    }
    Value::Object(out)
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
