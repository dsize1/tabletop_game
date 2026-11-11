//! 大厅与选将（S1）：`lobby:*` 房间动作 + `Phase::Setup` 选将。
//!
//! 与 v1 保持**动作 id 完全一致**（`lobby:join` / `lobby:start` / `setup:choose_general:<code>` …），
//! 这样 `apps/agent-gateway`（`#加入`/`#开始`/`#加机器人`/`#武将包`）与 Web 选将页无需改协议。
//!
//! 状态机：
//! ```text
//! new_game → Lobby（玩家 join / 机器人 add_bot / 配置 set_*）
//!          → lobby:start（发身份 + 主公候选）→ Setup（主公先选 → 其他人候选）
//!          → 全部选完 → finish_setup（定体力/建牌堆/发 4 张/进入回合）
//! ```
//!
//! 约束：
//! - 身份/奖惩/胜负只在 `Game.identity_game = true` 时生效（单测夹具不受影响）；
//! - 随机性走 `Game::rand_below`（确定性、可序列化、回放一致）；
//! - 不在本模块改 hp/牌区（选将只写座位属性，牌堆只 `shuffle_draw` / `to_void`）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cards;
use crate::generals::{self, Status};
use crate::identity;
use crate::kernel::{Game, Seat};
use crate::skills;

pub const MIN_PLAYERS: usize = 5;
pub const MAX_PLAYERS: usize = 8;
pub const MAX_CANDIDATE_COUNT: u32 = 7;
pub const MAX_HAND_CARDS_LIMIT: u32 = 30;
pub const INITIAL_HAND: usize = 4;

/// 房间配置（大厅可改；开局后固定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyConfig {
    /// 手牌持有上限（弃牌阶段；默认 20，菜单可选 10/20/30）。
    pub max_hand_cards: u32,
    /// 主公候选中的非主公将数量（默认 5）。
    pub lord_candidate_count: u32,
    /// 其他玩家候选数量（默认 5）。
    pub other_candidate_count: u32,
    /// 8 人双内局（1 主 1 忠 4 反 2 内）。
    pub double_spy: bool,
    /// 牌堆包组合（`standard` / `standard+junzheng`）。
    pub packs: String,
    /// 武将包组合（`standard` 恒为基数；`+` 连接）。
    pub general_packs: String,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            max_hand_cards: 20,
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

/// 大厅（仅表示「还在房间阶段」；配置在 `Game.cfg`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lobby {
    pub host: String,
}

/// 选将状态。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Setup {
    /// 主公座位。
    pub lord: usize,
    /// 座位 → 候选武将 code。
    pub offers: BTreeMap<usize, Vec<String>>,
    /// 座位 → 已选武将 code。
    pub picks: BTreeMap<usize, String>,
    /// 主公是否已选（决定何时给其他人发候选）。
    pub lord_picked: bool,
}

// ---------------------------------------------------------------------------
// 武将池
// ---------------------------------------------------------------------------

/// 可选武将 code（已上线 + 技能全部在 v2 注册表内 + 属于已选武将包）。
///
/// 「技能全部在注册表内」是硬门槛：注册后若某 live 武将的技能还没实现，
/// 该武将不进选将池（否则玩家选到会缺技能）。
pub fn available_codes(cfg: &LobbyConfig) -> Vec<&'static str> {
    let packs = cfg.general_pack_keys();
    generals::GENERALS
        .iter()
        .filter(|g| g.status == Status::Live)
        .filter(|g| packs.is_empty() || packs.contains(&g.pack))
        .filter(|g| g.skills.iter().all(|n| skills::code_by_name(n).is_some()))
        .map(|g| g.code)
        .collect()
}

