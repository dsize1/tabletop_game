//! 视图投影（V3 → **v1/v2 同形 schema**）：`public_board` / `player_area` / `special_zones`。
//!
//! Web（`apps/web/src/view/sgs/*` + `client/sgs.ts`）按 `data.game === "sanguosha"` 走三分区页，
//! 因此 V3 接管 `sanguosha` 时必须输出同一份字段（契约：`apps/web/src/view/sgs/types.ts`）。
//!
//! 隐藏信息规则（constitution I3）：手牌明细 / 候选人 / pending options/data 只出现在
//! `player_area`；公共区只给数量与公共牌区。

use serde_json::{json, Map, Value};

use crate::kernel::cards::Category;
use crate::kernel::generals;
use crate::kernel::{cards, Game, Slot, Zone};

use super::lobby::{Lobby, LobbyConfig, Setup, MAX_PLAYERS, MIN_PLAYERS};
use super::text;

pub const SCHEMA_VERSION: u32 = 1;

fn suit_key(suit: crate::kernel::Suit) -> &'static str {
    super::lobby::suit_key(suit)
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

fn category_key(c: Category) -> &'static str {
    match c {
        Category::Basic => "basic",
        Category::Trick => "trick",
        Category::Equip => "equip",
        Category::DelayedTrick => "trick",
    }
}

fn target_mode_key(mode: cards::TargetMode) -> &'static str {
    match mode {
        cards::TargetMode::None => "none",
        cards::TargetMode::Single => "single",
        cards::TargetMode::Chosen => "chosen",
        cards::TargetMode::AllAlive => "all",
        cards::TargetMode::SelfOnly => "self",
    }
}

fn damage_key(kind: crate::kernel::DamageKind) -> &'static str {
    match kind {
        crate::kernel::DamageKind::Normal => "normal",
        crate::kernel::DamageKind::Fire => "fire",
        crate::kernel::DamageKind::Thunder => "thunder",
    }
}

/// 牌实例视图（`def` = 定义 code，`name` = 中文名；Web `judgeIcon` 读 `def`）。
pub fn card_view(g: &Game, id: &str) -> Value {
    let Some(card) = g.card_defs.get(id) else {
        return json!({ "id": id, "missing": true });
    };
    let def = cards::def(&card.def);
    let name = def.map(|d| d.name).unwrap_or(card.def.as_str());
    let category = def.map(|d| d.category).unwrap_or(Category::Trick);
    let slot = def.and_then(|d| d.slot);
    let range_value = def.and_then(|d| d.weapon_range);
    let horse = def.and_then(|d| d.horse);
    json!({
        "id": id,
        "def": card.def,
        "name": name,
        "name_is_virtual": card.def == "sha" && id.starts_with("v:"),
        "suit": suit_key(card.suit),
        "suit_label": card.suit.label(),
        "rank": card.rank,
        "category": category_key(category),
        "kind": category_key(category),
        "equip_slot": slot.map(slot_key),
        "slot": slot.map(slot_key),
        "range_value": range_value.or(horse),
        "range_type": if range_value.is_some() {
            json!("attack")
        } else if horse.is_some() {
            json!("defense")
        } else {
            Value::Null
        },
        "effect": def.map(|d| format!("{:?}", d.effect)),
        "damage_kind": damage_key(def.map(|d| d.damage_kind).unwrap_or_default()),
        "target_mode": target_mode_key(def.map(|d| d.target_mode).unwrap_or(cards::TargetMode::Chosen)),
        "delayed": def.map(|d| d.is_delayed()).unwrap_or(false),
        "revealed": false,
        "marks": g.card_mark(id).map(|m| json!([m])).unwrap_or_else(|| json!([])),
    })
}

fn cards_view(g: &Game, ids: &[String]) -> Value {
    Value::Array(ids.iter().map(|id| card_view(g, id)).collect())
}

fn sorted_zone(g: &Game, pred: impl Fn(&Zone) -> bool) -> Vec<String> {
    let mut ids: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| pred(z))
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    ids
}

fn equips_view(g: &Game, seat: usize) -> Value {
    let mut m = Map::new();
    for slot in SLOTS {
        let id = g
            .cards
            .iter()
            .find(|(_, z)| matches!(z, Zone::Equip(s, sl) if *s == seat && *sl == slot))
            .map(|(id, _)| id.clone());
        m.insert(
            slot_key(slot).to_string(),
            id.map(|id| card_view(g, &id)).unwrap_or(Value::Null),
        );
    }
    Value::Object(m)
}

/// 判定区按 LIFO（后置先判）投影。
fn judge_zone_view(g: &Game, seat: usize) -> Value {
    let mut ids = g.judge_stacks.get(&seat).cloned().unwrap_or_default();
    ids.reverse();
    cards_view(g, &ids)
}

