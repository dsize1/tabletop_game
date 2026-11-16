//! 引擎层大厅 + 选将（V3）：动作 id 与 v1/v2 **完全一致**（网关/Web 无需改协议）。
//!
//! ```text
//! new_game → Lobby（lobby:join / lobby:add_bot / lobby:set_*）
//!          → lobby:start（发身份 + 主公候选）→ Setup（setup:choose_general:<code>）
//!          → 全部选完 → finish_setup（定体力/建牌堆/发 4 张/进入主公回合）
//! ```
//!
//! 与 v2 的差异（本阶段）：
//! - **不挂技能**：V3 的技能按 B0–B20 批次接入，`Seat.skills` 暂留空（「不依赖武将技能也能完整对局」）；
//!   武将包过滤只影响**候选池展示**，不影响结算；
//! - 随机性全部走注入的 `RandomSource`（洗牌/发身份/发候选），状态里存结果不存种子。

use std::collections::BTreeMap;

use game_engine_core::{pick_index, shuffle, RandomSource};
use serde::{Deserialize, Serialize};

use crate::kernel::generals;
use crate::kernel::{flow, turn, Card, Game, Identity, Suit, Zone};

pub const MIN_PLAYERS: usize = 5;
pub const MAX_PLAYERS: usize = 8;
pub const MAX_CANDIDATE_COUNT: u32 = 7;
pub const MAX_HAND_CARDS_LIMIT: u32 = 30;
pub const INITIAL_HAND: usize = 4;

/// 房间配置（大厅可改；开局后固定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyConfig {
    /// 手牌持有上限（房规；0 = 用体力值，v3 默认 0）。
    pub max_hand_cards: u32,
    pub lord_candidate_count: u32,
    pub other_candidate_count: u32,
    pub double_spy: bool,
    /// 牌堆包组合（`standard` / `standard+junzheng`）。
    pub packs: String,
    /// 武将包组合（`standard` 恒为基数；`+` 连接）。
    pub general_packs: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            max_hand_cards: 0,
            lord_candidate_count: 5,
            other_candidate_count: 5,
            double_spy: false,
            packs: "standard+junzheng".to_string(),
            general_packs: "standard+wind_fire_forest_mountain+yijiang+jiexian".to_string(),
        }
    }
}