/// 武将包组合预设（`standard` 基数 + 其余包的全部子集；按包数升序）。
pub fn general_pack_presets() -> Vec<String> {
    let others: Vec<&str> = generals::PACK_KEYS
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

pub fn general_pack_label(cfg: &LobbyConfig, mode: &str) -> (String, usize) {
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
    let count = available_codes(&LobbyConfig {
        general_packs: mode.to_string(),
        ..cfg.clone()
    })
    .len();
    (label, count)
}

// ---------------------------------------------------------------------------
// 合法动作（与 `apply` 同源，见 `legal ≡ execute` 门禁）
// ---------------------------------------------------------------------------

pub fn seat_of(g: &Game, player_id: &str) -> Option<usize> {
    g.seats.iter().position(|s| s.name == player_id)
}

/// 大厅 / 选将阶段的合法动作（`(id, description)`）。
pub fn legal_actions(g: &Game, player_id: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let seated = seat_of(g, player_id);
    if let Some(lobby) = &g.lobby {
        let n = g.seats.len();
        if seated.is_none() {
            if n < MAX_PLAYERS {
                out.push(("lobby:join".into(), "加入游戏".into()));
            }
            return out;
        }
        out.push(("lobby:leave".into(), "退出房间".into()));
        if lobby.host != player_id {
            return out;
        }
        if (MIN_PLAYERS..=MAX_PLAYERS).contains(&n) && !(g.cfg.double_spy && n != 8) {
            out.push(("lobby:start".into(), "开始游戏（发身份 → 选将）".into()));
        }
        if n < MAX_PLAYERS {
            out.push(("lobby:add_bot".into(), "加入机器人".into()));
        }
        out.push(("lobby:abort".into(), "中断本局（不结算）".into()));
        for s in &g.seats {
            if s.name != lobby.host && !s.bot {
                out.push((format!("lobby:kick:{}", s.name), format!("踢出 {}", s.name)));
            }
        }
        for v in [3u32, 5, 7] {
            if v != g.cfg.lord_candidate_count {
                out.push((
                    format!("lobby:set_lord_candidate_count:{v}"),
                    format!("主公候选非主公将 {v} 张"),
                ));
            }
        }
        for v in [3u32, 5, 7] {
            if v != g.cfg.other_candidate_count {
                out.push((
                    format!("lobby:set_other_candidate_count:{v}"),
                    format!("其他玩家候选 {v} 张"),
                ));
            }
        }
        for v in [10u32, 20, 30] {
            if v != g.cfg.max_hand_cards {
                out.push((
                    format!("lobby:set_max_hand_cards:{v}"),
                    format!("手牌持有上限 {v}"),
                ));
            }
        }
        if n == 8 {
            out.push((
                format!("lobby:set_double_spy:{}", u8::from(!g.cfg.double_spy)),
                if g.cfg.double_spy {
                    "关闭双内局".into()
                } else {
                    "开启双内局（1 主 1 忠 4 反 2 内）".into()
                },
            ));
        }
        for mode in ["standard", "standard+junzheng"] {
            if g.cfg.packs != mode {
                out.push((
                    format!("lobby:set_packs:{mode}"),
                    if mode == "standard" {
                        "牌堆：标准包（108 张）".into()
                    } else {
                        "牌堆：标准 + 军争（160 张）".into()
                    },
                ));
            }
        }
        for mode in general_pack_presets() {
            if g.cfg.general_packs != mode {
                let (label, count) = general_pack_label(&g.cfg, &mode);
                out.push((
                    format!("lobby:set_general_packs:{mode}"),
                    format!("武将包：{label}（可用 {count} 位）"),
                ));
            }
        }
        return out;
    }
    if let Some(setup) = &g.setup {
        if let Some(idx) = seated {
            if !setup.picks.contains_key(&idx) {
                for code in setup.offers.get(&idx).cloned().unwrap_or_default() {
                    let name = generals::by_code(&code).map(|d| d.name).unwrap_or("");
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

// ---------------------------------------------------------------------------
// 执行
// ---------------------------------------------------------------------------

pub fn apply(g: &mut Game, player_id: &str, action: &str) -> Result<String, String> {
    if let Some(rest) = action.strip_prefix("lobby:") {
        return apply_lobby(g, player_id, rest);
    }
    if let Some(code) = action.strip_prefix("setup:choose_general:") {
        return pick(g, player_id, code);
    }
    Err(format!("未知房间动作：{action}"))
}

fn apply_lobby(g: &mut Game, player_id: &str, rest: &str) -> Result<String, String> {
    // `abort` 是运维动作：任何未结束阶段都可中断（群侧 `#中断`）。
    if g.lobby.is_none() && rest != "abort" {
        return Err("已开局，不能修改房间".into());
    }
    let seated = seat_of(g, player_id);
    match rest {
        "join" => {
            if seated.is_some() {
                return Err("已在房间中".into());
            }
            if g.seats.len() >= MAX_PLAYERS {
                return Err("房间已满".into());
            }
            g.seats
                .push(Seat::new(player_id, "", &[], 0));
            Ok(format!("已加入（{}/{}）", g.seats.len(), MAX_PLAYERS))
        }
        "leave" => {
            let idx = seated.ok_or_else(|| "不在房间中".to_string())?;
            if g.host == player_id {
                abort(g, "房主离开，对局废弃");
                Ok("房主已离开，对局废弃".to_string())
            } else {
                g.seats.remove(idx);
                Ok(format!("已退出（{}/{}）", g.seats.len(), MAX_PLAYERS))
            }
        }
        "start" => {
            require_host(g, player_id)?;
            let n = g.seats.len();
            if n < MIN_PLAYERS {
                return Err(format!("至少 {MIN_PLAYERS} 人才能开始（当前 {n}）"));
            }
            let lord = start(g)?;
            Ok(format!("已开局：{n} 人，主公先选将（座位 {}）", lord + 1))
        }
        "abort" => {
            require_host(g, player_id)?;
            abort(g, "房主中断");
            Ok("已中断本局（不结算）".to_string())
        }
        "add_bot" => {
            require_host(g, player_id)?;
            if g.seats.len() >= MAX_PLAYERS {
                return Err("房间已满".into());
            }
            let bot = new_bot_id(g);
            let mut seat = Seat::new(&bot, "", &[], 0);
            seat.bot = true;
            g.seats.push(seat);
            Ok(format!(
                "已加入机器人 {bot}（{}/{}）",
                g.seats.len(),
                MAX_PLAYERS
            ))
        }
        _ if rest.starts_with("kick:") => {
            require_host(g, player_id)?;
            let target = &rest["kick:".len()..];
            if target == g.host {
                return Err("不能踢出房主".into());
            }
            let idx = seat_of(g, target).ok_or_else(|| "目标不在房间中".to_string())?;
            g.seats.remove(idx);
            Ok(format!("已踢出 {target}"))
        }
        _ if rest.starts_with("set_") => {
            require_host(g, player_id)?;
            let mut it = rest.splitn(2, ':');
            let key = it.next().unwrap_or_default();
            let value = it.next().ok_or_else(|| "缺少配置值".to_string())?;
            let key = key.trim_start_matches("set_");
            apply_config(g, key, value)?;
            Ok(format!("已设置 {key} = {value}"))
        }
        _ => Err(format!("未知房间动作：{rest}")),
    }
}

fn require_host(g: &Game, player_id: &str) -> Result<(), String> {
    // 房主身份在 `Game.host`（开局后 `lobby` 会被清空，但 `#中断` 仍需房主权限）。
    if g.host == player_id {
        Ok(())
    } else {
        Err("仅房主可执行该操作".into())
    }
}

fn abort(g: &mut Game, reason: &str) {
    g.finished = true;
    g.abort_reason = Some(reason.to_string());
    g.lobby = None;
    g.setup = None;
}

fn new_bot_id(g: &Game) -> String {
    (1..=1000)
        .map(|i| format!("bot:{i}"))
        .find(|id| !g.seats.iter().any(|s| s.name == *id))
        .unwrap_or_else(|| "bot:overflow".to_string())
}

fn apply_config(g: &mut Game, key: &str, value: &str) -> Result<(), String> {
    if key == "packs" {
        let keys: Vec<&str> = value.split('+').filter(|s| !s.is_empty()).collect();
        let known = cards::pack_keys();
        if keys.is_empty() || keys.iter().any(|k| !known.contains(k)) {
            return Err(format!("非法牌堆包：{value}"));
        }
        g.cfg.packs = value.to_string();
        return Ok(());
    }
    if key == "general_packs" {
        let keys: Vec<&str> = value.split('+').filter(|s| !s.is_empty()).collect();
        if keys.is_empty()
            || keys
                .iter()
                .any(|k| !generals::PACK_KEYS.contains(k))
        {
            return Err(format!("非法武将包：{value}"));
        }
        if !keys.contains(&"standard") {
            return Err("武将包必须包含 standard（基数）".into());
        }
        g.cfg.general_packs = value.to_string();
        return Ok(());
    }
    let v: u32 = value
        .parse()
        .map_err(|_| format!("非法配置值：{value}"))?;
    match key {
        "lord_candidate_count" => {
            if v > MAX_CANDIDATE_COUNT {
                return Err(format!("主公候选张数需 ≤ {MAX_CANDIDATE_COUNT}"));
            }
            g.cfg.lord_candidate_count = v;
        }
        "other_candidate_count" => {
            if v == 0 || v > MAX_CANDIDATE_COUNT {
                return Err(format!("其他玩家候选张数需在 1–{MAX_CANDIDATE_COUNT}"));
            }
            g.cfg.other_candidate_count = v;
        }
        "max_hand_cards" => {
            if v == 0 || v > MAX_HAND_CARDS_LIMIT {
                return Err(format!("持有上限需在 1–{MAX_HAND_CARDS_LIMIT}"));
            }
            g.cfg.max_hand_cards = v;
        }
        "double_spy" => g.cfg.double_spy = v != 0,
        _ => return Err(format!("未知配置项：{key}")),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 开局 → 选将 → 发牌
// ---------------------------------------------------------------------------

/// 发身份 + 给主公发候选；返回主公座位。
pub fn start(g: &mut Game) -> Result<usize, String> {
    let n = g.seats.len();
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
        return Err(format!(
            "人数需在 {MIN_PLAYERS}–{MAX_PLAYERS} 之间（当前 {n}）"
        ));
    }
    if g.cfg.double_spy && n != 8 {
        return Err("双内局仅支持 8 人".into());
    }
    let lord = identity::assign(g, g.cfg.double_spy)?;
    g.lobby = None;
    g.setup = Some(Setup {
        lord,
        ..Default::default()
    });
    g.turn = lord;
    offer_to_lord(g)?;
    auto_pick_bots(g)?;
    maybe_finish(g)?;
    Ok(lord)
}

/// 给主公发候选：包内全部主公将 + `lord_candidate_count` 张非主公（洗牌）。
fn offer_to_lord(g: &mut Game) -> Result<(), String> {
    let pool = available_codes(&g.cfg);
    let (mut lord_codes, mut non_lord): (Vec<&str>, Vec<&str>) = pool
        .into_iter()
        .partition(|c| generals::by_code(c).map(|d| d.lord).unwrap_or(false));
    identity::shuffle(g, &mut lord_codes);
    identity::shuffle(g, &mut non_lord);
    let extra = (g.cfg.lord_candidate_count as usize).min(non_lord.len());
    let mut offers: Vec<String> = lord_codes.iter().map(|c| (*c).to_string()).collect();
    offers.extend(non_lord[..extra].iter().map(|c| (*c).to_string()));
    if offers.is_empty() {
        return Err("武将池为空（检查武将包配置）".into());
    }
    let lord = g.setup.as_ref().map(|s| s.lord).unwrap_or(0);
    if let Some(setup) = g.setup.as_mut() {
        setup.offers.insert(lord, offers);
    }
    Ok(())
}

/// 主公选完 → 按座位顺序给其他人发候选（互不重复）。
fn distribute_offers(g: &mut Game) -> Result<(), String> {
    let Some(setup) = g.setup.as_ref() else {
        return Ok(());
    };
    let lord = setup.lord;
    let picks: Vec<String> = setup.picks.values().cloned().collect();
    let mut pool: Vec<String> = available_codes(&g.cfg)
        .into_iter()
        .map(str::to_string)
        .filter(|c| !picks.contains(c))
        .collect();
    identity::shuffle(g, &mut pool);

    let n = g.seats.len();
    let targets: Vec<usize> = (1..n)
        .map(|i| (lord + i) % n)
        .filter(|i| {
            g.setup
                .as_ref()
                .is_some_and(|s| !s.picks.contains_key(i))
        })
        .collect();
    let per = g.cfg.other_candidate_count as usize;
    let count = targets.len();
    if count == 0 {
        return Ok(());
    }
    // 8 人局 7×M 可能超过池子 → 均分降级（每人不低于 1）。
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
        if let Some(s) = g.setup.as_mut() {
            s.offers.insert(seat, take);
        }
    }
    Ok(())
}

fn pick(g: &mut Game, player_id: &str, code: &str) -> Result<String, String> {
    let Some(setup) = g.setup.as_ref() else {
        return Err("当前不是选将阶段".into());
    };
    let idx = seat_of(g, player_id).ok_or_else(|| "不在座玩家".to_string())?;
    if setup.picks.contains_key(&idx) {
        return Err("已选过武将".into());
    }
    let offers = setup
        .offers
        .get(&idx)
        .cloned()
        .ok_or_else(|| "尚未收到候选武将".to_string())?;
    if !offers.iter().any(|c| c == code) {
        return Err("该武将不在你的候选中".into());
    }
    let def = generals::by_code(code).ok_or_else(|| "未知武将".to_string())?;
    let is_lord = setup.lord == idx;
    if let Some(s) = g.setup.as_mut() {
        s.picks.insert(idx, code.to_string());
        if is_lord {
            s.lord_picked = true;
        }
    }
    if is_lord {
        distribute_offers(g)?;
        // 主公选完才给其他人发候选；机器人立刻补选。
        auto_pick_bots(g)?;
    }
    maybe_finish(g)?;
    Ok(format!("已选择武将：{}", def.name))
}

/// 机器人从自己的候选里固定取第一张（确定性）。
fn auto_pick_bots(g: &mut Game) -> Result<(), String> {
    loop {
        let Some((idx, code)) = g.setup.as_ref().and_then(|s| {
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
        let def = generals::by_code(&code).ok_or_else(|| "未知武将".to_string())?;
        let is_lord = g.setup.as_ref().is_some_and(|s| s.lord == idx);
        if let Some(s) = g.setup.as_mut() {
            s.picks.insert(idx, code);
            if is_lord {
                s.lord_picked = true;
            }
        }
        let _ = def;
        if is_lord {
            distribute_offers(g)?;
        }    }
    Ok(())
}

fn maybe_finish(g: &mut Game) -> Result<(), String> {
    let ready = g
        .setup
        .as_ref()
        .is_some_and(|s| s.picks.len() == g.seats.len());
    if ready {
        finish_setup(g)?;
    }
    Ok(())
}

/// 选将完成：定体力/势力/技能 → 组牌堆 → 发初始手牌 → 进入主公回合。
pub fn finish_setup(g: &mut Game) -> Result<(), String> {
    let setup = g.setup.clone().ok_or_else(|| "不在选将阶段".to_string())?;
    // 选将收尾 + 组牌堆 + 发牌 + 开局技全部属于 Setup：期间产生的事件不触发技能。
    g.lifecycle = crate::kernel::event::Lifecycle::Setup;
    if setup.picks.len() != g.seats.len() {
        return Err("选将未完成".into());
    }
    let lord = setup.lord;
    for i in 0..g.seats.len() {
        let code = setup
            .picks
            .get(&i)
            .cloned()
            .ok_or_else(|| "缺少选将结果".to_string())?;
        let def = generals::by_code(&code).ok_or_else(|| format!("未知武将 {code}"))?;
        let mut skills: Vec<String> = Vec::new();
        for name in def.skills {
            let sc = skills::code_by_name(name)
                .ok_or_else(|| format!("{} 的技能「{name}」未在 v2 注册表", def.name))?;
            if !skills.iter().any(|s| s == sc) {
                skills.push(sc.to_string());
            }
        }
        let bonus = if i == lord { 1 } else { 0 };
        let seat = &mut g.seats[i];
        seat.general = code;
        seat.kingdom = def.kingdom.to_string();
        seat.male = def.male;
        seat.max_hp = def.max_hp + bonus;
        seat.hp = seat.max_hp;
        seat.skills = skills;
        if i == lord {
            seat.flags.insert(identity::LORD.to_string());
        }
    }

    // 牌堆：未选中的包移出游戏（保持全量守恒）；再确定性洗牌。
    let packs = g.cfg.pack_keys();
    let keep: std::collections::BTreeSet<String> =
        cards::deck_of(&packs).into_iter().map(|c| c.id).collect();
    let extra: Vec<String> = g
        .ids_in(|z| *z == crate::kernel::Zone::Draw)
        .into_iter()
        .filter(|id| !keep.contains(id))
        .collect();
    for id in extra {
        g.to_void(&id)?;
    }
    g.shuffle_draw();

    // 初始手牌 4 张（逐个座位轮流摸）。发牌属于 Setup 生命周期：事件会带
    // `Lifecycle::Setup` 标记，路由层直接丢弃（发牌不是「获得」，见 engine-self-audit M2）。
    for _ in 0..INITIAL_HAND {
        for i in 0..g.seats.len() {
            g.draw(i, 1);
        }
    }

    g.setup = None;
    g.identity_game = true;
    skills::rebuild_subs(g);
    skills::on_game_start(g);
    crate::turn::begin_game_at(g, lord)
}

/// 一键开局（测试/示例/自对弈用）：建局 → 开赛 → 全员自动选将。
pub fn default_start(g: &mut Game) -> Result<(), String> {
    start(g)?;
    // 人类座位也自动选第一张候选（保持「无外部输入也能打满」）。
    loop {
        let Some((idx, code)) = g.setup.as_ref().and_then(|s| {
            (0..g.seats.len())
                .find(|i| {
                    !s.picks.contains_key(i) && s.offers.get(i).is_some_and(|o| !o.is_empty())
                })
                .and_then(|i| {
                    s.offers
                        .get(&i)
                        .and_then(|o| o.first())
                        .map(|c| (i, c.clone()))
                })
        }) else {
            break;
        };
        let pid = g.seats[idx].name.clone();
        pick(g, &pid, &code)?;
    }
    maybe_finish(g)?;
    Ok(())
}
