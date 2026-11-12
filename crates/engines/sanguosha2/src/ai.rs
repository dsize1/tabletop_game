//! 三国杀 v2 AI（S2）：**启发式**决策 + 机器人推进（`drive_bots`）。
//!
//! 分层（比 v1 简化，但不做「首个可行动作」）：
//! ```text
//! GameState → Game（parse）
//!    ├─ pending：按 kind 分类（濒死/响应/选牌/选目标/多选/弃牌…）逐类打分
//!    └─ 出牌阶段：动作打分（桃/装备/杀/锦囊/技能），目标取「最虚弱」的敌人
//!    → 最高分动作 → 引擎无条件重校验 → 下一步
//! ```
//!
//! 约束：
//! - **只用公开信息**：不读他人手牌明细、不读未公开身份、不读牌堆顺序（只读数量）；
//! - 确定性：无随机（随机源只用于引擎自身的洗牌/技能随机）；
//! - 兜底三层（与 v1 同口径）：动作被拒 → 回落 `pass/confirm/end_play`；状态回环 → 换未试过的动作；
//!   同一 (actor, action) 连续重复 8 次 → 判环中止。

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use game_engine_core::{Action, EngineEvent, GameManager, GameState, GameStatus, SeededRng};

use crate::cards::{CardKind, Effect};
use crate::kernel::{Game, Phase, Zone};
use crate::view;

/// 单次推进（一次 Web 动作之后）最多执行的机器人动作数。
pub const MAX_STEPS: usize = 200;

/// 停止原因（与 v1 同名字符串，server/gateway 文案不用改）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    HumanTurn,
    Finished,
    NoActor,
    NoLegalActions,
    MaxSteps,
    Loop,
    EngineError,
    NotSanguosha,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::HumanTurn => "human_turn",
            StopReason::Finished => "finished",
            StopReason::NoActor => "no_actor",
            StopReason::NoLegalActions => "no_legal_actions",
            StopReason::MaxSteps => "max_steps",
            StopReason::Loop => "loop",
            StopReason::EngineError => "engine_error",
            StopReason::NotSanguosha => "not_sanguosha",
        }
    }
}

/// 一条机器人决策轨迹（决策日志用；`seq` 对齐该步落库后的 `game_events.seq`）。
#[derive(Debug, Clone)]
pub struct BotDecision {
    pub actor: String,
    pub seq: i64,
    pub legal: Vec<Action>,
    pub candidates: Vec<Action>,
    pub chosen: String,
    pub priority: i32,
    pub score: f64,
    pub reason: String,
    pub state_digest: String,
    pub latency_ms: i64,
}

/// 一次推进的结果（与 v1 `DriveReport` 同名同义）。
#[derive(Debug, Clone, Default)]
pub struct DriveReport {
    pub steps: usize,
    pub decisions: Vec<BotDecision>,
    pub finished: bool,
    pub illegal: usize,
    pub loops: usize,
    pub stop: Option<StopReason>,
    pub last_error: Option<String>,
    pub errors: Vec<String>,
}

impl DriveReport {
    pub fn advanced(&self) -> bool {
        self.steps > 0
    }

    fn stop(mut self, reason: StopReason) -> Self {
        self.stop = Some(reason);
        self
    }

    fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.last_error = Some(msg.into());
        self
    }
}

/// 一条决策（含排障信息）。
#[derive(Debug, Clone)]
pub struct DecisionTrace {
    pub action: String,
    pub legal: Vec<Action>,
    pub candidates: Vec<Action>,
    pub priority: i32,
    pub score: f64,
    pub reason: String,
    pub state_digest: String,
}

// ---------------------------------------------------------------------------
// 摘要 / 待行动者
// ---------------------------------------------------------------------------

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 状态摘要（环检测/决策日志）：只含可见信息 + 交互进度（哈希），不含隐藏明细。
pub fn state_digest(g: &Game) -> String {
    let pending = g
        .pending
        .as_ref()
        .map(|p| {
            let data = serde_json::to_string(p.data()).unwrap_or_default();
            let opts: Vec<&str> = p.options().iter().map(|o| o.id.as_str()).collect();
            format!(
                "{}:{}:{:x}:{:x}",
                p.kind(),
                p.actor(),
                fnv1a(&data),
                fnv1a(&opts.join("|"))
            )
        })
        .unwrap_or_default();
    let seats: Vec<String> = g
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}:{}:{}:{}", i, s.hp, g.hand(i).len(), s.flipped as u8))
        .collect();
    let zones = format!(
        "{}:{}:{}:{}",
        g.ids_in(|z| *z == Zone::Draw).len(),
        g.ids_in(|z| *z == Zone::Discard).len(),
        g.ids_in(|z| *z == Zone::Processing).len(),
        g.ids_in(|z| *z == Zone::Void).len()
    );
    format!(
        "t{}:r{}:{:?}:{}|{}|{}|{}",
        g.turn, g.round, g.phase, g.turn_seq, pending, seats.join(","), zones
    )
}

