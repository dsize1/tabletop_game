//! 工作台「创建对局」沙盒开局（P5 基座，ADR-0034）。
//!
//! 目标：按用户指定的玩家数 / 座位武将 / 座位身份，直接生成一局**已开局**的
//! 三国杀状态（跳过 lobby 与选将），并复用生产路径的 `finish_setup`：
//! - 牌堆按 `Config.packs` 组堆并随机洗牌；每人初始手牌随机；
//! - 主公先手，`resolve::advance` 推进到出牌阶段 / 开局 pending；
//! - 开局技能（左慈化身、陆绩怀橘、刘虞宗祚、界袁绍血裔、界张郃巧变、卢植明任）正常执行。
//!
//! 该入口只被 server 的工作台预览调用；**不参与 Q 群开局流程**，也不写对局真相源。

use std::collections::{BTreeMap, BTreeSet};

use game_engine_core::{EngineError, EngineEvent, EnginePlugin, GameState, RandomSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::state::{Identity, SgsState, MAX_PLAYERS, MIN_PLAYERS};
use crate::{build_state, finish_setup, illegal, internal, SanguoshaEngine};

/// 单个座位的沙盒配置（全部可选；缺省随机）。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxSeat {
    /// 座位下标（0-based；缺省按数组顺序）。
    #[serde(default)]
    pub index: Option<usize>,
    /// 玩家 id（缺省 `bot:1..N`）；服务端会校验唯一。
    #[serde(default)]
    pub player_id: Option<String>,
    /// 武将 code（必须在默认 148 去重池内）。
    #[serde(default)]
    pub general: Option<String>,
    /// 身份（必须符合标准配比；缺省随机补足）。
    #[serde(default)]
    pub identity: Option<Identity>,
}

/// 沙盒开局入参。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxSpec {
    /// 玩家数 5–8。
    pub players: usize,
    #[serde(default)]
    pub seats: Vec<SandboxSeat>,
}

