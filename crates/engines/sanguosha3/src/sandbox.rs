//! 工作台沙盒（P5 基座 V3）：建局 + 场景注入。
//!
//! 边界（与 v2 `sandbox` 同口径）：
//! - **只**用于 `/internal/game/preview/*`（本机工作台）；不写 `game_state` / 事件真相源；
//! - 场景注入只**移动已有牌**（牌堆/弃牌堆），不得凭空造牌；注入后由 `validate_state` 守恒校验；
//! - 同 seed + 同场景 → 同状态（审计不变量）。

use std::collections::{BTreeMap, BTreeSet};

use game_engine_core::RandomSource;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::engine::lobby::{build_deck, mount_general, LobbyConfig, INITIAL_HAND};
use crate::engine::EngineState;
use crate::kernel::cards;
use crate::kernel::flow;
use crate::kernel::resolve::{self, FlowKind, Stage};
use crate::kernel::turn;
use crate::kernel::{Game, Identity, Phase, Slot, Zone};

// ---------------------------------------------------------------------------
// 沙盒建局
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SandboxSpec {
    #[serde(default)]
    pub players: usize,
    #[serde(default)]
    pub seats: Vec<SandboxSeat>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SandboxSeat {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub general: Option<String>,
    #[serde(default)]
    pub identity: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// 建「已开局」沙盒（含武将/技能/牌堆/初始手牌）。玩家 id 固定 `s0..sN`，座位 0 主公。
pub fn new_sandbox_game(
    spec: &Value,
    rng: &mut dyn RandomSource,
) -> Result<EngineState, String> {
    let spec: SandboxSpec =
        serde_json::from_value(spec.clone()).map_err(|e| format!("沙盒参数非法：{e}"))?;
    let players = spec.players.max(1);
    let mut g = start_seats(players);
    for seat in &spec.seats {
        let idx = seat.index.unwrap_or(0);
        if idx >= players {
            return Err(format!("座位下标越界：{idx}"));
        }
        if let Some(name) = seat.name.as_deref().filter(|s| !s.is_empty()) {
            g.seats[idx].name = name.to_string();
        }
        if let Some(id) = seat.identity.as_deref() {
            g.seats[idx].identity = Some(parse_identity(id)?);
        }
        if let Some(code) = seat.general.as_deref().filter(|s| !s.is_empty()) {
            mount_general(&mut g, idx, code)?;
        }
    }
    let cfg = LobbyConfig::default();
    build_deck(&mut g, &cfg.packs, rng)?;
    for _ in 0..INITIAL_HAND {
        for i in 0..players {
            flow::draw(&mut g, i, 1);
        }
    }
    g.started = true;
    g.rng = rng.next_u64() | 1;
    crate::kernel::skills::fire_game_start(&mut g);
    // 进入主公回合（无场景时也能得到合法待办；场景注入会重新定位回合）。
    let _ = turn::start(&mut g);
    g.log.clear();
    g.events.clear();
    Ok(EngineState {
        kernel: g,
        cfg,
        lobby: None,
        setup: None,
    })
}

fn start_seats(players: usize) -> Game {
    let mut g = turn::new_game(players);
    for i in 0..players {
        let seat = &mut g.seats[i];
        seat.name = format!("s{i}");
        seat.bot = true;
        seat.identity = Some(if i == 0 {
            Identity::Lord
        } else {
            Identity::Rebel
        });
    }
    g
}

fn parse_identity(s: &str) -> Result<Identity, String> {
    Ok(match s {
        "lord" => Identity::Lord,
        "loyalist" => Identity::Loyalist,
        "rebel" => Identity::Rebel,
        "traitor" | "spy" => Identity::Spy,
        other => return Err(format!("未知身份：{other}")),
    })
}

// ---------------------------------------------------------------------------
// 场景注入
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SceneSpec {
    #[serde(default)]
    pub round: Option<u32>,
    #[serde(default)]
    pub turn: Option<usize>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub turn_phase: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub deck_top: Option<Vec<String>>,
    #[serde(default)]
    pub counters: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    pub seats: Vec<SeatScene>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeatScene {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub player_id: Option<String>,
    #[serde(default)]
    pub general: Option<String>,
    #[serde(default)]
    pub identity: Option<String>,
    #[serde(default)]
    pub hp: Option<i32>,
    #[serde(default)]
    pub max_hp: Option<i32>,
    #[serde(default)]
    pub is_bot: Option<bool>,
    #[serde(default)]
    pub chained: Option<bool>,
    #[serde(default)]
    pub flipped: Option<bool>,
    #[serde(default)]
    pub hand: Option<Vec<String>>,
    #[serde(default)]
    pub hand_append: Option<Vec<String>>,
    #[serde(default)]
    pub equips: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub judge: Option<Vec<String>>,
}

/// 场景注入：座位/区域/牌堆顶/回合阶段；末尾把回合机驱动到目标阶段。
pub fn apply_scene(es: &mut EngineState, scene: &Value) -> Result<(), String> {
    let spec: SceneSpec =
        serde_json::from_value(scene.clone()).map_err(|e| format!("场景解析失败：{e}"))?;
    let g = &mut es.kernel;
    if let Some(seed) = spec.seed {
        g.rng = seed | 1;
    }
    if let Some(round) = spec.round {
        g.round = round;
    }
    let turn_seat = spec.turn.unwrap_or(0);
    if turn_seat >= g.seats.len() {
        return Err(format!("回合座位越界：{turn_seat}"));
    }
    let target_phase = phase_of(
        spec.turn_phase
            .as_deref()
            .or(spec.phase.as_deref())
            .unwrap_or("play"),
    )?;

    let mut used: BTreeSet<String> = BTreeSet::new();
    // 先做座位（武将/身份 → 体力上限），再区域注入。
    let mut seat_scenes = spec.seats.clone();
    seat_scenes.sort_by_key(|s| s.index.unwrap_or(0));
    for seat_c in &seat_scenes {
        let idx = seat_index(g, seat_c)?;
        if let Some(code) = seat_c.general.as_deref().filter(|s| !s.is_empty()) {
            mount_general(g, idx, code)?;
        }
        if let Some(id) = seat_c.identity.as_deref() {
            g.seats[idx].identity = Some(parse_identity(id)?);
        }
        if let Some(mh) = seat_c.max_hp {
            g.seats[idx].max_hp = mh.max(1);
        }
        if let Some(hp) = seat_c.hp {
            g.seats[idx].hp = hp;
        }
        if let Some(bot) = seat_c.is_bot {
            g.seats[idx].bot = bot;
        }
        if let Some(v) = seat_c.chained {
            g.seats[idx].chained = v;
        }
        if let Some(v) = seat_c.flipped {
            g.seats[idx].flipped = v;
        }
        if let Some(hand) = &seat_c.hand {
            for id in g.hand(idx) {
                to_zone(g, &id, Zone::Discard);
            }
            for token in hand {
                let id = take_card(g, token, &mut used)?;
                to_zone(g, &id, Zone::Hand(idx));
            }
        }
        if let Some(append) = &seat_c.hand_append {
            for token in append {
                let id = take_card(g, token, &mut used)?;
                to_zone(g, &id, Zone::Hand(idx));
            }
        }
        if let Some(equips) = &seat_c.equips {
            for (key, token) in equips {
                let slot = slot_of(key).ok_or_else(|| format!("未知装备槽：{key}"))?;
                for old in crate::kernel::gear::equips_of(g, idx) {
                    if matches!(g.cards.get(&old), Some(Zone::Equip(s, sl)) if *s == idx && *sl == slot) {
                        to_zone(g, &old, Zone::Discard);
                    }
                }
                let id = take_card(g, token, &mut used)?;
                to_zone(g, &id, Zone::Equip(idx, slot));
            }
        }
        if let Some(judge) = &seat_c.judge {
            for id in judge_zone_cards(g, idx) {
                to_zone(g, &id, Zone::Discard);
            }
            let mut stack = Vec::new();
            for token in judge {
                let id = take_card(g, token, &mut used)?;
                to_zone(g, &id, Zone::Judge(idx));
                stack.push(id);
            }
            g.judge_stacks.insert(idx, stack);
        }
    }

    if let Some(top) = &spec.deck_top {
        // 数组首个 = 最上面；牌堆末尾是顶，所以逆序 push。
        let mut ids = Vec::new();
        for token in top {
            let id = take_card(g, token, &mut used)?;
            to_zone(g, &id, Zone::Draw);
            ids.push(id);
        }
        for id in ids.into_iter().rev() {
            g.deck.push(id);
        }
    }
    if let Some(counters) = &spec.counters {
        for (k, v) in counters {
            g.set_counter(&format!("turn.{k}"), *v as i32);
        }
    }

    position_turn(g, turn_seat, target_phase)?;
    Ok(())
}

fn phase_of(s: &str) -> Result<Phase, String> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "prepare" => Phase::Prepare,
        "judge" => Phase::Judge,
        "draw" => Phase::Draw,
        "play" => Phase::Play,
        "discard" => Phase::Discard,
        "end" => Phase::End,
        // v1 兼容：`turn` / `lobby` / `setup` / `finished` / `dying` 视为「出牌阶段」。
        "turn" | "lobby" | "setup" | "finished" | "dying" => Phase::Play,
        other => return Err(format!("未知阶段：{other}")),
    })
}

