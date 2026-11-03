//! 工作台场景注入（P5 基座，ADR-0033）。
//!
//! 输入是**结构化 `SceneSpec`**（不是直接改 `GameState.data`），由引擎负责：
//! - 白名单字段 + 类型校验（`deny_unknown_fields`）；
//! - 资源只做**移动**（手牌/装备/判定/牌堆顶都从既有牌实例里取），不凭空造牌；
//! - 注入后重新做牌数守恒 + 引用完整性校验。
//!
//! 用途：工作台/批量武将验证脚本生成「最小局面」（指定武将、手牌、装备、回合阶段等）。
//! 预览宿主不写 `game_state`/`game_events`，见 `crates/server/src/preview.rs`。

use std::collections::{BTreeMap, BTreeSet};

use game_engine_core::{GameState, GameStatus};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::data;
use crate::skills;
use crate::state::{
    CardInst, EquipSlot, Identity, Phase, SgsState, TurnPhase, MAX_PLAYERS, MIN_PLAYERS,
    SCHEMA_VERSION,
};

/// 场景注入入参（全部字段可选；`seats` 按座位 index/player_id 定位）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    /// 轮次（主公回合 = 新一轮）。
    #[serde(default)]
    pub round: Option<u32>,
    /// 当前回合座位下标。
    #[serde(default)]
    pub turn: Option<usize>,
    /// 回合内阶段。
    #[serde(default)]
    pub turn_phase: Option<TurnPhase>,
    /// 全局阶段。
    #[serde(default)]
    pub phase: Option<Phase>,
    /// 随机种子（写入 `deck.seed` 与 `rng_state`；影响之后的摸牌/判定）。
    #[serde(default)]
    pub seed: Option<u64>,
    /// 牌堆顶（数组末尾 = 最上面；卡牌 token 规则见 `take_card`）。
    #[serde(default)]
    pub deck_top: Option<Vec<String>>,
    /// 本回合计数器（合并到 `turn_counters`）。
    #[serde(default)]
    pub counters: Option<BTreeMap<String, u32>>,
    /// 座位注入。
    #[serde(default)]
    pub seats: Vec<SeatScene>,
}

/// 单个座位的场景注入。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatScene {
    /// 座位下标（与 `player_id` 二选一；都给时必须一致）。
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub player_id: Option<String>,
    /// 武将 code（如 `sgs_hero_1`）。
    #[serde(default)]
    pub general: Option<String>,
    /// 身份（主公/忠臣/反贼/内奸）。
    #[serde(default)]
    pub identity: Option<Identity>,
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
    /// 手牌：**替换**（原手牌进弃牌堆，再从牌堆/弃牌堆取 token 指定的牌）。
    #[serde(default)]
    pub hand: Option<Vec<String>>,
    /// 手牌：追加。
    #[serde(default)]
    pub hand_append: Option<Vec<String>>,
    /// 装备：按槽位替换（原装备进弃牌堆）。
    #[serde(default)]
    pub equips: Option<BTreeMap<EquipSlot, String>>,
    /// 判定区：替换（原判定牌进弃牌堆；数组末尾 = 栈顶）。
    #[serde(default)]
    pub judge: Option<Vec<String>>,
    /// 计数型标记（如 `ju:3`；替换指定 key，未列出的保留）。
    #[serde(default)]
    pub mark_counts: Option<BTreeMap<String, u32>>,
    /// 武将牌上的标记（牌实例，按 token 取牌；替换指定 key）。
    #[serde(default)]
    pub mark_cards: Option<BTreeMap<String, Vec<String>>>,
    /// 技能私有 flags（合并/覆盖指定 key）。
    #[serde(default)]
    pub flags: Option<BTreeMap<String, bool>>,
    /// 额外技能（替换）。
    #[serde(default)]
    pub extra_skills: Option<Vec<String>>,
}

