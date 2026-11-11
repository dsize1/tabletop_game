//! 工作台沙盒 + 场景注入（S3）：供 `crates/server/src/preview.rs` 使用。
//!
//! 两个入口：
//! - [`new_sandbox_game`]：按用户指定的玩家数 / 座位武将 / 身份直接生成**已开局**状态
//!   （跳过 lobby 与选将，复用 `lobby::finish_setup`）；
//! - [`apply_scene`]：在既有局面上注入「最小局面」（阶段/回合/手牌/装备/判定/标记）。
//!
//! 约束（与 v1 同口径）：
//! - 资源只做**移动**（牌从既有牌实例里取），不凭空造牌；注入后重新校验守恒；
//! - 场景字段白名单（`deny_unknown_fields`）；牌 token 只能从摸牌堆/弃牌堆/移出区取；
//! - 纯内存（预览宿主不落库）。

use std::collections::{BTreeMap, BTreeSet};

use game_engine_core::{EngineError, EngineEvent, GameState, RandomSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cards;
use crate::generals;
use crate::identity;
use crate::kernel::{Game, Phase, Seat, Slot, Zone};
use crate::lobby;
use crate::Sanguosha2Engine;

fn illegal(msg: impl Into<String>) -> EngineError {
    EngineError::IllegalAction(msg.into())
}

fn internal(msg: impl Into<String>) -> EngineError {
    EngineError::Internal(msg.into())
}

// ---------------------------------------------------------------------------
// 沙盒开局
// ---------------------------------------------------------------------------

/// 单个座位的沙盒配置（全部可选；缺省随机）。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxSeat {
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub player_id: Option<String>,
    /// 武将 code（必须在当前武将池内）。
    #[serde(default)]
    pub general: Option<String>,
    /// 身份（`lord` / `loyalist` / `rebel` / `traitor`；须符合标准配比）。
    #[serde(default)]
    pub identity: Option<String>,
}

/// 沙盒开局入参。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxSpec {
    pub players: usize,
    #[serde(default)]
    pub seats: Vec<SandboxSeat>,
}

