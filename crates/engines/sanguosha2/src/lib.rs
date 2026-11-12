//! 三国杀引擎 v2（P-C 内核 spike → M1 卡牌结算 → M2 接线 → M3 视图/AI/回合驱动）。
//!
//! 目标：把 plan §8.5 的 6 类缺陷变成「写不出来 / 一定被测试抓到」：
//! | 缺陷模式 | 结构性对策 |
//! |---|---|
//! | A 触发点逐路径硬编码 | 唯一移动出口 + 语义事件；技能只订阅事件 |
//! | B 就地压帧被父帧 pop | 技能 handler 只返回 `Step`，栈只由 `advance` 改 |
//! | C 帧里暂存牌丢失 | 牌区唯一归属（`Zone::Frame`）+ `check_no_orphan_frames` |
//! | D 同一规则两处各写一半 | `active()` 单一状态源 + legal/execute 同源 + 订阅派生 |
//! | E 文案与语义相反 | `POutcome` 枚举 → 唯一文案表 |
//! | F 白名单/枚举式 UI | 视图/动作由注册表/属性派生 |
//!
//! M1（2026-11-06）：牌定义来自生成的 [`cards_generated`]（160 张），结算走
//! [`kernel::card_play::resolve_use`]。
//! M2（2026-11-06）：技能全部改走真实结算（`resolve_use` / `use_virtual` / `use_as`）；
//! `Game::use_card` 仅保留为「事件模拟」兼容入口。
//! M3（2026-11-06）：[`turn`] 回合驱动器 + [`view`] 三分区视图 + 手牌动作枚举（AI 最小接口）。
//!
//! 本 crate 已具备「大厅/选将/身份局/真实结算/三分区视图」（S1 起为注册做准备，见
//! `docs/games/sanguosha/engine-replacement/v2-gap-matrix.md`）。

pub mod ai;
pub mod cards;
pub mod cards_generated;
#[path = "generals_generated.rs"]
pub mod generals;
pub mod identity;
pub mod kernel;
pub mod lobby;
pub mod presentation;
pub mod sandbox;
pub mod skills;
pub mod turn;
pub mod view;

use game_engine_core::{
    Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus,
    RandomSource,
};
use serde_json::json;

use kernel::card_play;
use kernel::resolve;
use kernel::{Card, Game, Phase, Seat, Slot, Suit};

pub struct Sanguosha2Engine;

fn default_cards() -> Vec<Card> {
    vec![
        Card::new("c1", "杀", Suit::Spade, 7),
        Card::new("c2", "闪", Suit::Heart, 2),
        Card::new("c3", "桃", Suit::Diamond, 3),
        Card::new("c4", "无懈可击", Suit::Club, 4),
        Card::new("c5", "杀", Suit::Diamond, 9),
        Card::new("c6", "闪", Suit::Spade, 6),
        Card::equip("c7", "青釭剑", Suit::Spade, 6, Slot::Weapon),
        Card::equip("c8", "八卦阵", Suit::Spade, 2, Slot::Armor),
        Card::equip("c9", "赤兔", Suit::Heart, 5, Slot::HorseOffense),
        Card::equip("c10", "的卢", Suit::Club, 5, Slot::HorseDefense),
        Card::equip("c11", "木牛流马", Suit::Diamond, 5, Slot::Treasure),
        Card::new("c12", "杀", Suit::Club, 8),
        Card::new("c13", "闪", Suit::Diamond, 10),
        Card::new("c14", "桃", Suit::Heart, 12),
        Card::new("c15", "决斗", Suit::Spade, 1),
        Card::new("c16", "过河拆桥", Suit::Spade, 3),
        Card::new("c17", "顺手牵羊", Suit::Diamond, 4),
        Card::new("c18", "杀", Suit::Heart, 10),
        Card::new("c19", "闪", Suit::Club, 2),
        Card::new("c20", "桃", Suit::Club, 9),
    ]
}