/// 卡牌 token 解析优先级：
/// 1. 牌实例 id（如 `c12`）——必须存在；
/// 2. 定义码（如 `sha`）——从摸牌堆顶向下、弃牌堆、移出区依次找第一张未使用的。
///
/// 不接受从其他角色手牌/装备/判定区取牌（避免场景脚本「偷牌」造成难排查的副作用）。
fn take_card(
    st: &mut SgsState,
    token: &str,
    used: &mut BTreeSet<String>,
) -> Result<String, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("卡牌 token 不能为空".to_string());
    }
    if st.cards.contains_key(token) {
        if !used.insert(token.to_string()) {
            return Err(format!("卡牌 {token} 被重复使用"));
        }
        // 显式牌 id：必须位于可取材区域（摸牌堆/弃牌堆/移出区），并从原区域移除。
        for zone in [
            &mut st.deck.draw,
            &mut st.deck.discard,
            &mut st.deck.void,
        ] {
            if let Some(pos) = zone.iter().position(|x| x == token) {
                zone.remove(pos);
                return Ok(token.to_string());
            }
        }
        return Err(format!(
            "牌 {token} 不在可取材区域（只能从摸牌堆/弃牌堆/移出区取）"
        ));
    }
    let find_in = |ids: &[String], cards: &BTreeMap<String, CardInst>| -> Option<String> {
        ids.iter()
            .rev()
            .find(|id| {
                !used.contains(*id)
                    && cards.get(*id).is_some_and(|c| c.def == token)
            })
            .cloned()
    };
    let found = find_in(&st.deck.draw, &st.cards)
        .or_else(|| find_in(&st.deck.discard, &st.cards))
        .or_else(|| find_in(&st.deck.void, &st.cards));
    let Some(id) = found else {
        return Err(format!(
            "找不到可用的牌：{token}（只能从摸牌堆/弃牌堆/移出区取，且不能重复）"
        ));
    };
    // 从原区域移除。
    for zone in [
        &mut st.deck.draw,
        &mut st.deck.discard,
        &mut st.deck.void,
    ] {
        if let Some(pos) = zone.iter().position(|x| *x == id) {
            zone.remove(pos);
            break;
        }
    }
    used.insert(id.clone());
    Ok(id)
}

fn to_discard(st: &mut SgsState, ids: impl IntoIterator<Item = String>) {
    st.deck.discard.extend(ids);
}

fn seat_index(st: &SgsState, scene: &SeatScene) -> Result<usize, String> {
    match (scene.index, scene.player_id.as_deref()) {
        (Some(i), Some(pid)) => {
            let seat = st
                .seats
                .get(i)
                .ok_or_else(|| format!("座位下标越界：{i}"))?;
            if seat.player_id != pid {
                return Err(format!("座位 {i} 的玩家是 {}，不是 {pid}", seat.player_id));
            }
            Ok(i)
        }
        (Some(i), None) => {
            if i >= st.seats.len() {
                return Err(format!("座位下标越界：{i}（共 {} 人）", st.seats.len()));
            }
            Ok(i)
        }
        (None, Some(pid)) => st
            .seat_index(pid)
            .ok_or_else(|| format!("找不到玩家 {pid}")),
        (None, None) => Err("seats[] 需要 index 或 player_id".to_string()),
    }
}