fn marks_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    let mut codes: Vec<String> = Vec::new();
    for (_id, z) in g.cards.iter() {
        if let Zone::Mark(s, code) = z {
            if *s == seat && !codes.contains(code) {
                codes.push(code.clone());
            }
        }
    }
    for code in codes {
        let ids = sorted_zone(
            g,
            |z| matches!(z, Zone::Mark(s, c) if *s == seat && *c == code),
        );
        out.insert(code, cards_view(g, &ids));
    }
    Value::Object(out)
}

/// 可见计数标记白名单（技能接入后追加；`turn.*`/`phase.*` 内部计数不投影）。
pub const VISIBLE_COUNTER_MARKS: &[&str] = &[];

fn marks_count_view(g: &Game, seat: usize) -> Value {
    let mut out = Map::new();
    if let Some(s) = g.seats.get(seat) {
        for (code, n) in &s.counters {
            if VISIBLE_COUNTER_MARKS.contains(&code.as_str()) && *n > 0 {
                out.insert(code.clone(), json!(n));
            }
        }
    }
    Value::Object(out)
}

fn general_name(code: &str) -> String {
    if code.is_empty() {
        return String::new();
    }
    generals::def(code)
        .map(|g| g.name.to_string())
        .unwrap_or_else(|| code.to_string())
}

fn general_gid(code: &str) -> Option<u32> {
    generals::def(code).map(|g| g.gid)
}

/// 技能列表：自带 ∪ 授予（显示名含升级技，如「复难plus」）。
fn skills_view(g: &Game, seat: usize) -> Value {
    let Some(s) = g.seats.get(seat) else {
        return json!([]);
    };
    let mut codes: Vec<&str> = s.skills.iter().map(|c| c.as_str()).collect();
    codes.extend(s.granted.iter().map(|c| c.as_str()));
    let mut out: Vec<Value> = codes
        .into_iter()
        .filter(|c| crate::kernel::skills::def(c).is_some())
        .map(|c| {
            json!({
                "code": c,
                "name": crate::kernel::skills::display_name(g, seat, c),
            })
        })
        .collect();
    out.sort_by_key(|v| v.get("code").and_then(|c| c.as_str()).unwrap_or("").to_string());
    out.dedup();
    json!(out)
}

fn public_general(g: &Game, setup: Option<&Setup>, seat: usize) -> Option<String> {
    if let Some(st) = setup {
        if st.lord == seat {
            return st.picks.get(&seat).cloned();
        }
        return None;
    }
    let code = g.seats.get(seat).map(|s| s.general.clone()).unwrap_or_default();
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

pub fn phase_label(g: &Game, lobby: Option<&Lobby>, setup: Option<&Setup>) -> &'static str {
    if g.abort_reason.is_some() && lobby.is_none() && setup.is_none() {
        return "finished";
    }
    if lobby.is_some() {
        return "lobby";
    }
    if setup.is_some() {
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

fn hand_limit_view(g: &Game, cfg: &LobbyConfig, seat: usize) -> usize {
    let limit = g.hand_limit(seat);
    if cfg.max_hand_cards > 0 {
        limit.min(cfg.max_hand_cards as usize)
    } else {
        limit
    }
}

fn public_seat(g: &Game, cfg: &LobbyConfig, setup: Option<&Setup>, seat: usize) -> Value {
    let s = &g.seats[seat];
    let code = public_general(g, setup, seat);
    let def = code.as_deref().and_then(generals::def);
    let picked = !s.general.is_empty() || setup.is_some_and(|st| st.picks.contains_key(&seat));
    let identity_public = s.identity == Some(crate::kernel::Identity::Lord) || !s.alive;
    json!({
        "seat": seat,
        "player_id": s.name,
        "identity": if identity_public { s.identity.map(|i| format!("{i:?}").to_lowercase()) } else { None },
        "general": code,
        "general_name": code.as_deref().map(general_name).unwrap_or_default(),
        "kingdom": def.map(|d| d.kingdom).or(Some(s.kingdom.as_str())),
        "hp": if picked { json!(s.hp) } else { Value::Null },
        "max_hp": if picked { json!(s.max_hp) } else { Value::Null },
        "hand_count": g.hand_size(seat),
        "hand_limit": hand_limit_view(g, cfg, seat),
        "equips": equips_view(g, seat),
        "judge_zone": judge_zone_view(g, seat),
        "marks": marks_view(g, seat),
        "marks_count": marks_count_view(g, seat),
        "abolished_slots": Vec::<String>::new(),
        "judge_abolished": false,
        "flipped": s.flipped,
        "chained": s.chained,
        "alive": if picked { json!(s.alive) } else { Value::Null },
        "is_bot": s.bot,
        "has_picked": code.is_some(),
        "skills": if code.is_some() { skills_view(g, seat) } else { json!([]) },
        "transforms": json!({}),
    })
}

fn pending_public(g: &Game) -> Value {
    match &g.pending {
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
            "options": p
                .options()
                .iter()
                .filter(|o| !(p.escape_hidden() && o.id == p.escape_id()))
                .map(|o| json!({ "id": o.id, "label": o.label }))
                .collect::<Vec<_>>(),
            "escape": p.escape_id(),
            "data": p.data(),
        }),
        _ => Value::Null,
    }
}

