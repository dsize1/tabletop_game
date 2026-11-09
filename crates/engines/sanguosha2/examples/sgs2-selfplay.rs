//! v2 自对弈（M3 AI 最小接口验收）。
//!
//! 只通过 `EnginePlugin`（legal_actions / execute_action）+ 原始 `Game` 状态决策，
//! 不碰内核私有字段。确定性：同一 seed 结果可复现。
//!
//! 用法：
//!   cargo run -p sanguosha2 --example sgs2-selfplay -- --games 10 --players 5 --seed 1 --json
//!
//! 输出（--json）：
//!   { "engine":"sanguosha2", "games":10, "finished":10, "illegal":0, "loops":0,
//!     "stalled":0, "steps":1234, "max_steps":20000, "winners":{...},
//!     "pending_kinds":["card_response","discard_phase",...] }

use std::collections::{BTreeMap, BTreeSet, HashMap};

use game_engine_core::{EnginePlugin, GameState, SeededRng};
use sanguosha2::kernel::Game;
use sanguosha2::Sanguosha2Engine;

fn arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    for (i, a) in args.iter().enumerate() {
        if a == &format!("--{name}") {
            if let Some(v) = args.get(i + 1) {
                if !v.starts_with("--") {
                    return v.clone();
                }
            }
        }
    }
    default.to_string()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == format!("--{name}"))
}

fn parse_state(state: &GameState) -> Game {
    serde_json::from_value(state.data.clone()).expect("状态解析失败")
}

/// 决策策略（最小 AI）：
/// - pending：优先 respond/pick/target/cardtarget/discard，其次 confirm，最后 pass；
///   弃牌阶段必须弃到上限（只提供 discard 选项）。
/// - 出牌阶段：先出牌（`use:*`），再发技能（`skill:*`），最后 `end_play`。
fn choose(actions: &[game_engine_core::Action], g: &Game) -> Option<String> {
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    if g.pending.is_some() {
        let kind = g.pending.as_ref().map(|p| p.kind().to_string()).unwrap_or_default();
        let preference: &[&str] = if kind == "__discard" || kind == "discard_phase" {
            &["discard:", "confirm"]
        } else {
            &[
                "respond:",
                "pick:",
                "cardtarget:",
                "target:",
                "reveal:",
                "discard:",
                "choose:",
                "confirm",
            ]
        };
        for pref in preference {
            if let Some(id) = ids.iter().find(|id| id.starts_with(pref)) {
                return Some((*id).to_string());
            }
        }
        return ids
            .iter()
            .find(|id| **id == "pass")
            .map(|id| (*id).to_string())
            .or_else(|| ids.first().map(|id| (*id).to_string()));
    }
    for pref in ["use:", "skill:", "recast:", "end_play"] {
        if let Some(id) = ids.iter().find(|id| id.starts_with(pref)) {
            return Some((*id).to_string());
        }
    }
    ids.first().map(|id| (*id).to_string())
}

struct Summary {
    games: u32,
    finished: u32,
    illegal: u32,
    loops: u32,
    stalled: u32,
    steps: u64,
    max_steps: u32,
    winners: BTreeMap<String, u32>,
    pending_kinds: BTreeSet<String>,
    errors: Vec<String>,
}

