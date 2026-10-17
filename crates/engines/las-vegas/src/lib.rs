//! 拉斯维加斯（Las Vegas Royale）桌游引擎。
//!
//! 实现 `game-engine-core::EnginePlugin`。规则见 `docs/games/las-vegas/`（定稿 v1.0）。
//!
//! 本期范围（阶段 0–2）：lobby / setup / turn / 结算 / 三轮 / 终局；
//! 小游戏板块按 16→8→N 抽取并挂载到 1–3 号赌场，但**效果占位**（阶段 3 实现）。

use game_engine_core::{
    roll_die, shuffle, Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState,
    GameStatus, RandomSource,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub mod ai;
pub mod sg;
use sg::SgState;

/// 私有结构 schema 版本（v1：主流程；v2：小游戏；v3：大骰位置追踪；v4：F2 重做；
/// v5：E2 酒吧改为按大小骰分别记录；v6：H2 金框改为 `GoldFrame`）。
pub const SCHEMA_VERSION: u32 = 6;
/// 轮数。
pub const ROUNDS: u32 = 3;
/// 每人小骰 / 大骰。
pub const SMALL_DICE: u8 = 7;
pub const BIG_DICE: u8 = 1;
/// 初始筹码。
pub const INITIAL_CHIPS: u32 = 2;
/// 每人每轮（第 2 轮起）追加筹码。
pub const CHIPS_PER_ROUND: u32 = 2;
/// 赌场数。
pub const CASINOS: usize = 6;
/// 玩家人数上限。
pub const MAX_PLAYERS: usize = 6;
/// 小游戏 N 默认值与范围。
pub const DEFAULT_SMALL_GAMES: u32 = 3;
pub const MIN_SMALL_GAMES: u32 = 3;
pub const MAX_SMALL_GAMES: u32 = 6;
/// 筹码计分单位（元）。
pub const CHIP_VALUE: i64 = 10_000;
/// 钞票面额（元）：$3 万–$10 万（豪华版/Royale 口径，见攻略）。
pub const DENOMINATIONS: [u32; 8] = [
    30_000, 40_000, 50_000, 60_000, 70_000, 80_000, 90_000, 100_000,
];
/// 各面额张数（合计 90 张）。
pub const DENOMINATION_COPIES: [usize; 8] = [11, 11, 13, 15, 13, 11, 9, 7];

/// 引擎私有阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Lobby,
    Setup,
    TurnRoll,
    SmallGame,
    RoundEnd,
    Settling,
    Finished,
    Aborted,
}