/// 生成一局已开局状态（纯内存；不落库）。
pub fn new_sandbox_game(
    room_id: &str,
    group_id: &str,
    spec: &SandboxSpec,
    rng: &mut dyn RandomSource,
) -> Result<(GameState, Vec<EngineEvent>), EngineError> {
    let n = spec.players;
    if !(lobby::MIN_PLAYERS..=lobby::MAX_PLAYERS).contains(&n) {
        return Err(illegal(format!(
            "玩家人数需在 {}–{} 之间（当前 {n}）",
            lobby::MIN_PLAYERS,
            lobby::MAX_PLAYERS
        )));
    }
    // 1) 座位下标归一化。
    let mut resolved: Vec<Option<&SandboxSeat>> = vec![None; n];
    for (pos, seat) in spec.seats.iter().enumerate() {
        let idx = seat.index.unwrap_or(pos);
        if idx >= n {
            return Err(illegal(format!("座位 {} 越界（共 {n} 人）", idx + 1)));
        }
        if resolved[idx].is_some() {
            return Err(illegal(format!("座位 #{} 重复配置", idx + 1)));
        }
        resolved[idx] = Some(seat);
    }
    // 2) 玩家 id（默认 bot:N）。
    let mut players = Vec::with_capacity(n);
    let mut seen = BTreeSet::new();
    for (i, seat) in resolved.iter().enumerate() {
        let pid = seat
            .and_then(|s| s.player_id.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("bot:{}", i + 1));
        if !seen.insert(pid.clone()) {
            return Err(illegal(format!("玩家 id 重复：{pid}")));
        }
        players.push(pid);
    }

    // 3) 建局骨架（大厅态）→ 手工装配身份/武将 → 复用 finish_setup。
    let engine = Sanguosha2Engine;
    let (base_state, _) = game_engine_core::EnginePlugin::new_game(
        &engine,
        room_id,
        group_id,
        players.clone(),
        rng,
    );
    let mut g: Game = serde_json::from_value(base_state.data.clone()).map_err(|e| internal(format!("解析失败：{e}")))?;

    // 武将池：当前配置下可选且技能齐全。
    let pool: Vec<&'static str> = lobby::available_codes(&g.cfg);
    let mut requested_generals: Vec<Option<String>> = vec![None; n];
    let mut used = BTreeSet::new();
    for (i, seat) in resolved.iter().enumerate() {
        let Some(code) = seat
            .and_then(|s| s.general.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if !pool.iter().any(|c| *c == code) {
            return Err(illegal(format!(
                "武将不可用：{code}（须在 {} 位可选池内）",
                pool.len()
            )));
        }
        if !used.insert(code.to_string()) {
            return Err(illegal(format!("武将重复：{code}")));
        }
        requested_generals[i] = Some(code.to_string());
    }

    // 身份：标准配比池；指定身份从池中扣除，其余随机补足。
    let mut identity_pool: Vec<&'static str> =
        identity::identity_list(n, g.cfg.double_spy).map_err(illegal)?;
    let mut requested_identities: Vec<Option<String>> = vec![None; n];
    for (i, seat) in resolved.iter().enumerate() {
        let Some(id) = seat
            .and_then(|s| s.identity.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        // v1 兼容：内奸 code 在 v1 是 `spy`，v2 是 `traitor`。
        let id = if id == "spy" { "traitor" } else { id };
        let Some(pos) = identity_pool.iter().position(|x| *x == id) else {
            return Err(illegal(format!("身份超出标准配比：{id}")));
        };
        identity_pool.remove(pos);
        requested_identities[i] = Some(id.to_string());
    }
    identity::shuffle(&mut g, &mut identity_pool);
    for i in 0..n {
        g.seats[i].name = players[i].clone();
        g.seats[i].bot = true;
        g.seats[i].identity = match &requested_identities[i] {
            Some(id) => id.clone(),
            None => identity_pool
                .pop()
                .ok_or_else(|| internal("身份池不足（配比异常）"))?
                .to_string(),
        };
        g.seats[i].flags.remove(identity::LORD);
    }
    let lord = identity::lord_seat(&g).ok_or_else(|| internal("未分配主公"))?;
    g.seats[lord].flags.insert(identity::LORD.to_string());

    // 剩余武将随机（不重复）。
    let mut remaining: Vec<&'static str> = pool
        .into_iter()
        .filter(|c| !used.contains(*c))
        .collect();
    identity::shuffle(&mut g, &mut remaining);
    let mut picks = BTreeMap::new();
    for i in 0..n {
        let code = requested_generals[i]
            .clone()
            .or_else(|| remaining.pop().map(str::to_string))
            .ok_or_else(|| internal("武将池不足（无法为全部座位分配武将）"))?;
        picks.insert(i, code);
    }

    g.lobby = None;
    g.setup = Some(lobby::Setup {
        lord,
        offers: BTreeMap::new(),
        picks,
        lord_picked: true,
    });
    g.host = players[0].clone();
    g.host_like_players(&players);
    lobby::finish_setup(&mut g).map_err(internal)?;

    let events = vec![EngineEvent::new(
        "sandbox_created",
        json!({ "players": n, "all_bots": true }),
    )];
    let state = crate::state_of_public(room_id, group_id, players, &g);
    validate_game(&g).map_err(internal)?;
    Ok((state, events))
}

/// 沙盒表单元数据（武将 = 当前可选池；身份 = 标准配比）。
pub fn sandbox_meta() -> Value {
    let cfg = lobby::LobbyConfig::default();
    let generals: Vec<Value> = lobby::available_codes(&cfg)
        .into_iter()
        .filter_map(generals::by_code)
        .map(|g| {
            json!({
                "code": g.code,
                "name": g.name,
                "kingdom": g.kingdom,
                "pack": g.pack,
                "max_hp": g.max_hp,
                "is_lord": g.lord,
                "skills": g.skills,
            })
        })
        .collect();
    let mut identities = serde_json::Map::new();
    for n in lobby::MIN_PLAYERS..=lobby::MAX_PLAYERS {
        if let Ok(list) = identity::identity_list(n, false) {
            let mut counts: BTreeMap<String, u32> = BTreeMap::new();
            for id in list {
                *counts.entry(id.to_string()).or_insert(0) += 1;
            }
            // 中台/统计仍用 v1 的 `spy`：同时给出别名，避免中台「剩余内奸」恒为 0。
            if let Some(t) = counts.get("traitor").copied() {
                counts.insert("spy".to_string(), t);
            }
            identities.insert(n.to_string(), json!(counts));
        }
    }
    json!({
        "players": [lobby::MIN_PLAYERS, 6, 7, lobby::MAX_PLAYERS],
        "generals": generals,
        "identities": identities,
        "packs": {
            "cards": cards::pack_keys(),
            "general_packs": cfg.general_packs,
        },
    })
}

// ---------------------------------------------------------------------------
// 场景注入
// ---------------------------------------------------------------------------

/// 场景注入入参（全部字段可选）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    #[serde(default)]
    pub round: Option<u32>,
    #[serde(default)]
    pub turn: Option<usize>,
    /// 阶段：`prepare` / `judge` / `draw` / `play` / `discard` / `end`。
    ///
    /// 兼容 v1 场景：`turn` / `dying` / `lobby` / `setup` / `finished` 视为「不改阶段」，
    /// 具体阶段由 `turn_phase` 指定。
    #[serde(default)]
    pub phase: Option<String>,
    /// v1 兼容字段：`prepare` / `judge` / `draw` / `play` / `discard` / `end`（等价 `phase`）。
    #[serde(default)]
    pub turn_phase: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    /// 牌堆顶（数组末尾 = 最上面；token 规则见 `take_card`）。
    #[serde(default)]
    pub deck_top: Option<Vec<String>>,
    /// 本回合计数器（合并到 `turn.*`）。
    #[serde(default)]
    pub counters: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    pub seats: Vec<SeatScene>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub alive: Option<bool>,
    #[serde(default)]
    pub is_bot: Option<bool>,
    #[serde(default)]
    pub chained: Option<bool>,
    #[serde(default)]
    pub flipped: Option<bool>,
    /// 手牌：**替换**。
    #[serde(default)]
    pub hand: Option<Vec<String>>,
    /// 手牌：追加。
    #[serde(default)]
    pub hand_append: Option<Vec<String>>,
    /// 装备：按槽位替换（`weapon` / `armor` / `horse_offense` / `horse_defense` / `treasure`）。
    #[serde(default)]
    pub equips: Option<BTreeMap<String, String>>,
    /// 判定区：替换（数组末尾 = 栈顶）。
    #[serde(default)]
    pub judge: Option<Vec<String>>,
    #[serde(default)]
    pub mark_counts: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    pub mark_cards: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub flags: Option<BTreeMap<String, bool>>,
    /// 额外技能（替换 `granted`）。
    #[serde(default)]
    pub extra_skills: Option<Vec<String>>,
}

fn slot_of(key: &str) -> Result<Slot, String> {
    match key {
        "weapon" => Ok(Slot::Weapon),
        "armor" => Ok(Slot::Armor),
        "horse_offense" => Ok(Slot::HorseOffense),
        "horse_defense" => Ok(Slot::HorseDefense),
        "treasure" => Ok(Slot::Treasure),
        _ => Err(format!("未知装备槽：{key}")),
    }
}

fn phase_of(s: &str) -> Result<Phase, String> {
    match s {
        "prepare" => Ok(Phase::Prepare),
        "judge" => Ok(Phase::Judge),
        "draw" => Ok(Phase::Draw),
        "play" => Ok(Phase::Play),
        "discard" => Ok(Phase::Discard),
        "end" => Ok(Phase::End),
        other => Err(format!("未知阶段：{other}")),
    }
}

/// 卡牌 token：牌实例 id（`c12`）或牌名（`杀`）；只能从摸牌堆/弃牌堆/移出区取。
fn take_card(g: &mut Game, token: &str, used: &mut BTreeSet<String>) -> Result<String, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("卡牌 token 不能为空".to_string());
    }
    if g.cards.contains_key(token) {
        if !used.insert(token.to_string()) {
            return Err(format!("卡牌 {token} 被重复使用"));
        }
        for zone in [Zone::Draw, Zone::Discard, Zone::Void] {
            if g.zone_of(&token.to_string()) == Some(&zone) {
                return Ok(token.to_string());
            }
        }
        return Err(format!("牌 {token} 不在可取材区域（摸牌堆/弃牌堆/移出区）"));
    }
    // 兼容 v1 定义码（`sha` / `shan` / `wuxie`…）：先按牌名找，再按 code 映射成牌名。
    let wanted = cards::name_of_code(token).unwrap_or(token);
    let candidate = g
        .ids_in(|z| matches!(z, Zone::Draw | Zone::Discard | Zone::Void))
        .into_iter()
        .rev()
        .find(|id| !used.contains(id) && g.name_of(id) == wanted)
        .ok_or_else(|| format!("找不到可用的牌：{token}（不能从他人手牌/装备区取）"))?;
    used.insert(candidate.clone());
    Ok(candidate)
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
        (None, None) => Err("座位注入必须给 index 或 player_id".to_string()),
    }
}