/// `GameState` → 摘要（MCP/决策日志用；解析失败返回 None）。
pub fn state_digest_of(state: &GameState) -> Option<String> {
    let g: Game = serde_json::from_value(state.data.clone()).ok()?;
    Some(state_digest(&g))
}

// ---------------------------------------------------------------------------
// 动作打分
// ---------------------------------------------------------------------------

/// 手牌价值（弃牌/选牌用：越大越应保留）。
fn keep_value(g: &Game, id: &str) -> i32 {
    let def = g.def_of(&id.to_string());
    match def.effect {
        Effect::Tao => 100,
        Effect::Wuxie => 90,
        Effect::Sha => 70 + g.cards.get(id).map(|c| c.rank as i32).unwrap_or(0),
        Effect::Shan => 65,
        Effect::Jiu => 40,
        Effect::ExNihilo => 75,
        Effect::Duel | Effect::Dismantle | Effect::Snatch => 55,
        _ if def.kind == CardKind::Equip => 35,
        _ => 45,
    }
}

/// 目标偏好：血最少 > 手牌最少 > 座位号小（确定性）。
fn target_score(g: &Game, target: usize) -> i32 {
    let Some(s) = g.seats.get(target) else {
        return i32::MIN / 2;
    };
    if s.hp <= 0 {
        return i32::MIN / 2;
    }
    // 优先「快死了」的：血量越低越好打；手牌少=更脆。
    -(s.hp * 10) - (g.hand(target).len() as i32 * 2) - (target as i32)
}

/// 解析动作里的目标列表（`use:c1:2,3` / `target:2`）。
fn action_targets(action: &str) -> Vec<usize> {
    action
        .split(':')
        .skip(2)
        .flat_map(|s| s.split(','))
        .filter_map(|t| t.trim().parse::<usize>().ok())
        .collect()
}

fn best_target(g: &Game, seat: usize) -> Option<usize> {
    (0..g.seats.len())
        .filter(|t| *t != seat && g.seats[*t].hp > 0)
        .max_by_key(|t| target_score(g, *t))
}

/// 出牌阶段动作打分。
fn score_play_action(g: &Game, seat: usize, action: &str) -> (i32, String) {
    if action == "end_play" {
        return (5, "结束出牌".into());
    }
    if let Some(id) = action.strip_prefix("use:") {
        let id = id.split(':').next().unwrap_or_default().to_string();
        let name = g.name_of(&id);
        let def = g.def_of(&id);
        let targets = action_targets(action);
        let tgt_ok = !targets.is_empty() && targets.iter().all(|t| g.seats.get(*t).is_some_and(|s| s.hp > 0));
        let base = match def.effect {
            Effect::Tao => {
                // 终局房规：第 100 轮后桃不回血 → 不再打正分（别空耗手牌）。
                if g.endgame_no_heal() {
                    0
                } else if g.seats[seat].hp < g.seats[seat].max_hp {
                    120 + (g.seats[seat].max_hp - g.seats[seat].hp) * 5
                } else {
                    0
                }
            }
            Effect::Sha => {
                let t = targets.first().copied().unwrap_or(seat);
                let t_hp = g.seats.get(t).map(|s| s.hp).unwrap_or(9);
                let t_hand = g.hand(t).len() as i32;
                70 + if t_hp <= 1 { 30 } else { 0 } + if t_hand == 0 { 15 } else { 0 }
            }
            Effect::ExNihilo => 85,
            Effect::Duel => 60,
            Effect::Dismantle | Effect::Snatch => {
                let t = targets.first().copied().unwrap_or(seat);
                let has_equip = g.equip_in(t, crate::kernel::Slot::Weapon).is_some()
                    || g.equip_in(t, crate::kernel::Slot::Armor).is_some();
                60 + if has_equip { 10 } else { 0 }
            }
            Effect::Indulgence | Effect::SupplyShortage => 62,
            // 终局房规：桃园结义只剩回血 → 第 100 轮后无效，不给分。
            Effect::PeachGarden => {
                if g.endgame_no_heal() {
                    0
                } else {
                    58
                }
            }
            Effect::Harvest => 58,
            Effect::SavageAssault | Effect::ArcheryAttack => {
                // 自己血少时不放 AOE（怕被反噬）。
                if g.seats[seat].hp <= 2 {
                    30
                } else {
                    55
                }
            }
            Effect::Jiu => {
                if g.hand(seat).iter().any(|c| g.def_of(c).tags.sha) {
                    65
                } else {
                    10
                }
            }
            Effect::IronChain => 45,
            Effect::FireAttack => 50,
            Effect::Collateral => 40,
            _ if def.kind == CardKind::Equip => {
                if def.slot.is_some() {
                    80
                } else {
                    40
                }
            }
            _ => 40,
        };
        let score = if tgt_ok || targets.is_empty() { base } else { base - 60 };
        return (score, format!("使用【{name}】"));
    }
    if let Some(id) = action.strip_prefix("recast:") {
        let _ = id;
        return (42, "重铸".into());
    }
    if let Some(rest) = action.strip_prefix("trim:") {
        let _ = rest;
        return (35, "群体锦囊少指定目标".into());
    }
    if let Some(rest) = action.strip_prefix("convert:") {
        let parts: Vec<&str> = rest.split(':').collect();
        let to = parts.get(2).copied().unwrap_or("");
        let base = if to.contains("杀") { 60 } else { 45 };
        return (base, format!("转化使用【{to}】"));
    }
    if let Some(rest) = action.strip_prefix("skill:") {
        let code = rest.split(':').next().unwrap_or_default();
        let score = skill_value(g, seat, code);
        return (score, format!("发动技能 {code}"));
    }
    (20, "其它动作".into())
}