/// 解析并应用场景，返回新 `GameState`（不落库）。
pub fn apply_scene(state: &GameState, scene: &Value) -> Result<GameState, String> {
    if !scene.is_null() && !scene.is_object() {
        return Err("scene 必须是 JSON 对象".to_string());
    }
    let spec: SceneSpec = if scene.is_null() {
        SceneSpec::default()
    } else {
        serde_json::from_value(scene.clone()).map_err(|e| format!("场景格式错误：{e}"))?
    };
    let mut st = SgsState::parse(&state.data)?;
    let mut used: BTreeSet<String> = BTreeSet::new();

    if let Some(phase) = spec.phase {
        st.phase = phase;
    }
    if let Some(round) = spec.round {
        st.round = round;
    }
    if let Some(turn) = spec.turn {
        if turn >= st.seats.len() {
            return Err(format!("turn 越界：{turn}（共 {} 人）", st.seats.len()));
        }
        st.turn = turn;
    }
    if let Some(tp) = spec.turn_phase {
        st.turn_phase = tp;
    }
    if let Some(seed) = spec.seed {
        st.deck.seed = seed;
        // 与 new_game 一致：避免全 0 状态（xorshift 会卡死）。
        st.rng_state = seed | 1;
    }

    // 先处理座位（武将/手牌/装备），再处理牌堆顶（避免刚放到牌堆顶的牌被手牌取走时顺序混乱）。
    for seat_scene in &spec.seats {
        let idx = seat_index(&st, seat_scene)?;
        if let Some(code) = &seat_scene.general {
            let def = data::general(code).ok_or_else(|| format!("未知武将：{code}"))?;
            let seat = &mut st.seats[idx];
            seat.general = Some(code.clone());
            seat.general_name = def.name.to_string();
            let bonus = if seat.identity.is_lord() { 1 } else { 0 };
            seat.max_hp = (def.max_hp + bonus).min(crate::state::MAX_HP_LIMIT);
            seat.hp = seat.max_hp;
        }
        if let Some(identity) = seat_scene.identity {
            // 主公体力加成：变成主公 +1；不再是主公 -1（以武将基础值为准）。
            let was_lord = st.seats[idx].identity.is_lord();
            st.seats[idx].identity = identity;
            if was_lord != identity.is_lord() {
                if let Some(code) = st.seats[idx].general.clone() {
                    if let Some(def) = data::general(&code) {
                        let bonus = if identity.is_lord() { 1 } else { 0 };
                        st.seats[idx].max_hp =
                            (def.max_hp + bonus).min(crate::state::MAX_HP_LIMIT);
                    }
                }
            }
        }
        if let Some(max_hp) = seat_scene.max_hp {
            if !(1..=crate::state::MAX_HP_LIMIT).contains(&max_hp) {
                return Err(format!("max_hp 越界：{max_hp}"));
            }
            st.seats[idx].max_hp = max_hp;
        }
        if let Some(hp) = seat_scene.hp {
            if hp > st.seats[idx].max_hp {
                return Err(format!(
                    "hp={hp} 超过 max_hp={}（座位 {idx}）",
                    st.seats[idx].max_hp
                ));
            }
            st.seats[idx].hp = hp;
        }
        if let Some(v) = seat_scene.alive {
            st.seats[idx].alive = v;
        }
        if let Some(v) = seat_scene.is_bot {
            st.seats[idx].is_bot = v;
        }
        if let Some(v) = seat_scene.chained {
            st.seats[idx].chained = v;
        }
        if let Some(v) = seat_scene.flipped {
            st.seats[idx].flipped = v;
        }

        if let Some(hand) = &seat_scene.hand {
            let old = std::mem::take(&mut st.seats[idx].hand);
            to_discard(&mut st, old);
            for token in hand {
                let id = take_card(&mut st, token, &mut used)?;
                st.seats[idx].hand.push(id);
            }
        }
        if let Some(hand) = &seat_scene.hand_append {
            for token in hand {
                let id = take_card(&mut st, token, &mut used)?;
                st.seats[idx].hand.push(id);
            }
        }
        if let Some(equips) = &seat_scene.equips {
            for (slot, token) in equips {
                if st.seats[idx].abolished_slots.contains(slot) {
                    return Err(format!("座位 {idx} 的装备栏已废除：{slot:?}"));
                }
                let old = st.seats[idx].equips.get(slot).cloned().flatten();
                if let Some(old) = old {
                    to_discard(&mut st, std::iter::once(old));
                }
                let id = take_card(&mut st, token, &mut used)?;
                st.seats[idx].equips.insert(*slot, Some(id));
            }
        }
        if let Some(judge) = &seat_scene.judge {
            if st.seats[idx].judge_abolished {
                return Err(format!("座位 {idx} 的判定区已废除"));
            }
            let old = std::mem::take(&mut st.seats[idx].judge_zone);
            to_discard(&mut st, old);
            for token in judge {
                let id = take_card(&mut st, token, &mut used)?;
                st.seats[idx].judge_zone.push(id);
            }
        }
        if let Some(marks) = &seat_scene.mark_counts {
            for (k, v) in marks {
                if *v > 99 {
                    return Err(format!("标记 {k} 数量过大：{v}（≤99）"));
                }
                st.seats[idx].mark_counts.insert(k.clone(), *v);
            }
        }
        if let Some(marks) = &seat_scene.mark_cards {
            for (k, tokens) in marks {
                let old = st.seats[idx].marks.remove(k).unwrap_or_default();
                to_discard(&mut st, old);
                let mut ids = Vec::new();
                for token in tokens {
                    ids.push(take_card(&mut st, token, &mut used)?);
                }
                st.seats[idx].marks.insert(k.clone(), ids);
            }
        }
        if let Some(flags) = &seat_scene.flags {
            for (k, v) in flags {
                st.seats[idx].flags.insert(k.clone(), *v);
            }
        }
        if let Some(extra) = &seat_scene.extra_skills {
            for code in extra {
                if skills::def_of(code).is_none() {
                    return Err(format!("未知技能：{code}"));
                }
            }
            st.seats[idx].extra_skills = extra.clone();
        }
    }

    if let Some(top) = &spec.deck_top {
        let mut ids = Vec::new();
        for token in top {
            ids.push(take_card(&mut st, token, &mut used)?);
        }
        // 数组末尾 = 牌堆顶（draw 用 pop）。
        st.deck.draw.extend(ids);
    }

    if let Some(counters) = &spec.counters {
        for (k, v) in counters {
            st.turn_counters.insert(k.clone(), *v);
        }
    }

    let mut out = state.clone();
    out.players = st.seats.iter().map(|s| s.player_id.clone()).collect();
    out.status = if st.phase.is_over() {
        GameStatus::Finished
    } else {
        GameStatus::Active
    };
    out.data = serde_json::to_value(&st).map_err(|e| format!("序列化状态失败：{e}"))?;
    validate_state(&out)?;
    Ok(out)
}