fn seat_index(g: &Game, scene: &SeatScene) -> Result<usize, String> {
    match (scene.index, scene.player_id.as_deref()) {
        (Some(i), Some(pid)) => {
            if g.seats.get(i).map(|s| s.name.as_str()) != Some(pid) {
                return Err(format!("座位 {i} 的玩家不是 {pid}"));
            }
            Ok(i)
        }
        (Some(i), None) => {
            if i >= g.seats.len() {
                return Err(format!("座位下标越界：{i}"));
            }
            Ok(i)
        }
        (None, Some(pid)) => g
            .seats
            .iter()
            .position(|s| s.name == pid)
            .ok_or_else(|| format!("找不到玩家：{pid}")),
        (None, None) => Err("座位注入必须给 index 或 player_id".into()),
    }
}

fn slot_of(key: &str) -> Option<Slot> {
    Some(match key {
        "weapon" => Slot::Weapon,
        "armor" => Slot::Armor,
        "horse_offense" => Slot::HorseOffense,
        "horse_defense" => Slot::HorseDefense,
        "treasure" => Slot::Treasure,
        _ => return None,
    })
}

fn judge_zone_cards(g: &Game, seat: usize) -> Vec<String> {
    g.cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Judge(s) if *s == seat))
        .map(|(id, _)| id.clone())
        .collect()
}