/// 测试/沙盒用：3 人最小局面（周妃 / 卢植 / 路人）。
pub fn demo_game() -> Game {
    let seats = vec![
        Seat::new("p0", "zhoufei", &["liangyin", "kongsheng"], 3),
        Seat::new("p1", "luzhi", &["zhenliang"], 4),
        Seat::new("p2", "guest", &[], 4),
    ];
    Game::new(seats, default_cards())
}

pub(crate) fn state_of(room_id: &str, group_id: &str, players: Vec<String>, g: &Game) -> GameState {
    // `players` 与座位保持一致（大厅 `#加入`/`#加机器人` 后座位会增加；
    // 工作台/摘要按 `players.len()` 统计人数，不能停在建局时的房主列表）。
    let players = if g.seats.is_empty() {
        players
    } else {
        g.seats.iter().map(|s| s.name.clone()).collect()
    };
    GameState {
        game_id: "sanguosha".into(),
        room_id: room_id.to_string(),
        group_id: group_id.to_string(),
        status: if g.finished {
            GameStatus::Finished
        } else {
            GameStatus::Active
        },
        players,
        data: serde_json::to_value(g).unwrap_or(json!({})),
    }
}

/// 建局：**大厅阶段**（只含房主；`#加入` / `#加机器人` 再补位）。
///
/// `new_game` 不再直接开打（S1 起）：测试/示例需要可玩开局时用 [`Sanguosha2Engine::new_started_game`]。
fn new_lobby_game(
    room_id: &str,
    group_id: &str,
    players: Vec<String>,
    rng: &mut dyn RandomSource,
) -> (GameState, Vec<EngineEvent>) {
    let host = players
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let seats: Vec<Seat> = players
        .iter()
        .map(|p| Seat::new(p, "", &[], 0))
        .collect();
    let mut g = Game::new(seats, cards::deck());
    g.rand_state = rng.next_u64() | 1;
    g.cfg = lobby::LobbyConfig::default();
    g.host = host.clone();
    g.lobby = Some(lobby::Lobby { host: host.clone() });
    let state = state_of(room_id, group_id, players, &g);
    let events = vec![EngineEvent::new(
        "sgs2_lobby",
        json!({ "host": host, "seats": g.seats.len() }),
    )];
    (state, events)
}

fn parse(state: &GameState) -> Result<Game, EngineError> {
    serde_json::from_value::<Game>(state.data.clone())
        .map_err(|e| EngineError::Internal(format!("状态解析失败：{e}")))
}

/// 供 `sandbox` 模块构造对外状态（同 crate 内部；不对外暴露实现细节）。
pub(crate) fn state_of_public(
    room_id: &str,
    group_id: &str,
    players: Vec<String>,
    g: &Game,
) -> GameState {
    state_of(room_id, group_id, players, g)
}

impl Sanguosha2Engine {
    /// 一键开局（测试 / 示例 / 自对弈 / `check:sgs2-ai`）：建局 → 开赛 → 全员自动选将 →
    /// 发牌进入主公回合。**不经过大厅**（不模拟 join/开始，供自动化使用）。
    pub fn new_started_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        let seed = rng.next_u64();
        let (state, mut events) = new_lobby_game(room_id, group_id, players, rng);
        let Ok(mut g) = parse(&state) else {
            return (state, events);
        };
        g.rand_state = seed | 1;
        if let Err(e) = lobby::default_start(&mut g) {
            g.push_log(format!("自动开局失败：{e}"));
        }
        events.push(EngineEvent::new(
            "sgs2_started",
            json!({
                "seats": g.seats.len(),
                "lord": g.seats.iter().position(|s| s.identity == "lord"),
            }),
        ));
        (state_of(room_id, group_id, state.players.clone(), &g), events)
    }
}