/// 生成一局已开局状态（纯内存；不落库）。
///
/// `rng` 只用于开局种子（`new_game` 内部取一次），之后的身份/武将随机与洗牌
/// 都走 `SgsState` 内建的 `rng_state`，保证同一 seed 可复现。
pub fn new_sandbox_game(
    room_id: &str,
    group_id: &str,
    spec: &SandboxSpec,
    rng: &mut dyn RandomSource,
) -> Result<(GameState, Vec<EngineEvent>), EngineError> {
    let n = spec.players;
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
        return Err(illegal(format!(
            "玩家人数需在 {MIN_PLAYERS}–{MAX_PLAYERS} 之间（当前 {n}）"
        )));
    }

    // 1) 座位下标归一化（允许乱序；重复/越界报错）。
    let mut resolved: Vec<Option<&SandboxSeat>> = vec![None; n];
    for (pos, seat) in spec.seats.iter().enumerate() {
        let idx = seat.index.unwrap_or(pos);
        if idx >= n {
            return Err(illegal(format!(
                "座位 {} 越界（共 {n} 人，座位 #1–#{n}）",
                idx + 1
            )));
        }
        if resolved[idx].is_some() {
            return Err(illegal(format!("座位 #{} 重复配置", idx + 1)));
        }
        resolved[idx] = Some(seat);
    }

    // 2) 玩家 id（默认全 bot）。
    let mut players = Vec::with_capacity(n);
    let mut seen_pids = BTreeSet::new();
    for (i, seat) in resolved.iter().enumerate() {
        let pid = seat
            .and_then(|s| s.player_id.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("bot:{}", i + 1));
        if !seen_pids.insert(pid.clone()) {
            return Err(illegal(format!("玩家 id 重复：{pid}")));
        }
        players.push(pid);
    }

    // 3) 先借 `new_game` 建一个 lobby 骨架（复用全部 state 字段/默认 config/seed）。
    let engine = SanguoshaEngine;
    let (base_state, _) = engine.new_game(room_id, group_id, players.clone(), rng);
    let mut st = SgsState::parse(&base_state.data).map_err(internal)?;

    // 4) 武将：默认 148 去重池（与正常开局一致）；指定必须命中池子且不重复。
    let pool = crate::available_codes_for_packs(&st.config.general_packs);
    let mut requested_generals: Vec<Option<&'static str>> = vec![None; n];
    let mut used_generals = BTreeSet::new();
    for (i, seat) in resolved.iter().enumerate() {
        let Some(code) = seat
            .and_then(|s| s.general.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(found) = pool.iter().copied().find(|c| *c == code) else {
            return Err(illegal(format!(
                "武将不可用：{code}（须在默认 {pool_count} 位去重池内）",
                pool_count = pool.len()
            )));
        };
        if !used_generals.insert(found.to_string()) {
            return Err(illegal(format!("武将重复：{code}")));
        }
        requested_generals[i] = Some(found);
    }

    // 5) 身份：标准配比；指定身份从池中扣除，其余随机补足。
    let mut identity_pool = crate::identity_list(n, st.config.double_spy)?;
    let mut requested_identities: Vec<Option<Identity>> = vec![None; n];
    for (i, seat) in resolved.iter().enumerate() {
        let Some(identity) = seat.and_then(|s| s.identity) else {
            continue;
        };
        let Some(pos) = identity_pool.iter().position(|x| *x == identity) else {
            return Err(illegal(format!(
                "身份超出标准配比：{}",
                identity.code()
            )));
        };
        identity_pool.remove(pos);
        requested_identities[i] = Some(identity);
    }
    st.shuffle(&mut identity_pool);
    for i in 0..n {
        st.seats[i].player_id = players[i].clone();
        st.seats[i].is_bot = true;
        st.seats[i].identity = match requested_identities[i] {
            Some(identity) => identity,
            None => identity_pool
                .pop()
                .ok_or_else(|| internal("身份池不足（配比异常）"))?,
        };
    }

    // 6) 剩余武将随机（不重复）。
    let mut remaining: Vec<&'static str> = pool
        .into_iter()
        .filter(|code| !used_generals.contains(*code))
        .collect();
    st.shuffle(&mut remaining);
    let mut picks: BTreeMap<String, String> = BTreeMap::new();
    for i in 0..n {
        let code = requested_generals[i]
            .or_else(|| remaining.pop())
            .ok_or_else(|| internal("武将池不足（无法为全部座位分配武将）"))?;
        picks.insert(players[i].clone(), code.to_string());
    }

    let lord_idx = st
        .lord_index()
        .ok_or_else(|| internal("未分配主公（身份配比异常）"))?;
    st.host = players[0].clone();
    st.setup.lord = players[lord_idx].clone();
    st.setup.lord_picked = true;
    st.setup.picks = picks;

    // 7) 复用生产开局后半段：定体力、建牌堆、发手牌、跑开局技能、主公先手。
    let mut events = vec![EngineEvent::new(
        "sandbox_created",
        json!({
            "players": n,
            "seed": st.deck.seed,
            "all_bots": true,
        }),
    )];
    events.extend(finish_setup(&mut st)?);

    let state = build_state(&base_state, &st);
    crate::scene::validate_state(&state).map_err(internal)?;
    Ok((state, events))
}

/// 沙盒表单元数据（武将按默认 148 去重池；身份按标准配比）。
pub fn sandbox_meta() -> Value {
    let generals: Vec<Value> = crate::catalog::default_general_pool()
        .into_iter()
        .map(|g| {
            json!({
                "code": g.code,
                "name": g.name,
                "kingdom": g.kingdom,
                "pack": g.pack,
                "max_hp": g.max_hp,
                "is_lord": g.is_lord,
                "skills": g.skills,
            })
        })
        .collect();
    let mut identities = serde_json::Map::new();
    for n in MIN_PLAYERS..=MAX_PLAYERS {
        if let Ok(list) = crate::identity_list(n, false) {
            let mut counts = BTreeMap::new();
            for id in list {
                *counts.entry(id.code()).or_insert(0u32) += 1;
            }
            identities.insert(n.to_string(), json!(counts));
        }
    }
    json!({
        "players": [MIN_PLAYERS, 6, 7, MAX_PLAYERS],
        "generals": generals,
        "identities": identities,
        "packs": {
            "cards": ["standard", "junzheng"],
            "general_packs": crate::state::default_general_packs(),
        },
    })
}