impl LobbyConfig {
    pub fn pack_keys(&self) -> Vec<&str> {
        self.packs.split('+').filter(|s| !s.is_empty()).collect()
    }
    pub fn general_pack_keys(&self) -> Vec<&str> {
        self.general_packs
            .split('+')
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lobby {
    pub host: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Setup {
    pub lord: usize,
    pub offers: BTreeMap<usize, Vec<String>>,
    pub picks: BTreeMap<usize, String>,
    pub lord_picked: bool,
}

// ---------------------------------------------------------------------------
// 武将池
// ---------------------------------------------------------------------------

/// 已登记的武将包（生成物 `GeneralDef.pack`）。
pub const GENERAL_PACKS: [&str; 4] = [
    "standard",
    "wind_fire_forest_mountain",
    "yijiang",
    "jiexian",
];

/// 可选武将 code（**Live 口径，与 v2 / 全武将验证批次 / 中台同源**）。
///
/// 232 = 4 个包登记的全部名录（含 85 个被「界/将」同名去重排除的**原版**）；
/// 选将池只取 `Status::Live`（147），避免「张辽 / 界张辽」同局这类重复。
/// V3 阶段**不按技能实现度过滤**——技能按 B0–B20 批次接入，未接入不影响开局。
pub fn available_codes(cfg: &LobbyConfig) -> Vec<&'static str> {
    let packs = cfg.general_pack_keys();
    generals::GENERALS
        .iter()
        .filter(|g| g.status == generals::Status::Live)
        .filter(|g| packs.is_empty() || packs.contains(&g.pack))
        .map(|g| g.code)
        .collect()
}

/// 武将包组合预设（`standard` 基数 + 其余包的全部子集；按包数升序）。
pub fn general_pack_presets() -> Vec<String> {
    let others: Vec<&str> = GENERAL_PACKS
        .iter()
        .copied()
        .filter(|p| *p != "standard")
        .collect();
    let mut out: Vec<String> = Vec::new();
    for mask in 0usize..(1usize << others.len()) {
        let mut keys = vec!["standard"];
        for (i, p) in others.iter().enumerate() {
            if mask & (1 << i) != 0 {
                keys.push(*p);
            }
        }
        out.push(keys.join("+"));
    }
    out.sort_by_key(|mode| (mode.matches('+').count(), mode.clone()));
    out
}

pub fn general_pack_label(mode: &str) -> (String, usize) {
    let label = mode
        .split('+')
        .map(|p| match p {
            "standard" => "标准",
            "wind_fire_forest_mountain" => "风林火山",
            "yijiang" => "一将成名",
            "jiexian" => "界限突破",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let count = generals::GENERALS
        .iter()
        .filter(|g| g.status == generals::Status::Live)
        .filter(|g| mode.split('+').any(|p| p == g.pack))
        .count();
    (label, count)
}

// ---------------------------------------------------------------------------
// 合法动作（与 `apply` 同源：legal ≡ execute）
// ---------------------------------------------------------------------------

pub fn legal_actions(g: &Game, lobby: Option<&Lobby>, setup: Option<&Setup>, player_id: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let seated = g.seat_of(player_id);
    if let Some(_lobby) = lobby {
        let n = g.seats.len();
        if seated.is_none() {
            if n < MAX_PLAYERS {
                out.push(("lobby:join".into(), "加入游戏".into()));
            }
            return out;
        }
        out.push(("lobby:leave".into(), "退出房间".into()));
        if g.host.as_deref() != Some(player_id) {
            return out;
        }
        if (MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
            out.push(("lobby:start".into(), "开始游戏（发身份 → 选将）".into()));
        }
        if n < MAX_PLAYERS {
            out.push(("lobby:add_bot".into(), "加入机器人".into()));
        }
        out.push(("lobby:abort".into(), "中断本局（不结算）".into()));
        for s in &g.seats {
            if s.name != player_id && !s.bot {
                out.push((format!("lobby:kick:{}", s.name), format!("踢出 {}", s.name)));
            }
        }
        for mode in ["standard", "standard+junzheng"] {
            out.push((format!("lobby:set_packs:{mode}"), format!("牌堆：{mode}")));
        }
        for mode in general_pack_presets() {
            let (label, count) = general_pack_label(&mode);
            out.push((
                format!("lobby:set_general_packs:{mode}"),
                format!("武将包：{label}（可用 {count} 位）"),
            ));
        }
        return out;
    }
    if let Some(setup) = setup {
        if let Some(idx) = seated {
            if !setup.picks.contains_key(&idx) {
                for code in setup.offers.get(&idx).cloned().unwrap_or_default() {
                    let name = generals::def(&code).map(|d| d.name).unwrap_or("");
                    out.push((
                        format!("setup:choose_general:{code}"),
                        format!("选择武将：{name}"),
                    ));
                }
            }
        }
    }
    out
}

/// 大厅配置类动作（需要 `EngineState.cfg`，与 `legal_actions` 同步生成）。
pub fn config_actions(g: &Game, cfg: &LobbyConfig) -> Vec<(String, String)> {
    let mut out = vec![];
    for v in [3u32, 5, 7] {
        if v != cfg.lord_candidate_count {
            out.push((format!("lobby:set_lord_candidate_count:{v}"), format!("主公候选非主公将 {v} 张")));
        }
    }
    for v in [3u32, 5, 7] {
        if v != cfg.other_candidate_count {
            out.push((format!("lobby:set_other_candidate_count:{v}"), format!("其他玩家候选 {v} 张")));
        }
    }
    for v in [10u32, 20, 30] {
        if v != cfg.max_hand_cards {
            out.push((format!("lobby:set_max_hand_cards:{v}"), format!("手牌持有上限 {v}")));
        }
    }
    if g.seats.len() == 8 {
        out.push((
            format!("lobby:set_double_spy:{}", u8::from(!cfg.double_spy)),
            if cfg.double_spy { "关闭双内局".into() } else { "开启双内局（1 主 1 忠 4 反 2 内）".into() },
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// 执行
// ---------------------------------------------------------------------------

pub fn apply(
    g: &mut Game,
    cfg: &mut LobbyConfig,
    lobby: &mut Option<Lobby>,
    setup: &mut Option<Setup>,
    player_id: &str,
    action: &str,
    rng: &mut dyn RandomSource,
) -> Result<String, String> {
    if let Some(rest) = action.strip_prefix("lobby:") {
        return apply_lobby(g, cfg, lobby, setup, player_id, rest, rng);
    }
    if let Some(code) = action.strip_prefix("setup:choose_general:") {
        return pick(g, cfg, lobby, setup, player_id, code, rng);
    }
    Err(format!("未知房间动作：{action}"))
}

fn apply_lobby(
    g: &mut Game,
    cfg: &mut LobbyConfig,
    lobby: &mut Option<Lobby>,
    setup: &mut Option<Setup>,
    player_id: &str,
    rest: &str,
    rng: &mut dyn RandomSource,
) -> Result<String, String> {
    if lobby.is_none() && rest != "abort" {
        return Err("已开局，不能修改房间".into());
    }
    let seated = g.seat_of(player_id);
    match rest {
        "join" => {
            if seated.is_some() {
                return Err("已在房间中".into());
            }
            if g.seats.len() >= MAX_PLAYERS {
                return Err("房间已满".into());
            }
            let mut seat = crate::kernel::Seat {
                id: g.seats.len(),
                name: player_id.to_string(),
                alive: true,
                ..Default::default()
            };
            seat.hp = 0;
            seat.max_hp = 0;
            g.seats.push(seat);
            Ok(format!("已加入（{}/{}）", g.seats.len(), MAX_PLAYERS))
        }
        "leave" => {
            let idx = seated.ok_or_else(|| "不在房间中".to_string())?;
            if g.host.as_deref() == Some(player_id) {
                abort(g, lobby, setup, "房主离开，对局废弃");
                Ok("房主已离开，对局废弃".to_string())
            } else {
                g.seats.remove(idx);
                for (i, s) in g.seats.iter_mut().enumerate() {
                    s.id = i;
                }
                Ok(format!("已退出（{}/{}）", g.seats.len(), MAX_PLAYERS))
            }
        }
        "start" => {
            require_host(g, player_id)?;
            let n = g.seats.len();
            if n < MIN_PLAYERS {
                return Err(format!("至少 {MIN_PLAYERS} 人才能开始（当前 {n}）"));
            }
            let lord = start(g, cfg, lobby, setup, rng)?;
            Ok(format!("已开局：{n} 人，主公先选将（座位 {}）", lord + 1))
        }
        "abort" => {
            require_host(g, player_id)?;
            abort(g, lobby, setup, "房主中断");
            Ok("已中断本局（不结算）".to_string())
        }
        "add_bot" => {
            require_host(g, player_id)?;
            if g.seats.len() >= MAX_PLAYERS {
                return Err("房间已满".into());
            }
            let bot = new_bot_id(g);
            let mut seat = crate::kernel::Seat {
                id: g.seats.len(),
                name: bot.clone(),
                bot: true,
                alive: true,
                ..Default::default()
            };
            seat.hp = 0;
            seat.max_hp = 0;
            g.seats.push(seat);
            Ok(format!("已加入机器人 {bot}（{}/{}）", g.seats.len(), MAX_PLAYERS))
        }
        _ if rest.starts_with("kick:") => {
            require_host(g, player_id)?;
            let target = &rest["kick:".len()..];
            if g.host.as_deref() == Some(target) {
                return Err("不能踢出房主".into());
            }
            let idx = g.seat_of(target).ok_or_else(|| "目标不在房间中".to_string())?;
            g.seats.remove(idx);
            for (i, s) in g.seats.iter_mut().enumerate() {
                s.id = i;
            }
            Ok(format!("已踢出 {target}"))
        }
        _ if rest.starts_with("set_") => {
            require_host(g, player_id)?;
            let mut it = rest.splitn(2, ':');
            let key = it.next().unwrap_or_default();
            let value = it.next().ok_or_else(|| "缺少配置值".to_string())?;
            let key = key.trim_start_matches("set_");
            apply_config(cfg, key, value)?;
            Ok(format!("已设置 {key} = {value}"))
        }
        _ => Err(format!("未知房间动作：{rest}")),
    }
}

fn require_host(g: &Game, player_id: &str) -> Result<(), String> {
    if g.host.as_deref() == Some(player_id) {
        Ok(())
    } else {
        Err("仅房主可执行该操作".into())
    }
}

fn abort(g: &mut Game, lobby: &mut Option<Lobby>, setup: &mut Option<Setup>, reason: &str) {
    g.abort_reason = Some(reason.to_string());
    g.winner = None;
    g.pending = None;
    g.stack.clear();
    *lobby = None;
    *setup = None;
}

fn new_bot_id(g: &Game) -> String {
    (1..=1000)
        .map(|i| format!("bot:{i}"))
        .find(|id| g.seat_of(id).is_none())
        .unwrap_or_else(|| "bot:overflow".to_string())
}

fn apply_config(cfg: &mut LobbyConfig, key: &str, value: &str) -> Result<(), String> {
    if key == "packs" {
        let keys: Vec<&str> = value.split('+').filter(|s| !s.is_empty()).collect();
        if keys.is_empty() || keys.iter().any(|k| !matches!(*k, "standard" | "junzheng")) {
            return Err(format!("非法牌堆包：{value}"));
        }
        cfg.packs = value.to_string();
        return Ok(());
    }
    if key == "general_packs" {
        let keys: Vec<&str> = value.split('+').filter(|s| !s.is_empty()).collect();
        if keys.is_empty() || keys.iter().any(|k| !GENERAL_PACKS.contains(k)) {
            return Err(format!("非法武将包：{value}"));
        }
        if !keys.contains(&"standard") {
            return Err("武将包必须包含 standard（基数）".into());
        }
        cfg.general_packs = value.to_string();
        return Ok(());
    }
    let v: u32 = value.parse().map_err(|_| format!("非法配置值：{value}"))?;
    match key {
        "lord_candidate_count" => {
            if v > MAX_CANDIDATE_COUNT {
                return Err(format!("主公候选张数需 ≤ {MAX_CANDIDATE_COUNT}"));
            }
            cfg.lord_candidate_count = v;
        }
        "other_candidate_count" => {
            if v == 0 || v > MAX_CANDIDATE_COUNT {
                return Err(format!("其他玩家候选张数需在 1–{MAX_CANDIDATE_COUNT}"));
            }
            cfg.other_candidate_count = v;
        }
        "max_hand_cards" => {
            if v > MAX_HAND_CARDS_LIMIT {
                return Err(format!("持有上限需 ≤ {MAX_HAND_CARDS_LIMIT}"));
            }
            cfg.max_hand_cards = v;
        }
        "double_spy" => cfg.double_spy = v != 0,
        _ => return Err(format!("未知配置项：{key}")),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 开局 → 选将 → 发牌
// ---------------------------------------------------------------------------

/// 发身份 + 给主公发候选；返回主公座位。
pub fn start(
    g: &mut Game,
    cfg: &LobbyConfig,
    lobby: &mut Option<Lobby>,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<usize, String> {
    let n = g.seats.len();
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
        return Err(format!("人数需在 {MIN_PLAYERS}–{MAX_PLAYERS} 之间（当前 {n}）"));
    }
    if cfg.double_spy && n != 8 {
        return Err("双内局仅支持 8 人".into());
    }
    let lord = assign_identities(g, cfg.double_spy, rng)?;
    *lobby = None;
    *setup = Some(Setup {
        lord,
        ..Default::default()
    });
    g.turn = lord;
    offer_to_lord(g, cfg, setup, rng)?;
    auto_pick_bots(g, cfg, setup, rng)?;
    maybe_finish(g, cfg, setup, rng)?;
    Ok(lord)
}

/// 发身份（座位不重复）：5 人 1 主 1 忠 2 反 1 内；6 人 1/1/3/1；7 人 1/2/3/1；8 人 1/2/4/1（双内 1/1/4/2）。
fn assign_identities(
    g: &mut Game,
    double_spy: bool,
    rng: &mut dyn RandomSource,
) -> Result<usize, String> {
    let n = g.seats.len();
    let (loyal, rebel, spy) = match (n, double_spy) {
        (5, _) => (1, 2, 1),
        (6, _) => (1, 3, 1),
        (7, _) => (2, 3, 1),
        (8, false) => (2, 4, 1),
        (8, true) => (1, 4, 2),
        _ => return Err(format!("人数需在 {MIN_PLAYERS}–{MAX_PLAYERS} 之间（当前 {n}）")),
    };
    let lord = pick_index(rng, n);
    let mut pool: Vec<Identity> = Vec::new();
    for _ in 0..loyal {
        pool.push(Identity::Loyalist);
    }
    for _ in 0..rebel {
        pool.push(Identity::Rebel);
    }
    for _ in 0..spy {
        pool.push(Identity::Spy);
    }
    shuffle(rng, &mut pool);
    let mut k = 0usize;
    for i in 0..n {
        g.seats[i].identity = Some(if i == lord {
            Identity::Lord
        } else {
            let id = pool[k];
            k += 1;
            id
        });
    }
    g.turn = lord;
    Ok(lord)
}

/// 给主公发候选：包内全部主公将 + `lord_candidate_count` 张非主公（洗牌）。
fn offer_to_lord(
    _g: &mut Game,
    cfg: &LobbyConfig,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<(), String> {
    let pool = available_codes(cfg);
    let (mut lord_codes, mut non_lord): (Vec<&str>, Vec<&str>) = pool
        .into_iter()
        .partition(|c| generals::def(c).map(|d| d.lord).unwrap_or(false));
    shuffle(rng, &mut lord_codes);
    shuffle(rng, &mut non_lord);
    let extra = (cfg.lord_candidate_count as usize).min(non_lord.len());
    let mut offers: Vec<String> = lord_codes.iter().map(|c| (*c).to_string()).collect();
    offers.extend(non_lord[..extra].iter().map(|c| (*c).to_string()));
    if offers.is_empty() {
        return Err("武将池为空（检查武将包配置）".into());
    }
    let lord = setup.as_ref().map(|s| s.lord).unwrap_or(0);
    if let Some(s) = setup.as_mut() {
        s.offers.insert(lord, offers);
    }
    Ok(())
}

/// 主公选完 → 按座位顺序给其他人发候选（互不重复）。
fn distribute_offers(
    g: &Game,
    cfg: &LobbyConfig,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<(), String> {
    let Some(st) = setup.as_ref() else {
        return Ok(());
    };
    let lord = st.lord;
    let picks: Vec<String> = st.picks.values().cloned().collect();
    let mut pool: Vec<String> = available_codes(cfg)
        .into_iter()
        .map(str::to_string)
        .filter(|c| !picks.contains(c))
        .collect();
    shuffle(rng, &mut pool);
    let n = g.seats.len();
    let targets: Vec<usize> = (1..n)
        .map(|i| (lord + i) % n)
        .filter(|i| setup.as_ref().is_some_and(|s| !s.picks.contains_key(i)))
        .collect();
    let per = cfg.other_candidate_count as usize;
    let count = targets.len();
    if count == 0 {
        return Ok(());
    }
    let base = (pool.len() / count).min(per);
    let remainder = pool.len() % count;
    for (k, seat) in targets.into_iter().enumerate() {
        let take_n = if base < per && k < remainder {
            base + 1
        } else {
            base
        }
        .min(pool.len());
        let take: Vec<String> = pool.drain(..take_n).collect();
        if let Some(s) = setup.as_mut() {
            s.offers.insert(seat, take);
        }
    }
    Ok(())
}

fn pick(
    g: &mut Game,
    cfg: &LobbyConfig,
    lobby: &mut Option<Lobby>,
    setup: &mut Option<Setup>,
    player_id: &str,
    code: &str,
    rng: &mut dyn RandomSource,
) -> Result<String, String> {
    let Some(st) = setup.as_ref() else {
        return Err("当前不是选将阶段".into());
    };
    let idx = g.seat_of(player_id).ok_or_else(|| "不在座玩家".to_string())?;
    if st.picks.contains_key(&idx) {
        return Err("已选过武将".into());
    }
    let offers = st
        .offers
        .get(&idx)
        .cloned()
        .ok_or_else(|| "尚未收到候选武将".to_string())?;
    if !offers.iter().any(|c| c == code) {
        return Err("该武将不在你的候选中".into());
    }
    let def = generals::def(code).ok_or_else(|| "未知武将".to_string())?;
    let is_lord = st.lord == idx;
    if let Some(s) = setup.as_mut() {
        s.picks.insert(idx, code.to_string());
        if is_lord {
            s.lord_picked = true;
        }
    }
    let name = def.name;
    if is_lord {
        distribute_offers(g, cfg, setup, rng)?;
        auto_pick_bots(g, cfg, setup, rng)?;
    }
    maybe_finish(g, cfg, setup, rng)?;
    let _ = lobby;
    Ok(format!("已选择武将：{name}"))
}

/// 机器人从自己的候选里固定取第一张（确定性）。
fn auto_pick_bots(
    g: &mut Game,
    cfg: &LobbyConfig,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<(), String> {
    loop {
        let Some((idx, code)) = setup.as_ref().and_then(|s| {
            g.seats
                .iter()
                .enumerate()
                .find(|(i, seat)| {
                    seat.bot
                        && !s.picks.contains_key(i)
                        && s.offers.get(i).is_some_and(|o| !o.is_empty())
                })
                .and_then(|(i, _)| {
                    s.offers
                        .get(&i)
                        .and_then(|o| o.first())
                        .map(|c| (i, c.clone()))
                })
        }) else {
            break;
        };
        let is_lord = setup.as_ref().is_some_and(|s| s.lord == idx);
        if let Some(s) = setup.as_mut() {
            s.picks.insert(idx, code);
            if is_lord {
                s.lord_picked = true;
            }
        }
        if is_lord {
            distribute_offers(g, cfg, setup, rng)?;
        }
    }
    Ok(())
}

fn maybe_finish(
    g: &mut Game,
    cfg: &LobbyConfig,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<(), String> {
    let ready = setup
        .as_ref()
        .is_some_and(|s| s.picks.len() == g.seats.len());
    if ready {
        finish_setup(g, cfg, setup, rng)?;
    }
    Ok(())
}

/// 武将落地（正式选将/工作台沙盒共用）：势力/性别/体力上限 + 技能挂载（已收口）。
///
/// `lord_bonus` = 主公 +1 体力上限；技能只挂 `ENABLED_SKILLS`（未收口批次不挂）。
pub fn mount_general(g: &mut Game, seat: usize, code: &str) -> Result<(), String> {
    let is_lord = g.seats.get(seat).and_then(|s| s.identity) == Some(Identity::Lord);
    mount_general_as(g, seat, code, is_lord)
}

fn mount_general_as(g: &mut Game, seat: usize, code: &str, is_lord: bool) -> Result<(), String> {
    let def = generals::def(code).ok_or_else(|| format!("未知武将 {code}"))?;
    let Some(s) = g.seats.get_mut(seat) else {
        return Err(format!("座位 {seat} 不存在"));
    };
    let bonus = if is_lord { 1 } else { 0 };
    s.general = code.to_string();
    s.kingdom = def.kingdom.to_string();
    s.male = def.male;
    s.max_hp = def.max_hp + bonus;
    s.hp = s.max_hp;
    s.alive = true;
    // 技能挂载：中文名 → code（V3 注册表），只挂**已收口**批次（`skills::ENABLED_SKILLS`）。
    s.skills = def
        .skills
        .iter()
        .filter_map(|n| crate::kernel::skills::code_by_name(n))
        .filter(|c| crate::kernel::skills::enabled(c))
        .map(|c| c.to_string())
        .collect();
    s.skills.dedup();
    s.granted.clear();
    Ok(())
}

/// 选将完成：定体力/势力/性别 → 组牌堆 → 发 4 张 → 进入主公回合。
///
/// 技能**暂不挂载**（V3 按 B0–B20 批次接入）：本阶段目标是「武将/手牌/装备/锦囊」可
/// 完整对局，不依赖任何武将技能。
pub fn finish_setup(
    g: &mut Game,
    cfg: &LobbyConfig,
    setup: &mut Option<Setup>,
    rng: &mut dyn RandomSource,
) -> Result<(), String> {
    let st = setup.clone().ok_or_else(|| "不在选将阶段".to_string())?;
    if st.picks.len() != g.seats.len() {
        return Err("选将未完成".into());
    }
    let lord = st.lord;
    for i in 0..g.seats.len() {
        let code = st
            .picks
            .get(&i)
            .cloned()
            .ok_or_else(|| "缺少选将结果".to_string())?;
        mount_general_as(g, i, &code, i == lord)?;
    }
    *setup = None;
    g.hand_cap = cfg.max_hand_cards as usize;
    g.rng = rng.next_u64() | 1;
    g.log.clear();
    build_deck(g, &cfg.packs, rng)?;
    for _ in 0..INITIAL_HAND {
        for i in 0..g.seats.len() {
            flow::draw(g, i, 1);
        }
    }
    g.started = true;
    g.turn = lord;
    crate::kernel::skills::fire_game_start(g);
    turn::start(g)?;
    Ok(())
}

/// 组牌堆（按包过滤；实例 id `c1..cN` 确定性 = 生成物顺序）。
pub fn build_deck(g: &mut Game, packs: &str, rng: &mut dyn RandomSource) -> Result<(), String> {
    g.cards.clear();
    g.card_defs.clear();
    g.deck.clear();
    g.judge_stacks.clear();
    let keys: Vec<&str> = packs.split('+').filter(|s| !s.is_empty()).collect();
    let mut ids: Vec<String> = Vec::new();
    let mut n = 0usize;
    for phy in crate::cards_generated::DECK.iter() {
        if !keys.is_empty() && !keys.contains(&phy.pack) {
            continue;
        }
        let Some(def) = crate::cards_generated::DEFS.iter().find(|d| d.name == phy.name) else {
            continue;
        };
        n += 1;
        let id = format!("c{n}");
        g.card_defs.insert(
            id.clone(),
            Card {
                id: id.clone(),
                def: def.code.to_string(),
                suit: phy.suit,
                rank: phy.rank,
            },
        );
        g.cards.insert(id.clone(), Zone::Draw);
        ids.push(id);
    }
    if ids.is_empty() {
        return Err("牌堆为空（检查牌堆包配置）".into());
    }
    shuffle(rng, &mut ids);
    g.deck = ids;
    Ok(())
}

/// 花色 → 视图 key（供 `engine::view` 复用）。
pub fn suit_key(suit: Suit) -> &'static str {
    match suit {
        Suit::Spade => "spade",
        Suit::Heart => "heart",
        Suit::Club => "club",
        Suit::Diamond => "diamond",
        Suit::None => "none",
    }
}