impl EnginePlugin for Sanguosha2Engine {
    fn game_id(&self) -> &'static str {
        // OQ1=A（2026-11-10）：v2 接管正式引擎 id `sanguosha`（v1 不再注册）。
        "sanguosha"
    }

    fn rules(&self) -> String {
        "三国杀（内核 v2）：技能=声明+handler，事件带窗口，单一延迟队列，牌结算走真实流程。"
            .to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        new_lobby_game(room_id, group_id, players, rng)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let Ok(g) = parse(state) else {
            return Vec::new();
        };
        // 大厅 / 选将：未入座玩家也能 `lobby:join`（不查座位）。
        if g.lobby.is_some() || g.setup.is_some() {
            return lobby::legal_actions(&g, player_id)
                .into_iter()
                .map(|(id, description)| Action { id, description })
                .collect();
        }
        let Some(seat) = view::seat_of(&g, player_id) else {
            return Vec::new();
        };
        if let Some(p) = &g.pending {
            if p.actor() == seat {
                return p
                    .options()
                    .iter()
                    // 强制选择类待办的占位出口不下发（与视图一致）。
                    .filter(|o| !(p.escape_hidden() && o.id == p.escape_id()))
                    .map(|o| Action {
                        id: o.id.clone(),
                        description: o.label.clone(),
                    })
                    .collect();
            }
            return Vec::new();
        }
        if g.turn != seat || g.phase != Phase::Play {
            // 非当前回合/非出牌阶段：只保留「回合外也可发动」的主动技（矫诏plus/plusplus）。
            return skills::legal_actions_out_of_turn(&g, seat)
                .into_iter()
                .map(|(id, description)| Action { id, description })
                .collect();
        }
        let mut out: Vec<Action> = card_play::legal_uses(&g, seat)
            .into_iter()
            .map(|(id, description)| Action { id, description })
            .collect();
        out.extend(
            skills::legal_actions(&g, seat)
                .into_iter()
                .map(|(id, description)| Action { id, description }),
        );
        out
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        _rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        let mut g = parse(state)?;
        // 大厅 / 选将：动作协议与 v1 一致（`lobby:*` / `setup:choose_general:*`）；
        // 此阶段没有回合可推进，不调 `tick`（否则会推进回合）。
        if g.lobby.is_some() || g.setup.is_some() || action.starts_with("lobby:") {
            let message = lobby::apply(&mut g, player_id, action)
                .map_err(EngineError::IllegalAction)?;
            g.check_conservation().map_err(EngineError::Internal)?;
            g.check_no_orphan_frames().map_err(EngineError::Internal)?;
            let out = state_of(&state.room_id, &state.group_id, state.players.clone(), &g);
            return Ok(ActionResult {
                state: out,
                message,
                finished: g.finished,
                events: Vec::new(),
            });
        }
        let seat = view::seat_of(&g, player_id)
            .ok_or_else(|| EngineError::PermissionDenied("不在座".into()))?;

        let message = if g.pending.is_some() {
            resolve::submit(&mut g, seat, action).map_err(EngineError::IllegalAction)?
        } else if let Some(rest) = action.strip_prefix("use:") {
            let mut parts = rest.split(':');
            let id = parts
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| EngineError::IllegalAction("缺少牌".into()))?
                .to_string();
            let targets: Vec<usize> = parts
                .next()
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| t.parse::<usize>().ok())
                        .collect()
                })
                .unwrap_or_default();
            g.resolve_use(seat, &id, targets)
                .map_err(EngineError::IllegalAction)?;
            format!("使用【{}】", g.name_of(&id))
        } else if let Some(rest) = action.strip_prefix("convert:") {
            // 被动转化技：`convert:<skill>:<承载牌>:<生效牌名>[:目标]`
            let mut parts = rest.split(':');
            let skill = parts.next().unwrap_or_default().to_string();
            let id = parts
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| EngineError::IllegalAction("缺少承载牌".into()))?
                .to_string();
            let name = parts
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| EngineError::IllegalAction("缺少生效牌名".into()))?
                .to_string();
            let targets: Vec<usize> = parts
                .next()
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| t.parse::<usize>().ok())
                        .collect()
                })
                .unwrap_or_default();
            crate::skills::on_conversion_used(&mut g, seat, &skill, &id, &name);
            // 转化技的承载牌可来自手牌或**自己装备区**（界奇袭等）：走非严格通道，
            // 合法性由 `legal_uses` 的 `plan_use` 枚举保证（legal ≡ execute）。
            g.use_as(seat, &id, &name, targets, false, None)
                .map_err(EngineError::IllegalAction)?;
            format!("【{}】当【{}】使用", g.name_of(&id), name)
        } else if let Some(rest) = action.strip_prefix("trim:") {
            // 巧说（-1）：`trim:<牌>:<不指定的目标>`
            let mut parts = rest.split(':');
            let id = parts
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| EngineError::IllegalAction("缺少牌".into()))?
                .to_string();
            let excluded: usize = parts
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| EngineError::IllegalAction("缺少排除目标".into()))?;
            if !crate::skills::can_trim_targets(&g, seat) {
                return Err(EngineError::IllegalAction("当前不能少指定目标".into()));
            }
            g.counters
                .insert("turn.aoe_trim".to_string(), (excluded + 1) as u32);
            g.resolve_use(seat, &id, Vec::new())
                .map_err(EngineError::IllegalAction)?;
            format!("使用【{}】（少指定一个目标）", g.name_of(&id))
        } else if let Some(id) = action.strip_prefix("recast:") {
            let id = id.to_string();
            card_play::resolve_recast(&mut g, seat, &id).map_err(EngineError::IllegalAction)?;
            "重铸".to_string()
        } else if action.starts_with("skill:") {
            skills::execute_active(&mut g, seat, action).map_err(EngineError::IllegalAction)?
        } else if action == "end_play" {
            if g.turn != seat || g.phase != Phase::Play {
                return Err(EngineError::IllegalAction("当前不是你的出牌阶段".into()));
            }
            g.push_log(format!("{} 结束出牌阶段", g.seats[seat].name));
            let next = crate::skills::on_end_play(&mut g, seat);
            resolve::begin_phase(&mut g, seat, next).map_err(EngineError::Internal)?;
            "结束出牌阶段".to_string()
        } else {
            return Err(EngineError::IllegalAction(format!("未知动作：{action}")));
        };

        g.check_conservation().map_err(EngineError::Internal)?;
        // 技能/转化牌的入口只「压帧」不推进（`use_as`/`use_virtual`/`execute_active`）：
        // 动作结束后必须先把结算栈驱动到「需要输入」或清空，再进回合驱动器。
        // 否则会留下「有结算帧但无 pending」的中间态，下一步 `use:` 会报「请先完成当前结算」。
        resolve::advance(&mut g).map_err(EngineError::Internal)?;
        turn::tick(&mut g).map_err(EngineError::Internal)?;
        g.check_conservation().map_err(EngineError::Internal)?;
        g.check_no_orphan_frames().map_err(EngineError::Internal)?;
        g.check_no_stale_answer().map_err(EngineError::Internal)?;
        g.check_no_zombies().map_err(EngineError::Internal)?;
        let out = state_of(&state.room_id, &state.group_id, state.players.clone(), &g);
        Ok(ActionResult {
            state: out,
            message,
            finished: g.finished,
            events: Vec::new(),
        })
    }

    fn public_state(&self, state: &GameState) -> GameState {
        let Ok(g) = parse(state) else {
            return state.clone();
        };
        let mut out = state.clone();
        out.data = view::public_state(&g);
        out
    }

    fn player_view(&self, state: &GameState, viewer: &str) -> GameState {
        let Ok(g) = parse(state) else {
            return state.clone();
        };
        let Some(seat) = view::seat_of(&g, viewer) else {
            return self.public_state(state);
        };
        let mut out = state.clone();
        out.data = view::player_view(&g, seat);
        out
    }

    /// 场景注入（工作台模板/预览）：白名单字段 + 只移动既有牌 + 注入后守恒校验。
    fn apply_scene(
        &self,
        state: &GameState,
        scene: &serde_json::Value,
    ) -> Result<GameState, EngineError> {
        let mut g = parse(state)?;
        sandbox::apply_scene(&mut g, scene).map_err(EngineError::IllegalAction)?;
        Ok(state_of(&state.room_id, &state.group_id, state.players.clone(), &g))
    }

    /// 快照校验（工作台模板/预览/导入用）：牌区守恒 + 无帧遗留 + 待办出口 + 座位/技能合法。
    ///
    /// 返回 `Err` 表示该快照不可用于预览（模板被拒）；不会修改任何状态。
    fn validate_state(&self, state: &GameState) -> Result<(), String> {
        let g = parse(state).map_err(|e| format!("状态解析失败：{e}"))?;
        if g.seats.is_empty() || g.seats.len() > lobby::MAX_PLAYERS {
            return Err(format!("座位数不合法：{}", g.seats.len()));
        }
        if g.cards.len() != cards::DECK_SIZE {
            return Err(format!(
                "牌实例数 {} ≠ 牌堆上限 {}",
                g.cards.len(),
                cards::DECK_SIZE
            ));
        }
        g.check_conservation()?;
        g.check_no_orphan_frames()?;
        g.check_no_stale_answer()?;
        if let Some(p) = &g.pending {
            if p.actor() >= g.seats.len() {
                return Err(format!("待办行动者越界：{}", p.actor()));
            }
            if !p.has_option(p.escape_id()) {
                return Err(format!("待办 {} 缺少出口 {}", p.kind(), p.escape_id()));
            }
        }
        for (i, s) in g.seats.iter().enumerate() {
            if s.name.trim().is_empty() {
                return Err(format!("座位 {i} 缺少玩家 id"));
            }
            if !s.general.is_empty() && generals::by_code(&s.general).is_none() {
                return Err(format!("座位 {i} 武将不存在：{}", s.general));
            }
            for code in s.skills.iter().chain(s.granted.iter()) {
                if skills::def(code).is_none() {
                    return Err(format!("座位 {i} 技能未注册：{code}"));
                }
            }
        }
        Ok(())
    }

    fn summarize(&self, state: &GameState) -> String {
        let Ok(g) = parse(state) else {
            return "三国杀：状态异常".to_string();
        };
        let status = if g.finished { "已结束" } else { "进行中" };
        let stage = if let Some(l) = &g.lobby {
            format!(
                "等待加入（{}/{}）· 房主 {}",
                g.seats.len(),
                lobby::MAX_PLAYERS,
                l.host
            )
        } else if let Some(s) = &g.setup {
            format!(
                "选将中（已选 {}/{}）· 主公已选 {}",
                s.picks.len(),
                g.seats.len(),
                s.lord_picked
            )
        } else if let Some(reason) = &g.abort_reason {
            format!("已中断（{reason}）")
        } else {
            let current = g
                .seats
                .get(g.turn)
                .map(|s| s.name.as_str())
                .unwrap_or("-");
            format!("第 {} 回合 · {:?} · 当前 {}", g.round, g.phase, current)
        };
        let pending = g
            .pending
            .as_ref()
            .map(|p| {
                format!(
                    "等待 {} 响应（{}）",
                    g.seats.get(p.actor()).map(|s| s.name.as_str()).unwrap_or("-"),
                    p.kind()
                )
            })
            .unwrap_or_else(|| "自由行动".to_string());
        format!(
            "三国杀（{status}）：{stage} · {pending} · 存活 {}/{}",
            g.seats.iter().filter(|s| s.hp > 0).count(),
            g.seats.len()
        )
    }
}

#[cfg(test)]
mod smoke {
    use super::*;

    #[test]
    fn engine_plugin_roundtrip() {
        let engine = Sanguosha2Engine;
        let mut rng = game_engine_core::SeededRng::new(7);
        let (state, _) = engine.new_game("r1", "g1", vec!["p0".into(), "p1".into()], &mut rng);
        let actions = engine.legal_actions(&state, "p0");
        assert!(!actions.is_empty());
        let res = engine
            .execute_action(&state, "p0", &actions[0].id, &mut rng)
            .expect("动作应可执行");
        assert!(!res.message.is_empty());
    }
}