/// 技能基础分（按「收益倾向」的粗分类；不是语义白名单，仅影响优先级）。
fn skill_value(g: &Game, seat: usize, code: &str) -> i32 {
    // 摸牌/补牌/换牌类（净收益为正）
    const GOOD: &[&str] = &[
        "zhiheng", "kurou", "jizhi", "dimeng", "tuxi", "qixi", "guanxing", "luoshen", "qingguo",
        "yongsi", "jieyin", "fangquan", "remei", "jijiu", "lijian", "luanwu", "qimou", "shizhan",
        "zhiba", "lianhuan", "qiangxi", "tiaoxin", "chuli", "jianyan",
    ];
    // 明确「不主动用」的技能（会让局面变差 / 需要队友配合）。
    const AVOID: &[&str] = &["zili", "mingshi", "fengyin"];
    if AVOID.contains(&code) {
        return 8;
    }
    let base = if GOOD.contains(&code) { 58 } else { 30 };
    // 手牌少时不要乱发动消耗手牌的技能（除了摸牌类）。
    if g.hand(seat).is_empty() && !GOOD.contains(&code) {
        return 10;
    }
    base
}

// ---------------------------------------------------------------------------
// pending 打分
// ---------------------------------------------------------------------------

fn pending_choice(g: &Game, seat: usize, kind: &str, ids: &[&str], skill: Option<&str>) -> (i32, String) {
    let has = |p: &str| ids.iter().any(|id| *id == p || id.starts_with(p));
    let first = |p: &str| ids.iter().find(|id| id.starts_with(p)).map(|s| (*s).to_string());
    let pass = || ids.iter().any(|id| *id == "pass").then(|| "pass".to_string());

    let scored: (i32, String) = match kind {
        "dying" => {
            // 救自己 > 救别人（手牌多才救）。
            let victim = g.pending.as_ref().and_then(|p| p.data()["victim"].as_u64());
            let self_dying = victim.map(|v| v as usize) == Some(seat);
            if has("respond:") && (self_dying || g.hand(seat).len() >= 3) {
                return (95, first("respond:").unwrap_or_default());
            }
            (10, pass().unwrap_or_else(|| first("respond:").unwrap_or_default()))
        }
        "wuxie" => match pass() {
            Some(p) => (10, p),
            None => (-100, String::new()),
        },
        "card_response" => {
            let kind_ctx = g.pending.as_ref().and_then(|p| p.data()["kind"].as_str());
            let defensive = matches!(kind_ctx, Some("Shan") | None);
            if has("respond:") && (defensive || g.hand(seat).len() > 2) {
                let id = first("respond:").unwrap_or_default();
                (85, id)
            } else if let Some(p) = pass() {
                (10, p)
            } else {
                (50, first("respond:").unwrap_or_default())
            }
        }
        "pindian" => {
            // 拼点选点数最大的牌。
            let best = ids
                .iter()
                .filter(|id| id.starts_with("card:") || id.starts_with("choose:"))
                .filter_map(|id| {
                    let card = id.split(':').last().unwrap_or_default();
                    g.cards.get(card).map(|c| (c.rank, (*id).to_string()))
                })
                .max_by_key(|(r, _)| *r);
            match best {
                Some((_, id)) => (80, id),
                None => (10, pass().unwrap_or_default()),
            }
        }
        "discard_phase" => {
            // 弃价值最低的牌。
            let worst = ids
                .iter()
                .filter(|id| id.starts_with("discard:"))
                .min_by_key(|id| keep_value(g, id.trim_start_matches("discard:")))
                .map(|s| (*s).to_string());
            match worst {
                Some(id) => (90, id),
                None => (5, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        "choose_many" => {
            // 先选一张最不值钱的，再确认（两步收敛）。
            if has("confirm") {
                return (90, "confirm".into());
            }
            let worst = ids
                .iter()
                .filter(|id| id.starts_with("choose:"))
                .min_by_key(|id| keep_value(g, id.trim_start_matches("choose:")))
                .map(|s| (*s).to_string());
            match worst {
                Some(id) => (80, id),
                None => (10, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        "target" => {
            // 选最虚弱的目标。
            let best = ids
                .iter()
                .filter_map(|id| {
                    id.strip_prefix("target:")
                        .and_then(|t| t.parse::<usize>().ok())
                        .map(|t| (target_score(g, t), (*id).to_string()))
                })
                .max_by_key(|(s, _)| *s);
            match best {
                Some((_, id)) => (85, id),
                None => (10, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        "card_pick" | "card_target" | "harvest" => {
            let best = ids
                .iter()
                .filter(|id| {
                    id.starts_with("pick:")
                        || id.starts_with("choose:")
                        || id.starts_with("card:")
                })
                .max_by_key(|id| {
                    let card = id.split(':').last().unwrap_or_default();
                    keep_value(g, card)
                })
                .map(|s| (*s).to_string());
            match best {
                Some(id) => (80, id),
                None => (10, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        "fire_attack" => {
            let any = first("reveal:")
                .or_else(|| first("discard:"))
                .or_else(|| first("card:"))
                .or_else(|| first("choose:"))
                .or_else(|| first("pick:"));
            match any {
                Some(id) => (75, id),
                None => (10, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        "choice" | "choose" => {
            if has("confirm") {
                return (85, "confirm".into());
            }
            // 观星逐张：顶/底——留「桃/无懈/杀」在顶（其它置底）。
            if skill == Some("guanxing") {
                if let Some(top) = first("top") {
                    // 顶的牌对下家可能有利；保守：一半顶一半底会不稳定 → 全部置底（pass）更安全。
                    let _ = top;
                }
                if let Some(p) = pass() {
                    return (60, p);
                }
            }
            // 制霸（他人挑战）：手牌太少（≤1）时拒绝，别为了 1 张牌把拼点牌搭进去。
            if skill == Some("zhiba") {
                let hand = g.hand(seat).len();
                if hand <= 1 {
                    if let Some(p) = pass() {
                        return (60, p);
                    }
                }
            }
            // 二选一：作者顺序通常「收益优先」，取 opt1。
            if let Some(id) = first("opt1") {
                return (75, id);
            }
            if let Some(id) = first("opt2") {
                return (65, id);
            }
            if let Some(p) = pass() {
                return (40, p);
            }
            (50, String::new())
        }
        "discard" => {
            let worst = ids
                .iter()
                .filter(|id| id.starts_with("discard:"))
                .min_by_key(|id| keep_value(g, id.trim_start_matches("discard:")))
                .map(|s| (*s).to_string());
            match worst {
                Some(id) => (90, id),
                None => (10, pass().unwrap_or_else(|| "pass".into())),
            }
        }
        _ => {
            for pref in [
                "respond:",
                "pick:",
                "cardtarget:",
                "target:",
                "card:",
                "choose:",
                "discard:",
            ] {
                if let Some(id) = first(pref) {
                    return (70, id);
                }
            }
            if let Some(p) = pass() {
                return (10, p);
            }
            (50, String::new())
        }
    };
    // 兜底：策略给出空动作时取第一个可选项（绝不返回空 id）。
    if !scored.1.is_empty() || ids.is_empty() {
        scored
    } else {
        (scored.0, ids[0].to_string())
    }
}


// ---------------------------------------------------------------------------
// 决策入口
// ---------------------------------------------------------------------------

/// 决策：返回 `(action_id, priority, score, reason)`；`None` = 无可用动作。
pub fn decide_action(g: &Game, seat: usize, legal: &[Action]) -> Option<(String, i32, f64, String)> {
    if legal.is_empty() {
        return None;
    }
    if g.setup.is_some() || g.lobby.is_some() {
        // 大厅/选将：有 join/start 就先做；选将取第一个候选。
        let pick = legal
            .iter()
            .find(|a| a.id == "lobby:start")
            .or_else(|| legal.iter().find(|a| a.id.starts_with("setup:choose_general:")))
            .or_else(|| legal.iter().find(|a| a.id == "lobby:join"))
            .or_else(|| legal.iter().find(|a| a.id == "lobby:add_bot"))
            .or_else(|| legal.iter().find(|a| a.id == "lobby:abort"));
        if let Some(a) = pick {
            return Some((a.id.clone(), 90, 90.0, "大厅/选将".into()));
        }
        return legal
            .first()
            .map(|a| (a.id.clone(), 10, 10.0, "大厅兜底".into()));
    }
    if let Some(p) = &g.pending {
        let ids: Vec<&str> = legal.iter().map(|a| a.id.as_str()).collect();
        let (score, action) = pending_choice(g, seat, p.kind(), &ids, p.skill());
        let action = sanitize_action(legal, action);
        return Some((action, 80, score as f64, format!("pending:{}", p.kind())));
    }
    let mut best: Option<(i32, f64, String, String)> = None;
    for a in legal {
        let (score, reason) = score_play_action(g, seat, &a.id);
        let better = match &best {
            None => true,
            Some((s, _, _, _)) => score > *s,
        };
        if better {
            best = Some((score, score as f64, a.id.clone(), reason));
        }
    }
    best.map(|(p, s, id, reason)| (id, p, s, reason))
}

/// 决策落地前的**兜底校验**：返回的动作必须在 `legal` 内。
///
/// 为什么会漂移：`pending_choice` 的某些分支会直接给出 `pass`（历史写法），
/// 而强制待办（`escape_mandatory`，如火攻展示）的出口已被 `legal_actions` 隐藏 →
/// 决策日志出现「chosen 不在 legal 内」。这里统一收敛到第一个合法动作。
fn sanitize_action(legal: &[Action], action: String) -> String {
    if legal.iter().any(|a| a.id == action) {
        return action;
    }
    legal
        .first()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| action)
}

/// `EnginePlugin` 侧兼容：给 MCP/决策日志返回轨迹。
pub fn decide_traced(state: &GameState, actor: &str, legal: &[Action]) -> Option<DecisionTrace> {
    let g: Game = serde_json::from_value(state.data.clone()).ok()?;
    let seat = view::seat_of(&g, actor)?;
    let (action, priority, score, reason) = decide_action(&g, seat, legal)?;
    Some(DecisionTrace {
        action,
        legal: legal.to_vec(),
        candidates: legal.to_vec(),
        priority,
        score,
        reason,
        state_digest: state_digest(&g),
    })
}

// ---------------------------------------------------------------------------
// 机器人推进
// ---------------------------------------------------------------------------

/// 当前待行动者：pending.actor → 选将未选者 → 当前回合玩家。
pub fn current_actor(mgr: &GameManager, room: &str, g: &Game) -> Option<String> {
    if let Some(p) = &g.pending {
        return g.seats.get(p.actor()).map(|s| s.name.clone());
    }
    if g.setup.is_some() {
        return g
            .seats
            .iter()
            .map(|s| s.name.clone())
            .find(|pid| !mgr.legal_actions(room, pid).unwrap_or_default().is_empty());
    }
    g.seats.get(g.turn).map(|s| s.name.clone())
}

fn is_bot(g: &Game, actor: &str) -> bool {
    g.seats
        .iter()
        .find(|s| s.name == actor)
        .map(|s| s.bot)
        .unwrap_or(false)
}

/// 回环检测范围：只看**交互态**（pending）。
///
/// 出牌阶段「发动技能 → 放弃」会回到完全相同的非 pending 状态（属正常），
/// 若把非 pending 也纳入环检测，会把「技能试错」误判成环（2026-11-10 实测）。
/// 交互态里无懈/五谷天然多轮重复，也不参与。
fn loop_trackable(g: &Game) -> bool {
    match g.pending.as_ref().map(|p| p.kind()) {
        Some(kind) => !matches!(kind, "wuxie" | "harvest"),
        None => false,
    }
}

/// 一次 AI 决策（含破环信息）。
#[derive(Debug, Clone)]
pub struct AiChoice {
    pub action: String,
    pub priority: i32,
    pub score: f64,
    pub reason: String,
    pub state_digest: String,
    /// 是否由破环器改判。
    pub loop_breaker: bool,
}

/// 有状态的 AI（跨步记忆：本回合已试技能 / 环检测）。
///
/// `drive_bots` 与自对弈/示例共用同一份策略，避免两套「谁该行动 + 怎么选」漂移。
#[derive(Debug, Default, Clone)]
pub struct BotBrain {
    blocked: BTreeSet<String>,
    /// 本回合已发动过的主动技（每个 `skill:*` 动作每回合只试一次，防「发动→放弃/无进展」成环）。
    used_actions: BTreeSet<String>,
    last_skill: Option<(String, String)>,
    recent: VecDeque<String>,
    tried: BTreeMap<String, BTreeSet<String>>,
}

impl BotBrain {
    pub fn new() -> Self {
        Self::default()
    }

    /// 清空跨回合记忆（每个新房间/新局用新实例即可，也可显式调用）。
    pub fn reset(&mut self) {
        self.blocked.clear();
        self.used_actions.clear();
        self.last_skill = None;
        self.recent.clear();
        self.tried.clear();
    }

    /// 选一个动作；`None` = 判环中止（调用方记 `StopReason::Loop`）。
    pub fn choose(&mut self, g: &Game, seat: usize, legal: &[Action]) -> Option<AiChoice> {
        if legal.is_empty() {
            return None;
        }
        let turn_key = format!("{}:{}", g.turn_seq, g.turn);
        // 本回合已试过且无收益的技能动作不再重试。
        let filtered: Vec<Action> = legal
            .iter()
            .filter(|a| {
                a.id.starts_with("skill:")
                    && (self.blocked.contains(&format!("{turn_key}|{}", a.id))
                        || self.used_actions.contains(&format!("{turn_key}|{}", a.id)))
                    == false
            })
            .cloned()
            .collect();
        let effective: Vec<Action> = if filtered.is_empty() {
            legal
                .iter()
                .filter(|a| a.id == "end_play" || !a.id.starts_with("skill:"))
                .cloned()
                .collect()
        } else {
            filtered
        };
        let effective = if effective.is_empty() {
            legal.to_vec()
        } else {
            effective
        };

        let (mut action, priority, score, reason) = decide_action(g, seat, &effective)?;

        // 「技能→放弃」：把该技能在本回合屏蔽，防「发动→放弃」成环。
        if action == "pass" {
            if let Some((tk, sk)) = &self.last_skill {
                if *tk == turn_key && g.pending.as_ref().is_some_and(|p| p.skill().is_some()) {
                    self.blocked.insert(format!("{turn_key}|{sk}"));
                }
            }
        }
        if action.starts_with("skill:") {
            self.used_actions.insert(format!("{turn_key}|{action}"));
            self.last_skill = Some((turn_key.clone(), action.clone()));
        } else {
            self.last_skill = None;
        }

        // 环检测：同一交互状态重复出现 → 换未试过的动作。
        let digest = state_digest(g);
        let mut loop_breaker = false;
        if loop_trackable(g) {
            if self.recent.contains(&digest) {
                let seen = self.tried.entry(digest.clone()).or_default();
                match effective
                    .iter()
                    .find(|a| !seen.contains(&a.id) && a.id != action)
                {
                    Some(a) => {
                        seen.insert(a.id.clone());
                        action = a.id.clone();
                        loop_breaker = true;
                    }
                    None => {
                        // 所有选项都试过：优先「收口」（放弃/确认/结束），
                        // 真正的死循环交给上层的「同 (actor,action) 连续 8 次」兜底。
                        if let Some(id) = ["pass", "confirm", "end_play"]
                            .iter()
                            .find(|id| effective.iter().any(|a| a.id == **id))
                        {
                            action = (*id).to_string();
                        }
                    }
                }
            } else {
                self.recent.push_back(digest.clone());
                if self.recent.len() > 24 {
                    self.recent.pop_front();
                }
            }
        }
        Some(AiChoice {
            action,
            priority,
            score,
            reason: if loop_breaker {
                format!("loop_breaker:{reason}")
            } else {
                reason
            },
            state_digest: digest,
            loop_breaker,
        })
    }
}

/// 候选回落（引擎拒绝时）：`pass/confirm/end_play` → 其它任意未试动作。
pub fn fallback_action(legal: &[Action], failed: &str) -> Option<String> {
    for id in ["pass", "confirm", "end_play"] {
        if legal.iter().any(|a| a.id == id) {
            return Some(id.to_string());
        }
    }
    legal
        .iter()
        .map(|a| a.id.clone())
        .find(|id| id != failed)
}

/// 回合外主动技（矫诏 plus/plusplus）：找一台「不是当前回合、但回合外可发动」的机器人。
///
/// 口径（与 v1 一致）：回合外主动技**不占用**当前玩家的出牌权 —— 不压 pending，
/// 只是在推进循环里「顺路问一次」；人类玩家的窗口由 Web 端 `legal_actions` 直接提供。
fn out_of_turn_bot(mgr: &GameManager, room: &str, g: &Game) -> Option<(String, Vec<Action>)> {
    if g.pending.is_some() {
        return None;
    }
    for (i, s) in g.seats.iter().enumerate() {
        if !s.bot || i == g.turn || s.hp <= 0 {
            continue;
        }
        let Ok(legal) = mgr.legal_actions(room, &s.name) else {
            continue;
        };
        if legal.iter().any(|a| a.id.starts_with("skill:")) {
            return Some((s.name.clone(), legal));
        }
    }
    None
}

/// 推进机器人回合，直到人类回合 / 终局 / 上限。
///
/// `persist` 由调用方提供（server 传 `game_events` + 快照落库；测试传空实现）。
pub fn drive_bots(
    mgr: &GameManager,
    room: &str,
    max_steps: usize,
    mut persist: impl FnMut(&GameState, &[EngineEvent]) -> i64,
) -> DriveReport {
    let mut report = DriveReport::default();
    let mut brain = BotBrain::new();
    let mut repeat_key = String::new();
    let mut repeat_count = 0usize;

    for _ in 0..max_steps {
        let Some(state) = mgr.get_state(room) else {
            return report
                .stop(StopReason::NoActor)
                .with_error("状态丢失（房间不在内存且无快照）");
        };
        // 引擎 id：v2 接管前允许 `sanguosha2`；迁移后为 `sanguosha`。
        if state.game_id != "sanguosha2" && state.game_id != "sanguosha" {
            return report.stop(StopReason::NotSanguosha);
        }
        if state.status == GameStatus::Finished {
            report.finished = true;
            return report.stop(StopReason::Finished);
        }
        let Ok(g) = serde_json::from_value::<Game>(state.data.clone()) else {
            return report
                .stop(StopReason::EngineError)
                .with_error("状态解析失败");
        };
        let Some(actor) = current_actor(mgr, room, &g) else {
            return report.stop(StopReason::NoActor);
        };
        // 人类回合：先把「回合外主动技」的机器人窗口顺路问掉，再停下。
        let (actor, legal) = if !is_bot(&g, &actor) {
            match out_of_turn_bot(mgr, room, &g) {
                Some((pid, legal)) => (pid, legal),
                None => return report.stop(StopReason::HumanTurn),
            }
        } else {
            match mgr.legal_actions(room, &actor) {
                Ok(l) => (actor, l),
                Err(e) => {
                    return report
                        .stop(StopReason::EngineError)
                        .with_error(format!("legal_actions 失败: {e}"));
                }
            }
        };
        if legal.is_empty() {
            return report
                .stop(StopReason::NoLegalActions)
                .with_error(format!("机器人 {actor} 无合法动作（phase={:?}）", g.phase));
        }
        let Some(seat) = view::seat_of(&g, &actor) else {
            return report
                .stop(StopReason::EngineError)
                .with_error(format!("{actor} 不在座"));
        };

        let t0 = Instant::now();
        let Some(choice) = brain.choose(&g, seat, &legal) else {
            return report
                .stop(StopReason::Loop)
                .with_error(format!("状态回环且无可试动作（{actor}）"));
        };
        if choice.loop_breaker {
            report.loops += 1;
        }

        // 同一 (actor, action) 连续 8 次 → 判环。 
        let key = format!("{actor}|{}", choice.action);
        if key == repeat_key {
            repeat_count += 1;
            if repeat_count >= 8 {
                return report
                    .stop(StopReason::Loop)
                    .with_error(format!("{key} 连续重复 {repeat_count} 次"));
            }
        } else {
            repeat_key = key;
            repeat_count = 1;
        }

        match mgr.execute_action(room, &actor, &choice.action) {
            Ok(res) => {
                let seq = persist(&res.state, &res.events);
                report.steps += 1;
                report.decisions.push(BotDecision {
                    actor,
                    seq,
                    legal: legal.clone(),
                    candidates: legal.clone(),
                    chosen: choice.action,
                    priority: choice.priority,
                    score: choice.score,
                    reason: choice.reason,
                    state_digest: choice.state_digest,
                    latency_ms: t0.elapsed().as_millis() as i64,
                });
                if res.finished {
                    report.finished = true;
                    return report.stop(StopReason::Finished);
                }
            }
            Err(e) => {
                report.illegal += 1;
                let engine_error = format!("{e}");
                report
                    .errors
                    .push(format!("{} 被拒绝：{engine_error}", choice.action));
                let Some(fb) = fallback_action(&legal, &choice.action) else {
                    return report
                        .stop(StopReason::EngineError)
                        .with_error(format!("{} 被拒绝：{engine_error}", choice.action));
                };
                match mgr.execute_action(room, &actor, &fb) {
                    Ok(res) => {
                        let seq = persist(&res.state, &res.events);
                        report.steps += 1;
                        report.decisions.push(BotDecision {
                            actor,
                            seq,
                            legal: legal.clone(),
                            candidates: legal.clone(),
                            chosen: fb,
                            priority: choice.priority,
                            score: choice.score,
                            reason: format!("fallback:{}", choice.reason),
                            state_digest: choice.state_digest,
                            latency_ms: t0.elapsed().as_millis() as i64,
                        });
                        if res.finished {
                            report.finished = true;
                            return report.stop(StopReason::Finished);
                        }
                    }
                    Err(e2) => {
                        return report.stop(StopReason::EngineError).with_error(format!(
                            "{} 被拒绝（{engine_error}）；回落 {fb} 也失败：{e2}",
                            choice.action
                        ));
                    }
                }
            }
        }
    }
    report.stop(StopReason::MaxSteps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine_core::{EngineRegistry, SeededRng};

    fn manager(seed: u64) -> GameManager {
        let mut reg = EngineRegistry::new();
        reg.register(crate::Sanguosha2Engine);
        GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
    }

    fn human_plus_bots(mgr: &GameManager) -> String {
        let (st, _) = mgr.create_game("sanguosha", "drive-group", "u1").unwrap();
        let room = st.room_id.clone();
        for _ in 0..4 {
            mgr.execute_action(&room, "u1", "lobby:add_bot").unwrap();
        }
        mgr.execute_action(&room, "u1", "lobby:start").unwrap();
        let acts = mgr.legal_actions(&room, "u1").unwrap_or_default();
        if let Some(a) = acts
            .iter()
            .find(|a| a.id.starts_with("setup:choose_general:"))
        {
            mgr.execute_action(&room, "u1", &a.id).unwrap();
        }
        room
    }

    #[test]
    fn drive_stops_at_human_turn_and_records_decisions() {
        let mgr = manager(9001);
        let room = human_plus_bots(&mgr);
        let report = drive_bots(&mgr, &room, MAX_STEPS, |_, _| 0);
        assert!(
            report.advanced() || report.finished,
            "应推进机器人：{report:?}"
        );
        assert!(
            matches!(
                report.stop,
                Some(StopReason::HumanTurn) | Some(StopReason::Finished)
            ),
            "{report:?}"
        );
        assert_eq!(report.illegal, 0, "不应出现非法动作：{:?}", report.errors);
        assert!(!report.decisions.is_empty());
        for d in &report.decisions {
            assert!(!d.state_digest.is_empty());
            assert!(d.legal.iter().any(|a| a.id == d.chosen));
        }
    }

    #[test]
    fn drive_is_noop_for_unknown_room() {
        let mgr = manager(9002);
        let report = drive_bots(&mgr, "no-such-room", MAX_STEPS, |_, _| 0);
        assert_eq!(report.steps, 0);
        assert_eq!(report.stop, Some(StopReason::NoActor));
    }

    #[test]
    fn drive_can_finish_game_with_human_passing() {
        let mgr = manager(9003);
        let room = human_plus_bots(&mgr);
        for _ in 0..4000 {
            let report = drive_bots(&mgr, &room, MAX_STEPS, |_, _| 0);
            assert_eq!(report.illegal, 0, "非法：{report:?} {:?}", report.errors);
            if report.finished {
                return;
            }
            assert_eq!(report.stop, Some(StopReason::HumanTurn), "{report:?}");
            let state = mgr.get_state(&room).expect("state");
            let g: Game = serde_json::from_value(state.data).expect("parse");
            let actor = current_actor(&mgr, &room, &g).expect("actor");
            let legal = mgr.legal_actions(&room, &actor).unwrap_or_default();
            let pick = ["end_play", "pass", "confirm"]
                .iter()
                .find(|id| legal.iter().any(|a| a.id == **id))
                .map(|id| (*id).to_string())
                .unwrap_or_else(|| legal[0].id.clone());
            mgr.execute_action(&room, &actor, &pick)
                .expect("human action");
        }
        let st = mgr.get_state(&room).expect("state");
        let g: Game = serde_json::from_value(st.data).expect("parse");
        panic!(
            "未终局：round={} turn={} phase={:?} pending={:?} finished={} hp={:?}",
            g.round,
            g.turn,
            g.phase,
            g.pending.as_ref().map(|p| (p.kind().to_string(), p.actor())),
            g.finished,
            g.seats.iter().map(|s| s.hp).collect::<Vec<_>>()
        );
    }

    #[test]
    fn ai_prefers_lethal_sha_and_tao_when_hurt() {
        let engine = crate::Sanguosha2Engine;
        let mut rng = SeededRng::new(3);
        let (state, _) = engine.new_started_game(
            "r",
            "g",
            vec!["p0".into(), "p1".into(), "p2".into(), "p3".into(), "p4".into()],
            &mut rng,
        );
        let mut g: Game = serde_json::from_value(state.data).unwrap();
        // 构造：p0 出牌阶段、手牌有杀与桃、hp 受伤。
        g.turn = 0;
        g.phase = Phase::Play;
        g.seats[0].hp = g.seats[0].max_hp - 1;
        let sang = g
            .ids_in(|z| *z == Zone::Draw)
            .into_iter()
            .find(|id| g.def_of(id).tags.sha)
            .unwrap();
        g.to_hand(&sang, 0).unwrap();
        let tao = g
            .ids_in(|z| *z == Zone::Draw)
            .into_iter()
            .find(|id| g.def_of(id).tags.tao)
            .unwrap();
        g.to_hand(&tao, 0).unwrap();
        let legal: Vec<Action> = crate::kernel::card_play::legal_uses(&g, 0)
            .into_iter()
            .map(|(id, description)| Action { id, description })
            .chain(std::iter::once(Action {
                id: "end_play".into(),
                description: "结束".into(),
            }))
            .collect();
        let (action, _, _, reason) = decide_action(&g, 0, &legal).expect("应有决策");
        let played = action.split(':').nth(1).unwrap_or_default().to_string();
        assert!(
            g.def_of(&played).tags.tao || reason.contains("桃"),
            "受伤时应优先用桃：{action} {reason}"
        );
    }
}
