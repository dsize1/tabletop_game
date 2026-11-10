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
//! 本 crate 在迁移完成前**不注册**进 `game-engine-mcp`（见计划 O3）。

pub mod cards;
pub mod cards_generated;
#[path = "generals_generated.rs"]
pub mod generals;
pub mod kernel;
pub mod presentation;
pub mod skills;
pub mod turn;
pub mod view;

use game_engine_core::{
    shuffle, Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus,
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

fn seat_template(i: usize) -> (&'static str, &'static [&'static str], i32) {
    match i {
        0 => ("zhoufei", &["liangyin", "kongsheng"], 3),
        1 => ("luzhi", &["zhenliang"], 4),
        2 => ("caoren", &["jushou", "jiewei"], 4),
        3 => ("zhoutai", &["buqu", "fenji"], 4),
        _ => ("guest", &[], 4),
    }
}

fn seats_for(players: &[String]) -> Vec<Seat> {
    let mut seats: Vec<Seat> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (general, skills, hp) = seat_template(i);
            Seat::new(p, general, skills, hp)
        })
        .collect();
    if seats.is_empty() {
        seats.push(Seat::new("p0", "zhoufei", &["liangyin", "kongsheng"], 3));
    }
    // 身份：第 0 位主公，其余反贼（M3 自对弈/胜负判定用；完整身份局在 M4 之后扩展）。
    seats[0].identity = "lord".to_string();
    seats[0].flags.insert("lord".to_string());
    for s in seats.iter_mut().skip(1) {
        s.identity = "rebel".to_string();
    }
    seats
}

fn state_of(
    room_id: &str,
    group_id: &str,
    players: Vec<String>,
    g: &Game,
) -> GameState {
    GameState {
        game_id: "sanguosha2".into(),
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

fn parse(state: &GameState) -> Result<Game, EngineError> {
    serde_json::from_value::<Game>(state.data.clone())
        .map_err(|e| EngineError::Internal(format!("状态解析失败：{e}")))
}

impl EnginePlugin for Sanguosha2Engine {
    fn game_id(&self) -> &'static str {
        "sanguosha2"
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
        let seats = seats_for(&players);
        let mut deck = cards::deck();
        shuffle(rng, &mut deck);
        let mut g = Game::new(seats, deck);
        // 初始手牌 4 张（主公 +1 留到身份局细化）。
        for i in 0..g.seats.len() {
            g.draw(i, 4);
        }
        if let Err(e) = turn::begin_game(&mut g) {
            g.push_log(format!("开局推进失败：{e}"));
        }
        let state = state_of(room_id, group_id, players, &g);
        let mut events = vec![EngineEvent::new("sgs2_started", json!({}))];
        if let Some(p) = &g.pending {
            events.push(EngineEvent::new(
                "sgs2_pending",
                json!({ "actor": g.seats[p.actor()].name, "kind": p.kind(), "skill": p.skill() }),
            ));
        }
        (state, events)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let Ok(g) = parse(state) else {
            return Vec::new();
        };
        let Some(seat) = view::seat_of(&g, player_id) else {
            return Vec::new();
        };
        if let Some(p) = &g.pending {
            if p.actor() == seat {
                return p
                    .options()
                    .iter()
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
            g.use_as(seat, &id, &name, targets, true, None)
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
            let next = crate::skills::on_end_play(&mut g, seat);
            resolve::begin_phase(&mut g, seat, next).map_err(EngineError::Internal)?;
            "结束出牌阶段".to_string()
        } else {
            return Err(EngineError::IllegalAction(format!("未知动作：{action}")));
        };

        g.check_conservation().map_err(EngineError::Internal)?;
        turn::tick(&mut g).map_err(EngineError::Internal)?;
        g.check_conservation().map_err(EngineError::Internal)?;
        g.check_no_orphan_frames().map_err(EngineError::Internal)?;
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

    fn summarize(&self, state: &GameState) -> String {
        let Ok(g) = parse(state) else {
            return "三国杀：状态异常".to_string();
        };
        let status = if g.finished { "已结束" } else { "进行中" };
        let current = g
            .seats
            .get(g.turn)
            .map(|s| s.name.as_str())
            .unwrap_or("-");
        let pending = g
            .pending
            .as_ref()
            .map(|p| format!("等待 {}", g.seats[p.actor()].name))
            .unwrap_or_else(|| "自由行动".to_string());
        format!(
            "三国杀（{status}）：第 {} 回合 · {:?} · 当前 {} · {} · 存活 {}/{}",
            g.round,
            g.phase,
            current,
            pending,
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