impl Phase {
    fn is_over(self) -> bool {
        matches!(self, Phase::Finished | Phase::Aborted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Roll {
    #[serde(default)]
    pub small: Vec<u8>,
    #[serde(default)]
    pub big: Option<u8>,
}

impl Roll {
    fn points(&self) -> Vec<u8> {
        let mut v = self.small.clone();
        if let Some(b) = self.big {
            v.push(b);
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub player_id: String,
    pub small_remaining: u8,
    pub big_remaining: u8,
    pub chips: u32,
    pub banknotes: Vec<u32>,
    /// 是否为机器人（测试补位；由内置启发式 AI 自动行动）。
    #[serde(default)]
    pub is_bot: bool,
}

impl Seat {
    fn new(player_id: String) -> Self {
        Self {
            player_id,
            small_remaining: SMALL_DICE,
            big_remaining: BIG_DICE,
            chips: INITIAL_CHIPS,
            banknotes: Vec::new(),
            is_bot: false,
        }
    }

    fn new_bot(player_id: String) -> Self {
        let mut s = Self::new(player_id);
        s.is_bot = true;
        s
    }

    fn has_dice(&self) -> bool {
        self.small_remaining > 0 || self.big_remaining > 0
    }

    fn total(&self) -> i64 {
        self.banknotes.iter().map(|n| *n as i64).sum::<i64>() + self.chips as i64 * CHIP_VALUE
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Casino {
    /// 各玩家（含灰色骰虚拟玩家 `"gray"`）在此的等效骰数（大骰权重 2）。
    #[serde(default)]
    pub dice: BTreeMap<String, u32>,
    /// 各玩家在此的**大骰**数量（每颗已在 `dice` 中计入 2 点权重）。
    /// 仅用于需要区分大小骰的小游戏（H1 取骰时不拆大骰）。
    #[serde(default)]
    pub big_dice: BTreeMap<String, u32>,
    /// 2 张钞票面额；结算前公共视图脱敏为 `0`（未知）。
    #[serde(default)]
    pub banknotes: Vec<u32>,
    /// 挂载的小游戏（仅 1–3 号；N>3 时可多块）。
    #[serde(default)]
    pub small_games: Vec<String>,
}

impl Casino {
    fn empty() -> Self {
        Self {
            dice: BTreeMap::new(),
            big_dice: BTreeMap::new(),
            banknotes: Vec::new(),
            small_games: Vec::new(),
        }
    }
}

/// 引擎私有状态（序列化进 `GameState.data`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LasVegasState {
    pub schema_version: u32,
    pub phase: Phase,
    pub host: String,
    pub round: u32,
    pub start_player: Option<String>,
    /// 当前回合玩家在 `seats` 的下标。
    pub turn: usize,
    pub seats: Vec<Seat>,
    #[serde(default)]
    pub roll: Roll,
    pub casinos: Vec<Casino>,
    pub small_game_count: u32,
    /// 小游戏子状态（阶段 3a 起；schema v2）。
    #[serde(default)]
    pub sg: SgState,
    #[serde(default)]
    pub scores: BTreeMap<String, i64>,
    #[serde(default)]
    pub winners: Vec<String>,
}

pub struct LasVegasEngine;

impl LasVegasEngine {
    fn parse(data: &serde_json::Value) -> Result<LasVegasState, EngineError> {
        serde_json::from_value(data.clone())
            .map_err(|e| EngineError::Internal(format!("状态解析失败: {e}")))
    }

    fn build_state(old: &GameState, st: &LasVegasState) -> GameState {
        let status = if st.phase.is_over() {
            GameStatus::Finished
        } else {
            GameStatus::Active
        };
        GameState {
            game_id: old.game_id.clone(),
            room_id: old.room_id.clone(),
            group_id: old.group_id.clone(),
            status,
            players: st.seats.iter().map(|s| s.player_id.clone()).collect(),
            data: serde_json::to_value(st).unwrap_or(json!({})),
        }
    }

    /// 每轮设置：重置骰子、追加筹码（第 2 轮起）、铺钞票、抽小游戏。
    fn setup_round(&self, st: &mut LasVegasState, round: u32, rng: &mut dyn RandomSource) -> serde_json::Value {
        st.round = round;
        st.phase = Phase::Setup;
        for seat in &mut st.seats {
            seat.small_remaining = SMALL_DICE;
            seat.big_remaining = BIG_DICE;
            if round >= 2 {
                seat.chips += CHIPS_PER_ROUND;
            }
        }
        for c in &mut st.casinos {
            c.dice.clear();
            c.big_dice.clear();
            c.banknotes.clear();
            c.small_games.clear();
        }

        // 钞票：全部 90 张重洗 → 抽 6 组 × 2 张 → 按组总额降序铺 6→1 号赌场。
        let mut deck = banknote_deck();
        shuffle(rng, &mut deck);
        let mut pairs: Vec<Vec<u32>> = deck.chunks(2).take(CASINOS).map(|c| c.to_vec()).collect();
        pairs.sort_by(|a, b| {
            let sa: u32 = a.iter().sum();
            let sb: u32 = b.iter().sum();
            sb.cmp(&sa)
                .then_with(|| b.iter().max().cmp(&a.iter().max()))
        });
        for (rank, pair) in pairs.iter().enumerate() {
            st.casinos[CASINOS - 1 - rank].banknotes = pair.clone();
        }

        // 小游戏：第 1 轮可用 lobby:set_small_games 覆盖；之后每轮重新 16→8→N。
        let tiles = if round == 1 {
            match &st.sg.small_games_override {
                Some(list) if !list.is_empty() => list.clone(),
                _ => draw_small_games(st.small_game_count, rng),
            }
        } else {
            draw_small_games(st.small_game_count, rng)
        };
        for (i, t) in tiles.iter().enumerate() {
            st.casinos[i % 3].small_games.push(t.clone());
        }
        // 小游戏设置阶段：灰骰/摸彩/指示物按抽中的小游戏重摆。
        st.sg.setup_round(&tiles, &mut st.casinos);

        json!({
            "round": round,
            "banknotes": st.casinos.iter().map(|c| c.banknotes.clone()).collect::<Vec<_>>(),
            "small_games": tiles,
        })
    }

    /// 回合开始：为当前玩家掷出手中全部骰子。
    ///
    /// 边界：若掷出的点数**全部**被 E1 禁止且玩家无筹码可重试，则免费重掷（否则无法行动）。
    fn begin_turn(&self, st: &mut LasVegasState, rng: &mut dyn RandomSource) -> Vec<EngineEvent> {
        st.phase = Phase::TurnRoll;
        st.sg.h1_used_turn = false; // H1 每回合限 1 颗
        let mut events = Vec::new();
        // D2 权力优势（用户裁定 2026-10-17）：持有特权标记的玩家**不掷骰**，
        // 改为手动选 1 颗手中骰子放到自选赌场（放置后会正常触发该赌场小游戏）。
        if let Some(seat) = st.seats.get(st.turn) {
            if st.sg.privilege.as_deref() == Some(seat.player_id.as_str()) {
                let player_id = seat.player_id.clone();
                st.roll = Roll::default();
                events.push(EngineEvent::new(
                    "small_game",
                    json!({ "game": "D2", "player": player_id, "skip_roll": true }),
                ));
                return events;
            }
        }
        for _ in 0..100 {
            let Some(seat) = st.seats.get(st.turn) else {
                return events;
            };
            let (small_remaining, big_remaining, player_id, chips) = (
                seat.small_remaining,
                seat.big_remaining,
                seat.player_id.clone(),
                seat.chips,
            );
            let small: Vec<u8> = (0..small_remaining).map(|_| roll_die(rng)).collect();
            let big = if big_remaining > 0 {
                Some(roll_die(rng))
            } else {
                None
            };
            st.roll = Roll {
                small: small.clone(),
                big,
            };
            events.push(EngineEvent::new(
                "dice_rolled",
                json!({ "player_id": player_id, "small": small, "big": big }),
            ));
            let banned = st.sg.banned;
            let can_place = st
                .roll
                .points()
                .iter()
                .any(|p| banned != Some((*p - 1) as usize));
            if can_place || chips >= 1 {
                break;
            }
            // 全部点数被禁止且无筹码 → 免费重掷，避免死锁
            events.push(EngineEvent::new(
                "dice_rerolled_free",
                json!({ "player_id": player_id }),
            ));
        }
        events
    }

    /// 动作后推进到下一位仍有骰子的玩家；若全部用尽返回 `true`（进入结算）。
    fn advance_turn(&self, st: &mut LasVegasState, rng: &mut dyn RandomSource) -> (Vec<EngineEvent>, bool) {
        let n = st.seats.len();
        if n == 0 {
            return (vec![], true);
        }
        for k in 1..=n {
            let idx = (st.turn + k) % n;
            if st.seats[idx].has_dice() {
                st.turn = idx;
                return (self.begin_turn(st, rng), false);
            }
        }
        (vec![], true)
    }

    /// 本轮结算（1→6 号赌场），随后进入下一轮或终局。
    fn settle_round(&self, st: &mut LasVegasState, rng: &mut dyn RandomSource) -> (Vec<EngineEvent>, bool) {
        st.phase = Phase::Settling;
        let mut events = Vec::new();
        // 结算阶段第一步（用户裁定 2026-10-17）：按赌场顺序 1→6，先跑「结算阶段进入」的小游戏
        // （B1 加骰 / C2 罚金），全部跑完才正式逐场结算。
        if sg::pre_settle_pass(st, rng, &mut events) {
            st.phase = Phase::SmallGame;
            return (events, false);
        }

        let mut logs = Vec::new();
        let mut top_winners: Vec<Option<String>> = vec![None; CASINOS];
        for ci in 0..CASINOS {
            // C2 厄运降临：移除平手前，最少骰子者（含 0）付 $5 万。
            sg::pre_settle_casino(st, ci, &mut events);
            let (log, top) = settle_casino(&mut st.casinos[ci], &mut st.seats);
            top_winners[ci] = top;
            logs.push(json!({ "casino": ci + 1, "allocations": log }));
        }
        // C1 掌声响起：大奖标记兑换 $10 万钞票。
        sg::post_settle(st, &mut events);
        // G2 游戏场 / H2 金框 / H1 格位。
        sg::settle_extras(st, &mut events);
        let settled_round = st.round;
        events.push(EngineEvent::new(
            "round_settled",
            json!({ "round": settled_round, "casinos": logs }),
        ));
        for c in &mut st.casinos {
            c.banknotes.clear();
        }

        if st.round < ROUNDS {
            let next = st.round + 1;
            let setup = self.setup_round(st, next, rng);
            let mut start = st.start_player.clone().unwrap_or_else(|| st.host.clone());
            for ci in (0..CASINOS).rev() {
                if let Some(Some(p)) = top_winners.get(ci) {
                    start = p.clone();
                    break;
                }
            }
            st.start_player = Some(start.clone());
            st.turn = st
                .seats
                .iter()
                .position(|s| s.player_id == start)
                .unwrap_or(0);
            events.push(EngineEvent::new(
                "round_setup",
                json!({ "round": next, "start_player": start, "setup": setup }),
            ));
            let mut rolled = self.begin_turn(st, rng);
            st.phase = Phase::TurnRoll;
            events.append(&mut rolled);
            (events, false)
        } else {
            let mut scored: Vec<(String, i64, usize, u32)> = st
                .seats
                .iter()
                .map(|s| (s.player_id.clone(), s.total(), s.banknotes.len(), s.chips))
                .collect();
            scored.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| b.2.cmp(&a.2))
                    .then_with(|| b.3.cmp(&a.3))
            });
            st.scores = scored.iter().map(|x| (x.0.clone(), x.1)).collect();
            st.winners = match scored.first() {
                Some(first) => scored
                    .iter()
                    .filter(|x| x.1 == first.1 && x.2 == first.2 && x.3 == first.3)
                    .map(|x| x.0.clone())
                    .collect(),
                None => Vec::new(),
            };
            st.phase = Phase::Finished;
            events.push(EngineEvent::new(
                "game_settled",
                json!({
                    "scores": st.scores,
                    "ranking": scored.iter().map(|x| json!({
                        "player_id": x.0, "total": x.1, "banknotes": x.2, "chips": x.3
                    })).collect::<Vec<_>>(),
                    "winners": st.winners,
                }),
            ));
            (events, true)
        }
    }

    /// 应用一次下注后推进回合；返回事件与是否终局。
    fn after_action(&self, st: &mut LasVegasState, rng: &mut dyn RandomSource) -> (Vec<EngineEvent>, bool) {
        let (mut events, round_end) = self.advance_turn(st, rng);
        if round_end {
            let (mut settle_events, finished) = self.settle_round(st, rng);
            events.append(&mut settle_events);
            (events, finished)
        } else {
            (events, false)
        }
    }
}

/// 公共视图脱敏：钞票面额、G1 摸彩内容、A1 的选项↔枚数映射。
fn mask_public(st: &mut LasVegasState) {
    for c in &mut st.casinos {
        c.banknotes = vec![0u32; c.banknotes.len()];
    }
    st.sg.lottery = vec![0u32; st.sg.lottery.len()];
    if let Some(p) = st.sg.pending.as_mut() {
        if p.game == "A1" {
            if let Some(o) = p.data.as_object_mut() {
                o.remove("perm");
                o.remove("count");
                o.remove("n");
            }
        }
    }
}

impl EnginePlugin for LasVegasEngine {
    fn game_id(&self) -> &'static str {
        "las-vegas"
    }

    fn rules(&self) -> String {
        "拉斯维加斯（Las Vegas Royale）：2–6 人，3 轮。每人 7 小骰 + 1 大骰；\
         轮到你时掷出手中全部骰子，选一种点数把该点数的骰子全部放到对应赌场（大骰在赌场算 2 个小骰）。\
         可支付 1 枚筹码弃本回合。每人每轮开始有 / 追加 2 枚筹码。\
         一轮结束从 1 号赌场起结算：先剔除骰数并列的玩家，剩余骰数最多者拿该赌场面额最高的钞票、\
         第二多者拿次高的。3 轮后按「钞票总额 + 筹码数 × $1 万」计分，平局依次比钞票张数、筹码数。"
            .to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        _rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        // create_game 保证 players = [host]
        let host = players
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::Lobby,
            host: host.clone(),
            round: 1,
            start_player: None,
            turn: 0,
            seats: players.iter().cloned().map(Seat::new).collect(),
            roll: Roll::default(),
            casinos: (0..CASINOS).map(|_| Casino::empty()).collect(),
            small_game_count: DEFAULT_SMALL_GAMES,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let state = GameState {
            game_id: self.game_id().to_string(),
            room_id: room_id.to_string(),
            group_id: group_id.to_string(),
            status: GameStatus::Active,
            players: players.clone(),
            data: serde_json::to_value(&st).unwrap_or(json!({})),
        };
        let events = vec![EngineEvent::new(
            "game_created",
            json!({ "host": host, "group_id": group_id }),
        )];
        (state, events)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let st = match Self::parse(&state.data) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        match st.phase {
            Phase::Lobby => {
                let in_seats = st.seats.iter().any(|s| s.player_id == player_id);
                let mut out = Vec::new();
                if !in_seats {
                    if st.seats.len() < MAX_PLAYERS {
                        out.push(Action {
                            id: "lobby:join".into(),
                            description: "加入游戏".into(),
                        });
                    }
                } else {
                    out.push(Action {
                        id: "lobby:leave".into(),
                        description: "退出游戏设置".into(),
                    });
                    if player_id == st.host {
                        if (2..=MAX_PLAYERS).contains(&st.seats.len()) {
                            out.push(Action {
                                id: "lobby:start".into(),
                                description: "开始游戏".into(),
                            });
                        }
                        if st.seats.len() < MAX_PLAYERS {
                            out.push(Action {
                                id: "lobby:add_bot".into(),
                                description: "加入机器人".into(),
                            });
                        }
                        out.push(Action {
                            id: "lobby:abort".into(),
                            description: "中断本局（不结算）".into(),
                        });
                        for s in &st.seats {
                            if s.player_id != st.host {
                                out.push(Action {
                                    id: format!("lobby:kick:{}", s.player_id),
                                    description: format!("踢出 {}", s.player_id),
                                });
                            }
                        }
                        for n in MIN_SMALL_GAMES..=MAX_SMALL_GAMES {
                            if n != st.small_game_count {
                                out.push(Action {
                                    id: format!("lobby:set_small_game_count:{n}"),
                                    description: format!("设置小游戏数量为 {n}"),
                                });
                            }
                        }
                    }
                }
                out
            }
            Phase::TurnRoll => {
                let Some(cur) = st.seats.get(st.turn) else {
                    return Vec::new();
                };
                if cur.player_id != player_id {
                    return Vec::new();
                }
                let banned = st.sg.banned;
                let mut out: Vec<Action> = st
                    .roll
                    .points()
                    .into_iter()
                    .filter(|n| banned != Some((*n - 1) as usize))
                    .map(|n| Action {
                        id: format!("place:{n}"),
                        description: format!("把本轮所有 {n} 点骰子放到 {n} 号赌场"),
                    })
                    .collect();
                // D2 特权回合没有投掷结果 → 不提供「弃回合」。
                if cur.chips >= 1 && !st.roll.points().is_empty() {
                    out.push(Action {
                        id: "reroll".into(),
                        description: "支付 1 枚筹码，弃本回合".into(),
                    });
                }
                // 回合内小游戏动作（D2 特权 / G2 移游戏场 / H1 移格）
                out.extend(sg::legal_turn_sg(&st, player_id));
                out
            }
            // 小游戏交互：仅 pending.actor 可动
            Phase::SmallGame => sg::legal_for_pending(&st, player_id),
            _ => Vec::new(),
        }
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        let mut st = Self::parse(&state.data)?;
        if st.phase.is_over() {
            return Err(EngineError::IllegalAction("对局已结束".into()));
        }

        // 恢复原语：不改状态，只重新生成「当前该谁做什么」的提示（配合 web 加密选项/超时重发）。
        if action == "resume" || action == "hint" {
            return Ok(ActionResult {
                state: Self::build_state(state, &st),
                message: sg::resume_message(&st),
                finished: false,
                events: Vec::new(),
            });
        }

        let mut events: Vec<EngineEvent> = Vec::new();
        let message: String;
        let mut finished = false;

        if let Some(rest) = action.strip_prefix("lobby:") {
            // 房主可在**任意 Active 阶段**中断（规则 §8.6）；其余大厅动作仅限 lobby。
            if st.phase != Phase::Lobby && rest != "abort" {
                return Err(EngineError::IllegalAction("当前阶段不可执行大厅动作".into()));
            }
            match rest {
                "join" => {
                    if st.seats.iter().any(|s| s.player_id == player_id) {
                        return Err(EngineError::IllegalAction("你已在本局中".into()));
                    }
                    if st.seats.len() >= MAX_PLAYERS {
                        return Err(EngineError::IllegalAction("人数已满".into()));
                    }
                    st.seats.push(Seat::new(player_id.to_string()));
                    events.push(EngineEvent::new(
                        "player_joined",
                        json!({ "player_id": player_id }),
                    ));
                    message = format!("{player_id} 加入了游戏");
                }
                "add_bot" => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可加机器人".into()));
                    }
                    if st.seats.len() >= MAX_PLAYERS {
                        return Err(EngineError::IllegalAction("人数已满".into()));
                    }
                    let n = st.seats.iter().filter(|s| s.is_bot).count() + 1;
                    let id = format!("bot{n}");
                    st.seats.push(Seat::new_bot(id.clone()));
                    events.push(EngineEvent::new(
                        "bot_added",
                        json!({ "player_id": id }),
                    ));
                    message = format!("机器人 {id} 已加入");
                }
                "leave" => {
                    if !st.seats.iter().any(|s| s.player_id == player_id) {
                        return Err(EngineError::PermissionDenied("你不在本局中".into()));
                    }
                    if player_id == st.host {
                        st.phase = Phase::Aborted;
                        finished = true;
                        events.push(EngineEvent::new(
                            "game_aborted",
                            json!({ "host": player_id, "reason": "host_left" }),
                        ));
                        message = "发起人退出，本局废弃".to_string();
                    } else {
                        st.seats.retain(|s| s.player_id != player_id);
                        events.push(EngineEvent::new(
                            "player_left",
                            json!({ "player_id": player_id }),
                        ));
                        message = format!("{player_id} 退出了游戏");
                    }
                }
                _ if rest.starts_with("kick:") => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可踢人".into()));
                    }
                    let target = rest.trim_start_matches("kick:").to_string();
                    if target == st.host || !st.seats.iter().any(|s| s.player_id == target) {
                        return Err(EngineError::IllegalAction("目标不在本局中".into()));
                    }
                    st.seats.retain(|s| s.player_id != target);
                    events.push(EngineEvent::new(
                        "player_kicked",
                        json!({ "host": player_id, "target": target }),
                    ));
                    message = format!("已踢出 {target}");
                }
                _ if rest.starts_with("set_small_games:") => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可设置".into()));
                    }
                    let csv = rest.trim_start_matches("set_small_games:");
                    let ids: Vec<String> = csv
                        .split(|c| c == ',' || c == '，' || c == '/' || c == ' ')
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_ascii_uppercase())
                        .collect();
                    if !(MIN_SMALL_GAMES as usize..=MAX_SMALL_GAMES as usize).contains(&ids.len()) {
                        return Err(EngineError::IllegalAction("小游戏数量须 3–6".into()));
                    }
                    let mut letters = std::collections::BTreeSet::new();
                    for id in &ids {
                        if sg::rule_text(id).is_none() {
                            return Err(EngineError::IllegalAction(format!("未知小游戏 {id}")));
                        }
                        let letter = id.chars().next().unwrap_or('?');
                        if !letters.insert(letter) {
                            return Err(EngineError::IllegalAction(format!(
                                "同一字母组只能选 1 个：{id}"
                            )));
                        }
                    }
                    st.small_game_count = ids.len() as u32;
                    st.sg.small_games_override = Some(ids.clone());
                    events.push(EngineEvent::new(
                        "small_games_set",
                        json!({ "tiles": ids }),
                    ));
                    message = format!("开局小游戏已设为 {}", ids.join(","));
                }
                _ if rest.starts_with("set_small_game_count:") => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可设置".into()));
                    }
                    let n: u32 = rest
                        .trim_start_matches("set_small_game_count:")
                        .parse()
                        .map_err(|_| EngineError::IllegalAction("小游戏数量必须是数字".into()))?;
                    if !(MIN_SMALL_GAMES..=MAX_SMALL_GAMES).contains(&n) {
                        return Err(EngineError::IllegalAction(
                            "小游戏数量须在 3–6 之间".into(),
                        ));
                    }
                    st.small_game_count = n;
                    events.push(EngineEvent::new(
                        "small_game_count_set",
                        json!({ "host": player_id, "n": n }),
                    ));
                    message = format!("小游戏数量设为 {n}");
                }
                "start" => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可开始游戏".into()));
                    }
                    if !(2..=MAX_PLAYERS).contains(&st.seats.len()) {
                        return Err(EngineError::IllegalAction(
                            "开赛人数须在 2–6 之间".into(),
                        ));
                    }
                    let setup = self.setup_round(&mut st, 1, rng);
                    let start = st.host.clone();
                    st.start_player = Some(start.clone());
                    st.turn = st
                        .seats
                        .iter()
                        .position(|s| s.player_id == start)
                        .unwrap_or(0);
                    events.push(EngineEvent::new(
                        "game_started",
                        json!({
                            "seats": st.seats.iter().map(|s| s.player_id.clone()).collect::<Vec<_>>(),
                            "start_player": start,
                            "setup": setup,
                        }),
                    ));
                    let mut rolled = self.begin_turn(&mut st, rng);
                    events.append(&mut rolled);
                    message = format!("游戏开始，起始玩家 {start}，请下注");
                }
                "abort" => {
                    if player_id != st.host {
                        return Err(EngineError::PermissionDenied("仅发起人可中断本局".into()));
                    }
                    st.phase = Phase::Aborted;
                    finished = true;
                    events.push(EngineEvent::new(
                        "game_aborted",
                        json!({ "host": player_id, "reason": "abort" }),
                    ));
                    message = "本局已被发起人中断".to_string();
                }
                _ => return Err(EngineError::IllegalAction("无法识别的大厅动作".into())),
            }
        } else if let Some(nstr) = action.strip_prefix("place:") {
            if st.phase != Phase::TurnRoll {
                return Err(EngineError::IllegalAction("当前阶段不能下注".into()));
            }
            let n: u8 = nstr
                .parse()
                .map_err(|_| EngineError::IllegalAction("点数必须是数字".into()))?;
            if !(1..=CASINOS as u8).contains(&n) {
                return Err(EngineError::IllegalAction("点数须在 1–6 之间".into()));
            }
            let cur = st
                .seats
                .get(st.turn)
                .ok_or_else(|| EngineError::Internal("回合下标越界".into()))?
                .clone();
            if cur.player_id != player_id {
                return Err(EngineError::PermissionDenied("未轮到你".into()));
            }
            if !st.roll.points().contains(&n) {
                return Err(EngineError::IllegalAction(format!(
                    "本轮未掷出 {n} 点"
                )));
            }
            if st.sg.banned == Some((n - 1) as usize) {
                return Err(EngineError::IllegalAction("该赌场被禁止进入".into()));
            }

            let matched_small = st.roll.small.iter().filter(|&&v| v == n).count() as u32;
            let big_matched = st.roll.big == Some(n);
            let weight = matched_small + if big_matched { 2 } else { 0 };
            {
                let seat = &mut st.seats[st.turn];
                seat.small_remaining = seat.small_remaining.saturating_sub(matched_small as u8);
                if big_matched {
                    seat.big_remaining = seat.big_remaining.saturating_sub(1);
                }
                *st.casinos[(n - 1) as usize]
                    .dice
                    .entry(player_id.to_string())
                    .or_insert(0) += weight;
                if big_matched {
                    *st.casinos[(n - 1) as usize]
                        .big_dice
                        .entry(player_id.to_string())
                        .or_insert(0) += 1;
                }
                // 若已无骰子则清空本轮投掷结果
                if !seat.has_dice() {
                    st.roll = Roll::default();
                }
            }
            events.push(EngineEvent::new(
                "dice_placed",
                json!({ "player_id": player_id, "point": n, "weight": weight, "casino": n }),
            ));
            // 下注后触发的小游戏（自动 + 交互；交互类挂 pending 暂停回合）。
            let events_before = events.len();
            sg::on_place(&mut st, (n - 1) as usize, player_id, rng, &mut events);
            if let Some(pending) = st.sg.pending.clone() {
                st.phase = Phase::SmallGame;
                message = format!(
                    "你把 {n} 点的 {weight} 个骰子放到 {n} 号赌场；触发小游戏 {}，等待 {} 操作",
                    pending.game, pending.actor
                );
            } else {
                // 只对**实际触发**的小游戏给出提示（避免 C1 未到第 5 颗却显示“触发”）。
                let mut triggered: Vec<String> = Vec::new();
                for e in &events[events_before..] {
                    if let Some(g) = e.payload.get("game").and_then(|v| v.as_str()) {
                        triggered.push(g.to_string());
                    }
                }
                let mut parts: Vec<String> = Vec::new();
                // 自动类小游戏（A2/D1/C1 等）直接给出「掷了什么、得了什么」的可读提示；
                // 未实现摘要的编号退化为「触发小游戏 X」。
                let mut detailed: Vec<String> = sg::event_summaries(&events[events_before..]);
                let covered: std::collections::BTreeSet<&str> = detailed
                    .iter()
                    .filter_map(|l| l.split_whitespace().next())
                    .collect();
                triggered.retain(|g| !covered.contains(g.as_str()));
                parts.append(&mut detailed);
                if !triggered.is_empty() {
                    parts.push(format!("触发小游戏 {}", triggered.join("/")));
                }
                let settle: Vec<String> = st.casinos[(n - 1) as usize]
                    .small_games
                    .iter()
                    .filter(|t| t.as_str() == "B1" || t.as_str() == "C2")
                    .cloned()
                    .collect();
                if !settle.is_empty() {
                    parts.push(format!("{} 结算前触发", settle.join("/")));
                }
                let note = if parts.is_empty() {
                    String::new()
                } else {
                    format!("（{}）", parts.join("；"))
                };
                let (mut chain, fin) = self.after_action(&mut st, rng);
                events.append(&mut chain);
                finished = fin;
                message = format!("你把 {n} 点的 {weight} 个骰子放到了 {n} 号赌场{note}");
            }
        } else if action == "reroll" {
            if st.phase != Phase::TurnRoll {
                return Err(EngineError::IllegalAction("当前阶段不能弃回合".into()));
            }
            // D2 特权持有者本回合不掷骰，没有可弃的投掷结果。
            if st.roll.points().is_empty() {
                return Err(EngineError::IllegalAction(
                    "本轮没有投掷结果（D2 特权回合），请用特权放骰".into(),
                ));
            }
            let cur = st
                .seats
                .get(st.turn)
                .ok_or_else(|| EngineError::Internal("回合下标越界".into()))?;
            if cur.player_id != player_id {
                return Err(EngineError::PermissionDenied("未轮到你".into()));
            }
            if cur.chips < 1 {
                return Err(EngineError::IllegalAction("筹码不足".into()));
            }
            st.seats[st.turn].chips -= 1;
            st.roll = Roll::default();
            events.push(EngineEvent::new(
                "reroll_paid",
                json!({ "player_id": player_id, "chips_left": st.seats[st.turn].chips }),
            ));
            let (mut chain, fin) = self.after_action(&mut st, rng);
            events.append(&mut chain);
            finished = fin;
            message = "你支付了 1 枚筹码，跳过本回合".to_string();
        } else if action.starts_with("sg:") {
            if st.phase == Phase::SmallGame {
                let (done, mut ev, m) = sg::resolve(&mut st, action, player_id, rng)?;
                events.append(&mut ev);
                message = m;
                if done {
                    let was_g1 = st
                        .sg
                        .pending
                        .as_ref()
                        .map(|p| p.game == "G1")
                        .unwrap_or(false);
                    st.sg.pending = None;
                    // 同一次下注可能触发多个交互：队列里还有就继续处理（不推进回合）。
                    if let Some((game, actor)) = sg::promote_queued_pending(&mut st) {
                        st.phase = Phase::SmallGame;
                        let next_msg =
                            format!("{message}；接下来是小游戏 {game}，等待 {actor} 操作");
                        let new_state = Self::build_state(state, &st);
                        return Ok(ActionResult {
                            state: new_state,
                            message: next_msg,
                            finished: false,
                            events,
                        });
                    }
                    if was_g1 {
                        // G1 属于结算前钩子：完成后恢复结算（而非推进回合）
                        let (mut chain, fin) = self.settle_round(&mut st, rng);
                        events.append(&mut chain);
                        finished = fin;
                    } else {
                        st.phase = Phase::TurnRoll;
                        let (mut chain, fin) = self.after_action(&mut st, rng);
                        events.append(&mut chain);
                        finished = fin;
                    }
                }
            } else {
                match sg::resolve_turn_sg(&mut st, action, player_id, rng) {
                    Some(Ok((mut ev, m, advance))) => {
                        events.append(&mut ev);
                        message = m;
                        if advance {
                            let (mut chain, fin) = self.after_action(&mut st, rng);
                            events.append(&mut chain);
                            finished = fin;
                        }
                    }
                    Some(Err(e)) => return Err(e),
                    None => {
                        return Err(EngineError::IllegalAction(format!(
                            "无法识别的动作: {action}"
                        )))
                    }
                }
            }
        } else {
            return Err(EngineError::IllegalAction(format!(
                "无法识别的动作: {action}"
            )));
        }

        let new_state = Self::build_state(state, &st);
        Ok(ActionResult {
            state: new_state,
            message,
            finished,
            events,
        })
    }

    fn public_state(&self, state: &GameState) -> GameState {
        let Ok(mut st) = Self::parse(&state.data) else {
            return state.clone();
        };
        mask_public(&mut st);
        Self::build_state(state, &st)
    }

    /// 个人视图：公共脱敏 + 只给本人的加密选项（A1 代码 ↔ 枚数）。
    ///
    /// 代码本身可以公开（群里能看到代码，但没有对应关系就推不出枚数），
    /// 因此 `codes` 保留在公共视图，`perm/count` 只走个人视图。
    fn player_view(&self, state: &GameState, viewer: &str) -> GameState {
        let Ok(mut st) = Self::parse(&state.data) else {
            return self.public_state(state);
        };
        // 必须在脱敏**之前**取出私有选项（脱敏会删掉 perm）。
        let opts = st
            .sg
            .pending
            .as_ref()
            .filter(|p| p.actor == viewer)
            .and_then(|p| sg::private_options(p, &st.sg.lottery));
        mask_public(&mut st);
        st.sg.pending_options = opts;
        Self::build_state(state, &st)
    }

    fn summarize(&self, state: &GameState) -> String {
        let Ok(st) = Self::parse(&state.data) else {
            return self.game_id().to_string();
        };
        match st.phase {
            Phase::Lobby => {
                let names = st
                    .seats
                    .iter()
                    .map(|s| s.player_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "拉斯维加斯（等待开局）：房主 {}，已加入 {} 人 [{}]，小游戏 N={}",
                    st.host,
                    st.seats.len(),
                    names,
                    st.small_game_count
                )
            }
            Phase::Finished => {
                let winners = st.winners.join(", ");
                format!("拉斯维加斯（已结束）：胜者 {winners}；比分 {:?}", st.scores)
            }
            Phase::Aborted => "拉斯维加斯：本局已废弃（不结算）".to_string(),
            Phase::SmallGame => {
                let (actor, game) = st
                    .sg
                    .pending
                    .as_ref()
                    .map(|p| (p.actor.clone(), p.game.clone()))
                    .unwrap_or_else(|| ("-".to_string(), "-".to_string()));
                format!(
                    "拉斯维加斯 第 {}/{} 轮，等待 {} 处理小游戏 {}",
                    st.round, ROUNDS, actor, game
                )
            }
            _ => {
                let cur = st
                    .seats
                    .get(st.turn)
                    .map(|s| {
                        format!(
                            "{}（手牌 {}小{}大，筹码 {}）",
                            s.player_id, s.small_remaining, s.big_remaining, s.chips
                        )
                    })
                    .unwrap_or_else(|| "-".to_string());
                let placed: Vec<String> = (0..CASINOS)
                    .map(|i| {
                        let sg = st.casinos[i].small_games.join("/");
                        let tag = if sg.is_empty() {
                            String::new()
                        } else {
                            format!("({sg})")
                        };
                        format!("{}号{}骰{}", i + 1, st.casinos[i].dice.values().sum::<u32>(), tag)
                    })
                    .collect();
                // 展示本轮掷骰结果（先掷后选）：人类/主持人都能看到具体点数与颗数。
                let roll = if st.roll.small.is_empty() && st.roll.big.is_none() {
                    "（尚未掷骰）".to_string()
                } else {
                    let small = st
                        .roll
                        .small
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let big = st
                        .roll
                        .big
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    format!("掷出 小[{small}] 大{big}；")
                };
                format!(
                    "拉斯维加斯 第 {}/{} 轮，等待 {} 下注；{roll}赌场骰数 [{}]",
                    st.round,
                    ROUNDS,
                    cur,
                    placed.join(" ")
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 结算与抽牌辅助
// ---------------------------------------------------------------------------

/// 结算单个赌场：先剔除骰数并列者，剩余骰数最多/次多者拿最高/次高钞票。
/// 返回（分配明细，第一名玩家）。
fn settle_casino(casino: &mut Casino, seats: &mut [Seat]) -> (Vec<serde_json::Value>, Option<String>) {
    let counts = casino.dice.clone();
    let mut by_value: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (p, c) in &counts {
        if *c > 0 {
            by_value.entry(*c).or_default().push(p.clone());
        }
    }
    let mut kept: Vec<(String, u32)> = counts
        .iter()
        .filter(|(_p, c)| **c > 0 && by_value.get(c).map(|v| v.len() == 1).unwrap_or(false))
        .map(|(p, c)| (p.clone(), *c))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut notes = casino.banknotes.clone();
    notes.sort_unstable_by(|a, b| b.cmp(a));

    // 灰骰参与排名但不领钞票：钞票按 kept 中的**玩家**顺序发放（最多 2 张）。
    let mut logs = Vec::new();
    let mut top: Option<String> = None;
    let mut note_idx = 0usize;
    for (p, c) in &kept {
        let note = if p != "gray" && note_idx < notes.len() {
            let n = notes[note_idx];
            note_idx += 1;
            Some(n)
        } else {
            None
        };
        if let Some(n) = note {
            if let Some(seat) = seats.iter_mut().find(|s| &s.player_id == p) {
                seat.banknotes.push(n);
            }
        }
        if top.is_none() && p != "gray" {
            top = Some(p.clone());
        }
        logs.push(json!({ "player_id": p, "dice": c, "note": note }));
    }
    casino.dice.clear();
    casino.big_dice.clear();
    (logs, top)
}

/// 完整钞票牌库：3–10 万按 11/11/13/15/13/11/9/7 各若干张（共 90 张）。
fn banknote_deck() -> Vec<u32> {
    let mut deck = Vec::with_capacity(DENOMINATION_COPIES.iter().sum());
    for (d, copies) in DENOMINATIONS.iter().zip(DENOMINATION_COPIES.iter()) {
        for _ in 0..*copies {
            deck.push(*d);
        }
    }
    deck
}

/// 16→8→N：每字母组 A–H 取 1 个 → 洗牌取 N 个。
fn draw_small_games(n: u32, rng: &mut dyn RandomSource) -> Vec<String> {
    const LETTERS: [char; 8] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
    let mut candidates: Vec<String> = Vec::with_capacity(8);
    for letter in LETTERS {
        let side = (rng.next_u64() % 2) + 1;
        candidates.push(format!("{letter}{side}"));
    }
    shuffle(rng, &mut candidates);
    let take = (n as usize).min(candidates.len());
    candidates.truncate(take);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine_core::{EngineRegistry, GameManager, SeededRng};

    fn manager(seed: u64) -> GameManager {
        let mut reg = EngineRegistry::new();
        reg.register(LasVegasEngine);
        GameManager::with_rng(reg, Box::new(SeededRng::new(seed)))
    }

    fn new_lobby(mgr: &GameManager, host: &str) -> GameState {
        mgr.create_game("las-vegas", "group-1", host).unwrap().0
    }

    fn start_two(mgr: &GameManager, host: &str, p2: &str) -> GameState {
        let st = new_lobby(mgr, host);
        mgr.execute_action(&st.room_id, p2, "lobby:join").unwrap();
        let r = mgr.execute_action(&st.room_id, host, "lobby:start").unwrap();
        r.state
    }

    fn data(state: &GameState) -> LasVegasState {
        serde_json::from_value(state.data.clone()).unwrap()
    }

    /// 若处于小游戏交互，替 pending.actor 选第一个合法 sg 动作，直到退出交互态。
    fn resolve_pending(mgr: &GameManager, mut st: GameState) -> GameState {
        loop {
            let d = data(&st);
            if d.phase != Phase::SmallGame {
                return st;
            }
            let actor = d
                .sg
                .pending
                .as_ref()
                .map(|p| p.actor.clone())
                .unwrap_or_default();
            let actions = mgr.legal_actions(&st.room_id, &actor).unwrap();
            let action = actions
                .first()
                .map(|a| a.id.clone())
                .expect("pending 应有合法动作");
            st = mgr.execute_action(&st.room_id, &actor, &action).unwrap().state;
        }
    }

    #[test]
    fn lobby_create_join_start() {
        let mgr = manager(1);
        let st = new_lobby(&mgr, "host");
        let d = data(&st);
        assert_eq!(d.phase, Phase::Lobby);
        assert_eq!(d.seats.len(), 1);
        assert_eq!(d.small_game_count, DEFAULT_SMALL_GAMES);

        let st = mgr.execute_action(&st.room_id, "p2", "lobby:join").unwrap().state;
        assert_eq!(data(&st).seats.len(), 2);

        let out = mgr.execute_action(&st.room_id, "host", "lobby:start").unwrap();
        let d = data(&out.state);
        assert_eq!(d.phase, Phase::TurnRoll);
        assert_eq!(d.round, 1);
        assert_eq!(d.seats.len(), 2);
        for s in &d.seats {
            assert_eq!(s.small_remaining, SMALL_DICE);
            assert_eq!(s.big_remaining, BIG_DICE);
            assert_eq!(s.chips, INITIAL_CHIPS);
        }
        // 每赌场 2 张钞票
        for c in &d.casinos {
            assert_eq!(c.banknotes.len(), 2);
        }
        // 6 号赌场总额 >= 1 号赌场
        let sum6: u32 = d.casinos[5].banknotes.iter().sum();
        let sum1: u32 = d.casinos[0].banknotes.iter().sum();
        assert!(sum6 >= sum1);
    }

    #[test]
    fn non_host_cannot_start_or_abort() {
        let mgr = manager(2);
        let st = new_lobby(&mgr, "host");
        mgr.execute_action(&st.room_id, "p2", "lobby:join").unwrap();
        assert!(matches!(
            mgr.execute_action(&st.room_id, "p2", "lobby:start"),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(matches!(
            mgr.execute_action(&st.room_id, "p2", "lobby:abort"),
            Err(EngineError::PermissionDenied(_))
        ));
    }

    #[test]
    fn host_leave_aborts_game() {
        let mgr = manager(3);
        let st = new_lobby(&mgr, "host");
        mgr.execute_action(&st.room_id, "p2", "lobby:join").unwrap();
        let out = mgr.execute_action(&st.room_id, "host", "lobby:leave").unwrap();
        assert!(out.finished);
        assert_eq!(data(&out.state).phase, Phase::Aborted);
        assert!(mgr.get_active_game("group-1").is_none());
    }

    #[test]
    fn host_can_add_bot() {
        let mgr = manager(11);
        let st = new_lobby(&mgr, "host");
        let out = mgr
            .execute_action(&st.room_id, "host", "lobby:add_bot")
            .unwrap();
        let d = data(&out.state);
        assert_eq!(d.seats.len(), 2);
        assert!(d.seats[1].is_bot);
        assert!(matches!(
            mgr.execute_action(&st.room_id, "p2", "lobby:add_bot"),
            Err(EngineError::PermissionDenied(_))
        ));
    }

    #[test]
    fn host_can_abort_during_play() {
        let mgr = manager(12);
        let st = start_two(&mgr, "host", "p2");
        let out = mgr.execute_action(&st.room_id, "host", "lobby:abort").unwrap();
        assert!(out.finished);
        assert_eq!(data(&out.state).phase, Phase::Aborted);
        assert!(mgr.get_active_game("group-1").is_none());
        // 非房主不可中断
        let mgr2 = manager(13);
        let st2 = start_two(&mgr2, "host", "p2");
        assert!(matches!(
            mgr2.execute_action(&st2.room_id, "p2", "lobby:abort"),
            Err(EngineError::PermissionDenied(_))
        ));
    }

    #[test]
    fn human_place_with_interactive_small_game_waits() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games.push("F2".into());
        // F2 要求场上至少有 1 颗灰骰（重做版）。
        casinos[0].dice.insert("gray".into(), 1);
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("bot1".into())],
            roll: Roll { small: vec![1], big: None },
            casinos,
            small_game_count: 3,
            sg: SgState {
                f2_slots: vec![None; sg::F2_SLOTS],
                ..Default::default()
            },
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        let mut rng = SeededRng::new(1);
        let out = engine.execute_action(&gs, "h", "place:1", &mut rng).unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.phase, Phase::SmallGame, "应停在小游戏交互");
        assert_eq!(d.sg.pending.as_ref().unwrap().actor, "h", "应由真人操作");
        assert_eq!(d.seats[d.turn].player_id, "h", "回合不应推进");
        assert!(!out.finished);
    }

    #[test]
    fn place_only_allows_rolled_points() {
        let engine = LasVegasEngine;
        let casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        let mut h = Seat::new("h".into());
        h.small_remaining = 3; // 与 roll 的 3 颗一致
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![h, Seat::new("bot1".into())],
            roll: Roll {
                small: vec![1, 1, 3],
                big: None,
            },
            casinos,
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        let mut rng = SeededRng::new(1);
        // 未掷出的点数不可下注
        assert!(matches!(
            engine.execute_action(&gs, "h", "place:2", &mut rng),
            Err(EngineError::IllegalAction(_))
        ));
        // 已掷出的 1 点：2 颗全部放入 1 号赌场
        let out = engine.execute_action(&gs, "h", "place:1", &mut rng).unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.casinos[0].dice.get("h").copied(), Some(2));
        assert_eq!(d.seats[0].small_remaining, 1, "未放置的 3 点留在手中");
    }

    #[test]
    fn f1_two_step_group_then_casino() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games.push("F1".into());
        let mut h = Seat::new("h".into());
        h.small_remaining = 1;
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![h, Seat::new("bot1".into())],
            roll: Roll { small: vec![1], big: None },
            casinos,
            small_game_count: 3,
            sg: SgState { gray_groups: sg::F1_GROUPS.to_vec(), ..Default::default() },
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        let mut rng = SeededRng::new(1);
        let out = engine.execute_action(&gs, "h", "place:1", &mut rng).unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.phase, Phase::SmallGame);
        assert_eq!(d.sg.pending.as_ref().unwrap().game, "F1");
        assert_eq!(d.sg.pending.as_ref().unwrap().stage, "group");
        // 第一轮：只返回「选组」
        let acts = engine.legal_actions(&out.state, "h");
        assert!(acts.iter().all(|a| a.id.starts_with("sg:F1:group:")));
        assert_eq!(acts.len(), 5);
        let step1 = engine
            .execute_action(&out.state, "h", "sg:F1:group:0", &mut rng)
            .unwrap();
        let d1: LasVegasState = serde_json::from_value(step1.state.data.clone()).unwrap();
        assert_eq!(d1.sg.pending.as_ref().unwrap().stage, "casino");
        // 第二轮：只返回「选赌场」
        let acts2 = engine.legal_actions(&step1.state, "h");
        assert!(acts2.iter().all(|a| a.id.starts_with("sg:F1:move:0:")));
        assert_eq!(acts2.len(), CASINOS);
        let step2 = engine
            .execute_action(&step1.state, "h", "sg:F1:move:0:3", &mut rng)
            .unwrap();
        let d2: LasVegasState = serde_json::from_value(step2.state.data.clone()).unwrap();
        assert!(d2.sg.pending.is_none());
        assert_eq!(d2.casinos[2].dice.get("gray").copied(), Some(sg::F1_GROUPS[0]));
        assert_eq!(d2.sg.gray_groups[0], 0);
    }

    #[test]
    fn h1_source_and_slot_are_player_choices() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games.push("H1".into());
        let mut h = Seat::new("h".into());
        h.small_remaining = 1;
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![h, Seat::new("bot1".into())],
            roll: Roll { small: vec![1], big: None },
            casinos,
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        let mut rng = SeededRng::new(1);
        let out = engine.execute_action(&gs, "h", "place:1", &mut rng).unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.phase, Phase::SmallGame);
        assert_eq!(d.sg.pending.as_ref().unwrap().game, "H1");
        assert_eq!(d.sg.pending.as_ref().unwrap().stage, "source");
        let acts = engine.legal_actions(&out.state, "h");
        assert!(acts.iter().any(|a| a.id == "sg:H1:take:casino"));
        assert!(!acts.iter().any(|a| a.id.starts_with("sg:H1:slot:")));

        // 选来源（该赌场骰子）→ 进入格位选择
        let step = engine
            .execute_action(&out.state, "h", "sg:H1:take:casino", &mut rng)
            .unwrap();
        let d2: LasVegasState = serde_json::from_value(step.state.data.clone()).unwrap();
        assert_eq!(d2.sg.pending.as_ref().unwrap().stage, "slot");
        let slot_acts = engine.legal_actions(&step.state, "h");
        assert_eq!(slot_acts.len(), 6, "格位由玩家自选 1–6");

        // 选 5 号格 → 结算按 5 号格给 $5 万（格位自选）
        let res = engine
            .execute_action(&step.state, "h", "sg:H1:slot:5", &mut rng)
            .unwrap();
        let d3: LasVegasState = serde_json::from_value(res.state.data.clone()).unwrap();
        assert!(d3.sg.pending.is_none());
        assert_eq!(d3.sg.h1_slots[4].as_ref().unwrap().owner, "h");
        assert_eq!(d3.casinos[0].dice.get("h").copied(), None, "骰子已移入格位");
    }

    #[test]
    fn h1_slot_rewards_follow_ladder() {
        // 1 格 → 1 筹码；2 格 → 2 筹码；3–6 格 → $3/4/5/6 万 钞票（多颗叠加）。
        let mut st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::Settling,
            host: "a".into(),
            round: 1,
            start_player: Some("a".into()),
            turn: 0,
            seats: vec![Seat::new("a".into()), Seat::new("b".into())],
            roll: Roll::default(),
            casinos: (0..CASINOS).map(|_| Casino::empty()).collect(),
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        st.casinos[0].small_games.push("H1".into());
        st.sg.h1_slots = vec![None; 6];
        let slot = |owner: &str, dice: u32, big: u32| {
            Some(crate::sg::H1Slot {
                owner: owner.into(),
                dice,
                big,
            })
        };
        st.sg.h1_slots[1] = slot("a", 2, 0); // 2 格 × 2 颗 → 4 筹码
        st.sg.h1_slots[3] = slot("b", 1, 0); // 4 格 → $4 万
        st.sg.h1_slots[5] = slot("a", 2, 1); // 6 格：1 小 + 1 大（大骰也只算 1 颗）→ 2 张 $6 万
        let mut events = Vec::new();
        crate::sg::settle_extras(&mut st, &mut events);
        assert_eq!(st.seats[0].chips, INITIAL_CHIPS + 4);
        assert_eq!(st.seats[1].banknotes, vec![40_000]);
        assert_eq!(st.seats[0].banknotes, vec![60_000, 60_000]);
        assert!(st.sg.h1_slots.iter().all(|s| s.is_none()), "结算后清空格位");
    }

    #[test]
    fn h1_kick_returns_dice_to_slot_casino_without_small_game() {
        // 3 号格已被 b 占（2 小 + 1 大）→ a 挤入后：b 的骰子整批回 3 号赌场（权重 1+1+2=4，大骰 1 颗）。
        let mut st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::SmallGame,
            host: "a".into(),
            round: 1,
            start_player: Some("a".into()),
            turn: 0,
            seats: vec![Seat::new("a".into()), Seat::new("b".into())],
            roll: Roll::default(),
            casinos: (0..CASINOS).map(|_| Casino::empty()).collect(),
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        // 3 号赌场：b 原有 1 颗小骰，挂 C1 + D2。
        st.casinos[2].small_games = vec!["C1".into(), "D2".into()];
        st.casinos[2].dice.insert("b".into(), 1);
        st.sg.h1_slots = vec![None; 6];
        st.sg.h1_slots[2] = Some(crate::sg::H1Slot {
            owner: "b".into(),
            dice: 3,
            big: 1,
        });
        let mut events = Vec::new();
        crate::sg::h1_slot_put(&mut st, "a", 3, false, &mut events);

        assert_eq!(
            st.casinos[2].dice.get("b").copied(),
            Some(5),
            "1 颗原有 + 挤回的 2 小 + 大骰 2 权重 = 5"
        );
        assert_eq!(st.casinos[2].big_dice.get("b").copied(), Some(1));
        assert!(
            st.sg.c1_winner.is_none(),
            "挤出加骰到第 5 颗不得触发 C1（规则：不触发该赌场小游戏）"
        );
        assert!(
            st.sg.privilege.is_none(),
            "D2 只在主动下注时判定 → 挤出加骰不重算特权（保持原状：无）"
        );
        let s = st.sg.h1_slots[2].clone().unwrap();
        assert_eq!((s.owner.as_str(), s.dice, s.big), ("a", 1, 0));
        assert!(
            events.iter().any(|e| e
                .payload
                .get("kicked_player")
                .map(|v| v == "b")
                .unwrap_or(false)),
            "挤出应入事件（可复盘）"
        );
    }

    #[test]
    fn h1_prefers_small_die_and_never_halves_big() {
        let engine = LasVegasEngine;
        let run = |dice: u32, big: u32| -> LasVegasState {
            let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
            casinos[0].small_games.push("H1".into());
            casinos[0].dice.insert("h".into(), dice);
            if big > 0 {
                casinos[0].big_dice.insert("h".into(), big);
            }
            let st = LasVegasState {
                schema_version: SCHEMA_VERSION,
                phase: Phase::SmallGame,
                host: "h".into(),
                round: 1,
                start_player: Some("h".into()),
                turn: 0,
                seats: vec![Seat::new("h".into()), Seat::new("bot1".into())],
                roll: Roll::default(),
                casinos,
                small_game_count: 3,
                sg: SgState {
                    pending: Some(sg::SgPending {
                        game: "H1".into(),
                        casino: 0,
                        actor: "h".into(),
                        stage: "source".into(),
                        data: json!({}),
                    }),
                    ..Default::default()
                },
                scores: BTreeMap::new(),
                winners: Vec::new(),
            };
            let gs = GameState {
                game_id: "las-vegas".into(),
                room_id: "r".into(),
                group_id: "g".into(),
                status: GameStatus::Active,
                players: vec!["h".into(), "bot1".into()],
                data: serde_json::to_value(&st).unwrap(),
            };
            let mut rng = SeededRng::new(7);
            let step = engine
                .execute_action(&gs, "h", "sg:H1:take:casino", &mut rng)
                .unwrap();
            let out = engine
                .execute_action(&step.state, "h", "sg:H1:slot:2", &mut rng)
                .unwrap();
            serde_json::from_value(out.state.data.clone()).unwrap()
        };

        // 既有小骰又有大骰（1 小 + 1 大 = 权重 3）：优先取小骰。
        let d = run(3, 1);
        assert_eq!(d.casinos[0].dice.get("h").copied(), Some(2), "取走的是小骰");
        assert_eq!(d.casinos[0].big_dice.get("h").copied(), Some(1));
        let s = d.sg.h1_slots[1].as_ref().unwrap();
        assert_eq!((s.dice, s.big), (1, 0));

        // 只剩大骰（权重 2）：整颗取走，不留半个；格内只算 1 颗。
        let d = run(2, 1);
        assert_eq!(d.casinos[0].dice.get("h").copied(), None, "大骰不可拆");
        assert_eq!(d.casinos[0].big_dice.get("h").copied(), None);
        let s = d.sg.h1_slots[1].as_ref().unwrap();
        assert_eq!((s.dice, s.big), (1, 1), "大骰进格与小骰同价（只算 1 颗）");
    }

    #[test]
    fn lobby_can_override_opening_small_games() {
        let mgr = manager(14);
        let st = new_lobby(&mgr, "host");
        mgr.execute_action(&st.room_id, "p2", "lobby:join").unwrap();
        let out = mgr
            .execute_action(&st.room_id, "host", "lobby:set_small_games:H1,G1,F1")
            .unwrap();
        assert!(out.message.contains("H1"));
        let started = mgr.execute_action(&st.room_id, "host", "lobby:start").unwrap();
        let d = data(&started.state);
        let tiles: Vec<String> = d
            .casinos
            .iter()
            .take(3)
            .map(|c| c.small_games.join("/"))
            .collect();
        assert_eq!(
            tiles,
            vec!["H1".to_string(), "G1".to_string(), "F1".to_string()]
        );
    }

    #[test]
    fn h1_actions_only_after_betting_at_h1_casino() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games.push("H1".into());
        let mut h = Seat::new("h".into());
        h.small_remaining = 1;
        let st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![h, Seat::new("bot1".into())],
            roll: Roll { small: vec![1], big: None },
            casinos,
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        // 下注前：不提供 H1 动作
        let before = engine.legal_actions(&gs, "h");
        assert!(
            !before.iter().any(|a| a.id.starts_with("sg:H1:")),
            "下注前不应出现 H1 动作"
        );
        let mut rng = SeededRng::new(1);
        // 下注 1 号（H1 赌场）后才出现 H1
        let out = engine.execute_action(&gs, "h", "place:1", &mut rng).unwrap();
        let after = engine.legal_actions(&out.state, "h");
        assert!(after.iter().any(|a| a.id == "sg:H1:take:casino"));
        assert!(after.iter().any(|a| a.id == "sg:H1:skip"));
    }

    #[test]
    fn g1_runs_before_settlement_and_resumes() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games.push("G1".into());
        casinos[0].banknotes = vec![70_000, 20_000];
        casinos[0].dice.insert("h".into(), 3);
        casinos[0].dice.insert("bot1".into(), 1);
        let mut st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("bot1".into())],
            roll: Roll::default(),
            casinos,
            small_game_count: 3,
            sg: SgState { lottery: vec![2, 2, 6, 0, 0, 0], ..Default::default() },
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let mut rng = SeededRng::new(1);
        let (events, finished) = engine.settle_round(&mut st, &mut rng);
        assert!(!finished);
        assert_eq!(st.phase, Phase::SmallGame, "结算前应先停 G1");
        let p = st.sg.pending.as_ref().unwrap();
        assert_eq!(p.game, "G1");
        assert_eq!(p.stage, "split");
        assert_eq!(p.actor, "bot1", "左手玩家分堆");
        assert_eq!(p.data["winner"], "h");
        assert_eq!(p.data["total"], 10);
        assert!(!events.is_empty());

        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "bot1".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        // 多选：从牌面页拿到的代码里选 1 个（这里用 pending 里的第 3 个代码）
        let codes = sg::pending_codes(st.sg.pending.as_ref().unwrap());
        let split = engine
            .execute_action(&gs, "bot1", &format!("sg:G1:split:{}", codes[2]), &mut rng)
            .unwrap();
        let d: LasVegasState = serde_json::from_value(split.state.data.clone()).unwrap();
        assert_eq!(d.sg.pending.as_ref().unwrap().actor, "h");
        assert_eq!(d.sg.pending.as_ref().unwrap().stage, "pick");

        let pick = engine
            .execute_action(&split.state, "h", "sg:G1:pick:0", &mut rng)
            .unwrap();
        let d2: LasVegasState = serde_json::from_value(pick.state.data.clone()).unwrap();
        assert!(d2.sg.pending.is_none());
        let hseat = d2.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert!(hseat.banknotes.contains(&60_000), "G1 奖励入账");
        assert!(hseat.banknotes.contains(&70_000), "结算最高面额入账");
        // 结算已恢复（本轮结束 → 第 2 轮 setup 或终局）
        assert!(d2.round >= 2 || d2.phase == Phase::Finished);
    }

    #[test]
    fn duplicate_create_rejected() {
        let mgr = manager(4);
        new_lobby(&mgr, "host");
        assert!(mgr.create_game("las-vegas", "group-1", "host2").is_err());
    }

    #[test]
    fn place_reduces_dice_and_moves_turn() {
        let mgr = manager(5);
        let st = start_two(&mgr, "host", "p2");
        let d = data(&st);
        let cur = d.seats[d.turn].player_id.clone();
        let point = d.roll.points()[0];
        let out = mgr
            .execute_action(&st.room_id, &cur, &format!("place:{point}"))
            .unwrap();
        let st2 = resolve_pending(&mgr, out.state);
        let d2 = data(&st2);
        let placed: u32 = d2.casinos[(point - 1) as usize].dice.values().sum();
        assert!(placed > 0);
        // 回合已推进到另一名玩家
        assert_ne!(d2.seats[d2.turn].player_id, cur);
    }

    #[test]
    fn reroll_costs_chip() {
        let mgr = manager(6);
        let st = start_two(&mgr, "host", "p2");
        let d = data(&st);
        let cur = d.seats[d.turn].player_id.clone();
        let out = mgr.execute_action(&st.room_id, &cur, "reroll").unwrap();
        let d2 = data(&out.state);
        let seat = d2.seats.iter().find(|s| s.player_id == cur).unwrap();
        assert_eq!(seat.chips, INITIAL_CHIPS - 1);
    }

    #[test]
    fn non_current_player_cannot_place() {
        let mgr = manager(7);
        let st = start_two(&mgr, "host", "p2");
        let d = data(&st);
        let cur = d.seats[d.turn].player_id.clone();
        let other = d
            .seats
            .iter()
            .find(|s| s.player_id != cur)
            .unwrap()
            .player_id
            .clone();
        let point = d.roll.points()[0];
        assert!(matches!(
            mgr.execute_action(&st.room_id, &other, &format!("place:{point}")),
            Err(EngineError::PermissionDenied(_))
        ));
    }

    #[test]
    fn settle_removes_ties_set_algorithm() {
        // {红3, 蓝2, 黄2, 绿1} → 名次 [红3, 绿1]
        let mut casino = Casino::empty();
        casino.banknotes = vec![60_000, 30_000];
        casino.dice.insert("red".into(), 3);
        casino.dice.insert("blue".into(), 2);
        casino.dice.insert("yellow".into(), 2);
        casino.dice.insert("green".into(), 1);
        let mut seats = vec![
            Seat::new("red".into()),
            Seat::new("blue".into()),
            Seat::new("yellow".into()),
            Seat::new("green".into()),
        ];
        let (logs, top) = settle_casino(&mut casino, &mut seats);
        assert_eq!(top.as_deref(), Some("red"));
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0]["player_id"], "red");
        assert_eq!(logs[1]["player_id"], "green");
        let red = seats.iter().find(|s| s.player_id == "red").unwrap();
        let green = seats.iter().find(|s| s.player_id == "green").unwrap();
        assert_eq!(red.banknotes, vec![60_000]);
        assert_eq!(green.banknotes, vec![30_000]);
        let blue = seats.iter().find(|s| s.player_id == "blue").unwrap();
        assert!(blue.banknotes.is_empty());
    }

    #[test]
    fn settle_removes_ties_example_two() {
        // {红3, 蓝3, 黄2, 绿1} → 名次 [黄2, 绿1]
        let mut casino = Casino::empty();
        casino.banknotes = vec![50_000, 40_000];
        casino.dice.insert("red".into(), 3);
        casino.dice.insert("blue".into(), 3);
        casino.dice.insert("yellow".into(), 2);
        casino.dice.insert("green".into(), 1);
        let mut seats: Vec<Seat> = ["red", "blue", "yellow", "green"]
            .iter()
            .map(|p| Seat::new((*p).to_string()))
            .collect();
        let (logs, top) = settle_casino(&mut casino, &mut seats);
        assert_eq!(top.as_deref(), Some("yellow"));
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0]["player_id"], "yellow");
        assert_eq!(logs[1]["player_id"], "green");
    }

    #[test]
    fn gray_dice_rank_but_no_note() {
        let mut casino = Casino::empty();
        casino.banknotes = vec![70_000, 20_000];
        casino.dice.insert("gray".into(), 3);
        casino.dice.insert("solo".into(), 1);
        let mut seats = vec![Seat::new("solo".into())];
        let (logs, top) = settle_casino(&mut casino, &mut seats);
        // gray 参与排名但不领钞票：solo（最高名次玩家）拿最高那张，top 也应是 solo
        assert_eq!(top.as_deref(), Some("solo"));
        assert_eq!(seats[0].banknotes, vec![70_000]);
        assert_eq!(logs.len(), 2);
    }

    #[test]
    fn public_state_masks_banknotes() {
        let mgr = manager(8);
        let st = start_two(&mgr, "host", "p2");
        let pub_state = mgr.public_state(&st.room_id).unwrap();
        let d = data(&pub_state);
        for c in &d.casinos {
            assert_eq!(c.banknotes.len(), 2);
            assert!(c.banknotes.iter().all(|n| *n == 0), "面额应脱敏为 0");
        }
        // 完整状态仍有面额
        let full = data(&st);
        assert!(full.casinos.iter().any(|c| c.banknotes.iter().any(|n| *n > 0)));
    }

    #[test]
    fn d2_privilege_holder_skips_roll_and_triggers_small_game() {
        let engine = LasVegasEngine;
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        casinos[0].small_games = vec!["D2".into()];
        // 5 号赌场挂 H1，并已有 h 的 1 颗骰（D2 放进去 → 触发 H1 的「刚下注」分支）
        casinos[4].small_games = vec!["H1".into()];
        casinos[4].dice.insert("h".into(), 1);
        let mut st = LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("p2".into())],
            roll: Roll::default(),
            casinos,
            small_game_count: 3,
            sg: SgState {
                privilege: Some("h".into()),
                ..Default::default()
            },
            scores: BTreeMap::new(),
            winners: Vec::new(),
        };
        let mut rng = SeededRng::new(9);
        // 特权持有者回合：不掷骰
        let ev = engine.begin_turn(&mut st, &mut rng);
        assert!(st.roll.points().is_empty(), "D2 持有者不掷骰");
        assert!(ev
            .iter()
            .any(|e| e.payload.get("skip_roll").and_then(|v| v.as_bool()).unwrap_or(false)));

        // 合法动作只有 D2 放置（6 个）
        let acts = engine.legal_actions(
            &GameState {
                game_id: "las-vegas".into(),
                room_id: "r".into(),
                group_id: "g".into(),
                status: GameStatus::Active,
                players: vec!["h".into(), "p2".into()],
                data: serde_json::to_value(&st).unwrap(),
            },
            "h",
        );
        assert_eq!(acts.len(), CASINOS);
        assert!(acts.iter().all(|a| a.id.starts_with("sg:D2:use:")));

        // 放到 5 号赌场 → 触发 H1（挂 pending 并暂停回合）
        let gs = GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: vec!["h".into(), "p2".into()],
            data: serde_json::to_value(&st).unwrap(),
        };
        let out = engine.execute_action(&gs, "h", "sg:D2:use:5", &mut rng).unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.casinos[4].dice.get("h").copied(), Some(2), "5 号赌场 +1 颗");
        assert_eq!(d.phase, Phase::SmallGame, "放入会触发该赌场小游戏");
        assert_eq!(d.sg.pending.as_ref().unwrap().game, "H1");
    }

    // ================= 跨板块集成测试（2026-10-17） =================

    fn base_state(tiles: &[&str]) -> LasVegasState {
        let mut casinos: Vec<Casino> = (0..CASINOS).map(|_| Casino::empty()).collect();
        for (i, t) in tiles.iter().enumerate() {
            casinos[i % 3].small_games.push((*t).to_string());
        }
        LasVegasState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::TurnRoll,
            host: "h".into(),
            round: 1,
            start_player: Some("h".into()),
            turn: 0,
            seats: vec![Seat::new("h".into()), Seat::new("p2".into())],
            roll: Roll::default(),
            casinos,
            small_game_count: 3,
            sg: SgState::default(),
            scores: BTreeMap::new(),
            winners: Vec::new(),
        }
    }

    fn gs_of(st: &LasVegasState) -> GameState {
        GameState {
            game_id: "las-vegas".into(),
            room_id: "r".into(),
            group_id: "g".into(),
            status: GameStatus::Active,
            players: st.seats.iter().map(|s| s.player_id.clone()).collect(),
            data: serde_json::to_value(st).unwrap(),
        }
    }

    #[test]
    fn integration_g2_move_then_field_settles() {
        // 下注 → G2 交互 → 移 2 颗进游戏场 → 结算时按第七赌场发 $6 万
        let engine = LasVegasEngine;
        let mut st = base_state(&["G2", "X", "Y"]);
        st.casinos[0].banknotes = vec![50_000, 30_000];
        st.seats[0].small_remaining = 1;
        st.roll = Roll { small: vec![1], big: None };
        let mut rng = SeededRng::new(21);
        let out = engine
            .execute_action(&gs_of(&st), "h", "place:1", &mut rng)
            .unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.phase, Phase::SmallGame);
        assert_eq!(d.sg.pending.as_ref().unwrap().game, "G2");
        // 移 1 颗进游戏场
        let step = engine
            .execute_action(&out.state, "h", "sg:G2:move:1", &mut rng)
            .unwrap();
        let mut d2: LasVegasState = serde_json::from_value(step.state.data.clone()).unwrap();
        assert_eq!(d2.sg.game_field.get("h").copied(), Some(1));
        // 强制结算（此时全场手里无骰）
        d2.seats.iter_mut().for_each(|s| {
            s.small_remaining = 0;
            s.big_remaining = 0;
        });
        let (ev, _fin) = engine.settle_round(&mut d2, &mut rng);
        assert!(ev.iter().any(|e| e.payload.get("game").and_then(|v| v.as_str()) == Some("G2")));
        let h = d2.seats.iter().find(|s| s.player_id == "h").unwrap();
        // 游戏场：h 有 1 颗且唯一 → 第一 $6 万；原赌场 1 颗骰仍拿钞票
        assert!(h.banknotes.contains(&60_000), "游戏场第一名 $6 万: {:?}", h.banknotes);
        assert!(d2.sg.game_field.is_empty(), "结算后清空游戏场");
    }

    #[test]
    fn integration_pending_queue_keeps_both_bet_games() {
        // 同一赌场同时挂 G2 与 H1：同一次下注应先把 G2 处理完，再自动接到 H1
        let engine = LasVegasEngine;
        let mut st = base_state(&["G2"]);
        st.casinos[0].small_games.push("H1".into());
        st.seats[0].small_remaining = 2;
        st.roll = Roll { small: vec![1], big: None };
        let mut rng = SeededRng::new(4);
        let out = engine
            .execute_action(&gs_of(&st), "h", "place:1", &mut rng)
            .unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.sg.pending.as_ref().unwrap().game, "G2", "先触发 G2");
        let step = engine
            .execute_action(&out.state, "h", "sg:G2:move:0", &mut rng)
            .unwrap();
        let d2: LasVegasState = serde_json::from_value(step.state.data.clone()).unwrap();
        assert_eq!(d2.phase, Phase::SmallGame, "队列里还有 H1");
        assert_eq!(d2.sg.pending.as_ref().unwrap().game, "H1", "第二个交互不被吞掉");
        assert_eq!(d2.sg.pending.as_ref().unwrap().stage, "source");
    }

    #[test]
    fn integration_b1_black_dice_feed_c2_penalty() {
        // B1 在 1 号、C2 在 3 号：B1 加的黑骰先落地，C2 判定看得到
        let engine = LasVegasEngine;
        let mut st = base_state(&["B1", "X", "C2"]);
        st.casinos[0].small_games = vec!["B1".into()];
        st.casinos[2].small_games = vec!["C2".into()];
        st.casinos[0].dice.insert("h".into(), 3);
        st.casinos[2].dice.insert("h".into(), 1);
        st.casinos[2].dice.insert("p2".into(), 2);
        st.seats[1].chips = 3;
        let mut rng = SeededRng::new(6);
        let mut ev = Vec::new();
        // 第一次 pass 停在 B1（交互）
        assert!(sg::pre_settle_pass(&mut st, &mut rng, &mut ev), "B1 应挂 pending");
        let p = st.sg.pending.clone().unwrap();
        let black: Vec<u32> = serde_json::from_value(p.data.get("black").unwrap().clone()).unwrap();
        // 把黑骰放到 3 号赌场（若点数不匹配则等于放 0 颗，仍然验证顺序）
        let k = black.iter().filter(|d| **d == 3).count() as u32;
        let before_p2 = st.seats[1].chips;
        let mut ev2 = Vec::new();
        // 手工结算 B1（直接调 resolve）
        sg::resolve(&mut st, "sg:B1:put:3", "h", &mut rng).unwrap();
        assert!(!sg::pre_settle_pass(&mut st, &mut rng, &mut ev2), "第二次 pass 不再挂 B1");
        // C2 已执行：最少者 p2 付 5 万（先钞票再筹码）
        assert!(st.sg.c2_done.contains(&2), "C2 已在 3 号赌场执行");
        assert!(st.seats[1].chips <= before_p2, "C2 罚金不会让筹码变多");
        if k > 0 {
            assert!(st.casinos[2].dice.get("h").copied().unwrap_or(0) > 1, "B1 加骰已入 3 号");
        }
        let _ = engine;
    }

    #[test]
    fn integration_resume_does_not_mutate_state() {
        let engine = LasVegasEngine;
        let mut st = base_state(&["A1"]);
        st.sg.pending = Some(sg::SgPending {
            game: "A1".into(),
            casino: 0,
            actor: "h".into(),
            stage: "hide".into(),
            data: json!({ "initiator": "h", "perm": [1, 2, 3], "codes": ["AAAA", "BBBB", "CCCC"] }),
        });
        st.phase = Phase::SmallGame;
        let before = serde_json::to_string(&st).unwrap();
        let out = engine.execute_action(&gs_of(&st), "h", "resume", &mut SeededRng::new(1)).unwrap();
        let after: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(serde_json::to_string(&after).unwrap(), before, "resume 不改状态");
        assert!(out.message.contains("A1"), "提示应指明小游戏: {}", out.message);
        assert!(out.events.is_empty(), "resume 不产生事件");
    }

    #[test]
    fn integration_c1_big_die_counts_as_one_piece() {
        // C1：4 小 + 1 大 = 5 颗 → 立即 $10 万（颗数口径）
        let engine = LasVegasEngine;
        let mut st = base_state(&["C1", "X", "Y"]);
        st.casinos[0].dice.insert("h".into(), 4);
        st.seats[0].small_remaining = 1;
        st.seats[0].big_remaining = 1;
        st.roll = Roll { small: vec![1], big: Some(1) };
        let mut rng = SeededRng::new(8);
        let out = engine
            .execute_action(&gs_of(&st), "h", "place:1", &mut rng)
            .unwrap();
        let d: LasVegasState = serde_json::from_value(out.state.data.clone()).unwrap();
        assert_eq!(d.sg.c1_winner.as_deref(), Some("h"));
        let h = d.seats.iter().find(|s| s.player_id == "h").unwrap();
        assert!(h.banknotes.contains(&100_000), "颗数达 5 立即 $10 万");
        assert!(out.message.contains("C1"), "回复应含 C1 提示: {}", out.message);
    }

    #[test]
    fn three_rounds_reach_finished() {
        let mgr = manager(9);
        let mut st = start_two(&mgr, "host", "p2");
        let mut guard = 0;
        while data(&st).phase != Phase::Finished {
            guard += 1;
            assert!(guard < 5000, "回合数异常");
            st = resolve_pending(&mgr, st);
            let d = data(&st);
            if d.phase == Phase::Finished {
                break;
            }
            let cur = d.seats[d.turn].player_id.clone();
            let points = d.roll.points();
            let banned = d.sg.banned;
            // D2 特权持有者不掷骰（roll 为空）→ 用特权放 1 颗。
            let action = if points.is_empty() {
                "sg:D2:use:1".to_string()
            } else {
                let point = points
                    .iter()
                    .find(|p| banned != Some((**p - 1) as usize))
                    .copied()
                    .unwrap_or(points[0]);
                format!("place:{point}")
            };
            st = mgr
                .execute_action(&st.room_id, &cur, &action)
                .unwrap()
                .state;
        }
        let d = data(&st);
        assert_eq!(d.phase, Phase::Finished);
        assert_eq!(st.status, GameStatus::Finished);
        assert_eq!(d.scores.len(), 2);
        assert!(!d.winners.is_empty());
        assert!(mgr.get_active_game("group-1").is_none());
        // 终局后拒绝任何动作
        assert!(matches!(
            mgr.execute_action(&st.room_id, "host", "place:1"),
            Err(EngineError::IllegalAction(_))
        ));
    }

    #[test]
    fn banknote_deck_has_90_cards_per_denomination_counts() {
        let deck = banknote_deck();
        assert_eq!(deck.len(), 90);
        for (d, copies) in DENOMINATIONS.iter().zip(DENOMINATION_COPIES.iter()) {
            assert_eq!(
                deck.iter().filter(|x| *x == d).count(),
                *copies,
                "面额 {d} 张数不符"
            );
        }
    }

    #[test]
    fn small_games_unique_per_letter_and_count() {
        let mut rng = SeededRng::new(3);
        let tiles = draw_small_games(4, &mut rng);
        assert_eq!(tiles.len(), 4);
        let letters: std::collections::BTreeSet<char> = tiles.iter().map(|t| t.chars().next().unwrap()).collect();
        assert_eq!(letters.len(), 4, "每字母组只应出现一次");
    }

    #[test]
    fn seeded_run_is_deterministic() {
        // 相同种子的两次对局应产生相同初始赌场钞票/小游戏。
        let run = |seed: u64| {
            let mgr = manager(seed);
            let st = start_two(&mgr, "host", "p2");
            let d = data(&st);
            (
                d.casinos.iter().map(|c| c.banknotes.clone()).collect::<Vec<_>>(),
                d.casinos.iter().map(|c| c.small_games.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(run(42), run(42));
    }
}