/// 场景注入（`EnginePlugin::apply_scene` 的实现）。
pub fn apply_scene(g: &mut Game, scene: &Value) -> Result<(), String> {
    let spec: SceneSpec =
        serde_json::from_value(scene.clone()).map_err(|e| format!("场景解析失败：{e}"))?;
    let mut used: BTreeSet<String> = BTreeSet::new();

    if let Some(seed) = spec.seed {
        g.rand_state = seed | 1;
    }
    if let Some(round) = spec.round {
        g.round = round;
    }
    if let Some(turn) = spec.turn {
        if turn >= g.seats.len() {
            return Err(format!("回合座位越界：{turn}"));
        }
        g.turn = turn;
    }
    if let Some(p) = spec.phase.as_deref() {
        match p {
            "turn" | "dying" | "lobby" | "setup" | "finished" => {}
            other => g.phase = phase_of(other)?,
        }
    }
    if let Some(p) = spec.turn_phase.as_deref() {
        g.phase = phase_of(p)?;
    }
    if let Some(top) = &spec.deck_top {
        // 数组末尾 = 最上面：按倒序逐张置于牌堆顶。
        let mut ids = Vec::new();
        for token in top {
            let id = take_card(g, token, &mut used)?;
            ids.push(id);
        }
        for id in ids.into_iter().rev() {
            g.put_on_top(&id).map_err(|e| e.to_string())?;
        }
    }
    if let Some(counters) = &spec.counters {
        for (k, v) in counters {
            g.counters.insert(format!("turn.{k}"), *v);
        }
    }

    for seat_scene in &spec.seats {
        let idx = seat_index(g, seat_scene)?;
        // 武将/身份先改（影响体力上限默认值）。
        if let Some(code) = seat_scene
            .general
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let def = generals::by_code(code).ok_or_else(|| format!("未知武将：{code}"))?;
            let skills: Vec<String> = def
                .skills
                .iter()
                .filter_map(|n| crate::skills::code_by_name(n).map(str::to_string))
                .collect();
            let seat = &mut g.seats[idx];
            seat.general = code.to_string();
            seat.kingdom = def.kingdom.to_string();
            seat.male = def.male;
            seat.skills = skills;
            crate::skills::rebuild_subs(g);
        }
        if let Some(id) = &seat_scene.identity {
            if !matches!(id.as_str(), "lord" | "loyalist" | "rebel" | "traitor") {
                return Err(format!("未知身份：{id}"));
            }
            g.seats[idx].identity = id.clone();
            g.seats[idx].flags.remove(identity::LORD);
            if id == identity::LORD {
                g.seats[idx].flags.insert(identity::LORD.to_string());
            }
        }
        if let Some(mh) = seat_scene.max_hp {
            g.seats[idx].max_hp = mh;
            g.seats[idx].hp = g.seats[idx].hp.min(mh);
        }
        if let Some(hp) = seat_scene.hp {
            g.seats[idx].hp = hp;
        }
        if let Some(alive) = seat_scene.alive {
            if !alive && g.seats[idx].hp > 0 {
                g.seats[idx].hp = 0;
            } else if alive && g.seats[idx].hp <= 0 {
                g.seats[idx].hp = 1;
            }
        }
        if let Some(bot) = seat_scene.is_bot {
            g.seats[idx].bot = bot;
        }
        if let Some(v) = seat_scene.chained {
            g.seats[idx].chained = v;
        }
        if let Some(v) = seat_scene.flipped {
            g.seats[idx].flipped = v;
        }
        if let Some(hand) = &seat_scene.hand {
            let old = g.hand(idx);
            for id in old {
                g.to_discard(&id, "scene").map_err(|e| e.to_string())?;
            }
            for token in hand {
                let id = take_card(g, token, &mut used)?;
                g.to_hand(&id, idx).map_err(|e| e.to_string())?;
            }
        }
        if let Some(append) = &seat_scene.hand_append {
            for token in append {
                let id = take_card(g, token, &mut used)?;
                g.to_hand(&id, idx).map_err(|e| e.to_string())?;
            }
        }
        if let Some(equips) = &seat_scene.equips {
            for (key, token) in equips {
                let slot = slot_of(key)?;
                if let Some(old) = g.equip_in(idx, slot) {
                    g.to_discard(&old, "scene").map_err(|e| e.to_string())?;
                }
                let id = take_card(g, token, &mut used)?;
                let def = g.def_of(&id);
                if def.slot != Some(slot) {
                    return Err(format!("牌【{}】不能装备到 {key}", g.name_of(&id)));
                }
                g.to_equip(&id, idx, slot).map_err(|e| e.to_string())?;
            }
        }
        if let Some(judge) = &seat_scene.judge {
            let old = g.judge_zone(idx);
            for id in old {
                g.to_discard(&id, "scene").map_err(|e| e.to_string())?;
            }
            for token in judge {
                let id = take_card(g, token, &mut used)?;
                g.to_judge(&id, idx).map_err(|e| e.to_string())?;
            }
        }
        if let Some(counts) = &seat_scene.mark_counts {
            for (k, v) in counts {
                g.seats[idx].counters.insert(k.clone(), *v);
            }
        }
        if let Some(cards) = &seat_scene.mark_cards {
            for (code, tokens) in cards {
                let old = g.mark_cards(idx, code);
                for id in old {
                    g.to_discard(&id, "scene").map_err(|e| e.to_string())?;
                }
                for token in tokens {
                    let id = take_card(g, token, &mut used)?;
                    g.to_mark(&id, idx, code).map_err(|e| e.to_string())?;
                }
            }
        }
        if let Some(flags) = &seat_scene.flags {
            for (k, v) in flags {
                if *v {
                    g.seats[idx].flags.insert(k.clone());
                } else {
                    g.seats[idx].flags.remove(k);
                }
            }
        }
        if let Some(extra) = &seat_scene.extra_skills {
            let mut skills = Vec::new();
            for code in extra {
                if crate::skills::def(code).is_none() {
                    return Err(format!("技能未注册：{code}"));
                }
                skills.push(code.clone());
            }
            g.seats[idx].granted = skills;
            crate::skills::rebuild_subs(g);
        }
    }

    validate_game(g)
}