/// 校验一份三国杀完整状态快照。
pub fn validate_state(state: &GameState) -> Result<(), String> {
    if state.game_id != "sanguosha" {
        return Err(format!("不是三国杀对局：{}", state.game_id));
    }
    if state.room_id.trim().is_empty() {
        return Err("room_id 不能为空".to_string());
    }
    let st = SgsState::parse(&state.data)?;
    if st.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "schema_version={} 与引擎当前版本 {SCHEMA_VERSION} 不一致",
            st.schema_version
        ));
    }
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&st.seats.len()) {
        return Err(format!(
            "座位数 {} 不在 {MIN_PLAYERS}–{MAX_PLAYERS} 范围",
            st.seats.len()
        ));
    }
    if state.players.len() != st.seats.len() {
        return Err(format!(
            "players({}) 与 seats({}) 数量不一致",
            state.players.len(),
            st.seats.len()
        ));
    }
    let mut player_ids = BTreeSet::new();
    for (i, seat) in st.seats.iter().enumerate() {
        if seat.player_id.trim().is_empty() {
            return Err(format!("座位 {i} 的 player_id 为空"));
        }
        if !player_ids.insert(seat.player_id.clone()) {
            return Err(format!("玩家重复：{}", seat.player_id));
        }
        if state.players[i] != seat.player_id {
            return Err(format!(
                "players[{i}]={} 与 seats[{i}].player_id={} 不一致",
                state.players[i], seat.player_id
            ));
        }
        if seat.max_hp < 0 || seat.max_hp > crate::state::MAX_HP_LIMIT {
            return Err(format!("座位 {i} max_hp 越界：{}", seat.max_hp));
        }
        if seat.hp > seat.max_hp + 5 {
            return Err(format!("座位 {i} hp={} 明显超过 max_hp={}", seat.hp, seat.max_hp));
        }
        if seat.hand.len() as u32 > crate::state::MAX_HAND_CARDS_LIMIT {
            return Err(format!("座位 {i} 手牌过多：{}", seat.hand.len()));
        }
        if let Some(code) = seat.general.as_deref() {
            let def = data::general(code).ok_or_else(|| format!("座位 {i} 未知武将：{code}"))?;
            if seat.general_name != def.name {
                return Err(format!(
                    "座位 {i} general_name={} 与武将 {} 不一致",
                    seat.general_name, def.name
                ));
            }
        } else if matches!(st.phase, Phase::Turn | Phase::Dying) {
            return Err(format!("座位 {i} 在回合阶段缺少武将"));
        }
        for skill in &seat.extra_skills {
            if skills::def_of(skill).is_none() {
                return Err(format!("座位 {i} 未知技能：{skill}"));
            }
        }
        for code in &seat.huashen {
            if data::general(code).is_none() {
                return Err(format!("座位 {i} 化身未知武将：{code}"));
            }
        }
    }
    if matches!(st.phase, Phase::Turn | Phase::Dying | Phase::Finished)
        && st.lord_index().is_none()
    {
        return Err("回合阶段必须恰有一位主公".to_string());
    }
    if st.turn >= st.seats.len() {
        return Err(format!("turn 越界：{}（共 {} 人）", st.turn, st.seats.len()));
    }

    // 引用完整性：所有区域里的牌实例必须存在于 `cards`，且不能重复占位。
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut check_zone = |name: &str, ids: &[String]| -> Result<(), String> {
        for id in ids {
            if !st.cards.contains_key(id) {
                return Err(format!("{name} 引用了不存在的牌实例：{id}"));
            }
            if !seen.insert(id.clone()) {
                return Err(format!("牌实例 {id} 同时出现在多个区域"));
            }
        }
        Ok(())
    };
    check_zone("摸牌堆", &st.deck.draw)?;
    check_zone("弃牌堆", &st.deck.discard)?;
    check_zone("处理区", &st.deck.processing)?;
    check_zone("移出区", &st.deck.void)?;
    for (i, seat) in st.seats.iter().enumerate() {
        check_zone(&format!("座位 {i} 手牌"), &seat.hand)?;
        let equips: Vec<String> = seat.equips.values().flatten().cloned().collect();
        check_zone(&format!("座位 {i} 装备"), &equips)?;
        check_zone(&format!("座位 {i} 判定区"), &seat.judge_zone)?;
        for (k, ids) in &seat.marks {
            check_zone(&format!("座位 {i} 标记 {k}"), ids)?;
        }
    }
    for id in st.cards.keys() {
        if !seen.contains(id) {
            return Err(format!("牌实例 {id} 未出现在任何区域（牌数不守恒）"));
        }
    }
    st.check_conservation()?;
    Ok(())
}