fn to_zone(g: &mut Game, id: &str, zone: Zone) {
    g.deck.retain(|c| c != id);
    g.cards.insert(id.to_string(), zone);
}

/// 牌令牌解析（与 v2 `take_card` 同口径）：按 def code / 名称匹配**牌堆/弃牌堆**里的未使用牌。
fn take_card(g: &Game, token: &str, used: &mut BTreeSet<String>) -> Result<String, String> {
    let mut cands: Vec<String> = Vec::new();
    for (id, def) in g.card_defs.iter() {
        if used.contains(id) {
            continue;
        }
        let hit = def.def == token
            || cards::def(&def.def)
                .map(|d| d.code == token || d.name == token)
                .unwrap_or(false);
        if !hit {
            continue;
        }
        match g.cards.get(id) {
            Some(Zone::Draw) | Some(Zone::Discard) => cands.push(id.clone()),
            _ => {}
        }
    }
    // 确定性：同 seed 必须选到同一张实例（`HashMap` 迭代序不可依赖）。
    cands.sort_by_key(|id| id_num(id));
    if let Some(id) = cands.into_iter().next() {
        used.insert(id.clone());
        return Ok(id);
    }
    Err(format!(
        "找不到可用的牌：{token}（不能从他人手牌/装备区取）"
    ))
}

/// 牌实例 id → 数值（`c12` > `c2`）；非该格式按 0。
fn id_num(id: &str) -> u64 {
    id.strip_prefix('c')
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

/// 把回合机定位到目标阶段（准备/判定/摸牌会被跳过，且不触发其阶段开始技）。
fn position_turn(g: &mut Game, seat: usize, target: Phase) -> Result<(), String> {
    g.stack.clear();
    g.pending = None;
    g.answer = None;
    g.answer_skill = None;
    g.events.clear();
    g.log.clear();
    turn::begin_turn(g, seat);
    for phase in turn::PHASES {
        if phase == target {
            break;
        }
        turn::skip_phase(g, phase);
    }
    resolve::push_flow(
        g,
        FlowKind::Turn,
        seat,
        Stage::PhaseStep,
        json!({ "seat": seat, "pi": 0, "entered": false }),
    );
    resolve::advance(g)
}

/// 座位摘要（预览工作台展示；不读私有信息）。
pub fn seat_summary(g: &Game) -> Value {
    let seats: Vec<Value> = g
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "index": i,
                "player_id": s.name,
                "general": s.general,
                "general_name": crate::kernel::generals::def(&s.general).map(|d| d.name).unwrap_or(s.general.as_str()),
                "identity": s.identity.map(|id| match id {
                    Identity::Lord => "lord",
                    Identity::Loyalist => "loyalist",
                    Identity::Rebel => "rebel",
                    Identity::Spy => "traitor",
                }),
                "hp": s.hp,
                "max_hp": s.max_hp,
                "alive": s.alive,
                "is_bot": s.bot,
            })
        })
        .collect();
    json!({
        "phase": format!("{:?}", g.phase).to_ascii_lowercase(),
        "turn": g.turn,
        "turn_phase": format!("{:?}", g.phase).to_ascii_lowercase(),
        "round": g.round,
        "seats": seats,
    })
}