/// 快照校验（沙盒/场景注入后）：守恒 + 无帧遗留 + 引用完整。
pub fn validate_game(g: &Game) -> Result<(), String> {
    if g.cards.len() != cards::DECK_SIZE {
        return Err(format!(
            "牌实例数 {} ≠ 牌堆上限 {}",
            g.cards.len(),
            cards::DECK_SIZE
        ));
    }
    g.check_conservation()?;
    g.check_no_orphan_frames()?;
    for (i, s) in g.seats.iter().enumerate() {
        if !s.general.is_empty() && generals::by_code(&s.general).is_none() {
            return Err(format!("座位 {i} 武将不存在：{}", s.general));
        }
        for code in s.skills.iter().chain(s.granted.iter()) {
            if crate::skills::def(code).is_none() {
                return Err(format!("座位 {i} 技能未注册：{code}"));
            }
        }
    }
    Ok(())
}

/// 便捷：把座位名写回 `Game`（沙盒用；v2 的座位名即 player_id）。
trait HostLike {
    fn host_like_players(&mut self, players: &[String]);
}

impl HostLike for Game {
    fn host_like_players(&mut self, players: &[String]) {
        for (i, pid) in players.iter().enumerate() {
            if let Some(seat) = self.seats.get_mut(i) {
                seat.name = pid.clone();
            }
        }
    }
}