fn main() {
    let games: u32 = arg("games", "10").parse().unwrap_or(10);
    let players: usize = arg("players", "5").parse().unwrap_or(5);
    let seed: u64 = arg("seed", "1").parse().unwrap_or(1);
    let max_steps: u32 = arg("max-steps", "20000").parse().unwrap_or(20000);
    let json_out = flag("json");
    let log_path = arg("log", "");
    let mut decision_log: Vec<serde_json::Value> = Vec::new();
    let mut log_step = 0u32;

    let engine = Sanguosha2Engine;
    let mut s = Summary {
        games,
        finished: 0,
        illegal: 0,
        loops: 0,
        stalled: 0,
        steps: 0,
        max_steps,
        winners: BTreeMap::new(),
        pending_kinds: BTreeSet::new(),
        errors: Vec::new(),
    };

    for game_idx in 0..games {
        log_step = 0;
        let mut rng = SeededRng::new(seed + game_idx as u64 * 7919);
        let players_ids: Vec<String> = (0..players).map(|i| format!("p{i}")).collect();
        let (mut state, _) = engine.new_game(
            &format!("r{game_idx}"),
            "g1",
            players_ids.clone(),
            &mut rng,
        );
        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut steps = 0u32;
        let mut done = false;

        while steps < max_steps {
            let g = parse_state(&state);
            if g.finished {
                done = true;
                let w = g.winner.clone().unwrap_or_else(|| "unknown".to_string());
                *s.winners.entry(w).or_insert(0) += 1;
                break;
            }
            if let Some(p) = &g.pending {
                s.pending_kinds.insert(p.kind().to_string());
            }
            let actor = match &g.pending {
                Some(p) => g.seats[p.actor()].name.clone(),
                None => g.seats[g.turn].name.clone(),
            };
            let actions = engine.legal_actions(&state, &actor);
            let Some(action) = choose(&actions, &g) else {
                s.stalled += 1;
                s.errors.push(format!(
                    "game {game_idx}: no legal action (turn={}, phase={:?}, pending={:?})",
                    g.turn,
                    g.phase,
                    g.pending.as_ref().map(|p| p.kind())
                ));
                break;
            };

            // 环检测：同一（回合/阶段/行动者/动作/体力/手牌/牌堆）重复出现 → 判环。
            let hp: Vec<i32> = g.seats.iter().map(|x| x.hp).collect();
            let hands: Vec<usize> = (0..g.seats.len()).map(|i| g.hand(i).len()).collect();
            let sig = format!(
                "{:?}",
                (
                    g.turn,
                    g.phase,
                    g.pending.as_ref().map(|p| p.kind()),
                    actor.clone(),
                    action.clone(),
                    hp.clone(),
                    hands.clone()
                )
            );
            *seen.entry(sig).or_insert(0) += 1;
            if seen.values().copied().max().unwrap_or(0) > 6 {
                s.loops += 1;
                s.errors.push(format!("game {game_idx}: loop detected at {action}"));
                break;
            }

            // 决策日志（每步一行，供 `check:sgs2-decision-log` 校验/排障）。
            if !log_path.is_empty() {
                log_step += 1;
                decision_log.push(serde_json::json!({
                    "game": game_idx,
                    "step": log_step,
                    "turn": g.turn,
                    "round": g.round,
                    "phase": format!("{:?}", g.phase),
                    "actor": actor,
                    "pending": g.pending.as_ref().map(|p| p.kind().to_string()),
                    "legal_count": actions.len(),
                    "legal": actions.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
                    "chosen": action.clone(),
                    "hp": hp.clone(),
                    "hand_counts": hands.clone(),
                    "deck_count": g.ids_in(|z| *z == sanguosha2::kernel::Zone::Draw).len(),
                }));
            }

            match engine.execute_action(&state, &actor, &action, &mut rng) {
                Ok(res) => {
                    steps += 1;
                    s.steps += 1;
                    state = res.state;
                    if res.finished {
                        let g = parse_state(&state);
                        let w = g.winner.clone().unwrap_or_else(|| "unknown".to_string());
                        *s.winners.entry(w).or_insert(0) += 1;
                        done = true;
                        break;
                    }
                }
                Err(e) => {
                    s.illegal += 1;
                    s.errors
                        .push(format!("game {game_idx}: illegal {action}: {e}"));
                    break;
                }
            }
        }
        if !done {
            s.stalled += 1;
        } else {
            s.finished += 1;
        }
    }

    if !log_path.is_empty() {
        let body = decision_log
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect::<Vec<_>>()
            .join("
");
        if let Err(e) = std::fs::write(&log_path, format!("{body}
")) {
            eprintln!("[sgs2-selfplay] 决策日志写入失败 {log_path}: {e}");
        }
    }

    if json_out {
        let out = serde_json::json!({
            "engine": "sanguosha2",
            "games": s.games,
            "players": players,
            "seed": seed,
            "finished": s.finished,
            "illegal": s.illegal,
            "loops": s.loops,
            "stalled": s.stalled,
            "steps": s.steps,
            "max_steps": s.max_steps,
            "winners": s.winners,
            "pending_kinds": s.pending_kinds,
            "errors": s.errors,
        });
        println!("{}", serde_json::to_string(&out).unwrap());
    } else {
        println!(
            "[sgs2-selfplay] {} 局 / 终局 {} / illegal {} / loops {} / stalled {} / 步数 {}",
            s.games, s.finished, s.illegal, s.loops, s.stalled, s.steps
        );
        println!("  winners: {:?}", s.winners);
        println!("  pending kinds: {:?}", s.pending_kinds);
        for e in &s.errors {
            println!("  ! {e}");
        }
    }
}