fn setup_public(g: &Game, setup: Option<&Setup>) -> Value {
    match setup {
        Some(s) => json!({
            "lord": g.seats.get(s.lord).map(|x| x.name.clone()),
            "lord_picked": s.lord_picked,
            "picked_count": s.picks.len(),
        }),
        None => Value::Null,
    }
}

fn config_view(cfg: &LobbyConfig) -> Value {
    json!({
        "max_hand_cards": cfg.max_hand_cards,
        "lord_candidate_count": cfg.lord_candidate_count,
        "other_candidate_count": cfg.other_candidate_count,
        "double_spy": cfg.double_spy,
        "packs": cfg.packs,
        "general_packs": cfg.general_packs,
        "min_players": MIN_PLAYERS,
        "max_players": MAX_PLAYERS,
    })
}

fn history_view(g: &Game) -> Value {
    Value::Array(
        text::history(g)
            .into_iter()
            .map(|(seq, text)| {
                json!({
                    "seq": seq,
                    "turn": g.round,
                    "actor": Value::Null,
                    "text": text,
                })
            })
            .collect(),
    )
}

fn winner_view(g: &Game) -> Value {
    match g.winner {
        Some(w) => {
            let team = match w {
                crate::kernel::Winner::Lord => "lord",
                crate::kernel::Winner::Rebel => "rebel",
                crate::kernel::Winner::Spy => "spy",
            };
            let players: Vec<String> = g
                .seats
                .iter()
                .filter(|s| {
                    let id = format!("{:?}", s.identity.unwrap_or(crate::kernel::Identity::Rebel));
                    match w {
                        crate::kernel::Winner::Lord => id == "Lord" || id == "Loyalist",
                        crate::kernel::Winner::Rebel => id == "Rebel",
                        crate::kernel::Winner::Spy => id == "Spy",
                    }
                })
                .map(|s| s.name.clone())
                .collect();
            json!({ "team": team, "players": players })
        }
        None => Value::Null,
    }
}