/// 场景说明（工作台 UI 展示用；与 `SceneSpec` 字段保持一致）。
pub fn scene_schema() -> Value {
    json!({
        "round": "u32? 轮次",
        "turn": "usize? 当前回合座位下标",
        "turn_phase": "prepare|judge|draw|play|discard|end?",
        "phase": "lobby|setup|turn|dying|finished?",
        "seed": "u64? 随机种子",
        "deck_top": ["sha", "c12"],
        "counters": { "sha_used": 1 },
        "seats": [{
            "index": 0,
            "player_id": "openid?（与 index 二选一）",
            "general": "sgs_hero_1",
            "identity": "lord|loyalist|rebel|spy?",
            "hp": 4,
            "max_hp": 4,
            "alive": true,
            "is_bot": false,
            "chained": false,
            "flipped": false,
            "hand": ["sha", "shan"],
            "hand_append": ["wuzhong_shengyou"],
            "equips": { "weapon": "qinggang_jian" },
            "judge": ["lebu_sishu"],
            "mark_counts": { "ju": 3 },
            "mark_cards": { "chuang": ["sha"] },
            "flags": { "awakened": true },
            "extra_skills": ["wusheng"]
        }],
        "note": "卡牌 token = 牌实例 id（c12）或定义码（sha）；定义码从摸牌堆/弃牌堆/移出区取第一张，不能重复使用"
    })
}