fn player_area(g: &Game, cfg: &LobbyConfig, setup: Option<&Setup>, seat: usize) -> Value {
    let s = &g.seats[seat];
    let code = if s.general.is_empty() {
        setup
            .and_then(|st| st.picks.get(&seat).cloned())
            .unwrap_or_default()
    } else {
        s.general.clone()
    };
    let def = generals::def(&code);
    let setup_view = match setup {
        Some(st) => {
            let offers = st.offers.get(&seat).cloned().unwrap_or_default();
            let details: Vec<Value> = offers
                .iter()
                .map(|c| {
                    let d = generals::def(c);
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
        "identity": s.identity.map(|i| format!("{i:?}").to_lowercase()),
        "general": if code.is_empty() { Value::Null } else { json!(code) },
        "general_gid": general_gid(&code),
        "kingdom": def.map(|d| d.kingdom).or(Some(s.kingdom.as_str())),
        "general_name": general_name(&code),
        "hp": if code.is_empty() { Value::Null } else { json!(s.hp) },
        "max_hp": if code.is_empty() { Value::Null } else { json!(s.max_hp) },
        "hand": cards_view(g, &g.hand(seat)),
        "hand_limit": hand_limit_view(g, cfg, seat),
        "equips": equips_view(g, seat),
        "judge_zone": judge_zone_view(g, seat),
        "marks": marks_view(g, seat),
        "marks_count": marks_count_view(g, seat),
        "transforms": json!({}),
        "abolished_slots": Vec::<String>::new(),
        "judge_abolished": false,
        "flipped": s.flipped,
        "chained": s.chained,
        "skills": skills_view(g, seat),
        "equipment_skills": [],
        "is_bot": s.bot,
        "is_current": g.turn == seat && setup.is_none(),
        "pending": pending_private(g, Some(seat)),
        "setup": setup_view,
        "private_reveals": [],
    })
}

/// 特殊交互区：五谷丰登选牌（v3 pending kind = `sgs3_harvest`）。
fn special_zones(g: &Game, viewer: Option<usize>) -> Value {
    let mut out = Map::new();
    if let Some(p) = &g.pending {
        if p.kind() == "sgs3_harvest" {
            let pool: Vec<String> = p.data()["revealed"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            // 选牌顺序：使用者起顺时针的存活角色。
            let order: Vec<String> = {
                let actor = p.actor();
                let n = g.seats.len();
                (0..n)
                    .map(|k| (actor + k) % n)
                    .filter(|s| g.seats[*s].alive)
                    .filter_map(|s| g.seats.get(s).map(|x| x.name.clone()))
                    .collect()
            };
            out.insert(
                "wugu".to_string(),
                json!({
                    "visibility": "public",
                    "cards": cards_view(g, &pool),
                    "choosers": order,
                    "cursor": p.data()["hidx"].as_u64().unwrap_or(0),
                    "current": g.seats.get(p.actor()).map(|s| s.name.clone()),
                    "picked": [],
                    "interaction": { "kind": "one", "commit": "pick" },
                }),
            );
        }
    }
    let _ = viewer;
    Value::Object(out)
}

/// 当前公开亮出的牌（五谷亮牌 / 火攻展示）。
fn revealed_view(g: &Game) -> Value {
    let mut ids: Vec<String> = Vec::new();
    if let Some(p) = &g.pending {
        if p.kind() == "sgs3_harvest" {
            if let Some(a) = p.data()["revealed"].as_array() {
                ids.extend(a.iter().filter_map(|v| v.as_str().map(str::to_string)));
            }
        }
        if let Some(id) = p.data()["shown"].as_str() {
            ids.push(id.to_string());
        }
    }
    cards_view(g, &ids)
}

fn root(
    g: &Game,
    cfg: &LobbyConfig,
    lobby: Option<&Lobby>,
    setup: Option<&Setup>,
    seat: Option<usize>,
) -> Value {
    let stage = phase_label(g, lobby, setup);
    let discard: Vec<String> = sorted_zone(g, |z| *z == Zone::Discard);
    let count = |pred: &dyn Fn(&Zone) -> bool| g.cards.values().filter(|z| pred(z)).count();
    let turn_name = g.seats.get(g.turn).map(|s| s.name.clone());
    json!({
        "schema_version": SCHEMA_VERSION,
        // Web 分派键：三分区交互页。
        "game": "sanguosha",
        "game_id": "sanguosha",
        "turn": g.turn,
        "phase": stage,
        "round": g.round,
        "endgame": {
            "no_heal": g.endgame_no_heal(),
            "round": g.round,
            "since": crate::kernel::ENDGAME_ROUND,
        },
        "public_board": {
            "phase": stage,
            "round": g.round,
            "turn_index": g.turn_seq,
            "turn": { "seat": g.turn, "phase": stage, "player_id": turn_name },
            "config": config_view(cfg),
            "seat_count": g.seats.len(),
            "seats": (0..g.seats.len()).map(|s| public_seat(g, cfg, setup, s)).collect::<Vec<_>>(),
            "deck": {
                "draw": count(&|z| matches!(z, Zone::Draw)),
                "discard": discard.len(),
                "processing": count(&|z| matches!(z, Zone::Processing)),
                "void": count(&|z| matches!(z, Zone::Void)),
            },
            "draw_count": count(&|z| matches!(z, Zone::Draw)),
            "void_count": count(&|z| matches!(z, Zone::Void)),
            "processing_count": count(&|z| matches!(z, Zone::Processing)),
            "frame_count": count(&|z| z.is_frame()),
            "discard_top": discard.last().map(|id| card_view(g, id)),
            "discard": cards_view(g, &discard),
            "revealed": revealed_view(g),
            "pending": pending_public(g),
            "setup": setup_public(g, setup),
            "winner": winner_view(g),
            "aborted": g.abort_reason,
            "history": history_view(g),
            "presentation": [],
            "log": [],
            "finished": g.finished(),
        },
        "player_area": match seat {
            Some(s) => player_area(g, cfg, setup, s),
            None => Value::Null,
        },
        "special_zones": special_zones(g, seat),
    })
}

pub fn public_state(g: &Game, cfg: &LobbyConfig, lobby: Option<&Lobby>, setup: Option<&Setup>) -> Value {
    root(g, cfg, lobby, setup, None)
}

pub fn player_view(
    g: &Game,
    cfg: &LobbyConfig,
    lobby: Option<&Lobby>,
    setup: Option<&Setup>,
    seat: usize,
) -> Value {
    root(g, cfg, lobby, setup, Some(seat))
}
